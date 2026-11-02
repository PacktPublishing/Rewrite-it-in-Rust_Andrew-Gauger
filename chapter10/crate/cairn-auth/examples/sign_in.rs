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

use cairn_auth::session::Credential;
use cairn_auth::token::{challenge, random_token};
use sqlx::postgres::PgConnectOptions;
use sqlx::{AssertSqlSafe, Connection, PgConnection};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let auth = std::env::var("CAIRN_AUTH_URL")
        .unwrap_or_else(|_| "http://localhost:3000".into());
    let db = std::env::var("CAIRN_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://127.0.0.1:5433/cairn".into());

    // 1. Listen on a loopback port, and keep a verifier only this
    //    process knows. The browser carries only its hash.
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let verifier = random_token();
    let url = format!(
        "{auth}/auth/start?port={port}&challenge={}",
        challenge(&verifier)
    );
    println!("Opening the browser to sign in...");
    open_browser(&url);

    // 2. Entra and cairn-auth send the browser back with a one-time code.
    let code = wait_for_code(&listener)?;

    // 3. Trade the code and the verifier for a Postgres login.
    let body =
        format!(r#"{{"code":"{code}","verifier":"{verifier}"}}"#);
    let response = openidconnect::reqwest::Client::new()
        .post(format!("{auth}/session"))
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await?
        .error_for_status()?;
    let credential: Credential =
        serde_json::from_slice(&response.bytes().await?)?;
    println!(
        "Signed in as {} ({}), until {}",
        credential.display_name,
        credential.user,
        credential.expires_at
    );

    // 4. Connect as the rep. The password lives only in this process.
    let options = PgConnectOptions::from_str(&db)?
        .username(&credential.user)
        .password(&credential.password);
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

/// "allowed", or the refusal as Postgres sent it: SQLSTATE and message.
fn outcome<T>(r: Result<T, sqlx::Error>) -> String {
    match r {
        Ok(_) => "allowed".into(),
        Err(sqlx::Error::Database(db)) => format!(
            "{} {}",
            db.code().unwrap_or_default(),
            db.message()
        ),
        Err(e) => e.to_string(),
    }
}

/// Opens the system browser, or with `CAIRN_NO_BROWSER` set, prints the
/// address to paste into one (a private window, to pick the account).
fn open_browser(url: &str) {
    if std::env::var_os("CAIRN_NO_BROWSER").is_some() {
        println!("Open this address in a browser:\n{url}");
        return;
    }
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    if std::process::Command::new(opener).arg(url).spawn().is_err() {
        println!("Open this address in a browser:\n{url}");
    }
}

/// Answers requests on the loopback port until one carries `code=`.
fn wait_for_code(listener: &TcpListener) -> std::io::Result<String> {
    loop {
        let (mut stream, _) = listener.accept()?;
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line)?;
        let code = line
            .split_whitespace()
            .nth(1)
            .and_then(|path| path.split_once("code="))
            .map(|(_, rest)| {
                rest.split('&').next().unwrap_or("").to_owned()
            });
        let page = match code {
            Some(_) => "Signed in to Cairn. You can close this tab.",
            None => "Waiting for sign-in.",
        };
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\n\
             content-length: {}\r\nconnection: close\r\n\r\n{page}",
            page.len()
        )?;
        if let Some(code) = code {
            return Ok(code);
        }
    }
}
