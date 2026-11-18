"""V1's manufacturer call log, counted by order and attempt.

    python3 mfr-log-counts.py > mfr-log-counts.txt

Reads ManufacturerOrders from part 2/CairnSystems_History.accdb with
mdb-export (mdbtools).
"""
import collections
import csv
import io
import pathlib
import subprocess

db = pathlib.Path(__file__).resolve().parents[2] / "part 2" / "CairnSystems_History.accdb"
text = subprocess.run(["mdb-export", str(db), "ManufacturerOrders"],
                      capture_output=True, text=True, check=True).stdout
rows = list(csv.DictReader(io.StringIO(text)))
orders = collections.defaultdict(dict)
for r in rows:
    orders[r["OrderID"]][int(r["Attempt"])] = r

print(f"{len(rows):,} calls for {len(orders):,} orders, "
      f"{rows[0]['SentAt']} to {rows[-1]['SentAt']}")
first_failed = [a for a in orders.values() if not a[1]["ManufacturerRef"]]
print(f"{len(first_failed)} orders whose first attempt was not accepted:")
kinds = collections.Counter(
    (a[1]["HttpStatus"] or "no answer", a[1]["ErrorText"] or "",
     "accepted on attempt 2" if a.get(2, {}).get("ManufacturerRef") else "never confirmed")
    for a in first_failed)
for (status, error, outcome), n in sorted(kinds.items(), key=lambda kv: -kv[1]):
    print(f"  {n:4}  {status:9}  {error or '-':58}  {outcome}")
doubles = sum(1 for a in orders.values()
              if sum(1 for r in a.values() if r["ManufacturerRef"]) > 1)
print(f"orders with two manufacturer order numbers in the log: {doubles}")
