//! Column and constraint shapes every table repeats.

use sea_orm_migration::prelude::*;

/// V1's status lists, from the forms' value lists (`CairnForms.bas`
/// L49–L58). Each becomes a check constraint, so a status nobody chose
/// from the list can't be saved.
pub const QUOTE_STATUS: [&str; 5] =
    ["Draft", "Sent", "Accepted", "Declined", "Expired"];
pub const ORDER_STATUS: [&str; 6] = [
    "Open",
    "In Production",
    "Shipped",
    "Invoiced",
    "Closed",
    "Cancelled",
];
pub const CUSTOMER_STATUS: [&str; 3] =
    ["Prospect", "Active", "Inactive"];
pub const MESSAGE_STATUS: [&str; 3] = ["New", "Returned", "Closed"];
pub const RMA_STATUS: [&str; 7] = [
    "Open",
    "Received",
    "In Repair",
    "Replaced",
    "Credited",
    "Closed",
    "Rejected",
];
pub const RMA_REASON: [&str; 6] = [
    "Dead on Arrival",
    "Failed in Warranty",
    "Damaged in Shipping",
    "Wrong Item Shipped",
    "Intermittent Fault",
    "Customer Damage",
];

/// An integer primary key the database assigns.
pub fn id(col: impl IntoIden) -> ColumnDef {
    ColumnDef::new(col)
        .integer()
        .not_null()
        .auto_increment()
        .primary_key()
        .to_owned()
}

/// The row's version, for optimistic concurrency (Chapter 12).
pub fn version() -> ColumnDef {
    ColumnDef::new(Alias::new("version"))
        .integer()
        .not_null()
        .default(1)
        .to_owned()
}

/// Money: exact decimal, four places, like Access's Currency type.
pub fn money(col: impl IntoIden) -> ColumnDef {
    ColumnDef::new(col)
        .decimal_len(19, 4)
        .not_null()
        .default(0)
        .to_owned()
}

/// A rate stored as a fraction (0.0725 is 7.25%), exact, between 0 and 1.
/// V1 kept these as floating-point `DOUBLE`.
pub fn fraction(col: impl IntoIden) -> ColumnDef {
    let col = col.into_iden();
    ColumnDef::new(col.clone())
        .decimal_len(9, 6)
        .not_null()
        .default(0)
        .check(Expr::col(col).between(0, 1))
        .to_owned()
}

/// A document number the database assigns at insert, from a per-year
/// counter: `Q-2026-0001`. See `next_document_number`.
pub fn document_number(
    col: impl IntoIden,
    prefix: &str,
) -> ColumnDef {
    ColumnDef::new(col)
        .string_len(20)
        .not_null()
        .unique_key()
        .default(Expr::cust(format!(
            "next_document_number('{prefix}')"
        )))
        .to_owned()
}

/// A named check that `col` holds one of `values`.
pub fn one_of(
    name: &str,
    col: impl IntoIden,
    values: &[&str],
) -> (String, Expr) {
    let values = values.iter().map(|v| Expr::val(*v));
    (name.to_owned(), Expr::col(col).is_in(values))
}

/// A named foreign key. Every key restricts deletes unless a migration
/// says otherwise.
pub fn fk(
    name: &str,
    from: (impl IntoIden, impl IntoIden),
    to: impl IntoIden,
) -> ForeignKeyCreateStatement {
    ForeignKey::create()
        .name(name)
        .from(from.0, from.1)
        .to(to, Alias::new("id"))
        .on_delete(ForeignKeyAction::Restrict)
        .to_owned()
}
