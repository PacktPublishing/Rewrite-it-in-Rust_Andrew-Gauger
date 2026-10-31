# Interview cast

The people the Analyst interviews in H2 and goes back to in H4. These are
people at a small company, described by job and by what they touch in the
file. They have no backstories. Where a person is in the database, the
name, title and rate come from `SalesReps`, and "what they touch" comes
from the data. Two people aren't in `SalesReps` because they aren't in
sales. Their names are new, and so are the details marked *(invented)*.

The numbers to bring to each person are in `README.md` and
`interview_data-output.txt`.

| Person | Job | What they touch in the file | What the interview is for |
|---|---|---|---|
| **Priya Shah** | Sales Manager (4% commission). Admin sign-in `pshah`. | Approves special pricing and terms. Reviews the commission statements before they're paid. Keeps the workbook that tracks commission against cash (Chapter 6). | R22 and P03: should commission follow cash? R24 and P06: which of the two commission figures is the real one? P02: Net 60 approvals that live only in Notes. R27: what "open quote value" should mean. |
| **Carla Mendes** | Senior Account Executive (6.5%). | Keeps `pipeline_CM.xlsx` (Chapter 6). Opened 264 of the 772 returns, more than anyone. Writes pricing codes into Notes both ways (`dist 15%` and `[DIST-15]`, three times each). | R28: what a qualified lead is (the AI never reached it). R34 and D08: how a return should be routed, and why nothing goes to Bench Test. P01: what the pricing codes mean. |
| **Sam Okafor** | Inside Sales / Reception (3%). Sign-in `reception`. | Took 1,619 of the 4,638 phone messages. Enters new callers as customers *(invented)*. | R30: phone-message rules. P04 and D03: which of a duplicated customer's two records is real. D06: who uses the shared `sales` and `temp` sign-ins, and the departed reps' sign-ins that still work. |
| **Linda Ortiz** | Bookkeeper, part-time. Not in `SalesReps`. | Uses the shared `admin` sign-in *(invented)*. Runs month-end: Cash Received, Open Receivables, Sales by Rep. Pays the manufacturer's invoices. | R09 and R10: receivables and receipts. R24: she's the one who would see a statement and Sales by Rep a few cents apart. D07: whether the manufacturer ever invoiced hardware twice. Only she can say, because Access recorded one MFR- number for each double order. |
| **Gary Dunn** | The contractor who maintains the file by the hour (Chapter 6's $85,000). Not staff. | The VBA, the forms and the `RepairTextDefaults` fix. He didn't write the original file. | Short. He knows what the code does, but not why. R12 and D10: whether two people saving at once ever got the same number. D01: when the quote-mark bug happened, and what he fixed. The interview shows the limit of the code's only reader. |

## Who isn't interviewed

- **The original author.** Gone, as the outline says. Nobody knows who
  built the first version.
- **The other reps.** Carla stands in for them. Nadia Petrova, the other
  Senior Account Executive, also writes both kinds of distributor code
  (4 each), so a second interview would repeat Carla's.
- **Reps who left** (Raj Malhotra, Ben Hartley, Owen Fitch, Tessa Grant).
  They appear only as sign-ins that still work.

## Notes for drafting

- Third person throughout. The Analyst runs the interviews.
- Interview notes are `>` blockquotes, as the house style has it for
  tickets and memos. Keep each to what the person said, in plain words.
- Each interview should surface one rule the code doesn't show and one
  the person believes that the code contradicts (`../TODO.md`, H2).
  - Priya believes commission follows cash, and the code pays on unpaid orders.
  - Carla believes a return in its first 90 days gets bench-tested, and no
    route ever sends them there.
  - Sam believes he never enters a customer twice, and 14 customers are
    in the file twice.
  - Linda believes the two commission reports agree, and they differ in
    397 of 818 rep-months.
- The cast is the author's to change. Linda's and Gary's names are
  placeholders until confirmed.
