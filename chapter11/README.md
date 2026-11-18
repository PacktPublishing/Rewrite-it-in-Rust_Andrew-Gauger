# Chapter 11: Talking to Postgres Without Freezing the UI

The workspace this chapter builds is in [`crate/`](crate/), as it stood at
the end of the chapter: everything from Chapter 10, plus the background
worker (`cairn-app/src/worker.rs`), the App Orchestrator
(`cairn-app/src/orchestrator.rs`), saves to Postgres (`save.rs`), the
manufacturer client and its retry prompt (`manufacturer.rs`), and
`mock-manufacturer`, a stand-in for the manufacturer's ordering API that
can drop a connection on purpose. Later chapters grow the workspace in
[`../part 2/crate/`](../part%202/crate/); this copy stays as Chapter 11
printed it.

| Folder | What it is |
|---|---|
| `crate/` | The Cairn V2 workspace as of this chapter. Its README says how to run it |
| `outputs/` | Captured output quoted in the chapter. Its README says how each was made |

The screens show placeholder rows from `dev-ipsum.sh`, not Cairn's data.
The latency figures come from `dev-latency`, a local proxy that delays
every packet by a fixed amount: they are injected values, not a
measurement of any real network.

Everything below needs PostgreSQL 17 and `sqlx-cli` 0.9.0
(`cargo install sqlx-cli --version 0.9.0`). Without `DATABASE_URL` the
build reads the committed `.sqlx` cache instead of the database.

```sh
cd crate
sh dev-db.sh
export DATABASE_URL=postgres://cairn_owner:cairn_owner_dev@127.0.0.1:5433/cairn
cargo run -p migration -- up
sh dev-ipsum.sh
cargo test --workspace
cargo sqlx prepare --workspace --check
```

The frame timings from the chapter's first section, blocking and on a
worker:

```sh
cargo run -p cairn-app --bin dev-latency -- --delay-ms 300 &
cargo test --release -p cairn-app --test scroll_timing -- --ignored --nocapture
```

The retry prompt, against the mock manufacturer dropping the first
connection:

```sh
cargo run -p mock-manufacturer -- --mode drop --times 1 --ledger target/mfr-ledger.json
CAIRN_MANUFACTURER_URL=http://localhost:8765/orders cargo run -p cairn-app
cargo run -p cairn-app --bin mfr-status -- <correlation-id>
```
