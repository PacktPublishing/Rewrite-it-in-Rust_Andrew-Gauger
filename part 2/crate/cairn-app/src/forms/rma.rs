//! frmRMAs: `BuildFrmRMAs`. Its locked Route and Days in service boxes
//! were `DLookUp`s into `qryRMARouting`; here they're Chapter 8's
//! `route` and `days_in_service`, over rows the context already holds.

use crate::context::{AppContext, Saved};
use crate::data::Snapshot;
use crate::screens::record::Table;
use crate::screens::sheet::SheetOf;
use cairn_core::rma::{days_in_service, route, Reason, ReturnFacts};
use cairn_core::{
    Category, CustomerId, Date, OrderId, ProductId, RepId,
};
use cairn_forms::{Form, FormEvents, RecordForm};
use egui::{RichText, Ui};

#[derive(Debug, Clone, PartialEq, Form)]
#[form(context = AppContext)]
pub struct RmaDraft {
    pub id: Option<i32>,
    #[field(label = "RMA #", read_only)]
    pub number: String,
    #[field(label = "Opened")]
    pub opened: Date,
    #[field(label = "Status", lookup = "rma_status", width = 120)]
    pub status: String,
    #[field(label = "Opened By", lookup = "reps")]
    pub opened_by: Option<RepId>,
    #[field(label = "Closed")]
    pub closed: Option<Date>,
    #[field(
        label = "Customer",
        lookup = "customers",
        required,
        column = 1
    )]
    pub customer: Option<CustomerId>,
    #[field(
        label = "Order",
        lookup = "customer_orders",
        filter = customer,
        column = 1
    )]
    pub order: Option<OrderId>,
    // All products, retired ones too: returns come back on old models.
    #[field(
        label = "Product",
        lookup = "products",
        required,
        column = 1
    )]
    pub product: Option<ProductId>,
    #[field(label = "Serial #", column = 1)]
    pub serial: String,
    #[field(
        label = "Reason",
        lookup = "rma_reasons",
        required,
        column = 1
    )]
    pub reason: String,
    #[field(label = "Ext. Warranty", column = 1)]
    pub extended_warranty: bool,
    #[field(label = "Disposition", column = 2)]
    pub disposition: String,
    #[field(label = "Mfr RMA #", column = 2)]
    pub manufacturer_ref: String,
    #[field(label = "Description", lines = 4, column = 2)]
    pub description: String,
}

impl FormEvents for RmaDraft {}

impl RmaDraft {
    /// Days from the order's ship date (or order date) to the opening.
    pub fn days(&self, ctx: &AppContext) -> Option<i32> {
        let order = self.order.and_then(|id| ctx.order(id));
        days_in_service(
            self.opened,
            order.map(|o| (o.date, o.ship_date)),
        )
    }

    /// Where V2 routes this return, or why it can't say.
    pub fn route(&self, ctx: &AppContext) -> String {
        let reason = Reason::try_from(self.reason.as_str());
        let product = self.product.and_then(|id| ctx.product(id));
        let category = product.and_then(|p| {
            Category::try_from(p.category.as_str()).ok()
        });
        let customer = self.customer.and_then(|id| ctx.customer(id));
        match (reason, category, customer) {
            (Ok(reason), Some(category), Some(c)) => {
                route(&ReturnFacts {
                    reason,
                    category,
                    extended_warranty: self.extended_warranty,
                    tax_exempt: c.tax_exempt,
                    days_in_service: self.days(ctx),
                })
                .to_string()
            }
            _ => "(needs a customer, a product and a reason)".into(),
        }
    }
}

impl Table for RmaDraft {
    const TITLE: &'static str = "Returns (RMA)";
    const SHEET: Option<SheetOf> = Some(SheetOf::Rmas);
    const FIND: Option<&'static str> = Some("rma_numbers");

    fn key(&self) -> Option<i32> {
        self.id
    }

    fn rows(data: &Snapshot) -> &[Self] {
        &data.rmas
    }

    /// The table's defaults. V1's `RMADefaults` numbered it and set the
    /// signed-in rep; Postgres numbers it at save (Chapter 9), and the
    /// signed-in rep arrives with sign-in (Chapter 11).
    fn blank(ctx: &AppContext) -> Self {
        RmaDraft {
            id: None,
            number: "(New)".into(),
            opened: ctx.today,
            status: "Open".into(),
            opened_by: None,
            closed: None,
            customer: None,
            order: None,
            product: None,
            serial: String::new(),
            reason: String::new(),
            extended_warranty: false,
            disposition: String::new(),
            manufacturer_ref: String::new(),
            description: String::new(),
        }
    }

    fn saved(self) -> Saved {
        Saved::Rma(self)
    }

    /// V1's locked boxes: the order on file, the route, the days.
    fn extras(
        form: &mut RecordForm<Self>,
        ui: &mut Ui,
        ctx: &AppContext,
    ) -> bool {
        let r = &form.draft;
        let on_file = match r.order {
            None => "(none)".to_string(),
            Some(id) => ctx
                .order(id)
                .map(|o| o.number.clone())
                .unwrap_or(format!("order {} - not found", id.0)),
        };
        let days = r
            .days(ctx)
            .map_or("(no order)".to_string(), |d| d.to_string());
        egui::Grid::new("rma-extras").num_columns(2).show(ui, |ui| {
            ui.label("Order on file");
            ui.label(on_file);
            ui.end_row();
            ui.label("Route");
            ui.label(RichText::new(r.route(ctx)).strong());
            ui.end_row();
            ui.label("Days in service");
            ui.label(days);
            ui.end_row();
        });
        false
    }
}
