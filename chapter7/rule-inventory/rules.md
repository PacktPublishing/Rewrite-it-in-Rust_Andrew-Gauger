# Cairn's Business Rules

*The ground-truth rule list for Chapter 7, built 2026-10-25 by reading the
VBA and saved queries inside `part 2/CairnSystems_History.accdb` (decoded to
`../ai-tests/outputs/vba-from-db/`). It replaces `inventory.md` as H5's count:
that file lists every query and routine, including the history generator and
the form builders, which aren't business rules.*

Each rule is written as a sentence a salesperson would recognize. Line
numbers are in the decoded modules (`CairnBuild.bas`, `CairnForms.bas`,
`CairnReports.bas`).

**Status** says what the rewrite does with the rule:

- **Keep:** port it as V1 behaves.
- **Flag:** port it, and take it to the sales team (`part 2/MISSION.md`,
  "Preserve and flag").
- **Fix:** a V1 defect the rewrite corrects (`MISSION.md`, "Fix, don't
  preserve").
- **Ask:** found while building this list, in neither MISSION list. Take it
  to the interviews before deciding.

**AI notes** says what the run's notes (`../ai-tests/outputs/notes/`) say
about the rule. **AI tests** counts tests from `../ai-tests/grading.md`
(right / wrong / tautological) and gives that file's rows.

## Rules in the code (46)

### Money

| ID | Rule | Where | Status | AI notes | AI tests |
|---|---|---|---|---|---|
| R01 | A line's total is quantity × price × (1 − discount), rounded to the cent. | `qryQuoteLines`, `qryOrderLines` | Keep | yes | 2 right |
| R02 | A line with no description prints the product's name. | `qryQuoteLines`, `qryOrderLines` | Keep | yes | 2 right |
| R03 | A document's subtotal is the sum of its line totals, or 0 with no lines. | `qryQuoteTotals`, `qryOrderTotals` | Keep | yes | inside R04's 2 |
| R04 | Tax is the subtotal × the tax rate, rounded to the cent. Shipping is never taxed. Total = subtotal + tax + shipping. | `qryQuoteTotals`, `qryOrderTotals` | Keep | yes | 2 right |
| R05 | Rounding sends an exact half cent to the even cent, and works on binary floating-point values, so some printed cents differ from exact arithmetic (528 of 13,531 orders' tax). | Access `Round`; the rate columns are `Double` | Flag | **no**. Treated as plain rounding | 0 |
| R06 | A line's cost is the product's cost today, not its cost when sold, so old orders' margins move when a cost changes. | `qryOrderLines` (`ol.Quantity*p.UnitCost`) | Ask | yes | 1 right |
| R07 | Paid is the sum of an order's payments. | `qryOrderPayments` | Keep | yes | 8 right |
| R08 | Balance due is the total minus paid. An order with no payments owes its whole total. | `qryOrderBalances` (LEFT JOIN) | Keep | **wrong**. Note 018 says unpaid orders drop out | 0, tests skipped |
| R09 | An order is an open receivable when it owes more than half a cent and isn't Cancelled. Its age counts from the order date. The customer's terms are printed but don't change the age. | `qryOpenReceivables` | Ask | yes | 0, tests skipped |
| R10 | A receipt shows what was paid on the order up to and including that payment: earlier dates first, then lower payment IDs on the same day. | `ReceiptPaidToDate`, CairnReports L503 | Keep | yes, with a false claim that no order has two payments on one day (6 do) | 6 right |
| R11 | A receipt's number is the payment ID, padded to 5 digits. | `rptReceipt`, CairnReports L151 | Keep | not reached | 0 |

### Quotes, orders and defaults

| ID | Rule | Where | Status | AI notes | AI tests |
|---|---|---|---|---|---|
| R12 | Quotes, orders and returns are numbered `Q-`, `SO-` and `RMA-`, then the year and a 4-digit count that restarts each January. The next number is this year's highest plus one, taken when the record is first saved. | `NextNumber` L671, `DocBeforeInsert` L700 | Fix: two people saving at once get the same number | yes. The race is missed | 10 right |
| R13 | Picking a customer on a quote or order sets the tax rate (0 if tax-exempt, otherwise the company default, copied so a later change doesn't reprice it), the customer's rep if none is set, and the primary contact. Opening a quote or order from a customer starts on that customer. | `DocCustomerChanged` L706, `DocLoad` L694 | Keep | yes | 18 right |
| R14 | On an activity, the rep defaults to the customer's rep and the contact to the primary contact, only if blank. | `ActivityCustomerChanged` L722, `ActivityDefaults` L731 | Keep | partly. `ActivityCustomerChanged` not reached | 5 right |
| R15 | Picking a product copies its current price onto the line. Only active products are offered. | `LineProductChanged` L736, `RS_PRODUCTS` L48 | Keep | yes | 7 right |
| R16 | A new quote is valid for 30 days from the day it's written. | frmQuotes `ValidUntil` default, CairnForms L414 | Keep | yes | 3 right |
| R17 | A quote with no terms of its own prints the company's standard terms. | `qryQuotePrint` | Keep | yes | 1 right |
| R18 | Create Order from Quote: one order per quote (a second try offers to open the first); a quote with no lines is refused; the header and lines are copied into an Open order dated today; the quote is marked Accepted. The steps aren't one transaction. | `QuoteToOrder` L755 | Fix: one transaction | yes. "No transaction" missed | 0, tests skipped |
| R19 | Statuses, terms and other choices come from fixed lists on the forms (quote, order and customer status; payment terms, including Net 60; activity type; payment method; message status; RMA reason and status; product category). The tables accept any text. | value lists, CairnForms L49–58 | Ask: typed statuses, and what to do with retired values (D02) | yes. Believes the lists are closed in the data too | 3 right, **2 wrong** |
| R20 | New records start as: quote Draft; order Open; customer Prospect on Net 30, not tax-exempt, in the USA; rep at 5% and active; line quantity 1. | table defaults, CairnBuild L215–330 | Keep | yes. The quote-mark bug (D01) missed | 4 right |

### Commission and month-end

| ID | Rule | Where | Status | AI notes | AI tests |
|---|---|---|---|---|---|
| R21 | An order's commission is its net sales × the rep's rate, rounded to the cent. Net sales are the line totals after discount, before tax and shipping. | `qryCommissionDetail` | Keep | yes | 1 right |
| R22 | Commission counts every order dated in the month unless it's Cancelled, whether or not it has been paid. | `qryCommissionDetail`, `CommissionCriteria` | Flag | stated, not flagged | 0. `qryCommissionDetail` tests skipped |
| R23 | A month report takes any date and covers that calendar month, from the 1st up to the 1st of the next. | `PrintCommission` L470, `PrintRMASummary` L428, `CommissionCriteria` L490, the month-end queries | Keep | yes | 5 right, **5 tautological** |
| R24 | The commission statement rounds each order and adds them up. Sales by Rep adds up the month and rounds once. The two differ by up to 5 cents in 397 of 818 rep-months. Carla's September 2026: $25,701.07 on the statement, $25,701.06 on Sales by Rep. | `qryCommissionDetail` vs `qryMonthEnd_SalesByRep` (`commission_rounding.py`) | Ask: which figure is paid? | **no**. Notes 017 and 023 each state their half | 0 |
| R25 | Month-end Orders, Sales by Rep and Sales by Product leave out Cancelled orders. Cash Received counts payments by payment date. Quote Pipeline counts every quote written that month by status, declined included. | the five `qryMonthEnd_*` queries | Keep | yes | 0, all five skipped |
| R26 | A commission statement opened outside the menu says "All dates". | `CommissionPeriod` L498 | Keep | yes | 3 right |

### Sales pipeline and messages

| ID | Rule | Where | Status | AI notes | AI tests |
|---|---|---|---|---|---|
| R27 | A customer's open quote value is its largest Sent quote, not the total of its Sent quotes. | `qryOpenQuoteValue` | Ask | yes, not questioned | 4 right |
| R28 | A qualified lead is a Prospect with a Director or Manager on file, and either a Sent quote of $5,000 or more, or 2 activities in the last 90 days in Utilities, Food & Beverage or Healthcare. | `qryQualifiedLeads`, `qryRecentTouches`, `qryOpenQuoteValue` | Flag | **skipped** | 0 |
| R29 | Follow-ups due are open activities whose follow-up date is within the next 7 days, overdue ones included. | `qryFollowUpsDue` | Keep | **skipped** | 0 |
| R30 | Picking a customer on a phone message fills in the company every time, and the phone only if it's blank. | `MessageCustomerChanged` L741 | Keep | yes | 5 right |
| R31 | Mark Returned sets the message to Returned and stamps the time once. | `MessageReturned` L748 | Keep | yes | 4 right |
| R32 | Open messages are the ones not Returned or Closed, urgent first, then oldest first. | `qryOpenPhoneMessages` | Keep | not reached | 0 |

### Returns

| ID | Rule | Where | Status | AI notes | AI tests |
|---|---|---|---|---|---|
| R33 | Days in service run from the order's ship date (or order date, if it never shipped) to the day the return was opened. | `qryRMARouting` | Keep | yes | 2 right |
| R34 | A return's route is the first match of nine branches, in order: Carrier Claim, Advance Replacement (DOA ≤ 30 days), Account Manager (Service), Manufacturer Repair (≤ 365 days or extended warranty), Advance Replacement (DOA), Bench Test (≤ 90 days, unreachable), Account Manager (tax-exempt), Scrap (> 730 days), Customer Pays Repair. | `qryRMARouting` | Flag: Bench Test | yes | 12 right; row 78 finds Bench Test |
| R35 | A return whose order isn't in the file has no days in service and falls through to Customer Pays Repair. | `qryRMARouting` + no foreign keys on `RMAs` | Fix | **no** | 0 |
| R36 | A new return gets the next `RMA-` number and is opened by whoever is signed in. | `RMADefaults` L843 | Keep | yes | 7 right |

### Manufacturer orders

| ID | Rule | Where | Status | AI notes | AI tests |
|---|---|---|---|---|---|
| R37 | Only hardware lines (Sensor, Gateway, Module, Kit, Accessory) go to the manufacturer. An order with none, or a company with no manufacturer address, sends nothing. | `SendManufacturerOrder` L854 | Keep | **skipped** | 1 right |
| R38 | Every attempt is logged with its number. Success is HTTP 201 with a manufacturer order ID. Retry sends the same request again with nothing to mark it a repeat, and the request is text pasted together. | `SendManufacturerOrder` L854 | Fix | **skipped** | 0 |
| R39 | An order shows its manufacturer order numbers in the order they were sent, comma-separated, leaving out failed attempts. | `MfrRefs` L927 | Keep | yes | 6 right |

### Sign-in and printing

| ID | Rule | Where | Status | AI notes | AI tests |
|---|---|---|---|---|---|
| R40 | Signing in needs an active user whose name and password match exactly. The last sign-in time is recorded. Passwords are plain text and the query is pasted together from what was typed. | `LoginCheck` L793 | Fix | yes. Every defect missed | 0, tests skipped |
| R41 | Nobody reaches the menu without signing in, and only admins see the Users screen. | `MenuLoad` L826, `MenuTimer` L834 | Fix: replaced by sign-in through Entra | not reached | 0 |
| R42 | An invoice prints at most 6 lines. Its totals count all of them. | `rptInvoice`, CairnReports L34 | Fix | not reached | 0 |

### What the tables enforce

| ID | Rule | Where | Status | AI notes | AI tests |
|---|---|---|---|---|---|
| R43 | Quotes, orders and activities need a customer and a rep, contacts need a customer, and lines need a product, each of which must exist. | relationships, CairnBuild L215–330 | Keep | yes | 15 right |
| R44 | Deleting a customer deletes its contacts and activities, and is refused while it has quotes or orders. Deleting a quote or order deletes its lines. A product with lines can't be deleted. | relationships | Keep | yes | 3 right, **2 wrong** |
| R45 | Quote numbers, order numbers and SKUs are unique. | indexes | Keep | yes | 2 right |
| R46 | Returns aren't tied to orders, products or customers, and quote lines aren't held to their quote. | `RMAs`; the QuoteLines relationship | Fix | partly. `table:RMAs` skipped | 0 |

## Defects in the data (10)

These aren't rules. They're what the rules let happen, and Chapter 15's
import has to deal with each one. Most are designed in (`part 2/README.md`).

| ID | Defect | Count | Rule | Surfaced by the AI |
|---|---|---|---|---|
| D01 | Text defaults saved with their quote marks | 9 quotes `'Draft'`, 4 customers `'Prospect'` | R20 | **yes**: grading row 125 fails on it |
| D02 | Retired statuses still in use | 31 orders Back Order (2017–18), 12 quotes On Hold (2018) | R19 | no. Notes 027/028 call the lists closed |
| D03 | Customers entered twice under a slightly different name | 14 | none | no |
| D04 | Orphaned quote lines | 162 | R46 | no |
| D05 | Returns for orders not in the file | 29 of 772 | R35, R46 | no. Note 005 calls RMAs empty |
| D06 | Sign-ins that should be gone: 3 departed reps, shared `sales` and `temp` | 5 of 17 | R40 | no |
| D07 | Manufacturer orders whose first attempt failed, some ordered twice | 246 orders | R38 | no. Note 012 calls ManufacturerOrders empty |
| D08 | A routing branch nothing reaches | 0 of 772 returns at Bench Test | R34 | **yes**: grading row 78 |
| D09 | Two commission figures for one rep and month | 397 of 818 rep-months | R24 | no |
| D10 | Two people saving at once get the same number | (a race, not a count) | R12 | no |

## Rules only people hold (6)

The code has nothing for these, so no reading of it finds them. They are
the interview list. Counts from `data_checks.py`.

| ID | Rule | Evidence | Ask |
|---|---|---|---|
| P01 | Special pricing is written into customer Notes as codes, each rep their own way, and no code reads them. | 76 customers: `dist 15%` 26, `[DIST-15]` 17, `net 60 ok - per Priya` 16, `[GOV]` 9, `KEY ACCT` 6, `gov pricing` 2 | each rep |
| P02 | Priya approves Net 60 by hand, and the terms field doesn't record it. | The 16 customers whose Notes say "net 60 ok" are on Net 30 (10), Net 15 (3), Due on Receipt (2) and Net 45 (1). The 17 customers on Net 60 have no such note. | Priya Shah |
| P03 | Commission should follow cash. | The sales manager's workbook tracks commission against payments (Chapter 6). It disagrees with R22. | Priya Shah |
| P04 | Which of a duplicated customer's two records is the real one. | D03 | Sam Okafor, the reps |
| P05 | What Back Order and On Hold meant, and whether they can come back. | D02 | the sales team |
| P06 | Which commission figure gets paid. | D09 | Priya Shah, payroll |

## H5's numbers

| | Rules | Stated in the AI's notes | With a right AI test |
|---|---|---|---|
| Keep | 29 | 25 | 23 |
| Flag, Fix or Ask: needs a decision | 17 | 10 | 5 |
| Only people hold | 6 | 0 | 0 |
| **All** | **52** | **35** | **28** |

- The AI's tests cover 23 of the 29 rules the rewrite simply keeps, and 5
  of the 17 that need someone to decide. Of those 5, only two tests show
  the problem: Bench Test (R34) and the `'Prospect'` quote marks (R19, D01).
- 2 of the 10 data defects surface.
- None of the 6 rules people hold could have surfaced. Neither could R24:
  each half of it sits in a different unit, and the walk shows the model
  one unit at a time.

The AI's suite is incomplete exactly where the decisions are. The routine
rules are mostly covered; the rules worth an interview mostly aren't.

## Reproducing the numbers

```sh
python3 -I chapter7/rule-inventory/commission_rounding.py   # R24, D09
python3 -I chapter7/rule-inventory/data_checks.py           # R19, D01, D02, P01, P02
python3 -I chapter7/ai-tests/grading_table.py --rules       # AI tests per rule
```

Their output is saved beside each script as `*-output.txt`. The other
counts come from `part 2/README.md` (D03–D07), `part 2/MISSION.md` and
`chapter6/export-demo/rounding.py` (R05).
