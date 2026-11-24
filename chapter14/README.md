# Chapter 14: Printing an Invoice Without Drawing It

The workspace this chapter builds is in [`crate/`](crate/), as it stood at
the end of the chapter (tag `ch14-reports`): everything from Chapter 13,
plus five reports (quote, receipt, invoice, commission statements, RMA
summary) and six month-end lists, each as a PDF or a CSV. A report is a
typed view, a `handlebars` template read from disk, and WeasyPrint run as a
subprocess on a worker of its own. Later chapters grow the workspace in
[`../part 2/crate/`](../part%202/crate/); this copy stays as Chapter 14
printed it.

| Folder | What it is |
|---|---|
| `crate/` | The Cairn V2 workspace as of this chapter. Its README says how to run it |
| `outputs/` | Captured output and the PDFs quoted in the chapter. Its README says how each was made |

WeasyPrint is not a Rust crate. Install it first (`brew install weasyprint`;
70.0 here). The report tests also read the PDFs back with poppler's
`pdfinfo` and `pdftotext`.

The tests that need no database:

```sh
cd crate
cargo test -p cairn-app --test reports -- --nocapture --test-threads=1
cargo test --workspace
```

The tests that go through the real worker and Postgres are ignored by
default. They need PostgreSQL 17 and, for the last one, the `dev-ipsum.sh`
rows:

```sh
cd crate
sh dev-db.sh
export DATABASE_URL=postgres://cairn_owner:cairn_owner_dev@127.0.0.1:5433/cairn
cargo run -p migration -- up
sh dev-seed.sh yourname.onmicrosoft.com
sh dev-ipsum.sh
cargo test --release -p cairn-app --test report_pipeline -- --ignored --nocapture --test-threads=1
```

To try the buttons, run the window from `crate/`
(`CAIRN_WEASYPRINT=$(which weasyprint) cargo run -p cairn-app`). The
Month-end column on the menu has a month picker, the six lists with a CSV
button each, Commission Statements and RMA Summary. Print Quote, Print
Receipt and Print Invoice are on the quote and order screens, and work on a
saved record. The templates are in `crate/cairn-app/templates/`: change a
column width in `report.css` or swap `logo.svg` and print again, with no
rebuild.

The screens show placeholder rows, not Cairn's data.
