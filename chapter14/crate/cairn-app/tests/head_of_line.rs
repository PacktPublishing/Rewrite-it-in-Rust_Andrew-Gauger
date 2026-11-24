//! Head-of-line blocking (Chapter 12, H3): the manufacturer's slow call
//! against a customer lookup that needs a few milliseconds. The mock is
//! in slow mode, 20 seconds an order. Four sends, then one lookup; the
//! lookup's time from request to answer. Writes to the dev database as
//! a rep, and takes about 20 seconds, so it is ignored by default:
//!
//!   cargo test -p cairn-app --test head_of_line -- --ignored --nocapture

mod common;

use common::rep::{forget, sign_in};

use cairn_app::context::Saved;
use cairn_app::data::{Rows, Table};
use cairn_app::forms::OrderLineDraft;
use cairn_app::manufacturer_orchestrator::{
    self, ManufacturerOrchestrator,
};
use cairn_app::orchestrator::{Orchestrator, ScreenId};
use cairn_app::save;
use cairn_app::worker::{
    self, RequestId, Response, ToManufacturer, Workers, CAPACITY,
};
use cairn_core::manufacturer::{
    ManufacturerOrder, ShipTo, SourceLine,
};
use cairn_core::{Category, OrderId, OrderStatus, ProductId, RepId};
use mock_manufacturer::{serve, Mode, Settings};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Four sales orders of the rep's own, for four sends.
async fn four_orders(db: &PgPool, rep: i32) -> Vec<i32> {
    let (customer, product): (i32, i32) = sqlx::query_as(
        "SELECT (SELECT min(id) FROM customers),
                (SELECT min(id) FROM products
                  WHERE category <> 'Service')",
    )
    .fetch_one(db)
    .await
    .unwrap();
    let mut ids = Vec::new();
    for _ in 0..4 {
        let mut order =
            common::order(1, "", customer, OrderStatus::Open);
        order.id = None;
        order.version = 0;
        order.rep = Some(RepId(rep));
        order.date = "2026-11-18".parse().unwrap();
        let saved = Saved::Order {
            order,
            lines: vec![OrderLineDraft {
                id: None,
                version: 0,
                product: Some(ProductId(product)),
                description: String::new(),
                quantity: 1,
                unit_price: Decimal::new(29900, 2),
                discount: 0.0,
                serial_numbers: String::new(),
            }],
            payments: Vec::new(),
            removed_lines: Vec::new(),
            removed_payments: Vec::new(),
            touched: None,
        };
        let (stored, _) = save::save(db, saved).await.unwrap();
        ids.push(stored.key);
    }
    ids
}

fn to_send(order_id: i32) -> ToManufacturer {
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
        order_id: OrderId(order_id),
        order: ManufacturerOrder::new(
            "SO-2026-1100",
            None,
            ship_to,
            &lines,
        )
        .unwrap(),
    }
}

async fn slow_mock(delay: Duration) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let ledger = std::env::temp_dir()
        .join(format!("cairn-app-mfr-{}.json", Uuid::new_v4()));
    tokio::spawn(serve(
        listener,
        Settings {
            mode: Mode::Slow,
            times: 0,
            delay,
            ledger,
            first_order_number: 200001,
        },
    ));
    format!("http://{addr}/orders")
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "writes to the dev database; about 20 seconds"]
async fn a_customer_lookup_behind_four_slow_sends() {
    let rep = sign_in(1).await;
    let db = rep.pool().await;
    let orders = four_orders(&db, rep.rep_id).await;
    let url = slow_mock(Duration::from_secs(20)).await;

    let workers = Arc::new(
        Workers::signed_in(db.clone()).with_manufacturer(&url),
    );
    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    let records_workers = workers.clone();
    tokio::spawn(worker::run(
        from_ui,
        to_ui,
        cairn_app::data::POOL_SIZE as usize,
        || {},
        move |r| worker::handle(records_workers.clone(), r),
    ));
    let mut o = Orchestrator::new(requests, responses);
    // The manufacturer's own channels, worker and limit.
    let (mfr_requests, mfr_from_ui) = mpsc::channel(CAPACITY);
    let (mfr_to_ui, mfr_responses) = mpsc::channel(CAPACITY);
    let mfr_workers = workers.clone();
    tokio::spawn(worker::run(
        mfr_from_ui,
        mfr_to_ui,
        manufacturer_orchestrator::LIMIT,
        || {},
        move |r| worker::handle(mfr_workers.clone(), r),
    ));
    let mut mfr =
        ManufacturerOrchestrator::new(mfr_requests, mfr_responses);
    let lookup = Rows::Record(Table::Customers, 1);

    // The lookup alone.
    let alone = answered(&mut o, &mut mfr, lookup).await;
    println!("a customer lookup alone: {} ms", alone.as_millis());

    // Four sends, 20 seconds each, then the same lookup.
    for id in &orders {
        mfr.send(to_send(*id), ScreenId(1));
    }
    let behind = answered(&mut o, &mut mfr, lookup).await;
    println!(
        "the same lookup behind four slow sends: {} ms",
        behind.as_millis()
    );
    forget(&rep).await;
}

/// Ask for `rows` and wait for its answer: the time from request to
/// answer. Other answers that arrive meanwhile are let go.
async fn answered(
    o: &mut Orchestrator,
    mfr: &mut ManufacturerOrchestrator,
    rows: Rows,
) -> Duration {
    let started = Instant::now();
    let id: RequestId = o.read(rows, ScreenId(1));
    loop {
        let _ = mfr.poll();
        for a in o.poll() {
            if let Response::Read { id: got, .. } = a.response {
                if got == id {
                    return started.elapsed();
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
