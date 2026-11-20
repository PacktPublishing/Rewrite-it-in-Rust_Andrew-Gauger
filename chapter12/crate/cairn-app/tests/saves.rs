//! Saves through the workers, written to the dev database as a rep: a
//! member of `cairn_app` with a live `users` row, issued the way
//! `cairn-auth` issues one. Never as `cairn_owner`, who owns the tables
//! and so skips row-level security, the session check and the audit
//! trail's identity. Each test signs in its own rep, and removes the
//! rep, its rows and their audit entries afterwards. Ignored by default:
//!
//!   cargo test -p cairn-app --test saves -- --ignored --nocapture

mod common;

use common::rep::{forget, sign_in};
use common::until;

use cairn_app::app::CairnApp;
use cairn_app::context::{AppContext, Gone, Saved, Target};
use cairn_app::data::{self, Rows, Snapshot, Table};
use cairn_app::forms::{OrderLineDraft, PaymentDraft};
use cairn_app::orchestrator::Orchestrator;
use cairn_app::save;
use cairn_app::worker::{self, Workers, CAPACITY};
use cairn_core::{
    CustomerId, OrderId, OrderStatus, ProductId, RepId,
};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::mpsc;

/// An existing customer and product, to hang an order from.
async fn customer_and_product(db: &PgPool) -> (i32, i32) {
    sqlx::query_as(
        "SELECT (SELECT min(id) FROM customers),
                (SELECT min(id) FROM products)",
    )
    .fetch_one(db)
    .await
    .unwrap()
}

fn new_order(customer: i32, product: i32, rep: i32) -> Saved {
    let mut order = common::order(1, "", customer, OrderStatus::Open);
    order.id = None;
    order.rep = Some(RepId(rep));
    order.date = "2026-11-03".parse().unwrap();
    order.notes = "Chapter 11 save test".into();
    let line = |qty, cents| OrderLineDraft {
        id: None,
        version: 0,
        product: Some(ProductId(product)),
        description: String::new(),
        quantity: qty,
        unit_price: Decimal::new(cents, 2),
        discount: 0.05,
        serial_numbers: String::new(),
    };
    Saved::Order {
        order,
        lines: vec![line(12, 29900), line(1, 49900)],
        payments: vec![PaymentDraft {
            id: None,
            version: 0,
            date: "2026-11-03".parse().unwrap(),
            amount: Decimal::new(100000, 2),
            method: "Check".into(),
            reference: "1042".into(),
            received_by: Some(RepId(rep)),
            notes: String::new(),
        }],
        removed_lines: Vec::new(),
        removed_payments: Vec::new(),
        touched: None,
    }
}

#[tokio::test]
#[ignore = "writes to the dev database"]
async fn a_new_order_gets_its_number_and_version_from_postgres() {
    let rep = sign_in(1).await;
    let db = rep.pool().await;
    let (customer, product) = customer_and_product(&db).await;

    let saved = new_order(customer, product, rep.rep_id);
    let (stored, back) = save::save(&db, saved).await.unwrap();
    println!("inserted: {stored:?}");
    let number = stored.number.clone().unwrap();
    assert!(number.starts_with("SO-"), "{number}");
    assert_eq!(stored.version, 1);
    let Saved::Order {
        mut order,
        lines,
        payments,
        ..
    } = back
    else {
        unreachable!()
    };
    assert_eq!(order.id, Some(OrderId(stored.key)));
    assert_eq!(order.number, number);
    assert!(payments[0].id.is_some());

    // What a lookup reads back is what was saved.
    let rows = Rows::Record(Table::Orders, stored.key);
    let fresh = data::read(&db, rows).await.unwrap();
    assert_eq!(fresh.orders, vec![order.clone()]);
    assert_eq!(fresh.order_lines[&OrderId(stored.key)], lines);
    assert_eq!(fresh.payments[&OrderId(stored.key)], payments);

    // Saved again: the same number, the next version.
    order.customer_po = "PO-77".into();
    let again = Saved::Order {
        order,
        lines,
        payments,
        removed_lines: Vec::new(),
        removed_payments: Vec::new(),
        touched: None,
    };
    let (stored, _) = save::save(&db, again).await.unwrap();
    println!("updated:  {stored:?}");
    assert_eq!(stored.number.as_deref(), Some(number.as_str()));
    assert_eq!(stored.version, 2);
    db.close().await;
    forget(&rep).await;
}

