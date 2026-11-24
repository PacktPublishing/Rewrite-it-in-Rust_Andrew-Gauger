//! Return routing, V1's `qryRMARouting`: nine `IIf` branches, first match
//! wins, read in order.

use crate::date::Date;
use crate::product::Category;
use serde::{Deserialize, Serialize};
use std::fmt;

/// `VL_RMAREASON`. The `Reason` column accepts any text; these are the six
/// the form offers.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub enum Reason {
    DeadOnArrival,
    FailedInWarranty,
    DamagedInShipping,
    WrongItemShipped,
    IntermittentFault,
    CustomerDamage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownReason(pub String);

impl fmt::Display for UnknownReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a return reason V1 offers: {:?}", self.0)
    }
}

impl std::error::Error for UnknownReason {}

impl TryFrom<&str> for Reason {
    type Error = UnknownReason;

    fn try_from(text: &str) -> Result<Reason, UnknownReason> {
        match text {
            "Dead on Arrival" => Ok(Reason::DeadOnArrival),
            "Failed in Warranty" => Ok(Reason::FailedInWarranty),
            "Damaged in Shipping" => Ok(Reason::DamagedInShipping),
            "Wrong Item Shipped" => Ok(Reason::WrongItemShipped),
            "Intermittent Fault" => Ok(Reason::IntermittentFault),
            "Customer Damage" => Ok(Reason::CustomerDamage),
            _ => Err(UnknownReason(text.to_string())),
        }
    }
}

/// Where a return goes. V1 has nine branches and seven routes; Advance
/// Replacement and Account Manager each appear twice.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub enum Route {
    CarrierClaim,
    AdvanceReplacement,
    AccountManager,
    ManufacturerRepair,
    BenchTest,
    ScrapOutOfWarranty,
    CustomerPaysRepair,
    UnknownOrder,
}

impl fmt::Display for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Route::CarrierClaim => "Carrier Claim",
            Route::AdvanceReplacement => "Advance Replacement",
            Route::AccountManager => "Account Manager",
            Route::ManufacturerRepair => "Manufacturer Repair",
            Route::BenchTest => "Bench Test",
            Route::ScrapOutOfWarranty => "Scrap - Out of Warranty",
            Route::CustomerPaysRepair => "Customer Pays Repair",
            Route::UnknownOrder => "Unknown Order",
        })
    }
}

/// What the routing query reads about one return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnFacts {
    pub reason: Reason,
    pub category: Category,
    pub extended_warranty: bool,
    pub tax_exempt: bool,
    /// `None` when the return's order isn't in the file.
    pub days_in_service: Option<i32>,
}

/// R33: from the order's ship date, or its order date if it never shipped,
/// to the day the return was opened. `None` when there is no order.
pub fn days_in_service(
    opened: Date,
    order: Option<(Date, Option<Date>)>,
) -> Option<i32> {
    let (order_date, ship_date) = order?;
    let from = ship_date.unwrap_or(order_date);
    Some(opened.days_since(from) as i32)
}

/// R34, in V1's branch order.
pub fn route(r: &ReturnFacts) -> Route {
    let dead_on_arrival = r.reason == Reason::DeadOnArrival;

    if r.reason == Reason::DamagedInShipping {
        return Route::CarrierClaim;
    }
    if dead_on_arrival {
        // FIX: R35 V1 compares a blank to 30 and reads the Null as false.
        let Some(days) = r.days_in_service else {
            return Route::UnknownOrder;
        };
        if days <= 30 {
            return Route::AdvanceReplacement;
        }
    }
    if r.category == Category::Service {
        return Route::AccountManager;
    }
    if r.extended_warranty {
        return Route::ManufacturerRepair;
    }
    // FIX: R35 every branch below needs the days.
    let Some(days) = r.days_in_service else {
        return Route::UnknownOrder;
    };
    if days <= 365 {
        return Route::ManufacturerRepair;
    }
    if dead_on_arrival {
        return Route::AdvanceReplacement;
    }
    // FLAG: R34 Bench Test is unreachable: the 365-day branch above
    // takes every return of 90 days or less. 0 of 772 route here.
    if days <= 90 {
        return Route::BenchTest;
    }
    if r.tax_exempt {
        return Route::AccountManager;
    }
    if days > 730 {
        return Route::ScrapOutOfWarranty;
    }
    Route::CustomerPaysRepair
}

