#!/usr/bin/env python3
"""Do Cairn's two commission figures agree?

The commission statement (rptCommission over qryCommissionDetail) rounds each
order's commission to the cent and adds them up. The month-end Sales by Rep
query (qryMonthEnd_SalesByRep) adds up the month's net sales and rounds once.
This counts the rep-months where the two printed figures differ, using
Chapter 6's model of Access's Round (chapter6/export-demo/rounding.py).

Usage: python3 -I chapter7/rule-inventory/commission_rounding.py [path/to/CairnSystems_History.accdb]
"""
import csv
import subprocess
import sys
from collections import defaultdict
from datetime import datetime
from decimal import Decimal as D, ROUND_HALF_EVEN
from pathlib import Path

DB = sys.argv[1] if len(sys.argv) > 1 else str(Path(__file__).resolve().parents[2] / "part 2" / "CairnSystems_History.accdb")


def table(name):
    out = subprocess.run(["mdb-export", DB, name], capture_output=True, text=True, check=True).stdout
    return list(csv.DictReader(out.splitlines()))


def access(x_float):
    return D(round(x_float * 100)) / 100  # as rounding.py: the double times 100, ties to even


reps = {r["RepID"]: r for r in table("SalesReps")}
subtotal = defaultdict(D)
for l in table("OrderLines"):
    subtotal[l["OrderID"]] += (D(l["Quantity"]) * D(l["UnitPrice"]) * (1 - D(l["DiscountPct"] or "0"))).quantize(D("0.01"), ROUND_HALF_EVEN)

months = defaultdict(list)
for o in table("Orders"):
    if o["OrderStatus"] == "Cancelled":
        continue
    d = datetime.strptime(o["OrderDate"][:8], "%m/%d/%y")
    months[(d.year, d.month, o["RepID"])].append(subtotal[o["OrderID"]])

differ = []
for (y, m, rep), subs in sorted(months.items()):
    rate = float(reps[rep]["CommissionRate"])
    statement = sum(access(float(s) * rate) for s in subs)
    month_end = access(float(sum(subs)) * rate)
    if statement != month_end:
        differ.append((y, m, reps[rep]["FirstName"] + " " + reps[rep]["LastName"], statement, month_end))

print(f"rep-months with orders: {len(months)}")
print(f"statement and month-end figures differ: {len(differ)}")
print(f"largest gap: ${max(abs(s - e) for *_, s, e in differ)}")
print("September 2026:")
for y, m, name, s, e in differ:
    if (y, m) == (2026, 9):
        print(f"  {name}: statement ${s}, Sales by Rep ${e}")
