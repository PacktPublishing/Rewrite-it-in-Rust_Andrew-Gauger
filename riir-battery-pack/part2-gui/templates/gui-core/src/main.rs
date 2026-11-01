{% if form_kit or derive_macro %}
//! One record on screen through the form kit in `forms/`: its fields,
//! Save and Undo. egui draws it again every frame from `App`.

use {{ crate_name }}_forms::{Form, RecordForm};
{% if not derive_macro %}
use {{ crate_name }}_forms::{FieldSpec, FieldValue};
{% endif %}

{% if derive_macro %}
/// The fields `#[derive(Form)]` puts on the form, in Tab order.
#[derive(Debug, Clone, PartialEq, Form)]
{% else %}
/// A record for the form. Its `Form` impl is written by hand below; the
/// derive macro option writes it for you.
#[derive(Debug, Clone, PartialEq)]
{% endif %}
struct Customer {
{% if derive_macro %}
    #[field(label = "Company")]
{% endif %}
    company: String,
{% if derive_macro %}
    #[field(label = "Phone")]
{% endif %}
    phone: String,
{% if derive_macro %}
    #[field(label = "Credit Limit")]
{% endif %}
    credit_limit: f64,
}
{% if not derive_macro %}

impl Form for Customer {
    const FIELDS: &'static [FieldSpec] = &[
        FieldSpec { name: "company", label: "Company" },
        FieldSpec { name: "phone", label: "Phone" },
        FieldSpec { name: "credit_limit", label: "Credit Limit" },
    ];

    fn field(&self, i: usize) -> &dyn FieldValue {
        match i {
            0 => &self.company,
            1 => &self.phone,
            _ => &self.credit_limit,
        }
    }

    fn field_mut(&mut self, i: usize) -> &mut dyn FieldValue {
        match i {
            0 => &mut self.company,
            1 => &mut self.phone,
            _ => &mut self.credit_limit,
        }
    }
}
{% endif %}

struct App {
    form: RecordForm<Customer>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Customer");
            self.form.ui(ui);
            ui.horizontal(|ui| {
                let dirty = self.form.is_dirty();
                if ui.add_enabled(dirty, egui::Button::new("Save")).clicked() {
                    // Saved in memory; a real app sends it to the database.
                    let _saved = self.form.save();
                }
                if ui.add_enabled(dirty, egui::Button::new("Undo")).clicked() {
                    self.form.undo();
                }
                if dirty {
                    ui.label("(unsaved)");
                }
            });
        });
    }
}

fn main() -> eframe::Result {
    let customer = Customer {
        company: "Shasta Creamery".into(),
        phone: "555-3383".into(),
        credit_limit: 25_000.0,
    };
    eframe::run_native(
        "{{ project_name }}",
        eframe::NativeOptions::default(),
        Box::new(|_cc| {
            Ok(Box::new(App {
                form: RecordForm::new(customer),
            }))
        }),
    )
}
{% else %}
//! A starting point: one window, drawn again every frame from the app's
//! own state. egui keeps no copy of what's on screen; `App` is the only
//! place the name lives.

struct App {
    name: String,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("{{ project_name }}");
            ui.horizontal(|ui| {
                let label = ui.label("Your name");
                ui.text_edit_singleline(&mut self.name)
                    .labelled_by(label.id);
            });
            // Drawn from `self.name` this frame, so it follows every key.
            if !self.name.is_empty() {
                ui.label(format!("Hello, {}.", self.name));
            }
        });
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "{{ project_name }}",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(App { name: String::new() }))),
    )
}
{% endif %}
