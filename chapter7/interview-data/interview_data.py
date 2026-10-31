#!/usr/bin/env python3
"""Every number in the interview packet, read from the History database with mdbtools.

The packet is the set of claims the Analyst brings to each interview: what
the code does, what the AI's tests say, and what the data shows. Each
section below is one person (or role) and prints the numbers for them.
Rule and defect IDs refer to chapter7/rule-inventory/rules.md.

Queries that use Date() are evaluated as of --as-of, which defaults to the
last day in the file (the last sign-in, activity, order and message are all
on 10/09/26). The history is generated up to the day it is run, so a
regenerated database needs a different date.

Usage: python3 -I chapter7/interview-data/interview_data.py [--as-of 2026-10-09] [path/to/CairnSystems_History.accdb]
"""
import argparse
import csv
import re
import subprocess
from collections import Counter, defaultdict
from datetime import datetime, timedelta
from decimal import Decimal as D
from pathlib import Path

ap = argparse.ArgumentParser()
ap.add_argument("db", nargs="?", default=str(Path(__file__).resolve().parents[2] / "part 2" / "CairnSystems_History.accdb"))
ap.add_argument("--as-of", default="2026-10-09")
args = ap.parse_args()
TODAY = datetime.strptime(args.as_of, "%Y-%m-%d")


def table(name):
    out = subprocess.run(["mdb-export", args.db, name], capture_output=True, text=True, check=True).stdout
    return list(csv.DictReader(out.splitlines()))


def date(s):
    return datetime.strptime(s[:8], "%m/%d/%y") if s else None


def access(x):
    return D(round(x * 100)) / 100  # Access Round(x, 2) on a double, ties to even


def money(x):
    return f"${x:,.2f}"


T = {n: table(n) for n in ["Customers", "Contacts", "SalesReps", "Quotes", "QuoteLines", "Orders", "OrderLines",
                           "Payments", "Activities", "PhoneMessages", "Users", "RMAs", "ManufacturerOrders", "Products"]}
cust = {c["CustomerID"]: c for c in T["Customers"]}
reps = {r["RepID"]: r for r in T["SalesReps"]}
orders = {o["OrderID"]: o for o in T["Orders"]}
prods = {p["ProductID"]: p for p in T["Products"]}


def rep_name(rid):
    return f'{reps[rid]["FirstName"]} {reps[rid]["LastName"]}'


def line_total(l):
    return access(float(l["Quantity"]) * float(l["UnitPrice"]) * (1 - float(l["DiscountPct"] or 0)))


def doc_totals(docs, lines, key):
    """Subtotal and total per document, as qryQuoteTotals / qryOrderTotals compute them."""
    sub = defaultdict(D)
    for l in lines:
        sub[l[key]] += line_total(l)
    out = {}
    for d in docs:
        s = sub[d[key]]
        out[d[key]] = (s, s + access(float(s) * float(d["TaxRate"])) + D(d["ShippingCost"]).quantize(D("0.01")))
    return out


order_tot = doc_totals(T["Orders"], T["OrderLines"], "OrderID")
quote_tot = doc_totals(T["Quotes"], T["QuoteLines"], "QuoteID")
paid = defaultdict(D)
for p in T["Payments"]:
    paid[p["OrderID"]] += D(p["Amount"])


def balance(oid):
    return order_tot[oid][1] - paid[oid]


def commission(oid):
    return access(float(order_tot[oid][0]) * float(reps[orders[oid]["RepID"]]["CommissionRate"]))


def section(title):
    print(f"\n## {title}")


def say(label, value):
    print(f"- {label}: {value}")


print(f"# Interview packet numbers, as of {TODAY:%Y-%m-%d}")

# ---------------------------------------------------------------- Priya
section("Priya Shah, sales manager")
sept = [o for o in T["Orders"] if o["OrderStatus"] != "Cancelled" and date(o["OrderDate"]).strftime("%Y-%m") == "2026-09"]
say("R22 September 2026 orders (not cancelled)", len(sept))
say("R22   of those, no payment yet", sum(1 for o in sept if paid[o["OrderID"]] == 0))
say("R22   of those, a balance still owing", sum(1 for o in sept if balance(o["OrderID"]) > D("0.005")))
say("R22   commission on them, all reps (statement)", money(sum(commission(o["OrderID"]) for o in sept)))
say("R22   of which on orders still owing", money(sum(commission(o["OrderID"]) for o in sept if balance(o["OrderID"]) > D("0.005"))))
carla = next(r for r in reps if rep_name(r) == "Carla Mendes")
c_sept = [o for o in sept if o["RepID"] == carla]
say("R22 Carla, September 2026: orders", len(c_sept))
say("R22   commission statement total", money(sum(commission(o["OrderID"]) for o in c_sept)))
say("R22   still owed by her customers on those orders", money(sum(max(balance(o["OrderID"]), D(0)) for o in c_sept)))
say("R22   orders with no payment yet", sum(1 for o in c_sept if paid[o["OrderID"]] == 0))

