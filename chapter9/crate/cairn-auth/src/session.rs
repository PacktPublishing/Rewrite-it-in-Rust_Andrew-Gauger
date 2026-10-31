//! The one thing `cairn-auth` changes in Postgres: a rep's login role, its
//! password and expiry, and the rep's row in `users`.

use crate::token::random_token;
use postgres_protocol::password::scram_sha_256;
use sqlx::{AssertSqlSafe, PgPool};
use uuid::Uuid;

/// Who Entra says signed in, after `cairn-auth` has matched them to Cairn.
#[derive(Clone, Debug)]
pub struct Identity {
    pub oid: Uuid,
    pub upn: String,
    pub display_name: String,
    pub rep_id: Option<i32>,
    pub is_admin: bool,
}

/// What the desktop app gets back: a Postgres login and when it ends.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Credential {
    pub user: String,
    pub password: String,
    pub expires_at: String,
    pub display_name: String,
}

/// One login role per person. Built from Entra's object ID, never from
/// anything a person typed.
pub fn role_name(oid: &Uuid) -> String {
    format!("rep_{}", oid.simple())
}

/// Gives the rep's role a new password that Postgres expires by itself.
pub async fn issue(
    db: &PgPool,
    who: &Identity,
    lifetime_secs: f64,
) -> Result<Credential, sqlx::Error> {
    let role = role_name(&who.oid);
    let password = random_token();
    let hashed = scram_sha_256(password.as_bytes());
    let mut tx = db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(&role)
        .execute(&mut *tx)
        .await?;
    let until: String = sqlx::query_scalar(
        "SELECT (now() + make_interval(secs => $1))::text",
    )
    .bind(lifetime_secs)
    .fetch_one(&mut *tx)
    .await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT FROM pg_roles WHERE rolname = $1)",
    )
    .bind(&role)
    .fetch_one(&mut *tx)
    .await?;
    // CREATE ROLE and ALTER ROLE take no bind parameters. Every piece
    // here is ours: hex digits, a SCRAM hash, a timestamp from now().
    if !exists {
        let sql =
            format!("CREATE ROLE {role} LOGIN IN ROLE cairn_app");
        sqlx::query(AssertSqlSafe(sql)).execute(&mut *tx).await?;
    }
    let sql = format!(
        "ALTER ROLE {role} PASSWORD '{hashed}' VALID UNTIL '{until}'"
    );
    sqlx::query(AssertSqlSafe(sql)).execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO users (entra_oid, upn, display_name, role_name,
                            rep_id, is_admin, signed_in_at, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, now(), $7::timestamptz)
         ON CONFLICT (entra_oid) DO UPDATE SET
             upn = EXCLUDED.upn, display_name = EXCLUDED.display_name,
             rep_id = EXCLUDED.rep_id, is_admin = EXCLUDED.is_admin,
             signed_in_at = now(), expires_at = EXCLUDED.expires_at",
    )
    .bind(who.oid)
    .bind(&who.upn)
    .bind(&who.display_name)
    .bind(&role)
    .bind(who.rep_id)
    .bind(who.is_admin)
    .bind(&until)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Credential {
        user: role,
        password,
        expires_at: until,
        display_name: who.display_name.clone(),
    })
}

/// Matches an Entra sign-in to Cairn: a rep whose `entra_upn` is this
/// sign-in name, or anyone Entra gives the `Cairn.Admin` role. Anyone else
/// in the tenant is refused.
pub async fn admit(
    db: &PgPool,
    oid: Uuid,
    upn: &str,
    display_name: &str,
    roles: &[String],
) -> Result<Option<Identity>, sqlx::Error> {
    let rep_id: Option<i32> = sqlx::query_scalar(
        "SELECT id FROM sales_reps
         WHERE lower(entra_upn) = lower($1) AND active",
    )
    .bind(upn)
    .fetch_optional(db)
    .await?;
    let is_admin = roles.iter().any(|r| r == "Cairn.Admin");
    if rep_id.is_none() && !is_admin {
        return Ok(None);
    }
    Ok(Some(Identity {
        oid,
        upn: upn.to_owned(),
        display_name: display_name.to_owned(),
        rep_id,
        is_admin,
    }))
}