/// V1's answer exactly, Null fall-through included, for parity checks
/// against Access. Never `UnknownOrder`. A comparison against a missing
/// number is false, the way `IIf` treats Null.
pub fn route_v1(r: &ReturnFacts) -> Route {
    let days = r.days_in_service;
    let at_most = |n| days.is_some_and(|d| d <= n);
    let more_than = |n| days.is_some_and(|d| d > n);
    let dead_on_arrival = r.reason == Reason::DeadOnArrival;

    if r.reason == Reason::DamagedInShipping {
        Route::CarrierClaim
    } else if dead_on_arrival && at_most(30) {
        Route::AdvanceReplacement
    } else if r.category == Category::Service {
        Route::AccountManager
    } else if at_most(365) || r.extended_warranty {
        Route::ManufacturerRepair
    } else if dead_on_arrival {
        Route::AdvanceReplacement
    } else if at_most(90) {
        Route::BenchTest
    } else if r.tax_exempt {
        Route::AccountManager
    } else if more_than(730) {
        Route::ScrapOutOfWarranty
    } else {
        Route::CustomerPaysRepair
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(reason: Reason, days: Option<i32>) -> ReturnFacts {
        ReturnFacts {
            reason,
            category: Category::Sensor,
            extended_warranty: false,
            tax_exempt: false,
            days_in_service: days,
        }
    }

    fn both(r: &ReturnFacts) -> (Route, Route) {
        (route(r), route_v1(r))
    }

    #[test]
    fn damaged_in_shipping_goes_to_the_carrier_whatever_its_age() {
        for days in [Some(1), Some(800), None] {
            let r = facts(Reason::DamagedInShipping, days);
            assert_eq!(
                both(&r),
                (Route::CarrierClaim, Route::CarrierClaim)
            );
        }
    }

    #[test]
    fn dead_on_arrival_within_30_days_gets_a_replacement() {
        let r = facts(Reason::DeadOnArrival, Some(30));
        assert_eq!(route(&r), Route::AdvanceReplacement);
    }

    #[test]
    fn dead_on_arrival_after_30_days_is_repaired_within_a_year() {
        let r = facts(Reason::DeadOnArrival, Some(31));
        assert_eq!(route(&r), Route::ManufacturerRepair);
    }

    #[test]
    fn dead_on_arrival_after_a_year_still_gets_a_replacement() {
        let r = facts(Reason::DeadOnArrival, Some(366));
        assert_eq!(route(&r), Route::AdvanceReplacement);
    }

    #[test]
    fn a_service_line_goes_to_the_account_manager() {
        let r = ReturnFacts {
            category: Category::Service,
            ..facts(Reason::FailedInWarranty, Some(10))
        };
        assert_eq!(route(&r), Route::AccountManager);
    }

    #[test]
    fn manufacturer_repairs_anything_up_to_a_year() {
        assert_eq!(
            route(&facts(Reason::FailedInWarranty, Some(365))),
            Route::ManufacturerRepair
        );
        assert_eq!(
            route(&facts(Reason::FailedInWarranty, Some(366))),
            Route::CustomerPaysRepair
        );
    }

    #[test]
    fn extended_warranty_is_repaired_at_any_age() {
        let r = ReturnFacts {
            extended_warranty: true,
            ..facts(Reason::FailedInWarranty, Some(900))
        };
        assert_eq!(route(&r), Route::ManufacturerRepair);
    }

    #[test]
    fn extended_warranty_is_repaired_even_with_no_order_on_file() {
        let r = ReturnFacts {
            extended_warranty: true,
            ..facts(Reason::FailedInWarranty, None)
        };
        assert_eq!(
            both(&r),
            (Route::ManufacturerRepair, Route::ManufacturerRepair)
        );
    }

    #[test]
    fn nothing_reaches_bench_test() {
        for days in 0..=90 {
            assert_ne!(
                route(&facts(Reason::IntermittentFault, Some(days))),
                Route::BenchTest
            );
        }
    }

    #[test]
    fn a_tax_exempt_customer_after_a_year_goes_to_the_account_manager(
    ) {
        let r = ReturnFacts {
            tax_exempt: true,
            ..facts(Reason::FailedInWarranty, Some(800))
        };
        assert_eq!(route(&r), Route::AccountManager);
    }

    #[test]
    fn scrap_starts_after_730_days() {
        assert_eq!(
            route(&facts(Reason::FailedInWarranty, Some(730))),
            Route::CustomerPaysRepair
        );
        assert_eq!(
            route(&facts(Reason::FailedInWarranty, Some(731))),
            Route::ScrapOutOfWarranty
        );
    }

    #[test]
    fn no_order_on_file_is_unknown_not_customer_pays() {
        let r = facts(Reason::FailedInWarranty, None);
        assert_eq!(
            both(&r),
            (Route::UnknownOrder, Route::CustomerPaysRepair)
        );
    }

    #[test]
    fn dead_on_arrival_with_no_order_is_unknown_not_a_replacement() {
        let r = facts(Reason::DeadOnArrival, None);
        assert_eq!(
            both(&r),
            (Route::UnknownOrder, Route::AdvanceReplacement)
        );
    }

    #[test]
    fn tax_exempt_with_no_order_is_unknown_not_the_account_manager() {
        let r = ReturnFacts {
            tax_exempt: true,
            ..facts(Reason::FailedInWarranty, None)
        };
        assert_eq!(
            both(&r),
            (Route::UnknownOrder, Route::AccountManager)
        );
    }

    #[test]
    fn days_run_from_ship_date_or_order_date() {
        let d = |s: &str| s.parse::<Date>().unwrap();
        let opened = d("2026-10-09");
        assert_eq!(
            days_in_service(
                opened,
                Some((d("2026-09-01"), Some(d("2026-09-09"))))
            ),
            Some(30)
        );
        assert_eq!(
            days_in_service(opened, Some((d("2026-09-01"), None))),
            Some(38)
        );
        assert_eq!(days_in_service(opened, None), None);
    }

    #[test]
    fn reasons_parse_from_v1_text_only() {
        assert_eq!(
            Reason::try_from("Dead on Arrival"),
            Ok(Reason::DeadOnArrival)
        );
        assert!(Reason::try_from("dead on arrival").is_err());
    }

    #[test]
    fn routes_print_v1_text() {
        assert_eq!(
            Route::ScrapOutOfWarranty.to_string(),
            "Scrap - Out of Warranty"
        );
    }
}
