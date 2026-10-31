// Tests for vba:CairnForms.MfrRefs (CairnForms L926-941).
//
// VBA: Public Function MfrRefs(ByVal orderID As Variant) As Variant
//   MfrRefs = Null
//   If IsNull(orderID) Then Exit Function
//   On Error GoTo Done
//   Set rs = CurrentDb.OpenRecordset("SELECT ManufacturerRef FROM ManufacturerOrders
//       WHERE OrderID=" & orderID & " AND ManufacturerRef Is Not Null ORDER BY SentAt", dbOpenSnapshot)
//   Do Until rs.EOF
//       s = s & IIf(Len(s), ", ", "") & rs!ManufacturerRef
//       rs.MoveNext
//   Loop
//   If Len(s) Then MfrRefs = s
// Done:
//
// Assumed Rust surface (not implemented here):
//   fn mfr_refs(order_id: Option<i32>) -> Option<String>
//   fn seed_manufacturer_orders(order_id: i32, rows: &[(Option<&str>, &str)])
//       // each row: (manufacturerref, sentat)
//   fn drop_manufacturer_orders_table()

use crate::cairnforms::{drop_manufacturer_orders_table, mfr_refs, seed_manufacturer_orders};

// Rule 1 (L929-930): a Null orderID returns Null immediately, before any query.
#[test]
fn mfr_refs_null_order_id_returns_null() {
    assert_eq!(mfr_refs(None), None);
}

// Rule 4 (L929, L939): with no matching rows the accumulator stays empty and the
// result stays Null (initialized Null, only assigned when Len(s) > 0).
#[test]
fn mfr_refs_order_with_no_rows_returns_null() {
    seed_manufacturer_orders(100, &[]);
    assert_eq!(mfr_refs(Some(100)), None);
}

// Rule 3 (L935): a single ref is returned as-is, with no leading or trailing ", ".
#[test]
fn mfr_refs_single_ref_has_no_separator() {
    seed_manufacturer_orders(100, &[(Some("MFR-100231"), "2024-01-01 10:00:00")]);
    assert_eq!(mfr_refs(Some(100)), Some("MFR-100231".to_string()));
}

// Rules 2+3 (L932-933, L935): rows whose ManufacturerRef is Null (failed send
// attempts) are excluded, and the remaining refs are joined with ", " in
// SentAt order. Rows are seeded in the REVERSE of SentAt order to pin the
// ORDER BY SentAt, not insertion order.
#[test]
fn mfr_refs_excludes_null_refs_and_joins_in_sentat_order() {
    seed_manufacturer_orders(
        100,
        &[
            (Some("MFR-100232"), "2024-01-01 12:00:00"), // inserted first, latest SentAt
            (None, "2024-01-01 11:00:00"),              // failed attempt: ref is Null
            (Some("MFR-100231"), "2024-01-01 10:00:00"), // inserted last, earliest SentAt
        ],
    );
    assert_eq!(mfr_refs(Some(100)), Some("MFR-100231, MFR-100232".to_string()));
}

// Rule (L932): the WHERE clause matches OrderID exactly, so refs recorded for
// other orders never leak into the result.
#[test]
fn mfr_refs_ignores_refs_of_other_orders() {
    seed_manufacturer_orders(100, &[(Some("MFR-100231"), "2024-01-01 10:00:00")]);
    seed_manufacturer_orders(200, &[(Some("MFR-999999"), "2024-01-01 09:00:00")]);
    assert_eq!(mfr_refs(Some(100)), Some("MFR-100231".to_string()));
    assert_eq!(mfr_refs(Some(200)), Some("MFR-999999".to_string()));
}

// Rule 5 (L931, L939-941): if the ManufacturerOrders table does not exist
// (older databases), the error handler jumps to Done and the function returns
// the Null it was initialized with, rather than raising an error.
#[test]
fn mfr_refs_missing_table_returns_null() {
    drop_manufacturer_orders_table();
    assert_eq!(mfr_refs(Some(100)), None);
}

// ---- Added for the book (2026-10-28). Not part of the AI's output. ----
// Stubs with the signatures the tests above assume, so the file compiles.
// Each body is todo!() until the rule is implemented.
mod cairnforms {
    pub fn mfr_refs(_order_id: Option<i32>) -> Option<String> { todo!() }
    pub fn seed_manufacturer_orders(_order_id: i32, _rows: &[(Option<&str>, &str)]) { todo!() }
    pub fn drop_manufacturer_orders_table() { todo!() }
}
