//! Chapter 14: a report through the real worker. The click is a
//! `print`, the channel is the report orchestrator's own, the fetch is
//! Postgres, the bake is WeasyPrint. Writes to the dev database as a
//! rep, so it is ignored by default:
//!
//!   cargo test -p cairn-app --test report_pipeline -- --ignored \
//!       --nocapture --test-threads=1

mod common;

use common::rep::{sign_in, Cleanup};

use cairn_app::context::Saved;
use cairn_app::data::{Rows, Table};
use cairn_app::forms::OrderLineDraft;
use cairn_app::orchestrator::{Orchestrator, ScreenId};
use cairn_app::report_orchestrator::{
    self, Printed, ReportOrchestrator,
};
use cairn_app::reports::{MonthList, Renderer, Report};
use cairn_app::save;
use cairn_app::worker::{
    self, Failure, RequestId, Response, Workers, CAPACITY,
};
use cairn_core::{OrderId, OrderStatus, ProductId, RepId};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// An order of the rep's own with `n` lines. The two products differ
/// so the lines do.
async fn order_with(db: &PgPool, rep: i32, n: i32) -> i32 {
    let (customer, product): (i32, i32) = sqlx::query_as(
        "SELECT (SELECT min(id) FROM customers),
                (SELECT min(id) FROM products
                  WHERE category <> 'Service')",
    )
    .fetch_one(db)
    .await
    .unwrap();
    let mut order = common::order(1, "", customer, OrderStatus::Open);
    order.id = None;
    order.version = 0;
    order.rep = Some(RepId(rep));
    order.date = "2026-11-18".parse().unwrap();
    order.tax_rate = 0.0825;
    let lines = (1..=n)
        .map(|i| OrderLineDraft {
            id: None,
            version: 0,
            product: Some(ProductId(product)),
            description: format!("Line {i} of {n}"),
            quantity: i,
            unit_price: Decimal::new(29900, 2),
            discount: 0.0,
            serial_numbers: String::new(),
        })
        .collect();
    let saved = Saved::Order {
        order,
        lines,
        payments: Vec::new(),
        removed_lines: Vec::new(),
        removed_payments: Vec::new(),
        touched: None,
    };
    save::save(db, saved).await.unwrap().0.key
}

struct Rig {
    records: Orchestrator,
    reports: ReportOrchestrator,
}

fn rig(db: &PgPool, renderer: Renderer) -> Rig {
    let workers = Arc::new(
        Workers::signed_in(db.clone()).with_renderer(renderer),
    );
    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    let w = workers.clone();
    tokio::spawn(worker::run(
        from_ui,
        to_ui,
        cairn_app::data::POOL_SIZE as usize,
        || {},
        move |r| worker::handle(w.clone(), r),
    ));
    let (rpt_requests, rpt_from_ui) = mpsc::channel(CAPACITY);
    let (rpt_to_ui, rpt_responses) = mpsc::channel(CAPACITY);
    let w = workers.clone();
    tokio::spawn(worker::run(
        rpt_from_ui,
        rpt_to_ui,
        report_orchestrator::LIMIT,
        || {},
        move |r| worker::handle(w.clone(), r),
    ));
    Rig {
        records: Orchestrator::new(requests, responses),
        reports: ReportOrchestrator::new(rpt_requests, rpt_responses),
    }
}

fn renderer(weasyprint: &Path) -> Renderer {
    Renderer::new(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("templates"),
        std::env::temp_dir().join("cairn-reports/pipeline"),
    )
    .with_weasyprint(weasyprint)
}

/// A stand-in for `weasyprint` that waits first, or refuses.
fn wrapper(name: &str, body: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("cairn-{name}.sh"));
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    Command::new("chmod").arg("+x").arg(&path).status().unwrap();
    path
}

