//! Getting a Postgres login for the rep at the keyboard: the one job at
//! the app's edge that differs by platform. The desktop opens the system
//! browser and listens on a loopback port (`cairn_auth::desktop`); a
//! tablet would need an app link instead; a test issues one itself.
//! The workers ask for a login when the window opens and again whenever
//! Postgres says the last one has run out.

use sqlx::postgres::PgConnectOptions;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

/// A login, as the options the pool connects with, or why there's none.
pub type Login<'a> = Pin<
    Box<
        dyn Future<Output = Result<PgConnectOptions, Refused>>
            + Send
            + 'a,
    >,
>;

pub trait SignIn: Send + Sync + 'static {
    /// Sign the rep in, waiting as long as that takes. A browser
    /// sign-in can take a minute; nothing on the UI thread waits for it.
    fn sign_in(&self) -> Login<'_>;
}

/// Why a sign-in produced no login.
#[derive(Debug, Clone)]
pub struct Refused(pub String);

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Entra ID through `cairn-auth`: the browser, the loopback port, the
/// code traded for a Postgres login that expires on its own.
pub struct Entra {
    /// `cairn-auth`'s address, such as `http://localhost:3000`.
    pub auth: String,
    /// Host, port and database. The login comes from the sign-in.
    pub database: PgConnectOptions,
}

impl SignIn for Entra {
    fn sign_in(&self) -> Login<'_> {
        Box::pin(async move {
            let credential = cairn_auth::desktop::sign_in(&self.auth)
                .await
                .map_err(|e| Refused(e.to_string()))?;
            Ok(credential.connect_options(&self.database))
        })
    }
}

/// A fixed login from a URL, for a database with no `cairn-auth` in
/// front of it: the dev database as `cairn_owner`, by default. It never
/// expires, so it's never asked twice.
pub struct Fixed(pub PgConnectOptions);

impl SignIn for Fixed {
    fn sign_in(&self) -> Login<'_> {
        let login = self.0.clone();
        Box::pin(async move { Ok(login) })
    }
}
