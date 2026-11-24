//! Quote and order totals: `qryQuoteLines`/`qryOrderLines` and
//! `qryQuoteTotals`/`qryOrderTotals`.

use crate::ids::ProductId;
use crate::rounding::{access_round, access_round_product, to_f64};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// One line of a quote or an order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocLine {
    pub product: ProductId,
    /// R02: blank prints the product's name.
    pub description: Option<String>,
    pub quantity: i32,
    pub unit_price: Decimal,
    /// A fraction, 0.05 for 5%. A `Double` in V1.
    pub discount: f64,
}

/// R01: quantity × price × (1 − discount), rounded to the cent.
pub fn line_total(line: &DocLine) -> Decimal {
    let gross = Decimal::from(line.quantity) * line.unit_price;
    access_round(to_f64(gross) * (1.0 - line.discount))
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
pub struct Totals {
    pub subtotal: Decimal,
    pub tax: Decimal,
    pub shipping: Decimal,
    pub total: Decimal,
}

/// R03, R04: subtotal is the sum of line totals (0 with no lines). Tax is
/// the subtotal × the rate, rounded. Shipping is never taxed.
pub fn totals(
    lines: &[DocLine],
    tax_rate: f64,
    shipping: Decimal,
) -> Totals {
    let subtotal: Decimal = lines.iter().map(line_total).sum();
    let tax = access_round_product(subtotal, tax_rate);
    Totals {
        subtotal,
        tax,
        shipping,
        total: subtotal + tax + shipping,
    }
}

/// R07, R08: paid is the sum of payments; an order with none owes its
/// whole total.
pub fn balance_due(total: Decimal, payments: &[Decimal]) -> Decimal {
    total - payments.iter().sum::<Decimal>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn line(quantity: i32, price: &str, discount: f64) -> DocLine {
        DocLine {
            product: ProductId(1),
            description: None,
            quantity,
            unit_price: dec(price),
            discount,
        }
    }

    #[test]
    fn a_line_is_quantity_times_price_less_discount() {
        assert_eq!(
            line_total(&line(120, "89.00", 0.05)),
            dec("10146.00")
        );
    }

    #[test]
    fn shipping_is_never_taxed() {
        let t =
            totals(&[line(10, "100.00", 0.0)], 0.0825, dec("75.00"));
        assert_eq!(
            t,
            Totals {
                subtotal: dec("1000.00"),
                tax: dec("82.50"),
                shipping: dec("75.00"),
                total: dec("1157.50")
            }
        );
    }

    #[test]
    fn no_lines_is_a_zero_subtotal() {
        assert_eq!(totals(&[], 0.0825, dec("0")).subtotal, dec("0"));
    }

    #[test]
    fn unpaid_orders_owe_their_whole_total() {
        assert_eq!(balance_due(dec("1157.50"), &[]), dec("1157.50"));
        assert_eq!(
            balance_due(
                dec("1157.50"),
                &[dec("1000"), dec("157.50")]
            ),
            dec("0")
        );
    }
}
