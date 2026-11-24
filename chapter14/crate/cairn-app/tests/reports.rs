//! Chapter 14: the reports, rendered for real. A small snapshot, no
//! database. Needs `weasyprint` on the PATH and `pdfinfo` and
//! `pdftotext` (poppler) to read the PDFs back.
//!
//!   cargo test -p cairn-app --test reports -- --nocapture

mod common;

use cairn_app::context::AppContext;
use cairn_app::forms::{
    OrderLineDraft, PaymentDraft, QuoteDraft, QuoteLineDraft,
    RmaDraft,
};
use cairn_app::reports::{
    commission_statements, invoice, month_list, receipt, MonthList,
    Renderer, Report, ReportError,
};
use cairn_core::{
    CustomerId, Date, OrderId, PaymentId, ProductId, QuoteId,
};
use rust_decimal::Decimal;
use std::path::{Path, PathBuf};
use std::process::Command;

fn renderer(name: &str) -> Renderer {
    let out = std::env::temp_dir().join("cairn-reports").join(name);
    Renderer::new(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("templates"),
        out,
    )
}

fn lines(n: i32) -> Vec<OrderLineDraft> {
    (1..=n)
        .map(|i| OrderLineDraft {
            id: None,
            version: 0,
            product: Some(ProductId(if i % 2 == 0 {
                11
            } else {
                12
            })),
            description: String::new(),
            quantity: i,
            unit_price: Decimal::new(7900 + 100 * i as i64, 2),
            discount: if i % 5 == 0 { 0.05 } else { 0.0 },
            serial_numbers: String::new(),
        })
        .collect()
}

/// The common snapshot, plus order 4 (8 lines, V1's clipped case)
/// and order 5 (40 lines), each with a payment.
fn ctx() -> AppContext {
    let mut d = common::snapshot();
    for (id, number, n) in
        [(4, "SO-2026-0004", 8), (5, "SO-2026-0005", 40)]
    {
        let mut o = common::order(
            id,
            number,
            766,
            cairn_core::OrderStatus::Shipped,
        );
        o.tax_rate = 0.0825;
        o.shipping = Decimal::new(4500, 2);
        d.orders.push(o);
        d.order_lines.insert(OrderId(id), lines(n));
        d.payments.insert(
            OrderId(id),
            vec![PaymentDraft {
                id: Some(PaymentId(id * 10)),
                version: 1,
                date: Date::new(2026, 10, 20).unwrap(),
                amount: Decimal::new(100_000, 2),
                method: "Check".into(),
                reference: "1042".into(),
                received_by: None,
                notes: String::new(),
            }],
        );
    }
    let mut ctx = AppContext::new(d);
    let mut q = QuoteDraft::blank(&ctx, Some(CustomerId(766)));
    q.id = Some(QuoteId(1));
    q.number = "QT-2026-0001".into();
    q.date = Date::new(2026, 10, 6).unwrap();
    q.tax_rate = 0.0825;
    ctx.data.quotes.push(q);
    ctx.data.quote_lines.insert(
        QuoteId(1),
        lines(3)
            .into_iter()
            .map(|l| QuoteLineDraft {
                id: None,
                version: 0,
                product: l.product,
                description: l.description,
                quantity: l.quantity,
                unit_price: l.unit_price,
                discount: l.discount,
            })
            .collect(),
    );
    for (i, (reason, customer, day)) in [
        ("Dead on Arrival", 766, 3),
        ("Damaged in Shipping", 767, 9),
        ("Failed in Warranty", 766, 12),
    ]
    .into_iter()
    .enumerate()
    {
        ctx.data.rmas.push(RmaDraft {
            id: Some(i as i32 + 1),
            version: 1,
            number: format!("RMA-2026-{:04}", i + 1),
            opened: Date::new(2026, 10, day).unwrap(),
            status: "Open".into(),
            opened_by: None,
            closed: None,
            customer: Some(CustomerId(customer)),
            order: Some(OrderId(4)),
            product: Some(ProductId(11)),
            serial: String::new(),
            reason: reason.into(),
            extended_warranty: false,
            disposition: String::new(),
            manufacturer_ref: String::new(),
            description: String::new(),
        });
    }
    ctx.rebuild();
    ctx
}

fn pdf(ctx: &AppContext, r: &Renderer, report: Report) -> PathBuf {
    let view = cairn_app::reports::view(ctx, report).expect("view");
    let html = r.html(report.template(), &view).expect("template");
    let to = r.out.join(report.file_name());
    std::fs::create_dir_all(&r.out).unwrap();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(r.bake(&html, &to)).expect("weasyprint");
    to
}

fn pages(pdf: &Path) -> u32 {
    let out = Command::new("pdfinfo").arg(pdf).output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("Pages:"))
        .and_then(|n| n.trim().parse().ok())
        .expect("pdfinfo prints Pages")
}

fn text(pdf: &Path, page: u32) -> String {
    let out = Command::new("pdftotext")
        .args(["-layout", "-f", &page.to_string()])
        .args(["-l", &page.to_string()])
        .arg(pdf)
        .arg("-")
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into()
}

#[test]
fn the_invoice_prints_all_eight_lines() {
    let ctx = ctx();
    let v = invoice(&ctx, OrderId(4)).unwrap();
    assert_eq!(v.line_count, 8);
    assert_eq!(v.lines.len(), 8);
    let r = renderer("eight");
    let file = pdf(&ctx, &r, Report::Invoice(OrderId(4)));
    let page = text(&file, 1);
    println!("{} page(s)", pages(&file));
    assert_eq!(pages(&file), 1);
    for n in 1..=8 {
        assert!(page.contains(&v.lines[n - 1].total), "line {n}");
    }
    assert!(page.contains(&v.total), "the total is on the page");
}

