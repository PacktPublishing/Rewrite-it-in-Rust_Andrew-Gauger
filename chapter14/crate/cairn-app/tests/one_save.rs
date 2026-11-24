//! One save per record in flight (Chapter 12): a second save of a
//! record waits for the first, and goes with the version the first was
//! stored at. Without that, both carry the version the rep read and the
//! second is refused by the check Chapter 12 added. Over a fake worker,
//! no database.
//!
//!   cargo test -p cairn-app --test one_save -- --nocapture

mod common;

use cairn_app::context::Saved;
use cairn_app::orchestrator::{Orchestrator, ScreenId};
use cairn_app::save::Stored;
use cairn_app::worker::{Request, Response, CAPACITY};
use rust_decimal::Decimal;
use tokio::sync::mpsc::{self, Receiver, Sender};

struct Fake {
    requests: Receiver<Request>,
    responses: Sender<Response>,
}

fn fake() -> (Orchestrator, Fake) {
    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    (
        Orchestrator::new(requests, responses),
        Fake {
            requests: from_ui,
            responses: to_ui,
        },
    )
}

impl Fake {
    fn take(&mut self) -> Vec<Request> {
        std::iter::from_fn(|| self.requests.try_recv().ok()).collect()
    }
}

/// The sensor, saved at the version the rep read (1) with a new price.
fn sensor(cents: i64) -> Saved {
    let mut p = common::product(11, "CS-TH050", "Sensor", 7900);
    p.unit_price = Decimal::new(cents, 2);
    Saved::Product(p)
}

fn version_of(r: &Request) -> i32 {
    match r {
        Request::Save { saved, .. } => match &**saved {
            Saved::Product(p) => p.version,
            _ => unreachable!(),
        },
        _ => unreachable!(),
    }
}

fn stored(r: Request, version: i32) -> Response {
    let Request::Save { id, mut saved } = r else {
        unreachable!()
    };
    if let Saved::Product(p) = &mut *saved {
        p.version = version;
    }
    Response::Saved {
        id,
        stored: Stored {
            key: 11,
            number: None,
            version,
        },
        saved,
    }
}

#[test]
fn a_second_save_of_a_record_waits_and_goes_with_the_new_version() {
    let (mut o, mut worker) = fake();
    o.save(sensor(8500), ScreenId(1));
    o.save(sensor(9000), ScreenId(2));
    let sent = worker.take();
    println!("sent now: {} (second held)", sent.len());
    assert_eq!(sent.len(), 1, "the second waits for the first");
    assert_eq!(version_of(&sent[0]), 1);

    // The first is stored at version 2.
    let first = sent.into_iter().next().unwrap();
    worker.responses.try_send(stored(first, 2)).unwrap();
    o.poll();
    let sent = worker.take();
    assert_eq!(sent.len(), 1, "now the second goes");
    println!(
        "then: the second, rebased to version {}",
        version_of(&sent[0])
    );
    assert_eq!(version_of(&sent[0]), 2);
}

#[test]
fn saves_of_different_records_go_together() {
    let (mut o, mut worker) = fake();
    o.save(sensor(8500), ScreenId(1));
    let mut other = common::product(12, "CS-GW200", "Gateway", 49900);
    other.unit_price = Decimal::new(45000, 2);
    o.save(Saved::Product(other), ScreenId(1));
    assert_eq!(worker.take().len(), 2);
}

#[test]
fn a_failed_first_save_lets_the_second_go_as_it_was() {
    let (mut o, mut worker) = fake();
    o.save(sensor(8500), ScreenId(1));
    o.save(sensor(9000), ScreenId(2));
    let first = worker.take().remove(0);
    worker
        .responses
        .try_send(Response::Failed {
            id: first.id(),
            error: cairn_app::worker::Failure::Database(
                sqlx::Error::PoolTimedOut,
            ),
        })
        .unwrap();
    o.poll();
    let sent = worker.take();
    assert_eq!(sent.len(), 1);
    assert_eq!(
        version_of(&sent[0]),
        1,
        "nothing was stored to rebase on"
    );
}

/// What the check does without the rule: both saves carry version 1.
/// The first is stored at 2, so the second is refused by Chapter 12's
/// version check.
#[test]
fn without_the_rule_both_go_at_version_one() {
    let (o, mut worker) = fake();
    let mut o = o.with_unordered_saves();
    o.save(sensor(8500), ScreenId(1));
    o.save(sensor(9000), ScreenId(2));
    let sent = worker.take();
    let versions: Vec<i32> = sent.iter().map(version_of).collect();
    println!("unordered: both carry version {versions:?}");
    assert_eq!(versions, vec![1, 1]);
}
