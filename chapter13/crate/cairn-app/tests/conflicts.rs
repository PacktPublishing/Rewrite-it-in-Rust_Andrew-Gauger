//! Two reps saving one record (Chapter 12). Each save is conditional on
//! the version the rep read: the second one writes nothing, and the
//! answer is a `Conflict` carrying the record as Postgres has it now.
//! Written to the dev database as two reps, each a member of
//! `cairn_app` with a live `users` row, never as `cairn_owner`. Ignored
//! by default:
//!
//!   cargo test -p cairn-app --test conflicts -- --ignored --nocapture

mod common;

use common::rep::{forget, sign_in};

use cairn_app::context::Saved;
use cairn_app::data::{self, Rows, Table};
use cairn_app::save::{self, Rejected};
use cairn_app::worker::{
    self, Request, RequestId, Response, Workers,
};
use std::sync::Arc;

fn new_product(tag: &str) -> Saved {
    let mut p = common::product(0, "", "Conflict test sensor", 7900);
    p.id = None;
    p.sku = format!("C-{tag}");
    Saved::Product(p)
}

fn product(saved: Saved) -> cairn_app::forms::ProductDraft {
    match saved {
        Saved::Product(p) => p,
        _ => unreachable!(),
    }
}

#[tokio::test]
#[ignore = "writes to the dev database"]
async fn the_second_save_of_a_stale_version_writes_nothing() {
    let (a, b) = (sign_in(1).await, sign_in(2).await);
    let (db_a, db_b) = (a.pool().await, b.pool().await);

    let (stored, saved) =
        save::save(&db_a, new_product(&a.oid[..8])).await.unwrap();
    let read = product(saved);
    assert_eq!(read.version, stored.version);

    // Both reps have the product open at this version.
    let (mut mine_a, mut mine_b) = (read.clone(), read);
    mine_a.name = "Renamed by A".into();
    mine_b.unit_price = rust_decimal::Decimal::new(8100, 2);

    let (after_a, _) =
        save::save(&db_a, Saved::Product(mine_a)).await.unwrap();
    assert_eq!(after_a.version, stored.version + 1);
    println!("A saved: version {}", after_a.version);

    let refused =
        save::save(&db_b, Saved::Product(mine_b.clone())).await;
    assert!(matches!(refused, Err(Rejected::Stale)), "{refused:?}");
    println!("B refused: stale, version {} sent", mine_b.version);

    // B's save changed nothing: the row is A's, at A's version.
    let rows = Rows::Record(Table::Products, stored.key);
    let now = data::read(&db_a, rows).await.unwrap();
    assert_eq!(now.products[0].name, "Renamed by A");
    assert_eq!(now.products[0].version, after_a.version);
    assert_ne!(
        now.products[0].unit_price,
        rust_decimal::Decimal::new(8100, 2)
    );

    // B, having seen A's version, saves on top of it.
    mine_b.version = after_a.version;
    let (after_b, _) =
        save::save(&db_b, Saved::Product(mine_b)).await.unwrap();
    assert_eq!(after_b.version, after_a.version + 1);

    forget(&a).await;
    forget(&b).await;
}

#[tokio::test]
#[ignore = "writes to the dev database"]
async fn the_worker_answers_a_stale_save_with_a_conflict() {
    let (a, b) = (sign_in(1).await, sign_in(2).await);
    let (db_a, db_b) = (a.pool().await, b.pool().await);
    let (stored, saved) =
        save::save(&db_a, new_product(&a.oid[..8])).await.unwrap();
    let read = product(saved);

    let mut mine_a = read.clone();
    mine_a.name = "Renamed by A".into();
    save::save(&db_a, Saved::Product(mine_a)).await.unwrap();

    let mut mine_b = read;
    mine_b.name = "Renamed by B".into();
    let w = Arc::new(Workers::signed_in(db_b));
    let answer = worker::handle(
        w,
        Request::Save {
            id: RequestId(1),
            saved: Box::new(Saved::Product(mine_b)),
        },
    )
    .await;
    let Response::Conflict {
        table,
        key,
        mine,
        theirs,
        ..
    } = answer
    else {
        panic!("expected a conflict, got {answer:?}");
    };
    assert_eq!((table, key), (Table::Products, stored.key));
    assert_eq!(product(*mine).name, "Renamed by B");
    assert_eq!(theirs.products[0].name, "Renamed by A");
    println!(
        "conflict on products {key}: mine {:?}, theirs {:?} at v{}",
        "Renamed by B",
        theirs.products[0].name,
        theirs.products[0].version
    );

    forget(&a).await;
    forget(&b).await;
}

