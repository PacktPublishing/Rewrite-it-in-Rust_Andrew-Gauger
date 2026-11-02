#!/bin/sh
# Placeholder rows for laying out screens, after dev-seed.sh:
#
#   sh dev-ipsum.sh
#
# Replaces every business row (customers through RMAs) with generated
# "lorem ipsum" content at V1's History volumes: 1,000 customers, about
# 2,000 contacts, 21 products, 19,427 quotes, 13,531 orders, about 17,000
# payments, 68,257 activities, 4,638 phone messages and 2,192 RMAs. Keeps
# company_info, sales_reps and users from dev-seed.sh. The same seed gives
# the same rows every run. Value lists (statuses, terms, categories)
# are V1's. None of it is Cairn's data: Chapter 15 imports
# V1's history for real.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
psql -h "$here/.pgdata" -p "${CAIRN_PG_PORT:-5433}" -U cairn_owner -d cairn \
    -q -o /dev/null -v ON_ERROR_STOP=1 <<'SQL'
BEGIN;
SELECT setseed(0.42);

CREATE FUNCTION pg_temp.ipsum(n int) RETURNS text LANGUAGE plpgsql AS $$
DECLARE
    words text[] := ARRAY['lorem', 'ipsum', 'dolor', 'sit', 'amet',
        'consectetur', 'adipiscing', 'elit', 'sed', 'do', 'eiusmod',
        'tempor', 'incididunt', 'ut', 'labore', 'et', 'dolore', 'magna',
        'aliqua', 'enim', 'ad', 'minim', 'veniam', 'quis', 'nostrud',
        'exercitation', 'ullamco', 'laboris', 'nisi', 'aliquip', 'ex', 'ea',
        'commodo', 'consequat', 'duis', 'aute', 'irure', 'in',
        'reprehenderit', 'voluptate', 'velit', 'esse', 'cillum', 'fugiat',
        'nulla', 'pariatur', 'excepteur', 'sint', 'occaecat', 'cupidatat',
        'non', 'proident', 'sunt', 'culpa', 'qui', 'officia', 'deserunt',
        'mollit', 'anim', 'id', 'est', 'laborum'];
    out text[] := '{}';
BEGIN
    FOR i IN 1..n LOOP
        out := out || words[1 + floor(random() * cardinality(words))::int];
    END LOOP;
    RETURN array_to_string(out, ' ');
END $$;

CREATE FUNCTION pg_temp.pick(a text[]) RETURNS text LANGUAGE sql AS $$
    SELECT a[1 + floor(random() * cardinality(a))::int]
$$;

-- A day in V1's ten years, November 2016 to October 2026.
CREATE FUNCTION pg_temp.day() RETURNS date LANGUAGE sql AS $$
    SELECT date '2016-11-01' + floor(random() * 3652)::int
$$;

CREATE FUNCTION pg_temp.phone() RETURNS text LANGUAGE sql AS $$
    SELECT '555-' || lpad(floor(random() * 10000)::int::text, 4, '0')
$$;

-- Generated rows aren't edits, so they skip the audit trail.
ALTER TABLE customers DISABLE TRIGGER USER;
ALTER TABLE contacts DISABLE TRIGGER USER;
ALTER TABLE products DISABLE TRIGGER USER;
ALTER TABLE quotes DISABLE TRIGGER USER;
ALTER TABLE quote_lines DISABLE TRIGGER USER;
ALTER TABLE orders DISABLE TRIGGER USER;
ALTER TABLE order_lines DISABLE TRIGGER USER;
ALTER TABLE payments DISABLE TRIGGER USER;
ALTER TABLE activities DISABLE TRIGGER USER;
ALTER TABLE phone_messages DISABLE TRIGGER USER;
ALTER TABLE rmas DISABLE TRIGGER USER;

TRUNCATE customers, contacts, products, quotes, quote_lines, orders,
    order_lines, payments, activities, phone_messages, rmas,
    manufacturer_orders, document_counters, audit_log
    RESTART IDENTITY CASCADE;

INSERT INTO customers (company_name, industry, phone, email, website,
    bill_address, bill_city, bill_state, bill_zip, ship_address, ship_city,
    ship_state, ship_zip, assigned_rep_id, payment_terms, tax_exempt,
    customer_status, customer_since, notes)
