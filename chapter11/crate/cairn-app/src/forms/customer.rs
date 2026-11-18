//! frmCustomers and sfrmContacts: `BuildFrmCustomers`, `BuildSfrmContacts`.

use crate::context::AppContext;
use cairn_core::{
    ContactId, CustomerId, CustomerStatus, Date, RepId,
};
use cairn_forms::{Form, FormEvents};

#[derive(Debug, Clone, PartialEq, Form)]
#[form(context = AppContext)]
pub struct CustomerDraft {
    pub id: Option<CustomerId>,
    #[field(label = "Company", required)]
    pub company_name: String,
    #[field(label = "Industry")]
    pub industry: String,
    #[field(
        label = "Status",
        lookup = "customer_status",
        width = 120
    )]
    pub status: CustomerStatus,
    #[field(label = "Sales Rep", lookup = "reps")]
    pub rep: Option<RepId>,
    // V1's only combo not limited to its list: any terms can be typed.
    #[field(
        label = "Terms",
        lookup = "terms",
        any_text,
        width = 120
    )]
    pub terms: String,
    #[field(label = "Customer Since")]
    pub since: Option<Date>,
    #[field(label = "Tax Exempt")]
    pub tax_exempt: bool,
    #[field(label = "Phone", column = 1)]
    pub phone: String,
    #[field(label = "Email", column = 1)]
    pub email: String,
    #[field(label = "Website", column = 1)]
    pub website: String,
    #[field(label = "Bill Address", column = 1)]
    pub bill_address: String,
    #[field(label = "Bill City", column = 1)]
    pub bill_city: String,
    #[field(label = "Bill State", column = 1, width = 60)]
    pub bill_state: String,
    #[field(label = "Bill Zip", column = 1, width = 100)]
    pub bill_zip: String,
    #[field(label = "Bill Country", column = 1)]
    pub bill_country: String,
    #[field(label = "Ship Address", column = 2)]
    pub ship_address: String,
    #[field(label = "Ship City", column = 2)]
    pub ship_city: String,
    #[field(label = "Ship State", column = 2, width = 60)]
    pub ship_state: String,
    #[field(label = "Ship Zip", column = 2, width = 100)]
    pub ship_zip: String,
    #[field(label = "Ship Country", column = 2)]
    pub ship_country: String,
    #[field(label = "Notes", lines = 3, column = 2)]
    pub notes: String,
}

impl FormEvents for CustomerDraft {}

impl CustomerDraft {
    /// The table's defaults: a prospect, Net 30, in the USA, from today.
    pub fn blank(ctx: &AppContext) -> Self {
        CustomerDraft {
            id: None,
            company_name: String::new(),
            industry: String::new(),
            status: CustomerStatus::Prospect,
            rep: None,
            terms: "Net 30".into(),
            since: Some(ctx.today),
            tax_exempt: false,
            phone: String::new(),
            email: String::new(),
            website: String::new(),
            bill_address: String::new(),
            bill_city: String::new(),
            bill_state: String::new(),
            bill_zip: String::new(),
            bill_country: "USA".into(),
            ship_address: String::new(),
            ship_city: String::new(),
            ship_state: String::new(),
            ship_zip: String::new(),
            ship_country: "USA".into(),
            notes: String::new(),
        }
    }
}

/// A row of the Contacts tab.
#[derive(Debug, Clone, PartialEq, Form)]
#[form(context = AppContext)]
pub struct ContactDraft {
    pub id: Option<ContactId>,
    pub customer: Option<CustomerId>,
    #[field(label = "First Name", required, width = 100)]
    pub first_name: String,
    #[field(label = "Last Name", width = 100)]
    pub last_name: String,
    #[field(label = "Title", width = 140)]
    pub title: String,
    #[field(label = "Phone", width = 100)]
    pub phone: String,
    #[field(label = "Mobile", width = 100)]
    pub mobile: String,
    #[field(label = "Email", width = 180)]
    pub email: String,
    #[field(label = "Primary", width = 50)]
    pub is_primary: bool,
    #[field(label = "Notes", width = 180)]
    pub notes: String,
}

impl FormEvents for ContactDraft {}

impl ContactDraft {
    pub fn blank(customer: Option<CustomerId>) -> Self {
        ContactDraft {
            id: None,
            customer,
            first_name: String::new(),
            last_name: String::new(),
            title: String::new(),
            phone: String::new(),
            mobile: String::new(),
            email: String::new(),
            is_primary: false,
            notes: String::new(),
        }
    }

    pub fn name(&self) -> String {
        match self.last_name.as_str() {
            "" => self.first_name.clone(),
            last => format!("{} {last}", self.first_name),
        }
    }
}
