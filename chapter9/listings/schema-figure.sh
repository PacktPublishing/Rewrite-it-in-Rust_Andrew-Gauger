#!/bin/sh
# Figure 9.3: the returns table and what it points to, drawn from the
# migrated database. tbls writes Graphviz source; this lays it out left to
# right, keeps only the arrows out of rmas and the key columns they point
# to, and drops the "[BASE TABLE]" and "[integer]" labels so it reads at
# page width. Needs tbls and Graphviz (brew install tbls graphviz).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out="$here/../outputs"
tbls out -c "$here/tbls.yml" -t dot --table public.rmas --distance 1 \
    -o "$out/schema-rmas.dot"
awk '/^  "public\.[a-z_]*" \[shape=none/ { table = $1 }
     /<tr><td port="/ && table != "\"public.rmas\"" && !/port="id"/ { next }
     { print }' "$out/schema-rmas.dot" |
sed -e '/^  "public\.[a-z_]*":"[a-z_]*" -> /{/^  "public\.rmas"/!d;}' \
    -e 's/rankdir=TB/rankdir=LR/' -e 's/splines=none/splines=true/' \
    -e 's/margin=0.6/margin=0.05/' \
    -e 's#&nbsp;&nbsp;&nbsp;&nbsp;<font color="\#666666">\[BASE TABLE\]</font>##' \
    -e 's# <font color="\#666666">\[integer\]</font>##g' \
    -e 's/public\.//g' |
dot -Tpng -Gdpi=50 -Gnodesep=0.15 -Granksep=0.6 -o "$out/schema-rmas.png"
