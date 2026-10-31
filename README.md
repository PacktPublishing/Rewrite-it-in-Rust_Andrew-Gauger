# Rewrite It in Rust

This is the code repository for *Rewrite It in Rust*, by Andrew Gauger, published by Packt.

The book follows one fictional company, Cairn Systems, through three rewrites of three legacy systems. Part I moves the math behind a fleet-tracking map out of JavaScript and into a Rust core compiled to WebAssembly. Part II retires a ten-year-old Microsoft Access database, starting by recovering its business rules and modeling them in Rust.

Every benchmark, test result, and compiler error printed in the book came from the code in this repository. Where a folder holds captured output (`*-output.txt`, `outputs/`, `bench-output.json`), it is the verbatim output of the run the chapter quotes.

## Chapters

| Chapter | Title | Code |
|---|---|---|
| 1 | Inventory Legacy Systems | [No code](chapter1/) |
| 2 | Choosing a Strategy, Finding the Bottleneck | [chapter2](chapter2/): the four-language interpreter-lock benchmark, plus an [ARM-optimized Rust build](chapter2/gil-demo/README.md#arm-optimization) |
| 3 | Mounting a Rust Core Compiled to WebAssembly | [chapter3](chapter3/): the Rails site before and after `viz-core`, plus the browser benchmark |
| 4 | From Bounding Box to Density Grid | [chapter4](chapter4/): Web Mercator projection and kernel density estimation in `viz-core` |
| 5 | Painting the Heatmap Onto a Moving Map | [chapter5](chapter5/): painting the canvas from Rust with `web-sys`, plus the paint benchmark |
| 6 | The File Nobody Could Touch | [chapter6](chapter6/): the naive "just export it" run against PostgreSQL |
| 7 | Reading the Ruins | [chapter7](chapter7/): rule inventory, interview data, and the AI test-drafting harness |
| 8 | Modeling the Rules in Rust | [chapter8](chapter8/): the `cairn-core` crate as of this chapter, in [`chapter8/crate`](chapter8/crate/) |
| 9 | Building the Replacement: Strategy, Identity, and Schema | [chapter9](chapter9/): the workspace as of this chapter, with the `migration` and `cairn-auth` crates, in [`chapter9/crate`](chapter9/crate/), and the test Entra tenant |

## The Part II database

Chapters 6 to 8 all work against the same Access database, kept once in [`part 2/`](part%202/) at the root of this repository. Scripts in those chapters find it by relative path, so keep the folder name as it is, space included.

- `CairnSystems1.accdb`: the working database with sample data.
- `CairnSystems_History.accdb`: the same database filled with ten years of fictional history. This is the copy the chapters measure.
- `CairnBuild.bas`, `CairnForms.bas`, `CairnReports.bas`, `CairnHistory.bas`: the VBA that generates both databases from nothing.
- `MockManufacturer/`: a stand-in for the manufacturer's order API.
- `crate/`: the Cairn V2 workspace: `cairn-core`, the business rules (Chapter 8), and `migration` and `cairn-auth`, the schema and sign-in (Chapter 9).

Everyone in the database, and the company itself, is fictional. The sample passwords in `part 2/README.md` are deliberately weak, because Chapter 8 deletes that table.

## Software and hardware

| Chapters | Software | Notes |
|---|---|---|
| 2 to 8 | Rust 1.95 or later (`rustup`, `cargo`) | Captured output was produced with rustc 1.99.0 |
| 3 to 5 | `wasm-pack`, the `wasm32-unknown-unknown` target | `rustup target add wasm32-unknown-unknown` |
| 3 to 5 | Ruby 3.4.1, Rails 8.0 | For the Cairn Systems website |
| 3, 5 | Node.js and Playwright | For the browser benchmarks |
| 2 | Python 3.14, Ruby 3.4, Node.js 23 | For the interpreter-lock comparison |
| 6 to 8 | `mdbtools` (`mdb-export`) | Reads the Access database on macOS and Linux |
| 6, 9 | PostgreSQL (17.11 for Chapter 9) | `initdb`, `pg_ctl`, and `psql` on your `PATH` |
| 6 to 8 | Microsoft Access 2010 or later on Windows | Only to open the database interactively |
| 7 | Python 3, plus an Apple Silicon Mac for a local model run | Only to re-run the AI harness itself |
| 9 | Azure CLI (`az`) 2.91 or later | `brew install azure-cli`. Only to set up the test Entra tenant; see [`chapter9/cairn-auth`](chapter9/cairn-auth/) |

Run a chapter's commands from inside that chapter's folder unless its README says otherwise.

## License

The code in this repository is available under the [MIT License](LICENSE).
