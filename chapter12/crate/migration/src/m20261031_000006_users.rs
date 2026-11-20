//! Who is signed in. V1's Users table, with its plain-text passwords, is
//! not ported. This one has no password column: `cairn-auth` writes one
//! row per person, mapped to that person's Postgres login role, and
//! Postgres keeps the password (hashed) and its expiry on the role.

use crate::tables::{SalesReps, Users as U};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(U::Table)
                .col(crate::columns::id(U::Id))
                .col(
                    ColumnDef::new(U::EntraOid)
                        .uuid()
                        .not_null()
                        .unique_key(),
                )
                .col(
                    ColumnDef::new(U::Upn).string_len(255).not_null(),
                )
                .col(
                    ColumnDef::new(U::DisplayName)
                        .string_len(200)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(U::RoleName)
                        .string_len(63)
                        .not_null()
                        .unique_key(),
                )
                .col(ColumnDef::new(U::RepId).integer())
                .col(
                    ColumnDef::new(U::IsAdmin)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(
                    ColumnDef::new(U::SignedInAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(U::ExpiresAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(&mut crate::columns::fk(
                    "fk_users_rep",
                    (U::Table, U::RepId),
                    SalesReps::Table,
                ))
                .to_owned(),
        )
        .await?;

        // Postgres checks a role's VALID UNTIL only when it logs in, so a
        // connection opened before the expiry outlives it. Every table's
        // access policy calls this check, so an open connection loses
        // access at expires_at too. Its error is SQLSTATE 28000, the same
        // class as a refused login: either way, the app asks the rep to
        // sign in again.
        m.get_connection()
            .execute_unprepared(
                r#"
CREATE OR REPLACE FUNCTION cairn_session_check() RETURNS boolean
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path = public AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM users
                   WHERE role_name = session_user
                     AND expires_at > now()) THEN
        RAISE EXCEPTION 'Cairn session for % has expired', session_user
            USING ERRCODE = 'invalid_authorization_specification',
                  HINT = 'Sign in again.';
    END IF;
    RETURN true;
END
$$;
REVOKE ALL ON FUNCTION cairn_session_check() FROM PUBLIC;

-- Sign-out: the session ends now, for every open connection. The app
-- then throws its copy of the password away.
CREATE OR REPLACE FUNCTION cairn_sign_out() RETURNS void
LANGUAGE sql SECURITY DEFINER SET search_path = public AS $$
    UPDATE users SET expires_at = now() WHERE role_name = session_user
$$;
REVOKE ALL ON FUNCTION cairn_sign_out() FROM PUBLIC;
"#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.get_connection()
            .execute_unprepared(
                "DROP FUNCTION cairn_sign_out();
                 DROP FUNCTION cairn_session_check();",
            )
            .await?;
        m.drop_table(Table::drop().table(U::Table).to_owned()).await
    }
}
