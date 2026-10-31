//! Loads the CSV exports from the History database in `tests/fixtures/`.
#![allow(dead_code)]

use cairn_core::{
    days_in_service, totals, Category, CommissionOrder, Date,
    DocLine, OrderStatus, ProductId, Reason, RepId, ReturnFacts,
};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

type Row = HashMap<String, String>;

fn rows(file: &str) -> Vec<Row> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(file);
    let mut reader = csv::Reader::from_path(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    reader
        .deserialize()
        .map(|r| r.expect("a fixture row"))
        .collect()
}

fn dec(s: &str) -> Decimal {
    if s.is_empty() {
        Decimal::ZERO
    } else {
        Decimal::from_str(s).unwrap_or_else(|e| panic!("{s:?}: {e}"))
    }
}

fn float(s: &str) -> f64 {
    if s.is_empty() {
        0.0
    } else {
        s.parse().unwrap()
    }
}

/// mdb-export's default `MM/DD/YY HH:MM:SS`. The history starts in 2016.
fn v1_date(s: &str) -> Date {
    let (m, d, y) = (&s[0..2], &s[3..5], &s[6..8]);
    Date::new(
        2000 + y.parse::<i32>().unwrap(),
        m.parse().unwrap(),
        d.parse().unwrap(),
    )
    .unwrap()
}

fn iso_date(s: &str) -> Option<Date> {
    (!s.is_empty()).then(|| s.parse().unwrap())
}

pub struct September {
    pub orders: Vec<CommissionOrder>,
    pub rates: Vec<(RepId, f64)>,
    /// Order number to (net sales, tax rate).
    pub tax_inputs: HashMap<String, (Decimal, f64)>,
}

pub fn september_2026() -> September {
    let mut lines: HashMap<String, Vec<DocLine>> = HashMap::new();
    for l in rows("order_lines_sept_2026.csv") {
        lines.entry(l["OrderID"].clone()).or_default().push(
            DocLine {
                product: ProductId(l["ProductID"].parse().unwrap()),
                description: Some(l["LineDescription"].clone())
                    .filter(|d| !d.is_empty()),
                quantity: l["Quantity"].parse().unwrap(),
                unit_price: dec(&l["UnitPrice"]),
                discount: float(&l["DiscountPct"]),
            },
        );
    }
    let mut paid: HashMap<String, Decimal> = HashMap::new();
    for p in rows("payments_sept_2026.csv") {
        *paid.entry(p["OrderID"].clone()).or_default() +=
            dec(&p["Amount"]);
    }
    let mut orders = Vec::new();
    let mut tax_inputs = HashMap::new();
    for o in rows("orders_sept_2026.csv") {
        let t = totals(
            lines.get(&o["OrderID"]).map_or(&[][..], |v| v),
            float(&o["TaxRate"]),
            dec(&o["ShippingCost"]),
        );
        let amount_paid =
            paid.get(&o["OrderID"]).copied().unwrap_or_default();
        tax_inputs.insert(
            o["OrderNumber"].clone(),
            (t.subtotal, float(&o["TaxRate"])),
        );
        orders.push(CommissionOrder {
            order_number: o["OrderNumber"].clone(),
            rep: RepId(o["RepID"].parse().unwrap()),
            order_date: v1_date(&o["OrderDate"]),
            status: OrderStatus::try_from(o["OrderStatus"].as_str())
                .unwrap(),
            net_sales: t.subtotal,
            amount_paid,
            balance_due: t.total - amount_paid,
        });
    }
    let rates = rows("sales_reps.csv")
        .iter()
        .map(|r| {
            (
                RepId(r["RepID"].parse().unwrap()),
                float(&r["CommissionRate"]),
            )
        })
        .collect();
    September {
        orders,
        rates,
        tax_inputs,
    }
}

pub struct Return {
    pub number: String,
    pub order_on_file: bool,
    pub facts: ReturnFacts,
}

/// All 772 returns, joined the way `qryRMARouting` joins them.
pub fn all_returns() -> Vec<Return> {
    let orders: HashMap<String, (Date, Option<Date>)> =
        rows("rma_orders.csv")
            .into_iter()
            .map(|o| {
                (
                    o["OrderID"].clone(),
                    (
                        iso_date(&o["OrderDate"]).unwrap(),
                        iso_date(&o["ShipDate"]),
                    ),
                )
            })
            .collect();
    let category: HashMap<String, Category> = rows("products.csv")
        .into_iter()
        .map(|p| {
            (
                p["ProductID"].clone(),
                Category::try_from(p["Category"].as_str()).unwrap(),
            )
        })
        .collect();
    let tax_exempt: HashMap<String, bool> = rows("customers_tax.csv")
        .into_iter()
        .map(|c| (c["CustomerID"].clone(), c["TaxExempt"] == "1"))
        .collect();

    rows("rmas.csv")
        .into_iter()
        .map(|r| {
            let order = orders.get(&r["OrderID"]).copied();
            Return {
                number: r["RMANumber"].clone(),
                order_on_file: order.is_some(),
                facts: ReturnFacts {
                    reason: Reason::try_from(r["Reason"].as_str())
                        .unwrap(),
                    category: category[&r["ProductID"]],
                    extended_warranty: r["ExtendedWarranty"] == "1",
                    tax_exempt: tax_exempt[&r["CustomerID"]],
                    days_in_service: days_in_service(
                        iso_date(&r["OpenedDate"]).unwrap(),
                        order,
                    ),
                },
            }
        })
        .collect()
}
