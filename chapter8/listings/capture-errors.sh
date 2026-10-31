#!/bin/sh
# Captures the two compiler errors H4 quotes, each from a copy of
# first-route with one change. Writes to chapter8/outputs/.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out="$here/../outputs"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

capture() { # name, sed script
    rm -rf "$work/first-route"
    cp -R "$here/first-route" "$work/first-route"
    rm -rf "$work/first-route/target"
    sed -i '' "$2" "$work/first-route/src/lib.rs"
    sed -i '' 's#path = "../../crate#path = "'"$here"'/../crate#' "$work/first-route/Cargo.toml"
    (cd "$work/first-route" && cargo build -q 2>&1 | sed '/^warning/,/^$/d') > "$out/$1" || true
}

# The days become optional, and the query's comparisons stay as written.
capture e0308-first-route-option.txt 's/pub days_in_service: i32,/pub days_in_service: Option<i32>,/'

# A route for the returns with no order, and no text for it yet.
capture e0004-first-route-unknown-order.txt 's/    CustomerPaysRepair,$/    CustomerPaysRepair,\
    UnknownOrder,/'
