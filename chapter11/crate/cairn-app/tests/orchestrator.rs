//! The App Orchestrator against a fake worker: the test holds the far
//! ends of both channels and answers by hand, in whatever order it
//! likes. No database, no network, no runtime.
//!
//!   cargo test -p cairn-app --test orchestrator -- --nocapture

mod common;

use cairn_app::app::CairnApp;
use cairn_app::context::{AppContext, Saved, Target};
use cairn_app::data::{Clock, Rows, Snapshot, Table};
use cairn_app::manufacturer::Sent;
use cairn_app::orchestrator::{Orchestrator, ScreenId};
use cairn_app::save::Stored;
use cairn_app::worker::{Failure, Request, Response, CAPACITY};
use cairn_core::{OrderId, ProductId};
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use tokio::sync::mpsc::{self, Receiver, Sender};

/// The worker's side of the channels.
struct Fake {
    requests: Receiver<Request>,
    responses: Sender<Response>,
}

fn fake() -> (Orchestrator, Fake) {
    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    let o = Orchestrator::new(requests, responses);
    let f = Fake {
        requests: from_ui,
        responses: to_ui,
    };
    (o, f)
}

impl Fake {
    /// Everything the UI has sent so far.
    fn take(&mut self) -> Vec<Request> {
        std::iter::from_fn(|| self.requests.try_recv().ok()).collect()
    }

    fn answer(&self, response: Response) {
        self.responses.try_send(response).unwrap();
    }

    /// A save, stored with key `key`.
    fn store(&self, request: Request, key: i32) {
        let Request::Save { id, mut saved } = request else {
            panic!("a save");
        };
        if let Saved::Product(p) = &mut *saved {
            p.id = Some(ProductId(key));
        }
        let stored = Stored {
            key,
            number: None,
            version: 2,
        };
        self.answer(Response::Saved { id, stored, saved });
    }
}

fn customers(names: &[&str]) -> Box<Snapshot> {
    let customers = names
        .iter()
        .enumerate()
        .map(|(i, n)| common::customer(800 + i as i32, n, false, 1))
        .collect();
    Box::new(Snapshot {
        customers,
        ..Snapshot::default()
    })
}

/// Frames until the answers are in, and the context rebuilt from them
/// is swapped in.
fn settle(h: &mut Harness<'_, CairnApp>) {
    h.run_steps(2);
    h.state_mut().settle();
    h.run_steps(2);
}

fn names(app: &CairnApp) -> Vec<String> {
    let d = &app.ctx.data;
    d.customers.iter().map(|c| c.company_name.clone()).collect()
}

/// H4 P3: two refreshes of the Customers list. The first is slow; its
/// answer arrives after the second's and is dropped.
#[test]
fn a_slow_answer_overtaken_by_a_newer_one_is_dropped() {
    let (o, mut worker) = fake();
    let mut app = CairnApp::new(common::context()).with(o);
    let list = Rows::Table(Table::Customers);
    app.read(list);
    app.read(list);
    let sent = worker.take();
    let [first, second] = &sent[..] else {
        panic!("two requests")
    };
    println!("sent {:?} and {:?}", first.id(), second.id());

    // The second answer arrives first: one customer has gone.
    worker.answer(Response::Read {
        id: second.id(),
        rows: list,
        data: customers(&["Bay Dairy"]),
    });
    let mut h = Harness::builder()
        .build_ui_state(|ui, a: &mut CairnApp| a.show(ui), app);
    settle(&mut h);
    println!("{:?} arrives: {:?}", second.id(), names(h.state()));

    // Then the first, slow one, with the list as it was before.
    worker.answer(Response::Read {
        id: first.id(),
        rows: list,
        data: customers(&["Bay Dairy", "Shasta Creamery"]),
    });
    settle(&mut h);
    let stale = h.state().orchestrator.as_ref().unwrap().stale;
    println!(
        "{:?} arrives late: {:?}, {stale} answer dropped as stale",
        first.id(),
        names(h.state())
    );
    assert_eq!(names(h.state()), ["Bay Dairy"]);
    assert_eq!(stale, 1);
}