async fn printed(rig: &mut Rig) -> Printed {
    loop {
        let _ = rig.records.poll();
        if let Some(p) = rig.reports.poll().pop() {
            return p;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn pages(pdf: &Path) -> String {
    let out = Command::new("pdfinfo").arg(pdf).output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find(|l| l.starts_with("Pages:"))
        .unwrap_or("Pages: ?")
        .to_string()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "writes to the dev database"]
async fn an_eight_line_invoice_through_the_worker() {
    let rep = Cleanup(sign_in(1).await);
    let db = rep.0.pool().await;
    let order = order_with(&db, rep.0.rep_id, 8).await;
    let mut rig = rig(&db, renderer(Path::new("weasyprint")));

    let started = Instant::now();
    rig.reports.print(Report::Invoice(OrderId(order)));
    let Printed::Ready { path, .. } = printed(&mut rig).await else {
        panic!("the invoice did not render");
    };
    println!(
        "invoice for an 8-line order: {} ms, {}, {}",
        started.elapsed().as_millis(),
        pages(&path),
        path.display()
    );
    assert_eq!(pages(&path), "Pages:           1");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "writes to the dev database; about 6 seconds"]
async fn a_slow_render_does_not_hold_up_a_lookup() {
    let rep = Cleanup(sign_in(1).await);
    let db = rep.0.pool().await;
    let order = order_with(&db, rep.0.rep_id, 8).await;
    let other = order_with(&db, rep.0.rep_id, 5).await;
    let slow =
        wrapper("slow-weasyprint", "sleep 3\nexec weasyprint \"$@\"");
    let mut rig = rig(&db, renderer(&slow));
    let lookup = Rows::Record(Table::Customers, 1);

    let alone = answered(&mut rig, lookup).await;
    println!("a customer lookup alone: {} ms", alone.as_millis());

    // Two renders at 3 seconds each fill the report worker's limit.
    rig.reports.print(Report::Invoice(OrderId(order)));
    rig.reports.print(Report::Invoice(OrderId(other)));
    let behind = answered(&mut rig, lookup).await;
    println!(
        "the same lookup behind two slow renders: {} ms",
        behind.as_millis()
    );
    assert!(behind < Duration::from_millis(500));
}

async fn answered(rig: &mut Rig, rows: Rows) -> Duration {
    let started = Instant::now();
    let id: RequestId = rig.records.read(rows, ScreenId(1));
    loop {
        let _ = rig.reports.poll();
        for a in rig.records.poll() {
            if let Response::Read { id: got, .. } = a.response {
                if got == id {
                    return started.elapsed();
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "writes to the dev database"]
async fn weasyprint_failures_come_back_in_its_own_words() {
    let rep = Cleanup(sign_in(1).await);
    let db = rep.0.pool().await;
    let order = order_with(&db, rep.0.rep_id, 8).await;

    // Not installed.
    let mut r = rig(&db, renderer(Path::new("/nowhere/weasyprint")));
    r.reports.print(Report::Invoice(OrderId(order)));
    let Printed::Failed { error, .. } = printed(&mut r).await else {
        panic!("expected a failure");
    };
    println!("missing: {error}");
    assert!(matches!(error, Failure::Report(_)));

    // Installed, and refuses.
    let refuses = wrapper(
        "refusing-weasyprint",
        "echo 'ERROR: no such font' >&2\nexit 3",
    );
    let mut r = rig(&db, renderer(&refuses));
    r.reports.print(Report::Invoice(OrderId(order)));
    let Printed::Failed { error, .. } = printed(&mut r).await else {
        panic!("expected a failure");
    };
    println!("refused: {error}");

    // The record is gone.
    let mut r = rig(&db, renderer(Path::new("weasyprint")));
    r.reports.print(Report::Invoice(OrderId(2_000_000_000)));
    let Printed::Failed { error, .. } = printed(&mut r).await else {
        panic!("expected a failure");
    };
    println!("gone: {error}");
}

/// All five reports from the dev database's placeholder history
/// (`dev-ipsum.sh`: 13,531 orders, 19,427 quotes, 2,192 RMAs), each
/// timed from the click to the PDF.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs dev-ipsum.sh rows; reads the dev database"]
async fn every_report_from_the_placeholder_history() {
    let rep = Cleanup(sign_in(1).await);
    let db = rep.0.pool().await;
    let (quote,): (i32,) = sqlx::query_as(
        "SELECT id FROM quotes ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    let (order, payment): (i32, i32) = sqlx::query_as(
        "SELECT order_id, id FROM payments ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    let (month,): (String,) = sqlx::query_as(
        "SELECT to_char(date_trunc('month', max(opened_date)),
                        'YYYY-MM-DD') FROM rmas",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    let month: cairn_core::Date = month.parse().unwrap();
    let mut rig = rig(&db, renderer(Path::new("weasyprint")));
    for report in [
        Report::Quote(cairn_core::QuoteId(quote)),
        Report::Invoice(OrderId(order)),
        Report::Receipt {
            order: OrderId(order),
            payment: cairn_core::PaymentId(payment),
        },
        Report::Commission(month),
        Report::RmaSummary(month),
        Report::List {
            list: MonthList::Orders,
            month,
            csv: false,
        },
        Report::List {
            list: MonthList::SalesByProduct,
            month,
            csv: false,
        },
        Report::List {
            list: MonthList::CashReceived,
            month,
            csv: true,
        },
        Report::List {
            list: MonthList::QuotePipeline,
            month,
            csv: false,
        },
        Report::List {
            list: MonthList::OpenReceivables,
            month,
            csv: false,
        },
    ] {
        let started = Instant::now();
        rig.reports.print(report);
        let Printed::Ready { path, .. } = printed(&mut rig).await
        else {
            panic!("{report:?} did not render");
        };
        let what = if path.extension().is_some_and(|e| e == "csv") {
            format!(
                "{} bytes",
                std::fs::metadata(&path).unwrap().len()
            )
        } else {
            pages(&path)
        };
        println!(
            "{report:?}: {} ms, {what}",
            started.elapsed().as_millis(),
        );
    }
}
