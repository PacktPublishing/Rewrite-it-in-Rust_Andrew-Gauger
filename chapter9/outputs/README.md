# Chapter 9: captured output

Every file here is verbatim output from the run named beside it. The
database is the local one from `part 2/crate/dev-db.sh`, migrated and
seeded with `dev-seed.sh` (PostgreSQL 17.11, rustc 1.99.0, 2026-10-31).

| File | What made it |
|---|---|
| `sign-in-mtorres.txt` | `cairn-auth` run with `CAIRN_SESSION_SECONDS=120`, then `CAIRN_NO_BROWSER=1 CAIRN_WAIT_FOR_EXPIRY=1 cargo run -p cairn-auth --example sign_in`, signed in through the test Entra tenant as `mtorres` in a private browser window (the second sign-in of 2026-10-31; refusals print as SQLSTATE and Postgres's message) |
| `audit-row.txt` | `psql -a -x` as `cairn_owner`, after the first sign-in of 2026-10-31, which also ran `UPDATE quotes SET quote_status = 'Sent' WHERE quote_number = 'Q-2026-0001'`: the audit row Maya's update wrote |
| `rma-missing-order.txt` | `psql -a` as `cairn_owner`, inserting RMA-2025-0045 the way V1 holds it, with order 14237 |
| `e0277-dynamic-sql.txt` | `listings/capture-errors.sh`: `cairn-auth` built with the `ALTER ROLE` string passed to `sqlx::query` without `AssertSqlSafe` |
| `schema-rmas.png`, `schema-rmas.dot` | `listings/schema-figure.sh` (tbls 1.96.1, Graphviz 16.1.0). Figure 9.3 |
| `schema-all.png` | `tbls out -c listings/tbls.yml -t png`: the whole schema, key columns only |
