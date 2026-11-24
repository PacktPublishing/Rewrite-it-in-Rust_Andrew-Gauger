//! Runs against the local database from `dev-db.sh`, migrated:
//!
//!   sh dev-db.sh && DATABASE_URL=... cargo run -p migration -- up
//!   cargo test -p cairn-auth
//!
//! Each test makes its own identity (a random Entra object ID) and its
//! own rows, held by a `Fixture`. When the fixture drops, even when the
//! test panics, it deletes those rows, the users row, the role, and the
//! audit entries they made, and gives back the document numbers it took,
//! so the tests can share the development database, run in parallel,
//! and leave it as they found it.

use cairn_auth::desktop::{session_expired, Expired};
use cairn_auth::session::{self, Credential, Identity};
use sqlx::postgres::{
    PgConnectOptions, PgDatabaseError, PgPool, PgPoolOptions,
};
use sqlx::{Connection, PgConnection};
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

const PORT: u16 = 5433;

/// The superuser, over the local socket: test setup and cleanup only.
async fn superuser() -> PgPool {
    let socket =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.pgdata");
    let opts = PgConnectOptions::new()
        .socket(socket)
        .port(PORT)
        .username("postgres")
        .database("cairn");
    PgPoolOptions::new().connect_with(opts).await.unwrap()
}

/// `cairn-auth`'s own login, as the service uses it.
async fn cairn_auth() -> PgPool {
    PgPool::connect(&format!(
        "postgres://cairn_auth:cairn_auth_dev@127.0.0.1:{PORT}/cairn"
    ))
    .await
    .unwrap()
}

async fn login(c: &Credential) -> Result<PgConnection, sqlx::Error> {
    let opts = PgConnectOptions::new()
        .host("127.0.0.1")
        .port(PORT)
        .database("cairn")
        .username(&c.user)
        .password(&c.password);
    PgConnection::connect_with(&opts).await
}

fn someone() -> Identity {
    Identity {
        oid: Uuid::new_v4(),
        upn: "test@cairn.example".into(),
        display_name: "Test Rep".into(),
        rep_id: None,
        is_admin: false,
    }
}

/// One test's identity and the rows it made. Dropping it cleans up.
struct Fixture {
    who: Identity,
    customers: Vec<i32>,
    products: Vec<i32>,
    reps: Vec<i32>,
}

impl Fixture {
    fn new() -> Self {
        Fixture {
            who: someone(),
            customers: vec![],
            products: vec![],
            reps: vec![],
        }
    }

    /// A customer and a product of the test's own.
    async fn customer_and_product(
        &mut self,
        su: &PgPool,
    ) -> (i32, i32) {
        let customer: i32 = sqlx::query_scalar(
            "INSERT INTO customers (company_name) VALUES ('Test Co')
             RETURNING id",
        )
        .fetch_one(su)
        .await
        .unwrap();
        self.customers.push(customer);
        let product: i32 = sqlx::query_scalar(
            "INSERT INTO products (sku, product_name)
             VALUES ('T-' || left(md5(random()::text), 12),
                     'Test sensor')
             RETURNING id",
        )
        .fetch_one(su)
        .await
        .unwrap();
        self.products.push(product);
        (customer, product)
    }

    /// A sales rep of the test's own.
    async fn rep(&mut self, su: &PgPool) -> i32 {
        let rep: i32 = sqlx::query_scalar(
            "INSERT INTO sales_reps (first_name, last_name)
             VALUES ('T', 'R') RETURNING id",
        )
        .fetch_one(su)
        .await
        .unwrap();
        self.reps.push(rep);
        rep
    }
}

impl Drop for Fixture {
    /// The test's runtime may be unwinding, so the cleanup gets a
    /// thread and a runtime of its own.
    fn drop(&mut self) {
        let oid = self.who.oid;
        let customers = std::mem::take(&mut self.customers);
        let products = std::mem::take(&mut self.products);
        let reps = std::mem::take(&mut self.reps);
        let done = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(clean_up(oid, customers, products, reps))
        })
        .join();
        if done.is_err() && !std::thread::panicking() {
            panic!("test cleanup failed for {oid}");
        }
    }
}

