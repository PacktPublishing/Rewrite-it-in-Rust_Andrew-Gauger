//! Tests for table:OrderLines — rules pinned from the DDL:
//!   "orderlineid" SERIAL, PRIMARY KEY ("orderlineid")
//!   "orderid" INTEGER NOT NULL, FK -> Orders(orderid) ON DELETE CASCADE
//!   "productid" INTEGER NOT NULL, FK -> Products(productid) (no cascade)
//!   "linedescription" VARCHAR(255) / "quantity" INTEGER /
//!   "unitprice" NUMERIC(15,2) / "discountpct" DOUBLE PRECISION /
//!   "serialnumbers" TEXT  — all nullable, no CHECKs.
//!
//! Assumed API (not implemented here):
//!   insert_order_line(orderid, productid, linedescription, quantity, unitprice, discountpct, serialnumbers) -> Result<i64>
//!   delete_order(orderid) / delete_product(productid) -> Result<()>
//!   count_order_lines(orderid) -> i64
//!   get_order_line(orderlineid) -> Option<OrderLine>
//!
//! Concrete values mirror the seed data (CairnBuild AddLine, lines 451-462).

use crate::db::{
    count_order_lines, delete_order, delete_product, get_order_line, insert_order_line,
};

fn full_line(orderid: i64, productid: i64) -> Result<i64, String> {
    insert_order_line(
        orderid,
        productid,
        Some("Custom description"),
        Some(40),
        Some(199.00),
        Some(0.10),
        Some("SN-0001,SN-0002"),
    )
}

// 1. orderlineid is a SERIAL primary key: each insert gets a fresh unique id.
#[test]
fn orderlineid_is_auto_assigned_and_unique_per_line() {
    let a = full_line(1, 4).expect("insert line for order 1 / product 4");
    let b = full_line(1, 5).expect("insert second line for order 1 / product 5");
    assert_ne!(a, b, "two lines must get distinct auto-assigned ids");
    assert!(a > 0 && b > 0, "serial ids are positive");
}

// 2. Primary key: no insert may reuse an existing orderlineid.
#[test]
fn duplicate_orderlineid_is_rejected() {
    let a = full_line(1, 4).expect("first insert");
    let b = full_line(1, 5).expect("second insert");
    let c = full_line(1, 8).expect("third insert");
    assert!((c != a) && (c != b), "no insert may reuse an existing orderlineid");
}

// 3. orderid is NOT NULL and an FK to Orders: a line without an existing order is rejected.
#[test]
fn line_without_existing_order_is_rejected() {
    let r = insert_order_line(999_999, 4, None, Some(1), Some(10.00), None, None);
    assert!(r.is_err(), "orderid NOT NULL + FK to Orders: unknown order must fail");
}

// 4. productid is NOT NULL and an FK to Products: a line without an existing product is rejected.
#[test]
fn line_without_existing_product_is_rejected() {
    let r = insert_order_line(1, 999_999, None, Some(1), Some(10.00), None, None);
    assert!(r.is_err(), "productid NOT NULL + FK to Products: unknown product must fail");
}

// 5. ON DELETE CASCADE on orderid: deleting an order deletes its lines.
#[test]
fn deleting_order_cascades_to_its_lines() {
    let a = full_line(1, 4).expect("line A on order 1");
    let b = full_line(1, 5).expect("line B on order 1");
    assert_eq!(count_order_lines(1), 2);
    delete_order(1).expect("delete order 1");
    assert_eq!(count_order_lines(1), 0, "cascade must remove both lines");
    assert!(get_order_line(a).is_none(), "line A gone after order delete");
    assert!(get_order_line(b).is_none(), "line B gone after order delete");
}

// 6. productid FK has NO cascade: deleting a product that has lines is blocked.
#[test]
fn deleting_product_with_lines_is_blocked() {
    let a = full_line(1, 4).expect("line on product 4");
    let r = delete_product(4);
    assert!(r.is_err(), "FK productid has no ON DELETE CASCADE: delete must be blocked");
    assert!(get_order_line(a).is_some(), "line survives because product delete was blocked");
}

// 7. linedescription is nullable: a line with no description is accepted.
#[test]
fn line_without_description_is_accepted() {
    let a = insert_order_line(1, 8, None, Some(42), Some(36.00), Some(0.0), None)
        .expect("linedescription is nullable");
    assert!(get_order_line(a).unwrap().linedescription.is_none());
}

// 8. quantity is nullable with no CHECK: the schema does not enforce positivity.
#[test]
fn quantity_is_not_enforced_by_schema() {
    let a = insert_order_line(1, 9, None, None, Some(1200.00), None, None)
        .expect("quantity is nullable");
    assert!(get_order_line(a).unwrap().quantity.is_none());
    // DDL has no CHECK on quantity, so 0 is not a schema violation.
    let b = insert_order_line(1, 9, None, Some(0), Some(1200.00), None, None);
    assert!(b.is_ok(), "no CHECK on quantity: 0 must not be a schema violation");
}

// 9. unitprice is nullable NUMERIC(15,2): a line without a price is accepted.
#[test]
fn line_without_unitprice_is_accepted() {
    let a = insert_order_line(1, 1, None, Some(120), None, Some(0.05), None)
        .expect("unitprice is nullable");
    assert!(get_order_line(a).unwrap().unitprice.is_none());
}

// 10. discountpct is nullable DOUBLE PRECISION with no CHECK.
#[test]
fn line_without_discount_is_accepted() {
    let a = insert_order_line(1, 5, None, Some(2), Some(649.00), None, None)
        .expect("discountpct is nullable");
    assert!(get_order_line(a).unwrap().discountpct.is_none());
}

// 11. serialnumbers is nullable TEXT.
#[test]
fn line_without_serialnumbers_is_accepted() {
    let a = insert_order_line(1, 7, None, Some(120), Some(19.00), Some(0.0), None)
        .expect("serialnumbers is nullable");
    assert!(get_order_line(a).unwrap().serialnumbers.is_none());
}

// ---- Added for the book (2026-10-28). Not part of the AI's output. ----
// Stubs with the signatures the tests above assume, so the file compiles.
// Each body is todo!() until the rule is implemented.
mod db {
    pub struct OrderLine {
        pub linedescription: Option<String>, pub quantity: Option<i32>, pub unitprice: Option<f64>,
        pub discountpct: Option<f64>, pub serialnumbers: Option<String>,
    }
    pub fn insert_order_line(_orderid: i64, _productid: i64, _linedescription: Option<&str>, _quantity: Option<i32>,
                             _unitprice: Option<f64>, _discountpct: Option<f64>, _serialnumbers: Option<&str>) -> Result<i64, String> { todo!() }
    pub fn count_order_lines(_orderid: i64) -> usize { todo!() }
    pub fn delete_order(_orderid: i64) -> Result<(), String> { todo!() }
    pub fn delete_product(_productid: i64) -> Result<(), String> { todo!() }
    pub fn get_order_line(_orderlineid: i64) -> Option<OrderLine> { todo!() }
}
