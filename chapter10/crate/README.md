# Cairn Systems V2, as of Chapter 10

The workspace exactly as Chapter 10 prints it. Later chapters grow the
workspace in [`../../part 2/crate/`](../../part%202/crate/); this copy stays
as it was. Seven members:

| Crate | Job | Chapter |
|---|---|---|
| `cairn-core` | Business rules: no user interface, no database, no network | 8 |
| `migration` | The Postgres schema, as reversible Rust migrations | 9 |
| `cairn-auth` | Sign-in through Entra ID; hands the app a Postgres login that expires on its own | 9 |
| `company-by-hand` | V1's Company Info form drawn with plain egui, no kit, no database: the chapter's first screen | 10 |
| `cairn-forms` | The form kit: Access-style bound forms, lookups and datasheets on egui | 10 |
| `cairn-forms-derive` | `#[derive(Form)]` | 10 |
| `cairn-app` | The desktop app | 10–14 |

## Running it

`cairn-core` needs only Rust. The other two need PostgreSQL 17
(`initdb`, `pg_ctl` and `psql` on your `PATH`):

```sh
sh dev-db.sh                                 # a local Postgres in .pgdata/, on port 5433
export DATABASE_URL=postgres://cairn_owner:cairn_owner_dev@127.0.0.1:5433/cairn
cargo run -p migration -- up                 # or down, fresh, status
sh dev-seed.sh yournamegmail.onmicrosoft.com # a few of V1's rows, reps linked to Entra
sh dev-ipsum.sh                              # placeholder rows at V1's volumes, for screens
cargo test --workspace                       # 97 tests
cargo run -p cairn-app                       # the app, over whatever the database holds
cargo run -p company-by-hand                 # Chapter 10's first screen, no database
cargo clippy --all-targets --workspace -- -D warnings
```

Signing in for real needs a test Entra tenant, set up by the companion
repository's `chapter9/cairn-auth/setup-entra.sh`, which writes the `.env`
file named below:

```sh
CAIRN_AUTH_ENV=path/to/chapter9/cairn-auth/.env cargo run -p cairn-auth
cargo run -p cairn-auth --example sign_in    # the desktop side, in another terminal
```

## cairn-core

Built to `../../part 2/chapter8/workspace/artifacts/mission.md`. Its status
enums started as an earlier agent run's. The rest
was written by hand.

```sh
cargo test                                   # 69 tests
cargo clippy --all-targets -- -D warnings
cargo run -q --example v1_pasted_json        # V1's pasted-together request vs. cairn-core's
```

- **Port, don't refactor.** Every rule behaves as V1 does. Rules that look
  wrong are kept and marked `// FLAG:`; `cairn-core/FLAGS.md` lists them for
  the sales team, and `tests/flags.rs` keeps the two in step. The V1
  defects this crate corrects are marked `// FIX:`.
- **The oracle is the contract.** `tests/september_2026.rs` asserts what
  Access printed for September 2026, and `tests/routing_772.rs` routes all
  772 returns, both under V1's rules and V2's. Every figure fails under
  plain half-up rounding.
- **Fixtures** in `cairn-core/tests/fixtures/` are exports of
  `../../part 2/CairnSystems_History.accdb`. The September files came with the
  mission. The routing files come from `export_rma_fixtures.sh` (needs
  mdbtools).

Resolved versions (2026-10-30, rustc 1.99.0): rust_decimal 1.43.0, serde
1.0.229, serde_json 1.0.154, uuid 1.29.0; csv 1.4.0 (tests only).

## migration

Eight migrations, one per logical group, each with a `down` that undoes
its `up` exactly (`tests/round_trip.rs` proves it in a scratch database).
V1's 15 tables are carried across column for column, except `Users`. The
deliberate changes:

- **No password table.** `users` has no password column: one row per
  person, mapped to their Postgres login role, written only by
  `cairn-auth`. `sales_reps.entra_upn` links a rep to their Entra account.
- **Returns have foreign keys** to customers, orders, products and reps.
  V1's RMAs table had none.
- **Statuses are checked** against V1's own lists (`columns.rs`).
- **Money is exact.** Currency is `numeric(19,4)`; rates are
  `numeric(9,6)` between 0 and 1. V1 kept rates as floating point.
- **The database assigns document numbers** (`Q-2026-0001`) from a
  per-year counter, so two people can't get the same number.
- **`correlation_id`** on manufacturer orders, unique per attempt.
- **`version`** on every editable table, for Chapter 12.
- **`audit_log`**: a trigger on every editable table records who changed
  what, with the person's Entra object ID and name stored as values.
- **Access rules.** Reps (members of `cairn_app`) can read and write the
  business tables only while their session hasn't expired; can read
  `users` and `audit_log`; can write neither.

