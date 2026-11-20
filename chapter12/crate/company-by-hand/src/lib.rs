//! V1's Company Info form (`BuildFrmCompanyInfo`), drawn by hand with
//! egui's own widgets: a grid of labels and text boxes, a check on
//! whether anything changed, and Save and Undo. No form kit, no database.
//!
//! Everything here that repeats, or that has to be remembered, is what
//! `#[derive(Form)]` writes later: the label beside each box, the text a
//! number is typed as, the copy kept for Undo.

use egui::{Button, Grid, TextEdit, Ui};

/// The `CompanyInfo` row.
#[derive(Debug, Clone, PartialEq)]
pub struct CompanyInfo {
    pub company_name: String,
    pub address1: String,
    pub address2: String,
    pub city: String,
    pub state: String,
    pub zip: String,
    pub country: String,
    pub phone: String,
    pub email: String,
    pub website: String,
    /// A fraction: 0.0825 for 8.25%.
    pub default_tax_rate: f64,
    pub quote_terms: String,
    pub receipt_footer: String,
}

impl CompanyInfo {
    /// The row V1 shipped with (`CairnBuild.bas`), its placeholders
    /// still in it.
    pub fn v1() -> Self {
        CompanyInfo {
            company_name: "Cairn Systems".into(),
            address1: "[Street Address]".into(),
            address2: String::new(),
            city: "[City]".into(),
            state: "[ST]".into(),
            zip: "[Zip]".into(),
            country: "USA".into(),
            phone: "[Phone]".into(),
            email: "sales@cairnsystems.example".into(),
            website: "www.cairnsystems.example".into(),
            default_tax_rate: 0.0825,
            quote_terms: "Prices valid 30 days from quote date. Net 30 \
                on approved credit. Standard lead time 3-4 weeks ARO. \
                Firmware updates included for 12 months."
                .into(),
            receipt_footer: "Thank you for your business! Questions \
                about this receipt? Contact your account representative."
                .into(),
        }
    }
}

/// The screen's state. egui keeps none of it: each frame draws from
/// here, and what the user types lands here.
pub struct CompanyScreen {
    pub saved: CompanyInfo,
    pub draft: CompanyInfo,
    /// The tax rate as typed. A text box edits a `String`, and "8.2" on
    /// the way to "8.25" isn't a rate yet, so the text lives here.
    pub tax_text: String,
}

/// One label and its text box, as a row of the grid.
fn field(ui: &mut Ui, label: &str, value: &mut String) {
    let label = ui.label(label);
    ui.text_edit_singleline(value).labelled_by(label.id);
    ui.end_row();
}

fn percent(rate: f64) -> String {
    format!("{}", (rate * 10_000.0).round() / 100.0)
}

impl CompanyScreen {
    pub fn new(info: CompanyInfo) -> Self {
        CompanyScreen {
            tax_text: percent(info.default_tax_rate),
            saved: info.clone(),
            draft: info,
        }
    }

    /// One frame. Called every time egui draws, about 60 times a second
    /// while anything moves.
    pub fn ui(&mut self, ui: &mut Ui) {
        ui.heading("Company Info");
        // Every text box in this frame is 200 points wide.
        ui.spacing_mut().text_edit_width = 200.0;
        let d = &mut self.draft;
        ui.horizontal_top(|ui| {
            Grid::new("address").num_columns(2).show(ui, |ui| {
                field(ui, "Company Name", &mut d.company_name);
                field(ui, "Address 1", &mut d.address1);
                field(ui, "Address 2", &mut d.address2);
                field(ui, "City", &mut d.city);
                field(ui, "State", &mut d.state);
                field(ui, "Zip", &mut d.zip);
                field(ui, "Country", &mut d.country);
            });
            Grid::new("contact").num_columns(2).show(ui, |ui| {
                field(ui, "Phone", &mut d.phone);
                field(ui, "Email", &mut d.email);
                field(ui, "Website", &mut d.website);

                let label = ui.label("Default Tax Rate");
                let typed = ui
                    .add(TextEdit::singleline(&mut self.tax_text))
                    .labelled_by(label.id);
                if typed.changed() {
                    // Only text that reads as a number changes the rate.
                    let text =
                        self.tax_text.trim().trim_end_matches('%');
                    if let Ok(pct) = text.parse::<f64>() {
                        d.default_tax_rate = pct / 100.0;
                    }
                }
                ui.end_row();

                let label = ui.label("Quote Terms");
                ui.text_edit_multiline(&mut d.quote_terms)
                    .labelled_by(label.id);
                ui.end_row();

                let label = ui.label("Receipt Footer");
                ui.text_edit_multiline(&mut d.receipt_footer)
                    .labelled_by(label.id);
                ui.end_row();
            });
        });

        let dirty = self.draft != self.saved;
        ui.horizontal(|ui| {
            if ui.add_enabled(dirty, Button::new("Save")).clicked() {
                self.saved = self.draft.clone();
            }
            if ui.add_enabled(dirty, Button::new("Undo")).clicked() {
                self.draft = self.saved.clone();
                self.tax_text = percent(self.saved.default_tax_rate);
            }
            if dirty {
                ui.label("(unsaved)");
            }
        });
    }
}

impl eframe::App for CompanyScreen {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .show(ui, |ui| CompanyScreen::ui(self, ui));
    }
}