#[test]
fn a_long_invoice_repeats_its_header_and_numbers_its_pages() {
    let ctx = ctx();
    let r = renderer("forty");
    let file = pdf(&ctx, &r, Report::Invoice(OrderId(5)));
    let n = pages(&file);
    println!("{n} pages");
    assert!(n >= 2);
    let second = text(&file, 2);
    assert!(second.contains("Description"), "header repeats");
    assert!(second.contains(&format!("Page 2 of {n}")));
}

#[test]
fn every_report_renders_to_a_pdf() {
    let ctx = ctx();
    let r = renderer("all");
    let month = Date::new(2026, 10, 1).unwrap();
    let all = [
        Report::Quote(QuoteId(1)),
        Report::Invoice(OrderId(4)),
        Report::Receipt {
            order: OrderId(4),
            payment: PaymentId(40),
        },
        Report::Commission(month),
        Report::RmaSummary(month),
    ];
    for report in all {
        let file = pdf(&ctx, &r, report);
        let head = std::fs::read(&file).unwrap();
        assert!(head.starts_with(b"%PDF"), "{report:?}");
        println!("{report:?}: {} page(s)", pages(&file));
    }
}

#[test]
fn the_rma_summary_groups_returns_by_route() {
    let ctx = ctx();
    let v = cairn_app::reports::rma_summary(
        &ctx,
        Date::new(2026, 10, 1).unwrap(),
    );
    assert_eq!(v.total, 3);
    let counted: usize = v.groups.iter().map(|g| g.count).sum();
    assert_eq!(counted, 3);
    for g in &v.groups {
        println!("{} ({})", g.route, g.count);
    }
}

#[test]
fn the_receipt_matches_the_core_numbers() {
    let ctx = ctx();
    let v = receipt(&ctx, OrderId(4), PaymentId(40)).unwrap();
    assert_eq!(v.receipt_number, "00040");
    assert_eq!(v.amount, "$1,000.00");
}

#[test]
fn the_commission_totals_are_the_sum_of_the_statements() {
    let ctx = ctx();
    let v =
        commission_statements(&ctx, Date::new(2026, 10, 1).unwrap());
    assert!(!v.reps.is_empty());
    let n: usize = v.reps.iter().map(|r| r.order_count).sum();
    assert_eq!(v.order_count, n);
}

#[test]
fn a_missing_weasyprint_is_named() {
    let ctx = ctx();
    let r =
        renderer("missing").with_weasyprint("/nowhere/weasyprint");
    let view =
        cairn_app::reports::view(&ctx, Report::Invoice(OrderId(4)))
            .unwrap();
    let html = r.html("invoice", &view).unwrap();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let err = rt
        .block_on(r.bake(&html, &r.out.join("x.pdf")))
        .unwrap_err();
    println!("{err}");
    assert!(matches!(err, ReportError::NoWeasyPrint(_)));
}

#[test]
fn a_field_the_view_lacks_is_an_error_not_a_blank() {
    let ctx = ctx();
    let dir = std::env::temp_dir().join("cairn-reports/strict");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bad.hbs"), "Total: {{grand_total}}")
        .unwrap();
    let r = Renderer::new(&dir, &dir);
    let view =
        cairn_app::reports::view(&ctx, Report::Invoice(OrderId(4)))
            .unwrap();
    let err = r.html("bad", &view).unwrap_err();
    println!("{err}");
    assert!(matches!(err, ReportError::Template(_)));
}

fn october() -> Date {
    Date::new(2026, 10, 1).unwrap()
}

#[test]
fn the_month_end_lists_add_up() {
    let ctx = ctx();
    let orders = month_list(&ctx, MonthList::Orders, october());
    assert_eq!(orders.count, 5);
    // The total row is the sum of the rows above it.
    let cash = month_list(&ctx, MonthList::CashReceived, october());
    assert_eq!(cash.count, 2);
    assert_eq!(cash.total.unwrap().cells[5].v, "$2,000.00");
    let open =
        month_list(&ctx, MonthList::OpenReceivables, october());
    // Orders 1 to 5, less any paid in full: none are.
    assert_eq!(open.count, 5);
    let by_rep = month_list(&ctx, MonthList::SalesByRep, october());
    assert_eq!(by_rep.rows.len(), 1);
    let pipeline =
        month_list(&ctx, MonthList::QuotePipeline, october());
    assert_eq!(pipeline.count, 1);
}

#[test]
fn the_csv_is_the_same_list_with_cells_quoted() {
    let ctx = ctx();
    let csv =
        month_list(&ctx, MonthList::CashReceived, october()).csv();
    println!("{csv}");
    assert!(
        csv.starts_with("Date,Receipt,Order,Customer,Method,Amount")
    );
    // Money has a comma in it, so it is quoted.
    assert!(csv.contains("\"$1,000.00\""));
    assert!(csv.ends_with("\r\n"));
}

#[test]
fn every_month_end_list_renders_as_a_pdf() {
    let ctx = ctx();
    let r = renderer("lists");
    for list in MonthList::ALL {
        let file = pdf(
            &ctx,
            &r,
            Report::List {
                list,
                month: october(),
                csv: false,
            },
        );
        assert!(std::fs::read(&file).unwrap().starts_with(b"%PDF"));
        println!("{}: {} page(s)", list.title(), pages(&file));
    }
}
