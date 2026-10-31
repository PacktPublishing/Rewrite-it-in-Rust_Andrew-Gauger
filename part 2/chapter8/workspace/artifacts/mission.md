# Mission: cairn-core, the business rules of Cairn Systems V2 (Chapter 8)

Port the business rules of the Cairn Systems Orders & CRM Access database (V1, the VBA in the source folder) into one Rust library crate (`cairn-core`). This is a rules library only: pure functions and plain types, with unit tests. No user interface, no database, no network, no async. Later chapters put a database, forms and an app around it.

**Port, don't refactor.** Keep each rule as V1 actually behaves, odd behaviour included. Where a rule looks like a bug, keep it and mark it with a `// FLAG:` comment and a line in `FLAGS.md` (one line per flag) so it can go to the sales team. The only exceptions are the "Fix" items below.

**Where the rules live in V1.** The rules are in the VBA (`CairnBuild.bas`, `CairnForms.bas`, `CairnReports.bas`, `CairnHistory.bas`) and the saved queries those modules create (`qryRMARouting`, `qryQualifiedLeads`, `qryCommissionDetail`, `qryReceiptPrint`, ...). Read them. The numbers in this mission are authoritative where they differ from your reading.

## Crate

- Library only, edition 2021, package name `cairn-core`, the first member of the Cairn V2 workspace at `part 2/crate/` (Chapters 9–13 add `migration`, `cairn-forms`, `cairn-forms-derive` and `cairn-app` beside it).
- Start from the earlier run's status enums (`part 2/ws34/artifacts/v2/src/{quotes,orders}.rs`: `QuoteStatus` Draft/Sent/Accepted/Declined/Expired, `OrderStatus` Open/In Production/Shipped/Invoiced/Closed/Cancelled). They match V1's value lists. Drop that run's `sqlx`, `tokio`, `chrono` and its `Example` entity: this crate has no database and no async.
- Dependencies: `rust_decimal` (money), `serde` and `serde_json`, `uuid` (v4, serde), and `csv` (the integration tests read the fixtures with it). Declare them all under `[dependencies]`, not dev-dependencies.
- Money is `rust_decimal::Decimal`. Rates and discounts arrive as `f64` because V1 stores them as `Double`; see "Rounding parity".
- Modules, one concern each and every file under 500 lines: `rounding`, `totals`, `defaults`, `quote_to_order`, `receipt`, `commission`, `rma`, `leads`, `manufacturer`. Re-export the public API from `lib.rs`.

## Totals

- Line total = `round(quantity × unit price × (1 − discount), 2)`. V1 computes it on a `Double` (the discount is `Double`), so use the same V1 rounding as tax and commission below. No line total in the data lands on a half cent, so exact half-even would give the same cents, but one rounding function for every `Round` in V1 is simpler to defend.
- Subtotal is the sum of line totals. Tax = `round(subtotal × tax rate, 2)` using the V1 rounding below, on the subtotal only, so shipping is never taxed. Total = subtotal + tax + shipping.
- Paid = the sum of payments. Balance due = total − paid.

## Rounding parity (the hard requirement)

V1's `Round` sends exact halves to the even cent, and it does that on binary floating-point values: the tax rate, discount and commission rate are `Double`. Plain half-up rounding is wrong. The V2 function must reproduce V1's printed figures.

The "access" model, which is what Access actually does: take the subtotal (or net sales) as `f64`, multiply by the rate as `f64`, multiply that by 100 in floating point, round ties to even (`f64::round_ties_even`), then divide by 100 and convert to `Decimal` with two places. `source/fixtures/rounding.py` shows all three models side by side, and `rounding-output.txt` is its output.

Acceptance (these are test assertions):
- Carla Mendes (rep 7), September 2026: commission **$25,701.07**, unpaid **$328,267.47**. Half-up would give $25,701.08, which is wrong.
- `SO-2026-1053`: net sales 12098.00 × tax rate 0.0725 = **$877.10**. Half-up would give $877.11, which is wrong.
- All September 2026 commission: **$183,280.48** over **136** orders (141 in the month, 5 cancelled and excluded), of which **82** have no payment.

