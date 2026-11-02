//! The bridge to `egui_form`: a validation report keyed by field name.
//! `egui_form` draws the red outline and the message under the field, and
//! focuses the first bad field when a save is refused.

use egui_form::{EguiValidationReport, IntoFieldPath};
use std::borrow::Cow;

/// A field's name, as `egui_form` looks errors up by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldName(pub &'static str);

impl IntoFieldPath<FieldName> for FieldName {
    fn into_field_path(self) -> FieldName {
        self
    }
}

/// This frame's errors, one per field at most.
#[derive(Debug, Clone, Default)]
pub struct FieldErrors(pub Vec<(&'static str, String)>);

impl EguiValidationReport for FieldErrors {
    type FieldPath<'a> = FieldName;
    type Errors = Vec<(&'static str, String)>;

    fn get_field_error(
        &self,
        field: FieldName,
    ) -> Option<Cow<'static, str>> {
        self.0
            .iter()
            .find(|(name, _)| *name == field.0)
            .map(|(_, msg)| Cow::Owned(msg.clone()))
    }

    fn has_errors(&self) -> bool {
        !self.0.is_empty()
    }

    fn error_count(&self) -> usize {
        self.0.len()
    }

    fn get_errors(&self) -> Option<&Self::Errors> {
        (!self.0.is_empty()).then_some(&self.0)
    }
}
