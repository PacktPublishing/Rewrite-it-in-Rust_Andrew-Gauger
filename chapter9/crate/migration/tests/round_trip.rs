//! Every `down` undoes its `up` exactly: apply all, roll all back, apply
//! again, and the schema is the same. Chapter 15's cutover depends on it.
//!
//! Uses a scratch database on the local server from `dev-db.sh`.

use migration::{Migrator, MigratorTrait};
use sea_orm_migration::sea_orm::Database;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::path::PathBuf;

const SCRATCH: &str = "cairn_round_trip";

async fn superuser(db: &str) -> PgPool {
    let socket =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.pgdata");
    let opts = PgConnectOptions::new()
        .socket(socket)
        .port(5433)
        .username("postgres")
        .database(db);
    PgPoolOptions::new().connect_with(opts).await.unwrap()
}

/// Every table, column, constraint, index, policy, trigger and function
/// in `public`, one line each, sorted.
async fn schema(db: &PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT x FROM (
           SELECT 'column ' || table_name || '.' || column_name || ' '
                  || data_type || ' ' || is_nullable || ' '
                  || coalesce(column_default, '') AS x
             FROM information_schema.columns WHERE table_schema = 'public'
           UNION ALL
           SELECT 'constraint ' || conrelid::regclass || ' '
                  || pg_get_constraintdef(oid)
             FROM pg_constraint WHERE connamespace = 'public'::regnamespace
           UNION ALL
           SELECT 'index ' || indexdef FROM pg_indexes
            WHERE schemaname = 'public'
           UNION ALL
           SELECT 'policy ' || tablename || ' ' || policyname || ' '
                  || array_to_string(roles, ',') || ' ' || cmd || ' '
                  || coalesce(qual, '') || ' ' || coalesce(with_check, '')
             FROM pg_policies WHERE schemaname = 'public'
           UNION ALL
           SELECT 'rls ' || relname FROM pg_class
            WHERE relnamespace = 'public'::regnamespace AND relrowsecurity
           UNION ALL
           SELECT 'trigger ' || tgrelid::regclass || ' ' || tgname
             FROM pg_trigger WHERE NOT tgisinternal
           UNION ALL
           SELECT 'function ' || p.oid::regprocedure || ' '
                  || md5(pg_get_functiondef(p.oid))
             FROM pg_proc p WHERE pronamespace = 'public'::regnamespace
           UNION ALL
           SELECT 'grant ' || table_name || ' ' || grantee || ' '
                  || privilege_type
             FROM information_schema.role_table_grants
            WHERE table_schema = 'public' AND grantee LIKE 'cairn%'
         ) s
         WHERE x NOT LIKE '%seaql_migrations%'
         ORDER BY x",
    )
    .fetch_all(db)
    .await
    .unwrap()
}

#[tokio::test]
async fn down_undoes_up_exactly() {
    let admin = superuser("postgres").await;
    for sql in [
        format!("DROP DATABASE IF EXISTS {SCRATCH}"),
        format!("CREATE DATABASE {SCRATCH} OWNER cairn_owner"),
    ] {
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(&admin)
            .await
            .unwrap();
    }
    let scratch = superuser(SCRATCH).await;
    sqlx::query("ALTER SCHEMA public OWNER TO cairn_owner")
        .execute(&scratch)
        .await
        .unwrap();

    let url = format!(
        "postgres://cairn_owner:cairn_owner_dev@127.0.0.1:5433/{SCRATCH}"
    );
    let db = Database::connect(&url).await.unwrap();
    let before = schema(&scratch).await;
    Migrator::up(&db, None).await.unwrap();
    let first = schema(&scratch).await;
    assert!(first.len() > 300, "only {} schema lines", first.len());

    Migrator::down(&db, None).await.unwrap();
    assert_eq!(
        schema(&scratch).await,
        before,
        "down left something behind"
    );

    Migrator::up(&db, None).await.unwrap();
    assert_eq!(schema(&scratch).await, first);

    db.close().await.unwrap();
    scratch.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE {SCRATCH}"
    )))
    .execute(&admin)
    .await
    .unwrap();
}