SELECT name, pg_temp.pick(ARRAY['Agriculture', 'Food & Beverage',
        'Logistics', 'Utilities', 'Manufacturing', 'Healthcare',
        'Property Management', 'Education']),
    pg_temp.phone(), 'office@' || slug || '.example',
    'www.' || slug || '.example', addr, city, st, zip, addr, city, st, zip,
    1 + floor(random() * 14)::int,
    pg_temp.pick(ARRAY['Due on Receipt', 'Net 15', 'Net 30', 'Net 30',
        'Net 45', 'Net 60']),
    random() < 0.08,
    pg_temp.pick(ARRAY['Active', 'Active', 'Active', 'Prospect', 'Inactive']),
    pg_temp.day(), CASE WHEN random() < 0.3 THEN pg_temp.ipsum(8) END
FROM (
    SELECT n, initcap(pg_temp.ipsum(2)) || ' '
            || pg_temp.pick(ARRAY['Inc', 'LLC', 'Co', 'Ltd', 'Group'])
            AS name,
        floor(100 + random() * 9900)::int || ' '
            || initcap(pg_temp.ipsum(1)) || ' '
            || pg_temp.pick(ARRAY['St', 'Ave', 'Rd', 'Blvd', 'Way']) AS addr,
        initcap(pg_temp.ipsum(1)) AS city,
        pg_temp.pick(ARRAY['CA', 'OR', 'WA', 'NV', 'AZ', 'ID', 'UT']) AS st,
        lpad(floor(random() * 100000)::int::text, 5, '0') AS zip
    FROM generate_series(1, 1000) n
) g, LATERAL (SELECT 'lorem' || g.n AS slug) s;

INSERT INTO contacts (customer_id, first_name, last_name, title, email,
    phone, mobile, is_primary)
SELECT c.id, initcap(pg_temp.ipsum(1)), initcap(pg_temp.ipsum(1)),
    pg_temp.pick(ARRAY['Operations Manager', 'Facilities Lead', 'Buyer',
        'Plant Manager', 'Owner', 'IT Director']),
    'contact' || c.id || '-' || k || '@lorem' || c.id || '.example',
    pg_temp.phone(), pg_temp.phone(), k = 1
FROM customers c,
    LATERAL generate_series(1, 1 + floor(random() * 3 + c.id * 0)::int) k;

INSERT INTO products (sku, product_name, category, description, unit_price,
    unit_cost, firmware_version, lead_time_days, active)
SELECT 'LI-' || lpad(n::text, 4, '0'),
    initcap(pg_temp.ipsum(3)),
    pg_temp.pick(ARRAY['Sensor', 'Gateway', 'Module', 'Accessory',
        'Service', 'Kit']),
    pg_temp.ipsum(14), price, round(price * 0.4, 4),
    floor(1 + random() * 3)::int || '.' || floor(random() * 10)::int || '.'
        || floor(random() * 10)::int,
    7 * (1 + floor(random() * 6)::int), n <= 18
FROM generate_series(1, 21) n,
    LATERAL (SELECT round((20 + random() * 1200 + n * 0)::numeric, 2)
        AS price) p;

INSERT INTO quotes (quote_number, customer_id, contact_id, rep_id,
    quote_date, valid_until, quote_status, tax_rate, shipping_cost, terms,
    notes)
SELECT 'Q-' || extract(year FROM d)::int || '-' || lpad(
        (row_number() OVER (PARTITION BY extract(year FROM d) ORDER BY d, n))
            ::text, 4, '0'),
    c.id, (SELECT id FROM contacts WHERE customer_id = c.id AND is_primary),
    c.assigned_rep_id, d, d + 30,
    CASE WHEN n <= 13531 THEN 'Accepted'
        ELSE pg_temp.pick(ARRAY['Draft', 'Sent', 'Declined', 'Expired']) END,
    CASE WHEN c.tax_exempt THEN 0 ELSE 0.0825 END,
    round((random() * 80)::numeric, 2), pg_temp.ipsum(12),
    CASE WHEN random() < 0.2 THEN pg_temp.ipsum(6) END
FROM (
    SELECT n, pg_temp.day() AS d, 1 + floor(random() * 1000)::int AS cid
    FROM generate_series(1, 19427) n
) g
JOIN customers c ON c.id = g.cid
ORDER BY d, n;

INSERT INTO quote_lines (quote_id, product_id, line_description, quantity,
    unit_price, discount_pct)
SELECT q.id, p.id, p.product_name, 1 + floor(random() * 40)::int,
    p.unit_price,
    CASE WHEN random() < 0.25 THEN 0.05 * (1 + floor(random() * 3)) ELSE 0 END
FROM quotes q,
    LATERAL generate_series(1, 1 + floor(random() * 7 + q.id * 0)::int) k,
    LATERAL (SELECT * FROM products
             ORDER BY random() + q.id * 0 + k * 0 LIMIT 1) p;

