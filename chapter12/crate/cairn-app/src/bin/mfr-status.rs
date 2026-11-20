//! What the manufacturer holds for one correlation ID, for whoever
//! answers "did this order go through?" With `DATABASE_URL` set, the
//! local attempts for it too.
//!
//!   cargo run -p cairn-app --bin mfr-status -- <correlation-id>
//!
//! `CAIRN_MANUFACTURER_URL` overrides the dev mock's
//! `http://localhost:8765/orders`.

use cairn_app::manufacturer::{self, Manufacturer};
use std::time::Duration;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(Ok(id)) =
        std::env::args().nth(1).map(|a| a.parse::<Uuid>())
    else {
        eprintln!("usage: mfr-status <correlation-id>");
        std::process::exit(2);
    };
    let url = std::env::var("CAIRN_MANUFACTURER_URL")
        .unwrap_or_else(|_| "http://localhost:8765/orders".into());
    let mfr = Manufacturer::new(&url, Duration::from_secs(10))?;
    let held = mfr.lookup(id).await?;
    println!("manufacturer ({url}), correlationId {id}:");
    if held.is_empty() {
        println!("  no order");
    }
    for o in &held {
        let units: i64 = o.lines.iter().map(|l| l.qty).sum();
        println!(
            "  {}  received {}  {}  {}  {} line(s), {units} unit(s)",
            o.mfr_order_id,
            o.received_at,
            o.dealer_po.as_deref().unwrap_or("-"),
            o.customer.as_deref().unwrap_or("-"),
            o.lines.len(),
        );
    }
    let Ok(db) = std::env::var("DATABASE_URL") else {
        return Ok(());
    };
    let pool = sqlx::PgPool::connect(&db).await?;
    println!("manufacturer_orders:");
    for a in manufacturer::attempts(&pool, id).await? {
        println!(
            "  attempt {}  {} UTC  order {}  http {}  ref {}{}",
            a.attempt,
            a.sent_at,
            a.order_id,
            a.http_status.map_or("-".into(), |s| s.to_string()),
            a.manufacturer_ref.as_deref().unwrap_or("-"),
            a.error_text
                .map(|e| format!("  {e}"))
                .unwrap_or_default(),
        );
    }
    Ok(())
}
