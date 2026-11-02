//! frmActivities and sfrmCustomerActivities: `BuildFrmActivities`,
//! `BuildSfrmCustomerActivities`.

use crate::context::{AppContext, Command, Saved};
use crate::data::Snapshot;
use crate::screens::record::Table;
use crate::screens::sheet::SheetOf;
use cairn_core::{ContactId, CustomerId, Date, RepId};
use cairn_forms::{DateTime, Form, FormEvents};

#[derive(Debug, Clone, PartialEq, Form)]
#[form(context = AppContext)]
pub struct ActivityDraft {
    pub id: Option<i32>,
    #[field(
        label = "Customer",
        lookup = "customers",
        required,
        after_update = customer_changed
    )]
    pub customer: Option<CustomerId>,
    #[field(label = "Contact", lookup = "contacts", filter = customer)]
    pub contact: Option<ContactId>,
    #[field(label = "Rep", lookup = "reps", required)]
    pub rep: Option<RepId>,
    #[field(label = "Date / Time", width = 150)]
    pub date: DateTime,
    #[field(label = "Type", lookup = "activity_types", width = 120)]
    pub activity_type: String,
    #[field(label = "Follow-up Date")]
    pub follow_up: Option<Date>,
    #[field(label = "Completed")]
    pub completed: bool,
    #[field(label = "Subject", column = 1, width = 260)]
    pub subject: String,
    #[field(label = "Details", lines = 7, column = 1, width = 260)]
    pub details: String,
}

/// V1's `ActivityCustomerChanged`: the customer's rep and primary
/// contact, if the activity has none yet.
fn customer_changed(
    a: &mut ActivityDraft,
    ctx: &AppContext,
) -> Vec<Command> {
    if let Some(c) = a.customer.and_then(|id| ctx.customer(id)) {
        a.rep = a.rep.or(c.rep);
        a.contact = a.contact.or(ctx.primary_contact(c.id.unwrap()));
    }
    Vec::new()
}

impl FormEvents for ActivityDraft {}

impl Table for ActivityDraft {
    const TITLE: &'static str = "Activity (call / meeting / note)";
    const SHEET: Option<SheetOf> = Some(SheetOf::Activities);

    fn key(&self) -> Option<i32> {
        self.id
    }

    fn rows(data: &Snapshot) -> &[Self] {
        &data.activities
    }

    fn blank(ctx: &AppContext) -> Self {
        ActivityDraft::blank(ctx, None)
    }

    fn saved(self) -> Saved {
        Saved::Activity(self)
    }
}

impl ActivityDraft {
    /// A new activity: now, a note, for `customer` if one is given (V1's
    /// "Log Activity" button on a customer).
    pub fn blank(
        ctx: &AppContext,
        customer: Option<CustomerId>,
    ) -> Self {
        let mut a = ActivityDraft {
            id: None,
            customer,
            contact: None,
            rep: None,
            date: ctx.now(),
            activity_type: "Note".into(),
            follow_up: None,
            completed: false,
            subject: String::new(),
            details: String::new(),
        };
        customer_changed(&mut a, ctx);
        a
    }

    pub fn line(&self) -> ActivityLine {
        ActivityLine {
            id: self.id,
            customer: self.customer,
            date: self.date,
            activity_type: self.activity_type.clone(),
            subject: self.subject.clone(),
            details: self.details.clone(),
            contact: self.contact,
            rep: self.rep,
            follow_up: self.follow_up,
            completed: self.completed,
        }
    }
}

/// A row of the customer's Activities tab. The customer is hidden: it
/// narrows the Contact column's list.
#[derive(Debug, Clone, PartialEq, Form)]
#[form(context = AppContext)]
pub struct ActivityLine {
    pub id: Option<i32>,
    #[field(hidden)]
    pub customer: Option<CustomerId>,
    #[field(label = "Date", width = 140)]
    pub date: DateTime,
    #[field(label = "Type", lookup = "activity_types", width = 80)]
    pub activity_type: String,
    #[field(label = "Subject", width = 160)]
    pub subject: String,
    #[field(label = "Details", width = 220)]
    pub details: String,
    #[field(
        label = "Contact",
        lookup = "contacts",
        filter = customer,
        width = 120
    )]
    pub contact: Option<ContactId>,
    #[field(label = "Rep", lookup = "reps", required, width = 120)]
    pub rep: Option<RepId>,
    #[field(label = "Follow Up", width = 90)]
    pub follow_up: Option<Date>,
    #[field(label = "Done", width = 40)]
    pub completed: bool,
}

impl FormEvents for ActivityLine {}

impl ActivityLine {
    pub fn draft(&self) -> ActivityDraft {
        ActivityDraft {
            id: self.id,
            customer: self.customer,
            contact: self.contact,
            rep: self.rep,
            date: self.date,
            activity_type: self.activity_type.clone(),
            follow_up: self.follow_up,
            completed: self.completed,
            subject: self.subject.clone(),
            details: self.details.clone(),
        }
    }
}