// Chapter 12, H1 P5a: a version per row, lines included. A save sends
// only what the rep changed, so two reps on different parts of one
// order both land, and two reps on the same row conflict on that row.

use cairn_app::context::{Gone, Touched};
use cairn_app::forms::{OrderDraft, OrderLineDraft};
use cairn_core::{OrderStatus, ProductId, RepId};
use rust_decimal::Decimal;

async fn an_order(
    db: &sqlx::PgPool,
    rep: i32,
) -> (OrderDraft, Vec<OrderLineDraft>) {
    let (customer, product): (i32, i32) = sqlx::query_as(
        "SELECT (SELECT min(id) FROM customers),
                (SELECT min(id) FROM products)",
    )
    .fetch_one(db)
    .await
    .unwrap();
    let mut order = common::order(1, "", customer, OrderStatus::Open);
    order.id = None;
    order.version = 0;
    order.rep = Some(RepId(rep));
    order.date = "2026-11-18".parse().unwrap();
    order.notes = "Chapter 12 conflict test".into();
    let line = |qty| OrderLineDraft {
        id: None,
        version: 0,
        product: Some(ProductId(product)),
        description: String::new(),
        quantity: qty,
        unit_price: Decimal::new(29900, 2),
        discount: 0.0,
        serial_numbers: String::new(),
    };
    let saved = Saved::Order {
        order,
        lines: vec![line(12), line(1)],
        payments: Vec::new(),
        removed_lines: Vec::new(),
        removed_payments: Vec::new(),
        touched: None,
    };
    let (_, back) = save::save(db, saved).await.unwrap();
    let Saved::Order { order, lines, .. } = back else {
        unreachable!()
    };
    (order, lines)
}

fn order_save(
    order: &OrderDraft,
    lines: &[OrderLineDraft],
    removed: Vec<Gone>,
    touched: Touched,
) -> Saved {
    Saved::Order {
        order: order.clone(),
        lines: lines.to_vec(),
        payments: Vec::new(),
        removed_lines: removed,
        removed_payments: Vec::new(),
        touched: Some(touched),
    }
}

async fn read_order(
    db: &sqlx::PgPool,
    key: i32,
) -> (OrderDraft, Vec<OrderLineDraft>) {
    let d = data::read(db, Rows::Record(Table::Orders, key))
        .await
        .unwrap();
    let id = cairn_core::OrderId(key);
    (d.orders[0].clone(), d.order_lines[&id].clone())
}

#[tokio::test]
#[ignore = "writes to the dev database"]
async fn a_header_and_a_line_changed_by_two_reps_both_land() {
    let (a, b) = (sign_in(1).await, sign_in(2).await);
    let (db_a, db_b) = (a.pool().await, b.pool().await);
    let (order, lines) = an_order(&db_a, a.rep_id).await;
    let key = order.id.unwrap().0;

    // Both reps have the order open. B reworks line 2's quantity.
    let mut b_lines = lines.clone();
    b_lines[1].quantity = 3;
    let b_touched = Touched {
        lines: vec![b_lines[1].id.unwrap()],
        ..Touched::default()
    };
    // A changes the header (the notes) and saves first.
    let mut a_order = order.clone();
    a_order.notes = "Ship to the new dock".into();
    let a_touched = Touched {
        header: true,
        ..Touched::default()
    };
    save::save(
        &db_a,
        order_save(&a_order, &lines, vec![], a_touched),
    )
    .await
    .unwrap();

    // B saves on a header version that is now stale. The header is not
    // B's to write, so it isn't checked.
    save::save(
        &db_b,
        order_save(&order, &b_lines, vec![], b_touched),
    )
    .await
    .unwrap();

    let (now, now_lines) = read_order(&db_a, key).await;
    println!(
        "header notes {:?} (v{}); line 2 qty {} (v{})",
        now.notes,
        now.version,
        now_lines[1].quantity,
        now_lines[1].version
    );
    assert_eq!(now.notes, "Ship to the new dock");
    assert_eq!(now_lines[1].quantity, 3);
    assert_eq!(now_lines[0].quantity, 12);

    forget(&a).await;
    forget(&b).await;
}

