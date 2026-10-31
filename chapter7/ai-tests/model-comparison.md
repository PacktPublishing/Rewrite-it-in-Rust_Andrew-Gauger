# The drafting model and the re-evaluating model

H3 names both models. A small open model running locally drafted the test
suite. A frontier model then re-evaluated its output against the
database. This file holds the numbers for that comparison and what makes
it unfair, so the prose doesn't overclaim.

## The two runs

| | Drafting | Re-evaluating |
|---|---|---|
| Model | Qwen3.8-27B, 4-bit (`mlx-community/Qwen3.8-27B-4bit`, revision `10c35caa`) | Claude Opus 5.5 (`claude-opus-5-5`) |
| Where | Locally on an Apple M3 Pro with 36 GB, through `mlx_lm.server` | Claude Code, in the author's terminal |
| Harness | `run.py`: a breadth-first walk, one unit per round. `run-rs/` is its canonical Rust successor and fixes the bugs this run hit | An interactive session, steered by the author |
| Context | 16,384 tokens per round (largest prompt 12,240) | The whole session, summarized twice when it filled |
| Tools | `sql` over `mdb-sql`, which returned blank for 1,477 of 2,444 calls | `mdb-export` to CSV, Python and the shell. Every query worked |
| Time | 344,909 s = **95.8 hours**, unattended (2026-10-21 to 2026-10-25) | About **1.7 hours** of model working time *(see below)*, spread over a day |
| Calls | 1,972 model calls, 3,040 tool calls | not counted |
| Tokens | 10.45M prompt, 1.42M completion | not counted |
| Output | 41 notes, 24 test files, 181 tests. Reached 46 of 187 units | `grading.md` (181 tests graded), `rules.md` (52 rules, 10 defects), `notes-audit.md` (71 claims), `interview-data/` |

**How the 1.7 hours was measured.** From the session transcript, I added
up the gaps between consecutive events from the review request
(2026-10-25 21:17 UTC) to the cast and reader decision (2026-10-26 22:34
UTC). Any gap over 5 minutes counts as 5 minutes, so time waiting for the
author doesn't count. With a 3-minute cap the total is 1.2 hours, and with
a 10-minute cap it's 2.4 hours. The 10-minute research subagent ran in
parallel and isn't included. Reproduce with the snippet at the end.

## What the re-evaluation found

- **The tests are mostly right.** 169 of 181 are right, 4 wrong and 8
  tautological (`grading.md`).
- **The suite is incomplete.** It reaches 28 of 52 rules: 23 of the 29
  routine ones, 5 of the 17 that need a decision, and 0 of the 6 that
  only people hold (`rules.md`).
- **Its beliefs about the data:** 54 of 71 claims true, 12 false and 5
  incomplete. Every false one follows a blank tool result
  (`notes-audit.md`).
- **One rule neither run's notes put together.** Notes 017 and 023 each
  state one commission formula correctly. Comparing them shows the two
  printed figures differ in 397 of 818 rep-months (R24). The drafting
  harness showed the model one unit at a time, so it could never set the
  two side by side.

## Why the comparison isn't fair, and what to say about it

- **The re-evaluator had working tools.** The drafting model's SQL tool
  returned blank results that looked like empty tables. That was the
  harness's fault (`run-py-issues.md` §1), not the model's. Most of its 12
  false claims trace to it.
- **The re-evaluator had the answer key.** `part 2/README.md` ("Built this
  way on purpose") and `MISSION.md` list the designed defects. The
  drafting model was never shown them. `rules.md` was built from the code
  first and checked against the key, but the key was in hand.
- **The re-evaluator had a draft.** It started from 41 notes and 181
  tests, not from a blank page. That's the chapter's thesis applied to the
  frontier model too: reacting to an incomplete draft is faster than
  writing from nothing.
- **The re-evaluator made a mistake of its own.** It first called the
  `u031` Net 60 test wrong "because Priya approves Net 60 by hand". The
  form's own list (`VL_TERMS`) offers Net 60, so the test is wrong against
  the code. The author caught the framing, and `grading.md` was corrected.
  A frontier model re-evaluating is still a draft to check.
- **A person steered it.** The author chose what to do next at each step:
  keep the run, change the thesis, build the rule list. The drafting run
  had no one in the loop for 95.8 hours.

**The honest sentence for H3** is something like: a 27-billion-parameter
model on a laptop drafted the suite unattended in four days, and a
frontier model, given the draft, working tools and the answer key,
checked it in under two hours of its own time. The useful part is the
pairing: cheap and slow to draw the shape, expensive and fast to
correct it. Neither replaces the interviews.

## Harness concepts the reader needs (for H3)

Each one shows up in this run's numbers:

| Concept | In this run |
|---|---|
| Context budget | 16,384 tokens per round. 21 of 103 rounds ran out of context. |
| Tool fidelity | `mdb-sql` exits 0 on errors and doesn't support aggregates, so a broken query looked like an empty table. |
| Walk order | Breadth first, one unit per round: the model never saw two units together, so it missed R24. 141 units were never reached. |
| Output contract | "Assume a function exists, don't implement it." 22 of 24 files fail to compile (E0425 and similar), by design. |
| Stopping and retry rules | A reply cut off at `max_tokens` was treated as "no tool call". 15 of 41 test rounds were skipped as "no tool call". |
| Checking the output | Every claim and test checked against the data: `notes_audit.py`, `grading_table.py`. |

## Reproducing the active-time figure

```python
# Run from ~/.claude/projects/-Users-andy-Users-andy-rewrite-it-rust/
import json
from datetime import datetime
f = "93d4ea7a-5566-4d27-a529-53c92db6de8c.jsonl"
ts = sorted(datetime.fromisoformat(d["timestamp"].replace("Z", "+00:00"))
            for d in map(json.loads, open(f)) if d.get("timestamp"))
lo = datetime.fromisoformat("2026-10-25T21:17:00+00:00")
hi = datetime.fromisoformat("2026-10-26T22:34:00+00:00")
ts = [t for t in ts if lo <= t <= hi]
print(sum(min((b - a).total_seconds(), 300) for a, b in zip(ts, ts[1:])) / 3600)
```
