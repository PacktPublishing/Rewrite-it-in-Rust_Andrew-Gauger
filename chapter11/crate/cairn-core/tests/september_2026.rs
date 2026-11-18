//! What Access printed for September 2026. The oracle is the contract: these
//! figures are V1's, and each one fails under plain half-up rounding.

mod common;

use cairn_core::commission::all_reps;
use cairn_core::{
    access_round_product, sales_by_rep_commission, statement, Date,
    RepId,
};
use rust_decimal::Decimal;
use std::str::FromStr;

const CARLA: RepId = RepId(7);

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

fn september() -> Date {
    "2026-09-01".parse().unwrap()
}

fn carla_rate(s: &common::September) -> f64 {
    s.rates.iter().find(|(rep, _)| *rep == CARLA).unwrap().1
}

#[test]
fn carlas_september_statement_is_25_701_07_with_328_267_47_unpaid() {
    let s = common::september_2026();
    let st = statement(&s.orders, CARLA, carla_rate(&s), september());
    assert_eq!(st.commission, dec("25701.07"));
    assert_eq!(st.unpaid, dec("328267.47"));
}

#[test]
fn sales_by_rep_says_25_701_06_for_the_same_month() {
    let s = common::september_2026();
    assert_eq!(
        sales_by_rep_commission(
            &s.orders,
            CARLA,
            carla_rate(&s),
            september()
        ),
        dec("25701.06")
    );
}

#[test]
fn septembers_commission_is_183_280_48_on_136_orders_82_of_them_unpaid(
) {
    let s = common::september_2026();
    let total = all_reps(&s.orders, &s.rates, september());
    assert_eq!(total.commission, dec("183280.48"));
    assert_eq!(total.orders, 136);
    assert_eq!(total.orders_with_no_payment, 82);
}

#[test]
fn so_2026_1053_is_taxed_877_10() {
    let s = common::september_2026();
    let (net_sales, rate) = s.tax_inputs["SO-2026-1053"];
    assert_eq!(net_sales, dec("12098.00"));
    assert_eq!(access_round_product(net_sales, rate), dec("877.10"));
}
