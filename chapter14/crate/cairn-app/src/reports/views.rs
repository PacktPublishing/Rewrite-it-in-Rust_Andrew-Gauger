//! What each template may ask for. Every field is already text: the
//! money, the dates and the percentages are formatted here, once, from
//! `cairn-core`'s numbers, so a template cannot do its own arithmetic
//! and a printed total cannot disagree with the screen.

use super::Report;
use crate::context::AppContext;
use crate::forms::customer::CustomerDraft;
use cairn_core::commission::{self, CommissionOrder};
use cairn_core::date::Date;
use cairn_core::rma::{self, Reason, ReturnFacts};
use cairn_core::totals::balance_due;
use cairn_core::{
    line_total, paid_to_date, receipt_number, totals, Category,
    DocLine, OrderId, Payment, PaymentId, QuoteId, RepId,
};
use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::BTreeMap;

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// `$1,234.56`, or `-$12.00`.
pub fn money(d: Decimal) -> String {
    let cents = d.round_dp(2);
    let text = format!("{:.2}", cents.abs());
    let (whole, frac) = text.split_once('.').unwrap_or((&text, "00"));
    let mut grouped = String::new();
    for (i, c) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    let sign = if cents.is_sign_negative() && !cents.is_zero() {
        "-"
    } else {
        ""
    };
    format!("{sign}${grouped}.{frac}")
}

/// `10/31/2026`, as the screens show it.
pub fn date(d: Date) -> String {
    format!("{}/{}/{}", d.month(), d.day(), d.year())
}

fn opt_date(d: Option<Date>) -> String {
    d.map(date).unwrap_or_default()
}

/// A fraction as a percentage: 0.0725 is `7.25%`, 0.05 is `5%`.
fn percent(f: f64) -> String {
    let text = format!("{:.3}", f * 100.0);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text}%")
}

pub fn month_label(d: Date) -> String {
    format!("{} {}", MONTHS[d.month() as usize - 1], d.year())
}

#[derive(Debug, Serialize)]
pub struct Letterhead {
    pub name: String,
    pub address: Vec<String>,
    pub contact: String,
}

fn letterhead(ctx: &AppContext) -> Letterhead {
    let c = &ctx.data.company;
    let city = [c.city.as_str(), c.state.as_str(), c.zip.as_str()]
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let address = [c.address1.clone(), c.address2.clone(), city]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    let contact = [&c.phone, &c.email, &c.website]
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("  ·  ");
    Letterhead {
        name: c.company_name.clone(),
        address,
        contact,
    }
}

