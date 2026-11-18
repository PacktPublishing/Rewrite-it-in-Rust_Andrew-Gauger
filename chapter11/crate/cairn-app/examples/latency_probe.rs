//! Time one query on an open connection against opening a new one,
//! through whatever address `--url` points at (dev-latency, usually).
//!
//! ```text
//! cargo run -p cairn-app --example latency_probe -- \
//!     postgres://cairn_owner:cairn_owner_dev@127.0.0.1:5434/cairn
//! ```

use std::time::{Duration, Instant};

use sqlx::{Connection, PgConnection};

const QUERY: &str =
    "SELECT company_name FROM customers ORDER BY id LIMIT 1";
const RUNS: usize = 7;

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn report(label: &str, mut times: Vec<Duration>) {
    times.sort();
    println!(
        "{label:<30} median {:>8.1} ms   max {:>8.1} ms   (n={})",
        ms(times[times.len() / 2]),
        ms(*times.last().unwrap()),
        times.len()
    );
}

async fn one_row(conn: &mut PgConnection) -> sqlx::Result<String> {
    sqlx::query_scalar(QUERY).fetch_one(conn).await
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> sqlx::Result<()> {
    let url =
        std::env::args().nth(1).expect("usage: latency_probe URL");
    println!("query: {QUERY}");

    // (a) An open connection; the first run prepares the statement,
    // which sqlx then caches, so the timed runs reuse it.
    let mut conn = PgConnection::connect(&url).await?;
    let first = Instant::now();
    let name = one_row(&mut conn).await?;
    println!("row: {name:?}");
    println!(
        "{:<30} {:>8.1} ms",
        "first run (prepare + execute)",
        ms(first.elapsed())
    );
    let mut cached = Vec::new();
    for _ in 0..RUNS {
        let t = Instant::now();
        one_row(&mut conn).await?;
        cached.push(t.elapsed());
    }
    conn.close().await?;
    report("open conn, cached statement", cached);

    // (b) A new connection each time: TCP, startup, SCRAM sign-in,
    // then the same query (prepared fresh on the new connection).
    let (mut connects, mut totals) = (Vec::new(), Vec::new());
    for _ in 0..RUNS {
        let t = Instant::now();
        let mut conn = PgConnection::connect(&url).await?;
        connects.push(t.elapsed());
        one_row(&mut conn).await?;
        totals.push(t.elapsed());
        conn.close().await?;
    }
    report("new conn: connect only", connects);
    report("new conn: connect + query", totals);
    Ok(())
}