/// H4 P4: a policy per kind, with the channel full of saves.
#[test]
fn a_save_waits_a_refresh_coalesces_a_lookup_replaces() {
    let (mut o, mut worker) = fake();
    let screen = ScreenId(1);
    let product = |n: i32| {
        let p = common::product(n, &format!("T-{n}"), "Sensor", 100);
        Saved::Product(p)
    };
    for n in 0..10 {
        o.save(product(n), screen);
    }
    println!(
        "10 saves: {} in the channel, {} waiting; {:?}",
        o.in_channel(),
        o.waiting(),
        o.status().unwrap()
    );
    assert_eq!((o.in_channel(), o.waiting()), (CAPACITY, 2));

    let list = Rows::Table(Table::Customers);
    let a = o.read(list, screen);
    let b = o.read(list, screen);
    println!(
        "two refreshes: {a:?} and {b:?}, {} waiting",
        o.waiting()
    );
    assert_eq!((a, b, o.waiting()), (a, a, 3));

    let shasta = Rows::Record(Table::Customers, 766);
    let bay = Rows::Record(Table::Customers, 767);
    o.read(shasta, screen);
    let picked = o.read(bay, screen);
    println!("two lookups: {} waiting", o.waiting());
    assert_eq!(o.waiting(), 4);

    // The worker drains the channel; each frame sends what fits.
    let mut sent = Vec::new();
    while o.waiting() > 0 || o.in_channel() > 0 {
        sent.extend(worker.take());
        o.poll();
    }
    let reads: Vec<String> = sent
        .iter()
        .filter_map(|r| match r {
            Request::Read { id, rows } => {
                Some(format!("{id:?} {rows:?}"))
            }
            _ => None,
        })
        .collect();
    let saves = sent.len() - reads.len();
    println!("sent: {saves} saves, then {reads:?}");
    assert_eq!(saves, 10);
    assert_eq!(
        reads,
        [format!("{a:?} {list:?}"), format!("{picked:?} {bay:?}")]
    );
}

/// H4 P5: a failure is a value the status line draws, and the save it
/// failed is kept for Try again.
#[test]
fn a_failed_save_shows_and_can_be_sent_again() {
    let (mut o, mut worker) = fake();
    let p = common::product(11, "CS-TH050", "Sensor", 8500);
    o.save(Saved::Product(p.clone()), ScreenId(1));
    let sent = worker.take().remove(0);
    worker.answer(Response::Failed {
        id: sent.id(),
        error: Failure::Database(sqlx::Error::PoolTimedOut),
    });
    assert!(o.poll().is_empty());
    let failed = &o.failures()[0];
    println!("status line: {}", failed.describe());
    assert_eq!(
        failed.describe(),
        "Couldn't save: pool timed out while waiting for an open connection"
    );
    let again = o.retry(failed.id).unwrap();
    let resent = worker.take().remove(0);
    let Request::Save { id, saved } = resent else {
        panic!("a save")
    };
    println!("Try again: {again:?}, the same record");
    assert_eq!(id, again);
    assert!(matches!(*saved, Saved::Product(q) if q == p));
    assert!(o.failures().is_empty());
}

/// The Products form, over a fake worker, with Unit Price typed in.
fn product_form(price: &str) -> (Harness<'static, CairnApp>, Fake) {
    let (o, worker) = fake();
    let mut app = CairnApp::new(common::context()).with(o);
    app.open(Target::Product(None));
    let mut h = Harness::builder()
        .with_size([1180.0, 760.0])
        .with_theme(egui::Theme::Light)
        .build_ui_state(|ui, app: &mut CairnApp| app.show(ui), app);
    h.run_steps(4);
    let field =
        h.get_by_role_and_label(Role::TextInput, "Unit Price");
    field.focus();
    h.run_steps(4);
    h.get_by_role_and_label(Role::TextInput, "Unit Price")
        .type_text(price);
    h.run_steps(4);
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
    h.run_steps(4);
    (h, worker)
}

