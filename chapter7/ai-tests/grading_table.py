#!/usr/bin/env python3
"""The per-test grading table for grading.md, and per-rule tallies for rules.md.

Test names are read from crate/tests/; the grades, rule IDs and reasons below
are the hand grading (2026-10-25). Rule IDs are ../rule-inventory/rules.md's.

Usage: python3 -I grading_table.py          the grading.md table
       python3 -I grading_table.py --rules  tests per rule: right/wrong/tautological
"""
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

TESTS = Path(__file__).resolve().parent / "crate" / "tests"

# file prefix -> (rule, kind, grade, reason) for every test in it, unless OVERRIDE says otherwise
FILES = {
    "u001": ("R12", "code", "right", "Format and max-plus-one match CairnForms L671-681"),
    "u004": ("R14", "code", "right", "Matches L731-733, including On Error Resume Next"),
    "u005": ("R36", "code", "right", "Same numbering as NextNumber; IsNull guards match L844-845"),
    "u006": ("R15", "code", "right", "Prices match Products"),
    "u007": ("R13", "code", "right", "Customers 1-3, tax, reps and primary contacts match the data"),
    "u010": ("R30", "code", "right", "Names and phones match Customers 1 and 2"),
    "u011": ("R31", "code", "right", "Matches L748-752"),
    "u012": ("R39", "code", "right", "Matches L927-942; invents seeding helpers"),
    "u013": ("R23", "code", "tautological", "Implements commission_criteria and sql_date in the test file, then tests its own code"),
    "u014": ("R26", "code", "right", "Matches IIf(Len(mPeriod), mPeriod, \"All dates\")"),
    "u015": ("R10", "code", "right", "Amounts match Payments 1-9"),
    "u020": ("R27", "code", "right", "Max of Sent quotes, grouped by customer"),
    "u021": ("R34", "code", "right", "Branch order matches the IIf chain"),
    "u027": ("R13", "code", "right", "Matches DocCustomerChanged L706-720"),
    "u028": ("-", "schema", "right", "Restates the table definition"),
    "u029": ("-", "schema", "right", "Restates the table definition"),
    "u031": ("-", "data", "right", "Matches the data"),
    "u032": ("-", "data", "right", "Matches the one CompanyInfo row"),
    "u033": ("-", "schema", "right", "Restates the table definition"),
    "u034": ("R23", "code", "right", "Format(d, \"\\#yyyy\\-mm\\-dd hh\\:nn\\:ss\\#\")"),
    "u036": ("-", "code", "right", "Matches the rep names"),
    "u037": ("R07", "code", "right", "Orders 1, 2, 5 and 7 match Payments"),
    "u038": ("R01", "code", "right", "Matches the saved SQL"),
    "u041": ("-", "schema", "right", "Restates the table definition"),
}

