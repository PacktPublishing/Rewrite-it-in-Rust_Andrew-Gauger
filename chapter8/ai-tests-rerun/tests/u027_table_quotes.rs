// Tests for table:Quotes business rules.
// Evidence:
//   - Schema: quotes_uq_quotes_number_idx (QuoteNumber UNIQUE NOT NULL),
//     CustomerID/RepID NOT NULL, ContactID nullable, FKs to Customers/Contacts/SalesReps.
//   - CairnBuild L260-268: QuoteStatus DEFAULT 'Draft', TaxRate DEFAULT 0,
//     ShippingCost DEFAULT 0, QuoteDate DEFAULT Date().
//   - CairnForms L49: VL_QUOTESTATUS = "Draft;Sent;Accepted;Declined;Expired".
//   - CairnForms L414: ValidUntil DefaultValue "=Date()+30".
//   - CairnForms L706-719 (DocCustomerChanged): tax/rep/contact defaults on customer pick.

use crate::quotes::*;

// ---- QuoteStatus list (CairnForms L49) ----

#[test]
fn quote_status_list_is_exactly_five_values_in_order() {
    // VL_QUOTESTATUS = "Draft;Sent;Accepted;Declined;Expired"
    assert_eq!(
        quote_status_list(),
        vec!["Draft", "Sent", "Accepted", "Declined", "Expired"]
    );
}

#[test]
fn quote_status_list_has_no_duplicates() {
    let mut v = quote_status_list();
    let n = v.len();
    v.sort();
    v.dedup();
    assert_eq!(v.len(), n, "status list must not contain duplicates");
}

// ---- Default QuoteStatus (CairnBuild L264 DEFAULT 'Draft') ----

#[test]
fn default_quote_status_is_draft() {
    assert_eq!(default_quote_status(), "Draft");
}

// ---- ValidUntil default = QuoteDate + 30 days (CairnForms L414) ----

fn d(y: i32, m: u32, day: u32) -> Date {
    Date::from_ymd(y, m, day)
}

#[test]
fn default_valid_until_is_quote_date_plus_30_days_same_month() {
    // 2024-01-15 + 30 days = 2024-02-14 (Jan has 31 days: 15+16=31, +14=Feb 14)
    assert_eq!(default_valid_until(d(2024, 1, 15)), d(2024, 2, 14));
}

#[test]
fn default_valid_until_is_quote_date_plus_30_days_crossing_year() {
    // 2024-12-25 + 30 days = 2025-01-24 (Dec 25 + 6 = Dec 31, + 24 = Jan 24)
    assert_eq!(default_valid_until(d(2024, 12, 25)), d(2025, 1, 24));
}

#[test]
fn default_valid_until_is_quote_date_plus_30_days_leap_feb() {
    // 2024-02-10 + 30 days = 2024-03-11 (Feb 2024 has 29 days: 10+19=29, +11=Mar 11)
    assert_eq!(default_valid_until(d(2024, 2, 10)), d(2024, 3, 11));
}

// ---- TaxRate default on customer change (CairnForms L709-713) ----
// If Customers.TaxExempt then TaxRate = 0, else TaxRate = Nz(CompanyInfo.DefaultTaxRate, 0).

#[test]
fn default_tax_rate_is_zero_when_customer_is_tax_exempt() {
    // Even with a non-zero company default, an exempt customer gets 0.
    assert_eq!(default_tax_rate(true, 0.0725), 0.0);
}

#[test]
fn default_tax_rate_uses_company_default_when_not_exempt() {
    assert_eq!(default_tax_rate(false, 0.0725), 0.0725);
}

#[test]
fn default_tax_rate_uses_company_default_value() {
    // Observed rates in data: 0, 0.0725, 0.0775, 0.0825.
    assert_eq!(default_tax_rate(false, 0.0825), 0.0825);
}

// ---- Rep default on customer change (CairnForms L714) ----
// If RepID is null, set to Customers.AssignedRepID.

#[test]
fn default_rep_is_the_customers_assigned_rep() {
    let c = Customer {
        tax_exempt: false,
        assigned_rep_id: Some(7),
        primary_contact_id: Some(42),
    };
    assert_eq!(default_rep(&c), Some(7));
}

#[test]
fn default_rep_is_none_when_customer_has_no_assigned_rep() {
    let c = Customer {
        tax_exempt: false,
        assigned_rep_id: None,
        primary_contact_id: Some(42),
    };
    assert_eq!(default_rep(&c), None);
}