/// The users row and the login role.
async fn forget(su: &PgPool, oid: &Uuid) {
    let role = session::role_name(oid);
    sqlx::query("DELETE FROM users WHERE entra_oid = $1")
        .bind(oid)
        .execute(su)
        .await
        .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP ROLE IF EXISTS {role}"
    )))
    .execute(su)
    .await
    .unwrap();
}

/// Everything one fixture made. `audit_log` grants no one DELETE, so
/// this runs as the superuser, and only here.
async fn clean_up(
    oid: Uuid,
    customers: Vec<i32>,
    products: Vec<i32>,
    reps: Vec<i32>,
) {
    let su = superuser().await;
    let role = session::role_name(&oid);
    let mut tx = su.begin().await.unwrap();
    // The highest number this test took, per counter.
    let taken: Vec<(String, i32, i32)> = sqlx::query_as(
        "SELECT split_part(n, '-', 1), split_part(n, '-', 2)::int,
                max(split_part(n, '-', 3)::int)
         FROM (SELECT quote_number AS n FROM quotes
               WHERE customer_id = ANY($1)
               UNION ALL
               SELECT rma_number FROM rmas
               WHERE customer_id = ANY($1)) t
         GROUP BY 1, 2",
    )
    .bind(&customers)
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    let rmas: Vec<i32> = sqlx::query_scalar(
        "DELETE FROM rmas WHERE customer_id = ANY($1) RETURNING id",
    )
    .bind(&customers)
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    let quotes: Vec<i32> = sqlx::query_scalar(
        "DELETE FROM quotes WHERE customer_id = ANY($1) RETURNING id",
    )
    .bind(&customers)
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    for (table, ids) in [
        ("customers", &customers),
        ("products", &products),
        ("sales_reps", &reps),
    ] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM {table} WHERE id = ANY($1)"
        )))
        .bind(ids)
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    // Give the numbers back, unless someone has taken a later one.
    for (prefix, year, last) in taken {
        sqlx::query(
            "UPDATE document_counters c SET last_number = coalesce(
                 (SELECT max(split_part(n, '-', 3)::int)
                  FROM (SELECT quote_number AS n FROM quotes
                        UNION ALL SELECT order_number FROM orders
                        UNION ALL SELECT rma_number FROM rmas) d
                  WHERE n LIKE c.prefix || '-' || c.year || '-%'), 0)
             WHERE prefix = $1 AND year = $2 AND last_number = $3",
        )
        .bind(prefix)
        .bind(year)
        .bind(last)
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    // Last, so the entries the deletes above wrote go too.
    sqlx::query(
        "DELETE FROM audit_log
         WHERE db_role = $1 OR entra_oid = $2
            OR (table_name = 'customers' AND row_id = ANY($3))
            OR (table_name = 'products' AND row_id = ANY($4))
            OR (table_name = 'sales_reps' AND row_id = ANY($5))
            OR (table_name = 'quotes' AND row_id = ANY($6))
            OR (table_name = 'rmas' AND row_id = ANY($7))",
    )
    .bind(&role)
    .bind(oid)
    .bind(&customers)
    .bind(&products)
    .bind(&reps)
    .bind(&quotes)
    .bind(&rmas)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    forget(&su, &oid).await;
}

fn sqlstate(e: &sqlx::Error) -> String {
    e.as_database_error()
        .and_then(|d| d.code())
        .map(|c| c.into_owned())
        .unwrap_or_default()
}

