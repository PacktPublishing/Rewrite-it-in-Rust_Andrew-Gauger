//! Tests for query:qryOpenQuoteValue
//!
//! Saved SQL:  SELECT CustomerID, Max(QuoteTotal) FROM [qryQuoteTotals] WHERE QuoteStatus = 'Sent'
//! Canonical (CairnBuild L151):
//!   SELECT CustomerID, Max(QuoteTotal) AS OpenQuote FROM qryQuoteTotals
//!   WHERE QuoteStatus = 'Sent' GROUP BY CustomerID;
//!
//! Rules pinned here:
//!   1. Only quotes with QuoteStatus = 'Sent' count as "open".
//!   2. Per customer the open quote value is the MAX of QuoteTotal (not the sum).
//!   3. A customer with no Sent quote produces no row.
//!
//! QuoteTotal comes from qryQuoteTotals (CairnBuild L506-511):
//!   Subtotal  = Nz(Sum(l.LineTotal), 0)
//!   TaxAmount = Round(Subtotal * TaxRate, 2)
//!   QuoteTotal= Subtotal + TaxAmount + ShippingCost
//! where each LineTotal = Round(Quantity * UnitPrice * (1 - DiscountPct), 2) (L502).
//!
//! The harness is assumed to expose:
//!   fn open_quote_value(quotes: &[Quote]) -> Vec<(i64, f64)>
//! returning one (CustomerID, OpenQuote) pair per customer that has >= 1 Sent quote.

use std::collections::HashMap;

#[allow(dead_code)]
struct QuoteLine {
    quantity: f64,
    unit_price: f64,
    discount_pct: f64,
}

#[allow(dead_code)]
struct Quote {
    customer_id: i64,
    quote_status: &'static str,
    tax_rate: f64,
    shipping_cost: f64,
    lines: Vec<QuoteLine>,
}

fn line(q: f64, p: f64, d: f64) -> QuoteLine {
    QuoteLine { quantity: q, unit_price: p, discount_pct: d }
}

fn sent(c: i64, tax: f64, ship: f64, lines: Vec<QuoteLine>) -> Quote {
    Quote { customer_id: c, quote_status: "Sent", tax_rate: tax, shipping_cost: ship, lines }
}

fn draft(c: i64, tax: f64, ship: f64, lines: Vec<QuoteLine>) -> Quote {
    Quote { customer_id: c, quote_status: "Draft", tax_rate: tax, shipping_cost: ship, lines }
}

fn accepted(c: i64, tax: f64, ship: f64, lines: Vec<QuoteLine>) -> Quote {
    Quote { customer_id: c, quote_status: "Accepted", tax_rate: tax, shipping_cost: ship, lines }
}

fn map_of(pairs: &[(i64, f64)]) -> HashMap<i64, f64> {
    pairs.iter().copied().collect()
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.005
}

/// Rule 1: only QuoteStatus = 'Sent' is treated as open.
/// Customer 1 has a Draft quote (total 9000) and a Sent quote (total 5000).
/// The Draft must be ignored, so the open value is 5000, not 9000.
#[test]
fn open_quote_value_ignores_non_sent_statuses() {
    let quotes = vec![
        draft(1, 0.0, 0.0, vec![line(1.0, 9000.0, 0.0)]),
        sent(1, 0.0, 0.0, vec![line(1.0, 5000.0, 0.0)]),
    ];
    let got = map_of(&open_quote_value(&quotes));
    assert_eq!(got.get(&1).copied(), Some(5000.0));
}

/// Rule 2: the open value is the MAX of QuoteTotal across a customer's Sent
/// quotes, not the sum. Customer 2 has Sent quotes totalling 1000 and 5332.68.
#[test]
fn open_quote_value_is_max_not_sum_across_sent_quotes() {
    let quotes = vec![
        sent(2, 0.0, 0.0, vec![line(1.0, 1000.0, 0.0)]),
        sent(2, 0.0, 0.0, vec![line(1.0, 5332.68, 0.0)]),
    ];
    let got = map_of(&open_quote_value(&quotes));
    assert_eq!(got.get(&2).copied(), Some(5332.68));
}

/// Full QuoteTotal pipeline on real data (quote 3, customer 4, Sent,
/// TaxRate 0.0775, Shipping 40.00; lines 30x89, 6x199, 1x649, 1x399, disc 0).
/// Subtotal 4912.00, Tax Round(4912*0.0775,2)=380.68, QuoteTotal 5332.68.
#[test]
fn open_quote_value_real_quote3_customer4_is_5332_68() {
    let quotes = vec![sent(
        4,
        0.0775,
        40.0,
        vec![
            line(30.0, 89.0, 0.0),
            line(6.0, 199.0, 0.0),
            line(1.0, 649.0, 0.0),
            line(1.0, 399.0, 0.0),
        ],
    )];
    let got = map_of(&open_quote_value(&quotes));
    assert_eq!(got.get(&4).copied(), Some(5332.68));
}

/// Zero tax rate: QuoteTotal = Subtotal + Shipping.
/// Real data (quote 18978, customer 1, Sent, TaxRate 0, Shipping 103.00;
/// lines 72x199, 2x649, 72x19, 72x36, disc 0).
/// Subtotal 19586.00, Tax 0, QuoteTotal 19689.00.
#[test]
fn open_quote_value_zero_tax_is_subtotal_plus_shipping() {
    let quotes = vec![sent(
        1,
        0.0,
        103.0,
        vec![
            line(72.0, 199.0, 0.0),
            line(2.0, 649.0, 0.0),
            line(72.0, 19.0, 0.0),
            line(72.0, 36.0, 0.0),
        ],
    )];
    let got = map_of(&open_quote_value(&quotes));
    assert!(approx(got.get(&1).copied().unwrap_or(f64::NAN), 19689.0));
}

/// Rule 3: a customer whose quotes are all non-Sent yields no row at all
/// (the WHERE is applied before the GROUP BY).
#[test]
fn customer_with_no_sent_quote_is_absent() {
    let quotes = vec![
        draft(5, 0.0, 0.0, vec![line(1.0, 1000.0, 0.0)]),
        accepted(5, 0.0, 0.0, vec![line(1.0, 2000.0, 0.0)]),
    ];
    let got = map_of(&open_quote_value(&quotes));
    assert!(!got.contains_key(&5));
}

/// Multiple customers are aggregated independently: each gets its own MAX.
#[test]
fn open_quote_value_computed_independently_per_customer() {
    let quotes = vec![
        sent(1, 0.0, 0.0, vec![line(1.0, 1000.0, 0.0)]),
        sent(1, 0.0, 0.0, vec![line(1.0, 2000.0, 0.0)]),
        sent(2, 0.0, 0.0, vec![line(1.0, 3000.0, 0.0)]),
    ];
    let got = map_of(&open_quote_value(&quotes));
    assert_eq!(got.get(&1).copied(), Some(2000.0));
    assert_eq!(got.get(&2).copied(), Some(3000.0));
}
