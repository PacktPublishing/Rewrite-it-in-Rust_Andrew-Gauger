# Interview data

The numbers the Analyst brings to each interview in H2 and H4. They're
grouped by person, and each number carries a rule or defect ID from
`../rule-inventory/rules.md`.

```sh
python3 -I chapter7/interview-data/interview_data.py            # as of 2026-10-09
python3 -I chapter7/interview-data/interview_data.py --as-of 2026-10-09 path/to/History.accdb
```

The output is saved as `interview_data-output.txt`. `qryQualifiedLeads`
counts touches in the last 90 days from `Date()`, so that figure depends on
`--as-of`. The default is the last day in the file: the last sign-in,
activity, order and message are all on 10/09/26.

## The packet

The interview cast isn't confirmed yet (`../TODO.md`). The last two roles
are new.

| Person | What to bring | Numbers |
|---|---|---|
| Priya Shah, sales manager (4%) | Commission pays on orders nobody has paid (R22, P03). The statement and Sales by Rep disagree (R24, P06). Net 60 lives in Notes (P02). "Open quote value" is the largest Sent quote, not the total (R27). | 82 of 136 September orders have no payment yet. Carla's September statement is $25,701.07 on orders whose customers still owe $328,267.47. The two commission figures differ in 397 of 818 rep-months, by up to $0.05 (Carla, September: $25,701.07 vs $25,701.06). 16 customers have "net 60 ok - per Priya" in Notes and none is on Net 60; the 17 on Net 60 have no note. 5 customers have more than one Sent quote, and $46,470.92 of pipeline doesn't show. |
| Carla Mendes, rep (6.5%) | What a qualified lead is (R28; the AI never reached it). Pricing codes typed her own way (P01). | 39 qualified leads: 13 by a quote of $5,000 or more, 26 by touches in a target industry. 183 of 206 prospects have a Director or Manager on file; 7 have only an Owner and can't qualify. The 4 customers stored as `'Prospect'` with quote marks never qualify (D01). Codes in 76 customers' Notes: `dist 15%` 26, `[DIST-15]` 17. Carla uses both, 3 times each. |
| Sam Okafor, inside sales and reception (3%) | Phone-message rules (R30). Sign-ins (R40, D06). | 4,638 messages, 4 still New. 17 sign-ins, all with plain-text passwords. 3 still work for reps who left (`rmalhotra`, `bhartley`, `tgrant`). Shared `sales` and `temp` logins. 3 sign-ins share reception's password. |
| Returns or service lead | The routing chain (R34). | 772 returns: 441 Manufacturer Repair, 104 Customer Pays Repair, 103 Advance Replacement, 64 Carrier Claim, 44 Scrap, 16 Account Manager, 0 Bench Test (D08). 29 returns are for orders not in the file (D05). With no order, days in service is Null, and 18 of them fall through to Customer Pays Repair. |
| Whoever places manufacturer orders | Retries and double orders (R38, D07). | 9,054 calls since 03/04/19 for 8,742 orders. On 246 orders the first attempt got no answer. 206 were retried and went through, and 40 got no answer twice. 286 MFR- numbers are missing from Access's range, one for each call that got no answer. 66 calls failed with a 500 error; those created nothing, and the retry was safe. |
| The sales team | Retired statuses (D02, P05), and the leftovers of bugs and deletions (D01, D04). | 31 orders still Back Order, 12 quotes On Hold. 9 quotes stored as `'Draft'`. 162 quote lines whose quote is gone. |

## Caveats before any of this goes into prose

- **The double orders come from the answer key, not the file.** The
  generator (`part 2/CairnHistory.bas`, `OpsMfrOrders`) says each call that
  got no answer had in fact been taken by the manufacturer. In the file
  itself, the evidence is the 286 MFR- numbers missing from the range. The
  Analyst can see the gap but has to ask the manufacturer what's behind it.
- **The routing and qualified-lead counts are Python re-creations** of
  `qryRMARouting` and `qryQualifiedLeads`, with Access's Null logic and
  `Like` matching modeled by hand. Check both against Access, for example
  in the screenshot harness, before the figures go into prose.
- **"Unpaid" has two readings.** 82 of 136 September orders have no
  payment at all. 131 of 136 still owe something. The packet uses 82.
