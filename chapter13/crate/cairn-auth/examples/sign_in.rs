//! The desktop side of signing in, the way Chapter 10's app does it when
//! it opens.
//!
//!   cargo run -p cairn-auth --example sign_in
//!   cargo run -p cairn-auth --example sign_in -- "UPDATE quotes ..."
//!
//! Any arguments are SQL to run as the signed-in rep. `CAIRN_AUTH_URL`
//! (default http://localhost:3000) and `CAIRN_DATABASE_URL` (default
//! postgres://127.0.0.1:5433/cairn, no user or password) say where to go.
//! `CAIRN_NO_BROWSER` prints the sign-in address instead of opening it.

use cairn_auth::desktop::{session_expired, SignIn};
use sqlx::postgres::PgConnectOptions;
use sqlx::{AssertSqlSafe, Connection, PgConnection};
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let auth = std::env::var("CAIRN_AUTH_URL")
        .unwrap_or_else(|_| "http://localhost:3000".into());
    let db = std::env::var("CAIRN_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://127.0.0.1:5433/cairn".into());

    // 1 to 3. A loopback port, a verifier, the browser, the code, and
    //    the trade for a Postgres login: all in cairn_auth::desktop.
    let flow = SignIn::start(&auth).await?;
    println!("Opening the browser to sign in...");
    flow.open_browser();
    let credential = flow.finish().await?;
    println!(
        "Signed in as {} ({}), until {}",
        credential.display_name,
        credential.user,
        credential.expires_at
    );

    // 4. Connect as the rep. The password lives only in this process.
    let options =
        credential.connect_options(&PgConnectOptions::from_str(&db)?);
    let mut conn = PgConnection::connect_with(&options).await?;
    let (user, name): (String, String) = sqlx::query_as(
        "SELECT current_user::text, display_name FROM users
         WHERE role_name = current_user",
    )
    .fetch_one(&mut conn)
    .await?;
    println!("Postgres says current_user = {user} ({name})");
    for sql in std::env::args().skip(1) {
        let done = sqlx::raw_sql(AssertSqlSafe(sql.clone()))
            .execute(&mut conn)
            .await?;
        println!("{sql}\n  {} row(s)", done.rows_affected());
    }

    // With CAIRN_WAIT_FOR_EXPIRY set, stay connected past the expiry,
    // then try the open connection and a new login.
    if std::env::var_os("CAIRN_WAIT_FOR_EXPIRY").is_some() {
        let left: f64 = sqlx::query_scalar(
            "SELECT extract(epoch FROM expires_at - now())::float8
             FROM users WHERE role_name = current_user",
        )
        .fetch_one(&mut conn)
        .await?;
        println!(
            "Waiting {left:.0} seconds for the login to expire..."
        );
        let wait = std::time::Duration::from_secs_f64(left + 2.0);
        tokio::time::sleep(wait).await;
        let open = sqlx::query("SELECT count(*) FROM quotes")
            .execute(&mut conn)
            .await;
        println!("The open connection: {}", outcome(open));
        let new = PgConnection::connect_with(&options).await;
        println!("A new login: {}", outcome(new));
    }
    Ok(())
}

/// "allowed", or the refusal as Postgres sent it: SQLSTATE, message,
/// and which way the login ran out.
fn outcome<T>(r: Result<T, sqlx::Error>) -> String {
    let e = match r {
        Ok(_) => return "allowed".into(),
        Err(e) => e,
    };
    match e.as_database_error() {
        Some(db) => format!(
            "{} {} ({:?})",
            db.code().unwrap_or_default(),
            db.message(),
            session_expired(&e)
        ),
        None => e.to_string(),
    }
}
