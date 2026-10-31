-- qryCommissionDetail + CommissionCriteria, translated by hand:
-- net sales = sum of round(qty * price * (1 - discount), 2) per order,
-- commission = round(net sales * rep rate, 2), every order dated in the
-- month counts unless Cancelled, paid or not.
WITH lines AS (
  SELECT orderid, sum(round(quantity * unitprice * (1 - discountpct::numeric), 2)) AS subtotal
  FROM orderlines GROUP BY orderid),
paid AS (SELECT orderid, sum(amount) AS paid FROM payments GROUP BY orderid),
o AS (
  SELECT o.orderid, o.orderstatus, o.repid,
         coalesce(l.subtotal, 0) AS subtotal,
         coalesce(p.paid, 0) AS paid
  FROM orders o
  LEFT JOIN lines l USING (orderid)
  LEFT JOIN paid p USING (orderid)
  WHERE o.orderdate >= '2026-09-01' AND o.orderdate < '2026-10-01')
SELECT count(*) FILTER (WHERE orderstatus <> 'Cancelled') AS orders_counted,
       count(*) FILTER (WHERE orderstatus = 'Cancelled') AS cancelled,
       sum(subtotal) FILTER (WHERE orderstatus <> 'Cancelled') AS net_sales,
       sum(round(subtotal * r.commissionrate::numeric, 2)) FILTER (WHERE orderstatus <> 'Cancelled') AS commission,
       count(*) FILTER (WHERE orderstatus <> 'Cancelled' AND paid = 0) AS orders_unpaid,
       sum(round(subtotal * r.commissionrate::numeric, 2)) FILTER (WHERE orderstatus <> 'Cancelled' AND paid = 0) AS commission_on_unpaid
FROM o JOIN salesreps r ON r.repid = o.repid;
