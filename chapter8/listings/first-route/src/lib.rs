//! The first, literal port of `qryRMARouting`, before the blanks.
//! `cairn-core`'s `rma.rs` is where it ends up.

use cairn_core::{Category, Reason};
use std::fmt;

/// Where a return goes. Nine branches, seven routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    CarrierClaim,
    AdvanceReplacement,
    AccountManager,
    ManufacturerRepair,
    BenchTest,
    ScrapOutOfWarranty,
    CustomerPaysRepair,
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
        })
    }
}

pub struct ReturnFacts {
    pub reason: Reason,
    pub category: Category,
    pub extended_warranty: bool,
    pub tax_exempt: bool,
    pub days_in_service: i32,
}

/// The query's nine branches, in its order. First match wins.
pub fn route(r: &ReturnFacts) -> Route {
    let days = r.days_in_service;
    let dead_on_arrival = r.reason == Reason::DeadOnArrival;

    if r.reason == Reason::DamagedInShipping {
        Route::CarrierClaim
    } else if dead_on_arrival && days <= 30 {
        Route::AdvanceReplacement
    } else if r.category == Category::Service {
        Route::AccountManager
    } else if days <= 365 || r.extended_warranty {
        Route::ManufacturerRepair
    } else if dead_on_arrival {
        Route::AdvanceReplacement
    } else if days <= 90 {
        Route::BenchTest
    } else if r.tax_exempt {
        Route::AccountManager
    } else if days > 730 {
        Route::ScrapOutOfWarranty
    } else {
        Route::CustomerPaysRepair
    }
}