-- The first 13,531 quotes were accepted, and each became an order.
INSERT INTO orders (order_number, customer_id, contact_id, rep_id,
    quote_id, customer_po, order_date, required_date, ship_date,
    order_status, tax_rate, shipping_cost, ship_via, tracking_number, notes)
SELECT 'SO-' || extract(year FROM od)::int || '-' || lpad(
        (row_number() OVER (PARTITION BY extract(year FROM od) ORDER BY od, q.id))
            ::text, 4, '0'),
    q.customer_id, q.contact_id, q.rep_id, q.id,
    'PO-' || floor(random() * 100000)::int, od, od + 21,
    CASE WHEN st IN ('Open', 'In Production', 'Cancelled') THEN NULL
        ELSE od + 14 END,
    st, q.tax_rate, q.shipping_cost,
    pg_temp.pick(ARRAY['UPS Ground', 'FedEx 2Day', 'Freight', 'Will Call']),
    CASE WHEN st IN ('Shipped', 'Invoiced', 'Closed')
        THEN '1Z' || upper(md5(q.id::text)) END,
    CASE WHEN random() < 0.15 THEN pg_temp.ipsum(6) END
FROM quotes q,
    LATERAL (SELECT CASE WHEN q.quote_date > date '2026-10-21'
        THEN q.quote_date
        ELSE q.quote_date + floor(random() * 10)::int END AS od) o,
    LATERAL (SELECT CASE
        WHEN od > date '2026-09-15' THEN pg_temp.pick(ARRAY['Open',
            'In Production', 'Shipped'])
        ELSE pg_temp.pick(ARRAY['Closed', 'Closed', 'Closed', 'Invoiced',
            'Cancelled']) END AS st) s
WHERE q.quote_status = 'Accepted'
ORDER BY od, q.id;

INSERT INTO order_lines (order_id, product_id, line_description, quantity,
    unit_price, discount_pct, serial_numbers)
SELECT o.id, l.product_id, l.line_description, l.quantity, l.unit_price,
    l.discount_pct,
    CASE WHEN o.ship_date IS NOT NULL
        THEN 'SN' || lpad(l.id::text, 7, '0') END
FROM orders o JOIN quote_lines l ON l.quote_id = o.quote_id
ORDER BY o.id, l.id;

INSERT INTO payments (order_id, payment_date, amount, payment_method,
    payment_ref, received_by_rep_id)
SELECT o.id, o.order_date + 30 * k,
    round((50 + random() * 5000)::numeric, 2),
    pg_temp.pick(ARRAY['Check', 'ACH', 'Wire', 'Credit Card', 'Cash']),
    'REF-' || floor(random() * 1000000)::int, 4
FROM orders o,
    LATERAL generate_series(1, 1 + floor(random() * 2 + o.id * 0)::int) k
WHERE o.order_status IN ('Shipped', 'Invoiced', 'Closed')
    AND o.order_date + 30 * k <= date '2026-10-31';

INSERT INTO activities (customer_id, contact_id, rep_id, activity_date,
    activity_type, subject, details, follow_up_date, completed)
SELECT c.id,
    (SELECT id FROM contacts WHERE customer_id = c.id AND is_primary),
    c.assigned_rep_id, t,
    pg_temp.pick(ARRAY['Note', 'Call', 'Email', 'Meeting', 'Task']),
    initcap(pg_temp.ipsum(4)), pg_temp.ipsum(20),
    CASE WHEN random() < 0.3 THEN t::date + 7 END, t < now() - interval '14 days'
FROM generate_series(1, 68257) n,
    LATERAL (SELECT 1 + floor(random() * 1000 + n * 0)::int AS cid) r
    JOIN customers c ON c.id = r.cid,
    LATERAL (SELECT pg_temp.day() + time '08:00'
        + floor(random() * 600 + n * 0) * interval '1 minute' AS t) a;

INSERT INTO phone_messages (received_at, for_rep_id, taken_by_rep_id,
    customer_id, caller_name, caller_company, caller_phone, message, urgent,
    please_call_back, will_call_again, message_status, returned_at)
SELECT t, 1 + floor(random() * 14)::int, 4, c.id,
    initcap(pg_temp.ipsum(2)), c.company_name, pg_temp.phone(),
    pg_temp.ipsum(15), random() < 0.1, random() < 0.7, random() < 0.2, st,
    CASE WHEN st <> 'New' THEN t + interval '3 hours' END
FROM generate_series(1, 4638) n,
    LATERAL (SELECT 1 + floor(random() * 1000 + n * 0)::int AS cid) r
    JOIN customers c ON c.id = r.cid,
    LATERAL (SELECT pg_temp.day() + time '08:00'
        + floor(random() * 600 + n * 0) * interval '1 minute' AS t) a,
    LATERAL (SELECT CASE WHEN t > now() - interval '7 days' THEN 'New'
        ELSE pg_temp.pick(ARRAY['Returned', 'Closed']) END AS st) s;