// ---- Contact default on customer change (CairnForms L716) ----
// ContactID = DLookup("ContactID","Contacts","CustomerID=cid AND IsPrimary=True").

#[test]
fn default_contact_is_the_customers_primary_contact() {
    let c = Customer {
        tax_exempt: false,
        assigned_rep_id: Some(7),
        primary_contact_id: Some(42),
    };
    assert_eq!(default_contact(&c), Some(42));
}

#[test]
fn default_contact_is_none_when_customer_has_no_primary_contact() {
    let c = Customer {
        tax_exempt: false,
        assigned_rep_id: Some(7),
        primary_contact_id: None,
    };
    assert_eq!(default_contact(&c), None);
}

// ---- Customer-change guard (CairnForms L708: If IsNull(cid) Then Exit Function) ----
// When no customer is picked, none of the tax/rep/contact defaults are applied.

#[test]
fn customer_change_is_noop_when_no_customer_selected() {
    // No customer -> tax rate, rep and contact are left untouched (None / unchanged).
    let r = apply_customer_change(None);
    assert!(r.is_none(), "no customer selected must not set any defaults");
}

#[test]
fn customer_change_sets_all_three_defaults_when_customer_selected() {
    let c = Customer {
        tax_exempt: true,
        assigned_rep_id: Some(7),
        primary_contact_id: Some(42),
    };
    let r = apply_customer_change(Some(&c));
    let r = r.expect("a selected customer must produce defaults");
    assert_eq!(r.tax_rate, 0.0, "exempt customer -> tax rate 0");
    assert_eq!(r.rep_id, Some(7));
    assert_eq!(r.contact_id, Some(42));
}

// ---- Added for the book (2026-10-28). Not part of the AI's output. ----
// Stubs with the signatures the tests above assume, so the file compiles.
// Each body is todo!() until the rule is implemented.
#[path = "common/adapt.rs"]
mod adapt;
mod quotes {
    #[derive(Debug, PartialEq, Clone, Copy)]
    pub struct Date;
    impl Date {
        pub fn from_ymd(_y: i32, _m: u32, _d: u32) -> Self { todo!() }
    }
    pub struct Customer { pub tax_exempt: bool, pub assigned_rep_id: Option<i32>, pub primary_contact_id: Option<i32> }
    pub struct Defaults { pub tax_rate: f64, pub rep_id: Option<i32>, pub contact_id: Option<i32> }
    pub fn quote_status_list() -> Vec<&'static str> { todo!() }
    pub fn default_quote_status() -> &'static str { todo!() }
    pub fn default_valid_until(_quote_date: Date) -> Date { todo!() }
    // ---- Wired to cairn-core for Chapter 8 (2026-10-30). Not part of the AI's output. ----
    // The adapter converts types and calls cairn-core; see tests/common/adapt.rs.
    use super::adapt;
    use cairn_core::{ContactId, CustomerRow, RepId};

    fn row(c: &Customer) -> CustomerRow {
        let Some(rep) = c.assigned_rep_id else {
            adapt::not_representable("a customer with no assigned rep (CustomerRow requires one, R43)")
        };
        CustomerRow { tax_exempt: c.tax_exempt, assigned_rep: RepId(rep), primary_contact: c.primary_contact_id.map(ContactId) }
    }
    fn picked(c: &Customer) -> cairn_core::DocumentDefaults {
        cairn_core::on_customer_picked(None, &row(c), &adapt::company())
    }
    pub fn default_tax_rate(tax_exempt: bool, company_rate: f64) -> f64 {
        // The rep and contact don't affect the tax rate; RepId(0) only fills the row.
        let row = CustomerRow { tax_exempt, assigned_rep: RepId(0), primary_contact: None };
        cairn_core::default_tax_rate(&row, &cairn_core::CompanyInfo { default_tax_rate: company_rate })
    }
    pub fn default_rep(c: &Customer) -> Option<i32> {
        Some(picked(c).rep.0)
    }
    pub fn default_contact(c: &Customer) -> Option<i32> {
        picked(c).contact.map(|id| id.0)
    }
    pub fn apply_customer_change(c: Option<&Customer>) -> Option<Defaults> {
        c.map(|c| {
            let d = picked(c);
            Defaults { tax_rate: d.tax_rate, rep_id: Some(d.rep.0), contact_id: d.contact.map(|id| id.0) }
        })
    }
}
