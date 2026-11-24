//! The worker loop and its channels, with handlers that stand in for
//! Postgres: no database, no network.
//!
//!   cargo test -p cairn-app --test worker -- --nocapture

mod common;

use cairn_app::context::Saved;
use cairn_app::data::{self, Rows, Snapshot, Table};
use cairn_app::orchestrator::{Orchestrator, ScreenId};
use cairn_app::save::{self, Stored};
use cairn_app::worker::{
    self, Request, RequestId, Response, Workers, CAPACITY,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Semaphore};

/// A save as the worker answers it, without Postgres.
fn stored(request: Request) -> Response {
    let Request::Save { id, saved } = request else {
        panic!("a save");
    };
    Response::Saved {
        id,
        stored: Stored {
            key: id.0 as i32,
            number: None,
            version: 1,
        },
        saved,
    }
}

fn new_product(n: usize) -> Saved {
    let mut p =
        common::product(0, &format!("T-{n:03}"), "Burst", 100);
    p.id = None;
    Saved::Product(p)
}

#[test]
fn a_panicking_task_is_answered_and_the_rest_carry_on() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, mut responses) = mpsc::channel(CAPACITY);
    rt.spawn(worker::run(
        from_ui,
        to_ui,
        4,
        || {},
        |r| async move {
            match r {
                Request::Read {
                    id: RequestId(1), ..
                } => {
                    panic!("a bug in the read")
                }
                Request::Read { id, rows } => Response::Read {
                    id,
                    rows,
                    data: Box::default(),
                },
                save => stored(save),
            }
        },
    ));
    let customers = Rows::Table(Table::Customers);
    for id in [1, 2] {
        let id = RequestId(id);
        requests
            .try_send(Request::Read {
                id,
                rows: customers,
            })
            .unwrap();
    }
    let mut answers: Vec<Response> = (0..2)
        .map(|_| rt.block_on(responses.recv()).unwrap())
        .collect();
    answers.sort_by_key(Response::id);
    for a in &answers {
        match a {
            Response::Failed { id, error } => {
                println!("{id:?}: Failed: {error}")
            }
            other => println!("{:?}: answered", other.id()),
        }
    }
    assert!(matches!(
        &answers[0],
        Response::Failed { id: RequestId(1), error }
            if error.to_string().contains("a bug in the read")
    ));
    assert!(matches!(answers[1], Response::Read { .. }));
}

#[test]
fn a_full_channel_says_so_at_once() {
    let (tx, _rx) = mpsc::channel::<Request>(CAPACITY);
    let mut results = Vec::new();
    for n in 0..50 {
        let saved = Box::new(new_product(n));
        let id = RequestId(n as u64 + 1);
        let start = Instant::now();
        let r = tx.try_send(Request::Save { id, saved });
        results.push((r.is_ok(), start.elapsed()));
    }
    let sent = results.iter().filter(|(ok, _)| *ok).count();
    let slowest = results.iter().map(|(_, t)| *t).max().unwrap();
    println!(
        "bounded, capacity {CAPACITY}: 50 try_send, {sent} accepted, \
         {} Full (the first Full was #{}), slowest call {:?}",
        50 - sent,
        sent + 1,
        slowest
    );
    assert_eq!(sent, CAPACITY);

    let (tx, _rx) = mpsc::unbounded_channel::<Request>();
    for n in 0..50 {
        let saved = Box::new(new_product(n));
        let id = RequestId(n as u64 + 1);
        tx.send(Request::Save { id, saved }).unwrap();
    }
    println!(
        "unbounded: 50 send, 50 accepted, 50 held in memory, \
         and nothing told the sender to slow down"
    );
}

/// H3 P5: fifty saves in one frame (Save & New held down). The first
/// four run; the channel holds eight; the orchestrator holds the rest
/// and the screen says so. None is dropped.
#[test]
fn fifty_saves_in_one_frame() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    // Postgres is slow: nothing finishes until the gate opens.
    let gate = Arc::new(Semaphore::new(0));
    let g = gate.clone();
    rt.spawn(worker::run(
        from_ui,
        to_ui,
        4,
        || {},
        move |r| {
            let g = g.clone();
            async move {
                drop(g.acquire().await.unwrap());
                tokio::time::sleep(Duration::from_millis(5)).await;
                stored(r)
            }
        },
    ));
    let mut o = Orchestrator::new(requests, responses);
    let screen = ScreenId(1);
    for n in 0..50 {
        o.save(new_product(n), screen);
    }
    // Let the worker take what it can, then one frame's poll.
    std::thread::sleep(Duration::from_millis(100));
    o.poll();
    let running = o.in_flight() - o.in_channel();
    println!(
        "after one frame: {running} running, {} in the channel, \
         {} waiting in the orchestrator; status {:?}",
        o.in_channel(),
        o.waiting(),
        o.status().unwrap()
    );
    assert_eq!(
        (running, o.in_channel(), o.waiting()),
        (4, CAPACITY, 50 - 4 - CAPACITY)
    );

    gate.add_permits(50);
    let (mut saved, mut frames) = (0, 0);
    let start = Instant::now();
    while saved < 50 {
        assert!(frames < 1_000, "{saved} of 50 saved");
        std::thread::sleep(Duration::from_millis(16));
        saved += o.poll().len();
        frames += 1;
    }
    println!(
        "all 50 saved after {frames} frames ({:.0} ms); status {:?}, \
         {} failures",
        start.elapsed().as_secs_f64() * 1e3,
        o.status(),
        o.failures().len()
    );
    assert_eq!(o.status(), None);
    assert!(o.failures().is_empty());
}

/// H3 P3: a task is one allocation the size of its future. A thread's
/// default stack is 2 MiB (`std::thread` docs).
#[test]
fn the_size_of_a_worker_task() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    // A pool that never connects: the futures are only measured.
    let db = {
        let _rt = rt.enter();
        data::pool_options()
            .connect_lazy("postgres://nobody@127.0.0.1:1/none")
            .unwrap()
    };
    let read = Request::Read {
        id: RequestId(1),
        rows: Rows::All,
    };
    let w = Arc::new(Workers::signed_in(db.clone()));
    let handle = worker::handle(w, read);
    let saving = save::save(&db, new_product(1));
    let reading = data::read(&db, Rows::All);
    println!(
        "worker::handle future: {} bytes",
        std::mem::size_of_val(&handle)
    );
    println!(
        "  of which save::save: {} bytes, data::read: {} bytes",
        std::mem::size_of_val(&saving),
        std::mem::size_of_val(&reading)
    );
    println!(
        "Request: {} bytes, Response: {} bytes, Snapshot: {} bytes",
        std::mem::size_of::<Request>(),
        std::mem::size_of::<Response>(),
        std::mem::size_of::<Snapshot>()
    );
    println!(
        "a thread's default stack: {} bytes (2 MiB)",
        2 * 1024 * 1024
    );
    assert!(std::mem::size_of_val(&handle) < 2 * 1024 * 1024);
}