fn address(
    name: &str,
    street: &str,
    city: &str,
    state: &str,
    zip: &str,
    country: &str,
) -> Vec<String> {
    let town = [city, state, zip]
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    [name, street, town.as_str(), country]
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn bill_to(c: &CustomerDraft) -> Vec<String> {
    address(
        &c.company_name,
        &c.bill_address,
        &c.bill_city,
        &c.bill_state,
        &c.bill_zip,
        &c.bill_country,
    )
}

fn ship_to(c: &CustomerDraft) -> Vec<String> {
    let blank = c.ship_address.is_empty() && c.ship_city.is_empty();
    if blank {
        return bill_to(c);
    }
    address(
        &c.company_name,
        &c.ship_address,
        &c.ship_city,
        &c.ship_state,
        &c.ship_zip,
        &c.ship_country,
    )
}

#[derive(Debug, Serialize)]
pub struct LineView {
    pub n: usize,
    pub sku: String,
    pub description: String,
    pub quantity: i32,
    pub unit_price: String,
    pub discount: String,
    pub total: String,
}

fn lines(
    ctx: &AppContext,
    rows: impl Iterator<Item = (Option<cairn_core::ProductId>, DocLine)>,
) -> Vec<LineView> {
    rows.enumerate()
        .map(|(i, (product, l))| {
            let p = product.and_then(|id| ctx.product(id));
            let description = l
                .description
                .clone()
                .filter(|d| !d.is_empty())
                .or_else(|| p.map(|p| p.name.clone()))
                .unwrap_or_default();
            LineView {
                n: i + 1,
                sku: p.map(|p| p.sku.clone()).unwrap_or_default(),
                description,
                quantity: l.quantity,
                unit_price: money(l.unit_price),
                discount: if l.discount == 0.0 {
                    String::new()
                } else {
                    percent(l.discount)
                },
                total: money(line_total(&l)),
            }
        })
        .collect()
}

#[derive(Debug, Serialize)]
pub struct InvoiceView {
    pub company: Letterhead,
    pub number: String,
    pub date: String,
    pub customer_po: String,
    pub terms: String,
    pub rep: String,
    pub bill_to: Vec<String>,
    pub ship_to: Vec<String>,
    pub ship_via: String,
    pub tracking: String,
    pub line_count: usize,
    pub lines: Vec<LineView>,
    pub subtotal: String,
    pub tax_rate: String,
    pub tax: String,
    pub shipping: String,
    pub total: String,
    pub paid: String,
    pub balance_due: String,
    pub notes: String,
}

pub fn invoice(ctx: &AppContext, id: OrderId) -> Option<InvoiceView> {
    let o = ctx.order(id)?;
    let c = ctx.customer(o.customer?)?;
    let rows = ctx.data.order_lines.get(&id)?;
    let docs: Vec<DocLine> =
        rows.iter().map(|l| l.doc_line()).collect();
    let t = totals(&docs, o.tax_rate, o.shipping);
    let paid: Vec<Decimal> = ctx
        .data
        .payments
        .get(&id)
        .map(|p| p.iter().map(|p| p.amount).collect())
        .unwrap_or_default();
    let owed = balance_due(t.total, &paid);
    Some(InvoiceView {
        company: letterhead(ctx),
        number: o.number.clone(),
        date: date(o.ship_date.unwrap_or(o.date)),
        customer_po: o.customer_po.clone(),
        terms: c.terms.clone(),
        rep: ctx.rep_name(o.rep),
        bill_to: bill_to(c),
        ship_to: ship_to(c),
        ship_via: o.ship_via.clone(),
        tracking: o.tracking.clone(),
        line_count: docs.len(),
        lines: lines(
            ctx,
            rows.iter().map(|l| l.product).zip(docs.iter().cloned()),
        ),
        subtotal: money(t.subtotal),
        tax_rate: percent(o.tax_rate),
        tax: money(t.tax),
        shipping: money(t.shipping),
        total: money(t.total),
        paid: money(t.total - owed),
        balance_due: money(owed),
        notes: o.notes.clone(),
    })
}

#[derive(Debug, Serialize)]
pub struct QuoteView {
    pub company: Letterhead,
    pub number: String,
    pub date: String,
    pub valid_until: String,
    pub rep: String,
    pub bill_to: Vec<String>,
    pub lines: Vec<LineView>,
    pub subtotal: String,
    pub tax_rate: String,
    pub tax: String,
    pub shipping: String,
    pub total: String,
    pub terms: String,
    pub notes: String,
}

pub fn quote(ctx: &AppContext, id: QuoteId) -> Option<QuoteView> {
    let q = ctx.data.quotes.iter().find(|q| q.id == Some(id))?;
    let c = ctx.customer(q.customer?)?;
    let rows = ctx.data.quote_lines.get(&id)?;
    let docs: Vec<DocLine> =
        rows.iter().map(|l| l.doc_line()).collect();
    let t = totals(&docs, q.tax_rate, q.shipping);
    Some(QuoteView {
        company: letterhead(ctx),
        number: q.number.clone(),
        date: date(q.date),
        valid_until: opt_date(q.valid_until),
        rep: ctx.rep_name(q.rep),
        bill_to: bill_to(c),
        lines: lines(
            ctx,
            rows.iter().map(|l| l.product).zip(docs.iter().cloned()),
        ),
        subtotal: money(t.subtotal),
        tax_rate: percent(q.tax_rate),
        tax: money(t.tax),
        shipping: money(t.shipping),
        total: money(t.total),
        terms: if q.terms.is_empty() {
            ctx.data.company.quote_terms.clone()
        } else {
            q.terms.clone()
        },
        notes: q.notes.clone(),
    })
}

#[derive(Debug, Serialize)]
pub struct ReceiptView {
    pub company: Letterhead,
    pub receipt_number: String,
    pub date: String,
    pub order_number: String,
    pub customer: Vec<String>,
    pub amount: String,
    pub method: String,
    pub reference: String,
    pub order_total: String,
    pub paid_to_date: String,
    pub balance: String,
    pub footer: String,
}

pub fn receipt(
    ctx: &AppContext,
    order: OrderId,
    payment: PaymentId,
) -> Option<ReceiptView> {
    let o = ctx.order(order)?;
    let c = ctx.customer(o.customer?)?;
    let all = ctx.data.payments.get(&order)?;
    let core: Vec<Payment> = all
        .iter()
        .filter_map(|p| {
            Some(Payment {
                id: p.id?,
                date: p.date,
                amount: p.amount,
            })
        })
        .collect();
    let this = core.iter().find(|p| p.id == payment)?;
    let method = all.iter().find(|p| p.id == Some(payment))?;
    let docs: Vec<DocLine> = ctx
        .data
        .order_lines
        .get(&order)
        .map(|r| r.iter().map(|l| l.doc_line()).collect())
        .unwrap_or_default();
    let t = totals(&docs, o.tax_rate, o.shipping);
    let paid = paid_to_date(&core, this);
    Some(ReceiptView {
        company: letterhead(ctx),
        receipt_number: receipt_number(payment),
        date: date(this.date),
        order_number: o.number.clone(),
        customer: bill_to(c),
        amount: money(this.amount),
        method: method.method.clone(),
        reference: method.reference.clone(),
        order_total: money(t.total),
        paid_to_date: money(paid),
        balance: money(t.total - paid),
        footer: ctx.data.company.receipt_footer.clone(),
    })
}

#[derive(Debug, Serialize)]
pub struct CommissionRow {
    pub order: String,
    pub date: String,
    pub status: String,
    pub net_sales: String,
    pub paid: String,
    pub balance: String,
    pub commission: String,
}

#[derive(Debug, Serialize)]
pub struct RepStatement {
    pub rep: String,
    pub rate: String,
    pub orders: Vec<CommissionRow>,
    pub order_count: usize,
    pub net_sales: String,
    pub commission: String,
    pub unpaid: String,
    pub no_payment: usize,
}

#[derive(Debug, Serialize)]
pub struct CommissionView {
    pub company: Letterhead,
    pub month: String,
    pub reps: Vec<RepStatement>,
    pub order_count: usize,
    pub net_sales: String,
    pub commission: String,
    pub unpaid: String,
    pub no_payment: usize,
}

/// Every order in the snapshot, as `qryCommissionDetail` sees it.
fn commission_orders(ctx: &AppContext) -> Vec<CommissionOrder> {
    ctx.data
        .orders
        .iter()
        .filter_map(|o| {
            let id = o.id?;
            let docs: Vec<DocLine> = ctx
                .data
                .order_lines
                .get(&id)
                .map(|r| r.iter().map(|l| l.doc_line()).collect())
                .unwrap_or_default();
            let net: Decimal = docs.iter().map(line_total).sum();
            let t = totals(&docs, o.tax_rate, o.shipping);
            let paid: Decimal = ctx
                .data
                .payments
                .get(&id)
                .map(|p| p.iter().map(|p| p.amount).sum())
                .unwrap_or_default();
            Some(CommissionOrder {
                order_number: o.number.clone(),
                rep: o.rep?,
                order_date: o.date,
                status: o.status,
                net_sales: net,
                amount_paid: paid,
                balance_due: t.total - paid,
            })
        })
        .collect()
}

pub fn commission_statements(
    ctx: &AppContext,
    month: Date,
) -> CommissionView {
    let orders = commission_orders(ctx);
    let rates: Vec<(RepId, f64)> = ctx
        .data
        .reps
        .iter()
        .filter_map(|r| Some((r.id?, r.commission_rate)))
        .collect();
    let mut reps = Vec::new();
    for r in &ctx.data.reps {
        let Some(id) = r.id else { continue };
        let s = commission::statement(
            &orders,
            id,
            r.commission_rate,
            month,
        );
        if s.orders == 0 {
            continue;
        }
        let mut rows: Vec<&CommissionOrder> = orders
            .iter()
            .filter(|o| {
                o.rep == id
                    && o.order_date.in_same_month(month)
                    && o.status != cairn_core::OrderStatus::Cancelled
            })
            .collect();
        rows.sort_by(|a, b| a.order_number.cmp(&b.order_number));
        reps.push(RepStatement {
            rep: r.name(),
            rate: percent(r.commission_rate),
            orders: rows
                .iter()
                .map(|o| CommissionRow {
                    order: o.order_number.clone(),
                    date: date(o.order_date),
                    status: o.status.to_string(),
                    net_sales: money(o.net_sales),
                    paid: money(o.amount_paid),
                    balance: money(o.balance_due),
                    commission: money(
                        cairn_core::access_round_product(
                            o.net_sales,
                            r.commission_rate,
                        ),
                    ),
                })
                .collect(),
            order_count: s.orders,
            net_sales: money(s.net_sales),
            commission: money(s.commission),
            unpaid: money(s.unpaid),
            no_payment: s.orders_with_no_payment,
        });
    }
    let all = commission::all_reps(&orders, &rates, month);
    CommissionView {
        company: letterhead(ctx),
        month: month_label(month),
        reps,
        order_count: all.orders,
        net_sales: money(all.net_sales),
        commission: money(all.commission),
        unpaid: money(all.unpaid),
        no_payment: all.orders_with_no_payment,
    }
}

#[derive(Debug, Serialize)]
pub struct RmaRow {
    pub number: String,
    pub opened: String,
    pub customer: String,
    pub product: String,
    pub reason: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct RouteGroup {
    pub route: String,
    pub count: usize,
    pub returns: Vec<RmaRow>,
}

#[derive(Debug, Serialize)]
pub struct RmaSummaryView {
    pub company: Letterhead,
    pub month: String,
    pub groups: Vec<RouteGroup>,
    pub total: usize,
}

pub fn rma_summary(ctx: &AppContext, month: Date) -> RmaSummaryView {
    let mut groups: BTreeMap<String, Vec<RmaRow>> = BTreeMap::new();
    let mut total = 0;
    for r in &ctx.data.rmas {
        if !r.opened.in_same_month(month) {
            continue;
        }
        total += 1;
        let product = r.product.and_then(|id| ctx.product(id));
        let customer = r.customer.and_then(|id| ctx.customer(id));
        let category = product.and_then(|p| {
            Category::try_from(p.category.as_str()).ok()
        });
        let route = match (
            Reason::try_from(r.reason.as_str()),
            category,
            customer,
        ) {
            (Ok(reason), Some(category), Some(c)) => {
                rma::route(&ReturnFacts {
                    reason,
                    category,
                    extended_warranty: r.extended_warranty,
                    tax_exempt: c.tax_exempt,
                    days_in_service: r.days(ctx),
                })
                .to_string()
            }
            _ => "Needs a customer, a product and a reason".into(),
        };
        groups.entry(route).or_default().push(RmaRow {
            number: r.number.clone(),
            opened: date(r.opened),
            customer: customer
                .map(|c| c.company_name.clone())
                .unwrap_or_default(),
            product: product
                .map(|p| p.name.clone())
                .unwrap_or_default(),
            reason: r.reason.clone(),
            status: r.status.clone(),
        });
    }
    RmaSummaryView {
        company: letterhead(ctx),
        month: month_label(month),
        groups: groups
            .into_iter()
            .map(|(route, returns)| RouteGroup {
                route,
                count: returns.len(),
                returns,
            })
            .collect(),
        total,
    }
}

/// The view a report's template reads, as JSON, or `None` when the
/// record is not in the snapshot.
pub fn view(
    ctx: &AppContext,
    report: Report,
) -> Option<serde_json::Value> {
    let v = match report {
        Report::Quote(q) => serde_json::to_value(quote(ctx, q)?),
        Report::Receipt { order, payment } => {
            serde_json::to_value(receipt(ctx, order, payment)?)
        }
        Report::Invoice(o) => serde_json::to_value(invoice(ctx, o)?),
        Report::Commission(m) => {
            serde_json::to_value(commission_statements(ctx, m))
        }
        Report::RmaSummary(m) => {
            serde_json::to_value(rma_summary(ctx, m))
        }
        Report::List { list, month, .. } => {
            serde_json::to_value(month_list(ctx, list, month))
        }
    };
    v.ok()
}

/// The month-end lists (V1's `qryMonthEnd_*`). One layout, six
/// questions: each list is a title, columns, rows and an optional
/// total row. The same view makes the PDF and the CSV, so the two
/// cannot disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MonthList {
    Orders,
    SalesByRep,
    SalesByProduct,
    CashReceived,
    QuotePipeline,
    OpenReceivables,
}

impl MonthList {
    pub const ALL: [MonthList; 6] = [
        MonthList::Orders,
        MonthList::SalesByRep,
        MonthList::SalesByProduct,
        MonthList::CashReceived,
        MonthList::QuotePipeline,
        MonthList::OpenReceivables,
    ];

    pub fn title(self) -> &'static str {
        match self {
            MonthList::Orders => "Orders",
            MonthList::SalesByRep => "Sales by Rep",
            MonthList::SalesByProduct => "Sales by Product",
            MonthList::CashReceived => "Cash Received",
            MonthList::QuotePipeline => "Quote Pipeline",
            MonthList::OpenReceivables => "Open Receivables",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            MonthList::Orders => "orders",
            MonthList::SalesByRep => "sales-by-rep",
            MonthList::SalesByProduct => "sales-by-product",
            MonthList::CashReceived => "cash-received",
            MonthList::QuotePipeline => "quote-pipeline",
            MonthList::OpenReceivables => "open-receivables",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Column {
    pub name: String,
    pub right: bool,
}

#[derive(Debug, Serialize)]
pub struct Cell {
    pub v: String,
    pub right: bool,
}

#[derive(Debug, Serialize)]
pub struct Row {
    pub cells: Vec<Cell>,
}

#[derive(Debug, Serialize)]
pub struct ListView {
    pub company: Letterhead,
    pub title: String,
    pub month: String,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    pub count: usize,
    pub total: Option<Row>,
}

impl ListView {
    /// Comma-separated, one header row, quoted where a cell needs it.
    /// Money keeps its `$` and commas, as the page shows it.
    pub fn csv(&self) -> String {
        fn cell(v: &str) -> String {
            if v.contains([',', '"', '\n']) {
                format!("\"{}\"", v.replace('"', "\"\""))
            } else {
                v.to_string()
            }
        }
        let line = |cells: Vec<&str>| {
            cells.into_iter().map(cell).collect::<Vec<_>>().join(",")
        };
        let mut out = vec![line(
            self.columns.iter().map(|c| c.name.as_str()).collect(),
        )];
        for r in self.rows.iter().chain(self.total.iter()) {
            out.push(line(
                r.cells.iter().map(|c| c.v.as_str()).collect(),
            ));
        }
        out.join("\r\n") + "\r\n"
    }
}

fn list_view(
    ctx: &AppContext,
    list: MonthList,
    month: Date,
    columns: &[(&str, bool)],
    rows: Vec<Vec<String>>,
    total: Option<Vec<String>>,
) -> ListView {
    let row = |cells: Vec<String>| Row {
        cells: cells
            .into_iter()
            .zip(columns)
            .map(|(v, (_, right))| Cell { v, right: *right })
            .collect(),
    };
    let count = rows.len();
    ListView {
        company: letterhead(ctx),
        title: list.title().into(),
        month: month_label(month),
        columns: columns
            .iter()
            .map(|(name, right)| Column {
                name: name.to_string(),
                right: *right,
            })
            .collect(),
        rows: rows.into_iter().map(row).collect(),
        count,
        total: total.map(row),
    }
}

/// An order's net sales (line totals), its totals, and what is paid.
fn order_money(
    ctx: &AppContext,
    id: OrderId,
) -> Option<(Decimal, cairn_core::Totals, Decimal)> {
    let o = ctx.order(id)?;
    let docs: Vec<DocLine> = ctx
        .data
        .order_lines
        .get(&id)
        .map(|r| r.iter().map(|l| l.doc_line()).collect())
        .unwrap_or_default();
    let net = docs.iter().map(line_total).sum();
    let t = totals(&docs, o.tax_rate, o.shipping);
    let paid = ctx
        .data
        .payments
        .get(&id)
        .map(|p| p.iter().map(|p| p.amount).sum())
        .unwrap_or_default();
    Some((net, t, paid))
}

fn sum_money<'a>(it: impl Iterator<Item = &'a Decimal>) -> Decimal {
    it.copied().sum()
}

pub fn month_list(
    ctx: &AppContext,
    list: MonthList,
    month: Date,
) -> ListView {
    use cairn_core::OrderStatus::Cancelled;
    let in_month = |d: Date| d.in_same_month(month);
    match list {
        MonthList::Orders => {
            let mut rows = Vec::new();
            let mut total = Decimal::ZERO;
            let mut orders: Vec<_> = ctx
                .data
                .orders
                .iter()
                .filter(|o| in_month(o.date))
                .collect();
            orders.sort_by(|a, b| a.number.cmp(&b.number));
            for o in orders {
                let Some(id) = o.id else { continue };
                let Some((_, t, _)) = order_money(ctx, id) else {
                    continue;
                };
                total += t.total;
                rows.push(vec![
                    o.number.clone(),
                    date(o.date),
                    ctx.customer_name(o.customer),
                    ctx.rep_name(o.rep),
                    o.status.to_string(),
                    money(t.total),
                ]);
            }
            let n = rows.len();
            list_view(
                ctx,
                list,
                month,
                &[
                    ("Order", false),
                    ("Date", false),
                    ("Customer", false),
                    ("Rep", false),
                    ("Status", false),
                    ("Total", true),
                ],
                rows,
                Some(vec![
                    format!("{n} orders"),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    money(total),
                ]),
            )
        }
        MonthList::SalesByRep => {
            let orders = commission_orders(ctx);
            let mut rows = Vec::new();
            let (mut n, mut net, mut pay) =
                (0, Decimal::ZERO, Decimal::ZERO);
            for r in &ctx.data.reps {
                let Some(id) = r.id else { continue };
                let mine: Vec<&CommissionOrder> = orders
                    .iter()
                    .filter(|o| {
                        o.rep == id
                            && in_month(o.order_date)
                            && o.status
                                != cairn_core::OrderStatus::Cancelled
                    })
                    .collect();
                if mine.is_empty() {
                    continue;
                }
                let sales: Decimal =
                    mine.iter().map(|o| o.net_sales).sum();
                // The month-end figure: add the month, round once.
                let comm = commission::sales_by_rep_commission(
                    &orders,
                    id,
                    r.commission_rate,
                    month,
                );
                n += mine.len();
                net += sales;
                pay += comm;
                rows.push(vec![
                    r.name(),
                    mine.len().to_string(),
                    money(sales),
                    percent(r.commission_rate),
                    money(comm),
                ]);
            }
            list_view(
                ctx,
                list,
                month,
                &[
                    ("Sales rep", false),
                    ("Orders", true),
                    ("Net sales", true),
                    ("Rate", true),
                    ("Commission", true),
                ],
                rows,
                Some(vec![
                    "All reps".into(),
                    n.to_string(),
                    money(net),
                    String::new(),
                    money(pay),
                ]),
            )
        }
        MonthList::SalesByProduct => {
            let mut by: BTreeMap<String, (String, i64, Decimal)> =
                BTreeMap::new();
            for o in &ctx.data.orders {
                let (Some(id), true) = (o.id, in_month(o.date))
                else {
                    continue;
                };
                if o.status == Cancelled {
                    continue;
                }
                for l in ctx
                    .data
                    .order_lines
                    .get(&id)
                    .into_iter()
                    .flatten()
                {
                    let p = l.product.and_then(|p| ctx.product(p));
                    let key = p
                        .map(|p| p.sku.clone())
                        .unwrap_or_else(|| "(no product)".into());
                    let e = by.entry(key).or_insert_with(|| {
                        (
                            p.map(|p| p.name.clone())
                                .unwrap_or_default(),
                            0,
                            Decimal::ZERO,
                        )
                    });
                    e.1 += l.quantity as i64;
                    e.2 += line_total(&l.doc_line());
                }
            }
            let (mut qty, mut net) = (0, Decimal::ZERO);
            let rows = by
                .into_iter()
                .map(|(sku, (name, q, n))| {
                    qty += q;
                    net += n;
                    vec![sku, name, q.to_string(), money(n)]
                })
                .collect();
            list_view(
                ctx,
                list,
                month,
                &[
                    ("SKU", false),
                    ("Product", false),
                    ("Units", true),
                    ("Net sales", true),
                ],
                rows,
                Some(vec![
                    "All products".into(),
                    String::new(),
                    qty.to_string(),
                    money(net),
                ]),
            )
        }
        MonthList::CashReceived => {
            let mut found = Vec::new();
            for o in &ctx.data.orders {
                let Some(id) = o.id else { continue };
                for p in
                    ctx.data.payments.get(&id).into_iter().flatten()
                {
                    if in_month(p.date) {
                        found.push((p.date, p.id, o, p));
                    }
                }
            }
            found.sort_by_key(|(d, id, _, _)| (*d, *id));
            let total =
                sum_money(found.iter().map(|(_, _, _, p)| &p.amount));
            let rows: Vec<Vec<String>> = found
                .iter()
                .map(|(d, id, o, p)| {
                    vec![
                        date(*d),
                        id.map(cairn_core::receipt_number)
                            .unwrap_or_default(),
                        o.number.clone(),
                        ctx.customer_name(o.customer),
                        p.method.clone(),
                        money(p.amount),
                    ]
                })
                .collect();
            let n = rows.len();
            list_view(
                ctx,
                list,
                month,
                &[
                    ("Date", false),
                    ("Receipt", false),
                    ("Order", false),
                    ("Customer", false),
                    ("Method", false),
                    ("Amount", true),
                ],
                rows,
                Some(vec![
                    format!("{n} payments"),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    money(total),
                ]),
            )
        }
        MonthList::QuotePipeline => {
            let mut by: BTreeMap<String, (usize, Decimal)> =
                BTreeMap::new();
            for q in &ctx.data.quotes {
                let (Some(id), true) = (q.id, in_month(q.date))
                else {
                    continue;
                };
                let docs: Vec<DocLine> = ctx
                    .data
                    .quote_lines
                    .get(&id)
                    .map(|r| r.iter().map(|l| l.doc_line()).collect())
                    .unwrap_or_default();
                let t = totals(&docs, q.tax_rate, q.shipping);
                let e = by
                    .entry(q.status.to_string())
                    .or_insert((0, Decimal::ZERO));
                e.0 += 1;
                e.1 += t.total;
            }
            let (mut n, mut value) = (0, Decimal::ZERO);
            let rows = by
                .into_iter()
                .map(|(status, (c, v))| {
                    n += c;
                    value += v;
                    vec![status, c.to_string(), money(v)]
                })
                .collect();
            list_view(
                ctx,
                list,
                month,
                &[
                    ("Status", false),
                    ("Quotes", true),
                    ("Value", true),
                ],
                rows,
                Some(vec!["All".into(), n.to_string(), money(value)]),
            )
        }
        MonthList::OpenReceivables => {
            // Everything dated on or before the month's last day that
            // still owes money, oldest first.
            let end = month_end(month);
            let mut open = Vec::new();
            for o in &ctx.data.orders {
                let Some(id) = o.id else { continue };
                if o.status == Cancelled || o.date > end {
                    continue;
                }
                let Some((_, t, paid)) = order_money(ctx, id) else {
                    continue;
                };
                let due = t.total - paid;
                if due > Decimal::ZERO {
                    open.push((o, t.total, paid, due));
                }
            }
            open.sort_by(|a, b| {
                (a.0.date, &a.0.number).cmp(&(b.0.date, &b.0.number))
            });
            let due = sum_money(open.iter().map(|(_, _, _, d)| d));
            let rows: Vec<Vec<String>> = open
                .iter()
                .map(|(o, total, paid, due)| {
                    vec![
                        o.number.clone(),
                        date(o.date),
                        ctx.customer_name(o.customer),
                        money(*total),
                        money(*paid),
                        money(*due),
                        end.days_since(o.date).to_string(),
                    ]
                })
                .collect();
            let n = rows.len();
            list_view(
                ctx,
                list,
                month,
                &[
                    ("Order", false),
                    ("Date", false),
                    ("Customer", false),
                    ("Total", true),
                    ("Paid", true),
                    ("Balance", true),
                    ("Days", true),
                ],
                rows,
                Some(vec![
                    format!("{n} orders"),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    money(due),
                    String::new(),
                ]),
            )
        }
    }
}

/// The last day of `d`'s month.
pub fn month_end(d: Date) -> Date {
    let (y, m) = if d.month() == 12 {
        (d.year() + 1, 1)
    } else {
        (d.year(), d.month() + 1)
    };
    Date::new(y, m, 1)
        .map(|first| first.add_days(-1))
        .unwrap_or(d)
}

/// The first day of the month `by` months from `d`'s.
pub fn shift_month(d: Date, by: i32) -> Date {
    let index = d.year() * 12 + d.month() as i32 - 1 + by;
    Date::new(
        index.div_euclid(12),
        index.rem_euclid(12) as u32 + 1,
        1,
    )
    .unwrap_or(d)
}
