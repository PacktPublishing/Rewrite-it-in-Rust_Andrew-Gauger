#!/usr/bin/env python3
"""Route all 772 returns the way qryRMARouting does, and find the ones
whose route rested on a Null days-in-service.

A Python port of the eight nested IIf calls in qryRMARouting
(part 2/CairnBuild.bas, CreateOperationsQueries). Access's three-valued
logic: a comparison against Null is Null, and IIf treats Null as False.
The port tracks which returns reached their route only because a
comparison came back Null. Those are the returns cairn-core's
`Option<days>` forces a decision on (Chapter 8, H4).

Its totals must match Access's own (chapter7/stage3-windows/results.md,
checks 1a and 1b) before any number here goes in the chapter.

Usage: python3 -I chapter8/rule-checks/route_check.py [path/to/CairnSystems_History.accdb]
"""
import csv
import subprocess
import sys
from collections import Counter
from datetime import date
from pathlib import Path

DB = sys.argv[1] if len(sys.argv) > 1 else str(Path(__file__).resolve().parents[2] / "part 2" / "CairnSystems_History.accdb")


def table(name):
    out = subprocess.run(["mdb-export", "-D", "%Y-%m-%d", "-T", "%Y-%m-%d", DB, name],
                         capture_output=True, text=True, check=True).stdout
    return list(csv.DictReader(out.splitlines()))


def day(s):
    return date.fromisoformat(s[:10]) if s else None


customers = {r["CustomerID"]: r for r in table("Customers")}
products = {r["ProductID"]: r for r in table("Products")}
orders = {r["OrderID"]: r for r in table("Orders")}


def route(r):
    """(route, rested_on_null) in qryRMARouting's branch order."""
    o = orders.get(r["OrderID"])
    days = (day(r["OpenedDate"]) - (day(o["ShipDate"]) or day(o["OrderDate"]))).days if o else None
    reason, ew = r["Reason"], r["ExtendedWarranty"] == "1"
    category = products.get(r["ProductID"], {}).get("Category")
    tax_exempt = customers.get(r["CustomerID"], {}).get("TaxExempt") == "1"
    null = False
    if reason == "Damaged in Shipping":
        return "Carrier Claim", null
    if reason == "Dead on Arrival":
        if days is None:
            null = True
        elif days <= 30:
            return "Advance Replacement", null
    if category == "Service":
        return "Account Manager", null
    if ew:  # [DaysInService]<=365 Or ExtendedWarranty: True Or Null is True
        return "Manufacturer Repair", null
    if days is None:
        null = True
    elif days <= 365:
        return "Manufacturer Repair", null
    if reason == "Dead on Arrival":
        return "Advance Replacement", null
    if days is not None and days <= 90:
        return "Bench Test", null
    if tax_exempt:
        return "Account Manager", null
    if days is not None and days > 730:
        return "Scrap - Out of Warranty", null
    return "Customer Pays Repair", null


rmas = table("RMAs")
print(f"returns: {len(rmas)}")
for name, n in Counter(route(r)[0] for r in rmas).most_common():
    print(f"  {name}: {n}")
print("  Bench Test: 0" if not any(route(r)[0] == "Bench Test" for r in rmas) else "")

orphans = [r for r in rmas if r["OrderID"] not in orders]
print(f"returns whose order isn't in the file: {len(orphans)}")
print(f"  OrderID blank: {sum(1 for r in orphans if not r['OrderID'])}")
print(f"  OrderID points at no order: {sum(1 for r in orphans if r['OrderID'])}")
for (name, null), n in Counter(route(r) for r in orphans).most_common():
    print(f"  {name}{' (rested on Null)' if null else ''}: {n}")
print(f"orphans whose route rested on Null: {sum(1 for r in orphans if route(r)[1])}")