months = defaultdict(list)
for o in T["Orders"]:
    if o["OrderStatus"] != "Cancelled":
        d = date(o["OrderDate"])
        months[(d.year, d.month, o["RepID"])].append(order_tot[o["OrderID"]][0])
gaps = []
for (y, m, rid), subs in months.items():
    rate = float(reps[rid]["CommissionRate"])
    statement = sum(access(float(s) * rate) for s in subs)
    by_rep = access(float(sum(subs)) * rate)
    if statement != by_rep:
        gaps.append((y, m, rid, statement, by_rep))
say("R24 rep-months where the statement and Sales by Rep disagree", f"{len(gaps)} of {len(months)}")
say("R24   largest gap", money(max(abs(s - b) for *_, s, b in gaps)))
for y, m, rid, s, b in gaps:
    if (y, m, rid) == (2026, 9, carla):
        say("R24   Carla, September 2026", f"statement {money(s)}, Sales by Rep {money(b)}")

notes = {c["CustomerID"]: c["Notes"] or "" for c in T["Customers"]}
n60_note = {c for c, n in notes.items() if re.search(r"(?i)net 60 ok", n)}
n60_terms = {c for c in cust if cust[c]["PaymentTerms"] == "Net 60"}
say("P02 customers whose Notes say 'net 60 ok - per Priya'", len(n60_note))
say("P02   their terms field", dict(Counter(cust[c]["PaymentTerms"] for c in n60_note)))
say("P02 customers on Net 60 in the terms field", len(n60_terms))
say("P02   in both groups", len(n60_note & n60_terms))

open_quotes = defaultdict(list)
for q in T["Quotes"]:
    if q["QuoteStatus"] == "Sent":
        open_quotes[q["CustomerID"]].append(quote_tot[q["QuoteID"]][1])
multi = {c: v for c, v in open_quotes.items() if len(v) > 1}
say("R27 customers with a Sent quote", len(open_quotes))
say("R27   with more than one, so 'open quote value' (the largest) understates the pipeline", len(multi))
say("R27   pipeline left out by taking the largest", money(sum(sum(v) - max(v) for v in multi.values())))

# ---------------------------------------------------------------- Carla
section("Carla Mendes, rep")
status = Counter(c["CustomerStatus"] for c in T["Customers"])
say("R28 customers by status", dict(status))
dm = {k["CustomerID"] for k in T["Contacts"] if re.search(r"(?i)director|manager", k["Title"] or "")}
prospects = [c for c in cust if cust[c]["CustomerStatus"] == "Prospect"]
say("R28 prospects with a Director or Manager on file", f"{sum(1 for c in prospects if c in dm)} of {len(prospects)}")
since = TODAY - timedelta(days=90)
touches = Counter(a["CustomerID"] for a in T["Activities"] if date(a["ActivityDate"]) >= since)
best_open = {c: max(v) for c, v in open_quotes.items()}
qualified = [c for c in prospects if c in dm and (
    best_open.get(c, D(0)) >= 5000 or (touches[c] >= 2 and cust[c]["Industry"] in ("Utilities", "Food & Beverage", "Healthcare")))]
say("R28 qualified leads (qryQualifiedLeads)", len(qualified))
say("R28   by quote of $5,000 or more", sum(1 for c in qualified if best_open.get(c, D(0)) >= 5000))
say("R28   by 2+ touches in 90 days in a target industry only", sum(1 for c in qualified if best_open.get(c, D(0)) < 5000))
say("R28 customers who aren't prospects and so never qualify", len(cust) - len(prospects))
say("R28   of those, stored as 'Prospect' with quote marks (D01)", status.get("'Prospect'", 0))
owner = {k["CustomerID"] for k in T["Contacts"] if re.search(r"(?i)\bowner\b", k["Title"] or "")}
say("R28 customers with a contact titled Owner", len(owner))
say("R28   prospects with an Owner but no Director or Manager, so they can't qualify",
    sum(1 for c in prospects if c in owner and c not in dm))
codes = [r"\[DIST-15\]", r"(?i)dist 15%", r"(?i)net 60 ok", r"\[GOV\]", r"(?i)gov pricing", r"KEY ACCT"]
for pat in codes:
    say(f"P01 Notes matching {pat}", sum(1 for n in notes.values() if re.search(pat, n)))
say("P01 customers with any pricing code", sum(1 for n in notes.values() if any(re.search(p, n) for p in codes)))
by_rep = defaultdict(Counter)
for c, n in notes.items():
    for pat, label in [(r"\[DIST-15\]", "[DIST-15]"), (r"(?i)dist 15%", "dist 15%")]:
        if re.search(pat, n):
            by_rep[rep_name(cust[c]["AssignedRepID"])][label] += 1
say("P01 distributor code by assigned rep", {r: dict(v) for r, v in sorted(by_rep.items())})

