//! Tests for query:qryOrderPayments
//!
//! Saved SQL: SELECT OrderID, Sum(Amount), Max(PaymentDate) FROM [Payments]
//! (Access implicitly groups on the non-aggregated OrderID; canonical form per
//! CairnBuild L550: ... GROUP BY OrderID, with aliases AmountPaid / LastPaymentDate.)
//!
//! Schema (Payments): OrderID INTEGER NOT NULL FK->Orders; PaymentDate TIMESTAMP
//! nullable; Amount NUMERIC(15,2) NOT NULL.
//!
//! Assumed function (not implemented here): rolls up one order's payment rows
//! into the query's output row. Returns None when the order has no payment rows
//! (plain aggregation over Payments, no LEFT JOIN — the zero default is applied
//! downstream in qryOrderBalances via Nz(p.AmountPaid, 0)).

use chrono::NaiveDate;
use decimal::prelude::PrimitiveDecimal;
use decimal::Decimal;

fn d(s: &str) -> Decimal {
    s.parse::<Decimal>().unwrap()
}

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// One row of the Payments table as the query sees it.
struct PaymentRow {
    amount: Decimal,
    payment_date: Option<NaiveDate>,
}

/// Assumed: the per-order rollup implemented by qryOrderPayments.
/// Returns (AmountPaid, LastPaymentDate) for the order, or None if the order
/// has no payment rows.
fn order_payments(order_id: i32, rows: &[PaymentRow]) -> Option<(Decimal, Option<NaiveDate>)> {
    let _ = order_id;
    unimplemented!("assumed function for query:qryOrderPayments")
}

/// Rule 1: AmountPaid = Sum(Amount) over all of the order's payment rows.
/// Data example: order 1 has 6262.00 (2026-08-26) + 6262.00 (2026-09-24)
/// => 12524.00.
#[test]
fn amount_paid_is_sum_of_all_payment_rows() {
    let rows = vec![
        PaymentRow { amount: d("6262.00"), payment_date: Some(date("2026-08-26")) },
        PaymentRow { amount: d("6262.00"), payment_date: Some(date("2026-09-24")) },
    ];
    let (paid, _) = order_payments(1, &rows).expect("order 1 has payments, so the query emits a row");
    assert_eq!(paid, d("12524.00"));
}

/// Rule 1 (single-row group): order 2 has one payment of 7000.00 => 7000.00.
#[test]
fn amount_paid_single_payment_row() {
    let rows = vec![PaymentRow { amount: d("7000.00"), payment_date: Some(date("2026-09-12")) }];
    let (paid, _) = order_payments(2, &rows).unwrap();
    assert_eq!(paid, d("7000.00"));
}

/// Rule 1 (fractional cents preserved, NUMERIC(15,2)): order 5 has
/// 43439.36 + 43439.36 => 86878.72.
#[test]
fn amount_paid_sums_fractional_amounts() {
    let rows = vec![
        PaymentRow { amount: d("43439.36"), payment_date: Some(date("2016-01-06")) },
        PaymentRow { amount: d("43439.36"), payment_date: Some(date("2016-03-25")) },
    ];
    let (paid, _) = order_payments(5, &rows).unwrap();
    assert_eq!(paid, d("86878.72"));
}

/// Rule 2: LastPaymentDate = Max(PaymentDate) over the order's payments.
/// Data example: order 7 has 20021.04 (2016-01-04) + 20021.04 (2016-02-15)
/// => LastPaymentDate 2016-02-15.
#[test]
fn last_payment_date_is_max_of_payment_dates() {
    let rows = vec![
        PaymentRow { amount: d("20021.04"), payment_date: Some(date("2016-01-04")) },
        PaymentRow { amount: d("20021.04"), payment_date: Some(date("2016-02-15")) },
    ];
    let (_, last) = order_payments(7, &rows).unwrap();
    assert_eq!(last, Some(date("2016-02-15")));
}

/// Rule 2: PaymentDate is nullable in the schema and SQL Max ignores NULLs.
/// One dated row (2026-09-24) + one NULL-dated row => Max is the dated one.
#[test]
fn last_payment_date_ignores_null_dates() {
    let rows = vec![
        PaymentRow { amount: d("100.00"), payment_date: None },
        PaymentRow { amount: d("200.00"), payment_date: Some(date("2026-09-24")) },
    ];
    let (_, last) = order_payments(9, &rows).unwrap();
    assert_eq!(last, Some(date("2026-09-24")));
}

/// Rule 2: if every payment row of an order has a NULL PaymentDate, Max is
/// NULL, but the order still produces a row (it has payments).
#[test]
fn last_payment_date_is_null_when_all_dates_null() {
    let rows = vec![
        PaymentRow { amount: d("100.00"), payment_date: None },
        PaymentRow { amount: d("50.00"), payment_date: None },
    ];
    let (paid, last) = order_payments(9, &rows).unwrap();
    assert_eq!(paid, d("150.00"));
    assert_eq!(last, None);
}

/// Rule 4: the query is a plain aggregation over Payments (no LEFT JOIN), so
/// an order with no payment rows produces NO row. The zero default for such
/// orders is applied downstream in qryOrderBalances via Nz(p.AmountPaid, 0).
#[test]
fn order_without_payments_produces_no_row() {
    assert!(order_payments(42, &[]).is_none());
}

/// Rule 5: output columns are (OrderID, AmountPaid, LastPaymentDate) — the
/// rollup returns AmountPaid first, LastPaymentDate second, in that order.
/// Data example: order 1 => (12524.00, 2026-09-24).
#[test]
fn output_is_amount_paid_then_last_payment_date() {
    let rows = vec![
        PaymentRow { amount: d("6262.00"), payment_date: Some(date("2026-08-26")) },
        PaymentRow { amount: d("6262.00"), payment_date: Some(date("2026-09-24")) },
    ];
    let (paid, last) = order_payments(1, &rows).unwrap();
    assert_eq!(paid, d("12524.00"));
    assert_eq!(last, Some(date("2026-09-24")));
}
