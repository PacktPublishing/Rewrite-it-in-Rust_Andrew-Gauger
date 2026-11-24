# Flags for the sales team

Rules `cairn-core` ports exactly as V1 behaves, though they look wrong. Each
is a `// FLAG:` comment in the code; `tests/flags.rs` keeps this list and the
code in step. Changing any of them is the sales team's decision, made once,
in the open. The rule IDs are Chapter 7's (`chapter7/rule-inventory/rules.md`).

- **R05** `rounding.rs`: Money rounds the way Access does, half to even on binary floating-point values, so some printed cents differ from exact arithmetic (528 of 13,531 orders' tax; `SO-2026-1053` prints $877.10, exact half-up gives $877.11).
- **R22** `commission.rs`: Commission counts every order dated in the month unless it's Cancelled, paid or not. In September 2026, 82 of 136 orders had no payment. Priya: "If the customer hasn't paid, the rep hasn't earned it."
- **R24** `commission.rs`: Two commission figures. The statement rounds each order and adds them up; Sales by Rep adds up the month and rounds once. They differ by up to 5 cents in 397 of 818 rep-months (Carla, September 2026: $25,701.07 against $25,701.06). Both are ported. Which one is paid?
- **R28** `leads.rs`: A qualified lead is a Prospect with a Director or Manager on file, and either a Sent quote of $5,000 or more, or 2 activities in the last 90 days in Utilities, Food & Beverage or Healthcare. An existing customer buying more never counts, and neither does a small business whose contact is the owner.
- **R34** `rma.rs`: Bench Test can't be reached. The branch before it sends every return of a year or less to Manufacturer Repair, so 0 of 772 returns have gone to Bench Test. Carla believes returns in their first 90 days are bench-tested.
