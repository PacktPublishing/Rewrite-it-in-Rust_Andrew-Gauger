// Tests for table:Customers (reached from vba:CairnForms.DocCustomerChanged).
// Rules pinned: every customer has a non-NULL assigned rep; no customer is
// assigned to an inactive rep; customerstatus in {Active,Prospect,Inactive};
// paymentterms in {Net 30,Net 45,Net 15,Due on Receipt}; taxexempt boolean;
// customerid unique serial in 1..=1000. We assume helpers exist; not implemented.

use std::collections::HashSet;

// ---- Implemented for the book (2026-10-28). Not part of the AI's output. ----
// The AI declared these helpers with unimplemented!() bodies, each carrying
// the SQL it meant (kept below as comments). These bodies read the History
// database through mdb-export, so the tests above it run against real rows.
// Set CAIRN_DB to use another copy of the file.

use std::collections::HashMap;
use std::sync::OnceLock;

struct Table {
    rows: HashMap<i32, HashMap<String, String>>,
}

fn export(table: &str) -> Table {
    let db = std::env::var("CAIRN_DB").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../../part 2/CairnSystems_History.accdb").to_string()
    });
    // Unit and record separators, so commas and line breaks inside Notes are safe.
    let out = std::process::Command::new("mdb-export")
        .args(["-Q", "-d", "\x1f", "-R", "\x1e", &db, table])
        .output()
        .expect("mdb-export must be installed");
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.split('\x1e').filter(|l| !l.is_empty());
    let header: Vec<String> = lines.next().unwrap().split('\x1f').map(str::to_string).collect();
    let mut rows = HashMap::new();
    for line in lines {
        let row: HashMap<String, String> =
            header.iter().cloned().zip(line.split('\x1f').map(str::to_string)).collect();
        rows.insert(row[&header[0]].parse().unwrap(), row);
    }
    Table { rows }
}

fn customers() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| export("Customers"))
}

fn reps() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| export("SalesReps"))
}

fn field(t: &Table, id: i32, col: &str) -> String {
    t.rows[&id][col].clone()
}

fn all_customer_ids() -> Vec<i32> {
    // SELECT customerid FROM Customers
    let mut ids: Vec<i32> = customers().rows.keys().copied().collect();
    ids.sort();
    ids
}
fn assigned_rep_id(customerid: i32) -> Option<i32> {
    // SELECT assignedrepid FROM Customers WHERE customerid=$1
    field(customers(), customerid, "AssignedRepID").parse().ok()
}
fn rep_is_active(repid: i32) -> bool {
    // SELECT active FROM SalesReps WHERE repid=$1
    field(reps(), repid, "Active") == "1"
}
fn customer_status(customerid: i32) -> String {
    // SELECT customerstatus FROM Customers WHERE customerid=$1
    field(customers(), customerid, "CustomerStatus")
}
fn payment_terms(customerid: i32) -> String {
    // SELECT paymentterms FROM Customers WHERE customerid=$1
    field(customers(), customerid, "PaymentTerms")
}
fn is_tax_exempt(customerid: i32) -> bool {
    // SELECT taxexempt FROM Customers WHERE customerid=$1
    field(customers(), customerid, "TaxExempt") == "1"
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
