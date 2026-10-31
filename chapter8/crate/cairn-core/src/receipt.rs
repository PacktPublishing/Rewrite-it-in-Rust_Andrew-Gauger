//! Receipts: R10 and R11.

use crate::date::Date;
use crate::ids::PaymentId;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
pub struct Payment {
    pub id: PaymentId,
    pub date: Date,
    pub amount: Decimal,
}

/// R11: the payment ID, padded to 5 digits.
pub fn receipt_number(payment: PaymentId) -> String {
    format!("{:05}", payment.0)
}

/// R10, V1's `ReceiptPaidToDate`: what was paid on the order up to and
/// including this payment. Earlier dates first, then lower payment IDs on
/// the same day. `payments` are the order's payments.
pub fn paid_to_date(payments: &[Payment], this: &Payment) -> Decimal {
    payments
        .iter()
        .filter(|p| (p.date, p.id) <= (this.date, this.id))
        .map(|p| p.amount)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pay(id: i32, date: &str, amount: i64) -> Payment {
        Payment {
            id: PaymentId(id),
            date: date.parse().unwrap(),
            amount: Decimal::from(amount),
        }
    }

    #[test]
    fn receipt_numbers_are_five_digits() {
        assert_eq!(receipt_number(PaymentId(2)), "00002");
    }

    #[test]
    fn paid_to_date_counts_earlier_days_and_lower_ids_on_the_same_day(
    ) {
        let payments = [
            pay(9, "2026-09-12", 100),
            pay(4, "2026-09-15", 50),
            pay(7, "2026-09-15", 25),
        ];
        assert_eq!(
            paid_to_date(&payments, &payments[0]),
            Decimal::from(100)
        );
        assert_eq!(
            paid_to_date(&payments, &payments[1]),
            Decimal::from(150)
        );
        assert_eq!(
            paid_to_date(&payments, &payments[2]),
            Decimal::from(175)
        );
    }
}
