#!/usr/bin/env bash
# "Just export it to a real database": the naive path, run for real.
# Reproduces every console block in Heading 4 of Chapter 6.
#
# Needs: mdbtools (tested with v1.0.1) and a local PostgreSQL install
# (initdb, pg_ctl, psql on PATH). Spins up a throwaway cluster in a temp
# directory on port 55432 and tears it down at the end.
#
# Usage: chapter6/export-demo/run.sh [path/to/CairnSystems_History.accdb]
# Default database: the canonical Part II copy in "part 2/" at the repo root.
# output.txt was captured 2026-10-14 against the pre-operations database
# (12 tables, 254,607 rows); re-run before Chapter 15 quotes it.

set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
DB="${1:-$HERE/../../part 2/CairnSystems_History.accdb}"
D="$(mktemp -d /tmp/ch6pg.XXXXXX)"
export PGHOST="$D" PGPORT=55432 PGDATABASE=cairn

initdb -D "$D/data" -U "$(whoami)" >/dev/null 2>&1
pg_ctl -D "$D/data" -o "-p $PGPORT -k $D" -l "$D/log" -w start >/dev/null
createdb cairn
trap 'pg_ctl -D "$D/data" -m fast stop >/dev/null; rm -rf "$D"' EXIT

echo "== mdbtools version"
mdb-schema --version

echo "== 1. Schema"
mdb-schema "$DB" postgres > "$D/schema.sql"
echo "FOREIGN KEY statements in the export: $(grep -c 'FOREIGN KEY' "$D/schema.sql")"
(cd "$D" && psql -q -f schema.sql 2>&1 | head -2)
echo "Foreign keys Postgres actually has: $(psql -Atc "SELECT count(*) FROM information_schema.table_constraints WHERE constraint_type = 'FOREIGN KEY'")"

echo "== 2. Rows, first attempt"
for t in $(mdb-tables -1 "$DB"); do
  mdb-export -I postgres -q "'" -D '%Y-%m-%d %H:%M:%S' "$DB" "$t" > "$D/$t.sql"
  if ! psql -q -1 -v ON_ERROR_STOP=1 -f "$D/$t.sql" >/dev/null 2>"$D/$t.err"; then
    echo "$t: $(grep ERROR "$D/$t.err" | sed 's/^psql:[^ ]* //')"
  fi
done

echo "== 2b. Rows, retrying the refused tables with -B (booleans as TRUE/FALSE)"
for t in $(mdb-tables -1 "$DB"); do
  if [ -s "$D/$t.err" ]; then
    mdb-export -B -I postgres -q "'" -D '%Y-%m-%d %H:%M:%S' "$DB" "$t" > "$D/$t.sql"
    psql -q -1 -v ON_ERROR_STOP=1 -f "$D/$t.sql" >/dev/null
  fi
done
psql -c "ANALYZE" >/dev/null
psql -c "SELECT relname AS table, n_live_tup AS rows FROM pg_stat_user_tables ORDER BY relname"
psql -Atc "SELECT 'total rows: ' || sum(n_live_tup) FROM pg_stat_user_tables"

echo "== 3. Writing to it"
psql -c "INSERT INTO orders (ordernumber, customerid, repid, orderdate, taxrate, shippingcost)
VALUES ('SO-2026-1227', 99999, 1, now(), 0.0825, 0);" 2>&1
psql -qAtc "SELECT setval('orders_orderid_seq', (SELECT max(orderid) FROM orders))" >/dev/null
psql -c "INSERT INTO orders (ordernumber, customerid, repid, orderdate, taxrate, shippingcost)
VALUES ('SO-2026-1227', 99999, 1, now(), 0.0825, 0)
RETURNING orderid, customerid, orderstatus;" 2>&1

echo "== 4. A saved query, verbatim from the Access file"
(cd "$HERE" && psql -f qryOrderLines.sql 2>&1)

echo "== 5. September 2026 commission, Access's rule translated by hand"
(cd "$HERE" && psql -f commission-sept-2026.sql)


echo "== 6. Postgres rounding SO-2026-1053's tax (net sales 12098.00 x 7.25%)"
psql -c "SELECT round(12098.00 * 0.0725, 2) AS tax;"
