//! The desktop side of signing in: steps 1 and 4 of the flow, and the
//! two ways the login it returns runs out.
//!
//! The app calls [`SignIn::start`], shows or opens [`SignIn::url`], then
//! awaits [`SignIn::finish`] for a [`Credential`]. Nothing here touches
//! the disk: the password lives in the returned value and nowhere else.

use crate::session::Credential;
use crate::token::{challenge, random_token};
use sqlx::postgres::{PgConnectOptions, PgDatabaseError, PgSeverity};
use sqlx::PgConnection;
use std::fmt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// A sign-in in progress: a loopback port waiting for the browser, and a
/// verifier only this process knows. The browser carries only its hash.
pub struct SignIn {
    auth: String,
    listener: TcpListener,
    verifier: String,
    url: String,
}

impl SignIn {
    /// Step 1. `auth` is `cairn-auth`'s address, such as
    /// `http://localhost:3000`.
    pub async fn start(auth: &str) -> Result<SignIn, SignInError> {
        let auth = auth.trim_end_matches('/').to_owned();
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let verifier = random_token();
        let url = format!(
            "{auth}/auth/start?port={port}&challenge={}",
            challenge(&verifier)
        );
        Ok(SignIn {
            auth,
            listener,
            verifier,
            url,
        })
    }

    /// The address the rep's browser has to visit.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Opens the system browser, or with `CAIRN_NO_BROWSER` set, prints
    /// the address to paste into one (a private window, to pick the
    /// account). Also prints it if no browser would open.
    pub fn open_browser(&self) {
        if std::env::var_os("CAIRN_NO_BROWSER").is_some() {
            println!("Open this address in a browser:\n{}", self.url);
            return;
        }
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(windows) {
            "explorer"
        } else {
            "xdg-open"
        };
        let spawned =
            std::process::Command::new(opener).arg(&self.url).spawn();
        if spawned.is_err() {
            println!("Open this address in a browser:\n{}", self.url);
        }
    }

    /// Steps 2 to 4: waits for the browser to come back with a one-time
    /// code, then trades the code and the verifier for a Postgres login.
    /// Dropping the future gives up and closes the port.
    pub async fn finish(self) -> Result<Credential, SignInError> {
        let code = wait_for_code(&self.listener).await?;
        let body = format!(
            r#"{{"code":"{code}","verifier":"{}"}}"#,
            self.verifier
        );
        let response = openidconnect::reqwest::Client::new()
            .post(format!("{}/session", self.auth))
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(serde_json::from_slice(&response.bytes().await?)?)
    }
}

/// The whole flow with the system browser: start, open, finish.
pub async fn sign_in(auth: &str) -> Result<Credential, SignInError> {
    let flow = SignIn::start(auth).await?;
    flow.open_browser();
    flow.finish().await
}

impl Credential {
    /// `base` (host, port, database) logged in as this rep.
    pub fn connect_options(
        &self,
        base: &PgConnectOptions,
    ) -> PgConnectOptions {
        base.clone().username(&self.user).password(&self.password)
    }

    /// Ends the session for every open connection at once
    /// (`cairn_sign_out()`), then drops the password with `self`.
    pub async fn sign_out(
        self,
        conn: &mut PgConnection,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT cairn_sign_out()").execute(conn).await?;
        Ok(())
    }
}

/// Which way a login ran out. The app handles both the same way: keep
/// the draft, sign in again, resend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expired {
    /// A new connection refused at login, past the role's `VALID UNTIL`
    /// or after a sign-in elsewhere reset the password (28P01).
    NewConnection,
    /// An open connection whose statement failed
    /// `cairn_session_check()`, past `users.expires_at` or after sign-out
    /// (28000).
    OpenConnection,
}

/// `Some` when `err` is Postgres refusing this rep's login (SQLSTATE
/// class 28), and which way; `None` for anything else.
pub fn session_expired(err: &sqlx::Error) -> Option<Expired> {
    let db = err.as_database_error()?;
    // A refused login ends the connection (FATAL); the session check
    // fails one statement on a connection that stays open (ERROR).
    let at_login = db
        .try_downcast_ref::<PgDatabaseError>()
        .is_some_and(|e| e.severity() == PgSeverity::Fatal);
    expiry(&db.code()?, at_login)
}

fn expiry(code: &str, at_login: bool) -> Option<Expired> {
    match code {
        "28000" if !at_login => Some(Expired::OpenConnection),
        // 28000 at login is a refusal no new password fixes: no
        // pg_hba.conf entry, or a role made NOLOGIN.
        "28000" => None,
        c if c.starts_with("28") => Some(Expired::NewConnection),
        _ => None,
    }
}

/// Why a sign-in didn't produce a credential.
#[derive(Debug)]
pub enum SignInError {
    /// The loopback port.
    Io(std::io::Error),
    /// Reaching `cairn-auth`, or its refusal of the code.
    Http(openidconnect::reqwest::Error),
    /// `cairn-auth` answered with something that isn't a credential.
    Body(serde_json::Error),
}

