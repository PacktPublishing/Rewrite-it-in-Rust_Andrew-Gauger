# Chapter 8: Modeling the Rules in Rust

The crate this chapter builds, `cairn-core`, is in [`crate/`](crate/), as it stood at the end of the chapter. Later chapters grow the workspace in [`../part 2/crate/`](../part%202/crate/), next to the database it replaces; this copy stays as Chapter 8 printed it.

```sh
cd crate
cargo test                                      # every test, including tests/september_2026.rs
cargo clippy --all-targets -- -D warnings
cargo run -q --example v1_pasted_json
```

| Folder | What it is |
|---|---|
| `crate/` | `cairn-core` as of this chapter |
| `listings/first-route/` | The first version of the RMA routing rule |
| `ai-tests-rerun/` | Chapter 7's AI-written tests, wired to `cairn-core` |
| `rule-checks/` | Checks of the routing rule against the database |
| `outputs/` | Captured compiler errors and test runs quoted in the chapter. Its README says how each was made |
| `windows-capture/` | The manufacturer-order failure captured in Access on Windows |

`ai-tests-rerun` fails most of its tests on purpose: most of the functions the model assumed are still `todo!()` stubs. `outputs/ai-tests-rerun.txt` is the expected result.
