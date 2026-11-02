//! A screen per V1 form. Single-table forms share `RecordScreen`; the
//! three with subforms (customers, quotes, orders) have their own.

pub mod customer;
pub mod document;
pub mod record;
pub mod sheet;

use crate::context::{AppContext, Command, Saved};
use cairn_core::Date;
use egui::{Key, KeyboardShortcut, Modifiers, Ui};

/// One open form.
pub trait Screen {
    fn ui(&mut self, ui: &mut Ui, ctx: &AppContext) -> Vec<Command>;

    /// Unsaved changes on the record or its subforms.
    fn is_dirty(&self) -> bool;

    /// Save now, if every field is valid: the record to save. If not,
    /// the first bad field gets the focus.
    fn try_save(&mut self, ui: &mut Ui) -> Option<Saved>;

    /// The save went through, and the record's key is `key`.
    fn saved(&mut self, key: i32, ctx: &AppContext);
}

/// A move in Access's navigation bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    First,
    Prev,
    Next,
    Last,
    New,
}

impl Move {
    /// Where `self` lands, from `at` among `count` records. `None` is the
    /// new, blank record.
    pub fn from(
        self,
        at: Option<usize>,
        count: usize,
    ) -> Option<usize> {
        let last = count.checked_sub(1)?;
        match (self, at) {
            (Move::New, _) => None,
            (Move::First, _) => Some(0),
            (Move::Last, _) | (Move::Prev, None) => Some(last),
            (Move::Prev, Some(i)) => Some(i.saturating_sub(1)),
            (Move::Next, Some(i)) => Some((i + 1).min(last)),
            (Move::Next, None) => None,
        }
    }
}

/// Access's navigation bar: `Record: |< < 12 of 1,000 > >| New`.
/// Page Up and Page Down move a record, as in an Access single form.
pub fn nav_bar(
    ui: &mut Ui,
    at: Option<usize>,
    count: usize,
    allow_new: bool,
) -> Option<Move> {
    let mut mv = None;
    ui.horizontal(|ui| {
        ui.label("Record:");
        if ui.small_button("|<").on_hover_text("First").clicked() {
            mv = Some(Move::First);
        }
        if ui.small_button("<").on_hover_text("Previous").clicked() {
            mv = Some(Move::Prev);
        }
        let place = match at {
            Some(i) => format!(
                "{} of {}",
                thousands(i + 1),
                thousands(count)
            ),
            None => format!("(New) of {}", thousands(count)),
        };
        ui.label(place);
        if ui.small_button(">").on_hover_text("Next").clicked() {
            mv = Some(Move::Next);
        }
        if ui.small_button(">|").on_hover_text("Last").clicked() {
            mv = Some(Move::Last);
        }
        if allow_new
            && ui
                .small_button("New")
                .on_hover_text("New record")
                .clicked()
        {
            mv = Some(Move::New);
        }
    });
    let (up, down) = ui.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::PageUp),
            i.consume_key(Modifiers::NONE, Key::PageDown),
        )
    });
    mv.or(up.then_some(Move::Prev))
        .or(down.then_some(Move::Next))
}

/// Shift+Enter saves the record, as in Access. Take it before the fields
/// draw, or the focused one would treat it as Enter.
pub fn save_key(ui: &mut Ui) -> bool {
    let key = KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter);
    ui.input_mut(|i| i.consume_shortcut(&key))
}

/// The Save, Undo and Close buttons every form has, and Datasheet View
/// for those with one.
pub struct Buttons {
    pub save: bool,
    pub undo: bool,
    pub close: bool,
    pub sheet: bool,
}

pub fn buttons(ui: &mut Ui, has_sheet: bool) -> Buttons {
    ui.horizontal(|ui| Buttons {
        save: ui
            .button("Save")
            .on_hover_text("Shift+Enter, as in Access")
            .clicked(),
        undo: ui.button("Undo").on_hover_text("Esc twice").clicked(),
        close: ui.button("Close").clicked(),
        sheet: has_sheet
            && ui
                .button("Datasheet View")
                .on_hover_text("Every record as a row, as Access's View > Datasheet")
                .clicked(),
    })
    .inner
}

/// A disabled button for something a later chapter builds.
pub fn later(ui: &mut Ui, caption: &str, chapter: u8) {
    ui.add_enabled(false, egui::Button::new(caption))
        .on_disabled_hover_text(format!("Chapter {chapter}"));
}

/// `10/31/2026`
pub fn date_text(d: Date) -> String {
    format!("{}/{}/{}", d.month(), d.day(), d.year())
}

pub fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
