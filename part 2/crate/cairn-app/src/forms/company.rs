//! frmCompanyInfo, the letterhead: `BuildFrmCompanyInfo`. One record;
//! V1 allowed no additions or deletions.

use crate::context::{AppContext, Saved};
use crate::data::Snapshot;
use crate::screens::record::Table;
use cairn_core::rules::fraction;
use cairn_forms::{Form, FormEvents};

#[derive(Debug, Clone, PartialEq, Form)]
#[form(context = AppContext)]
pub struct CompanyDraft {
    pub id: Option<i32>,
    #[field(label = "Company Name", required)]
    pub company_name: String,
    #[field(label = "Address 1")]
    pub address1: String,
    #[field(label = "Address 2")]
    pub address2: String,
    #[field(label = "City")]
    pub city: String,
    #[field(label = "State")]
    pub state: String,
    #[field(label = "Zip")]
    pub zip: String,
    #[field(label = "Country")]
    pub country: String,
    #[field(label = "Phone", column = 1)]
    pub phone: String,
    #[field(label = "Email", column = 1)]
    pub email: String,
    #[field(label = "Website", column = 1)]
    pub website: String,
    #[field(label = "Default Tax Rate", validate = fraction, column = 1)]
    pub default_tax_rate: f64,
    #[field(
        label = "Quote Terms",
        lines = 4,
        column = 1,
        width = 260
    )]
    pub quote_terms: String,
    #[field(
        label = "Receipt Footer",
        lines = 2,
        column = 1,
        width = 260
    )]
    pub receipt_footer: String,
}

impl FormEvents for CompanyDraft {}

impl Table for CompanyDraft {
    const TITLE: &'static str =
        "Company Info (letterhead for quotes and receipts)";
    const SINGLE: bool = true;

    fn key(&self) -> Option<i32> {
        self.id
    }

    fn rows(data: &Snapshot) -> &[Self] {
        std::slice::from_ref(&data.company)
    }

    fn blank(ctx: &AppContext) -> Self {
        ctx.data.company.clone()
    }

    fn saved(self) -> Saved {
        Saved::Company(self)
    }
}
