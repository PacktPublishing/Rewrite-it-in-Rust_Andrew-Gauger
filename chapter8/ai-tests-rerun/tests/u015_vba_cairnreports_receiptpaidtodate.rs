// Tests for vba:CairnReports.ReceiptPaidToDate (CairnReports L502-510).
//
// Rule under test:
//   ReceiptPaidToDate(paymentID) = Nz(DSum("Amount", "Payments",
//       "OrderID=<oid> AND (PaymentDate < <d> OR (PaymentDate = <d> AND PaymentID <= <paymentID>))"), 0)
//   where oid/d come from DLookup on Payments by PaymentID; if the paymentID
//   does not exist (oid IsNull) the function returns 0 (Currency default).
//
// Same-day payments are ordered by PaymentID; the result always includes the
// payment itself; payments on other orders are never included.
//
// Concrete data (Payments table):
//   PaymentID 1: OrderID 1, 08/26/26 00:00:00, 6262.00
//   PaymentID 2: OrderID 1, 09/24/26 00:00:00, 6262.00
//   PaymentID 3: OrderID 2, 09/12/26 00:00:00, 7000.00
//   PaymentID 4: OrderID 3, 09/15/26 00:00:00, 3109.17
//   PaymentID 5: OrderID 5, 01/06/16 00:00:00, 43439.36
//   PaymentID 6: OrderID 5, 03/25/16 00:00:00, 43439.36
//   PaymentID 7: OrderID 6, 02/10/16 00:00:00, 31695.84
//   PaymentID 8: OrderID 7, 01/04/16 00:00:00, 20021.04
//   PaymentID 9: OrderID 7, 02/15/16 00:00:00, 20021.04
//   PaymentID 10: OrderID 8, 01/05/16 00:00:00, 1572.50

use crate::cairn_reports::ReceiptPaidToDate;

#[test]
fn nonexistent_payment_returns_zero() {
    // L507: If IsNull(oid) Then Exit Function -> Currency default 0.
    assert_eq!(ReceiptPaidToDate(999_999), 0.0);
}

#[test]
fn first_payment_of_order_returns_its_own_amount() {
    // Order 1, payment 1 (08/26/26): only itself qualifies (no earlier payment).
    assert_eq!(ReceiptPaidToDate(1), 6262.00);
}

#[test]
fn later_payment_includes_all_earlier_payments_on_same_order() {
    // Order 1, payment 2 (09/24/26): payment 1 (08/26/26) is strictly earlier,
    // plus itself -> 6262.00 + 6262.00.
    assert_eq!(ReceiptPaidToDate(2), 12524.00);
}

#[test]
fn payments_on_other_orders_are_excluded() {
    // Order 5 has two payments (5, 6). Payment 6 must include only order 5's
    // payments, not the 6262.00 payments of order 1 or the 7000.00 of order 2.
    assert_eq!(ReceiptPaidToDate(6), 86878.72); // 43439.36 * 2
}

#[test]
fn single_payment_order_returns_its_amount() {
    // Order 2 has exactly one payment.
    assert_eq!(ReceiptPaidToDate(3), 7000.00);
    // Order 3 has exactly one payment.
    assert_eq!(ReceiptPaidToDate(4), 3109.17);
}

#[test]
fn second_of_two_same_order_payments_sums_both() {
    // Order 7: payment 8 (01/04/16) then payment 9 (02/15/16).
    assert_eq!(ReceiptPaidToDate(8), 20021.04);
    assert_eq!(ReceiptPaidToDate(9), 40042.08);
}

// ---- Added for the book (2026-10-28). Not part of the AI's output. ----
// Stubs with the signatures the tests above assume, so the file compiles.
// Each body is todo!() until the rule is implemented.
#[path = "common/adapt.rs"]
mod adapt;
mod cairn_reports {
    // ---- Wired to cairn-core for Chapter 8 (2026-10-30). Not part of the AI's output. ----
    // Reads Payments from the History database (V1's DLookup/DSum), then
    // cairn-core's paid_to_date decides which payments count.
    use super::adapt;
    use cairn_core::{Payment, PaymentId};

    #[allow(non_snake_case)]
    pub fn ReceiptPaidToDate(payment_id: i32) -> f64 {
        let t = adapt::table("Payments");
        let payment = |id: i32, r: &std::collections::HashMap<String, String>| Payment {
            id: PaymentId(id),
            date: adapt::v1_date(&r["PaymentDate"]),
            amount: r["Amount"].parse().unwrap(),
        };
        let Some(this_row) = t.get(&payment_id) else {
            adapt::not_representable("a payment that doesn't exist (paid_to_date takes a Payment)")
        };
        let this = payment(payment_id, this_row);
        let order: Vec<Payment> = t
            .iter()
            .filter(|(_, r)| r["OrderID"] == this_row["OrderID"])
            .map(|(&id, r)| payment(id, r))
            .collect();
        adapt::f64_of(cairn_core::paid_to_date(&order, &this))
    }
}
