# Chapter 7 AI Test-Suite Run: Local Model Spec

*Spec only. Nothing here has been run. Target machine: Apple M3, 36 GB unified memory.*

The machine this repo was last worked on is an Intel MacBook Pro with 16 GB
(`MacBookPro16,2`). It cannot run this. Do the run on the M3 and copy
`chapter7/ai-tests/` back, or clone the repo there.

## Goal

Give a model the Cairn business logic with no hints, ask for a Rust test suite,
and save everything needed for the chapter to show a real run: prompt, inputs,
model identity, raw output, resulting crate, verbatim `cargo` output. The
grading against the oracle is a separate step (`TODO.md`, stage 3).

## 1. Model

Everything here is from secondary guides, not checked on Hugging Face. Confirm
repo names and file sizes on the model page before downloading.

| Role | Candidate | Approx. size | Why |
|------|-----------|--------------|-----|
| Primary | `mlx-community/Qwen3.8-27B-4bit` | ~15 GB | Newest 27B dense Qwen with a plain MLX 4-bit build |
| Variant | `mlx-community/Qwen3.8-27B-OptiQ-4bit` | ~19 GB | Mixed precision, some layers at 8-bit; better quality, more memory |
| Alternate | Qwen 3.6-35B-A3B MoE, MLX 4-bit | ~20 GB | Faster per token, older family |
| Not recommended | Devstral Small | n/a | Only the 2505 (May 2025) builds turned up on mlx-community; a newer one is unconfirmed |
| Avoid | Qwen3-Coder-Next | ~50 GB | Needs 64 GB+ |

Qwen3.8 builds are an image-text model family, so `mlx-lm` may need
`mlx-vlm` instead; the model page's usage example says which.

Pick one and name it in the chapter by its exact Hugging Face repo id and
revision hash. Budget: weights 17-20 GB, KV cache for a 32k context a few GB,
macOS and the browser the rest. Close other apps. Don't go past a 32k context
without checking `Activity Monitor` for swap.

## 2. Runtime and download

Use MLX (`mlx-lm`). It is the fastest path on Apple Silicon and pulls straight
from Hugging Face.

```sh
python3 -m venv ~/venvs/mlx && source ~/venvs/mlx/bin/activate
pip install -U mlx-lm "huggingface_hub[cli]"
hf download <org>/<repo-id> --local-dir ~/models/<name>   # note the revision it prints
```

Pin the versions (`pip freeze | grep -E "mlx|huggingface"`) into
`ai-tests/run-metadata.md`. Ollama is already installed on the Intel box but
the MLX route needs no daemon and logs token counts and speed directly.
(Fallback: `ollama pull hf.co/<org>/<repo>-GGUF:Q4_K_M`.)

## 3. Pointing it at the database

A local model can't open an `.accdb`. Extract text from it first, offline, with
mdbtools. That is also what the chapter should say the AI was given.

```sh
mkdir -p chapter7/ai-tests/inputs && cd chapter7/ai-tests/inputs
P="../../../part 2"
mdb-schema "$P/CairnSystems_History.accdb" postgres > schema.sql
for q in $(mdb-queries -1 "$P/CairnSystems_History.accdb"); do
  printf -- "-- %s\n%s\n\n" "$q" "$(mdb-queries "$P/CairnSystems_History.accdb" "$q")"
done > saved-queries.sql
cp "$P/CairnBuild.bas" "$P/CairnForms.bas" "$P/CairnReports.bas" .
```

**Decision (2026-10-21): the model reads the Access file and nothing else.**
The chapter's premise is a legacy system with no documentation and no tests.
`run.py`'s tools read `part 2/CairnSystems_History.accdb` directly: tables,
schema, saved queries and data through mdbtools, and the VBA decoded from the
file's own `MSysAccessStorage` table by `access_vba.py` (mdb-export plus
oletools' MS-OVBA decoder, no Access or Windows needed). That yields
`CairnBuild`, `CairnForms` and `CairnReports`, 2,723 lines, builders included:
in the story the setup code lived inside the file too. No exported `.bas`
file is given to the model. `CairnHistory.bas` isn't in the database, and
`part 2/README.md` (the answer key) is never reachable. Each run saves what
was decoded in `outputs/vba-from-db/`.

Earlier attempts are kept in `runs-failed/` for reference only: one ran out
of GPU memory, one read the exported `.bas` files, one a trimmed copy of them,
and one read the whole system into context before writing anything.

**Decision (2026-10-21): a breadth-first walk under a small, fixed context
budget.** The run has to work on a machine that cannot hold the system in
context, not only on one that can. The read-everything run reached 48,600
tokens before it had written a line. Now each round gets
`--context-budget` tokens (default 16,384) and holds one unit:

