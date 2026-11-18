//! Who may do what. Assumes the roles `dev-db.sh` creates:
//!
//! - `cairn_app`: every rep's login role is a member. Reads and writes
//!   the business tables, but only while its session hasn't expired.
//!   Reads `users` and `audit_log`, and can't write either.
//! - `cairn_auth`: the sign-in service. Reads reps to match an Entra
//!   identity, and writes `users`. Nothing else.

use crate::tables::EDITABLE;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for t in EDITABLE {
            db.execute_unprepared(&format!(
                "GRANT SELECT, INSERT, UPDATE, DELETE ON {t} TO cairn_app;
                 ALTER TABLE {t} ENABLE ROW LEVEL SECURITY;
                 CREATE POLICY signed_in ON {t} TO cairn_app
                     USING ((SELECT cairn_session_check()))
                     WITH CHECK ((SELECT cairn_session_check()));"
            ))
            .await?;
        }
        db.execute_unprepared(
            r#"
GRANT EXECUTE ON FUNCTION next_document_number(text),
                          cairn_session_check(),
                          cairn_sign_out() TO cairn_app;

GRANT SELECT ON users, audit_log TO cairn_app;
ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_log ENABLE ROW LEVEL SECURITY;
CREATE POLICY signed_in ON users FOR SELECT TO cairn_app
    USING ((SELECT cairn_session_check()));
CREATE POLICY signed_in ON audit_log FOR SELECT TO cairn_app
    USING ((SELECT cairn_session_check()));

GRANT SELECT (id, entra_upn, active) ON sales_reps TO cairn_auth;
CREATE POLICY match_identity ON sales_reps FOR SELECT TO cairn_auth
    USING (true);
GRANT SELECT, INSERT, UPDATE ON users TO cairn_auth;
CREATE POLICY sign_in ON users TO cairn_auth USING (true);
"#,
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        db.execute_unprepared(
            r#"
DROP POLICY sign_in ON users;
REVOKE SELECT, INSERT, UPDATE ON users FROM cairn_auth;
DROP POLICY match_identity ON sales_reps;
REVOKE SELECT (id, entra_upn, active) ON sales_reps FROM cairn_auth;

DROP POLICY signed_in ON audit_log;
DROP POLICY signed_in ON users;
ALTER TABLE audit_log DISABLE ROW LEVEL SECURITY;
ALTER TABLE users DISABLE ROW LEVEL SECURITY;
REVOKE SELECT ON users, audit_log FROM cairn_app;

REVOKE EXECUTE ON FUNCTION next_document_number(text),
                           cairn_session_check(),
                           cairn_sign_out() FROM cairn_app;
"#,
        )
        .await?;
        for t in EDITABLE {
            db.execute_unprepared(&format!(
                "DROP POLICY signed_in ON {t};
                 ALTER TABLE {t} DISABLE ROW LEVEL SECURITY;
                 REVOKE SELECT, INSERT, UPDATE, DELETE ON {t} FROM cairn_app;"
            ))
            .await?;
        }
        Ok(())
    }
}
