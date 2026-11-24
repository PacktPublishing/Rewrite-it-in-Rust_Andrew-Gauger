//! The manufacturer call against the Rust mock, started in-process on
//! an ephemeral port. The attempt-row tests write to the dev database
//! as a rep (`common::rep`), so they're ignored by default:
//!
//!   cargo test -p cairn-app --test manufacturer -- --ignored

mod common;

use cairn_app::app::CairnApp;
use cairn_app::context::{AppContext, Target};
use cairn_app::data::{self, Rows};
use cairn_app::manufacturer::{self, Manufacturer, Sent};
use cairn_app::manufacturer_orchestrator::{
    self, ManufacturerOrchestrator,
};
use cairn_app::orchestrator::Orchestrator;
use cairn_app::worker::{self, Workers, CAPACITY};
use cairn_core::manufacturer::{
    ManufacturerOrder, ShipTo, SourceLine,
};
use cairn_core::Category;
use cairn_core::OrderId;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use mock_manufacturer::{serve, Mode, Settings};
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use uuid::Uuid;

/// The mock on an ephemeral port: its `/orders` URL and its ledger.
async fn start(
    mode: Mode,
    times: u32,
    delay: Duration,
) -> (String, PathBuf) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let ledger = std::env::temp_dir()
        .join(format!("cairn-app-mfr-{}.json", Uuid::new_v4()));
    let settings = Settings {
        mode,
        times,
        delay,
        ledger: ledger.clone(),
        first_order_number: 200001,
    };
    tokio::spawn(serve(listener, settings));
    (format!("http://{addr}/orders"), ledger)
}

fn client(url: &str) -> Manufacturer {
    Manufacturer::new(url, Duration::from_secs(5)).unwrap()
}

fn order() -> ManufacturerOrder {
    let ship_to = ShipTo {
        name: "Vista Bakery".into(),
        address: "6503 Enterprise Rd".into(),
        city: "Lubbock".into(),
        state: "TX".into(),
        zip: "76161".into(),
    };
    let lines = [
        SourceLine {
            sku: "CS-TH100".into(),
            category: Category::Sensor,
            quantity: 385,
        },
        SourceLine {
            sku: "CS-GW400".into(),
            category: Category::Gateway,
            quantity: 8,
        },
    ];
    ManufacturerOrder::new("SO-2026-1100", None, ship_to, &lines)
        .unwrap()
}

/// The orders in the mock's ledger file.
fn ledger_orders(ledger: &PathBuf) -> usize {
    let v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(ledger).unwrap())
            .unwrap();
    v.as_array().unwrap().len()
}

#[tokio::test]
async fn normal_is_confirmed() {
    let (url, _) = start(Mode::Normal, 0, Duration::ZERO).await;
    let sent =
        manufacturer::outcome(&client(&url).post(&order()).await);
    assert_eq!(
        sent,
        Sent::Confirmed {
            mfr_order_id: "MFR-200001".into(),
            repeat: false
        }
    );
}

#[tokio::test]
async fn a_drop_is_unconfirmed_and_the_retry_gets_the_same_number() {
    let (url, ledger) = start(Mode::Drop, 1, Duration::ZERO).await;
    let mfr = client(&url);
    let order = order();
    println!("correlationId {}", order.correlation_id());

    let first = manufacturer::outcome(&mfr.post(&order).await);
    println!("attempt 1: {first:?}");
    assert!(matches!(first, Sent::Unconfirmed { .. }));

    let second =
        manufacturer::outcome(&mfr.post(&order.retry()).await);
    println!("attempt 2: {second:?}");
    assert_eq!(
        second,
        Sent::Confirmed {
            mfr_order_id: "MFR-200001".into(),
            repeat: true
        }
    );

    let held = mfr.lookup(order.correlation_id()).await.unwrap();
    let n = ledger_orders(&ledger);
    println!("ledger: {n} order(s); lookup: {held:?}");
    assert_eq!(n, 1);
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].mfr_order_id, "MFR-200001");
}

#[tokio::test]
async fn a_500_is_failed() {
    let (url, ledger) = start(Mode::Error, 0, Duration::ZERO).await;
    let sent =
        manufacturer::outcome(&client(&url).post(&order()).await);
    println!("{sent:?}");
    assert!(matches!(sent, Sent::Failed { .. }));
    assert!(!ledger.exists(), "the mock wrote no ledger: no order");
}

#[tokio::test]
async fn slower_than_the_timeout_is_unconfirmed() {
    // A short timeout for the test; the app's is longer. 300 ms
    // against a 2 s mock is enough to show the arm.
    let (url, _) = start(Mode::Slow, 0, Duration::from_secs(2)).await;
    let mfr =
        Manufacturer::new(&url, Duration::from_millis(300)).unwrap();
    let sent = manufacturer::outcome(&mfr.post(&order()).await);
    println!("{sent:?}");
    assert!(matches!(sent, Sent::Unconfirmed { .. }));
}