impl fmt::Display for SignInError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignInError::Io(e) => write!(f, "loopback listener: {e}"),
            SignInError::Http(e) => write!(f, "cairn-auth: {e}"),
            SignInError::Body(e) => {
                write!(f, "cairn-auth's answer: {e}")
            }
        }
    }
}

impl std::error::Error for SignInError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SignInError::Io(e) => Some(e),
            SignInError::Http(e) => Some(e),
            SignInError::Body(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for SignInError {
    fn from(e: std::io::Error) -> Self {
        SignInError::Io(e)
    }
}

impl From<openidconnect::reqwest::Error> for SignInError {
    fn from(e: openidconnect::reqwest::Error) -> Self {
        SignInError::Http(e)
    }
}

impl From<serde_json::Error> for SignInError {
    fn from(e: serde_json::Error) -> Self {
        SignInError::Body(e)
    }
}

/// Answers requests on the loopback port until one carries `code=`.
async fn wait_for_code(
    listener: &TcpListener,
) -> std::io::Result<String> {
    loop {
        let (mut stream, _) = listener.accept().await?;
        let mut line = String::new();
        BufReader::new(&mut stream).read_line(&mut line).await?;
        let code = code_in(&line);
        let page = match code {
            Some(_) => "Signed in to Cairn. You can close this tab.",
            None => "Waiting for sign-in.",
        };
        let reply = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\n\
             content-length: {}\r\nconnection: close\r\n\r\n{page}",
            page.len()
        );
        stream.write_all(reply.as_bytes()).await?;
        if let Some(code) = code {
            return Ok(code);
        }
    }
}

/// The `code=` value in a request line such as
/// `GET /callback?code=abc HTTP/1.1`.
fn code_in(request_line: &str) -> Option<String> {
    request_line
        .split_whitespace()
        .nth(1)
        .and_then(|path| path.split_once("code="))
        .map(|(_, rest)| {
            rest.split('&').next().unwrap_or("").to_owned()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::error::{DatabaseError, ErrorKind};
    use std::borrow::Cow;

    /// A database error with only a SQLSTATE, as a stand-in for the
    /// ones Postgres sends (the Postgres tests check the real ones).
    #[derive(Debug)]
    struct Coded(&'static str);

    impl fmt::Display for Coded {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "SQLSTATE {}", self.0)
        }
    }

    impl std::error::Error for Coded {}

    impl DatabaseError for Coded {
        fn message(&self) -> &str {
            self.0
        }
        fn code(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed(self.0))
        }
        fn as_error(
            &self,
        ) -> &(dyn std::error::Error + Send + Sync + 'static)
        {
            self
        }
        fn as_error_mut(
            &mut self,
        ) -> &mut (dyn std::error::Error + Send + Sync + 'static)
        {
            self
        }
        fn into_error(
            self: Box<Self>,
        ) -> Box<dyn std::error::Error + Send + Sync + 'static>
        {
            self
        }
        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
    }

    fn db(code: &'static str) -> sqlx::Error {
        sqlx::Error::Database(Box::new(Coded(code)))
    }

    #[test]
    fn class_28_is_an_expired_session_and_says_which_way() {
        assert_eq!(
            session_expired(&db("28P01")),
            Some(Expired::NewConnection)
        );
        assert_eq!(
            session_expired(&db("28000")),
            Some(Expired::OpenConnection)
        );
    }

    #[test]
    fn a_28000_refusing_the_login_is_not() {
        assert_eq!(expiry("28000", true), None);
        assert_eq!(
            expiry("28P01", true),
            Some(Expired::NewConnection)
        );
        assert_eq!(
            expiry("28000", false),
            Some(Expired::OpenConnection)
        );
    }

    #[test]
    fn anything_else_is_not() {
        for code in ["42501", "23503", "40001", "57P01", "08006"] {
            assert_eq!(session_expired(&db(code)), None, "{code}");
        }
        assert_eq!(session_expired(&sqlx::Error::PoolTimedOut), None);
        assert_eq!(session_expired(&sqlx::Error::RowNotFound), None);
    }

    #[test]
    fn the_code_comes_off_the_request_line() {
        let line = "GET /callback?code=abc-123&x=1 HTTP/1.1\r\n";
        assert_eq!(code_in(line).as_deref(), Some("abc-123"));
        assert_eq!(code_in("GET /favicon.ico HTTP/1.1\r\n"), None);
    }

    #[tokio::test]
    async fn the_url_carries_the_port_and_the_challenge() {
        let flow =
            SignIn::start("http://localhost:3000/").await.unwrap();
        let port = flow.listener.local_addr().unwrap().port();
        assert_eq!(
            flow.url(),
            format!(
                "http://localhost:3000/auth/start?port={port}\
                 &challenge={}",
                challenge(&flow.verifier)
            )
        );
    }
}
