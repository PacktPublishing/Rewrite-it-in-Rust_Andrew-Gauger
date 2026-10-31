#!/bin/sh
# A few rows from V1 for trying V2 by hand, after the migrations:
#
#   sh dev-seed.sh yournamegmail.onmicrosoft.com
#
# Loads V1's company letterhead and its 14 sales reps (with their V1 IDs),
# and links four of them to the test tenant's users from the errata's
# chapter9/cairn-auth/setup-entra.sh. Ben Hartley is inactive, as in V1.
# Also loads the customer and product from RMA-2025-0045 (Chapter 8), and
# one quote. Chapter 15 imports the rest of V1's history.
set -eu
domain=${1:?usage: sh dev-seed.sh <domain>.onmicrosoft.com}
here=$(cd "$(dirname "$0")" && pwd)
psql -h "$here/.pgdata" -p "${CAIRN_PG_PORT:-5433}" -U cairn_owner -d cairn \
    -q -o /dev/null -v ON_ERROR_STOP=1 -v domain="$domain" <<'SQL'
BEGIN;
INSERT INTO company_info (company_name, country, email, website,
    default_tax_rate, quote_terms, receipt_footer, manufacturer_api_url)
VALUES ('Cairn Systems', 'USA', 'sales@cairnsystems.example',
    'www.cairnsystems.example', 0.0825,
    'Prices valid 30 days from quote date. Net 30 on approved credit. Standard lead time 3-4 weeks ARO. Firmware updates included for 12 months.',
    'Thank you for your business! Questions about this receipt? Contact your account representative.',
    'http://localhost:8765/orders');

INSERT INTO sales_reps (id, first_name, last_name, title, email, phone,
    extension, commission_rate, hire_date, active) VALUES
 (1, 'Maya', 'Torres', 'Account Executive', 'maya.torres@cairnsystems.example', '555-0101', '101', 0.06, '2022-03-14', true),
 (2, 'Derek', 'Liu', 'Account Executive', 'derek.liu@cairnsystems.example', '555-0102', '102', 0.06, '2023-06-05', true),
 (3, 'Priya', 'Shah', 'Sales Manager', 'priya.shah@cairnsystems.example', '555-0100', '100', 0.04, '2020-09-01', true),
 (4, 'Sam', 'Okafor', 'Inside Sales / Reception', 'sam.okafor@cairnsystems.example', '555-0110', '110', 0.03, '2024-01-08', true),
 (5, 'Raj', 'Malhotra', 'Sales Director', 'raj.malhotra@cairnsystems.example', '555-0160', '166', 0.04, '2013-03-01', false),
 (6, 'Ben', 'Hartley', 'Account Executive', 'ben.hartley@cairnsystems.example', '555-0195', '198', 0.06, '2014-02-03', false),
 (7, 'Carla', 'Mendes', 'Senior Account Executive', 'carla.mendes@cairnsystems.example', '555-0175', '163', 0.065, '2015-05-11', true),
 (8, 'Owen', 'Fitch', 'Account Executive', 'owen.fitch@cairnsystems.example', '555-0178', '197', 0.06, '2016-01-04', false),
 (9, 'Nadia', 'Petrova', 'Senior Account Executive', 'nadia.petrova@cairnsystems.example', '555-0199', '128', 0.065, '2017-08-21', true),
 (10, 'Tessa', 'Grant', 'Account Executive', 'tessa.grant@cairnsystems.example', '555-0194', '189', 0.06, '2018-04-09', false),
 (11, 'Victor', 'Huang', 'Account Executive', 'victor.huang@cairnsystems.example', '555-0198', '143', 0.06, '2019-10-14', true),
 (12, 'Lena', 'Brandt', 'Account Executive', 'lena.brandt@cairnsystems.example', '555-0128', '150', 0.06, '2021-01-11', true),
 (13, 'Marcus', 'Doyle', 'Account Executive', 'marcus.doyle@cairnsystems.example', '555-0171', '123', 0.055, '2024-07-15', true),
 (14, 'Elise', 'Moreau', 'Account Executive', 'elise.moreau@cairnsystems.example', '555-0121', '157', 0.055, '2025-03-03', true);
SELECT setval(pg_get_serial_sequence('sales_reps', 'id'), 14);

UPDATE sales_reps SET entra_upn = u.name || '@' || :'domain'
FROM (VALUES (1, 'mtorres'), (2, 'dliu'), (3, 'pshah'), (6, 'bhartley'))
    AS u (id, name)
WHERE sales_reps.id = u.id;

INSERT INTO customers (id, company_name, industry, phone, email, website,
    bill_address, bill_city, bill_state, bill_zip, ship_address, ship_city,
    ship_state, ship_zip, assigned_rep_id, payment_terms, customer_status,
    customer_since, notes) VALUES
 (766, 'Shasta Creamery', 'Food & Beverage', '555-3383',
  'office@shastacreamery.example', 'www.shastacreamery.example',
  '1902 Enterprise Rd', 'Merced', 'CA', '92488',
  'Dock 6, 1902 Enterprise Rd', 'Merced', 'CA', '92488', 12, 'Net 60',
  'Active', '2026-03-10', 'dist 15%');
SELECT setval(pg_get_serial_sequence('customers', 'id'), 766);

INSERT INTO products (id, sku, product_name, category, unit_price,
    unit_cost, firmware_version, lead_time_days, active) VALUES
 (11, 'CS-TH050', 'Temperature/Humidity Sensor, Gen 1 (LoRaWAN)', 'Sensor',
  79.0000, 32.0000, '1.8.4', 21, false);
SELECT setval(pg_get_serial_sequence('products', 'id'), 11);

INSERT INTO quotes (customer_id, rep_id, tax_rate)
VALUES (766, 1, 0.0825);
COMMIT;
SQL
echo "Seeded. Reps linked to Entra: mtorres, dliu, pshah, bhartley @$domain"
