# What was wrong with run.py

*Written 2026-10-24 from the run that started 2026-10-21 14:15 (Qwen3.8-27B-4bit,
`outputs/transcript.jsonl`, `logs-run-console.txt`, `logs/server.log`). That run was
still going when this was written, so `run.py` is unchanged. Everything below is
fixed in `run-rs/` unless it says otherwise.*

Each item gives what went wrong, the evidence from the run, what it cost the
chapter, and the fix in `run-rs/`.

## 1. The sql tool returned blank results without saying why

**What happened.** `tool_sql` runs `mdb-sql` and returns its stdout. `_run`
(`run.py:92`) reports an error only on a non-zero exit, and `mdb-sql` exits 0
whatever happens. Three ways this went wrong, all measured on the Cairn file:

- **Syntax mdb-sql can't parse** (`ORDER BY`, `GROUP BY`, `DISTINCT`, `AS`
  aliases, joins, `IN`, `SELECT 1`). It prints `syntax error` / `Got no result`
  on stderr and a single blank line on stdout. The model saw `"\n"`.
- **`IS NULL` / `IS NOT NULL`.** It prints `Calling mdb_test_sarg on unknown
  operator` on stderr and returns the header with **no rows**, which looks like
  a real empty result and is wrong.
- **`COUNT(*)` with a `WHERE` clause never returns.** `_run` has no timeout, so
  one such query would have hung the run indefinitely. This run never sent one
  in a form mdb-sql could parse (the model kept adding `AS n`), so it was lucky.

The tool's description says none of this, and rows come back in table order
with a 20-row cap, so "the latest number" queries returned the first 20 rows.

**Evidence.** 1,159 of 1,879 sql calls (62%) came back blank. In round 2 alone
the model spent nine turns on `NextNumber` working out why. Notes 033 and 041
noticed the tool misbehaving and said so.

**Cost.** Five false claims about the data in the notes:

- RMAs is "currently empty" (005). It has 772 rows.
- ManufacturerOrders is "currently empty" (012). It has 9,054.
- No order has two payments on the same day (015). Six order/date pairs do.
- The quote and order status lists are closed (027, 028). Back Order (31),
  On Hold (12) and `'Draft'` (9) exist.
- The 2026 maximums are Q-1699 and SO-0999 (001). The real ones are
  Q-2026-1713 and SO-2026-1226. This is likely the 20-row cap combined with
  no `ORDER BY`.

**Fix.** `tools.rs`:

- `capture()` collects stderr and enforces a 30-second timeout.
- `sql_result()` turns a parse failure into an error that lists what mdb-sql
  supports, and refuses to show rows from a partly evaluated `WHERE`.
- Zero rows print as `(0 rows)`, and a cut result says how many rows matched
  in all.
- `COUNT(*)` with `WHERE` is refused before it runs.
- The tool's description now states the dialect. That is the one place where
  `run-rs` deliberately differs from `run.py`'s tool list.

## 2. Replies cut off at max_tokens looked like refusals

**What happened.** With thinking on, the model often spent its whole 4,096-token
reply reasoning and never reached a tool call. The server returned
`finish_reason: "length"`, empty content and no tool calls. `run.py` treated
that as "no tool call" and sent the generic nudge. The server drops reasoning
from the next prompt: the prompt grew by 25 tokens between attempts. So the
model started its reasoning over, hit the limit again, and after four replies
the unit was skipped.

**Evidence.** Every reply in these rounds had empty content and 11,000–20,000
characters of reasoning. The transcript doesn't log `finish_reason`; the
pattern above is the evidence.

**Cost.** Eight units skipped with "no tool calls":

- `qryQualifiedLeads` (notes). This is one of the chapter's three named rules.
- `QuoteToOrder`, `LoginCheck`, `DocBeforeInsert`, `DocCustomerChanged`,
  `qryQuoteTotals`, `qryOrderBalances` and `qryOpenReceivables` (tests).

**Fix.** `main.rs`:

- A reply with `finish_reason == "length"` and no tool call gets a nudge that
  says it was cut off, hands back the last 1,200 characters of its reasoning,
  and asks for a short reply and a save.
- The next request may use the room the round has left, up to twice
  `--max-tokens`. Prompt and reply together still stay within
  `--context-budget`.
- The transcript records `finish_reason` and `completion_tokens` for every
  reply.

