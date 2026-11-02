//! frmSalesReps: `BuildFrmSalesReps`.

use crate::context::{AppContext, Saved};
use crate::data::Snapshot;
use crate::screens::record::Table;
use crate::screens::sheet::SheetOf;
use cairn_core::rules::fraction;
use cairn_core::{Date, RepId};
use cairn_forms::{Form, FormEvents};

#[derive(Debug, Clone, PartialEq, Form)]
#[form(context = AppContext)]
pub struct RepDraft {
    pub id: Option<RepId>,
    #[field(label = "First Name", required)]
    pub first_name: String,
    #[field(label = "Last Name", required)]
    pub last_name: String,
    #[field(label = "Title")]
    pub title: String,
    #[field(label = "Hire Date")]
    pub hire_date: Option<Date>,
    #[field(label = "Active")]
    pub active: bool,
    #[field(label = "Email", column = 1)]
    pub email: String,
    #[field(label = "Phone", column = 1)]
    pub phone: String,
    #[field(label = "Extension", column = 1, width = 60)]
    pub extension: String,
    #[field(label = "Commission", validate = fraction, column = 1)]
    pub commission_rate: f64,
}

impl FormEvents for RepDraft {}

impl RepDraft {
    pub fn name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }
}

impl Table for RepDraft {
    const TITLE: &'static str = "Sales Reps";
    const SHEET: Option<SheetOf> = Some(SheetOf::Reps);
    const FIND: Option<&'static str> = Some("reps");

    fn key(&self) -> Option<i32> {
        self.id.map(|r| r.0)
    }

    fn rows(data: &Snapshot) -> &[Self] {
        &data.reps
    }

    /// The table's defaults: 5% commission, active.
    fn blank(_: &AppContext) -> Self {
        RepDraft {
            id: None,
            first_name: String::new(),
            last_name: String::new(),
            title: String::new(),
            hire_date: None,
            active: true,
            email: String::new(),
            phone: String::new(),
            extension: String::new(),
            commission_rate: 0.05,
        }
    }

    fn saved(self) -> Saved {
        Saved::Rep(self)
    }
}