#[tokio::test]
async fn a_credential_logs_in_as_the_reps_own_role() {
    let (auth, fx) = (cairn_auth().await, Fixture::new());
    let who = fx.who.clone();
    let c = session::issue(&auth, &who, 60.0).await.unwrap();
    let mut conn = login(&c).await.unwrap();
    let user: String =
        sqlx::query_scalar("SELECT current_user::text")
            .fetch_one(&mut conn)
            .await
            .unwrap();
    assert_eq!(user, session::role_name(&who.oid));
    assert!(user.starts_with("rep_") && user.len() == 36);
    conn.close().await.unwrap();
}

#[tokio::test]
async fn signing_in_again_resets_the_password_and_reuses_the_role() {
    let (su, auth, fx) =
        (superuser().await, cairn_auth().await, Fixture::new());
    let who = fx.who.clone();
    let first = session::issue(&auth, &who, 60.0).await.unwrap();
    let second = session::issue(&auth, &who, 60.0).await.unwrap();
    assert_eq!(first.user, second.user);
    let e = login(&first).await.unwrap_err();
    assert_eq!(sqlstate(&e), "28P01", "{e}");
    login(&second).await.unwrap().close().await.unwrap();
    let (roles, rows): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM pg_roles WHERE rolname = $1),
                (SELECT count(*) FROM users WHERE role_name = $1)",
    )
    .bind(&first.user)
    .fetch_one(&su)
    .await
    .unwrap();
    assert_eq!((roles, rows), (1, 1));
}

#[tokio::test]
async fn an_expired_login_is_refused_and_an_open_connection_loses_access(
) {
    let (auth, fx) = (cairn_auth().await, Fixture::new());
    let who = fx.who.clone();
    let c = session::issue(&auth, &who, 2.0).await.unwrap();
    let mut open = login(&c).await.unwrap();
    sqlx::query("SELECT count(*) FROM quotes")
        .execute(&mut open)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;

    // A new login: Postgres checks VALID UNTIL and refuses.
    let e = login(&c).await.unwrap_err();
    assert_eq!(sqlstate(&e), "28P01", "{e}");

    // The connection opened before the expiry is still connected, but
    // every table's policy now refuses it.
    let e = sqlx::query("SELECT count(*) FROM quotes")
        .execute(&mut open)
        .await
        .unwrap_err();
    assert_eq!(sqlstate(&e), "28000", "{e}");
    assert!(e.to_string().contains("has expired"), "{e}");
    open.close().await.unwrap();
}

#[tokio::test]
async fn sign_out_ends_access_on_open_connections_at_once() {
    let (auth, fx) = (cairn_auth().await, Fixture::new());
    let who = fx.who.clone();
    let c = session::issue(&auth, &who, 60.0).await.unwrap();
    let mut conn = login(&c).await.unwrap();
    sqlx::query("SELECT cairn_sign_out()")
        .execute(&mut conn)
        .await
        .unwrap();
    let e = sqlx::query("SELECT count(*) FROM customers")
        .execute(&mut conn)
        .await
        .unwrap_err();
    assert_eq!(sqlstate(&e), "28000", "{e}");
    conn.close().await.unwrap();
}

#[tokio::test]
async fn every_change_is_audited_with_the_name_as_it_was() {
    let (su, auth, mut fx) =
        (superuser().await, cairn_auth().await, Fixture::new());
    let who = fx.who.clone();
    let (customer, _) = fx.customer_and_product(&su).await;
    let c = session::issue(&auth, &who, 60.0).await.unwrap();
    let mut conn = login(&c).await.unwrap();
    sqlx::query("UPDATE customers SET customer_status = 'Active' WHERE id = $1")
        .bind(customer)
        .execute(&mut conn)
        .await
        .unwrap();
    conn.close().await.unwrap();
    forget(&su, &who.oid).await; // the users row is gone; the audit row isn't

    let (db_role, oid, name, old, new): (String, Uuid, String, String, String) =
        sqlx::query_as(
            "SELECT db_role, entra_oid, display_name,
                    old_row->>'customer_status', new_row->>'customer_status'
             FROM audit_log
             WHERE table_name = 'customers' AND row_id = $1
               AND action = 'UPDATE'",
        )
        .bind(customer)
        .fetch_one(&su)
        .await
        .unwrap();
    assert_eq!(db_role, session::role_name(&who.oid));
    assert_eq!((oid, name.as_str()), (who.oid, "Test Rep"));
    assert_eq!((old.as_str(), new.as_str()), ("Prospect", "Active"));
}

