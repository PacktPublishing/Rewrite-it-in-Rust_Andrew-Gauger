//! How long a frame takes to build: scrolling the Sales Orders list, all
//! 13,531 placeholder orders, and refreshing the Customers list while the
//! frames run. Build time only: the drawing happens on the GPU
//! afterwards. Reads the dev database, so it's ignored by default:
//!
//!   cargo test --release -p cairn-app --test scroll_timing -- --ignored --nocapture

mod common;

use cairn_app::app::CairnApp;
use cairn_app::context::{AppContext, Command, Saved, Target};
use cairn_app::data::{self, Rows, Table};
use cairn_app::orchestrator::Orchestrator;
use cairn_app::save;
use cairn_app::worker::{self, Workers, CAPACITY};
use egui::{pos2, vec2, Event, Modifiers, MouseWheelUnit};
use egui_kittest::kittest::{by, Queryable};
use egui_kittest::Harness;
use sqlx::PgPool;
use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

#[test]
#[ignore = "reads the dev database"]
fn scrolling_13531_orders() {
    let url = std::env::var("CAIRN_DATABASE_URL")
        .unwrap_or_else(|_| data::DEV_DATABASE_URL.into());
    let mut app =
        CairnApp::new(AppContext::new(data::load(&url).unwrap()));
    app.open(Target::OrderList);
    let rows = app.ctx.data.orders.len();
    let times = RefCell::new(Vec::<Duration>::new());
    let mut h = Harness::builder()
        .with_size([1180.0, 760.0])
        .build_ui_state(
            |ui, app: &mut CairnApp| {
                let start = Instant::now();
                app.show(ui);
                times.borrow_mut().push(start.elapsed());
            },
            app,
        );
    h.run();
    times.borrow_mut().clear();
    h.hover_at(pos2(400.0, 400.0));
    // Fling to the bottom, about 30 rows a frame, until the oldest order
    // (last in the list) is on screen.
    let oldest =
        h.state().ctx.data.orders.last().unwrap().number.clone();
    let on_screen = |h: &Harness<'_, CairnApp>| {
        h.root()
            .query_all(by().predicate(|n| {
                n.value().is_some_and(|v| v == oldest)
            }))
            .next()
            .is_some()
    };
    let mut frames = 0;
    while !on_screen(&h) {
        assert!(frames < 2_000, "didn't reach {oldest}");
        h.event(Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: vec2(0.0, -600.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
            source: egui::MouseWheelSource::Wheel,
        });
        h.step();
        frames += 1;
    }
    let mut t = times.borrow().clone();
    t.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!(
        "{rows} orders, {} frames: median {:.3} ms, 99th percentile {:.3} ms, max {:.3} ms",
        t.len(),
        ms(t[t.len() / 2]),
        ms(t[t.len() * 99 / 100]),
        ms(t[t.len() - 1]),
    );
}

/// How long the largest datasheet view, 68,257 activities, takes to
/// build when it opens: every field turned into text once.
#[test]
#[ignore = "reads the dev database"]
fn opening_the_activities_datasheet() {
    use cairn_app::screens::sheet::{Sheet, SheetOf};
    let url = std::env::var("CAIRN_DATABASE_URL")
        .unwrap_or_else(|_| data::DEV_DATABASE_URL.into());
    let ctx = AppContext::new(data::load(&url).unwrap());
    let start = Instant::now();
    let _sheet = Sheet::build(SheetOf::Activities, &ctx);
    println!(
        "{} activities: datasheet built in {:.0} ms",
        ctx.data.activities.len(),
        start.elapsed().as_secs_f64() * 1000.0
    );
}

/// Where H1's runs connect: `dev-latency`, started first with the delay
/// to measure, forwarding to the dev database.
///
///   ./target/debug/dev-latency --delay-ms 300
///   cargo test --release -p cairn-app --test scroll_timing blocking -- --ignored --nocapture
///   cargo test --release -p cairn-app --test scroll_timing off_the -- --ignored --nocapture
const PROXY: &str =
    "postgres://cairn_owner:cairn_owner_dev@127.0.0.1:5434/cairn";

/// 180 frames, three seconds at 60 a second, with a refresh of the
/// Customers list on every 45th.
const FRAMES: usize = 180;
const EVERY: usize = 45;