0. Survey: the model sees the inventory (names only, about 1,100 tokens),
   writes an overview and names the units to start from.
1. Notes, breadth first: the harness splits the file into 187 units (145 VBA
   procedures and module declarations, 27 saved queries, 15 tables) and
   finds the references between them. Each round shows one unit's source,
   its parent's notes and its references (a typical brief is about 500
   tokens). The model saves one notes file and names the references to
   follow; they join the back of the queue.
2. Tests, in the same order: one Rust test file per unit whose notes found
   rules.

The harness keeps the memory (queue, visited set, saved files). A round that
would overflow the budget is retried; a unit that fails twice is skipped.
`outputs/walk.md` records the order and lists every unit the walk never
reached, which is H5's coverage number. `run-rs/` is a Rust port, checked
against `fake_server.py`.

## 4. Context plan (superseded by the walk above)

The three `.bas` files total about 146 KB, roughly 40k tokens, plus schema and
queries. That's too much for one pass on a local model. Run three passes, each
with the same `schema.sql` + `saved-queries.sql` header:

1. `CairnBuild.bas` (the queries and defaults)
2. `CairnForms.bas` (numbering, quote-to-order, tax, price copy)
3. `CairnReports.bas` (commission period, receipt paid-to-date)

Each pass returns a Rust module of tests. Merge them into one crate. Record the
per-pass prompt and output separately.

## 5. Prompt

Save the exact text as `prompt.md` and print it in the chapter. Draft:

> You are given the Visual Basic for Applications (VBA) code, the schema, and
> the saved queries of a small Microsoft Access sales system. Nobody who wrote
> it is available and there is no documentation. Write a Rust test suite that
> pins down the business rules this code implements. For each test, give a name
> that states the rule, concrete input values, and the expected output you
> derive from the code. Do not invent behavior the code does not show. Assume a
> function with a plausible signature exists for each rule and call it; do not
> implement it. Output one `.rs` file in a single code block.

Do not describe the rules in the prompt. The chapter's point is the model draws
the shape of the migration on its own.

## 6. Settings

- Temperature 0, fixed seed, so the run can be repeated.
- Max output tokens high enough not to truncate (8k per pass). If a pass is cut
  off, say so in the chapter; do not splice.
- Disable thinking mode if the model has one, or keep it and save the trace.
  Pick one and record it.
- Run each pass once for the chapter. Extra runs go in `runs-extra/`.

```sh
mlx_lm.generate --model ~/models/<name> --temp 0 --seed 7 --max-tokens 8192 \
  --prompt "$(cat pass-1-prompt.txt)" | tee outputs/pass-1-raw.txt
```

(Check `mlx_lm.generate --help` for the current flag names; they have changed
between releases.)

## 7. Files to save (all under `chapter7/ai-tests/`)

```
SPEC.md                  this file
prompt.md                exact prompt
inputs/                  schema.sql, saved-queries.sql, the three .bas files
outputs/pass-N-raw.txt   raw model output, untouched
run-metadata.md          repo id, revision hash, quantization, mlx-lm version,
                         macOS version, chip, memory, date, settings, tokens/s
crate/                   merged Rust crate, tests as the model wrote them
logs/cargo-build.txt     verbatim
logs/cargo-test.txt      verbatim
```

## 8. Build the crate

Tests call functions the model assumed exist, so `cargo build` will fail.
That is expected and is itself chapter material. Don't fix the model's code to
make it compile. If the crate can't compile at all, add the minimum stub module
that declares the assumed functions with `todo!()` bodies so the tests run, and
record exactly what was added. Show the verbatim `cargo test` output, panics
included.

## 9. Then

Grade each test as right, wrong or unverifiable against the History data and
`chapter6/export-demo/rounding.py`, with a reason per wrong test. Those counts
are H3's and H5's numbers. The "almost none are right" claim has to come out of
that table. A local 27B model may do better or worse than the scaffold
expects, and the chapter should follow what the table says.

## Open decisions

- ~~Which model and quantization to name in the book.~~ Settled 2026-10-26: Qwen3.8-27B, 4-bit (`model-comparison.md`).
- Whether the prompt is printed in full in the chapter or only excerpted.
- ~~Whether `ai-tests/` is referenced as companion code.~~ Settled
  2026-10-26: the companion code is
  https://github.com/PacktPublishing/Rewrite-it-in-Rust_Andrew-Gauger, and
  `run-rs/` is the canonical harness.

Sources for the model sizes: [InsiderLLM, best local coding models](https://insiderllm.com/guides/best-local-coding-models-2026/), [willitrunai, M3 Pro 36GB](https://willitrunai.com/macs/m3-pro-36gb), [DEV Community, local LLMs on Apple Silicon](https://dev.to/brunocerberus/running-local-llms-on-apple-silicon-2ecm).
