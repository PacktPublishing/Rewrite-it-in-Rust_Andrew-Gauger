# Chapter 10: Make It Feel Like a Datasheet

The workspace this chapter builds is in [`crate/`](crate/), as it stood at
the end of the chapter: everything from Chapters 8 and 9, plus
`company-by-hand` (V1's Company Info form in plain egui), the form kit
`cairn-forms` and its derive macro `cairn-forms-derive`, and `cairn-app`,
the desktop app with every V1 form. Later chapters grow the workspace in
[`../part 2/crate/`](../part%202/crate/); this copy stays as Chapter 10
printed it.

| Folder | What it is |
|---|---|
| `crate/` | The Cairn V2 workspace as of this chapter. Its README says how to run it |
| `outputs/` | Captured output quoted in the chapter. Its README says how each was made |

Every save in this chapter stays in the app's window: nothing is written
back to Postgres until Chapter 11. The screens show placeholder rows from
`dev-ipsum.sh`, not Cairn's data.

The tests the chapter prints need only Rust, no database:

```sh
cd crate
cargo test -p cairn-app --test keyboard   # the keyboard section's tests
cargo test -p company-by-hand             # the hand-built form's costs
cargo run -p company-by-hand              # the first screen, no database
```

The app itself, and the scroll timing, need PostgreSQL 17:

```sh
cd crate
sh dev-db.sh
export DATABASE_URL=postgres://cairn_owner:cairn_owner_dev@127.0.0.1:5433/cairn
cargo run -p migration -- up
sh dev-ipsum.sh
cargo run -p cairn-app
cargo test --release -p cairn-app --test scroll_timing -- --ignored --nocapture
```

`cargo expand` (`cargo install cargo-expand`) reproduces
`outputs/cargo-expand-company.txt`:

```sh
cargo expand -p cairn-app --lib forms::company
```

The project in the chapter's second section starts from the Part II
battery pack; see [`../riir-battery-pack/`](../riir-battery-pack/).