/// H1's naive version: the frame waits for Postgres. `block_on` on a
/// runtime of the test's own, the way `data::load` runs before the
/// window opens.
fn refresh_blocking(rt: &Runtime, db: &PgPool, ctx: &mut AppContext) {
    let rows = Rows::Table(Table::Customers);
    let fresh = rt.block_on(data::read(db, rows)).unwrap();
    if ctx.data.merge(rows, fresh) {
        ctx.rebuild();
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Median and maximum frame time, and how many frames missed 60 a
/// second's 16.67 ms.
fn report(label: &str, times: &[Duration]) {
    let refreshes: Vec<String> = times[..FRAMES]
        .iter()
        .step_by(EVERY)
        .map(|d| format!("{:.1}", ms(*d)))
        .collect();
    let mut t = times.to_vec();
    t.sort();
    let slow = t.iter().filter(|d| ms(**d) > 16.67).count();
    println!(
        "{label}: {} frames, median {:.3} ms, max {:.3} ms, {slow} over 16.67 ms\n  \
         frames {}: {} ms",
        t.len(),
        ms(t[t.len() / 2]),
        ms(t[t.len() - 1]),
        (0..FRAMES)
            .step_by(EVERY)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", "),
        refreshes.join(", "),
    );
}

/// The Customers form, a frame at a time, with the refresh inside the
/// frame on an open connection or a new one each time.
fn blocking_frames(new_connection: bool) -> Vec<Duration> {
    let url = std::env::var("CAIRN_DATABASE_URL")
        .unwrap_or_else(|_| data::DEV_DATABASE_URL.into());
    let mut app =
        CairnApp::new(AppContext::new(data::load(&url).unwrap()));
    app.open(Target::Customer(None));
    let rt = Runtime::new().unwrap();
    let db = rt.block_on(data::connect(PROXY)).unwrap();
    let times = RefCell::new(Vec::new());
    // The harness's first frames lay the screen out, and refresh
    // nothing; then the 180 count from 0.
    let frame = RefCell::new(1_usize);
    let mut h = Harness::builder()
        .with_size([1180.0, 760.0])
        .build_ui_state(
            |ui, app: &mut CairnApp| {
                let start = Instant::now();
                let n = frame.replace_with(|n| *n + 1);
                if n.is_multiple_of(EVERY) && new_connection {
                    let db =
                        rt.block_on(data::connect(PROXY)).unwrap();
                    refresh_blocking(&rt, &db, &mut app.ctx);
                    rt.block_on(db.close());
                } else if n.is_multiple_of(EVERY) {
                    refresh_blocking(&rt, &db, &mut app.ctx);
                }
                app.show(ui);
                times.borrow_mut().push(start.elapsed());
            },
            app,
        );
    h.run();
    times.borrow_mut().clear();
    frame.replace(0);
    for _ in 0..FRAMES {
        let start = Instant::now();
        h.step();
        pace(start);
    }
    drop(h);
    times.into_inner()
}

/// Wait out the rest of a 60th of a second, as a window does between
/// frames. Both runs are paced the same: back to back, with no idle
/// time between them, a frame takes about a fifth as long (the CPU
/// stays busy and its caches warm), which no real window sees.
fn pace(start: Instant) {
    let frame = Duration::from_micros(16_667);
    std::thread::sleep(frame.saturating_sub(start.elapsed()));
}

/// What a refresh costs the frame even when Postgres answers at once:
/// merging the rows and rebuilding every lookup and total.
fn rebuild_alone() {
    let url = std::env::var("CAIRN_DATABASE_URL")
        .unwrap_or_else(|_| data::DEV_DATABASE_URL.into());
    let mut ctx = AppContext::new(data::load(&url).unwrap());
    let mut t: Vec<Duration> = (0..5)
        .map(|_| {
            let start = Instant::now();
            ctx.rebuild();
            start.elapsed()
        })
        .collect();
    t.sort();
    println!("AppContext::rebuild alone: median {:.3} ms", ms(t[2]));
}

#[test]
#[ignore = "needs dev-latency on 127.0.0.1:5434"]
fn blocking_refresh_in_the_frame() {
    println!("through {}", &PROXY[PROXY.rfind('@').unwrap() + 1..]);
    rebuild_alone();
    report(
        "refresh in the frame, open connection",
        &blocking_frames(false),
    );
    report(
        "refresh in the frame, new connection",
        &blocking_frames(true),
    );
}

/// The same frames with the refresh sent to a worker: the frame asks
/// and moves on. Paced at 60 a second, so the answers have time to
/// arrive, and run on until the last one has. As a rep, so the frames
/// can save: on frame 60, a new customer through the form's save path;
/// before frame 135, another written outside the app, so that refresh
/// brings something new. Returns the frame times, and the frames where
/// an answer arrived and where a rebuilt context was swapped in.
fn worker_frames() -> (Vec<Duration>, Vec<usize>, Vec<usize>, usize) {
    let url = std::env::var("CAIRN_DATABASE_URL")
        .unwrap_or_else(|_| data::DEV_DATABASE_URL.into());
    let ctx = AppContext::new(data::load(&url).unwrap());
    let rt = Runtime::new().unwrap();
    let rep = rt.block_on(common::rep::sign_in(1));
    let through = format!(
        "postgres://{}:{}@127.0.0.1:5434/cairn",
        rep.role, rep.password
    );
    let outside = rt.block_on(rep.pool());
    let _cleanup = common::rep::Cleanup(rep);
    let db = {
        let _rt = rt.enter();
        data::pool(through.parse().unwrap())
    };
    let w = Arc::new(Workers::signed_in(db));
    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    rt.spawn(worker::run(
        from_ui,
        to_ui,
        4,
        || {},
        move |r| worker::handle(w.clone(), r),
    ));
    let mut app = CairnApp::new(ctx)
        .with(Orchestrator::new(requests, responses));
    app.open(Target::Customer(None));
    let times = RefCell::new(Vec::new());
    let swapped = RefCell::new(Vec::new());
    let arrived = RefCell::new(Vec::new());
    let out = |app: &CairnApp| {
        app.orchestrator.as_ref().unwrap().in_flight()
    };
    // The harness's first frames lay the screen out, and refresh
    // nothing; then the 180 count from 0.
    let frame = RefCell::new(1_usize);
    let mut h = Harness::builder()
        .with_size([1180.0, 760.0])
        .build_ui_state(
            |ui, app: &mut CairnApp| {
                let start = Instant::now();
                let n = frame.replace_with(|n| *n + 1);
                if n.is_multiple_of(EVERY) && n < FRAMES {
                    app.read(Rows::Table(Table::Customers));
                }
                if n == SAVE_AT {
                    app.run(Command::save(new_customer(
                        "Frame Save Co",
                    )));
                }
                // A swapped-in context brings its own rows.
                let (asked, rows) =
                    (out(app), app.ctx.data.customers.as_ptr());
                app.show(ui);
                times.borrow_mut().push(start.elapsed());
                for _ in out(app)..asked {
                    arrived.borrow_mut().push(n);
                }
                if app.ctx.data.customers.as_ptr() != rows {
                    swapped.borrow_mut().push(n);
                }
            },
            app,
        );
    h.run_steps(2);
    times.borrow_mut().clear();
    arrived.borrow_mut().clear();
    frame.replace(0);
    let busy = |h: &Harness<'_, CairnApp>| {
        out(h.state()) > 0 || h.state().building()
    };
    let mut written = false;
    while *frame.borrow() < FRAMES || busy(&h) {
        assert!(*frame.borrow() < 2_000, "no answer");
        if !written && *frame.borrow() >= WRITE_AT {
            rt.block_on(insert_customer(
                &outside,
                "Frame Outside Co",
            ));
            written = true;
        }
        let start = Instant::now();
        h.step();
        pace(start);
    }
    let stale = h.state().orchestrator.as_ref().unwrap().stale;
    drop(h);
    rt.block_on(outside.close());
    let arrived = arrived.into_inner();
    (times.into_inner(), arrived, swapped.into_inner(), stale)
}

