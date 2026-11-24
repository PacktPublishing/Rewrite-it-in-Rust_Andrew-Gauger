//! Reports (Chapter 14): a typed view of the data, a handlebars
//! template read from disk, and WeasyPrint to make the PDF. Nothing
//! here draws a coordinate.

mod engine;
mod open;
mod views;

pub use engine::{Renderer, ReportError};
pub use open::{OpenFile, System};
pub use views::{
    commission_statements, invoice, money, month_end, month_label,
    month_list, quote, receipt, rma_summary, shift_month, view, Cell,
    Column, CommissionView, InvoiceView, ListView, MonthList,
    QuoteView, ReceiptView, RmaSummaryView, Row,
};

use crate::data::{Rows, Table};
use cairn_core::date::Date;
use cairn_core::{OrderId, PaymentId, QuoteId};

/// What the rep asked to print.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    Quote(QuoteId),
    Receipt {
        order: OrderId,
        payment: PaymentId,
    },
    Invoice(OrderId),
    /// Every rep's statement for the month containing this date.
    Commission(Date),
    /// Returns opened in the month containing this date.
    RmaSummary(Date),
    /// A month-end list, as a PDF or, with `csv`, a CSV file.
    List {
        list: MonthList,
        month: Date,
        csv: bool,
    },
}

impl Report {
    /// The template's name: `templates/<name>.hbs`.
    pub fn template(self) -> &'static str {
        match self {
            Report::Quote(_) => "quote",
            Report::Receipt { .. } => "receipt",
            Report::Invoice(_) => "invoice",
            Report::Commission(_) => "commission",
            Report::RmaSummary(_) => "rma-summary",
            Report::List { .. } => "list",
        }
    }

    /// The PDF's file name.
    pub fn file_name(self) -> String {
        match self {
            Report::Quote(q) => format!("quote-{}.pdf", q.0),
            Report::Receipt { payment, .. } => {
                format!("receipt-{:05}.pdf", payment.0)
            }
            Report::Invoice(o) => format!("invoice-{}.pdf", o.0),
            Report::Commission(d) => {
                format!(
                    "commission-{}-{:02}.pdf",
                    d.year(),
                    d.month()
                )
            }
            Report::RmaSummary(d) => {
                format!("rma-{}-{:02}.pdf", d.year(), d.month())
            }
            Report::List { list, month, csv } => format!(
                "{}-{}-{:02}.{}",
                list.slug(),
                month.year(),
                month.month(),
                if csv { "csv" } else { "pdf" }
            ),
        }
    }

    /// The rows the worker reads from Postgres before it builds the
    /// view. Each is a read the app already knows how to make.
    pub fn reads(self) -> Vec<Rows> {
        match self {
            Report::Quote(q) => {
                vec![Rows::Record(Table::Quotes, q.0)]
            }
            Report::Receipt { order, .. }
            | Report::Invoice(order) => {
                vec![Rows::Record(Table::Orders, order.0)]
            }
            Report::Commission(_) => vec![Rows::Table(Table::Orders)],
            Report::RmaSummary(_) => vec![Rows::Table(Table::Rmas)],
            Report::List { list, .. } => match list {
                MonthList::QuotePipeline => {
                    vec![Rows::Table(Table::Quotes)]
                }
                _ => vec![Rows::Table(Table::Orders)],
            },
        }
    }
}

use crate::context::AppContext;
use crate::data::{self, Snapshot};
use sqlx::PgPool;
use std::path::PathBuf;

/// The rows a report reads, brought back from Postgres and merged
/// into one snapshot, with the company and the reps every report
/// needs for its letterhead and names.
/// Past this many names, a report reads the whole table.
const MANY: usize = 20;

async fn fetch(
    db: &PgPool,
    report: Report,
) -> Result<Snapshot, ReportError> {
    let read = |rows| async move {
        data::read(db, rows).await.map_err(ReportError::Database)
    };
    let mut d = Snapshot {
        company: read(Rows::Table(Table::Company)).await?.company,
        reps: read(Rows::Table(Table::Reps)).await?.reps,
        ..Snapshot::default()
    };
    for rows in report.reads() {
        let got = data::read(db, rows)
            .await
            .map_err(ReportError::Database)?;
        d.orders.extend(got.orders);
        d.order_lines.extend(got.order_lines);
        d.payments.extend(got.payments);
        d.quotes.extend(got.quotes);
        d.quote_lines.extend(got.quote_lines);
        d.rmas.extend(got.rmas);
    }
    // Names and addresses: the customers and products these rows name.
    let mut customers: Vec<i32> = d
        .orders
        .iter()
        .filter_map(|o| o.customer)
        .chain(d.quotes.iter().filter_map(|q| q.customer))
        .chain(d.rmas.iter().filter_map(|r| r.customer))
        .map(|c| c.0)
        .collect();
    customers.sort_unstable();
    customers.dedup();
    let mut products: Vec<i32> = d
        .order_lines
        .values()
        .flatten()
        .filter_map(|l| l.product)
        .chain(
            d.quote_lines
                .values()
                .flatten()
                .filter_map(|l| l.product),
        )
        .chain(d.rmas.iter().filter_map(|r| r.product))
        .map(|p| p.0)
        .collect();
    products.sort_unstable();
    products.dedup();
    // One record at a time for a few, the whole table for many: a
    // month of returns names hundreds of customers, and a round trip
    // for each costs more than reading them all once.
    if customers.len() > MANY {
        d.customers =
            read(Rows::Table(Table::Customers)).await?.customers;
    } else {
        for c in customers {
            let got = read(Rows::Record(Table::Customers, c)).await?;
            d.customers.extend(got.customers);
        }
    }
    if products.len() > MANY {
        d.products =
            read(Rows::Table(Table::Products)).await?.products;
    } else {
        for p in products {
            let got = read(Rows::Record(Table::Products, p)).await?;
            d.products.extend(got.products);
        }
    }
    Ok(d)
}

/// The whole trip for one report: fetch, fill, bake. Returns where
/// the PDF is.
pub async fn render(
    db: &PgPool,
    renderer: &Renderer,
    report: Report,
) -> Result<PathBuf, ReportError> {
    let ctx = AppContext::new(fetch(db, report).await?);
    tokio::fs::create_dir_all(&renderer.out).await?;
    let to = renderer.out.join(report.file_name());
    // A CSV is the same list as its PDF, written as text.
    if let Report::List {
        list,
        month,
        csv: true,
    } = report
    {
        tokio::fs::write(&to, month_list(&ctx, list, month).csv())
            .await?;
        return Ok(to);
    }
    let view = view(&ctx, report)
        .ok_or_else(|| ReportError::Missing(format!("{report:?}")))?;
    let html = renderer.html(report.template(), &view)?;
    renderer.bake(&html, &to).await?;
    Ok(to)
}