# test name -> (rule, kind, grade, reason); None keeps the file's value
OVERRIDE = {
    # u001
    "quote_2026_max_1699_yields_1700": (None, None, None, "Arithmetic right; the name's 1699 is false (2026's highest is Q-2026-1713)"),
    "order_2026_max_999_yields_1000": (None, None, None, "Arithmetic right; the name's 999 is false (2026's highest is SO-2026-1226)"),
    # u006, u011
    "inactive_product_11_still_gets_stored_price_79_00": (None, None, None, "True of the function; the form's Products list filters WHERE Active, so users can't reach it"),
    "inactive_product_12_still_gets_stored_price_449_00": (None, None, None, "True of the function; the form's Products list filters WHERE Active"),
    "message_returned_sets_status_to_returned": (None, None, None, "Right; its fixture uses a status, Pending, that doesn't exist"),
    # u020
    "open_quote_value_is_max_not_sum_across_sent_quotes": (None, None, None, "Right about the query; never asks whether Max is what the business means"),
    "open_quote_value_real_quote3_customer4_is_5332_68": ("R04", None, None, "Quote 3 recomputed from QuoteLines: 4,912.00 + 380.68 tax + 40.00 shipping"),
    "open_quote_value_zero_tax_is_subtotal_plus_shipping": ("R04", None, None, "Quote 18978 recomputed from QuoteLines: 19,586.00 + 103.00"),
    # u021
    "days_in_service_uses_ship_date_when_shipped": ("R33", None, None, "RMA 1: 2016-01-22 to 2017-01-30 is 374 days"),
    "days_in_service_falls_back_to_order_date_when_never_shipped": ("R33", None, None, "Nz(o.ShipDate, o.OrderDate)"),
    "bench_test_branch_is_dead_for_ordinary_inputs": (None, None, None, "Right, and it finds the unreachable Bench Test branch (a designed defect)"),
    "rma1_data_row_routes_to_customer_pays_repair": (None, None, None, "RMA 1 is 374 days, Intermittent Fault, Gateway: Customer Pays Repair"),
    # u027
    "quote_status_list_is_exactly_five_values_in_order": ("R19", None, None, "Matches VL_QUOTESTATUS; the data also holds On Hold (12) and 'Draft' (9)"),
    "quote_status_list_has_no_duplicates": ("R19", None, None, "Matches VL_QUOTESTATUS"),
    "default_quote_status_is_draft": ("R20", "schema", None, "Right today; misses that this default is how 'Draft' got its quote marks (D01)"),
    "default_valid_until_is_quote_date_plus_30_days_same_month": ("R16", None, None, "=Date()+30 (CairnForms L414); date arithmetic checked"),
    "default_valid_until_is_quote_date_plus_30_days_crossing_year": ("R16", None, None, "=Date()+30; date arithmetic checked"),
    "default_valid_until_is_quote_date_plus_30_days_leap_feb": ("R16", None, None, "=Date()+30; date arithmetic checked"),
    # u028
    "order_number_not_null_rejects_null": ("R45", None, None, "OrderNumber TEXT(20) NOT NULL"),
    "order_number_unique_rejects_duplicate": ("R45", None, None, "UQ_Orders_Number"),
    "customer_required_rejects_null": ("R43", None, None, "FK_Orders_Customer; NOT NULL"),
    "rep_required_rejects_null": ("R43", None, None, "FK_Orders_Rep; NOT NULL"),
    "contact_optional_allows_null_and_enforces_fk": ("R43", None, None, "FK_Orders_Contact, nullable"),
    "quote_optional_allows_null_and_enforces_fk": ("R43", None, None, "FK_Orders_Quote, nullable"),
    "default_status_is_open": ("R20", None, None, "DEFAULT 'Open' (CairnBuild L284)"),
    "default_tax_rate_is_zero": ("R20", None, None, "DEFAULT 0 (CairnBuild L284)"),
    "status_list_is_the_six_observed_statuses": ("R19", "code", None, "Matches VL_ORDERSTATUS; never checks the data, which also holds Back Order (31)"),
    # u029
    "insert_valid_activity_succeeds": ("R43", None, None, "Row 1's references all exist"),
    "insert_null_customer_rejected": ("R43", None, None, "customerid NOT NULL"),
    "insert_nonexistent_customer_rejected": ("R43", None, None, "FK to Customers"),
    "insert_null_rep_rejected": ("R43", None, None, "repid NOT NULL"),
    "insert_nonexistent_rep_rejected": ("R43", None, None, "FK to SalesReps"),
    "insert_null_contact_accepted": ("R43", None, None, "contactid nullable"),
    "insert_nonexistent_contact_rejected": ("R43", None, None, "FK to Contacts"),
    "delete_customer_cascades_to_its_activities": ("R44", None, "wrong", "Customer 2 has 7 orders and 10 quotes, and those foreign keys don't cascade, so the delete is refused"),
    "delete_customer_with_no_activities_succeeds": ("R44", None, "wrong", "Customer 1000 has 2 orders, so the delete is refused"),
    # u031
    "every_customer_has_non_null_assigned_rep": (None, None, None, "0 of 1,000 null; its evidence was an IS NULL query mdb-sql can't run"),
    "no_customer_is_assigned_to_an_inactive_rep": (None, None, None, "No customer points at reps 5, 6, 8 or 10"),
    "customer_status_is_one_of_active_prospect_inactive": ("R19", None, "wrong", "4 customers are saved as 'Prospect' with the quote marks (D01)"),
    "payment_terms_is_one_of_the_known_values": ("R19", None, "wrong", "Leaves out Net 60, which the form's own list offers (VL_TERMS) and 17 customers have"),
    "taxexempt_is_a_boolean": (None, None, "tautological", "Calls is_tax_exempt and asserts nothing"),
    "customer_ids_are_unique_and_within_1_to_1000": (None, None, None, "True of today's 1,000 rows; it pins a count, not a rule"),
    "sample_rows_match_observed_data": (None, None, None, "Customers 1, 2, 4, 7, 19, 500, 1000 match"),
    # u032
    "default_tax_rate_is_8_25_percent": ("R13", None, None, "DefaultTaxRate 0.0825"),
    "default_tax_rate_falls_back_to_zero_when_unset": ("R13", "code", None, "Nz(DLookup(\"DefaultTaxRate\",...),0) (CairnForms L712)"),
    "quote_terms_fall_back_to_company_default_when_quote_terms_null": ("R17", "code", None, "Nz(b.Terms, ci.QuoteTerms); terms text matches"),
    "manufacturer_order_aborts_with_message_when_url_empty": ("R37", "code", None, "Message matches CairnForms L861"),
    "seller_city_state_zip_joins_city_comma_space_state_space_zip": (None, "code", None, "Matches the print queries"),
    # u033
    "contact_requires_a_customer": ("R43", None, None, "customerid NOT NULL"),
    "contact_customer_must_exist": ("R43", None, None, "FK_Contacts_Customer"),
    "deleting_customer_cascades_to_its_contacts": ("R44", None, None, "ON DELETE CASCADE; the test's customer has no orders"),
    "exactly_one_primary_contact_per_customer": ("R13", "data", None, "All 1,000 customers have exactly one primary contact"),
    "customer_without_contacts_has_no_primary": ("R13", None, None, "DLookup returns Null"),
    "isprimary_is_required": (None, None, "tautological", "Creates a contact with isprimary=true and reads it back; tests nothing about NOT NULL"),
    # u036
    "commission_is_net_sales_times_rate_rounded_to_cents": ("R21", None, None, "Round(Subtotal*CommissionRate, 2); no half-cent case, so R05 goes untested"),
    "new_rep_defaults_are_5_percent_rate_and_active": ("R20", "schema", None, "DEFAULT 0.05, Active DEFAULT -1 (CairnBuild L231-232)"),
    "commission_rate_displays_as_percent_of_net_sales": (None, None, None, "Format([CommissionRate],\"0.0%\") & \" of net sales\""),
    # u037
    "order_without_payments_produces_no_row": (None, None, None, "Right for this query alone; the LEFT JOIN that keeps the order lives in qryOrderBalances"),
    # u038
    "item_description_falls_back_to_product_name_when_line_description_is_null": ("R02", None, None, "Nz(ol.LineDescription, p.ProductName)"),
    "item_description_prefers_line_description_when_present": ("R02", None, None, "Nz(ol.LineDescription, p.ProductName)"),
    "line_total_is_discounted_total_rounded_to_two_decimals": (None, None, None, "Right; no half-cent case, so R05 goes untested"),
    "line_total_rounds_to_two_decimals": (None, None, None, "94.9905 to 94.99; not a tie, so R05 goes untested"),
    "line_cost_uses_current_product_unit_cost": ("R06", None, None, "ol.Quantity*p.UnitCost"),
    "line_with_no_matching_product_is_dropped_by_inner_join": ("-", None, None, "INNER JOIN Products"),
    # u041
    "line_without_existing_order_is_rejected": ("R43", None, None, "FK to Orders"),
    "line_without_existing_product_is_rejected": ("R43", None, None, "FK to Products"),
    "deleting_order_cascades_to_its_lines": ("R44", None, None, "ON DELETE CASCADE"),
    "deleting_product_with_lines_is_blocked": ("R44", None, None, "No cascade on the Products key"),
    "duplicate_orderlineid_is_rejected": (None, None, "tautological", "Inserts three rows and checks the ids differ, a repeat of test 1; never tries a duplicate"),
}


def rows():
    for f in sorted(TESTS.glob("u*.rs")):
        default = FILES[f.name[:4]]
        for name in re.findall(r"#\[test\]\s*fn (\w+)", f.read_text()):
            o = OVERRIDE.get(name, (None,) * 4)
            yield (f.name[:4], name) + tuple(o[i] if o[i] is not None else default[i] for i in range(4))


if "--rules" in sys.argv:
    tally = defaultdict(Counter)
    for _, _, rule, _, grade, _ in rows():
        tally[rule][grade] += 1
    for rule in sorted(tally):
        print(rule, dict(tally[rule]))
else:
    print("| # | File | Test | Rule | Kind | Grade | Reason |")
    print("|---|---|---|---|---|---|---|")
    for n, (f, name, rule, kind, grade, reason) in enumerate(rows(), 1):
        grade = grade if grade == "right" else f"**{grade}**"
        print(f"| {n} | `{f}` | `{name}` | {rule} | {kind} | {grade} | {reason} |")
