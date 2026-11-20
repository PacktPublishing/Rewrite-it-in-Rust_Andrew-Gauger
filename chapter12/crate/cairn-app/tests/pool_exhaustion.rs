//! What the fifth request gets when the app's four connections are all
//! busy: it waits `acquire_timeout`, then fails as a value. Talks to the
//! dev database, so it's ignored by default:
//!
//!   cargo test -p cairn-app --test pool_exhaustion -- --ignored --nocapture

use cairn_app::data;
use sqlx::postgres::PgPoolOptions;
use std::time::{Duration, Instant};

#[test]
#[ignore = "reads the dev database"]
fn the_fifth_request_times_out() {
    let url = std::env::var("CAIRN_DATABASE_URL")
        .unwrap_or_else(|_| data::DEV_DATABASE_URL.into());
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let pool = PgPoolOptions::new()
            .max_connections(4)
            // sqlx waits 30 seconds by default. Two is enough here.
            .acquire_timeout(Duration::from_secs(2))
            .connect(&url)
            .await
            .unwrap();

        // Four slow queries, one on each connection.
        let mut busy = Vec::new();
        for _ in 0..4 {
            let mut conn = pool.acquire().await.unwrap();
            busy.push(tokio::spawn(async move {
                sqlx::query("SELECT pg_sleep(5)")
                    .execute(&mut *conn)
                    .await
            }));
        }
        println!(
            "pool: {} connections, {} idle",
            pool.size(),
            pool.num_idle()
        );

        let start = Instant::now();
        let err = pool.acquire().await.unwrap_err();
        println!(
            "fifth acquire, after {:.1} s: {err}",
            start.elapsed().as_secs_f64()
        );
        println!("{err:?}");
        assert!(matches!(err, sqlx::Error::PoolTimedOut));

        // The slow queries themselves finish normally.
        for task in busy {
            task.await.unwrap().unwrap();
        }
    });
}
