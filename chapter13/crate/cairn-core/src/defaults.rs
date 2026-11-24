//! R13, V1's `DocCustomerChanged`: what picking a customer on a quote or an
//! order fills in.

use crate::ids::{ContactId, RepId};
use serde::{Deserialize, Serialize};

/// The `CompanyInfo` row's one value these rules read.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CompanyInfo {
    pub default_tax_rate: f64,
}

/// What a quote or order needs to know about the customer just picked.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
pub struct CustomerRow {
    pub tax_exempt: bool,
    pub assigned_rep: RepId,
    pub primary_contact: Option<ContactId>,
}

/// Tax-exempt means zero; otherwise the company's default. The rate is
/// copied onto the document, so changing the default never reprices an old
/// quote.
pub fn default_tax_rate(
    customer: &CustomerRow,
    company: &CompanyInfo,
) -> f64 {
    if customer.tax_exempt {
        0.0
    } else {
        company.default_tax_rate
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DocumentDefaults {
    pub tax_rate: f64,
    pub rep: RepId,
    pub contact: Option<ContactId>,
}

/// The tax rate is always reset. The rep is the customer's only if the
/// document has none yet. The contact is always the primary contact.
pub fn on_customer_picked(
    current_rep: Option<RepId>,
    customer: &CustomerRow,
    company: &CompanyInfo,
) -> DocumentDefaults {
    DocumentDefaults {
        tax_rate: default_tax_rate(customer, company),
        rep: current_rep.unwrap_or(customer.assigned_rep),
        contact: customer.primary_contact,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPANY: CompanyInfo = CompanyInfo {
        default_tax_rate: 0.0825,
    };

    fn customer(tax_exempt: bool) -> CustomerRow {
        CustomerRow {
            tax_exempt,
            assigned_rep: RepId(7),
            primary_contact: Some(ContactId(42)),
        }
    }

    #[test]
    fn tax_exempt_customers_pay_no_tax() {
        assert_eq!(default_tax_rate(&customer(true), &COMPANY), 0.0);
        assert_eq!(
            default_tax_rate(&customer(false), &COMPANY),
            0.0825
        );
    }

    #[test]
    fn a_rep_already_on_the_document_stays() {
        assert_eq!(
            on_customer_picked(
                Some(RepId(3)),
                &customer(false),
                &COMPANY
            )
            .rep,
            RepId(3)
        );
        assert_eq!(
            on_customer_picked(None, &customer(false), &COMPANY).rep,
            RepId(7)
        );
    }

    #[test]
    fn the_contact_is_the_primary_contact() {
        assert_eq!(
            on_customer_picked(None, &customer(false), &COMPANY)
                .contact,
            Some(ContactId(42))
        );
    }
}
