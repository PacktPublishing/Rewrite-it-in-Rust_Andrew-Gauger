#!/bin/sh
# Export the routing fixtures from the History database with mdbtools.
# Run from this folder: sh export_rma_fixtures.sh [path/to/CairnSystems_History.accdb]
set -eu
DB="${1:-../../../../../part 2/CairnSystems_History.accdb}"
F="-D %Y-%m-%d -T %Y-%m-%d"
mdb-export $F "$DB" RMAs > rmas.csv
mdb-export $F "$DB" Orders | python3 -I -c '
import csv, sys
r = csv.DictReader(sys.stdin); w = csv.writer(sys.stdout, lineterminator="\n")
w.writerow(["OrderID", "OrderDate", "ShipDate"])
for o in r: w.writerow([o["OrderID"], o["OrderDate"], o["ShipDate"]])' > rma_orders.csv
mdb-export "$DB" Products | python3 -I -c '
import csv, sys
r = csv.DictReader(sys.stdin); w = csv.writer(sys.stdout, lineterminator="\n")
w.writerow(["ProductID", "SKU", "Category"])
for p in r: w.writerow([p["ProductID"], p["SKU"], p["Category"]])' > products.csv
mdb-export "$DB" Customers | python3 -I -c '
import csv, sys
r = csv.DictReader(sys.stdin); w = csv.writer(sys.stdout, lineterminator="\n")
w.writerow(["CustomerID", "TaxExempt"])
for c in r: w.writerow([c["CustomerID"], c["TaxExempt"]])' > customers_tax.csv
