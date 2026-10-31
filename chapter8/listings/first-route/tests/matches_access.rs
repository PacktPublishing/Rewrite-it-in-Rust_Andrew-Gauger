//! The first port against Access, on every return it can take.

#[path = "../../../crate/cairn-core/tests/common/mod.rs"]
mod common;

use first_route::{route, ReturnFacts};

#[test]
fn every_return_with_an_order_routes_as_access_routes_it() {
    let returns = common::all_returns();
    let mut checked = 0;
    for r in &returns {
        let Some(days) = r.facts.days_in_service else {
            continue;
        };
        let first = ReturnFacts {
            reason: r.facts.reason,
            category: r.facts.category,
            extended_warranty: r.facts.extended_warranty,
            tax_exempt: r.facts.tax_exempt,
            days_in_service: days,
        };
        assert_eq!(
            route(&first).to_string(),
            cairn_core::route_v1(&r.facts).to_string(),
            "{}",
            r.number
        );
        checked += 1;
    }
    assert_eq!((returns.len(), checked), (772, 743));
}
