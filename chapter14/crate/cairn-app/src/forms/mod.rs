//! V1's forms, one module each, built from `BuildFrm...` in
//! `CairnForms.bas`: the same fields, in the same Tab order, with the same
//! AfterUpdate handlers. Each record type is also the snapshot's row, so a
//! save replaces a row.

pub mod activity;
pub mod company;
pub mod customer;
pub mod message;
pub mod order;
pub mod product;
pub mod quote;
pub mod rep;
pub mod rma;

pub use activity::{ActivityDraft, ActivityLine};
pub use company::CompanyDraft;
pub use customer::{ContactDraft, CustomerDraft};
pub use message::MessageDraft;
pub use order::{OrderDraft, OrderLineDraft, PaymentDraft};
pub use product::ProductDraft;
pub use quote::{QuoteDraft, QuoteLineDraft};
pub use rep::RepDraft;
pub use rma::RmaDraft;