INSERT INTO rmas (rma_number, customer_id, order_id, product_id,
    serial_number, opened_date, opened_by_rep_id, reason, extended_warranty,
    description, rma_status, disposition, closed_date)
SELECT 'RMA-' || extract(year FROM d)::int || '-' || lpad(
        (row_number() OVER (PARTITION BY extract(year FROM d) ORDER BY d, n))
            ::text, 4, '0'),
    l.customer_id, CASE WHEN random() < 0.9 THEN l.order_id END,
    l.product_id, l.serial_numbers, d, l.rep_id,
    pg_temp.pick(ARRAY['Dead on Arrival', 'Failed in Warranty',
        'Damaged in Shipping', 'Wrong Item Shipped', 'Intermittent Fault',
        'Customer Damage']),
    random() < 0.1, pg_temp.ipsum(12), st,
    CASE WHEN st IN ('Closed', 'Replaced', 'Credited', 'Rejected')
        THEN initcap(pg_temp.ipsum(3)) END,
    CASE WHEN st IN ('Closed', 'Replaced', 'Credited', 'Rejected')
        THEN d + 20 END
FROM generate_series(1, 2192) n,
    LATERAL (SELECT 1 + floor(random() * (SELECT max(id) FROM order_lines)
        + n * 0)::int AS lid) r
    JOIN (SELECT ol.*, o.customer_id, o.rep_id, o.order_date, o.ship_date
          FROM order_lines ol JOIN orders o ON o.id = ol.order_id) l
        ON l.id = r.lid,
    LATERAL (SELECT least(coalesce(l.ship_date, l.order_date)
        + floor(random() * 300)::int, date '2026-10-31') AS d) dd,
    LATERAL (SELECT CASE WHEN d > date '2026-09-01'
        THEN pg_temp.pick(ARRAY['Open', 'Received', 'In Repair'])
        ELSE pg_temp.pick(ARRAY['Closed', 'Replaced', 'Credited',
            'Rejected']) END AS st) s
ORDER BY d, n;

-- New documents continue each year's numbering.
INSERT INTO document_counters (prefix, year, last_number)
SELECT prefix, year, max(n) FROM (
    SELECT split_part(quote_number, '-', 1) AS prefix,
        split_part(quote_number, '-', 2)::int AS year,
        split_part(quote_number, '-', 3)::int AS n FROM quotes
    UNION ALL
    SELECT split_part(order_number, '-', 1), split_part(order_number, '-', 2)::int,
        split_part(order_number, '-', 3)::int FROM orders
    UNION ALL
    SELECT split_part(rma_number, '-', 1), split_part(rma_number, '-', 2)::int,
        split_part(rma_number, '-', 3)::int FROM rmas
) d GROUP BY prefix, year;

ALTER TABLE customers ENABLE TRIGGER USER;
ALTER TABLE contacts ENABLE TRIGGER USER;
ALTER TABLE products ENABLE TRIGGER USER;
ALTER TABLE quotes ENABLE TRIGGER USER;
ALTER TABLE quote_lines ENABLE TRIGGER USER;
ALTER TABLE orders ENABLE TRIGGER USER;
ALTER TABLE order_lines ENABLE TRIGGER USER;
ALTER TABLE payments ENABLE TRIGGER USER;
ALTER TABLE activities ENABLE TRIGGER USER;
ALTER TABLE phone_messages ENABLE TRIGGER USER;
ALTER TABLE rmas ENABLE TRIGGER USER;
COMMIT;
ANALYZE customers, contacts, products, quotes, quote_lines, orders,
    order_lines, payments, activities, phone_messages, rmas;
SQL
psql -h "$here/.pgdata" -p "${CAIRN_PG_PORT:-5433}" -U cairn_owner -d cairn \
    -At -F ' ' -c "
SELECT 'customers', count(*) FROM customers UNION ALL
SELECT 'contacts', count(*) FROM contacts UNION ALL
SELECT 'products', count(*) FROM products UNION ALL
SELECT 'quotes', count(*) FROM quotes UNION ALL
SELECT 'quote_lines', count(*) FROM quote_lines UNION ALL
SELECT 'orders', count(*) FROM orders UNION ALL
SELECT 'order_lines', count(*) FROM order_lines UNION ALL
SELECT 'payments', count(*) FROM payments UNION ALL
SELECT 'activities', count(*) FROM activities UNION ALL
SELECT 'phone_messages', count(*) FROM phone_messages UNION ALL
SELECT 'rmas', count(*) FROM rmas"