/// The frame a save goes out on, and the frame before which a customer
/// is written outside the app, ahead of the refresh on frame 135.
const SAVE_AT: usize = 60;
const WRITE_AT: usize = 100;

fn new_customer(name: &str) -> Saved {
    let mut c = common::customer(0, name, false, 1);
    c.id = None;
    Saved::Customer {
        customer: Box::new(c),
        contacts: Vec::new(),
        activities: Vec::new(),
        removed_contacts: Vec::new(),
        removed_activities: Vec::new(),
    }
}

async fn insert_customer(db: &PgPool, name: &str) {
    let Saved::Customer { customer, .. } = new_customer(name) else {
        unreachable!()
    };
    let saved = Saved::Customer {
        customer,
        contacts: Vec::new(),
        activities: Vec::new(),
        removed_contacts: Vec::new(),
        removed_activities: Vec::new(),
    };
    save::save(db, saved).await.unwrap();
}

#[test]
#[ignore = "needs dev-latency on 127.0.0.1:5434"]
fn refresh_off_the_ui_thread() {
    println!("through {}", &PROXY[PROXY.rfind('@').unwrap() + 1..]);
    let (times, arrived, swapped, stale) = worker_frames();
    report("refresh on a worker", &times);
    println!(
        "  answers arrived at frames {arrived:?}: {} drawn, \
         {stale} dropped as stale",
        arrived.len() - stale
    );
    println!(
        "  a save went out on frame {SAVE_AT}; a customer was written \
         outside the app before frame {WRITE_AT}"
    );
    for n in &swapped {
        println!(
            "  frame {n}: rebuilt context swapped in, {:.3} ms",
            ms(times[*n])
        );
    }
}
