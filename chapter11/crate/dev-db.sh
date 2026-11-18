#!/bin/sh
# A local Postgres for Cairn V2, in .pgdata/ beside this script.
#
#   sh dev-db.sh          # create it the first time, start it after that
#   sh dev-db.sh stop
#
# Listens on 127.0.0.1:5433 so it doesn't collide with another Postgres.
# Local socket connections are trusted (it's a development database);
# TCP connections need a password (SCRAM-SHA-256), the way the app and
# cairn-auth connect.
#
# The three roles every migration assumes:
#   cairn_owner  owns the database and runs the migrations
#   cairn_app    no login; every rep's role is a member of it
#   cairn_auth   cairn-auth's own login; may create roles and set their
#                passwords, and may hand out cairn_app without using it
set -eu
here=$(cd "$(dirname "$0")" && pwd)
data="$here/.pgdata"
port=${CAIRN_PG_PORT:-5433}

if [ "${1:-}" = stop ]; then
    pg_ctl -D "$data" stop
    exit
fi

if [ ! -f "$data/PG_VERSION" ]; then
    initdb -D "$data" -U postgres --auth-local=trust \
        --auth-host=scram-sha-256 >/dev/null
    fresh=1
fi
pg_ctl -D "$data" status >/dev/null 2>&1 ||
    pg_ctl -D "$data" -o "-p $port -k '$data' -c listen_addresses=127.0.0.1" \
        -l "$data/server.log" -w start >/dev/null

if [ -n "${fresh:-}" ]; then
    psql -h "$data" -p "$port" -U postgres -q -v ON_ERROR_STOP=1 <<'SQL'
CREATE ROLE cairn_owner LOGIN PASSWORD 'cairn_owner_dev';
CREATE ROLE cairn_app NOLOGIN;
CREATE ROLE cairn_auth LOGIN CREATEROLE PASSWORD 'cairn_auth_dev';
GRANT cairn_app TO cairn_auth WITH ADMIN TRUE, INHERIT FALSE, SET FALSE;
CREATE DATABASE cairn OWNER cairn_owner;
REVOKE ALL ON DATABASE cairn FROM PUBLIC;
GRANT CONNECT ON DATABASE cairn TO cairn_app, cairn_auth;
\c cairn
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
ALTER SCHEMA public OWNER TO cairn_owner;
SQL
    echo "Created .pgdata (PostgreSQL $(pg_ctl --version | awk '{print $3}'))"
fi
echo "Postgres is on 127.0.0.1:$port, database cairn"
echo "  migrations: DATABASE_URL=postgres://cairn_owner:cairn_owner_dev@127.0.0.1:$port/cairn"
