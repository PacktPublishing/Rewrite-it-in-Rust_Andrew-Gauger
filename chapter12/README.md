# Chapter 12: Concurrency Without Locking

The workspace this chapter builds is in [`crate/`](crate/), as it stood at
the end of the chapter (tag `ch12-concurrency`): everything from Chapter 11,
plus a version check on every save, the three-way conflict dialog, list rows
that show a save before Postgres answers, a second orchestrator and worker
for the manufacturer, and one rolling `tracing` log. Later chapters grow the
workspace in [`../part 2/crate/`](../part%202/crate/); this copy stays as
Chapter 12 printed it.

| Folder | What it is |
|---|---|
| `crate/` | The Cairn V2 workspace as of this chapter. Its README says how to run it |
| `outputs/` | Captured output and snapshots quoted in the chapter. Its README says how each was made |

The tests that need no database:

```sh
cd crate
cargo test --workspace
```

The tests that write to the dev database (two reps, each a member of
`cairn_app`) are ignored by default. They need PostgreSQL 17:

```sh
cd crate
sh dev-db.sh
export DATABASE_URL=postgres://cairn_owner:cairn_owner_dev@127.0.0.1:5433/cairn
cargo run -p migration -- up
cargo test -p cairn-app --test conflicts --test saves --test manufacturer -- --ignored --test-threads 1
cargo test -p cairn-app --test head_of_line -- --ignored --nocapture   # about 20 s
```

The two-saves log is written by the same test with the log file named:

```sh
TWO_SAVES_LOG=/tmp/two-saves.log cargo test -p cairn-app --test conflicts \
  without_the_rule -- --ignored
```

The screens show placeholder rows, not Cairn's data.
