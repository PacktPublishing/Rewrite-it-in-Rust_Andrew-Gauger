//! frmProducts: `BuildFrmProducts`.

use crate::context::{AppContext, Saved};
use crate::data::Snapshot;
use crate::screens::record::Table;
use crate::screens::sheet::SheetOf;
use cairn_core::ProductId;
use cairn_forms::{Form, FormEvents};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Form)]
#[form(context = AppContext)]
pub struct ProductDraft {
    pub id: Option<ProductId>,
    #[field(label = "SKU", required, width = 120)]
    pub sku: String,
    #[field(label = "Product Name", required, width = 260)]
    pub name: String,
    #[field(label = "Category", lookup = "categories", width = 120)]
    pub category: String,
    #[field(label = "Unit Price")]
    pub unit_price: Decimal,
    #[field(label = "Unit Cost")]
    pub unit_cost: Decimal,
    #[field(label = "Lead Time (days)", width = 60)]
    pub lead_time_days: Option<i32>,
    #[field(label = "Firmware")]
    pub firmware: String,
    #[field(label = "Active")]
    pub active: bool,
    #[field(
        label = "Description",
        lines = 7,
        column = 1,
        width = 260
    )]
    pub description: String,
}

impl FormEvents for ProductDraft {}

impl Table for ProductDraft {
    const TITLE: &'static str = "Products";
    const SHEET: Option<SheetOf> = Some(SheetOf::Products);
    const FIND: Option<&'static str> = Some("products");

    fn key(&self) -> Option<i32> {
        self.id.map(|p| p.0)
    }

    fn rows(data: &Snapshot) -> &[Self] {
        &data.products
    }

    fn blank(_: &AppContext) -> Self {
        ProductDraft {
            id: None,
            sku: String::new(),
            name: String::new(),
            category: String::new(),
            unit_price: Decimal::ZERO,
            unit_cost: Decimal::ZERO,
            lead_time_days: None,
            firmware: String::new(),
            active: true,
            description: String::new(),
        }
    }

    fn saved(self) -> Saved {
        Saved::Product(self)
    }
}
