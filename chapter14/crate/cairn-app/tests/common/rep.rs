//! A rep's login for the tests that write to the dev database: a
//! member of `cairn_app` with a live `users` row, issued the way
//! `cairn-auth` issues one, and removed afterwards with every row it
//! wrote.

use cairn_app::data;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use std::path::PathBuf;

pub const PORT: u16 = 5433;

/// A rep's login: a role that's a member of `cairn_app`, and its row in
/// `users`.
pub struct Rep {
    pub role: String,
    pub password: String,
    pub oid: String,
    pub rep_id: i32,
}

impl Rep {
    pub fn url(&self) -> String {
        format!(
            "postgres://{}:{}@127.0.0.1:{PORT}/cairn",
            self.role, self.password
        )
    }

    pub async fn pool(&self) -> PgPool {
        data::connect(&self.url()).await.unwrap()
    }
}

/// What `cairn_auth::session::issue` does, as `cairn_auth`: a login role
/// in `cairn_app`, a password that expires by itself, a `users` row
/// with the same expiry.
pub async fn sign_in(rep_id: i32) -> Rep {
    let auth = PgPool::connect(&format!(
        "postgres://cairn_auth:cairn_auth_dev@127.0.0.1:{PORT}/cairn"
    ))
    .await
    .unwrap();
    let (oid, password, until): (String, String, String) =
        sqlx::query_as(
            "SELECT gen_random_uuid()::text, md5(random()::text),
                    (now() + interval '1 hour')::text",
        )
        .fetch_one(&auth)
        .await
        .unwrap();
    let role = format!("rep_{}", oid.replace('-', ""));
    // CREATE ROLE takes no bind parameters. Every piece is ours: hex
    // digits and a timestamp from now().
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE ROLE {role} LOGIN IN ROLE cairn_app
             PASSWORD '{password}' VALID UNTIL '{until}'"
    )))
    .execute(&auth)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO users (entra_oid, upn, display_name, role_name,
                            rep_id, is_admin, signed_in_at, expires_at)
         VALUES ($1::uuid, 'test@cairn.example', 'Test Rep', $2, $3,
                 false, now(), $4::timestamptz)",
    )
    .bind(&oid)
    .bind(&role)
    .bind(rep_id)
    .bind(&until)
    .execute(&auth)
    .await
    .unwrap();
    Rep {
        role,
        password,
        oid,
        rep_id,
    }
}

/// The superuser, over the local socket: cleanup only.
pub async fn superuser() -> PgPool {
    let socket =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.pgdata");
    let opts = PgConnectOptions::new()
        .socket(socket)
        .port(PORT)
        .username("postgres")
        .database("cairn");
    PgPoolOptions::new().connect_with(opts).await.unwrap()
}

/// Remove the rep, every row its role wrote, and their audit entries.
/// Triggers are off for the cleanup (`replica`), so removing the rows
/// writes no audit entries of its own.
pub async fn forget(rep: &Rep) {
    let su = superuser().await;
    let mut su = su.acquire().await.unwrap();
    sqlx::query("SET session_replication_role = replica")
        .execute(&mut *su)
        .await
        .unwrap();
    let wrote: Vec<(String, i32)> = sqlx::query_as(
        "SELECT DISTINCT table_name, row_id FROM audit_log
          WHERE db_role = $1 AND action = 'INSERT'",
    )
    .bind(&rep.role)
    .fetch_all(&mut *su)
    .await
    .unwrap();
    // Children first, then what they hang from.
    for table in [
        "manufacturer_orders",
        "payments",
        "order_lines",
        "quote_lines",
        "contacts",
        "activities",
        "phone_messages",
        "orders",
        "quotes",
        "rmas",
        "products",
        "customers",
    ] {
        let ids: Vec<i32> = wrote
            .iter()
            .filter(|(t, _)| t == table)
            .map(|(_, id)| *id)
            .collect();
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM {table} WHERE id = ANY($1)"
        )))
        .bind(&ids)
        .execute(&mut *su)
        .await
        .unwrap();
    }
    sqlx::query("DELETE FROM audit_log WHERE db_role = $1")
        .bind(&rep.role)
        .execute(&mut *su)
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE role_name = $1")
        .bind(&rep.role)
        .execute(&mut *su)
        .await
        .unwrap();
    sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity
          WHERE usename = $1",
    )
    .bind(&rep.role)
    .execute(&mut *su)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP ROLE {}",
        rep.role
    )))
    .execute(&mut *su)
    .await
    .unwrap();
}

/// A rep that [`forget`]s itself when dropped, even when the test
/// panics: on a thread and a runtime of its own, since the test's may be
/// unwinding or blocked.
pub struct Cleanup(pub Rep);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let rep = Rep {
            role: std::mem::take(&mut self.0.role),
            password: String::new(),
            oid: String::new(),
            rep_id: self.0.rep_id,
        };
        std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(forget(&rep))
        })
        .join()
        .expect("cleanup");
    }
}