## Fixtures

`tests/fixtures/` already exists in the crate and holds CSV exports from the History database: `orders_sept_2026.csv` (141 orders), `order_lines_sept_2026.csv`, `payments_sept_2026.csv`, `sales_reps.csv`. Dates are `MM/DD/YY HH:MM:SS`. Columns: Orders `OrderID, OrderNumber, CustomerID, ContactID, RepID, QuoteID, CustomerPO, OrderDate, RequiredDate, ShipDate, OrderStatus, TaxRate, ShippingCost, ShipVia, TrackingNumber, Notes`; read the other three files' header rows for theirs. Write integration tests in `tests/` that load these files and assert the three acceptance figures above. Do not edit the fixtures.

The routing test reads `rmas.csv` (all 772 returns), `rma_orders.csv` (OrderID, OrderDate, ShipDate for every order), `products.csv` and `customers_tax.csv` (CustomerID, TaxExempt), exported from the History database by `export_rma_fixtures.sh`. Acceptance: Manufacturer Repair 441, Customer Pays Repair 104 − 18 = 86, Advance Replacement 103 − 2 = 101, Carrier Claim 64, Scrap - Out of Warranty 44, Account Manager 16 − 3 = 13, Bench Test 0, UnknownOrder 23. Under V1's own rules (the Null fall-through) the same file gives Access's 441/104/103/64/44/16; test that too, with a function that reproduces V1's answer for an unknown order.

## Defaults when a customer is picked (V1's `DocCustomerChanged`)

- Tax rate is 0 for tax-exempt customers, otherwise the company's default tax rate. It is copied onto the document, so changing the default never reprices an old quote.
- The rep is the customer's assigned rep, if the document does not already have one.
- The contact is the customer's primary contact.

## Quote to order

A pure function from a quote (header and lines) and context to an order draft: copy the header (customer, contact, rep, tax rate, shipping) and every line (product, description, quantity, unit price, discount) into a new order with status Open and a given date. The quote's resulting status is Accepted. Return a typed error, not a string, for: a quote with no lines (`NoLines`), and a quote that already has an order (`AlreadyHasOrder` carrying that order's id, so a caller can offer to open it). The caller wraps the persistence in one transaction (Fix: V1 runs separate statements with nothing to undo the first if a later one fails); this crate only produces the draft and the new statuses, atomically, or an error and no draft.

## Receipts

Numbered by payment ID, zero-padded to 5 digits. A receipt shows the amount paid and the balance as of that payment: the payments up to and including it, ordered by date, then ID.

## Commission

Commission = rate × net sales. Net sales are the order's line totals after discount, before tax and shipping. Every order dated in the month counts unless it is Cancelled, whether or not it is paid (FLAG: commission pays on unpaid orders; in September 2026, 82 of 136 orders had no payment). Provide a per-rep statement (orders, net sales, commission, unpaid balance) and an all-reps total for a month.

V1 has a second commission figure. The month-end Sales by Rep query (`qryMonthEnd_SalesByRep`) adds up the month's net sales and rounds once: `Round(Sum(Subtotal) × rate, 2)`. The statement rounds each order and adds them up. They differ by up to five cents in 397 of 818 rep-months. Port both, as two functions, each marked `// FLAG:` (nobody has decided which one is paid). Acceptance: Carla Mendes, September 2026, statement **$25,701.07**, Sales by Rep **$25,701.06**.

## RMA routing (V1's `qryRMARouting`)

