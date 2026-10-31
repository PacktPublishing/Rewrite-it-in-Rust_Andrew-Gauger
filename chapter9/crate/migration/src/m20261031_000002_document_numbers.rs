//! Document numbers the database assigns. V1's `NextNumber` took the
//! highest number in the form plus one, the moment someone started typing,
//! so two reps a minute apart got the same number (Chapter 6).
//!
//! Here a per-year counter row is locked and bumped inside the insert, so
//! two inserts at the same moment get different numbers, and a failed
//! insert gives its number back when it rolls back.

use crate::tables::DocumentCounters as D;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(D::Table)
                .col(
                    ColumnDef::new(D::Prefix)
                        .string_len(5)
                        .not_null(),
                )
                .col(ColumnDef::new(D::Year).integer().not_null())
                .col(
                    ColumnDef::new(D::LastNumber)
                        .integer()
                        .not_null(),
                )
                .primary_key(
                    Index::create().col(D::Prefix).col(D::Year),
                )
                .to_owned(),
        )
        .await?;
        // Format(n, "0000") in V1: at least four digits, more past 9999.
        m.get_connection()
            .execute_unprepared(
                r#"
CREATE OR REPLACE FUNCTION next_document_number(prefix text) RETURNS text
LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = public AS $$
    INSERT INTO document_counters AS c (prefix, year, last_number)
    VALUES (prefix, extract(year FROM current_date)::int, 1)
    ON CONFLICT (prefix, year)
        DO UPDATE SET last_number = c.last_number + 1
    RETURNING c.prefix || '-' || c.year || '-'
        || lpad(c.last_number::text,
                greatest(4, length(c.last_number::text)), '0')
$$;
REVOKE ALL ON FUNCTION next_document_number(text) FROM PUBLIC;
"#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.get_connection()
            .execute_unprepared(
                "DROP FUNCTION next_document_number(text)",
            )
            .await?;
        m.drop_table(Table::drop().table(D::Table).to_owned()).await
    }
}