## 3. Reasoning was counted as context

**What happened.** After each reply `run.py` set the round's measured size to
prompt tokens plus completion tokens. The completion includes the reasoning,
which the server never sends back. A long think made the round look nearly
full, which brought on the "save now" warning early and the budget-overflow
skip.

**Evidence.** `qryOrderTotals` overflowed twice (rounds 20 and 21) on turns
whose replies were 8,987 and 13,909 characters of reasoning, with measured
prompts of only about 9,000–9,500 tokens.

**Fix.** The measured size is the prompt the server reports. What carries
forward is the reply's content and tool calls, counted by characters. The
reasoning is taken out of the message history so the count matches what is
sent, whatever the backend does with it.

## 4. Over-long notes looped until the round ran out of room

**What happened.** `save_notes` refuses more than 2,000 characters with
"Tighten them." The model can't count characters. Three times it sent
identical over-long text back seven times in a row (2,031, 2,115 and 2,160
characters), each with a short reasoning, until the round overflowed.

**Cost.** `SendManufacturerOrder` (round 13) and `qryFollowUpsDue` (rounds 28
and 29) were skipped as "out of context". Their last turns are identical
`save_notes` calls answered by the same 71-character error. `table:RMAs`
(round 42) sent three different over-long versions before it overflowed, so
there the clearer error helps but the cut-on-repeat doesn't come into play.

**Fix.** The error says how many characters to cut and warns that sending the
same text again saves only what fits. An identical second try is cut at a line
or sentence end and saved with a `[cut by the harness: N of M characters kept]`
marker, and the reply says so.

## 5. A redundant `unit` argument was refused 48 times

**What happened.** The model often called `save_notes(unit=..., text=...)`.
`unit` isn't a parameter, so Python raised `unexpected keyword argument 'unit'`
and nothing was saved.

**Fix.** On `save_notes` and `save_tests`, a `unit` argument naming the current
unit is accepted and dropped. A different unit gets an error naming the
round's unit. The tool list is unchanged.

## 6. Saved-query SQL was lossy, and the tool didn't say so

**What happened.** `mdb-queries` rebuilds a saved query from its definition and
drops join types, `ON` conditions, `GROUP BY`, `HAVING` and `AS` aliases.
`qryOrderBalances` is a `LEFT JOIN ... ON t.OrderID = p.OrderID` in
`CairnBuild`, but it comes out as `FROM [qryOrderTotals],[qryOrderPayments]`.

**Cost.** Note 019 records a quirk that doesn't exist: "an order with no
payment rows is dropped... never appears in qryOpenReceivables." In other notes
the model did compare against the builder code and called the builder
canonical.

**Fix.** Every saved query's text, in the brief and from `get_saved_query`,
ends with an SQL comment that says what mdbtools leaves out.

## 7. A run that dies loses its walk and metadata

**What happened.** `walk.md` and `run-metadata.md` are written only after the
loop ends, and there is no resume. A kill, crash or power loss leaves neither,
and `walk.md` is where H5's "never reached" count comes from.

**Fix (partial).** Both files are rewritten between rounds, with `Finished:
False (still running...)` until the end. **Resume is not implemented**: a dead
run still starts over.

## Not fixed: design questions for the author

- **Coverage.** The walk only visits references the model chooses to follow.
  The queue emptied after 47 of 187 units. That's H5 material if it's on
  purpose. If not, add a sweep of unreached units after the queue empties.
- **Thinking mode isn't recorded.** SPEC §6 asks to pick one and record it.
  The run used the model's default (on) and the metadata doesn't say so.
- **`--prompt-cache-size 1`** makes every turn reprocess its whole prompt
  (8.58M prompt tokens over 1,568 calls). That's a server flag, not harness
  code.
- **`--max-turns 3000` may stop the tests phase.** 1,568 turns took the run to
  unit 22 of the 41 test rounds.

## Parity

`run-rs` no longer matches `run.py` byte for byte. The differences:

- the sql tool's description
- several tool messages
- two extra transcript fields
- reasoning left out of the message history

On the scripted walk (`fake_server.py`), on 2026-10-24, both harnesses still
wrote identical notes, test files, `walk.md` and metadata. That model never
triggers any of the paths above. Whichever harness produces the run the
chapter prints, name it: the two show the model different things.
