//! Tests for vba:CairnForms.DocLoad (CairnForms L693-698).
//!
//! DocLoad is the OnLoad handler for frmQuotes/frmOrders; it presets the
//! customer when a quote/order is opened from a customer record:
//!   L695: If IsNull(frm.OpenArgs) Then Exit Function
//!   L696: frm!CustomerID = CLng(frm.OpenArgs)
//!   L697: DocCustomerChanged frm
//!
//! The cascade (DocCustomerChanged, L706-719) then sets:
//!   taxRate   = 0 if Customers.TaxExempt else CompanyInfo.DefaultTaxRate (L709-713)
//!   repID     = Customers.AssignedRepID only if currently Null (L714)
//!   contactID = the customer's IsPrimary=True contact (L715-716)
//!
//! Data: Customers 1=Northfield Water District (TaxExempt=1, Rep 11),
//! 2=Bluepine Cold Storage (TaxExempt=0, Rep 1), 3=Harlow Manufacturing
//! (TaxExempt=0, Rep 1). Primary contacts: cust 1->1, 2->2, 3->3.
//! CompanyInfo.DefaultTaxRate = 0.0825.

use crate::cairn_forms::doc_load;

/// Test double for the quote/order form.
#[derive(Default, Clone, Debug)]
struct Form {
    open_args: Option<i64>,
    customer_id: Option<i64>,
    tax_rate: Option<f64>,
    rep_id: Option<i64>,
    contact_id: Option<i64>,
}

fn assert_close(actual: Option<f64>, expected: f64) {
    let a = actual.expect("value should be set");
    assert!((a - expected).abs() < 1e-9, "expected {expected}, got {a}");
}

#[test]
fn doc_load_null_open_args_does_nothing() {
    // L695: IsNull(OpenArgs) -> Exit Function; CustomerID unset, no cascade.
    let mut frm = Form { open_args: None, ..Default::default() };
    doc_load(&mut frm);
    assert_eq!(frm.customer_id, None);
    assert_eq!(frm.tax_rate, None);
    assert_eq!(frm.rep_id, None);
    assert_eq!(frm.contact_id, None);
}

#[test]
fn doc_load_open_args_2_presets_bluepine() {
    // L696: CustomerID = CLng("2"); cascade: TaxExempt=0 -> DefaultTaxRate,
    // Rep 1 (was null), primary contact 2.
    let mut frm = Form { open_args: Some(2), ..Default::default() };
    doc_load(&mut frm);
    assert_eq!(frm.customer_id, Some(2));
    assert_close(frm.tax_rate, 0.0825);
    assert_eq!(frm.rep_id, Some(1));
    assert_eq!(frm.contact_id, Some(2));
}

#[test]
fn doc_load_open_args_1_presets_tax_exempt_northfield() {
    // L709-710: TaxExempt=1 -> taxRate = 0; Rep 11; primary contact 1.
    let mut frm = Form { open_args: Some(1), ..Default::default() };
    doc_load(&mut frm);
    assert_eq!(frm.customer_id, Some(1));
    assert_close(frm.tax_rate, 0.0);
    assert_eq!(frm.rep_id, Some(11));
    assert_eq!(frm.contact_id, Some(1));
}

#[test]
fn doc_load_open_args_3_presets_harlow() {
    // TaxExempt=0 -> DefaultTaxRate; Rep 1; primary contact 3.
    let mut frm = Form { open_args: Some(3), ..Default::default() };
    doc_load(&mut frm);
    assert_eq!(frm.customer_id, Some(3));
    assert_close(frm.tax_rate, 0.0825);
    assert_eq!(frm.rep_id, Some(1));
    assert_eq!(frm.contact_id, Some(3));
}

#[test]
fn doc_load_keeps_existing_rep_id() {
    // L714: repID is only defaulted when currently Null; a preset rep stays.
    let mut frm = Form { open_args: Some(2), rep_id: Some(5), ..Default::default() };
    doc_load(&mut frm);
    assert_eq!(frm.rep_id, Some(5));
    assert_eq!(frm.customer_id, Some(2));
    assert_close(frm.tax_rate, 0.0825);
    assert_eq!(frm.contact_id, Some(2));
}

// ---- Added for the book (2026-10-28). Not part of the AI's output. ----
// Stubs with the signatures the tests above assume, so the file compiles.
// Each body is todo!() until the rule is implemented.
mod cairn_forms {
    use super::Form;
    pub fn doc_load(_frm: &mut Form) { todo!() }
}