#[tokio::test]
#[ignore = "writes to the dev database"]
async fn the_audit_row_carries_the_reps_identity() {
    let rep = sign_in(1).await;
    let db = rep.pool().await;
    let (customer, product) = customer_and_product(&db).await;
    let saved = new_order(customer, product, rep.rep_id);
    let (stored, _) = save::save(&db, saved).await.unwrap();

    let (action, db_role, oid, name): (
        String,
        String,
        String,
        String,
    ) = sqlx::query_as(
        "SELECT action, db_role, entra_oid::text, display_name
               FROM audit_log
              WHERE table_name = 'orders' AND row_id = $1",
    )
    .bind(stored.key)
    .fetch_one(&db)
    .await
    .unwrap();
    println!(
        "audit_log for orders {}: {action} by {db_role} \
         ({name}, Entra {oid})",
        stored.key
    );
    assert_eq!(
        (
            action.as_str(),
            db_role.as_str(),
            oid.as_str(),
            name.as_str()
        ),
        ("INSERT", rep.role.as_str(), rep.oid.as_str(), "Test Rep")
    );
    db.close().await;
    forget(&rep).await;
}

#[tokio::test]
#[ignore = "writes to the dev database"]
async fn a_customer_save_writes_only_the_subform_rows_that_changed() {
    let rep = sign_in(1).await;
    let db = rep.pool().await;
    let mut customer =
        common::customer(0, "Save Test Dairy", false, 1);
    customer.id = None;
    let mut contacts = vec![
        common::contact(0, 0, "Ana", "Ruiz"),
        common::contact(0, 0, "Joe", "Park"),
    ];
    for c in &mut contacts {
        c.id = None;
    }
    let first = Saved::Customer {
        customer: Box::new(customer),
        contacts,
        activities: Vec::new(),
        removed_contacts: Vec::new(),
        removed_activities: Vec::new(),
        touched: None,
    };
    let (stored, back) = save::save(&db, first).await.unwrap();
    let Saved::Customer {
        customer,
        mut contacts,
        ..
    } = back
    else {
        unreachable!()
    };
    assert_eq!(customer.id, Some(CustomerId(stored.key)));
    // Ana changes; Joe is deleted on the form.
    contacts[0].phone = "555-0100".into();
    let joe = contacts.remove(1);
    let second = Saved::Customer {
        customer,
        contacts,
        activities: Vec::new(),
        removed_contacts: vec![Gone {
            id: joe.id.unwrap().0,
            version: joe.version,
        }],
        removed_activities: Vec::new(),
        touched: None,
    };
    save::save(&db, second).await.unwrap();

    let audit: Vec<(String, String)> = sqlx::query_as(
        "SELECT table_name, action FROM audit_log
          WHERE db_role = $1 ORDER BY id",
    )
    .bind(&rep.role)
    .fetch_all(&db)
    .await
    .unwrap();
    println!("{audit:?}");
    let contact_rows: Vec<&str> = audit
        .iter()
        .filter(|(t, _)| t == "contacts")
        .map(|(_, a)| a.as_str())
        .collect();
    assert_eq!(
        contact_rows,
        ["INSERT", "INSERT", "DELETE", "UPDATE"]
    );
    db.close().await;
    forget(&rep).await;
}

/// The whole path: Shift+Enter on the Products form, a request, the
/// worker, Postgres, the answer, the form again.
#[test]
#[ignore = "writes to the dev database"]
fn shift_enter_saves_through_the_worker() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let rep = rt.block_on(sign_in(1));
    let db = rt.block_on(rep.pool());
    let mut product =
        common::product(0, "", "Save test sensor", 7900);
    product.id = None;
    product.sku = format!("T-{}", &rep.oid[..8]);
    let (stored, _) = rt
        .block_on(save::save(&db, Saved::Product(product)))
        .unwrap();

    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    let w = Arc::new(Workers::signed_in(db.clone()));
    rt.spawn(worker::run(
        from_ui,
        to_ui,
        4,
        || {},
        move |r| worker::handle(w.clone(), r),
    ));
    let ctx = AppContext::new(common::snapshot());
    let mut app = CairnApp::new(ctx)
        .with(Orchestrator::new(requests, responses));
    app.open(Target::Product(Some(stored.key)));
    let mut h = Harness::builder()
        .with_size([1180.0, 760.0])
        .build_ui_state(|ui, app: &mut CairnApp| app.show(ui), app);
    // The lookup brings the new product into the form.
    until(&mut h, "Save test sensor");
    let price =
        h.get_by_role_and_label(Role::TextInput, "Unit Price");
    price.focus();
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Unit Price")
        .type_text("85");
    h.run();
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
    let took = until(&mut h, "Saved");
    println!(
        "Shift+Enter to Saved: {:.0} ms",
        took.as_secs_f64() * 1e3
    );
    let price: Decimal = rt
        .block_on(
            sqlx::query_scalar(
                "SELECT unit_price FROM products WHERE id = $1",
            )
            .bind(stored.key)
            .fetch_one(&db),
        )
        .unwrap();
    assert_eq!(price, Decimal::new(85, 0));
    drop(h);
    rt.block_on(db.close());
    rt.block_on(forget(&rep));
}

