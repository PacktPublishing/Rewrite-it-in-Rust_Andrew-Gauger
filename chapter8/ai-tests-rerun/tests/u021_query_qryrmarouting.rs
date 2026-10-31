//! Tests for query:qryRMARouting (builder CairnBuild L133-148).
//! DaysInService = OpenedDate - Nz(ShipDate, OrderDate) (L136).
//! Route = nested IIf, first match wins, in order:
//!  1 'Damaged in Shipping' -> 'Carrier Claim' (L137)
//!  2 'Dead on Arrival' AND days<=30 -> 'Advance Replacement' (L138)
//!  3 Category='Service' -> 'Account Manager' (L139)
//!  4 days<=365 OR ExtendedWarranty -> 'Manufacturer Repair' (L140)
//!  5 'Dead on Arrival' (days>30) -> 'Advance Replacement' (L141)
//!  6 days<=90 -> 'Bench Test' (L142)
//!  7 TaxExempt -> 'Account Manager' (L143)
//!  8 days>730 -> 'Scrap - Out of Warranty' (L144)
//!  9 else -> 'Customer Pays Repair' (L144)
//! Assumed: fn days_in_service(opened, ship: Option, order) -> i64
//!          fn rma_route(reason, category, tax_exempt, ext_warranty, days) -> &str

use chrono::NaiveDate;

fn route(r: &str, c: &str, tax: bool, ext: bool, d: i64) -> &'static str {
    rma_route(r, c, tax, ext, d)
}

#[test]
fn days_in_service_uses_ship_date_when_shipped() {
    // RMA 1: Order 25 ShipDate 01/22/16, OpenedDate 01/30/17 => 374.
    let opened = NaiveDate::from_ymd_opt(2017, 1, 30).unwrap();
    let ship = Some(NaiveDate::from_ymd_opt(2016, 1, 22).unwrap());
    let order = NaiveDate::from_ymd_opt(2016, 1, 11).unwrap();
    assert_eq!(days_in_service(opened, ship, order), 374);
}

#[test]
fn days_in_service_falls_back_to_order_date_when_never_shipped() {
    // Nz(ShipDate, OrderDate): NULL ship => order date.
    let opened = NaiveDate::from_ymd_opt(2016, 2, 10).unwrap();
    let order = NaiveDate::from_ymd_opt(2016, 1, 11).unwrap();
    assert_eq!(days_in_service(opened, None, order), 30);
}

#[test]
fn damaged_in_shipping_always_carrier_claim() {
    // Branch 1 wins regardless of other inputs; beats Service (b3) and ext (b4).
    assert_eq!(route("Damaged in Shipping", "Gateway", false, false, 1), "Carrier Claim");
    assert_eq!(route("Damaged in Shipping", "Gateway", false, false, 731), "Carrier Claim");
    assert_eq!(route("Damaged in Shipping", "Service", false, true, 10), "Carrier Claim");
}

#[test]
fn doa_within_30_days_advance_replacement() {
    assert_eq!(route("Dead on Arrival", "Gateway", false, false, 0), "Advance Replacement");
    assert_eq!(route("Dead on Arrival", "Gateway", false, false, 30), "Advance Replacement"); // <=30 boundary
    // Branch 2 precedes branch 3 (Service).
    assert_eq!(route("Dead on Arrival", "Service", false, false, 30), "Advance Replacement");
}

#[test]
fn service_category_account_manager() {
    // Not DOA; any days/warranty/tax.
    assert_eq!(route("Intermittent Fault", "Service", false, false, 10), "Account Manager");
    assert_eq!(route("Intermittent Fault", "Service", true, true, 800), "Account Manager");
}

#[test]
fn within_one_year_manufacturer_repair() {
    assert_eq!(route("Intermittent Fault", "Gateway", false, false, 91), "Manufacturer Repair");
    assert_eq!(route("Intermittent Fault", "Gateway", false, false, 365), "Manufacturer Repair"); // <=365 boundary
}

#[test]
fn extended_warranty_manufacturer_repair_even_out_of_window() {
    // OR: ext warranty alone triggers, even at 800 days; precedes b6/b8.
    assert_eq!(route("Intermittent Fault", "Gateway", false, true, 800), "Manufacturer Repair");
    assert_eq!(route("Intermittent Fault", "Gateway", false, true, 731), "Manufacturer Repair");
}