#[test]
fn the_form_waits_for_its_save() {
    let (mut h, mut worker) = product_form("85");
    assert!(h.query_by_label("Saving…").is_some());
    // Typing now goes nowhere: the form is drawn but disabled.
    let price =
        h.get_by_role_and_label(Role::TextInput, "Unit Price");
    assert!(price.accesskit_node().is_disabled());
    let save = worker.take().remove(0);
    worker.store(save, 11);
    settle(&mut h);
    assert!(h.query_by_label("Saved").is_some());
    let price =
        h.get_by_role_and_label(Role::TextInput, "Unit Price");
    assert!(!price.accesskit_node().is_disabled());
    let products = &h.state().ctx.data.products;
    let sensor = products.iter().find(|p| p.sku == "CS-TH050");
    assert_eq!(sensor.unwrap().unit_price, 85.into());
}

/// H4's figure: one request in flight, then one failed.
#[test]
fn snapshots_in_flight_and_failed() {
    let (mut h, mut worker) = product_form("85");
    h.snapshot("request-in-flight");
    let save = worker.take().remove(0);
    worker.answer(Response::Failed {
        id: save.id(),
        error: Failure::Database(sqlx::Error::PoolTimedOut),
    });
    h.run_steps(4);
    assert!(h.query_by_label_contains("Couldn't save").is_some());
    h.snapshot("request-failed");
}

/// Sales order SO-2026-0002 open over a fake worker, Send to
/// Manufacturer pressed, and the attempt the button sent.
fn sent_once() -> (Harness<'static, CairnApp>, Fake, Request) {
    let (o, mut worker) = fake();
    // A stopped clock: the payments' blank row shows today.
    let mut data = common::snapshot();
    data.clock =
        Some(Clock::fixed("11/3/2026 9:00 AM".parse().unwrap()));
    let mut app = CairnApp::new(AppContext::new(data)).with(o);
    app.open(Target::Order(Some(OrderId(2))));
    let mut h = Harness::builder()
        .with_size([1180.0, 760.0])
        .with_theme(egui::Theme::Light)
        .build_ui_state(|ui, app: &mut CairnApp| app.show(ui), app);
    h.run_steps(4);
    // The order's lookup, answered with what the form already has.
    for r in worker.take() {
        if let Request::Read { id, rows } = r {
            let data = order_2(None);
            worker.answer(Response::Read { id, rows, data });
        }
    }
    settle(&mut h);
    let b =
        h.get_by_role_and_label(Role::Button, "Send to Manufacturer");
    assert!(!b.accesskit_node().is_disabled());
    h.get_by_role_and_label(Role::Button, "Send to Manufacturer")
        .click();
    h.run_steps(4);
    let mut sent = worker.take();
    assert_eq!(sent.len(), 1);
    (h, worker, sent.remove(0))
}

/// Order 2's lookup as the worker reads it: the order, its lines, and
/// its custom order if it's in doubt.
fn order_2(
    unconfirmed: Option<cairn_core::manufacturer::ManufacturerOrder>,
) -> Box<Snapshot> {
    let all = common::snapshot();
    let id = OrderId(2);
    let mut data = Snapshot {
        orders: all
            .orders
            .into_iter()
            .filter(|o| o.id == Some(id))
            .collect(),
        ..Snapshot::default()
    };
    data.order_lines.insert(id, all.order_lines[&id].clone());
    if let Some(order) = unconfirmed {
        data.unconfirmed.insert(id, order);
    }
    Box::new(data)
}

/// What the worker answers for one attempt.
fn answer_send(worker: &Fake, request: Request, sent: Sent) {
    let Request::Send { id, send } = request else {
        panic!("a send")
    };
    worker.answer(Response::Sent { id, send, sent });
}

