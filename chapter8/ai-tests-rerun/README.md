# ai-tests-rerun: Chapter 7's AI tests, run against cairn-core

A copy of `chapter7/ai-tests/crate-stubbed/` (the model's 173 compiling
tests with `todo!()` stubs). Chapter 7's two crates stay as that chapter
quotes them. Here, every assumed function that has a matching rule in
`part 2/crate/cairn-core` is wired to it.

```sh
cd chapter8/ai-tests-rerun
cargo test --no-fail-fast      # needs mdb-export on PATH (u015, u031)
```

Output: `chapter8/outputs/ai-tests-rerun.txt` (warnings and blank lines
stripped) and `ai-tests-rerun-raw.txt` (everything). Run 2026-10-30,
rustc 1.99.0.

## The rule for the adapters

An adapter converts types and calls `cairn-core`. It never decides a rule.
Where the model assumed a database lookup (V1's `DLookup`/`DSum`), the
adapter reads the History database with `mdb-export` (`tests/common/adapt.rs`),
the same way Chapter 7 already did for `u031`. Where the model's input
can't be built as a `cairn-core` value, the adapter panics with "not
representable in cairn-core" so that failure is counted apart from wrong
answers. Every edited place is marked "Wired to cairn-core for Chapter 8".

Wired (7 files): `u015` receipt paid-to-date, `u020` open quote value,
`u021` days in service and routing, `u027` the four customer-pick defaults,
`u028` `valid_order_status`, `u036` `commission`, `u038` `line_total` (the
model's own `unimplemented!()` body, replaced). Everything else stays a
stub: forms, tables, numbering and printing aren't in `cairn-core`, which
has no database and no user interface.

Two other edits: the database path in `u031` (one directory shallower
here), and the package name.

## Results

| Run | Pass | Fail |
|---|---|---|
| Chapter 7, stubs plus `u031` on real data (`crate-stubbed`) | 10 | 163 |
| The same copy here before wiring (reproduces it) | 10 | 163 |
| **Wired to cairn-core** | **46** | **127** |

Of the 46 passing:

- **36 run against cairn-core**: u021 13, u027 8, u020 6, u015 5,
  u038 2, u036 1, u028 1.
- 5 are `u013`'s tautological tests, which pass whatever the code does.
- 5 are `u031`'s, which read the customer table and test no code.

Of the 127 failing:

| Why | Tests |
|---|---|
| `todo!()`: no rule in cairn-core (forms, tables, numbering, printing) | 110 |
| The model's own `unimplemented!()`, not wired (u037's payment rollup, u038's description, cost and join) | 12 |
| Not representable in cairn-core | 2 |
| Wrong answer | 3 |

- **Not representable (2).** `u027` `default_rep_is_none_when_customer_has_no_assigned_rep`:
  `cairn-core`'s `CustomerRow` requires a rep, as V1's relationships do
  (R43). `u015` `nonexistent_payment_returns_zero`: `paid_to_date` takes a
  payment, so a payment that doesn't exist can't be asked about.
- **Wrong answers (3).** Two are Chapter 7's known wrong tests in `u031`
  (`'Prospect'` with quote marks, Net 60), still failing on real data.
  The third is new. See below.

## A wrong test Chapter 7 graded right

`u021` `doa_after_30_days_still_advance_replacement` asserts that a
dead-on-arrival return at 31 days goes to Advance Replacement. `cairn-core`
says Manufacturer Repair:

```
assertion `left == right` failed
  left: "Manufacturer Repair"
 right: "Advance Replacement"
```

`cairn-core` is right. In `qryRMARouting`, branch 4 (`[DaysInService]<=365
Or r.ExtendedWarranty` → Manufacturer Repair) comes before branch 5 (`Dead
on Arrival` → Advance Replacement), so a DOA return from 31 to 365 days is
repaired. The model's comment ("Missed b2 (days>30), caught by b5") skips
branch 4. The History database has 2 such returns, and Access routes both
to Manufacturer Repair (`route_v1` matches Access on all 772). The test's
other two assertions (90 days: wrong for the same reason; 731 days:
right) never run.

`chapter7/ai-tests/grading.md` row 77 grades it **right** ("Branch order
matches the IIf chain"). With it, Chapter 7's grade is 168 right, 5 wrong,
8 tautological, not 169 / 4 / 8. Chapter 7 is with the publisher; this is
recorded here and in `chapter8/TODO.md`, and neither Chapter 7's prose nor
its grading table has been changed.

## Rules accounted for

Chapter 7 defines a rule as accounted for when it has a sentence, a status,
and a test that runs against the new code. Counting tests in `cairn-core`
and the wired AI tests above (the AI's passing tests add no rule
`cairn-core`'s own tests don't already cover):

**22 of 52**: R01, R03, R04, R05, R07, R08, R10, R11, R13, R18, R19, R21,
R22, R23, R24, R27, R28, R33, R34, R35, R37, R38.

The other 30: 18 rules in the code that belong to the database, forms or
reports (R02, R06, R09, R12, R14, R15, R16, R17, R20, R25, R26, R29, R30,
R31, R32, R36, R39, R42), the four the schema enforces (R43 to R46), the two
sign-in rules deleted rather than ported (R40, R41), and the six only people
hold (P01 to P06).