An exhaustive `enum Route` and a function choosing it, evaluated in exactly V1's order. "Days in service" is the RMA opened date minus the order's ship date (or order date if unshipped).
1. Reason "Damaged in Shipping": **Carrier Claim**
2. Reason "Dead on Arrival" and at most 30 days: **Advance Replacement**
3. Product category "Service": **Account Manager**
4. At most 365 days, or extended warranty: **Manufacturer Repair**
5. Reason "Dead on Arrival": **Advance Replacement**
6. At most 90 days: **Bench Test** (FLAG: unreachable, rule 4 already catches it; 0 of 772 returns route here)
7. Customer tax-exempt: **Account Manager**
8. More than 730 days: **Scrap - Out of Warranty**
9. Otherwise: **Customer Pays Repair**

V1 has nine branches but seven distinct routes (Advance Replacement and Account Manager each appear twice). `Route` has those seven variants plus one more, `UnknownOrder`.

**Fix: the unknown order.** A return whose order is not in the file has no days in service. V1 makes it Null, every comparison against Null is Null, `IIf` treats Null as false, and the return falls through. Model days in service as `Option<u32>`. A branch that doesn't read the days (1, 3, and the extended-warranty half of 4) keeps V1's answer. The first branch that would read the days and finds `None` returns `UnknownOrder`. Over the History database's 772 returns that gives 23 `UnknownOrder` (V1 routed them 18 Customer Pays Repair, 3 Account Manager, 2 Advance Replacement) and keeps 6 orphans at Carrier Claim. Every other return routes exactly as V1 does.

The reason is an `enum Reason` of V1's six (`VL_RMAREASON`: Dead on Arrival, Failed in Warranty, Damaged in Shipping, Wrong Item Shipped, Intermittent Fault, Customer Damage), parsed from text with `TryFrom<&str>` that fails on anything else.

Write at least one `match` over `Route` with no wildcard arm, so adding a variant is a compile error wherever routes are handled. `Display` gives V1's exact route strings.

## Qualified leads (V1's `qryQualifiedLeads`)

A customer is a qualified lead when all three hold (FLAG: take the definition to the sales team):
- Its status is Prospect.
- It has a Sent quote of $5,000 or more, or at least 2 activities in the last 90 days and an industry of Utilities, Food & Beverage, or Healthcare.
- It has a contact whose title contains "Director" or "Manager".

Existing customers buying more never qualify, and neither do small businesses whose contact is the owner.

## The manufacturer request

A typed, serde-serialized struct, never a string pasted together (Fix: a quote mark in a name breaks V1's JSON). It keeps V1's wire shape:

```json
{"dealer": "CAIRN", "po": "SO-2026-1226", "customerPo": "...",
 "shipTo": {"name": "...", "address": "...", "city": "...", "state": "...", "zip": "..."},
 "lines": [{"sku": "CS-TH100", "qty": 4}]}
```

The array is `lines`, not `items`: that's what V1 sends and what the mock manufacturer reads (`MockManufacturer/Start-MockManufacturer.ps1`, `$parsed.lines`).

Only hardware goes to the manufacturer: lines whose product category is Sensor, Gateway, Module, Kit or Accessory, in line order. An order with no hardware lines is refused before a request exists (V1 shows a message and sends nothing). The constructor returns `Result`, with typed errors: no hardware lines, a zero quantity, an empty ship-to name.

It adds a `correlationId` (Fix), a UUID generated once per logical order and reused on every retry. Provide a constructor that generates it and a way to build the same request again with the same id. Test that a name containing a quote mark and a backslash serializes to valid JSON that parses back to the same value, and that two requests for the same order share a `correlationId` when built from one.

## Done when

- `cargo build`, `cargo clippy -- -D warnings` and `cargo test` all pass with no warnings.
- The three acceptance figures are asserted by tests against the fixtures, and those tests would fail under half-up rounding.
- Every numbered RMA rule and every qualified-lead condition has a test, including the boundary days (30, 90, 365, 730) and the unknown-order case.
- The 772-return routing test passes, for V2's routes and for V1's.
- Quote to order, receipts, defaults and the manufacturer request each have tests for the cases above.
- `FLAGS.md` lists each FLAG in the code.
