//! Commission. V1 prints two figures for the same rep and month, and they
//! don't always agree (R24). Both are here. Nobody has decided which is paid.

use crate::date::Date;
use crate::ids::RepId;
use crate::rounding::access_round_product;
use crate::status::OrderStatus;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// One order as `qryCommissionDetail` sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommissionOrder {
    pub order_number: String,
    pub rep: RepId,
    pub order_date: Date,
    pub status: OrderStatus,
    /// Line totals after discount, before tax and shipping.
    pub net_sales: Decimal,
    pub amount_paid: Decimal,
    pub balance_due: Decimal,
}

/// One rep's commission statement for a month.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Statement {
    pub orders: usize,
    pub net_sales: Decimal,
    pub commission: Decimal,
    pub unpaid: Decimal,
    pub orders_with_no_payment: usize,
}

// FLAG: R22 Commission counts every order dated in the month unless it is Cancelled, paid or not. In September 2026, 82 of 136 orders had no payment.
fn counts(o: &CommissionOrder, month: Date) -> bool {
    o.status != OrderStatus::Cancelled
        && o.order_date.in_same_month(month)
}

/// The commission statement (`rptCommission` over `qryCommissionDetail`):
/// each order's commission rounded to the cent, then added up.
// FLAG: R24 The statement rounds each order and adds; Sales by Rep adds the month and rounds once. They differ in 397 of 818 rep-months.
pub fn statement(
    orders: &[CommissionOrder],
    rep: RepId,
    rate: f64,
    month: Date,
) -> Statement {
    let mine =
        orders.iter().filter(|o| o.rep == rep && counts(o, month));
    let mut s = Statement {
        orders: 0,
        net_sales: Decimal::ZERO,
        commission: Decimal::ZERO,
        unpaid: Decimal::ZERO,
        orders_with_no_payment: 0,
    };
    for o in mine {
        s.orders += 1;
        s.net_sales += o.net_sales;
        s.commission += access_round_product(o.net_sales, rate);
        s.unpaid += o.balance_due;
        if o.amount_paid.is_zero() {
            s.orders_with_no_payment += 1;
        }
    }
    s
}

/// The month-end Sales by Rep figure (`qryMonthEnd_SalesByRep`): the
/// month's net sales added up, then rounded once.
pub fn sales_by_rep_commission(
    orders: &[CommissionOrder],
    rep: RepId,
    rate: f64,
    month: Date,
) -> Decimal {
    let net_sales: Decimal = orders
        .iter()
        .filter(|o| o.rep == rep && counts(o, month))
        .map(|o| o.net_sales)
        .sum();
    access_round_product(net_sales, rate)
}

/// Every rep's statement for the month, added up (the report's last page).
pub fn all_reps(
    orders: &[CommissionOrder],
    rates: &[(RepId, f64)],
    month: Date,
) -> Statement {
    let mut total = statement(&[], RepId(0), 0.0, month);
    for &(rep, rate) in rates {
        let s = statement(orders, rep, rate, month);
        total.orders += s.orders;
        total.net_sales += s.net_sales;
        total.commission += s.commission;
        total.unpaid += s.unpaid;
        total.orders_with_no_payment += s.orders_with_no_payment;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn order(
        date: &str,
        status: OrderStatus,
        net: &str,
        paid: &str,
    ) -> CommissionOrder {
        CommissionOrder {
            order_number: "SO-2026-0001".into(),
            rep: RepId(7),
            order_date: date.parse().unwrap(),
            status,
            net_sales: dec(net),
            amount_paid: dec(paid),
            balance_due: dec(net) - dec(paid),
        }
    }

    #[test]
    fn cancelled_orders_and_other_months_dont_count() {
        let sept: Date = "2026-09-15".parse().unwrap();
        let orders = [
            order("2026-09-01", OrderStatus::Shipped, "1000", "0"),
            order("2026-09-30", OrderStatus::Cancelled, "1000", "0"),
            order("2026-10-01", OrderStatus::Shipped, "1000", "0"),
        ];
        let s = statement(&orders, RepId(7), 0.065, sept);
        assert_eq!((s.orders, s.commission), (1, dec("65.00")));
    }

    #[test]
    fn commission_is_paid_on_orders_nobody_has_paid_for() {
        let sept: Date = "2026-09-15".parse().unwrap();
        let s = statement(
            &[order(
                "2026-09-01",
                OrderStatus::Invoiced,
                "1000",
                "0",
            )],
            RepId(7),
            0.065,
            sept,
        );
        assert_eq!(
            (s.commission, s.orders_with_no_payment),
            (dec("65.00"), 1)
        );
    }

    #[test]
    fn rounding_each_order_and_rounding_the_month_can_differ_by_a_cent(
    ) {
        let sept: Date = "2026-09-15".parse().unwrap();
        // 6.25% is exact in binary. Each $2.00 order earns exactly 12.5
        // cents, which rounds to 12 (even). The month's $4.00 earns 25.
        let orders = [
            order("2026-09-01", OrderStatus::Shipped, "2.00", "0"),
            order("2026-09-02", OrderStatus::Shipped, "2.00", "0"),
        ];
        assert_eq!(
            statement(&orders, RepId(7), 0.0625, sept).commission,
            dec("0.24")
        );
        assert_eq!(
            sales_by_rep_commission(&orders, RepId(7), 0.0625, sept),
            dec("0.25")
        );
    }
}
