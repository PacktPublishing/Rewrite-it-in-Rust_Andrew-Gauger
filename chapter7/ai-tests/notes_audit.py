#!/usr/bin/env python3
"""Check every data claim in the AI's notes against the History database.

Each check names the note, the claim as the note states it, what the note
says, and what the data says. A claim about the code (a join type, a
formula) is checked against the builder source in part 2/CairnBuild.bas,
because mdb-queries drops join types from the saved SQL.

Usage: python3 -I chapter7/ai-tests/notes_audit.py [path/to/CairnSystems_History.accdb]
"""
import csv
import re
import subprocess
import sys
from collections import Counter, defaultdict
from datetime import datetime
from decimal import Decimal as D
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DB = sys.argv[1] if len(sys.argv) > 1 else str(ROOT / "part 2" / "CairnSystems_History.accdb")
BUILD = (ROOT / "part 2" / "CairnBuild.bas").read_text(errors="replace")


def table(name):
    out = subprocess.run(["mdb-export", DB, name], capture_output=True, text=True, check=True).stdout
    return list(csv.DictReader(out.splitlines()))


def date(s):
    return datetime.strptime(s[:8], "%m/%d/%y") if s else None


def access(x):
    return D(round(x * 100)) / 100  # Access Round(x, 2) on a double, ties to even


T = {n: table(n) for n in ["Quotes", "QuoteLines", "Orders", "OrderLines", "Payments", "Customers",
                           "Contacts", "SalesReps", "Products", "CompanyInfo", "Users", "RMAs",
                           "ManufacturerOrders", "Activities"]}
by_id = {n: {r[next(iter(r))]: r for r in rows} for n, rows in T.items()}

results = []


def check(note, claim, said, actual, kind="exact", link=""):
    """kind: exact; approx (the note says "~", within 1%); seen (the note lists the
    values it saw, so a shorter list is incomplete, not false)."""
    if str(said) == str(actual):
        verdict = "true"
    elif kind == "approx" and abs(float(str(said).strip("~")) - float(actual)) <= 0.01 * float(actual):
        verdict = "true"
    elif kind == "seen" and set(str(said).split(", ")) < set(str(actual).split(", ")):
        verdict = "incomplete"
    else:
        verdict = "false"
    results.append((note, claim, said, actual, verdict, link))


# Money helpers, as qryQuoteLines / qryQuoteTotals / qryOrderTotals compute them.
def line_total(l):
    return access(float(l["Quantity"]) * float(l["UnitPrice"]) * (1 - float(l["DiscountPct"] or 0)))


q_lines, o_lines = defaultdict(list), defaultdict(list)
for l in T["QuoteLines"]:
    q_lines[l["QuoteID"]].append(l)
for l in T["OrderLines"]:
    o_lines[l["OrderID"]].append(l)


def totals(doc, lines):
    sub = sum((line_total(l) for l in lines), D(0))
    tax = access(float(sub) * float(doc["TaxRate"]))
    return sub, tax, sub + tax + D(doc["ShippingCost"]).quantize(D("0.01"))


def max_number(rows, field, stem):
    return max((r[field] for r in rows if r[field].startswith(stem)), default=None)


Q, O, P = T["Quotes"], T["Orders"], T["Payments"]
cust, reps, prods = by_id["Customers"], by_id["SalesReps"], by_id["Products"]
pays_by_order = defaultdict(list)
for p in P:
    pays_by_order[p["OrderID"]].append(p)

# 001 NextNumber
check("001", "largest 2026 quote number", "Q-2026-1699", max_number(Q, "QuoteNumber", "Q-2026-"))
check("001", "largest 2026 order number", "SO-2026-0999", max_number(O, "OrderNumber", "SO-2026-"))
check("001", "largest 2026 RMA number", "RMA-2026-0069", max_number(T["RMAs"], "RMANumber", "RMA-2026-"))