#[tokio::test]
async fn nobody_listening_is_failed() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let mfr = client(&format!("http://{addr}/orders"));
    let sent = manufacturer::outcome(&mfr.post(&order()).await);
    println!("{sent:?}");
    assert!(matches!(sent, Sent::Failed { .. }));
}

/// Each test sends for its own sales order, so they can run at once.
async fn order_id(pool: &PgPool, number: &str) -> i32 {
    sqlx::query_scalar(
        "SELECT id FROM orders WHERE order_number = $1",
    )
    .bind(number)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "writes to the dev database"]
async fn the_409_puts_the_number_on_attempt_2() {
    let rep = common::rep::sign_in(1).await;
    let pool = rep.pool().await;
    let order_id = order_id(&pool, "SO-2026-1100").await;
    let (url, ledger) = start(Mode::Drop, 1, Duration::ZERO).await;
    let mfr = client(&url);
    let order = order();
    let id = order.correlation_id();
    println!("correlationId {id}");

    let first =
        manufacturer::send(&pool, &mfr, order_id, None, &order)
            .await
            .unwrap();
    println!("attempt 1: {first:?}");
    assert!(matches!(first, Sent::Unconfirmed { .. }));
    assert!(manufacturer::is_unconfirmed(&pool, id).await.unwrap());
    assert_eq!(
        manufacturer::unconfirmed_for_order(&pool, order_id)
            .await
            .unwrap()
            .map(|o| o.correlation_id()),
        Some(id)
    );

    let second = manufacturer::send(
        &pool,
        &mfr,
        order_id,
        None,
        &order.retry(),
    )
    .await
    .unwrap();
    println!("attempt 2: {second:?}");
    assert!(matches!(second, Sent::Confirmed { repeat: true, .. }));
    assert!(!manufacturer::is_unconfirmed(&pool, id).await.unwrap());

    let rows = manufacturer::attempts(&pool, id).await.unwrap();
    for r in &rows {
        println!(
            "row: attempt {} http {:?} ref {:?} error {:?}",
            r.attempt,
            r.http_status,
            r.manufacturer_ref,
            r.error_text
        );
    }
    pool.close().await;
    common::rep::forget(&rep).await;

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].attempt, 1);
    assert_eq!(rows[0].http_status, None);
    assert_eq!(rows[0].manufacturer_ref, None);
    assert!(rows[0].error_text.is_some());
    assert_eq!(rows[1].attempt, 2);
    assert_eq!(rows[1].http_status, Some(409));
    assert_eq!(
        rows[1].manufacturer_ref.as_deref(),
        Some("MFR-200001")
    );
    assert_eq!(rows[1].error_text, None);
    assert_eq!(ledger_orders(&ledger), 1);
}

#[tokio::test]
#[ignore = "writes to the dev database"]
async fn a_new_order_is_attempt_1_with_its_number() {
    let rep = common::rep::sign_in(1).await;
    let pool = rep.pool().await;
    let order_id = order_id(&pool, "SO-2026-1099").await;
    let (url, _) = start(Mode::Normal, 0, Duration::ZERO).await;
    let order = order();
    let sent = manufacturer::send(
        &pool,
        &client(&url),
        order_id,
        None,
        &order,
    )
    .await
    .unwrap();
    let rows = manufacturer::attempts(&pool, order.correlation_id())
        .await
        .unwrap();
    pool.close().await;
    common::rep::forget(&rep).await;
    assert!(matches!(sent, Sent::Confirmed { repeat: false, .. }));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].http_status, Some(201));
    assert_eq!(
        rows[0].manufacturer_ref.as_deref(),
        Some("MFR-200001")
    );
}

#[test]
fn unconfirmed_means_no_answer_at_all() {
    assert!(manufacturer::in_doubt(None, None));
    assert!(!manufacturer::in_doubt(Some(500), None));
    assert!(!manufacturer::in_doubt(Some(400), None));
    assert!(!manufacturer::in_doubt(Some(409), Some("MFR-200001")));
    assert!(!manufacturer::in_doubt(Some(201), Some("MFR-200001")));
}

#[tokio::test]
#[ignore = "writes to the dev database"]
async fn a_500_is_not_unconfirmed() {
    let rep = common::rep::sign_in(1).await;
    let pool = rep.pool().await;
    let order_id = order_id(&pool, "SO-2026-1098").await;
    let (url, _) = start(Mode::Error, 0, Duration::ZERO).await;
    let order = order();
    let sent = manufacturer::send(
        &pool,
        &client(&url),
        order_id,
        None,
        &order,
    )
    .await
    .unwrap();
    let unconfirmed =
        manufacturer::is_unconfirmed(&pool, order.correlation_id())
            .await
            .unwrap();
    let rows = manufacturer::attempts(&pool, order.correlation_id())
        .await
        .unwrap();
    pool.close().await;
    common::rep::forget(&rep).await;
    println!("{sent:?}; unconfirmed: {unconfirmed}");
    assert!(matches!(sent, Sent::Failed { .. }));
    assert_eq!(rows[0].http_status, Some(500));
    assert!(!unconfirmed);
}

