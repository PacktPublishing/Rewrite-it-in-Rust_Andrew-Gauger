//! R28, V1's `qryQualifiedLeads`, with `qryOpenQuoteValue` (R27) and
//! `qryRecentTouches`. The one rule in the file with a comment saying it was
//! decided: "what sales management decided counts, written down once".

use crate::date::Date;
use crate::status::{CustomerStatus, QuoteStatus};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

const LEAD_INDUSTRIES: [&str; 3] =
    ["Utilities", "Food & Beverage", "Healthcare"];

/// What the query reads about one customer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeadFacts {
    pub status: CustomerStatus,
    pub industry: String,
    /// `None` with no Sent quote.
    pub open_quote_value: Option<Decimal>,
    pub recent_touches: usize,
    pub contact_titles: Vec<String>,
}

/// R27: the largest Sent quote, not the total of the Sent quotes.
pub fn open_quote_value(
    quotes: &[(QuoteStatus, Decimal)],
) -> Option<Decimal> {
    quotes
        .iter()
        .filter(|(s, _)| *s == QuoteStatus::Sent)
        .map(|&(_, total)| total)
        .max()
}

/// Activities dated on or after 90 days before today. Future dates count.
pub fn recent_touches(activity_dates: &[Date], today: Date) -> usize {
    let since = today.add_days(-90);
    activity_dates.iter().filter(|&&d| d >= since).count()
}

// FLAG: R28 A qualified lead is a Prospect with a Director or Manager on file, and a Sent quote of $5,000 or more or 2 touches in 90 days in three industries. An existing customer buying more never counts; neither does a business whose contact is the owner.
pub fn is_qualified_lead(f: &LeadFacts) -> bool {
    let big_quote =
        f.open_quote_value.is_some_and(|v| v >= Decimal::from(5000));
    let warm = f.recent_touches >= 2
        && LEAD_INDUSTRIES.contains(&f.industry.as_str());
    // Access's Like is case-insensitive.
    let decision_maker = f.contact_titles.iter().any(|t| {
        let t = t.to_lowercase();
        t.contains("director") || t.contains("manager")
    });
    f.status == CustomerStatus::Prospect
        && (big_quote || warm)
        && decision_maker
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lead() -> LeadFacts {
        LeadFacts {
            status: CustomerStatus::Prospect,
            industry: "Manufacturing".into(),
            open_quote_value: Some(Decimal::from(5000)),
            recent_touches: 0,
            contact_titles: vec!["Plant Manager".into()],
        }
    }

    #[test]
    fn a_prospect_with_a_5000_quote_and_a_manager_qualifies() {
        assert!(is_qualified_lead(&lead()));
    }

    #[test]
    fn a_4999_quote_does_not() {
        assert!(!is_qualified_lead(&LeadFacts {
            open_quote_value: Some(Decimal::new(499_999, 2)),
            ..lead()
        }));
    }

    #[test]
    fn two_touches_qualify_only_in_three_industries() {
        let warm = LeadFacts {
            open_quote_value: None,
            recent_touches: 2,
            ..lead()
        };
        assert!(!is_qualified_lead(&warm));
        assert!(is_qualified_lead(&LeadFacts {
            industry: "Healthcare".into(),
            ..warm.clone()
        }));
        assert!(!is_qualified_lead(&LeadFacts {
            industry: "Healthcare".into(),
            recent_touches: 1,
            ..warm
        }));
    }

    #[test]
    fn an_existing_customer_never_qualifies() {
        assert!(!is_qualified_lead(&LeadFacts {
            status: CustomerStatus::Active,
            ..lead()
        }));
    }

    #[test]
    fn a_business_whose_contact_is_the_owner_never_qualifies() {
        assert!(!is_qualified_lead(&LeadFacts {
            contact_titles: vec!["Owner".into()],
            ..lead()
        }));
    }

    #[test]
    fn titles_match_regardless_of_case() {
        assert!(is_qualified_lead(&LeadFacts {
            contact_titles: vec!["DIRECTOR OF OPS".into()],
            ..lead()
        }));
    }

    #[test]
    fn open_quote_value_is_the_largest_sent_quote() {
        let quotes = [
            (QuoteStatus::Sent, Decimal::from(3000)),
            (QuoteStatus::Sent, Decimal::from(4000)),
            (QuoteStatus::Declined, Decimal::from(9000)),
        ];
        assert_eq!(
            open_quote_value(&quotes),
            Some(Decimal::from(4000))
        );
        assert_eq!(open_quote_value(&[]), None);
    }

    #[test]
    fn touches_count_the_last_90_days_including_day_90() {
        let today: Date = "2026-10-09".parse().unwrap();
        let dates = [today.add_days(-90), today.add_days(-91), today];
        assert_eq!(recent_touches(&dates, today), 2);
    }
}