#[tokio::test]
#[ignore = "writes to the dev database"]
async fn two_reps_on_one_line_conflict_on_that_line_only() {
    let (a, b) = (sign_in(1).await, sign_in(2).await);
    let (db_a, db_b) = (a.pool().await, b.pool().await);
    let (order, lines) = an_order(&db_a, a.rep_id).await;
    let key = order.id.unwrap().0;
    let line2 = lines[1].id.unwrap();
    let touched = || Touched {
        lines: vec![line2],
        ..Touched::default()
    };

    let mut a_lines = lines.clone();
    a_lines[1].quantity = 5;
    save::save(
        &db_a,
        order_save(&order, &a_lines, vec![], touched()),
    )
    .await
    .unwrap();

    // B also changes line 2's quantity, and line 1's too: one refused
    // row refuses the save, and line 1 isn't written either.
    let mut b_lines = lines.clone();
    b_lines[0].quantity = 99;
    b_lines[1].quantity = 7;
    let both = Touched {
        lines: vec![lines[0].id.unwrap(), line2],
        ..Touched::default()
    };
    let refused =
        save::save(&db_b, order_save(&order, &b_lines, vec![], both))
            .await;
    assert!(matches!(refused, Err(Rejected::Stale)), "{refused:?}");

    let (_, now) = read_order(&db_a, key).await;
    println!(
        "after the refused save: qty {} and {}",
        now[0].quantity, now[1].quantity
    );
    assert_eq!(
        now[0].quantity, 12,
        "line 1 was written by a refused save"
    );
    assert_eq!(now[1].quantity, 5);

    forget(&a).await;
    forget(&b).await;
}

#[tokio::test]
#[ignore = "writes to the dev database"]
async fn a_line_edited_by_one_and_deleted_by_the_other_conflicts() {
    let (a, b) = (sign_in(1).await, sign_in(2).await);
    let (db_a, db_b) = (a.pool().await, b.pool().await);
    let (order, lines) = an_order(&db_a, a.rep_id).await;
    let key = order.id.unwrap().0;
    let line2 = lines[1].id.unwrap();

    // A edits line 2.
    let mut a_lines = lines.clone();
    a_lines[1].quantity = 5;
    let t = Touched {
        lines: vec![line2],
        ..Touched::default()
    };
    let (_, back) =
        save::save(&db_a, order_save(&order, &a_lines, vec![], t))
            .await
            .unwrap();
    let Saved::Order { lines: a_lines, .. } = back else {
        unreachable!()
    };

    // B deletes it, from the version B saw.
    let gone = Gone {
        id: line2,
        version: lines[1].version,
    };
    let kept = vec![lines[0].clone()];
    let refused = save::save(
        &db_b,
        order_save(&order, &kept, vec![gone], Touched::default()),
    )
    .await;
    assert!(matches!(refused, Err(Rejected::Stale)), "{refused:?}");
    let (_, now) = read_order(&db_a, key).await;
    assert_eq!(now.len(), 2, "the delete went through");

    // Now A deletes it, and B, who edited it, is the one refused: the
    // row is gone, so the conflict's `theirs` has no such line.
    let gone_now = Gone {
        id: line2,
        version: a_lines[1].version,
    };
    save::save(
        &db_a,
        order_save(&order, &kept, vec![gone_now], Touched::default()),
    )
    .await
    .unwrap();
    let mut b_lines = lines.clone();
    b_lines[1].quantity = 9;
    let t = Touched {
        lines: vec![line2],
        ..Touched::default()
    };
    let refused =
        save::save(&db_b, order_save(&order, &b_lines, vec![], t))
            .await;
    assert!(matches!(refused, Err(Rejected::Stale)), "{refused:?}");
    let (_, now) = read_order(&db_a, key).await;
    assert_eq!(now.len(), 1);

    forget(&a).await;
    forget(&b).await;
}

#[tokio::test]
#[ignore = "writes to the dev database"]
async fn a_tax_rate_and_a_quantity_changed_together_both_stand() {
    let (a, b) = (sign_in(1).await, sign_in(2).await);
    let (db_a, db_b) = (a.pool().await, b.pool().await);
    let (order, lines) = an_order(&db_a, a.rep_id).await;
    let key = order.id.unwrap().0;

    let mut a_order = order.clone();
    a_order.tax_rate = 0.0825;
    save::save(
        &db_a,
        order_save(
            &a_order,
            &lines,
            vec![],
            Touched {
                header: true,
                ..Touched::default()
            },
        ),
    )
    .await
    .unwrap();

    let mut b_lines = lines.clone();
    b_lines[0].quantity = 20;
    save::save(
        &db_b,
        order_save(
            &order,
            &b_lines,
            vec![],
            Touched {
                lines: vec![lines[0].id.unwrap()],
                ..Touched::default()
            },
        ),
    )
    .await
    .unwrap();

    let (now, now_lines) = read_order(&db_a, key).await;
    println!(
        "tax rate {}, line 1 qty {}",
        now.tax_rate, now_lines[0].quantity
    );
    assert_eq!(now.tax_rate, 0.0825);
    assert_eq!(now_lines[0].quantity, 20);

    forget(&a).await;
    forget(&b).await;
}

