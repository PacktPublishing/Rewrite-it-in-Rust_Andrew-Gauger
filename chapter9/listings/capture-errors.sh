#!/bin/sh
# Captures the compiler error H2 quotes, from a copy of the workspace with
# one change: the ALTER ROLE statement goes to sqlx without AssertSqlSafe.
# Writes to chapter9/outputs/.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
crate="$here/../crate"
out="$here/../outputs"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

rsync -a --exclude target --exclude .pgdata "$crate/" "$work/crate/"
sed -i '' 's/^    sqlx::query(AssertSqlSafe(sql))/    sqlx::query(sql)/' \
    "$work/crate/cairn-auth/src/session.rs"
(cd "$work/crate" && CARGO_TARGET_DIR="$crate/target/capture" \
    cargo build -q -p cairn-auth 2>&1 | sed '/^warning/,/^$/d') \
    > "$out/e0277-dynamic-sql.txt" || true
