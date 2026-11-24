# Chapter 13: The Order Form as a State Machine

The workspace this chapter builds is in [`crate/`](crate/), as it stood at
the end of the chapter (tag `ch13-orderform`): everything from Chapter 12,
plus the order form's save lifecycle as one value (`Phase`: `Editing`,
`Saving` or `Error`), Save & Close and Save & New, a save for a form the rep
already left that still lands while the screen change is skipped, and a
product search over every field. Two measured alternatives to the scan sit in
`cairn-forms` and are not wired into the app: a prefix index and a trigram
index. Later chapters grow the workspace in
[`../part 2/crate/`](../part%202/crate/); this copy stays as Chapter 13
printed it.

| Folder | What it is |
|---|---|
| `crate/` | The Cairn V2 workspace as of this chapter. Its README says how to run it |
| `outputs/` | Captured output quoted in the chapter. Its README says how each was made |

The tests that need no database:

```sh
cd crate
cargo test --workspace
cargo test -q --release -p cairn-app --test lookup_timing -- --nocapture
cargo test -q --release -p cairn-app --test lookup_profile ten_thousand -- --nocapture
```

The 10,000 products in the timing tests are built in code, not loaded from
Postgres. The search-threshold sweep and the million-row profile are ignored
by default:

```sh
cargo test -q --release -p cairn-app --test lookup_profile sweep -- --ignored --nocapture
```

The tests that write to the dev database need PostgreSQL 17:

```sh
cd crate
sh dev-db.sh
export DATABASE_URL=postgres://cairn_owner:cairn_owner_dev@127.0.0.1:5433/cairn
cargo run -p migration -- up
cargo test -p cairn-app --test conflicts --test saves --test manufacturer -- --ignored --test-threads 1
```

The screens show placeholder rows, not Cairn's data.