#[tokio::test]
#[ignore = "writes to the dev database"]
async fn an_answered_row_conflict_saves_over_the_other_reps_version()
{
    use cairn_forms::{merge_rows, Keyed, Section, Side};
    let (a, b) = (sign_in(1).await, sign_in(2).await);
    let (db_a, db_b) = (a.pool().await, b.pool().await);
    let (order, lines) = an_order(&db_a, a.rep_id).await;
    let key = order.id.unwrap().0;
    let line2 = lines[1].id.unwrap();
    let t = || Touched {
        lines: vec![line2],
        ..Touched::default()
    };

    // A and B both change line 2's quantity. A saves first.
    let mut a_lines = lines.clone();
    a_lines[1].quantity = 5;
    save::save(&db_a, order_save(&order, &a_lines, vec![], t()))
        .await
        .unwrap();
    let mut b_lines = lines.clone();
    b_lines[1].quantity = 3;
    let refused =
        save::save(&db_b, order_save(&order, &b_lines, vec![], t()))
            .await;
    assert!(matches!(refused, Err(Rejected::Stale)));

    // B's dialog: line 2's quantity, asked. B keeps their own.
    let (_, theirs) = read_order(&db_b, key).await;
    let mut m =
        merge_rows("Line", &lines, &b_lines, &[], &theirs, |l| {
            l.description.clone()
        });
    let qs = m.questions();
    println!("{}: {} / {}", qs[0].label, qs[0].mine, qs[0].theirs);
    assert_eq!(qs.len(), 1);
    m.answer(0, Side::Mine);
    assert!(m.resolved());

    // The merged save is checked against A's version, and lands.
    let merged = m.rows();
    assert_eq!(merged[1].version(), 2);
    let touched = Touched {
        lines: m.written(),
        ..Touched::default()
    };
    save::save(&db_b, order_save(&order, &merged, vec![], touched))
        .await
        .unwrap();
    let (_, now) = read_order(&db_a, key).await;
    println!(
        "line 2 quantity now {} (v{})",
        now[1].quantity, now[1].version
    );
    assert_eq!(now[1].quantity, 3);
    assert_eq!(now[1].version, 3);

    forget(&a).await;
    forget(&b).await;
}

// One save per record in flight, against Postgres: two saves of one
// product, a moment apart, from the version the rep read.

async fn two_saves(ordered: bool) -> Vec<String> {
    use cairn_app::orchestrator::{Orchestrator, ScreenId};
    use cairn_app::worker::CAPACITY;
    use tokio::sync::mpsc;

    // With TWO_SAVES_LOG=<file>, the run's tracing goes to that file.
    if let Ok(path) = std::env::var("TWO_SAVES_LOG") {
        let file = std::fs::File::create(path).unwrap();
        let _ = tracing_subscriber::fmt()
            .with_writer(std::sync::Mutex::new(file))
            .with_ansi(false)
            .try_init();
    }
    let rep = sign_in(1).await;
    let db = rep.pool().await;
    let (_, saved) =
        save::save(&db, new_product(&rep.oid[..8])).await.unwrap();
    let read = product(saved);

    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    let w = Arc::new(Workers::signed_in(db));
    tokio::spawn(worker::run(
        from_ui,
        to_ui,
        4,
        || {},
        move |r| worker::handle(w.clone(), r),
    ));
    let mut o = Orchestrator::new(requests, responses);
    if !ordered {
        o = o.with_unordered_saves();
    }
    let (mut first, mut second) = (read.clone(), read);
    first.name = "First save".into();
    second.unit_price = rust_decimal::Decimal::new(8100, 2);
    o.save(Saved::Product(first), ScreenId(1));
    o.save(Saved::Product(second), ScreenId(2));

    let mut out = Vec::new();
    for _ in 0..200 {
        for a in o.poll() {
            out.push(match a.response {
                Response::Saved { stored, .. } => {
                    format!("stored at version {}", stored.version)
                }
                Response::Conflict { .. } => "conflict".to_string(),
                other => format!("{other:?}"),
            });
        }
        if out.len() == 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10))
            .await;
    }
    forget(&rep).await;
    out
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "writes to the dev database"]
async fn two_saves_of_one_record_both_store_when_ordered() {
    let out = two_saves(true).await;
    println!("ordered: {out:?}");
    assert_eq!(
        out,
        vec!["stored at version 2", "stored at version 3"]
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "writes to the dev database"]
async fn without_the_rule_the_second_save_conflicts_with_the_first() {
    let mut out = two_saves(false).await;
    out.sort();
    println!("unordered: {out:?}");
    assert_eq!(out, vec!["conflict", "stored at version 2"]);
}
