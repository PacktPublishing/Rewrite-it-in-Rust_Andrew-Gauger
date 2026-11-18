//! The audit trail V1 never had: every insert, update and delete on an
//! editable table, with who made it.

use crate::tables::EDITABLE;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        // Who is stored as values, not as a link to users: an entry must
        // say who it was at the time, even after a name changes.
        db.execute_unprepared(
            r#"
CREATE TABLE audit_log (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    changed_at   timestamptz NOT NULL DEFAULT now(),
    table_name   text NOT NULL,
    row_id       integer NOT NULL,
    action       text NOT NULL
                 CHECK (action IN ('INSERT', 'UPDATE', 'DELETE')),
    db_role      text NOT NULL,
    entra_oid    uuid,
    display_name text,
    old_row      jsonb,
    new_row      jsonb
);
CREATE INDEX ix_audit_log_row ON audit_log (table_name, row_id);

-- Runs as the table owner (SECURITY DEFINER), so no rep's role needs, or
-- has, any right to write audit_log. Inside it, current_user is the
-- owner; session_user is still the rep who logged in.
CREATE OR REPLACE FUNCTION audit_row() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public AS $$
DECLARE
    who users%ROWTYPE;
    r   record := CASE TG_OP WHEN 'DELETE' THEN OLD ELSE NEW END;
BEGIN
    SELECT * INTO who FROM users WHERE role_name = session_user;
    INSERT INTO audit_log (table_name, row_id, action, db_role,
                           entra_oid, display_name, old_row, new_row)
    VALUES (TG_TABLE_NAME, r.id, TG_OP, session_user,
            who.entra_oid, who.display_name,
            CASE WHEN TG_OP <> 'INSERT' THEN to_jsonb(OLD) END,
            CASE WHEN TG_OP <> 'DELETE' THEN to_jsonb(NEW) END);
    RETURN NULL;
END
$$;
REVOKE ALL ON FUNCTION audit_row() FROM PUBLIC;
"#,
        )
        .await?;
        for t in EDITABLE {
            db.execute_unprepared(&format!(
                "CREATE TRIGGER audit AFTER INSERT OR UPDATE OR DELETE \
                 ON {t} FOR EACH ROW EXECUTE FUNCTION audit_row()"
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for t in EDITABLE {
            db.execute_unprepared(&format!(
                "DROP TRIGGER audit ON {t}"
            ))
            .await?;
        }
        db.execute_unprepared(
            "DROP FUNCTION audit_row();
             DROP TABLE audit_log;",
        )
        .await?;
        Ok(())
    }
}