# ---------------------------------------------------------------- Sam
section("Sam Okafor, inside sales and reception")
msgs = T["PhoneMessages"]
say("R30 phone messages", len(msgs))
say("R30   by status", dict(Counter(m["MessageStatus"] for m in msgs)))
users = T["Users"]
say("R40 sign-ins", len(users))
say("R40   with a plain-text password", sum(1 for u in users if u["Password"]))
departed = [u["UserName"] for u in users if u["RepID"] and reps[u["RepID"]]["Active"] != "1" and u["Active"] == "1"]
say("D06 sign-ins still active for reps who left", f"{len(departed)}: {', '.join(departed)}")
say("D06 shared sign-ins with no rep", ", ".join(u["UserName"] for u in users if not u["RepID"] and u["IsAdmin"] != "1"))
pw = Counter(u["Password"] for u in users)
say("R40 passwords used by more than one sign-in", sum(1 for n in pw.values() if n > 1))
say("R40   sign-ins sharing reception's password",
    sum(1 for u in users if u["Password"] == next(x["Password"] for x in users if x["UserName"] == "reception")))

# ---------------------------------------------------------------- Returns
section("Returns and service lead")
rmas = T["RMAs"]


def route(r):
    """qryRMARouting with Access's Null logic: a Null comparison is not true."""
    o = orders.get(r["OrderID"])
    start = date(o["ShipDate"] or o["OrderDate"]) if o else None
    days = (date(r["OpenedDate"]) - start).days if start else None
    c = cust.get(r["CustomerID"])
    p = prods.get(r["ProductID"])
    le = lambda n: days is not None and days <= n
    if r["Reason"] == "Damaged in Shipping":
        return "Carrier Claim"
    if r["Reason"] == "Dead on Arrival" and le(30):
        return "Advance Replacement"
    if p and p["Category"] == "Service":
        return "Account Manager"
    if le(365) or r["ExtendedWarranty"] == "1":
        return "Manufacturer Repair"
    if r["Reason"] == "Dead on Arrival":
        return "Advance Replacement"
    if le(90):
        return "Bench Test"
    if c and c["TaxExempt"] == "1":
        return "Account Manager"
    if days is not None and days > 730:
        return "Scrap - Out of Warranty"
    return "Customer Pays Repair"


routes = Counter(route(r) for r in rmas)
say("R34 returns", len(rmas))
say("R34   by route", dict(routes.most_common()))
say("D08 routed to Bench Test", routes.get("Bench Test", 0))
orphans = [r for r in rmas if r["OrderID"] not in orders]
say("D05 returns for orders not in the file", len(orphans))
say("D05   their routes (days in service is Null)", dict(Counter(route(r) for r in orphans).most_common()))

# ---------------------------------------------------------------- Manufacturer orders
section("Whoever places manufacturer orders")
calls = T["ManufacturerOrders"]
say("R38 calls to the manufacturer's order API", len(calls))
say("R38   first call", min(calls, key=lambda m: date(m["SentAt"]))["SentAt"][:8])
say("R38   by HTTP status", dict(Counter(m["HttpStatus"] or "(no answer)" for m in calls)))
say("R38   errors", dict(Counter(m["ErrorText"] for m in calls if m["ErrorText"])))
shapes = defaultdict(list)
for m in sorted(calls, key=lambda m: m["Attempt"]):
    shapes[m["OrderID"]].append(m["HttpStatus"] or "none")
shape_count = Counter(" then ".join(v) for v in shapes.values())
say("D07 orders sent", len(shapes))
say("D07   by what Access logged (HTTP status per attempt)", dict(shape_count.most_common()))
say("D07 orders whose first attempt got no answer (README's 'failed first attempt')",
    sum(1 for v in shapes.values() if v[0] == "none"))
say("D07   no answer, then a retry that worked", shape_count["none then 201"])
say("D07   no answer twice, then nobody retried", shape_count["none then none"])
say("D07   a 500 error, then a retry that worked (the manufacturer created nothing)", shape_count["500 then 201"])
refs = sorted(int(m["ManufacturerRef"].split("-")[1]) for m in calls if m["ManufacturerRef"])
missing = (refs[-1] - refs[0] + 1) - len(set(refs))
say("D07 MFR- numbers recorded", f"{len(refs)}, MFR-{refs[0]} to MFR-{refs[-1]}")
say("D07   numbers missing from that range (orders the manufacturer took that Access never recorded)", missing)

# ---------------------------------------------------------------- The whole team
section("The sales team")
say("D02 orders still Back Order", sum(1 for o in T["Orders"] if o["OrderStatus"] == "Back Order"))
say("D02 quotes still On Hold", sum(1 for q in T["Quotes"] if q["QuoteStatus"] == "On Hold"))
say("D01 quotes stored as 'Draft' with quote marks", sum(1 for q in T["Quotes"] if q["QuoteStatus"] == "'Draft'"))
say("D04 quote lines whose quote is gone", sum(1 for l in T["QuoteLines"] if l["QuoteID"] not in {q["QuoteID"] for q in T["Quotes"]}))