# 002 QuoteToOrder
q1, o1 = by_id["Quotes"]["1"], by_id["Orders"]["1"]
check("002", "quote 1 number and status", "Q-2026-1346 Accepted", f'{q1["QuoteNumber"]} {q1["QuoteStatus"]}')
check("002", "order 1 comes from quote 1", "SO-2026-0920 from quote 1", f'{o1["OrderNumber"]} from quote {o1["QuoteID"]}')
same = all((a["ProductID"], a["Quantity"], a["UnitPrice"], a["DiscountPct"]) == (b["ProductID"], b["Quantity"], b["UnitPrice"], b["DiscountPct"])
           for a, b in zip(q_lines["1"], o_lines["1"])) and len(q_lines["1"]) == len(o_lines["1"]) == 4
check("002", "quote 1's 4 lines copied 1:1 to order 1", True, same)

# 003 LoginCheck
users = {u["UserName"]: u for u in T["Users"]}
check("003", "ofitch is UserID 9 and inactive", "9 inactive", f'{users["ofitch"]["UserID"]} {"active" if users["ofitch"]["Active"] == "1" else "inactive"}')
check("003", "admin, sales and temp have no RepID", True, all(not users[n]["RepID"] for n in ["admin", "sales", "temp"]))

# 005 RMADefaults, 012 MfrRefs
check("005", "RMAs table is empty", 0, len(T["RMAs"]), link="D05")
check("012", "ManufacturerOrders table is empty", 0, len(T["ManufacturerOrders"]), link="D07")

# 006, 030 Products
def prod(i):
    p = prods[str(i)]
    return f'{p["SKU"]} {D(p["UnitPrice"]):.2f} {"active" if p["Active"] == "1" else "inactive"}'
for i, said in [(1, "CS-TH100 89.00 active"), (5, "CS-GW800 649.00 active"), (11, "CS-TH050 79.00 inactive"), (12, "CS-GW400 449.00 inactive")]:
    check("006", f"product {i}", said, prod(i))
check("030", "product count", 21, len(T["Products"]))
check("030", "active / inactive products", "19/2", f'{sum(p["Active"] == "1" for p in T["Products"])}/{sum(p["Active"] != "1" for p in T["Products"])}')
check("030", "products with no firmware version", "7,8,9,20,21", ",".join(p["ProductID"] for p in T["Products"] if not p["FirmwareVersion"]))

# 007, 009, 010 customers 1-3, contacts, tax rate
for cid, said in [("1", "Northfield Water District exempt rep 11"), ("2", "Bluepine Cold Storage taxed rep 1"), ("3", "Harlow Manufacturing taxed rep 1")]:
    c = cust[cid]
    check("007", f"customer {cid}", said, f'{c["CompanyName"]} {"exempt" if c["TaxExempt"] == "1" else "taxed"} rep {c["AssignedRepID"]}')
check("007", "default tax rate", "0.0825", T["CompanyInfo"][0]["DefaultTaxRate"])
check("010", "customer 1 and 2 phones", "555-2100 555-3300", f'{cust["1"]["Phone"]} {cust["2"]["Phone"]}')
primaries = Counter(c["CustomerID"] for c in T["Contacts"] if c["IsPrimary"] == "1")
check("009", "every customer has exactly one primary contact", True, all(primaries[c] == 1 for c in cust))
check("033", "contacts / customers", "1973/1000", f'{len(T["Contacts"])}/{len(T["Customers"])}')

# 015 ReceiptPaidToDate
same_day = Counter((p["OrderID"], p["PaymentDate"]) for p in P)
check("015", "orders with two payments on the same date", 0, sum(1 for n in same_day.values() if n > 1), link="R10")

# 016, 020, 035 quote totals
for qid, said in [("1", "12374.00 0.00 12524.00"), ("2", "14373.00 1185.77 15633.77"), ("3", "4912.00 380.68 5332.68")]:
    check("016", f"quote {qid} subtotal, tax, total", said, " ".join(f"{x:.2f}" for x in totals(by_id["Quotes"][qid], q_lines[qid])))