## cairn-auth

An `axum` service and a confidential OpenID Connect client for Entra ID.
It keeps nothing that outlives a restart.

1. The desktop app opens, listens on a loopback port, and sends the
   browser to `/auth/start` with the hash of a verifier only it holds.
2. Entra signs the person in. On `/auth/callback`, `cairn-auth` matches
   them to a rep (or the `Cairn.Admin` app role) and sends the browser back
   to the app with a one-time code. Nothing changes in Postgres.
3. The app posts the code and its verifier to `/session`. Only then does
   `cairn-auth` create the rep's login role if it's new, give it a new
   password (sent to Postgres already hashed) that expires in 4 hours
   (`VALID UNTIL`), and upsert the `users` row.

Postgres enforces the expiry: a new login after it fails, and every
table's policy refuses an open connection (SQLSTATE 28000).
`cairn_sign_out()` ends a session at once.

Resolved versions (2026-10-31): sea-orm-migration 2.0.4, sqlx 0.9.0, axum
0.8.9, openidconnect 4.0.1, tokio 1.54.1, postgres-protocol 0.6.12, sha2
0.11.1, base64 0.23.1, rand 0.10.3, url 2.5.8, tracing 0.1.44,
tracing-subscriber 0.3.23, async-trait 0.1.92.

## cairn-forms, cairn-forms-derive and cairn-app

V1 built its 20 forms from a handful of VBA helpers (`Txt`, `Cbo`, `Chk`,
`TotalBox`, `SubF`, `FindBox` in `../../part 2/CairnForms.bas`). The form kit is
those helpers on egui:

- **`RecordForm<T>`** edits one record as a bound form does: the record as
  loaded and the draft, so it knows when there are unsaved changes and can
  undo. Tab follows the fields' order, and locked fields are skipped.
  Enter moves to the next field, and arriving by keyboard selects the
  field. Esc undoes the field, and a second Esc the record. AfterUpdate
  runs when a changed field is left.
- **`#[derive(Form)]`** reads each field's `#[field(...)]`: label, lookup,
  `after_update` handler, `validate` rule, `required`, `read_only`, column.
  Handlers are ordinary functions that never do input or output; they
  return commands for the app.
- **Lookups** are egui 0.37's `CompletionPopup` plus a ranked list and
  V1's limit-to-list rule. One widget serves every combo box.
- **Validation** shows through `egui_form` (default features off): a red
  outline, a message under the field, and focus on the first bad field
  when a save is refused. The rules are plain functions in
  `cairn-core::rules`.
- **Datasheets** are `egui_extras::TableBuilder`: editable ones (a
  `RecordForm` per row, arrows up and down, a blank row for a new line,
  Ctrl+Minus to delete) and read-only lists that build only the rows in
  view.

`cairn-app` reads everything once, before its window opens (`data.rs`),
and saves only into that copy; Chapter 11 moves the database work to
background workers. Every V1 form is there except the two `cairn-auth`
replaced (sign-in and Users): the main menu and its two lists,
Customers (with Contacts, Activities, Quotes and Orders tabs), Quotes,
Sales Orders (with lines and payments, and a datasheet view of all
orders), Products, Sales Reps, Company Info, Activities, Phone Messages
and RMAs. Each is in `src/forms/`, field for field and in V1's Tab order.
Single-table forms share `screens/record.rs`, with Access's navigation
bar; moving to another record saves this one first, as Access did.
Buttons for later chapters (printing, month-end, quote to order, the
manufacturer) are there, disabled, naming the chapter. It connects as
`cairn_owner` on the dev database unless `CAIRN_DATABASE_URL` says
otherwise; signing in through `cairn-auth` arrives with Chapter 11.

```sh
sh dev-ipsum.sh                              # placeholder rows to look at
cargo run -p cairn-app
cargo test -p cairn-app --test keyboard      # V1's keystrokes, no database
cargo test -p cairn-app --test forms         # navigation, V1's buttons, no database
UPDATE_SNAPSHOTS=1 cargo test -p cairn-app --test screens -- --ignored
cargo test --release -p cairn-app --test scroll_timing -- --ignored --nocapture
cargo check --target aarch64-linux-android -p cairn-core -p cairn-forms
```

The screenshots are taken at 1180 × 760 and at 1024 × 768 (an iPad in
landscape): tablets are a target for later, so nothing in `cairn-core` or
`cairn-forms` may be desktop-only.

Resolved versions (2026-10-31): egui, eframe, egui_extras and egui_kittest
0.37.0, egui_form 0.11.0, syn 2.0.119, quote 1.0.47, proc-macro2 1.0.107.
