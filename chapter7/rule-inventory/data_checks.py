#!/usr/bin/env python3
"""The data counts rules.md cites, read from the History database with mdbtools.

Usage: python3 -I chapter7/rule-inventory/data_checks.py [path/to/CairnSystems_History.accdb]
"""
import csv
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

DB = sys.argv[1] if len(sys.argv) > 1 else str(Path(__file__).resolve().parents[2] / "part 2" / "CairnSystems_History.accdb")


def table(name):
    out = subprocess.run(["mdb-export", DB, name], capture_output=True, text=True, check=True).stdout
    return list(csv.DictReader(out.splitlines()))


customers, quotes, orders = table("Customers"), table("Quotes"), table("Orders")

print("D01/D02 status values as stored")
print("  quotes:   ", dict(Counter(q["QuoteStatus"] for q in quotes)))
print("  orders:   ", dict(Counter(o["OrderStatus"] for o in orders)))
print("  customers:", dict(Counter(c["CustomerStatus"] for c in customers)))

print("R19 payment terms as stored:", dict(Counter(c["PaymentTerms"] for c in customers)))

CODES = [r"\[DIST-15\]", r"(?i)dist 15%", r"(?i)net 60 ok", r"\[GOV\]", r"(?i)gov pricing", r"KEY ACCT"]
notes = {c["CustomerID"]: c["Notes"] or "" for c in customers}
print("P01 pricing codes in customer Notes:")
for pat in CODES:
    print(f"  {pat}: {sum(1 for n in notes.values() if re.search(pat, n))}")
print(f"  customers with any code: {sum(1 for n in notes.values() if any(re.search(p, n) for p in CODES))}")

net60_terms = {c["CustomerID"] for c in customers if c["PaymentTerms"] == "Net 60"}
net60_note = {c["CustomerID"] for c in customers if re.search(r"(?i)net 60 ok", notes[c["CustomerID"]])}
print("P02 Net 60:")
print(f"  terms say Net 60: {len(net60_terms)}; notes say 'net 60 ok': {len(net60_note)}; both: {len(net60_terms & net60_note)}")
print("  terms of the customers whose notes say 'net 60 ok':",
      dict(Counter(c["PaymentTerms"] for c in customers if c["CustomerID"] in net60_note)))
