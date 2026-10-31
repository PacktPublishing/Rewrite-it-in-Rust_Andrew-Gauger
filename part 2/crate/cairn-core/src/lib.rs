//! Cairn Systems' business rules, ported from the V1 Access database.
//!
//! Port, don't refactor: each rule behaves as V1 does. Rules that look like
//! bugs are kept and marked `// FLAG:` (listed in `FLAGS.md`). The few V1
//! defects this crate corrects are marked `// FIX:`.

pub mod commission;
pub mod date;
pub mod defaults;
pub mod ids;
pub mod leads;
pub mod manufacturer;
pub mod product;
pub mod quote_to_order;
pub mod receipt;
pub mod rma;
pub mod rounding;
pub mod status;
pub mod totals;

pub use commission::{
    sales_by_rep_commission, statement, CommissionOrder, Statement,
};
pub use date::Date;
pub use defaults::{
    default_tax_rate, on_customer_picked, CompanyInfo, CustomerRow,
    DocumentDefaults,
};
pub use ids::{
    ContactId, CustomerId, OrderId, PaymentId, ProductId, QuoteId,
    RepId,
};
pub use leads::{
    is_qualified_lead, open_quote_value, recent_touches, LeadFacts,
};
pub use manufacturer::{
    ManufacturerOrder, MfrLine, OrderError, ShipTo, SourceLine,
};
pub use product::Category;
pub use quote_to_order::{
    quote_to_order, OrderDraft, Quote, QuoteToOrderError,
};
pub use receipt::{paid_to_date, receipt_number, Payment};
pub use rma::{
    days_in_service, route, route_v1, Reason, ReturnFacts, Route,
    UnknownReason,
};
pub use rounding::{access_round, access_round_product};
pub use status::{
    CustomerStatus, OrderStatus, QuoteStatus, UnknownStatus,
};
pub use totals::{line_total, totals, DocLine, Totals};
