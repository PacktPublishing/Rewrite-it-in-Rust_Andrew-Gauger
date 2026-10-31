#!/usr/bin/env python3
"""What does Access mean by Round(x, 2)?

Reproduces the rounding numbers in Chapter 6 (Headings 3 and 4) straight
from the History database, using mdbtools to read it. Three models of
"round to cents" are checked against the values Access itself printed on
Carla Mendes's September 2026 commission statement (ScreenshotHarness run
2026-10-14_110149, shot 23):

  half-up     exact decimal, ties away from zero (what Postgres round() does
              on numeric, and what the hand-written SQL in run.sh does)
  half-even   exact decimal, ties to even (textbook banker's rounding)
  access      the double-precision product, times 100 in floating point,
              then ties to even; DiscountPct, TaxRate and CommissionRate
              are Double columns in the Access file

Usage: python3 -I chapter6/export-demo/rounding.py [path/to/CairnSystems_History.accdb]
"""
import csv
import subprocess
import sys
from decimal import Decimal as D, ROUND_HALF_EVEN, ROUND_HALF_UP
from pathlib import Path

DB = sys.argv[1] if len(sys.argv) > 1 else str(Path(__file__).resolve().parents[2] / "CairnSystems_History.accdb")


def table(name):
    out = subprocess.run(["mdb-export", DB, name], capture_output=True, text=True, check=True).stdout
    return list(csv.DictReader(out.splitlines()))


def half_up(x):
    return D(x).quantize(D("0.01"), ROUND_HALF_UP)


def half_even(x):
    return D(x).quantize(D("0.01"), ROUND_HALF_EVEN)


def access(x_float):
    return D(round(x_float * 100)) / 100  # Python's round() on a float is ties-to-even


orders, lines, payments, reps = table("Orders"), table("OrderLines"), table("Payments"), table("SalesReps")
rate = {r["RepID"]: r["CommissionRate"] for r in reps}

# Net sales per order. No line total in the file lands on a half cent, so the
# rounding model doesn't matter at this step.
subtotal, paid = {}, {}
for l in lines:
    lt = half_even(D(l["Quantity"]) * D(l["UnitPrice"]) * (1 - D(l["DiscountPct"] or "0")))
    subtotal[l["OrderID"]] = subtotal.get(l["OrderID"], D(0)) + lt
for p in payments:
    paid[p["OrderID"]] = paid.get(p["OrderID"], D(0)) + D(p["Amount"])

MODELS = {
    "half-up": lambda s, r: half_up(s * D(r)),
    "half-even": lambda s, r: half_even(s * D(r)),
    "access": lambda s, r: access(float(s) * float(r)),
}


def sept_2026(o):
    return o["OrderDate"].startswith("09/") and o["OrderDate"][6:8] == "26" and o["OrderStatus"] != "Cancelled"


print("== Carla Mendes, September 2026 (Access printed: commission $25,701.07, unpaid $328,267.47)")
for name, rnd in MODELS.items():
    comm = unpaid = D(0)
    for o in orders:
        if o["RepID"] == "7" and sept_2026(o):
            s = subtotal[o["OrderID"]]
            comm += rnd(s, rate["7"])
            unpaid += s + rnd(s, o["TaxRate"]) + D(o["ShippingCost"] or "0") - paid.get(o["OrderID"], D(0))
    print(f"  {name:<10} commission {comm:>12,.2f}   unpaid {unpaid:>14,.2f}")

print("\n== SO-2026-1053 tax: net sales x tax rate")
o = next(o for o in orders if o["OrderNumber"] == "SO-2026-1053")
s = subtotal[o["OrderID"]]
print(f"  {s} x {o['TaxRate']} = {s * D(o['TaxRate'])}")
for name, rnd in MODELS.items():
    print(f"  {name:<10} {rnd(s, o['TaxRate'])}")

print("\n== All September 2026 commission statements")
for name, rnd in MODELS.items():
    n = n_unpaid = 0
    comm = comm_unpaid = D(0)
    for o in orders:
        if sept_2026(o):
            c = rnd(subtotal.get(o["OrderID"], D(0)), rate[o["RepID"]])
            n += 1
            comm += c
            if paid.get(o["OrderID"], D(0)) == 0:
                n_unpaid += 1
                comm_unpaid += c
    print(f"  {name:<10} {n} orders, commission {comm:,.2f}; {n_unpaid} unpaid orders carry {comm_unpaid:,.2f}")

print("\n== Orders whose tax depends on the rounding model")
differ = sum(
    1 for o in orders
    if MODELS["access"](subtotal.get(o["OrderID"], D(0)), o["TaxRate"]) != MODELS["half-up"](subtotal.get(o["OrderID"], D(0)), o["TaxRate"])
)
print(f"  access vs half-up: {differ} of {len(orders)}")
