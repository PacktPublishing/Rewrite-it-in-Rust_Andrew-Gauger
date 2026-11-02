//! frmOrders, sfrmOrderLines and sfrmPayments: `BuildFrmOrders`,
//! `BuildSfrmLines` (with serial numbers), `BuildSfrmPayments`.

use crate::context::{AppContext, Command};
use crate::forms::quote::line_doc;
use cairn_core::rules::fraction;
use cairn_core::{
    ContactId, CustomerId, Date, DocLine, OrderId, OrderStatus,
    PaymentId, ProductId, QuoteId, RepId,
};
use cairn_forms::{Form, FormEvents};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Form)]
#[form(context = AppContext)]
pub struct OrderDraft {
    pub id: Option<OrderId>,
    #[field(label = "Order #", read_only)]
    pub number: String,
    #[field(label = "Order Date")]
    pub date: Date,
    #[field(label = "Required")]
    pub required: Option<Date>,
    #[field(label = "Ship Date")]
    pub ship_date: Option<Date>,
    #[field(label = "Status", lookup = "order_status", width = 120)]
    pub status: OrderStatus,
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
    #[field(
        label = "From Quote",
        lookup = "customer_quotes",
        filter = customer,
        column = 1,
        width = 140
    )]
    pub quote: Option<QuoteId>,
    #[field(label = "Customer PO", column = 1)]
    pub customer_po: String,
    #[field(label = "Tax Rate", validate = fraction, column = 1)]
    pub tax_rate: f64,
    #[field(label = "Shipping", money, column = 1)]
    pub shipping: Decimal,
    #[field(label = "Ship Via", column = 2)]
    pub ship_via: String,
    #[field(label = "Tracking #", column = 2)]
    pub tracking: String,
    #[field(label = "Notes", lines = 3, column = 2)]
    pub notes: String,
}

/// V1's `DocCustomerChanged`, as on the quote form.
fn customer_changed(
    o: &mut OrderDraft,
    ctx: &AppContext,
) -> Vec<Command> {
    let Some(row) =
        o.customer.and_then(|c| ctx.customer_row(c, o.rep))
    else {
        return Vec::new();
    };
    let picked =
        cairn_core::on_customer_picked(o.rep, &row, &ctx.company);
    o.tax_rate = picked.tax_rate;
    o.rep = Some(picked.rep);
    o.contact = picked.contact;
    Vec::new()
}

impl FormEvents for OrderDraft {}

impl OrderDraft {
    /// A new order, as frmOrders opened for data entry, optionally for a
    /// customer (V1's "New Order" on a customer, through `DocLoad`).
    pub fn blank(
        ctx: &AppContext,
        customer: Option<CustomerId>,
    ) -> Self {
        let mut o = OrderDraft {
            id: None,
            number: "(New)".into(),
            date: ctx.today,
            required: None,
            ship_date: None,
            status: OrderStatus::Open,
            rep: None,
            customer,
            contact: None,
            quote: None,
            customer_po: String::new(),
            tax_rate: 0.0,
            shipping: Decimal::ZERO,
            ship_via: String::new(),
            tracking: String::new(),
            notes: String::new(),
        };
        customer_changed(&mut o, ctx);
        o
    }
}

#[derive(Debug, Clone, PartialEq, Form)]
#[form(context = AppContext)]
pub struct OrderLineDraft {
    #[field(
        label = "Product",
        lookup = "products",
        required,
        after_update = product_changed,
        width = 110
    )]
    pub product: Option<ProductId>,
    #[field(
        label = "Description",
        placeholder = product_name,
        width = 220,
        hint = "Blank prints the product's name"
    )]
    pub description: String,
    #[field(label = "Qty", width = 50)]
    pub quantity: i32,
    #[field(label = "Unit Price", width = 90)]
    pub unit_price: Decimal,
    #[field(label = "Discount", validate = fraction, width = 70)]
    pub discount: f64,
    #[field(label = "Serial Numbers", width = 150)]
    pub serial_numbers: String,
}

/// V1's `LineProductChanged`: the product's price.
fn product_changed(
    line: &mut OrderLineDraft,
    ctx: &AppContext,
) -> Vec<Command> {
    if let Some(p) = line.product.and_then(|id| ctx.product(id)) {
        line.unit_price = p.unit_price;
    }
    Vec::new()
}

/// R02: a blank description prints the product's name, so an empty
/// cell shows it, faint.
fn product_name(line: &OrderLineDraft, ctx: &AppContext) -> String {
    line.product
        .and_then(|id| ctx.product(id))
        .map(|p| p.name.clone())
        .unwrap_or_default()
}

impl FormEvents for OrderLineDraft {}

impl OrderLineDraft {
    pub fn blank() -> Self {
        OrderLineDraft {
            product: None,
            description: String::new(),
            quantity: 1,
            unit_price: Decimal::ZERO,
            discount: 0.0,
            serial_numbers: String::new(),
        }
    }

    pub fn doc_line(&self) -> DocLine {
        line_doc(
            self.product,
            &self.description,
            self.quantity,
            self.unit_price,
            self.discount,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Form)]
#[form(context = AppContext)]
pub struct PaymentDraft {
    pub id: Option<PaymentId>,
    #[field(label = "Date", width = 90)]
    pub date: Date,
    #[field(label = "Amount", money, width = 90)]
    pub amount: Decimal,
    #[field(
        label = "Method",
        lookup = "payment_methods",
        width = 100
    )]
    pub method: String,
    #[field(label = "Reference", width = 120)]
    pub reference: String,
    #[field(label = "Received By", lookup = "reps", width = 130)]
    pub received_by: Option<RepId>,
    #[field(label = "Notes", width = 160)]
    pub notes: String,
}

impl FormEvents for PaymentDraft {}

impl PaymentDraft {
    pub fn blank(today: Date) -> Self {
        PaymentDraft {
            id: None,
            date: today,
            amount: Decimal::ZERO,
            method: String::new(),
            reference: String::new(),
            received_by: None,
            notes: String::new(),
        }
    }
}
