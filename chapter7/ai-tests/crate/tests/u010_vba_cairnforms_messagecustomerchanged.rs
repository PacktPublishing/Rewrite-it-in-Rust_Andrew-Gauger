// Tests for vba:CairnForms.MessageCustomerChanged (CairnForms L741-746).
//
// VBA under test:
//   Public Function MessageCustomerChanged(frm As Form)
//       Dim cid: cid = frm!CustomerID
//       If IsNull(cid) Then Exit Function
//       frm!CallerCompany = DLookup("CompanyName", "Customers", "CustomerID=" & cid)
//       If IsNull(frm!CallerPhone) Then frm!CallerPhone = DLookup("Phone", "Customers", "CustomerID=" & cid)
//   End Function
//
// Assumed API (plausible signatures, not implemented here):
//   fn message_customer_changed(frm: &mut PhoneMessageForm)
//   fn customer_company_name(customer_id: i32) -> Option<String>  // DLookup("CompanyName", ...)
//   fn customer_phone(customer_id: i32) -> Option<String>         // DLookup("Phone", ...)
//
// Data facts (Customers table):
//   CustomerID 1 = "Northfield Water District", Phone "555-2100"
//   CustomerID 2 = "Bluepine Cold Storage",     Phone "555-3300"
//   Max CustomerID in data is 1000, so 999_999 matches no row (DLookup -> Null).

use crate::cairn_forms::{
    customer_company_name, customer_phone, message_customer_changed, PhoneMessageForm,
};

/// Rule 1 (L743): if frm!CustomerID is Null the function exits immediately
/// and changes nothing — neither CallerCompany nor CallerPhone is touched.
#[test]
fn null_customer_id_changes_nothing() {
    let mut frm = PhoneMessageForm {
        customer_id: None,
        caller_company: Some("Stale Company".to_string()),
        caller_phone: Some("555-0000".to_string()),
    };
    message_customer_changed(&mut frm);
    assert_eq!(frm.customer_id, None);
    assert_eq!(frm.caller_company.as_deref(), Some("Stale Company"));
    assert_eq!(frm.caller_phone.as_deref(), Some("555-0000"));
}

/// Rule 2 (L744): a non-null CustomerID unconditionally overwrites
/// CallerCompany with the customer's CompanyName, even if a stale value
/// was already present.
#[test]
fn valid_customer_overwrites_stale_caller_company() {
    let mut frm = PhoneMessageForm {
        customer_id: Some(1),
        caller_company: Some("Stale Company".to_string()),
        caller_phone: None,
    };
    message_customer_changed(&mut frm);
    assert_eq!(
        frm.caller_company.as_deref(),
        customer_company_name(1).as_deref(),
        "CallerCompany must be the customer's CompanyName"
    );
    assert_eq!(frm.caller_company.as_deref(), Some("Northfield Water District"));
}

/// Rule 3 (L745): a non-null existing CallerPhone is never overwritten,
/// even when the selected customer has a different phone on file.
#[test]
fn existing_caller_phone_is_never_overwritten() {
    let mut frm = PhoneMessageForm {
        customer_id: Some(2),
        caller_company: None,
        caller_phone: Some("555-9999".to_string()),
    };
    message_customer_changed(&mut frm);
    assert_eq!(frm.caller_phone.as_deref(), Some("555-9999"));
    // Company still gets filled from the customer.
    assert_eq!(frm.caller_company.as_deref(), Some("Bluepine Cold Storage"));
}

/// Rule 3 (L745): a Null CallerPhone is filled from the customer's Phone.
#[test]
fn null_caller_phone_is_filled_from_customer() {
    let mut frm = PhoneMessageForm {
        customer_id: Some(2),
        caller_company: None,
        caller_phone: None,
    };
    message_customer_changed(&mut frm);
    assert_eq!(
        frm.caller_phone.as_deref(),
        customer_phone(2).as_deref(),
        "Null CallerPhone must be filled with the customer's Phone"
    );
    assert_eq!(frm.caller_phone.as_deref(), Some("555-3300"));
    assert_eq!(frm.caller_company.as_deref(), Some("Bluepine Cold Storage"));
}

/// DLookup semantics: a CustomerID that matches no row yields Null, so
/// CallerCompany becomes Null (overwriting any prior value) and a Null
/// CallerPhone stays Null.
#[test]
fn unknown_customer_yields_null_company_and_phone() {
    let mut frm = PhoneMessageForm {
        customer_id: Some(999_999),
        caller_company: Some("Stale Company".to_string()),
        caller_phone: None,
    };
    message_customer_changed(&mut frm);
    assert_eq!(
        frm.caller_company.as_deref(),
        customer_company_name(999_999).as_deref(),
        "DLookup on a missing customer returns Null"
    );
    assert_eq!(frm.caller_company, None);
    assert_eq!(frm.caller_phone, None);
}
