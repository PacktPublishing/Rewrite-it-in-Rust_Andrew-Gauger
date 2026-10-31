# run-rs: the AI test-suite run, in Rust

Started as a port of `../run.py`. On 2026-10-24 it was fixed for the defects
the first real run exposed. They are listed, with the evidence, in
`../run-py-issues.md`; `run.py` itself was left alone because that run was
still going. Same three commands, same flags, same tools, same briefs, and the
same files written to the same places (`outputs/`, `logs/`, `crate/`,
`run-metadata.md`).

```sh
cd chapter7/ai-tests/run-rs
cargo run --release -- selftest
cargo run --release -- run --backend mlx --server-args "--prompt-cache-size 1"
```

## How the run works

Both versions walk the Access system breadth first under a fixed context
budget per round (`--context-budget`, default 16,384 tokens), so a small
machine can run it:

- `units.rs` splits the database into units (each VBA procedure, each
  module's declarations, each saved query, each table) and finds the
  references between them.
- `walk.rs` holds the queue, the visited set and the records, and builds each
  round's brief: one unit's source, the notes of the unit that led to it, and
  its references. The harness keeps the memory, not the model.
- `tools.rs` has the tools. `save_notes` writes one notes file per unit and
  queues the references the model chose to follow; `save_tests` writes one
  test file per unit with rules, after every unit's notes are done.
- `main.rs` runs the rounds, retries a round that would overflow the budget,
  and writes `outputs/walk.md` (the order, plus every unit never reached).

## What it shares with run.py

- The MLX virtualenv (`~/venvs/mlx`) and model (`~/models/`) that `setup` creates.
- `../access_vba.py`, run with that virtualenv's Python to decode the VBA
  modules out of the database's `MSysAccessStorage` table.
- `../prompt.md`.

## Parity checks

`cargo test` pins what the model sees to `run.py`:

- `tests/fixtures/tool-specs.json`: the tool list, compared byte for byte
  except the sql tool's description, which here says what mdb-sql can't do.
- `tests/fixtures/system-prompt.json`: the system prompt.

Regenerate both after changing either in `run.py`:

```sh
cd chapter7/ai-tests
python3 -c "import json, run; json.dump(run.tool_specs(), open('run-rs/tests/fixtures/tool-specs.json', 'w'), indent=2)"
python3 -c "import json, run; print(json.dumps(run.SYSTEM))" > run-rs/tests/fixtures/system-prompt.json
```

The whole walk is checked against `../fake_server.py`, a scripted model that
needs no GPU and exercises every path (survey, nudges, malformed arguments, a
budget overflow and retry, a skipped test file):

```sh
cd chapter7/ai-tests
python3 fake_server.py --port 8099 &      # restart it between runs
python3 run.py run --no-server --port 8099 --workdir /tmp/walk-py
run-rs/target/release/run run --no-server --port 8099 --workdir /tmp/walk-rs
diff -r /tmp/walk-py/outputs/notes /tmp/walk-rs/outputs/notes
```

On 2026-10-21 the two walks wrote identical notes, test files, `walk.md`
and metadata, and 109 of 110 transcript records matched. The one difference
is the JSON parser's own wording in the malformed-arguments error. After the
2026-10-24 fixes the notes, test files, `walk.md` and metadata are still
identical; the scripted model never reaches the fixed paths. `selftest`
exercises the new sql errors against the real database.

On a machine without the MLX virtualenv (an Intel Mac, say), point `HOME` at a
throwaway one so `access_vba.py` can run, and keep the build out of the synced
folder:

```sh
python3 -m venv /tmp/fakehome/venvs/mlx && /tmp/fakehome/venvs/mlx/bin/pip install oletools
CARGO_TARGET_DIR=/tmp/run-rs-target cargo build --release
HOME=/tmp/fakehome /tmp/run-rs-target/release/run selftest --workdir /tmp/selftest-rs
```

## Fixed here, not in run.py

Details and evidence are in `../run-py-issues.md`.

- **sql tells the truth.** mdb-sql's parse errors, its unevaluated `IS NULL`,
  and a 30-second timeout all come back as errors instead of blank output.
  `COUNT(*)` with `WHERE` (which never returns) is refused. Zero rows print as
  `(0 rows)`, a cut result says how many rows matched, and the tool
  description states the dialect.
- **Cut-off replies.** A reply that hits `max_tokens` before calling a tool
  is told so, gets the end of its reasoning back, and may use up to twice
  `--max-tokens` from the round's remaining room on the next try.
- **Token accounting.** A round's size is the measured prompt plus the
  reply's content and tool calls. Reasoning isn't counted, and isn't sent
  back.
- **Over-long notes.** The error says how much to cut; the same text sent
  twice is cut and saved with a marker.
- **`unit` argument.** Accepted on the savers when it names the current unit.
- **Saved queries.** Each one ends with a comment saying what mdbtools leaves
  out (join types, `ON`, `GROUP BY`, `HAVING`, `AS`).
- **Progress.** `walk.md` and `run-metadata.md` are rewritten between rounds.
  There is still no resume.
- **Transcript.** Each reply records `finish_reason` and `completion_tokens`.

## Where it differs

- **JSON formatting.** The transcript and console use compact JSON; Python
  writes `{"a": 1}` and escapes non-ASCII characters. The content is the same.
- **Error wording.** Malformed tool-call JSON and bad arguments are reported
  with Rust's wording rather than Python's `TypeError` text. Where `run.py`
  would crash on a non-numeric `start` or a non-string argument, the Rust
  version returns an error to the model and keeps going.
- **Regex dialect.** `search_vba` uses the `regex` crate, which has no
  look-around or backreferences.
- **Setup's Python.** `run.py` builds the virtualenv with whichever Python runs
  it. The Rust version uses `--python` (default `python3`).
