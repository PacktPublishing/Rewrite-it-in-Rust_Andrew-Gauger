//! frmPhoneMessages, "While You Were Out": `BuildFrmPhoneMessages`.

use crate::context::{AppContext, Command, Saved};
use crate::data::Snapshot;
use crate::screens::record::Table;
use crate::screens::sheet::SheetOf;
use cairn_core::{CustomerId, RepId};
use cairn_forms::{DateTime, Form, FormEvents, RecordForm};
use egui::Ui;

#[derive(Debug, Clone, PartialEq, Form)]
#[form(context = AppContext)]
pub struct MessageDraft {
    pub id: Option<i32>,
    #[field(label = "For", lookup = "reps", required)]
    pub for_rep: Option<RepId>,
    #[field(label = "Received", width = 150)]
    pub received: DateTime,
    #[field(label = "Caller Name", required)]
    pub caller_name: String,
    #[field(
        label = "Customer",
        lookup = "customers",
        after_update = customer_changed
    )]
    pub customer: Option<CustomerId>,
    #[field(label = "Company")]
    pub caller_company: String,
    #[field(label = "Phone", width = 120)]
    pub caller_phone: String,
    #[field(label = "Taken By", lookup = "reps")]
    pub taken_by: Option<RepId>,
    #[field(label = "Urgent")]
    pub urgent: bool,
    #[field(label = "Please call back")]
    pub please_call_back: bool,
    #[field(label = "Will call again")]
    pub will_call_again: bool,
    #[field(label = "Message", lines = 5, column = 1, width = 260)]
    pub message: String,
    #[field(
        label = "Status",
        lookup = "message_status",
        column = 1,
        width = 120
    )]
    pub status: String,
    #[field(label = "Returned At", column = 1, width = 150)]
    pub returned_at: Option<DateTime>,
    #[field(
        label = "Return Notes",
        lines = 2,
        column = 1,
        width = 260
    )]
    pub return_notes: String,
}

/// V1's `MessageCustomerChanged`: the customer's name, and its phone if
/// the caller left none.
fn customer_changed(
    m: &mut MessageDraft,
    ctx: &AppContext,
) -> Vec<Command> {
    if let Some(c) = m.customer.and_then(|id| ctx.customer(id)) {
        m.caller_company = c.company_name.clone();
        if m.caller_phone.is_empty() {
            m.caller_phone = c.phone.clone();
        }
    }
    Vec::new()
}

impl FormEvents for MessageDraft {}

impl Table for MessageDraft {
    const TITLE: &'static str = "While You Were Out";
    const SHEET: Option<SheetOf> = Some(SheetOf::Messages);

    fn key(&self) -> Option<i32> {
        self.id
    }

    fn rows(data: &Snapshot) -> &[Self] {
        &data.messages
    }

    /// The table's defaults: now, please call back, new.
    fn blank(ctx: &AppContext) -> Self {
        MessageDraft {
            id: None,
            for_rep: None,
            received: ctx.now(),
            caller_name: String::new(),
            customer: None,
            caller_company: String::new(),
            caller_phone: String::new(),
            taken_by: None,
            urgent: false,
            please_call_back: true,
            will_call_again: false,
            message: String::new(),
            status: "New".into(),
            returned_at: None,
            return_notes: String::new(),
        }
    }

    fn saved(self) -> Saved {
        Saved::Message(self)
    }

    /// V1's "Mark Returned": returned now, and saved.
    fn extras(
        form: &mut RecordForm<Self>,
        ui: &mut Ui,
        ctx: &AppContext,
    ) -> bool {
        if ui.button("Mark Returned").clicked() {
            form.draft.status = "Returned".into();
            form.draft.returned_at =
                form.draft.returned_at.or(Some(ctx.now()));
            return true;
        }
        false
    }
}
