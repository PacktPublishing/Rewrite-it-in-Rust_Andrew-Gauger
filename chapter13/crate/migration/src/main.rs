//! `cargo run -p migration -- up` (or `down`, `status`, `fresh`).
//! Reads `DATABASE_URL`.

use sea_orm_migration::prelude::*;

#[tokio::main]
async fn main() {
    cli::run_cli(migration::Migrator).await;
}
