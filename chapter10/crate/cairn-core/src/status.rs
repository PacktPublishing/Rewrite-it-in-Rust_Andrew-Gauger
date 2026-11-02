//! V1's status value lists (`VL_QUOTESTATUS`, `VL_ORDERSTATUS`,
//! `VL_CUSTSTATUS` in CairnForms). The forms offer these. The tables accept
//! any text (R19), so text from V1 comes in through `TryFrom<&str>`, which
//! refuses anything else: `'Prospect'` with its quote marks (D01), or a
//! retired `Back Order` (D02).
//!
//! `QuoteStatus` and `OrderStatus` started as the earlier agent run's
//! (`part 2/ws34/artifacts/v2/src/{quotes,orders}.rs`), which got V1's lists
//! right.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownStatus(pub String);

impl fmt::Display for UnknownStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a status V1 offers: {:?}", self.0)
    }
}

impl std::error::Error for UnknownStatus {}

macro_rules! value_list {
    ($(#[$doc:meta])* $name:ident { $($variant:ident => $text:literal),* $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name { $($variant),* }

        impl $name {
            /// Every value, in the order V1's list offers them.
            pub const ALL: &'static [$name] = &[$($name::$variant),*];
        }

        impl TryFrom<&str> for $name {
            type Error = UnknownStatus;
            fn try_from(s: &str) -> Result<Self, UnknownStatus> {
                match s {
                    $($text => Ok($name::$variant),)*
                    _ => Err(UnknownStatus(s.to_string())),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(match self { $($name::$variant => $text),* })
            }
        }
    };
}

value_list!(
    /// `VL_QUOTESTATUS`
    QuoteStatus { Draft => "Draft", Sent => "Sent", Accepted => "Accepted", Declined => "Declined", Expired => "Expired" }
);

value_list!(
    /// `VL_ORDERSTATUS`
    OrderStatus {
        Open => "Open",
        InProduction => "In Production",
        Shipped => "Shipped",
        Invoiced => "Invoiced",
        Closed => "Closed",
        Cancelled => "Cancelled",
    }
);

value_list!(
    /// `VL_CUSTSTATUS`
    CustomerStatus { Prospect => "Prospect", Active => "Active", Inactive => "Inactive" }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_text_round_trips() {
        assert_eq!(
            OrderStatus::try_from("In Production"),
            Ok(OrderStatus::InProduction)
        );
        assert_eq!(
            OrderStatus::InProduction.to_string(),
            "In Production"
        );
    }

    #[test]
    fn quoted_prospect_is_not_a_prospect() {
        assert_eq!(
            CustomerStatus::try_from("'Prospect'"),
            Err(UnknownStatus("'Prospect'".into()))
        );
    }

    #[test]
    fn retired_statuses_are_refused() {
        assert!(OrderStatus::try_from("Back Order").is_err());
        assert!(QuoteStatus::try_from("On Hold").is_err());
    }
}
