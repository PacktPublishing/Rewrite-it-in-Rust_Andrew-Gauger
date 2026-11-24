-- Chapter 13: the 100,000 products in Postgres 17 with pg_trgm.
-- A scratch database on the dev server, never the Cairn database.
--   psql -h "part 2/crate/.pgdata" -p 5433 -U postgres -c "create database trgm_scratch"
--   PGTRGM_DIR=/tmp/pgtrgm cargo test -q --release -p cairn-app \
--     --test lookup_profile -- --ignored dump_for_postgres
--   psql ... -d trgm_scratch -f pg-trgm-setup.sql
create extension pg_trgm;
create table products(key int primary key, text text, detail text, keywords text);
\copy products from '/tmp/pgtrgm/rows.tsv'
create index p_text   on products using gin (lower(text)     gin_trgm_ops);
create index p_detail on products using gin (lower(detail)   gin_trgm_ops);
create index p_kw     on products using gin (lower(keywords) gin_trgm_ops);
analyze products;
