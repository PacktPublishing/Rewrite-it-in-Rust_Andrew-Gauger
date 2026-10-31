// Tests for table:SalesReps (reached from query:qryCommissionDetail).
//
// Evidence:
//  - CairnForms L46:  RS_REPS = "SELECT RepID, FirstName & ' ' & LastName AS RepName FROM SalesReps ORDER BY FirstName, LastName"
//  - CairnBuild L654: r.FirstName & ' ' & r.LastName AS RepName (commission detail)
//  - CairnBuild L657: CCur(Round(b.Subtotal * r.CommissionRate, 2)) AS Commission
//  - CairnBuild L231-232: CommissionRate DOUBLE DEFAULT 0.05, Active YESNO DEFAULT -1
//  - CairnReports L217: =Format([CommissionRate],"0.0%") & " of net sales"
//  - Data: rep 1 = Maya Torres (0.06, active); rep 5 = Raj Malhotra (0.04, inactive)

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn rep_name_is_first_name_space_last_name() {
    // Rep 1 in the data: Maya Torres
    assert_eq!(rep_name("Maya", "Torres"), "Maya Torres");
    // Rep 2: Derek Liu; Rep 3: Priya Shah
    assert_eq!(rep_name("Derek", "Liu"), "Derek Liu");
    assert_eq!(rep_name("Priya", "Shah"), "Priya Shah");
}

#[test]
fn commission_is_net_sales_times_rate_rounded_to_cents() {
    // 1234.56 * 0.06 = 74.0736 -> 74.07
    assert!(close(commission(1234.56, 0.06), 74.07));
    // 1000.00 * 0.05 = 50.00
    assert!(close(commission(1000.00, 0.05), 50.00));
    // 999.99 * 0.065 = 64.99935 -> 65.00
    assert!(close(commission(999.99, 0.065), 65.00));
    // 555.55 * 0.04 = 22.222 -> 22.22
    assert!(close(commission(555.55, 0.04), 22.22));
    // 100.00 * 0.03 = 3.00
    assert!(close(commission(100.00, 0.03), 3.00));
}

#[test]
fn new_rep_defaults_are_5_percent_rate_and_active() {
    // DDL: CommissionRate DOUBLE DEFAULT 0.05, Active YESNO DEFAULT -1 (true)
    assert!(close(default_commission_rate(), 0.05));
    assert!(default_active());
}

#[test]
fn commission_rate_displays_as_percent_of_net_sales() {
    // Format([CommissionRate],"0.0%") & " of net sales"
    assert_eq!(commission_rate_label(0.06), "6.0% of net sales");
    assert_eq!(commission_rate_label(0.065), "6.5% of net sales");
    assert_eq!(commission_rate_label(0.055), "5.5% of net sales");
    assert_eq!(commission_rate_label(0.03), "3.0% of net sales");
}

// ---- Added for the book (2026-10-28). Not part of the AI's output. ----
// Stubs with the signatures the tests above assume, so the file compiles.
// Each body is todo!() until the rule is implemented.
fn rep_name(_first: &str, _last: &str) -> String { todo!() }
fn commission(_net_sales: f64, _rate: f64) -> f64 { todo!() }
fn default_commission_rate() -> f64 { todo!() }
fn default_active() -> bool { todo!() }
fn commission_rate_label(_rate: f64) -> String { todo!() }