/// What `main` does: the window opens on an empty snapshot, and the
/// first request brings the rows.
#[test]
#[ignore = "reads the dev database"]
fn the_window_opens_before_its_rows() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let rep = rt.block_on(sign_in(1));
    let db = rt.block_on(rep.pool());
    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    let w = Arc::new(Workers::signed_in(db.clone()));
    rt.spawn(worker::run(
        from_ui,
        to_ui,
        4,
        || {},
        move |r| worker::handle(w.clone(), r),
    ));
    let ctx = AppContext::new(Snapshot::default());
    let mut app = CairnApp::new(ctx)
        .with(Orchestrator::new(requests, responses));
    app.read(Rows::All);
    let mut h = Harness::builder()
        .with_size([1180.0, 760.0])
        .build_ui_state(|ui, app: &mut CairnApp| app.show(ui), app);
    h.step();
    assert!(h.query_by_label("Connecting…").is_some());
    let took = until(&mut h, &format!("Connected as {}", rep.role));
    let d = &h.state().ctx.data;
    println!(
        "first frame: Connecting…; rows after {:.0} ms: {} customers, \
         {} orders, {} activities",
        took.as_secs_f64() * 1e3,
        d.customers.len(),
        d.orders.len(),
        d.activities.len()
    );
    assert!(!d.orders.is_empty());
    drop(h);
    rt.block_on(db.close());
    rt.block_on(forget(&rep));
}

#[tokio::test]
#[ignore = "reads the dev database"]
async fn now_is_the_servers_clock() {
    let rep = sign_in(1).await;
    let db = rep.pool().await;
    let snapshot =
        data::read(&db, Rows::Table(Table::Company)).await.unwrap();
    assert!(snapshot.clock.is_none());
    let all = data::read(&db, Rows::All).await.unwrap();
    let ours = all.clock.unwrap().now();
    let theirs: String = sqlx::query_scalar(
        "SELECT to_char(now(), 'YYYY-MM-DD HH24:MI')",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    println!("server {theirs}, app {ours}");
    assert_eq!(ours, theirs.parse().unwrap());
    db.close().await;
    forget(&rep).await;
}

/// V1's `Now()` was the PC's own time. sqlx starts every session in
/// UTC (`TimeZone=UTC` in the startup packet, after any `options`, so
/// it wins), and the pool sets this machine's zone once each connection
/// is open.
#[tokio::test]
#[ignore = "reads the dev database"]
async fn the_session_is_in_this_machines_zone() {
    let rep = sign_in(1).await;
    let zone = data::local_zone();
    let login: sqlx::postgres::PgConnectOptions =
        rep.url().parse().unwrap();
    let plain = PgPool::connect_with(
        login.clone().options([("timezone", zone.as_str())]),
    )
    .await
    .unwrap();
    let asked: String = sqlx::query_scalar("SHOW TimeZone")
        .fetch_one(&plain)
        .await
        .unwrap();
    plain.close().await;
    let db = data::connect_with(login).await.unwrap();
    let (session, local, utc): (String, String, String) = sqlx::query_as(
        "SELECT current_setting('TimeZone'),
                to_char(now(), 'YYYY-MM-DD HH24:MI'),
                to_char(now() AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI')",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    println!(
        "this machine: {zone}; options(timezone) alone: {asked}; \
         the app's pool: {session}, now() {local} (UTC {utc})"
    );
    db.close().await;
    forget(&rep).await;
    assert_eq!(asked, "UTC");
    assert_eq!(session, zone);
}
