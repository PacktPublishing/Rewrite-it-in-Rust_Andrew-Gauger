//! frmQuotes and sfrmQuoteLines: V1's `BuildFrmQuotes` and
//! `BuildSfrmLines`, field for field and in the same Tab order.

use crate::context::{AppContext, Command};
use cairn_core::rules::fraction;
use cairn_core::{
    ContactId, CustomerId, Date, DocLine, ProductId, QuoteId,
    QuoteStatus, RepId,
};
use cairn_forms::{Form, FormEvents};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Form)]
#[form(context = AppContext)]
pub struct QuoteDraft {
    pub id: Option<QuoteId>,
    #[field(label = "Quote #", read_only)]
    pub number: String,
    #[field(label = "Quote Date", required)]
    pub date: Date,
    #[field(label = "Valid Until")]
    pub valid_until: Option<Date>,
    #[field(label = "Status", lookup = "quote_status")]
    pub status: QuoteStatus,
    #[field(label = "Sales Rep", lookup = "reps", required)]
    pub rep: Option<RepId>,
    #[field(
        label = "Customer",
        lookup = "customers",
        required,
        after_update = customer_changed,
        column = 1
    )]
    pub customer: Option<CustomerId>,
    #[field(
        label = "Contact",
        lookup = "contacts",
        filter = customer,
        column = 1
    )]
    pub contact: Option<ContactId>,
    #[field(label = "Tax Rate", validate = fraction, column = 1)]
    pub tax_rate: f64,
    #[field(label = "Shipping", money, column = 1)]
    pub shipping: Decimal,
    #[field(label = "Notes", lines = 2, column = 2)]
    pub notes: String,
    #[field(
        label = "Terms",
        lines = 3,
        column = 2,
        hint = "Leave blank to use the standard terms from Company Info"
    )]
    pub terms: String,
}

/// V1's `DocCustomerChanged`, without its three `DLookup`s: the
/// customer's row is already in the context. The rules are Chapter 8's.
fn customer_changed(
    q: &mut QuoteDraft,
    ctx: &AppContext,
) -> Vec<Command> {
    let Some(row) =
        q.customer.and_then(|c| ctx.customer_row(c, q.rep))
    else {
        return Vec::new();
    };
    let picked =
        cairn_core::on_customer_picked(q.rep, &row, &ctx.company);
    q.tax_rate = picked.tax_rate;
    q.rep = Some(picked.rep);
    q.contact = picked.contact;
    Vec::new()
}

impl FormEvents for QuoteDraft {}

#[derive(Debug, Clone, PartialEq, Form)]
#[form(context = AppContext)]
pub struct QuoteLineDraft {
    #[field(
        label = "Product",
        lookup = "products",
        required,
        after_update = product_changed,
        width = 120
    )]
    pub product: Option<ProductId>,
    #[field(
        label = "Description",
        placeholder = product_name,
        width = 260,
        hint = "Blank prints the product's name"
    )]
    pub description: String,
    #[field(label = "Qty", width = 50)]
    pub quantity: i32,
    #[field(label = "Unit Price", width = 90)]
    pub unit_price: Decimal,
    #[field(label = "Discount", validate = fraction, width = 70)]
    pub discount: f64,
}

/// V1's `LineProductChanged`: the product's price.
fn product_changed(
    line: &mut QuoteLineDraft,
    ctx: &AppContext,
) -> Vec<Command> {
    if let Some(p) = line.product.and_then(|id| ctx.product(id)) {
        line.unit_price = p.unit_price;
    }
    Vec::new()
}

/// R02: a blank description prints the product's name, so an empty
/// cell shows it, faint.
fn product_name(line: &QuoteLineDraft, ctx: &AppContext) -> String {
    line.product
        .and_then(|id| ctx.product(id))
        .map(|p| p.name.clone())
        .unwrap_or_default()
}

impl FormEvents for QuoteLineDraft {}

impl QuoteLineDraft {
    pub fn doc_line(&self) -> DocLine {
        line_doc(
            self.product,
            &self.description,
            self.quantity,
            self.unit_price,
            self.discount,
        )
    }

    pub fn blank() -> Self {
        QuoteLineDraft {
            product: None,
            description: String::new(),
            quantity: 1,
            unit_price: Decimal::ZERO,
            discount: 0.0,
        }
    }
}

impl QuoteDraft {
    /// A new quote, with the table's defaults (today, valid 30 days),
    /// optionally for a customer: V1's "New Quote" on a customer, which
    /// ran `DocCustomerChanged` from `DocLoad`.
    pub fn blank(
        ctx: &AppContext,
        customer: Option<CustomerId>,
    ) -> Self {
        let mut q = QuoteDraft {
            id: None,
            number: "(New)".into(),
            date: ctx.today,
            valid_until: Some(ctx.today.add_days(30)),
            status: QuoteStatus::Draft,
            rep: None,
            customer,
            contact: None,
            tax_rate: 0.0,
            shipping: Decimal::ZERO,
            notes: String::new(),
            terms: String::new(),
        };
        customer_changed(&mut q, ctx);
        q
    }
}

/// A quote or order line as `cairn-core` totals it. A line still missing
/// its product totals the same; the save refuses it.
pub fn line_doc(
    product: Option<ProductId>,
    description: &str,
    quantity: i32,
    unit_price: Decimal,
    discount: f64,
) -> DocLine {
    DocLine {
        product: product.unwrap_or(ProductId(0)),
        description: (!description.is_empty())
            .then(|| description.into()),
        quantity,
        unit_price,
        discount,
    }
}
