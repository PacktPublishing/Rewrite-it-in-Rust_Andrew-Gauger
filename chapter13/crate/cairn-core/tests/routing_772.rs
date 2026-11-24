//! All 772 returns in the History database, routed by V2 and by V1's rules.

mod common;

use cairn_core::{route, route_v1, Reason, Route};
use std::collections::HashMap;

fn count(
    routes: impl Iterator<Item = Route>,
) -> HashMap<Route, usize> {
    let mut n = HashMap::new();
    for r in routes {
        *n.entry(r).or_default() += 1;
    }
    n
}

#[test]
fn v1s_rules_reproduce_what_access_routes() {
    // chapter7/stage3-windows/results.md, check 1a, read from Access itself.
    let returns = common::all_returns();
    assert_eq!(returns.len(), 772);
    let n = count(returns.iter().map(|r| route_v1(&r.facts)));
    assert_eq!(n[&Route::ManufacturerRepair], 441);
    assert_eq!(n[&Route::CustomerPaysRepair], 104);
    assert_eq!(n[&Route::AdvanceReplacement], 103);
    assert_eq!(n[&Route::CarrierClaim], 64);
    assert_eq!(n[&Route::ScrapOutOfWarranty], 44);
    assert_eq!(n[&Route::AccountManager], 16);
    assert_eq!(n.get(&Route::BenchTest), None);
}

#[test]
fn v2_routes_23_returns_as_unknown_order_and_everything_else_as_v1() {
    let returns = common::all_returns();
    let n = count(returns.iter().map(|r| route(&r.facts)));
    assert_eq!(n[&Route::ManufacturerRepair], 441);
    assert_eq!(n[&Route::CustomerPaysRepair], 86);
    assert_eq!(n[&Route::AdvanceReplacement], 101);
    assert_eq!(n[&Route::CarrierClaim], 64);
    assert_eq!(n[&Route::ScrapOutOfWarranty], 44);
    assert_eq!(n[&Route::AccountManager], 13);
    assert_eq!(n[&Route::UnknownOrder], 23);
    assert_eq!(n.get(&Route::BenchTest), None);

    for r in &returns {
        if r.order_on_file {
            assert_eq!(
                route(&r.facts),
                route_v1(&r.facts),
                "{}",
                r.number
            );
        }
    }
}

#[test]
fn of_29_returns_with_no_order_6_are_carrier_claims_and_23_are_unknown(
) {
    let orphans: Vec<_> = common::all_returns()
        .into_iter()
        .filter(|r| !r.order_on_file)
        .collect();
    assert_eq!(orphans.len(), 29);
    let v1 = count(
        orphans
            .iter()
            .filter(|r| route(&r.facts) == Route::UnknownOrder)
            .map(|r| route_v1(&r.facts)),
    );
    assert_eq!(v1[&Route::CustomerPaysRepair], 18);
    assert_eq!(v1[&Route::AccountManager], 3);
    assert_eq!(v1[&Route::AdvanceReplacement], 2);
    assert!(orphans
        .iter()
        .filter(|r| route(&r.facts) != Route::UnknownOrder)
        .all(|r| r.facts.reason == Reason::DamagedInShipping
            && route(&r.facts) == Route::CarrierClaim));
}

#[test]
fn rma_2025_0045_has_no_order_and_v1_billed_the_customer() {
    let r = common::all_returns()
        .into_iter()
        .find(|r| r.number == "RMA-2025-0045")
        .unwrap();
    assert_eq!(r.facts.days_in_service, None);
    assert_eq!(route_v1(&r.facts), Route::CustomerPaysRepair);
    assert_eq!(route(&r.facts), Route::UnknownOrder);
}