#[tokio::test]
async fn a_rep_can_read_the_audit_trail_but_not_change_it() {
    let (auth, fx) = (cairn_auth().await, Fixture::new());
    let who = fx.who.clone();
    let c = session::issue(&auth, &who, 60.0).await.unwrap();
    let mut conn = login(&c).await.unwrap();
    sqlx::query("SELECT count(*) FROM audit_log")
        .execute(&mut conn)
        .await
        .unwrap();
    for sql in [
        "DELETE FROM audit_log",
        "UPDATE audit_log SET display_name = 'nobody'",
        "INSERT INTO audit_log (table_name, row_id, action, db_role)
         VALUES ('quotes', 1, 'UPDATE', 'nobody')",
        "UPDATE users SET expires_at = now() + interval '1 year'",
    ] {
        let e =
            sqlx::query(sql).execute(&mut conn).await.unwrap_err();
        assert_eq!(sqlstate(&e), "42501", "{sql}: {e}"); // permission denied
    }
    conn.close().await.unwrap();
}

#[tokio::test]
async fn a_return_naming_a_missing_order_is_refused() {
    let (su, auth, mut fx) =
        (superuser().await, cairn_auth().await, Fixture::new());
    let who = fx.who.clone();
    let (customer, product) = fx.customer_and_product(&su).await;
    let c = session::issue(&auth, &who, 60.0).await.unwrap();
    let mut conn = login(&c).await.unwrap();
    let insert =
        "INSERT INTO rmas (customer_id, order_id, product_id, reason)
                  VALUES ($1, $2, $3, $4)";
    let missing_order = sqlx::query(insert)
        .bind(customer)
        .bind(Some(-14237))
        .bind(product)
        .bind("Failed in Warranty")
        .execute(&mut conn)
        .await
        .unwrap_err();
    assert_eq!(sqlstate(&missing_order), "23503", "{missing_order}");
    let made_up_reason = sqlx::query(insert)
        .bind(customer)
        .bind(None::<i32>)
        .bind(product)
        .bind("'Failed in Warranty'")
        .execute(&mut conn)
        .await
        .unwrap_err();
    assert_eq!(
        sqlstate(&made_up_reason),
        "23514",
        "{made_up_reason}"
    );
    // No order at all is allowed: Chapter 8 routes it to the account
    // manager. And the database numbers it.
    let number: String = sqlx::query_scalar(
        "INSERT INTO rmas (customer_id, order_id, product_id, reason)
         VALUES ($1, $2, $3, $4) RETURNING rma_number",
    )
    .bind(customer)
    .bind(None::<i32>)
    .bind(product)
    .bind("Failed in Warranty")
    .fetch_one(&mut conn)
    .await
    .unwrap();
    assert!(number.starts_with("RMA-20"), "{number}");
    conn.close().await.unwrap();
}

#[tokio::test]
async fn two_quotes_started_at_the_same_moment_get_different_numbers()
{
    let (su, auth, mut fx) =
        (superuser().await, cairn_auth().await, Fixture::new());
    let who = fx.who.clone();
    let (customer, _) = fx.customer_and_product(&su).await;
    let rep = fx.rep(&su).await;
    let c = session::issue(&auth, &who, 60.0).await.unwrap();
    let insert = |c: &Credential| {
        let c = Credential {
            user: c.user.clone(),
            password: c.password.clone(),
            expires_at: c.expires_at.clone(),
            display_name: c.display_name.clone(),
        };
        async move {
            let mut conn = login(&c).await.unwrap();
            sqlx::query_scalar::<_, String>(
                "INSERT INTO quotes (customer_id, rep_id) VALUES ($1, $2)
                 RETURNING quote_number",
            )
            .bind(customer)
            .bind(rep)
            .fetch_one(&mut conn)
            .await
            .unwrap()
        }
    };
    let (a, b) = tokio::join!(insert(&c), insert(&c));
    assert_ne!(a, b);
    assert!(a.starts_with("Q-20") && b.starts_with("Q-20"));
}

