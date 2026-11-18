//! Cairn's form kit: what V1's form helpers (`Txt`, `Cbo`, `Chk`,
//! `TotalBox`, `SubF`, `FindBox`) did for Access, on egui.
//!
//! - [`RecordForm`] edits one record the way a bound form does.
//! - `#[derive(Form)]` turns a struct into a form's fields: label, widget,
//!   Tab order, lookup, validation rule and AfterUpdate handler.
//! - [`Datasheet`] edits rows; [`ListView`] lists them, sorted and
//!   filtered from its column headers.
//! - Lookups are one widget for every combo box.
//!
//! Handlers never do input or output. They read what the form's context
//! already holds and return commands for the app to run.

pub mod datasheet;
pub mod field;
pub mod list;
pub mod lookup;
pub mod record;
pub mod report;
pub mod widgets;

pub use cairn_forms_derive::Form;
pub use datasheet::{Computed, Datasheet};
pub use field::{
    display, money, DateTime, FieldSpec, FieldValue, Kind,
};
pub use list::{ListColumn, ListView};
pub use lookup::{find_box, LookupList, LookupRow};
pub use record::{
    Cmd, Form, FormContext, FormEvents, Nav, RecordForm,
};