/// The words on screen that contain `part`.
fn text(h: &Harness<'_, CairnApp>, part: &str) -> String {
    let node = h.get_by_label_contains(part);
    let node = node.accesskit_node();
    node.label()
        .or(node.value())
        .unwrap_or_default()
        .to_string()
}

/// H5 end to end: Send to Manufacturer on the order form, through the
/// orchestrator and the real worker, as a rep, against the mock in drop
/// mode for one request. No answer, so the rep is asked; Retry gets the
/// 409 and the number; the manufacturer holds one order.
#[test]
#[ignore = "writes to the dev database"]
fn drop_then_retry_through_the_app() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let rep = rt.block_on(common::rep::sign_in(1));
    let db = rt.block_on(rep.pool());
    let _cleanup = common::rep::Cleanup(rep);
    // An order with hardware on it that has never been sent.
    let (order_id, number): (i32, String) = rt
        .block_on(
            sqlx::query_as(
                "SELECT o.id, o.order_number FROM orders o
                  WHERE EXISTS (SELECT FROM order_lines l
                                  JOIN products p ON p.id = l.product_id
                                 WHERE l.order_id = o.id
                                   AND p.category <> 'Service')
                    AND NOT EXISTS (SELECT FROM manufacturer_orders m
                                     WHERE m.order_id = o.id)
                  ORDER BY o.id LIMIT 1",
            )
            .fetch_one(&db),
        )
        .unwrap();
    let (url, ledger) =
        rt.block_on(start(Mode::Drop, 1, Duration::ZERO));
    println!("{number}; the mock in drop mode for 1 request");

    let workers = Arc::new(
        Workers::signed_in(db.clone()).with_manufacturer(&url),
    );
    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    let (mfr_requests, mfr_from_ui) = mpsc::channel(CAPACITY);
    let (mfr_to_ui, mfr_responses) = mpsc::channel(CAPACITY);
    let mfr_workers = workers.clone();
    rt.spawn(worker::run(
        from_ui,
        to_ui,
        4,
        || {},
        move |r| worker::handle(workers.clone(), r),
    ));
    // The manufacturer's own worker and limit.
    rt.spawn(worker::run(
        mfr_from_ui,
        mfr_to_ui,
        manufacturer_orchestrator::LIMIT,
        || {},
        move |r| worker::handle(mfr_workers.clone(), r),
    ));
    let data = rt.block_on(data::read(&db, Rows::All)).unwrap();
    let mut app = CairnApp::new(AppContext::new(data))
        .with(Orchestrator::new(requests, responses))
        .with_manufacturer(ManufacturerOrchestrator::new(
            mfr_requests,
            mfr_responses,
        ));
    app.open(Target::Order(Some(OrderId(order_id))));
    let mut h = Harness::builder()
        .with_size([1180.0, 760.0])
        .build_ui_state(|ui, app: &mut CairnApp| app.show(ui), app);
    common::until(&mut h, &number);
    h.get_by_role_and_label(Role::Button, "Send to Manufacturer")
        .click();
    println!("Send to Manufacturer");
    common::until(&mut h, "The manufacturer didn't answer");
    println!("prompt: {}", text(&h, "didn't answer"));
    let id = h.state().prompt().unwrap().order.correlation_id();
    let first = rt.block_on(manufacturer::attempts(&db, id)).unwrap();
    println!(
        "attempt 1: http {:?}, ref {:?}, error {:?}",
        first[0].http_status,
        first[0].manufacturer_ref,
        first[0].error_text
    );
    h.get_by_role_and_label(Role::Button, "Retry").click();
    println!("Retry");
    common::until(&mut h, "sent: MFR-");
    let status = text(&h, "sent: MFR-");
    println!("status line: {status}");

    let rows = rt.block_on(manufacturer::attempts(&db, id)).unwrap();
    for r in &rows {
        println!(
            "manufacturer_orders: attempt {} correlation {} http {:?} \
             ref {:?}",
            r.attempt, r.correlation_id, r.http_status, r.manufacturer_ref
        );
    }
    let held = ledger_orders(&ledger);
    println!("the mock's ledger: {held} order(s)");
    drop(h);
    rt.block_on(db.close());

    assert_eq!(
        status,
        format!("{number} sent: MFR-200001, already on file")
    );
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].http_status, Some(409));
    assert_eq!(
        rows[1].manufacturer_ref.as_deref(),
        Some("MFR-200001")
    );
    assert_eq!(held, 1);
}
