# Chapter 8 captured output

Everything here is pasted verbatim from a real run on 2026-10-30 (rustc
1.99.0, macOS). The crate is `part 2/crate/cairn-core`. Paste from these
files. Never retype them.

| File | What it is | How it was made |
|---|---|---|
| `cargo-test.txt` | 69 tests, all passing | `cargo test` in `part 2/crate` |
| `cargo-clippy.txt` | Clean | `cargo clippy --all-targets -- -D warnings` |
| `e0308-option-compare.txt` | H4: V1's comparison written against `Option<i32>` | A copy of the crate with `route`'s DOA branch written as `if r.days_in_service <= 30 {` |
| `e0004-non-exhaustive.txt` | H4: the new route with no display text | A copy of the crate with the `Route::UnknownOrder` arm removed from `impl Display for Route` |
| `half-up-fails.txt` | H4: exact decimal, half-up, against V1's figures | A copy of the crate whose `access_round_product` multiplies in `Decimal` and rounds `MidpointAwayFromZero`; `cargo test --test september_2026` |
| `v1-quote-mark-manufacturerorders.csv` | H3: **the real thing.** Access's `ManufacturerOrders` rows after Send to Manufacturer for `Harlow "Old Mill" Manufacturing` (two attempts, both HTTP 400, identical 243-character RequestBody) | `mdb-export` of `chapter8/windows-capture/CairnSystems1_quote-mark.accdb` (run on Windows 2026-10-30; see that folder's `notes.txt`) |
| `v1-pasted-json.txt` | H3: V1's string-built request breaking on a quote mark, then `cairn-core`'s | `cargo run -q --example v1_pasted_json`. **Staged**: no customer in the file has a `"` in its name, so the name is made up. Say so in the prose. The correlation ID is random on every run |

Line numbers in the two compiler errors are the modified copies', which
match the real `rma.rs` to within a few lines.

## Found while capturing

- **rustc's suggested fix for E0308 is wrong for this data.** It suggests
  `r.days_in_service <= Some(30)`. That compiles, and Rust orders `None`
  below every `Some`, so a blank counts as "at most 30 days." Applied to
  every comparison, the 23 returns with no order would route 21 Manufacturer
  Repair and 2 Advance Replacement. V1 routes them 18 Customer Pays Repair,
  3 Account Manager and 2 Advance Replacement. Three answers for the same
  blank: V1 says false, the suggestion says true, `UnknownOrder` says
  unknown. Candidate for H4 P2.
- **Half-up fails all three acceptance figures**, with the exact wrong
  values `rounding.py` predicts: $25,701.08 (Carla), $183,280.55 (all of
  September), $877.11 (`SO-2026-1053`).
