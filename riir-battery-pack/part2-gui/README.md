# riir-gui-battery-pack

Part 2 of the [riir battery pack](../README.md) -- an `egui` desktop GUI
backed by Postgres over `sqlx`, the crate curation behind Part II of
*Rewrite It in Rust*: The Database (invoice, order, and quote entry moved
off an Access frontend onto a Rust desktop app).

## Quick Start

```sh
cargo bp new riir-gui --template gui-core --name my-back-office
```

That makes a one-window eframe app. `gui-core` asks two questions,
both answered no unless you say otherwise:

- **A form kit** (`-d form_kit`): a `forms/` crate like the book's
  `cairn-forms`, cut down to a start: a `Form` trait, `FieldValue` for
  text and numbers, and `RecordForm`, which keeps the record as saved and
  the draft, marks unsaved changes, undoes, and shows an error beside a
  box that doesn't parse. `main.rs` puts a customer record on screen
  through it, with its `Form` impl written by hand.
- **A derive macro** (`-d derive_macro`): a `forms-derive/` crate like
  the book's `cairn-forms-derive`, which writes that `Form` impl from
  `#[field(label = "...")]`. It brings the form kit with it.

```sh
cargo bp new riir-gui --template gui-core --name my-back-office -d form_kit -d derive_macro
```

Add it to an existing crate instead:

```sh
cargo bp add riir-gui
```

HTML templating for invoice generation is opt-in:

```sh
cargo bp add riir-gui -F templating
```

## Why these crates, and not others

`egui` + `eframe` are the required pair: `egui` is the immediate-mode
widget library, `eframe` is the application shell that actually runs it
natively (and, unchanged, compiles the same UI to the web -- not this
book's path, but free). Immediate-mode over a retained-mode toolkit
because the whole UI redraws from the app's own state every frame; there
is no separate widget tree to keep in sync with the database by hand.

`tokio` and `sqlx` (with the `postgres` and `runtime-tokio` + `tls-rustls`
features) are required together: a blocking `sqlx` query issued directly
on the UI thread stalls the whole `egui` frame the same way a blocking
call would stall any other single-threaded event loop. `tokio` gives the
query somewhere else to run while the frame keeps redrawing.

`egui_extras` and `egui_form` (`forms`, on by default) are the rest of
the entry experience. `egui_extras::TableBuilder` builds only the rows in
view, so a datasheet of 13,531 orders scrolls at frame rate, and its cells
can hold text fields for the editable datasheets. `egui_form` wraps any
widget with a label and a validation error, and tracks egui's releases
within days; its `garde` and `validator` adapters stay off, because the
rules are plain Rust functions. No data-grid crate: egui already had the
pieces, and the grids on crates.io lag egui (`ui-grid-egui` 1.0.8 is on
0.35).

`handlebars` (`templating`) is opt-in because it only enters once the
book reaches invoice generation (Chapter 14). `tera` is a reasonable
in-place swap if a project prefers Jinja-style syntax -- not included as
a second default to avoid recommending two templating engines for the
same job.

**Desktop first, tablets later.** Nothing in this pack is desktop-only:
eframe runs on Android, and the book's code keeps platform-specific work
(opening a browser to sign in, opening a PDF) at the app's edge so an
Android or iPad build stays possible.

**Not curated here:** a PDF renderer. The outline's WeasyPrint is a Python
tool invoked as an external process, not a Cargo dependency -- this pack
only curates crates that actually live in `Cargo.toml`.

## License

Licensed under either of:

- [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
- [MIT license](https://opensource.org/license/mit)

at your option.