q3 = by_id["Quotes"]["3"]
check("020", "quote 3 is customer 4's, Sent", "4 Sent", f'{q3["CustomerID"]} {q3["QuoteStatus"]}')
sent4 = [totals(q, q_lines[q["QuoteID"]])[2] for q in Q if q["CustomerID"] == "4" and q["QuoteStatus"] == "Sent"]
check("020", "customer 4's open quote value", "5332.68", f"{max(sent4):.2f}")
check("040", "quote lines with a LineDescription", 0, sum(1 for l in T["QuoteLines"] if l["LineDescription"]))
check("040", "quote lines with quantity <= 0", 0, sum(1 for l in T["QuoteLines"] if int(l["Quantity"]) <= 0))

# 017, 023, 036 commission rates
rate = {f'{r["FirstName"]} {r["LastName"]}': r for r in T["SalesReps"]}
check("017", "Maya, Priya, Carla rates", "0.06 0.04 0.065", " ".join(rate[n]["CommissionRate"] for n in ["Maya Torres", "Priya Shah", "Carla Mendes"]))
check("036", "rep count", 14, len(T["SalesReps"]))
check("036", "inactive reps", "Ben Hartley, Owen Fitch, Raj Malhotra, Tessa Grant",
      ", ".join(sorted(n for n, r in rate.items() if r["Active"] != "1")))
check("036", "rate range", "0.03-0.065", f'{min(r["CommissionRate"] for r in T["SalesReps"])}-{max(r["CommissionRate"] for r in T["SalesReps"])}')

# 018, 022, 037 order 1 and payments
s1, t1, tot1 = totals(o1, o_lines["1"])
cost1 = sum(D(l["Quantity"]) * D(prods[l["ProductID"]]["UnitCost"]) for l in o_lines["1"])
paid1 = sum(D(p["Amount"]) for p in pays_by_order["1"])
check("022", "order 1 subtotal, cost, total, paid", "12374.00 5112.00 12524.00 12524.00", f"{s1:.2f} {cost1:.2f} {tot1:.2f} {paid1:.2f}")
check("018", "order 1 paid and last payment", "12524.00 09/24/26",
      f'{paid1:.2f} {max(pays_by_order["1"], key=lambda p: date(p["PaymentDate"]))["PaymentDate"][:8]}')
ql = by_id["QuoteLines"]
check("035", "line totals of quote lines 1-11", "7164.00 1298.00 1512.00 2400.00 10146.00 1947.00 2280.00 2670.00 1194.00 649.00 399.00",
      " ".join(f"{line_total(ql[str(i)]):.2f}" for i in range(1, 12)))
for oid, said in [("2", "7000.00 09/12/26"), ("5", "86878.72 03/25/16"), ("7", "40042.08 02/15/16")]:
    ps = pays_by_order[oid]
    check("037", f"order {oid} paid and last payment", said, f'{sum(D(p["Amount"]) for p in ps):.2f} {max(ps, key=lambda p: date(p["PaymentDate"]))["PaymentDate"][:8]}')
check("037", "payments with no date or amount", 0, sum(1 for p in P if not p["PaymentDate"] or not p["Amount"]))
check("039", "payment count", 17374, len(P))
check("039", "payment methods seen", "ACH, Check, Credit Card, Wire", ", ".join(sorted({p["PaymentMethod"] for p in P})))

# 019 the inner-join quirk, and payment terms
check("019", "qryOrderBalances joins qryOrderPayments with", "INNER JOIN",
      "LEFT JOIN" if "FROM qryOrderTotals AS t LEFT JOIN qryOrderPayments" in BUILD else "INNER JOIN")
unpaid = [o for o in O if o["OrderStatus"] != "Cancelled" and not pays_by_order[o["OrderID"]] and totals(o, o_lines[o["OrderID"]])[2] > D("0.005")]
check("019", "unpaid open orders the claim would drop from receivables", 0, len(unpaid), link="R08")
check("019", "payment terms seen", "Net 30, Net 45", ", ".join(sorted({c["PaymentTerms"] for c in T["Customers"]})), "seen", "P02")
check("031", "payment terms seen", "Due on Receipt, Net 15, Net 30, Net 45", ", ".join(sorted({c["PaymentTerms"] for c in T["Customers"]})), "seen", "P02")

