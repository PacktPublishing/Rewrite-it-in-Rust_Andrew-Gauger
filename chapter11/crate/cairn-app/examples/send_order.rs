//! One custom order sent the way the app sends it, recorded in
//! `manufacturer_orders`, and sent once more if it comes back
//! unconfirmed (the rep pressing Retry). For captures against the
//! mock:
//!
//!   cargo run -p cairn-app --example send_order -- SO-2026-1100
//!
//! `DATABASE_URL` and `CAIRN_MANUFACTURER_URL` override the dev ones.

use cairn_app::data;
use cairn_app::manufacturer::{self, Manufacturer, Sent};
use cairn_core::manufacturer::{
    ManufacturerOrder, ShipTo, SourceLine,
};
use cairn_core::Category;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let po = std::env::args().nth(1).unwrap_or("SO-2026-1100".into());
    let db = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| data::DEV_DATABASE_URL.into());
    let url = std::env::var("CAIRN_MANUFACTURER_URL")
        .unwrap_or_else(|_| "http://localhost:8765/orders".into());
    let pool = sqlx::PgPool::connect(&db).await?;
    let order_id: i32 = sqlx::query_scalar(
        "SELECT id FROM orders WHERE order_number = $1",
    )
    .bind(&po)
    .fetch_one(&pool)
    .await?;

    let ship_to = ShipTo {
        name: "Vista Bakery".into(),
        address: "6503 Enterprise Rd".into(),
        city: "Lubbock".into(),
        state: "TX".into(),
        zip: "76161".into(),
    };
    let lines = [
        SourceLine {
            sku: "CS-TH100".into(),
            category: Category::Sensor,
            quantity: 385,
        },
        SourceLine {
            sku: "CS-GW400".into(),
            category: Category::Gateway,
            quantity: 8,
        },
    ];
    let order = ManufacturerOrder::new(&po, None, ship_to, &lines)?;
    println!("{po}, correlationId {}", order.correlation_id());

    let mfr = Manufacturer::new(&url, Duration::from_secs(10))?;
    let mut sent =
        manufacturer::send(&pool, &mfr, order_id, None, &order)
            .await?;
    println!("attempt 1: {sent:?}");
    if let Sent::Unconfirmed { .. } = sent {
        println!("Retry (same correlationId)");
        sent = manufacturer::send(
            &pool,
            &mfr,
            order_id,
            None,
            &order.retry(),
        )
        .await?;
        println!("attempt 2: {sent:?}");
    }
    Ok(())
}
