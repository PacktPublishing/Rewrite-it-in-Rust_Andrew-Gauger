# Chapter 7: Reading the Ruins

| Folder | What it is |
|---|---|
| `rule-inventory/` | Scans the Access database for every query and routine. `rules.md` is the rule count the chapter tracks |
| `interview-data/` | The numbers brought to each interview, grouped by person |
| `ai-tests/` | The harness that turned a local model loose on the VBA to draft a test suite |
| `stage3-windows/` | Screenshots and results from checking the findings in Access on Windows |

## The AI test run

| Path | What it is |
|---|---|
| `ai-tests/run.py` | The Python harness the quoted run used |
| `ai-tests/run-rs/` | The Rust harness, with the bugs that run exposed fixed |
| `ai-tests/crate/` | The tests exactly as the model wrote them. They don't compile, and the chapter shows why |
| `ai-tests/crate-stubbed/` | The same tests with stubs added so they compile and run |
| `ai-tests/outputs/` | The run's transcript and captured `cargo` output |

```sh
cd ai-tests/run-rs && cargo test
cd ai-tests/crate && cargo build --tests --keep-going      # reproduces outputs/cargo-build.txt
cd ai-tests/crate-stubbed && cargo test --no-fail-fast    # needs mdb-export on PATH
```

Running `run.py` or `run-rs` again starts by clearing `crate/tests/`. Work on a copy if you want to keep the model's original tests.
