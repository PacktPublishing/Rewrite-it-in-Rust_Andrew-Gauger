# crate-stubbed: the AI's tests, made to compile

A copy of `../crate/`, the test crate the drafting model wrote. `../crate/`
stays exactly as the model left it, because the chapter quotes it. This
copy adds only what it takes to compile and run, with every addition
marked in the file as "Added for the book" or "Implemented for the book".

```sh
cd chapter7/ai-tests/crate-stubbed
cargo test --no-fail-fast              # needs mdb-export on PATH for u031
cargo test --test u031_table_customers # the two real failures H4 uses
```

## What was added

- **`todo!()` stubs.** In 20 files, the functions the tests assume are declared at
  the bottom of the file with the signature the tests call, and a
  `todo!()` body. Where the model imported a module that doesn't exist
  (`use crate::cairn_forms::doc_load`), the stub is a local module of that
  name, so the model's `use` line is untouched.
- **`chrono`** as a dev-dependency. Four files import it.
- **A stand-in `decimal` module** in `u037`. The model imported a `decimal`
  crate with a `prelude::PrimitiveDecimal` trait. No such dependency
  exists. `Decimal` is an `f64` alias here.
- **One edited line.** `u029` says `use super::activities::*;`, which can't
  resolve at a test crate's root (E0433). It now says `crate::`.
- **Real bodies for `u031`'s six helpers.** The model wrote them with
  `unimplemented!()` and the SQL it meant. They now read the Customers and
  SalesReps tables through `mdb-export` (set `CAIRN_DB` to use another copy
  of the file). Each one keeps the model's SQL as a comment.

## What couldn't be stubbed

`u033_table_contacts.rs` (8 tests) is in `tests-excluded/`. No signature
makes it compile, because the model used the same value two ways:
`add_contact(...)` returns something that is `.is_none()` and `.expect()`ed
(an `Option`) in some tests, and passed to `get_contact(a)` or added to
(`a + 1`) as a plain integer in others.

## Results (verbatim in `../outputs/`)

| Run | Compiles | Pass | Fail | File |
|---|---|---|---|---|
| The model's crate as written | 2 of 24 files | 0 | 13 | `cargo-build.txt`, `cargo-test.txt` |
| Stubs only | 23 files, 173 tests | 5 | 168 | `cargo-test-stubbed-baseline.txt` |
| Stubs, plus `u031` on real data | 23 files, 173 tests | 10 | 163 | `cargo-test-stubbed.txt` |

- **With stubs only, every failure is a placeholder.** 147 panic on
  `todo!()` ("not yet implemented") and 21 on the model's own
  `unimplemented!()` ("not implemented").
- **The 5 that pass are the 5 tautological tests in `u013`**, which test
  functions the model wrote inside the test file. They pass whatever Access
  does (`../grading.md`).
- **`u031` on real data: 5 pass, 2 fail** (`cargo-test-u031-real-data.txt`).
  The two failures are the wrong tests in `grading.md`, rows 125 and 126,
  failing on the rows `rules.md` predicts:
  - `customer 877 has unexpected customerstatus "'Prospect'"` (Eastside
    Cold Storage, the text-default bug, D01)
  - `customer 194 has unexpected paymentterms "Net 60"` (Oakridge
    Logistics; Net 60 is on the form's own list, R19, P02)
