{% if form_kit or derive_macro %}
//! {{ project_name }}'s form kit: one record edited the way an Access bound
//! form edits one. The record as saved and the draft being edited, the
//! text typed into each box, and an error for a box that doesn't parse.
//!
//! A starting point. Chapter 10 of *Rewrite It in Rust* grows this into
//! a kit with lookups, datasheets, keyboard rules and validation.

use egui::{Grid, RichText, TextEdit, Ui};
{% if derive_macro %}

/// `#[derive(Form)]` writes the `Form` impl from `#[field(...)]`.
pub use {{ crate_name }}_forms_derive::Form;
{% endif %}

/// One field: its name in the struct, and the label beside its box.
#[derive(Debug, Clone, Copy)]
pub struct FieldSpec {
    pub name: &'static str,
    pub label: &'static str,
}

/// A value a field can edit, as text.
pub trait FieldValue {
    fn text(&self) -> String;
    /// Set the value from what was typed, or say why it isn't one.
    fn set_text(&mut self, text: &str) -> Result<(), String>;
}

impl FieldValue for String {
    fn text(&self) -> String {
        self.clone()
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        text.clone_into(self);
        Ok(())
    }
}

impl FieldValue for i32 {
    fn text(&self) -> String {
        self.to_string()
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        *self = text
            .trim()
            .parse()
            .map_err(|_| format!("{text:?} isn't a whole number"))?;
        Ok(())
    }
}

impl FieldValue for f64 {
    fn text(&self) -> String {
        self.to_string()
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        *self = text
            .trim()
            .parse()
            .map_err(|_| format!("{text:?} isn't a number"))?;
        Ok(())
    }
}

/// A record a form can edit: its fields, in Tab order.
pub trait Form: Clone + PartialEq {
    const FIELDS: &'static [FieldSpec];
    fn field(&self, i: usize) -> &dyn FieldValue;
    fn field_mut(&mut self, i: usize) -> &mut dyn FieldValue;
}

/// One record being edited.
pub struct RecordForm<T: Form> {
    saved: T,
    pub draft: T,
    /// What's in each box, which may not parse yet.
    typed: Vec<String>,
    errors: Vec<Option<String>>,
}

impl<T: Form> RecordForm<T> {
    pub fn new(record: T) -> Self {
        let typed = (0..T::FIELDS.len())
            .map(|i| record.field(i).text())
            .collect();
        RecordForm {
            saved: record.clone(),
            draft: record,
            typed,
            errors: vec![None; T::FIELDS.len()],
        }
    }

    /// Unsaved changes, or a box that doesn't parse.
    pub fn is_dirty(&self) -> bool {
        self.draft != self.saved || self.errors.iter().any(Option::is_some)
    }

    /// Back to the record as saved.
    pub fn undo(&mut self) {
        *self = RecordForm::new(self.saved.clone());
    }

    /// The record to save, if every box parses.
    pub fn save(&mut self) -> Option<T> {
        if self.errors.iter().any(Option::is_some) {
            return None;
        }
        self.saved = self.draft.clone();
        Some(self.draft.clone())
    }

    /// The fields as a grid: label, box, and the box's error, if any.
    pub fn ui(&mut self, ui: &mut Ui) {
        Grid::new("record-form").num_columns(3).show(ui, |ui| {
            for (i, spec) in T::FIELDS.iter().enumerate() {
                let label = ui.label(spec.label);
                let edit = ui
                    .add(TextEdit::singleline(&mut self.typed[i]))
                    .labelled_by(label.id);
                if edit.changed() {
                    let typed = &self.typed[i];
                    self.errors[i] =
                        self.draft.field_mut(i).set_text(typed).err();
                }
                let error = self.errors[i].as_deref().unwrap_or("");
                let color = ui.visuals().error_fg_color;
                ui.label(RichText::new(error).color(color));
                ui.end_row();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, PartialEq)]
    struct Price {
        amount: f64,
    }

    impl Form for Price {
        const FIELDS: &'static [FieldSpec] = &[FieldSpec {
            name: "amount",
            label: "Amount",
        }];
        fn field(&self, _: usize) -> &dyn FieldValue {
            &self.amount
        }
        fn field_mut(&mut self, _: usize) -> &mut dyn FieldValue {
            &mut self.amount
        }
    }

    #[test]
    fn a_box_that_does_not_parse_blocks_the_save() {
        let mut form = RecordForm::new(Price { amount: 1.0 });
        form.errors[0] = form.draft.field_mut(0).set_text("1.x").err();
        assert!(form.is_dirty());
        assert!(form.save().is_none());
        form.undo();
        assert!(!form.is_dirty());
    }
}
{% endif %}
