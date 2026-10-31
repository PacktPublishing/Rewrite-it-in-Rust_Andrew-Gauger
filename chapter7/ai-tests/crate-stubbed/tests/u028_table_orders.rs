// Tests for table:Orders — rules pinned from the Access DDL (CairnBuild L279-289),
// the PostgreSQL schema (NOT NULL / UNIQUE / FK constraints) and observed data.
//
// Assumed (unimplemented) validator functions, one per rule:
//   fn order_number_not_null(n: Option<&str>) -> bool
//   fn order_number_unique(n: &str, existing: &[&str]) -> bool
//   fn customer_required(id: Option<i32>) -> bool
//   fn rep_required(id: Option<i32>) -> bool
//   fn contact_optional(id: Option<i32>, existing: &[i32]) -> bool
//   fn quote_optional(id: Option<i32>, existing: &[i32]) -> bool
//   fn default_status(s: Option<&str>) -> String
//   fn default_tax_rate(r: Option<f64>) -> f64
//   fn valid_order_status(s: &str) -> bool

#[test]
fn order_number_not_null_rejects_null() {
    // Schema: "ordernumber" VARCHAR(20) NOT NULL; Access: TEXT(20) NOT NULL (L281)
    assert!(!order_number_not_null(None));
    assert!(order_number_not_null(Some("SO-2026-0920")));
}

#[test]
fn order_number_unique_rejects_duplicate() {
    // Unique index orders_uq_orders_number_idx; Access UQ_Orders_Number (L281)
    let existing = [
        "SO-2016-0001", "SO-2016-0002", "SO-2026-0920",
        "SO-2026-0991", "SO-2026-1013", "SO-2026-1067",
    ];
    assert!(!order_number_unique("SO-2026-0920", &existing));
    assert!(order_number_unique("SO-2026-1068", &existing));
}

#[test]
fn customer_required_rejects_null() {
    // Schema: "customerid" INTEGER NOT NULL; FK to Customers (L282, L286)
    assert!(!customer_required(None));
    assert!(customer_required(Some(1)));
}

#[test]
fn rep_required_rejects_null() {
    // Schema: "repid" INTEGER NOT NULL; FK to SalesReps (L282, L288)
    assert!(!rep_required(None));
    assert!(rep_required(Some(1)));
}

#[test]
fn contact_optional_allows_null_and_enforces_fk() {
    // Schema: "contactid" INTEGER (nullable); FK to Contacts (L282, L287)
    let existing = [1, 2, 5, 6, 219]; // contactids observed in orders
    assert!(contact_optional(None, &existing));
    assert!(contact_optional(Some(219), &existing));
    assert!(!contact_optional(Some(999_999), &existing));
}

#[test]
fn quote_optional_allows_null_and_enforces_fk() {
    // Schema: "quoteid" INTEGER (nullable); FK to Quotes (L282, L289)
    // Orders may be standalone (no quote) or created from a quote.
    let existing = [1, 2, 6]; // quoteids observed in orders
    assert!(quote_optional(None, &existing));
    assert!(quote_optional(Some(6), &existing));
    assert!(!quote_optional(Some(999_999), &existing));
}

#[test]
fn default_status_is_open() {
    // Access DDL L284: OrderStatus TEXT(20) DEFAULT 'Open'
    assert_eq!(default_status(None), "Open");
    assert_eq!(default_status(Some("Shipped")), "Shipped");
}

#[test]
fn default_tax_rate_is_zero() {
    // Access DDL L284: TaxRate DOUBLE DEFAULT 0
    assert_eq!(default_tax_rate(None), 0.0);
    assert_eq!(default_tax_rate(Some(0.0825)), 0.0825);
}

#[test]
fn status_list_is_the_six_observed_statuses() {
    // CairnForms L50: VL_ORDERSTATUS = "Open;In Production;Shipped;Invoiced;Closed;Cancelled"
    // (data-level observation; the schema itself has no CHECK constraint)
    for s in ["Open", "In Production", "Shipped", "Invoiced", "Closed", "Cancelled"] {
        assert!(valid_order_status(s), "expected {s} to be a valid order status");
    }
    assert!(!valid_order_status("Pending"));
}

// ---- Added for the book (2026-10-28). Not part of the AI's output. ----
// Stubs with the signatures the tests above assume, so the file compiles.
// Each body is todo!() until the rule is implemented.
fn order_number_not_null(_n: Option<&str>) -> bool { todo!() }
fn order_number_unique(_n: &str, _existing: &[&str]) -> bool { todo!() }
fn customer_required(_id: Option<i32>) -> bool { todo!() }
fn rep_required(_id: Option<i32>) -> bool { todo!() }
fn contact_optional(_id: Option<i32>, _existing: &[i32]) -> bool { todo!() }
fn quote_optional(_id: Option<i32>, _existing: &[i32]) -> bool { todo!() }
fn default_status(_s: Option<&str>) -> String { todo!() }
fn default_tax_rate(_r: Option<f64>) -> f64 { todo!() }
fn valid_order_status(_s: &str) -> bool { todo!() }
