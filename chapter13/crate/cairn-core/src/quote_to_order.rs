//! R18, V1's `QuoteToOrder`: Create Order from Quote.

use crate::date::Date;
use crate::ids::{ContactId, CustomerId, OrderId, QuoteId, RepId};
use crate::status::{OrderStatus, QuoteStatus};
use crate::totals::DocLine;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub id: QuoteId,
    pub customer: CustomerId,
    pub contact: Option<ContactId>,
    pub rep: RepId,
    pub tax_rate: f64,
    pub shipping: Decimal,
    pub status: QuoteStatus,
    pub lines: Vec<DocLine>,
}

/// A new order, not yet saved. Its number is assigned when it's saved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderDraft {
    pub quote: QuoteId,
    pub customer: CustomerId,
    pub contact: Option<ContactId>,
    pub rep: RepId,
    pub order_date: Date,
    pub status: OrderStatus,
    pub tax_rate: f64,
    pub shipping: Decimal,
    pub lines: Vec<DocLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteToOrderError {
    /// One order per quote. V1 offers to open the first one.
    AlreadyHasOrder(OrderId),
    NoLines,
}

impl fmt::Display for QuoteToOrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuoteToOrderError::AlreadyHasOrder(id) => write!(f, "an order (id {}) was already created from this quote", id.0),
            QuoteToOrderError::NoLines => f.write_str("this quote has no line items"),
        }
    }
}

impl std::error::Error for QuoteToOrderError {}

/// The order draft and the quote's new status, or an error and nothing.
/// V1 checks for an existing order first, then for lines.
// FIX: R18 V1 runs separate INSERT and UPDATE statements with nothing to undo the first if a later one fails. This returns both results at once; the caller saves them in one transaction.
pub fn quote_to_order(
    quote: &Quote,
    existing_order: Option<OrderId>,
    today: Date,
) -> Result<(OrderDraft, QuoteStatus), QuoteToOrderError> {
    if let Some(id) = existing_order {
        return Err(QuoteToOrderError::AlreadyHasOrder(id));
    }
    if quote.lines.is_empty() {
        return Err(QuoteToOrderError::NoLines);
    }
    let draft = OrderDraft {
        quote: quote.id,
        customer: quote.customer,
        contact: quote.contact,
        rep: quote.rep,
        order_date: today,
        status: OrderStatus::Open,
        tax_rate: quote.tax_rate,
        shipping: quote.shipping,
        lines: quote.lines.clone(),
    };
    Ok((draft, QuoteStatus::Accepted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::ProductId;

    fn quote(lines: usize) -> Quote {
        Quote {
            id: QuoteId(18454),
            customer: CustomerId(761),
            contact: Some(ContactId(1511)),
            rep: RepId(7),
            tax_rate: 0.0725,
            shipping: Decimal::new(7300, 2),
            status: QuoteStatus::Sent,
            lines: (0..lines)
                .map(|i| DocLine {
                    product: ProductId(i as i32 + 1),
                    description: None,
                    quantity: 1,
                    unit_price: Decimal::new(10000, 2),
                    discount: 0.0,
                })
                .collect(),
        }
    }

    fn today() -> Date {
        "2026-09-23".parse().unwrap()
    }

    #[test]
    fn copies_the_header_and_every_line_into_an_open_order() {
        let q = quote(3);
        let (draft, status) =
            quote_to_order(&q, None, today()).unwrap();
        assert_eq!(status, QuoteStatus::Accepted);
        assert_eq!(draft.status, OrderStatus::Open);
        assert_eq!(draft.order_date, today());
        assert_eq!(
            (draft.customer, draft.contact, draft.rep),
            (q.customer, q.contact, q.rep)
        );
        assert_eq!(
            (draft.tax_rate, draft.shipping),
            (q.tax_rate, q.shipping)
        );
        assert_eq!(draft.lines, q.lines);
    }

    #[test]
    fn a_quote_with_no_lines_is_refused() {
        assert_eq!(
            quote_to_order(&quote(0), None, today()),
            Err(QuoteToOrderError::NoLines)
        );
    }

    #[test]
    fn a_second_order_from_one_quote_is_refused_with_the_first() {
        assert_eq!(
            quote_to_order(&quote(0), Some(OrderId(13359)), today()),
            Err(QuoteToOrderError::AlreadyHasOrder(OrderId(13359)))
        );
    }
}