# 021 RMA 1 routing
r1 = by_id["RMAs"]["1"]
o25 = by_id["Orders"][r1["OrderID"]]
days = (date(r1["OpenedDate"]) - date(o25["ShipDate"] or o25["OrderDate"])).days
check("021", "RMA 1 days in service", 374, days)

# 025 the month-end cash query
check("025", "qryMonthEnd_CashReceived joins its tables with", "nothing (a cross join)",
      "INNER JOIN" if "FROM (Payments AS p INNER JOIN Orders AS o ON p.OrderID = o.OrderID)" in BUILD else "nothing (a cross join)")

# 027 Quotes
check("027", "quote count", 19389, len(Q))
check("027", "quotes whose ValidUntil isn't QuoteDate + 30", 0, sum(1 for q in Q if (date(q["ValidUntil"]) - date(q["QuoteDate"])).days != 30))
check("027", "quote statuses", "Accepted, Declined, Draft, Expired, Sent", ", ".join(sorted(Counter(q["QuoteStatus"] for q in Q))), link="R19, D01, D02")
check("027", "quote tax rates", "0, 0.0725, 0.0775, 0.0825", ", ".join(sorted({q["TaxRate"] for q in Q}, key=float)), "seen")

# 028 Orders
check("028", "order numbers not shaped SO-YYYY-NNNN", 0, sum(1 for o in O if not re.fullmatch(r"SO-\d{4}-\d{4}", o["OrderNumber"])))
check("028", "order statuses", "Cancelled, Closed, In Production, Invoiced, Open, Shipped", ", ".join(sorted(Counter(o["OrderStatus"] for o in O))), link="R19, D02")
check("028", "order tax rates", "0, 0.0725, 0.0825", ", ".join(sorted({o["TaxRate"] for o in O}, key=float)), "seen")

# 029 Activities
check("029", "activity count", "~68148", len(T["Activities"]), "approx")
check("029", "activity types seen", "Call, Email, Meeting, Note", ", ".join(sorted({a["ActivityType"] for a in T["Activities"]})))

# 031 Customers
check("031", "customer ids", "1..1000", f'{min(int(c) for c in cust)}..{max(int(c) for c in cust)}')
check("031", "customers with no assigned rep", 0, sum(1 for c in T["Customers"] if not c["AssignedRepID"]))
inactive = {r["RepID"] for r in T["SalesReps"] if r["Active"] != "1"}
check("031", "customers assigned to an inactive rep", 0, sum(1 for c in T["Customers"] if c["AssignedRepID"] in inactive))
check("031", "customer statuses", "Active, Inactive, Prospect", ", ".join(sorted(Counter(c["CustomerStatus"] for c in T["Customers"]))), "seen", "D01")
for cid, said in [("500", "Ridgeline Industries Inactive Net 15"), ("1000", "ALDER TRANSIT Active Net 30")]:
    c = cust[cid]
    check("031", f"customer {cid}", said, f'{c["CompanyName"]} {c["CustomerStatus"]} {c["PaymentTerms"]}')
check("031", "tax-exempt among customers 1-11", "1, 6, 8, 11", ", ".join(c for c in map(str, range(1, 12)) if cust[c]["TaxExempt"] == "1"))

# 032 CompanyInfo
check("032", "CompanyInfo rows", 1, len(T["CompanyInfo"]))
check("032", "manufacturer API URL", "http://localhost:8765/orders", T["CompanyInfo"][0]["ManufacturerApiUrl"])

# 040 QuoteLines shape
check("040", "largest QuoteID on a line", "~17319", max(int(l["QuoteID"]) for l in T["QuoteLines"]), "approx", "D04")
check("040", "quote line count", "~78623", len(T["QuoteLines"]), "approx")


if __name__ == "__main__":
    for note, claim, said, actual, verdict, link in sorted(results, key=lambda r: r[0]):
        print(f"{note}\t{verdict}\t{claim}\tsaid: {said}\tdata: {actual}" + (f"\t{link}" if link else ""))
    tally = Counter(r[4] for r in results)
    print(f"\n{len(results)} claims in {len({r[0] for r in results})} notes: " + ", ".join(f"{n} {v}" for v, n in tally.most_common()))