/// H5's figure: no answer, so the rep is asked. Nothing is sent until
/// Retry, and Retry sends the same correlation ID.
#[test]
fn no_answer_asks_and_retry_sends_the_same_id() {
    let (mut h, mut worker, first) = sent_once();
    let Request::Send { send, .. } = &first else {
        panic!("a send")
    };
    let send = send.clone();
    let id = send.order.correlation_id();
    println!("attempt 1: {} correlationId {id}", send.order.po());
    answer_send(
        &worker,
        first,
        Sent::Unconfirmed {
            reason: "connection closed before message completed"
                .into(),
        },
    );
    h.run_steps(4);
    // The order's attempts, read back after the answer: in doubt.
    let order = send.order.clone();
    for r in worker.take() {
        if let Request::Read { id, rows } = r {
            let data = order_2(Some(order.clone()));
            worker.answer(Response::Read { id, rows, data });
        }
    }
    settle(&mut h);
    assert!(h
        .query_by_label_contains("The manufacturer didn't answer")
        .is_some());
    h.snapshot("manufacturer-prompt");
    // The prompt waits for the rep: nothing goes out by itself.
    h.run_steps(30);
    let none = worker.take();
    println!("30 frames later: {} requests sent", none.len());
    assert!(none.iter().all(|r| !matches!(r, Request::Send { .. })));

    h.get_by_role_and_label(Role::Button, "Retry").click();
    h.run_steps(4);
    let retry = worker
        .take()
        .into_iter()
        .find(|r| matches!(r, Request::Send { .. }));
    let retry = retry.expect("Retry sends");
    let Request::Send { send, .. } = &retry else {
        unreachable!()
    };
    println!("Retry: correlationId {}", send.order.correlation_id());
    assert_eq!(send.order.correlation_id(), id);
    answer_send(
        &worker,
        retry,
        Sent::Confirmed {
            mfr_order_id: "MFR-200001".into(),
            repeat: true,
        },
    );
    h.run_steps(4);
    let status = "SO-2026-0002 sent: MFR-200001, already on file";
    println!("status line: {status}");
    assert!(h.query_by_label(status).is_some());
    assert!(h.query_by_label_contains("didn't answer").is_none());
}

/// Cancel: the order stays unconfirmed, and opening it asks again.
#[test]
fn cancel_leaves_it_unconfirmed_and_opening_it_asks_again() {
    let (mut h, mut worker, first) = sent_once();
    let Request::Send { send, .. } = &first else {
        panic!("a send")
    };
    let order = send.order.clone();
    answer_send(
        &worker,
        first,
        Sent::Unconfirmed {
            reason: "operation timed out".into(),
        },
    );
    h.run_steps(4);
    h.get_by_role_and_label(Role::Button, "Cancel").click();
    h.run_steps(4);
    assert!(h.query_by_label_contains("didn't answer").is_none());
    assert!(h
        .query_by_label("SO-2026-0002 is unconfirmed")
        .is_some());

    // The order's attempts, read back after the answer: still in doubt.
    let rows = Rows::Record(Table::Orders, 2);
    for r in worker.take() {
        if let Request::Read { id, rows: asked } = r {
            assert_eq!(asked, rows);
            let data = order_2(Some(order.clone()));
            worker.answer(Response::Read { id, rows, data });
        }
    }
    settle(&mut h);
    assert!(h.query_by_label_contains("didn't answer").is_none());

    h.state_mut().open(Target::Menu);
    h.state_mut().open(Target::Order(Some(OrderId(2))));
    h.run_steps(4);
    assert!(h.query_by_label_contains("didn't answer").is_some());
    h.get_by_role_and_label(Role::Button, "Retry").click();
    h.run_steps(4);
    let again = worker.take().into_iter().find_map(|r| match r {
        Request::Send { send, .. } => Some(send),
        _ => None,
    });
    assert_eq!(
        again.unwrap().order.correlation_id(),
        order.correlation_id()
    );
}