#[test]
fn doa_after_30_days_still_advance_replacement() {
    // Missed b2 (days>30), caught by b5; precedes b6 (<=90) and b8 (>730).
    assert_eq!(route("Dead on Arrival", "Gateway", false, false, 31), "Advance Replacement");
    assert_eq!(route("Dead on Arrival", "Gateway", false, false, 90), "Advance Replacement");
    assert_eq!(route("Dead on Arrival", "Gateway", false, false, 731), "Advance Replacement");
}

#[test]
fn bench_test_branch_is_dead_for_ordinary_inputs() {
    // b6 (days<=90) is evaluated after b4 (days<=365 OR ext), so for
    // non-DOA/non-Service inputs b4 always matches first at <=90 days.
    assert_eq!(route("Intermittent Fault", "Gateway", false, false, 90), "Manufacturer Repair");
}

#[test]
fn tax_exempt_account_manager() {
    // b4 fails (days>365, no ext), not DOA/Service/<=90; b7 precedes b8.
    assert_eq!(route("Intermittent Fault", "Gateway", true, false, 366), "Account Manager");
    assert_eq!(route("Intermittent Fault", "Gateway", true, false, 731), "Account Manager");
}

#[test]
fn over_730_days_scrap_out_of_warranty() {
    // All earlier branches fail: ordinary reason/category, days>365, no ext, not tax.
    assert_eq!(route("Intermittent Fault", "Gateway", false, false, 731), "Scrap - Out of Warranty");
    assert_eq!(route("Intermittent Fault", "Gateway", false, false, 1000), "Scrap - Out of Warranty");
}

#[test]
fn exactly_730_days_is_not_scrap() {
    // Condition is strictly >730; 730 falls to the final branch.
    assert_eq!(route("Intermittent Fault", "Gateway", false, false, 730), "Customer Pays Repair");
}

#[test]
fn default_route_customer_pays_repair() {
    // 365 < days <= 730, no ext, not tax, ordinary reason/category.
    assert_eq!(route("Intermittent Fault", "Gateway", false, false, 366), "Customer Pays Repair");
    assert_eq!(route("Intermittent Fault", "Gateway", false, false, 730), "Customer Pays Repair");
}

#[test]
fn rma1_data_row_routes_to_customer_pays_repair() {
    // RMA 1: Reason 'Intermittent Fault', ExtWarranty 0, Product 12 'Gateway',
    // Customer 49 TaxExempt 0, Order 25 ShipDate 01/22/16, OpenedDate 01/30/17.
    let opened = NaiveDate::from_ymd_opt(2017, 1, 30).unwrap();
    let ship = Some(NaiveDate::from_ymd_opt(2016, 1, 22).unwrap());
    let order = NaiveDate::from_ymd_opt(2016, 1, 11).unwrap();
    let days = days_in_service(opened, ship, order);
    assert_eq!(days, 374);
    assert_eq!(route("Intermittent Fault", "Gateway", false, false, days), "Customer Pays Repair");
}

// ---- Added for the book (2026-10-28). Not part of the AI's output. ----
// Stubs with the signatures the tests above assume, so the file compiles.
// Each body is todo!() until the rule is implemented.
// ---- Wired to cairn-core for Chapter 8 (2026-10-30). Not part of the AI's output. ----
// The adapter converts types and calls cairn-core; see tests/common/adapt.rs.
#[path = "common/adapt.rs"]
mod adapt;
fn days_in_service(opened: NaiveDate, ship: Option<NaiveDate>, order: NaiveDate) -> i64 {
    let order_on_file = Some((adapt::date(order), ship.map(adapt::date)));
    i64::from(cairn_core::days_in_service(adapt::date(opened), order_on_file).expect("an order on file"))
}
fn rma_route(reason: &str, category: &str, tax_exempt: bool, ext_warranty: bool, days: i64) -> &'static str {
    let facts = cairn_core::ReturnFacts {
        reason: cairn_core::Reason::try_from(reason).unwrap(),
        category: cairn_core::Category::try_from(category).unwrap(),
        extended_warranty: ext_warranty,
        tax_exempt,
        days_in_service: Some(i32::try_from(days).unwrap()),
    };
    adapt::route_text(cairn_core::route(&facts))
}
