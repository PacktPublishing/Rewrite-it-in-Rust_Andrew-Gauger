// Tests for vba:CairnForms.LineProductChanged (CairnForms L736-739)
//
// VBA source:
//   Public Function LineProductChanged(frm As Form)
//       If IsNull(frm!productID) Then Exit Function
//       frm!UnitPrice = DLookup("UnitPrice", "Products", "ProductID=" & frm!productID)
//   End Function
//
// Rules pinned:
//   1. Null productID  -> function exits, UnitPrice left untouched (L737).
//   2. Non-null productID -> UnitPrice = DLookup("UnitPrice","Products","ProductID=" & id) (L738).
//   3. No Active-flag check: an inactive product still gets its stored UnitPrice.
//   4. DLookup returns Null for a nonexistent ProductID, so a bad id nulls UnitPrice.
//
// Data evidence (Products table):
//   ProductID 1  -> UnitPrice 89.00,  Active 1
//   ProductID 5  -> UnitPrice 649.00, Active 1
//   ProductID 11 -> UnitPrice 79.00,  Active 0
//   ProductID 12 -> UnitPrice 449.00, Active 0

use crate::cairn_forms::line_product_changed;

/// Minimal stand-in for the Access form: the two controls the procedure touches.
struct Form {
    product_id: Option<i32>,
    unit_price: Option<f64>,
}

impl Form {
    fn new(product_id: Option<i32>, unit_price: Option<f64>) -> Self {
        Form { product_id, unit_price }
    }
}

#[test]
fn null_product_id_leaves_unit_price_untouched() {
    // L737: If IsNull(frm!productID) Then Exit Function
    // Pre-existing price must survive unchanged.
    let mut frm = Form::new(None, Some(123.45));
    line_product_changed(&mut frm);
    assert_eq!(frm.unit_price, Some(123.45));
}

#[test]
fn null_product_id_with_no_price_stays_null() {
    // Same early-exit path when the price was already null.
    let mut frm = Form::new(None, None);
    line_product_changed(&mut frm);
    assert_eq!(frm.unit_price, None);
}

#[test]
fn active_product_1_sets_unit_price_to_89_00() {
    // L738: DLookup("UnitPrice","Products","ProductID=1") -> 89.00
    let mut frm = Form::new(Some(1), None);
    line_product_changed(&mut frm);
    assert!((frm.unit_price.unwrap() - 89.00).abs() < f64::EPSILON);
}

#[test]
fn active_product_5_sets_unit_price_to_649_00() {
    // L738: DLookup("UnitPrice","Products","ProductID=5") -> 649.00
    let mut frm = Form::new(Some(5), Some(1.00));
    line_product_changed(&mut frm);
    assert!((frm.unit_price.unwrap() - 649.00).abs() < f64::EPSILON);
}

#[test]
fn inactive_product_11_still_gets_stored_price_79_00() {
    // The code does NOT check the Active flag: ProductID 11 is Active=0
    // but its stored UnitPrice 79.00 is still applied.
    let mut frm = Form::new(Some(11), None);
    line_product_changed(&mut frm);
    assert!((frm.unit_price.unwrap() - 79.00).abs() < f64::EPSILON);
}

#[test]
fn inactive_product_12_still_gets_stored_price_449_00() {
    // Second inactive row (Active=0) to confirm the rule is general, not row-specific.
    let mut frm = Form::new(Some(12), None);
    line_product_changed(&mut frm);
    assert!((frm.unit_price.unwrap() - 449.00).abs() < f64::EPSILON);
}

#[test]
fn nonexistent_product_id_nulls_unit_price() {
    // Access DLookup returns Null when no row matches; the code has no guard,
    // so a bad id overwrites the price with Null.
    let mut frm = Form::new(Some(9999), Some(50.00));
    line_product_changed(&mut frm);
    assert_eq!(frm.unit_price, None);
}
