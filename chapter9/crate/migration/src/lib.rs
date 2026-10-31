//! Cairn V2's schema, as reversible Rust migrations. Each `up` has a
//! `down` that undoes it exactly.

pub use sea_orm_migration::prelude::*;

mod columns;
mod tables;

mod m20261031_000001_people;
mod m20261031_000002_document_numbers;
mod m20261031_000003_quotes_orders;
mod m20261031_000004_crm;
mod m20261031_000005_returns;
mod m20261031_000006_users;
mod m20261031_000007_audit;
mod m20261031_000008_access;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20261031_000001_people::Migration),
            Box::new(m20261031_000002_document_numbers::Migration),
            Box::new(m20261031_000003_quotes_orders::Migration),
            Box::new(m20261031_000004_crm::Migration),
            Box::new(m20261031_000005_returns::Migration),
            Box::new(m20261031_000006_users::Migration),
            Box::new(m20261031_000007_audit::Migration),
            Box::new(m20261031_000008_access::Migration),
        ]
    }
}