/// The refusal as the app sees it, for `--nocapture` runs: Display,
/// SQLSTATE, and Postgres's hint, verbatim.
fn show(path: &str, e: &sqlx::Error) {
    let hint = e
        .as_database_error()
        .and_then(|d| d.try_downcast_ref::<PgDatabaseError>())
        .and_then(|d| d.hint());
    println!(
        "{path}:\n  Display:  {e}\n  SQLSTATE: {}\n  hint:     {}\n  \
         session_expired: {:?}",
        sqlstate(e),
        hint.unwrap_or("(none)"),
        session_expired(e)
    );
}

#[tokio::test]
async fn a_new_connection_after_valid_until_is_an_expired_session() {
    let (auth, fx) = (cairn_auth().await, Fixture::new());
    let who = fx.who.clone();
    let c = session::issue(&auth, &who, 2.0).await.unwrap();
    login(&c).await.unwrap().close().await.unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
    let e = login(&c).await.unwrap_err();
    show("A new connection after VALID UNTIL", &e);
    assert_eq!(sqlstate(&e), "28P01", "{e}");
    assert_eq!(session_expired(&e), Some(Expired::NewConnection));
}

#[tokio::test]
async fn an_open_connection_after_expires_at_is_an_expired_session() {
    let (auth, fx) = (cairn_auth().await, Fixture::new());
    let who = fx.who.clone();
    let c = session::issue(&auth, &who, 2.0).await.unwrap();
    let mut open = login(&c).await.unwrap();
    sqlx::query("SELECT count(*) FROM quotes")
        .execute(&mut open)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
    let e = sqlx::query("SELECT count(*) FROM quotes")
        .execute(&mut open)
        .await
        .unwrap_err();
    show("An open connection after users.expires_at", &e);
    assert_eq!(sqlstate(&e), "28000", "{e}");
    assert_eq!(session_expired(&e), Some(Expired::OpenConnection));
    let hint = e
        .as_database_error()
        .and_then(|d| d.try_downcast_ref::<PgDatabaseError>())
        .and_then(|d| d.hint());
    assert_eq!(hint, Some("Sign in again."));
    open.close().await.unwrap();
}

#[tokio::test]
async fn other_refusals_are_not_an_expired_session() {
    let (auth, fx) = (cairn_auth().await, Fixture::new());
    let who = fx.who.clone();
    let c = session::issue(&auth, &who, 60.0).await.unwrap();
    let mut conn = login(&c).await.unwrap();
    let e = sqlx::query("DELETE FROM audit_log")
        .execute(&mut conn)
        .await
        .unwrap_err();
    assert_eq!(sqlstate(&e), "42501", "{e}");
    assert_eq!(session_expired(&e), None);
    conn.close().await.unwrap();
}

/// A 28000 while connecting is a refusal no new password fixes (here a
/// role made `NOLOGIN`, as offboarding does; a missing `pg_hba.conf`
/// entry is the same code), so it isn't an expired session.
#[tokio::test]
async fn a_28000_while_connecting_is_not_an_expired_session() {
    let (auth, fx) = (cairn_auth().await, Fixture::new());
    let who = fx.who.clone();
    let c = session::issue(&auth, &who, 60.0).await.unwrap();
    let su = superuser().await;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "ALTER ROLE {} NOLOGIN",
        c.user
    )))
    .execute(&su)
    .await
    .unwrap();
    let e = login(&c).await.unwrap_err();
    show("A role made NOLOGIN, connecting", &e);
    assert_eq!(sqlstate(&e), "28000", "{e}");
    assert_eq!(session_expired(&e), None);
}
