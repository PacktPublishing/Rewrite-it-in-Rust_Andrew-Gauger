# Cairn Systems V2

The Rust replacement for the Access database in this folder, built in the
book's chapter order. One member so far:

| Crate | Job | Chapter |
|---|---|---|
| `cairn-core` | Business rules: no user interface, no database, no network | 8 |

Chapters 9–13 add `migration`, `cairn-forms`, `cairn-forms-derive` and
`cairn-app` beside it.

## cairn-core

Built to `../chapter8/workspace/artifacts/mission.md`. Its status enums
started as the earlier agent run's (`../ws34/artifacts/v2/src/`). The rest
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
  `../CairnSystems_History.accdb`. The September files came with the
  mission. The routing files come from `export_rma_fixtures.sh` (needs
  mdbtools).

Resolved versions (2026-10-30, rustc 1.99.0): rust_decimal 1.43.0, serde
1.0.229, serde_json 1.0.154, uuid 1.29.0; csv 1.4.0 (tests only).
