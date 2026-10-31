# Notes audit: what the AI believed about the data

Every claim about the data in the AI's 41 notes (`outputs/notes/`),
checked against the History database. Claims about the code (a join type)
are checked against the builder source, `part 2/CairnBuild.bas`, because
`mdb-queries` drops join types from the saved SQL. `grading.md` grades the
tests. This file grades the beliefs the tests were written from.

Reproduce with `python3 -I chapter7/ai-tests/notes_audit.py`. The output
is saved as `notes_audit-output.txt`.

## The finding

**71 claims in 30 notes: 54 true, 12 false, 5 incomplete.** The other 11
notes make no claim about the data: they restate code.

The true claims are the ones the model could read off a few rows: quote
and order totals to the cent, every rate, every product price, the
customers it used as fixtures, the payments on orders 1, 2, 5 and 7. The
false ones are claims about the whole table: that it's empty, that a list
of values is closed, that something never happens.

Every false or incomplete claim follows a blank result from the `sql`
tool (`run-py-issues.md` §1). `mdb-sql` returns nothing for `COUNT`,
`MAX`, `DISTINCT` and `GROUP BY`, and the harness passed that blank on as
a successful empty result. The model read a blank as "zero rows", fell
back to `LIMIT` samples, and generalized from the sample. Five notes (025, 026, 033, 039, 041)
say the tool returned nothing and fall back on the schema. Of the notes with
false claims, only 025 does, and its false claim (a cross join) comes from
`mdb-queries` dropping the join, not from a blank. In the rest, the model
believed the blank.

## The false claims

| Note | The note says | The data says | Rule or defect |
|---|---|---|---|
| 001 | the largest 2026 quote is Q-2026-1699 | Q-2026-1713 | R12 |
| 001 | the largest 2026 order is SO-2026-0999 | SO-2026-1226 | R12 |
| 005 | the RMAs table is empty | 772 returns | D05 |
| 012 | the ManufacturerOrders table is empty | 9,054 calls | D07 |
| 015 | no order has two payments on the same date | 6 do | R10 |
| 019 | `qryOrderBalances` inner-joins the payments, so an unpaid order drops out of receivables | a LEFT JOIN; read the note's way, 232 unpaid open orders would vanish | R08 |
| 025 | `qryMonthEnd_CashReceived` is a cross join with no join predicates | INNER JOINs on order and customer | |
| 027 | the quote statuses are the five on the form, all present | also On Hold (12) and `'Draft'` (9) | R19, D01, D02 |
| 028 | the order statuses are exactly the six on the form | also Back Order (31) | R19, D02 |
| 030 | products 7, 8, 9, 20 and 21 have no firmware version | also 19 | |
| 040 | quote lines reach QuoteID ~17,319 | 19,427 | D04 |

(001 counts twice.)

**How one false claim formed (note 001).** The transcript shows each step:

1. `SELECT MAX(QuoteNumber) ... WHERE QuoteNumber LIKE 'Q-2026-%'` returned blank.
2. `... LIKE 'Q-2026-16%'` returned ten rows from the 1600s.
3. `... LIKE 'Q-2026-169%'` returned Q-2026-1690 through 1699.
4. The note says "Quotes max Q-2026-1699 (next Q-2026-1700)".

The model never asked for `Q-2026-17%`. The order number went the same way,
stopping at `SO-2026-099%`.

In the test round for this unit, the model rechecked before writing tests.
`QuoteNumber > 'Q-2026-1699'` returned rows, and it walked them up to
Q-2026-1713. Its conclusion: "The data has grown since my notes were taken
(Quotes now max at Q-2026-1713, Orders at SO-2026-1087...)". The database
never changed. The order figure was still wrong: the real maximum is
SO-2026-1226. The tests in `u001` take the maximum as an argument, so
they're right regardless.

**How the status lists closed (notes 027, 028).** For Quotes, `GROUP BY
quotestatus`, `DISTINCT quotestatus` and `COUNT(*)` all returned blank, over
and over. Then `WHERE quotestatus NOT IN ('Accepted','Sent','Draft')`
returned blank too. That's the query that would have found On Hold, and the
model read its blank as "none". The `u027` and `u028` status-list tests
match the form's value lists and are graded right. They would still pass
against a rewrite that can't load 52 of the file's rows.

## The incomplete claims

| Note | Values the note saw | Also in the data | Rule or defect |
|---|---|---|---|
| 019 | payment terms Net 30, Net 45 | Due on Receipt, Net 15, Net 60 | P02 |
| 031 | payment terms Due on Receipt, Net 15, Net 30, Net 45 | Net 60 (17 customers) | P02 |
| 031 | customer statuses Active, Inactive, Prospect | `'Prospect'` (4) | D01 |
| 027 | quote tax rates 0, 7.25%, 7.75%, 8.25% | 8.75%, 9.5% | |
| 028 | order tax rates 0, 7.25%, 8.25% | 7.75%, 8.75%, 9.5% | |

These notes say "observed" or "seen", so they're not false. But the `u031`
terms test turned "seen" into "the valid set" and asserts that Net 60 is
not a term. That's grading row 126, one of the four wrong tests.

## True for the wrong reason

Note 027 says every quote's ValidUntil is exactly 30 days after its
QuoteDate, "checked all 19389 rows, zero exceptions". It's true. The
query that would have checked it returned blank, which the model read as
zero exceptions. The count of 19,389 is real: `SELECT COUNT(*) FROM
"quotes"` is one of the few aggregates that came back.

## For the chapter

- **H3:** the model is accurate about what it can see. 54 of 71 claims
  hold, and every money figure it computed by hand is right to the cent.
- **H4:** the false claims are where the interviews start. They point at
  the returns nobody routes to Bench Test (D05, D08), the manufacturer
  double orders (D07), the retired statuses (D02, P05), and Net 60 (P02).
  Each was a question the model answered "none" because its tool said
  nothing.
- **Disclosure:** the chapter has to say the blank results were the
  harness's fault. `run-rs` fixes it (`run-py-issues.md`).
