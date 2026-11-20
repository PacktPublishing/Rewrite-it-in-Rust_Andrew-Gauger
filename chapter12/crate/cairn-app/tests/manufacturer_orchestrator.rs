//! The manufacturer's orchestrator on its own (Chapter 12): sends on a
//! channel of its own, answers back by `try_recv`, a failure kept for
//! Try again, and a login that ran out under an attempt noted for the
//! records side. Over a fake worker, no database.

mod common;

use cairn_app::manufacturer::Sent;
use cairn_app::manufacturer_orchestrator::ManufacturerOrchestrator;
use cairn_app::orchestrator::ScreenId;
use cairn_app::worker::{
    Failure, Request, Response, ToManufacturer, CAPACITY,
};
use cairn_core::manufacturer::{
    ManufacturerOrder, ShipTo, SourceLine,
};
use cairn_core::{Category, OrderId};
use tokio::sync::mpsc;

fn to_send(order: i32) -> ToManufacturer {
    let ship_to = ShipTo {
        name: "Vista Bakery".into(),
        address: "6503 Enterprise Rd".into(),
        city: "Lubbock".into(),
        state: "TX".into(),
        zip: "76161".into(),
    };
    let lines = [SourceLine {
        sku: "CS-TH100".into(),
        category: Category::Sensor,
        quantity: 385,
    }];
    ToManufacturer {
        order_id: OrderId(order),
        order: ManufacturerOrder::new(
            "SO-2026-1100",
            None,
            ship_to,
            &lines,
        )
        .unwrap(),
    }
}

#[test]
fn a_send_goes_out_and_its_answer_comes_back() {
    let (requests, mut from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    let mut m = ManufacturerOrchestrator::new(requests, responses);
    let id = m.send(to_send(2), ScreenId(1));
    assert!(m.sending(OrderId(2)));
    assert!(m.status().is_some());
    let Ok(Request::Send { id: sent, send }) = from_ui.try_recv()
    else {
        panic!("a send")
    };
    assert_eq!(sent, id);
    to_ui
        .try_send(Response::Sent {
            id,
            send,
            sent: Sent::Confirmed {
                mfr_order_id: "MFR-200001".into(),
                repeat: false,
            },
        })
        .unwrap();
    let heard = m.poll();
    assert_eq!(heard.len(), 1);
    assert!(!m.sending(OrderId(2)));
    assert!(m.status().is_none());
}

#[test]
fn a_burst_waits_here_in_order_not_in_the_channel() {
    let (requests, mut from_ui) = mpsc::channel(2);
    let (_to_ui, responses) = mpsc::channel(2);
    let mut m = ManufacturerOrchestrator::new(requests, responses);
    for n in 1..=5 {
        m.send(to_send(n), ScreenId(1));
    }
    // Two fit in the channel; three wait here. None is dropped.
    assert_eq!(
        std::iter::from_fn(|| from_ui.try_recv().ok()).count(),
        2
    );
    assert!(m.sending(OrderId(5)));
    m.poll();
    assert_eq!(
        std::iter::from_fn(|| from_ui.try_recv().ok()).count(),
        2
    );
}

#[test]
fn a_failure_is_kept_and_try_again_sends_it_again() {
    let (requests, mut from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    let mut m = ManufacturerOrchestrator::new(requests, responses);
    let id = m.send(to_send(2), ScreenId(1));
    let _ = from_ui.try_recv();
    to_ui
        .try_send(Response::Failed {
            id,
            error: Failure::Database(sqlx::Error::PoolTimedOut),
        })
        .unwrap();
    assert!(m.poll().is_empty());
    assert_eq!(m.failures().len(), 1);
    assert!(
        !m.take_expired(),
        "a pool timeout isn't a login that ran out"
    );

    assert!(m.retry(id).is_some());
    assert!(m.failures().is_empty());
    assert!(matches!(from_ui.try_recv(), Ok(Request::Send { .. })));
}

#[test]
fn the_prompt_belongs_to_this_side() {
    let (requests, _from_ui) = mpsc::channel(CAPACITY);
    let (_to_ui, responses) = mpsc::channel(CAPACITY);
    let mut m = ManufacturerOrchestrator::new(requests, responses);
    assert!(m.prompt().is_none());
    m.ask(Box::new(to_send(2)), ScreenId(3));
    assert!(m.prompt().is_some());
    assert!(m.asked(ScreenId(3)));
    m.clear_prompt();
    // Cancel holds until the order is opened again: same view, asked.
    assert!(m.prompt().is_none());
    assert!(m.asked(ScreenId(3)));
    assert!(!m.asked(ScreenId(4)));
}
