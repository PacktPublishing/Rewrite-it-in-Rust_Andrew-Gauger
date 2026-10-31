// Tests for table:Customers (reached from vba:CairnForms.DocCustomerChanged).
// Rules pinned: every customer has a non-NULL assigned rep; no customer is
// assigned to an inactive rep; customerstatus in {Active,Prospect,Inactive};
// paymentterms in {Net 30,Net 45,Net 15,Due on Receipt}; taxexempt boolean;
// customerid unique serial in 1..=1000. We assume helpers exist; not implemented.

use std::collections::HashSet;

fn all_customer_ids() -> Vec<i32> {
    unimplemented!("SELECT customerid FROM Customers")
}
fn assigned_rep_id(customerid: i32) -> Option<i32> {
    unimplemented!("SELECT assignedrepid FROM Customers WHERE customerid=$1")
}
fn rep_is_active(repid: i32) -> bool {
    unimplemented!("SELECT active FROM SalesReps WHERE repid=$1")
}
fn customer_status(customerid: i32) -> String {
    unimplemented!("SELECT customerstatus FROM Customers WHERE customerid=$1")
}
fn payment_terms(customerid: i32) -> String {
    unimplemented!("SELECT paymentterms FROM Customers WHERE customerid=$1")
}
fn is_tax_exempt(customerid: i32) -> bool {
    unimplemented!("SELECT taxexempt FROM Customers WHERE customerid=$1")
}

#[test]
fn every_customer_has_non_null_assigned_rep() {
    // Evidence: SELECT COUNT(*) FROM customers WHERE assignedrepid IS NULL -> 0.
    for id in all_customer_ids() {
        assert!(assigned_rep_id(id).is_some(),
            "customer {id} has NULL assignedrepid; every customer must have one");
    }
}

#[test]
fn no_customer_is_assigned_to_an_inactive_rep() {
    // Evidence: reps 5,6,8,10 have active=0 and 0 customers reference them.
    for id in all_customer_ids() {
        if let Some(repid) = assigned_rep_id(id) {
            assert!(rep_is_active(repid),
                "customer {id} assigned to inactive rep {repid}");
        }
    }
}

#[test]
fn customer_status_is_one_of_active_prospect_inactive() {
    let allowed: HashSet<&str> = ["Active", "Prospect", "Inactive"].into_iter().collect();
    for id in all_customer_ids() {
        let s = customer_status(id);
        assert!(allowed.contains(s.as_str()),
            "customer {id} has unexpected customerstatus {s:?}");
    }
}

#[test]
fn payment_terms_is_one_of_the_known_values() {
    let allowed: HashSet<&str> = ["Net 30", "Net 45", "Net 15", "Due on Receipt"]
        .into_iter().collect();
    for id in all_customer_ids() {
        let t = payment_terms(id);
        assert!(allowed.contains(t.as_str()),
            "customer {id} has unexpected paymentterms {t:?}");
    }
}

#[test]
fn taxexempt_is_a_boolean() {
    // Evidence: taxexempt BOOLEAN NOT NULL; observed 0/1 (e.g. 1,6,8,11 exempt).
    for id in all_customer_ids() {
        let _ = is_tax_exempt(id); // must be present, never NULL
    }
}

#[test]
fn customer_ids_are_unique_and_within_1_to_1000() {
    // Evidence: ids 1..1000 exist; 1001,1005,1010,1100,1200,2000 absent.
    let mut seen: HashSet<i32> = HashSet::new();
    for id in all_customer_ids() {
        assert!((1..=1000).contains(&id),
            "customerid {id} outside observed 1..=1000 range");
        assert!(seen.insert(id), "customerid {id} duplicated (pkey violation)");
    }
}

#[test]
fn sample_rows_match_observed_data() {
    // (id, status, terms, taxexempt, rep) from SQL.
    let rows: &[(i32, &str, &str, bool, i32)] = &[
        (1, "Active", "Net 45", true, 11),   // Northfield Water District
        (2, "Active", "Net 30", false, 1),   // Bluepine Cold Storage
        (4, "Prospect", "Net 30", false, 3), // Mesa Ridge Vineyards
        (7, "Inactive", "Net 30", false, 11),// Keystone Farms
        (19, "Inactive", "Due on Receipt", false, 7),
        (500, "Inactive", "Net 15", false, 11), // Ridgeline Industries
        (1000, "Active", "Net 30", false, 11),  // ALDER TRANSIT
    ];
    for &(id, status, terms, exempt, rep) in rows {
        assert_eq!(customer_status(id), status, "status of {id}");
        assert_eq!(payment_terms(id), terms, "terms of {id}");
        assert_eq!(is_tax_exempt(id), exempt, "taxexempt of {id}");
        assert_eq!(assigned_rep_id(id), Some(rep), "rep of {id}");
    }
}
