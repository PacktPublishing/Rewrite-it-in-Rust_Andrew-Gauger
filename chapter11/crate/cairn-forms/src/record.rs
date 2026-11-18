//! `RecordForm<T>`: one record being edited, the way an Access bound
//! form edits one. It keeps the record as loaded and the draft, so it
//! knows when there are unsaved changes (Access's pencil) and can undo.
//! Esc undoes the field, and a second Esc the record. Enter moves to the
//! next field, and AfterUpdate fires when a changed field is left.

use crate::field::{display, FieldSpec, FieldValue, Kind};
use crate::lookup::{suggestion, unwrapped, LookupList};
use crate::report::{FieldErrors, FieldName};
use egui::text::CCursorRange;
use egui::text_edit::TextEditState;
use egui::{
    AsIdSalt, CompletionPopup, EventFilter, Id, Key, Response,
    Stroke, StrokeKind, TextEdit, Ui,
};
use egui_form::FormField;

/// What a form's handlers can see: the lookups its combo boxes offer,
/// and whatever the app adds (the company row, cached customer rows). It
/// also names the commands handlers return instead of doing input or
/// output themselves.
pub trait FormContext {
    type Command;

    /// The rows for lookup `list`, narrowed by `filter` when the field
    /// has one (the customer whose contacts to offer).
    fn lookup(
        &self,
        list: &str,
        filter: Option<&str>,
    ) -> Option<&LookupList>;
}

/// A command a form's handler returns, for the app to run.
pub type Cmd<T> = <<T as Form>::Context as FormContext>::Command;

/// A record a form can edit. `#[derive(Form)]` writes this.
pub trait Form: Clone + PartialEq + 'static {
    type Context: FormContext;

    /// The fields, in Tab order.
    const FIELDS: &'static [FieldSpec];

    fn field(&self, i: usize) -> &dyn FieldValue;
    fn field_mut(&mut self, i: usize) -> &mut dyn FieldValue;

    /// The field's `validate` rule, if it has one.
    fn check(&self, i: usize) -> Result<(), String>;

    /// The field's `after_update` handler, if it has one.
    fn after_update(
        &mut self,
        i: usize,
        ctx: &Self::Context,
    ) -> Vec<Cmd<Self>>;

    /// Faint text an empty field shows, from its `placeholder` function:
    /// what will be used if it's left blank.
    fn placeholder(
        &self,
        i: usize,
        ctx: &Self::Context,
    ) -> Option<String>;
}

/// V1's form-level events. Each has a default that does nothing, so a
/// form writes only the ones it had in V1.
pub trait FormEvents: Form {
    /// The first change to a new record (V1's `BeforeInsert`).
    fn before_insert(
        &mut self,
        _ctx: &Self::Context,
    ) -> Vec<Cmd<Self>> {
        Vec::new()
    }

    /// A record was loaded into the form (V1's `OnCurrent`).
    fn on_current(&mut self, _ctx: &Self::Context) -> Vec<Cmd<Self>> {
        Vec::new()
    }
}

/// Where the keyboard asked to go from a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Next,
    Prev,
    Up,
    Down,
}

/// The field being typed in.
struct Edit {
    field: usize,
    buffer: String,
    buffer_at_focus: String,
    value_at_focus: String,
}

pub struct RecordForm<T: Form> {
    id: Id,
    original: T,
    pub draft: T,
    is_new: bool,
    inserted: bool,
    edit: Option<Edit>,
    /// Text that didn't parse, and why, per field.
    bad_text: Vec<Option<(String, String)>>,
    focus: Option<usize>,
    /// A focus move waits a frame, or the field it lands on would see
    /// the same Enter and pass the focus on again.
    focus_wait: bool,
    /// A save was refused, so a datasheet row shows its errors even
    /// before it's touched.
    refused: bool,
    report: Option<egui_form::Form<FieldErrors>>,
    commands: Vec<Cmd<T>>,
}

impl<T: FormEvents> RecordForm<T> {
    /// A form showing an existing record.
    pub fn new(
        id_salt: impl AsIdSalt,
        record: T,
        ctx: &T::Context,
    ) -> Self {
        let mut form = RecordForm {
            id: Id::unique(id_salt),
            original: record.clone(),
            draft: record,
            is_new: false,
            inserted: false,
            edit: None,
            bad_text: vec![None; T::FIELDS.len()],
            focus: None,
            focus_wait: false,
            refused: false,
            report: None,
            commands: Vec::new(),
        };
        let cmds = form.draft.on_current(ctx);
        form.original = form.draft.clone();
        form.commands.extend(cmds);
        form
    }

    /// A form on a new, blank record. `before_insert` runs on its first
    /// change.
    pub fn blank(id_salt: impl AsIdSalt, blank: T) -> Self {
        RecordForm {
            id: Id::unique(id_salt),
            original: blank.clone(),
            draft: blank,
            is_new: true,
            inserted: false,
            edit: None,
            bad_text: vec![None; T::FIELDS.len()],
            focus: None,
            focus_wait: false,
            refused: false,
            report: None,
            commands: Vec::new(),
        }
    }

    pub fn id(&self) -> Id {
        self.id
    }

    pub fn is_new(&self) -> bool {
        self.is_new
    }

    /// Unsaved changes: Access's pencil in the record selector.
    pub fn is_dirty(&self) -> bool {
        self.draft != self.original
            || self.bad_text.iter().any(Option::is_some)
    }

    pub fn undo_record(&mut self) {
        self.draft = self.original.clone();
        self.bad_text.fill(None);
        self.inserted = false;
    }

    /// Move the keyboard to field `i` on the next frame.
    pub fn focus_field(&mut self, i: usize) {
        self.focus = Some(i);
    }

    pub fn focus_first(&mut self) {
        self.focus = self.next_editable(None);
    }

    pub fn take_commands(&mut self) -> Vec<Cmd<T>> {
        std::mem::take(&mut self.commands)
    }

    /// The field after `i` (or the first) that Tab stops at.
    pub fn next_editable(&self, i: Option<usize>) -> Option<usize> {
        let start = i.map_or(0, |i| i + 1);
        (start..T::FIELDS.len()).find(|&j| Self::stops(j))
    }

    pub fn prev_editable(&self, i: usize) -> Option<usize> {
        (0..i).rev().find(|&j| Self::stops(j))
    }

    /// Does Tab stop at field `j`?
    fn stops(j: usize) -> bool {
        !T::FIELDS[j].read_only && !T::FIELDS[j].hidden
    }

    /// Why field `i` can't be saved, if it can't.
    pub fn error(&self, i: usize) -> Option<String> {
        if let Some((_, msg)) = &self.bad_text[i] {
            return Some(msg.clone());
        }
        let spec = &T::FIELDS[i];
        if spec.required && self.draft.field(i).is_empty() {
            return Some(format!("{} is required", spec.label));
        }
        self.draft.check(i).err()
    }

    /// Refuse a save: show this record's errors and focus the first.
    pub fn refuse(&mut self) -> bool {
        match self.first_error() {
            Some(i) => {
                self.refused = true;
                self.focus = Some(i);
                true
            }
            None => false,
        }
    }

    pub fn first_error(&self) -> Option<usize> {
        (0..T::FIELDS.len()).find(|&i| self.error(i).is_some())
    }

    /// Accept the draft as saved, if every field is valid. If not,
    /// `egui_form` outlines every bad field and focuses the first.
    /// Returns the record to save.
    pub fn try_save(&mut self, ui: &mut Ui) -> Option<T> {
        if let Some(report) = &mut self.report {
            if report.try_submit(ui).is_err() {
                // `egui_form` has focused the first bad field.
                self.focus = None;
                return None;
            }
        } else if let Some(i) = self.first_error() {
            self.focus = Some(i);
            return None;
        }
        self.original = self.draft.clone();
        self.is_new = false;
        Some(self.draft.clone())
    }

    /// The record as saved: the form is clean again.
    pub fn saved(&mut self, record: T) {
        self.original = record.clone();
        self.draft = record;
        self.is_new = false;
    }

    /// The whole form: each column of fields is a grid of labels and
    /// fields. The derive's field order is the Tab order, as V1's
    /// creation order was.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        ctx: &T::Context,
    ) -> Option<Nav> {
        self.begin();
        let columns =
            T::FIELDS.iter().map(|f| f.column).max().unwrap_or(0) + 1;
        let mut out = None;
        ui.horizontal_top(|ui| {
            for col in 0..columns {
                egui::Grid::new(self.id.with(("column", col)))
                    .num_columns(2)
                    .spacing([8.0, 0.0])
                    .show(ui, |ui| {
                        for i in 0..T::FIELDS.len() {
                            if T::FIELDS[i].column != col
                                || T::FIELDS[i].hidden
                            {
                                continue;
                            }
                            // Top-aligned: `egui_form` keeps a line under
                            // each field for its error.
                            let label = ui
                                .vertical(|ui| {
                                    ui.add_space(3.0);
                                    ui.add(
                                        egui::Label::new(T::FIELDS[i].label)
                                            .wrap_mode(egui::TextWrapMode::Extend),
                                    )
                                })
                                .inner;
                            let nav =
                                self.field_ui(ui, i, ctx, label.id, false);
                            if let Some(nav) = nav {
                                out = self.navigate(i, nav).or(out);
                            }
                            ui.end_row();
                        }
                    });
                ui.add_space(16.0);
            }
        });
        out
    }

    /// Move within the form. Returns the move if it leaves the form
    /// (Enter on the last field).
    pub fn navigate(&mut self, i: usize, nav: Nav) -> Option<Nav> {
        let target = match nav {
            Nav::Next | Nav::Down => self.next_editable(Some(i)),
            Nav::Prev | Nav::Up => self.prev_editable(i),
        };
        match target {
            Some(j) => {
                self.focus = Some(j);
                self.focus_wait = true;
                None
            }
            None => Some(nav),
        }
    }

    /// Start a frame: this frame's errors go to `egui_form`.
    pub fn begin(&mut self) {
        self.focus_wait = false;
        let errors = (0..T::FIELDS.len())
            .filter_map(|i| {
                self.error(i).map(|e| (T::FIELDS[i].name, e))
            })
            .collect();
        self.report = Some(
            egui_form::Form::new().add_report(FieldErrors(errors)),
        );
    }

    /// One field, named by `label` (its label, or in a datasheet its
    /// column header). On a form, `egui_form` draws its error under it. In
    /// a datasheet cell, the error is an outline and a tooltip.
    pub fn field_ui(
        &mut self,
        ui: &mut Ui,
        i: usize,
        ctx: &T::Context,
        label: Id,
        in_sheet: bool,
    ) -> Option<Nav> {
        let spec = T::FIELDS[i];
        let wid = self.id.with(i);
        if self.focus == Some(i)
            && !self.focus_wait
            && spec.kind != Kind::Check
        {
            // The request stays pending until the field has the focus: a
            // grid's first frame only measures, and drops it.
            ui.memory_mut(|m| m.request_focus(wid));
        }
        let focused = ui.memory(|m| m.has_focus(wid));
        let editing =
            self.edit.as_ref().is_some_and(|e| e.field == i);
        if editing && !focused {
            // The focus left before this frame drew the field: Tab, a
            // click elsewhere, or Esc, which egui handles first.
            let (escape, enter) = ui.input(|inp| {
                (
                    inp.key_pressed(Key::Escape),
                    inp.key_pressed(Key::Enter),
                )
            });
            return self.finish_edit(ui, i, ctx, escape, enter);
        }
        let list = match spec.kind {
            Kind::Lookup { list, filter, .. } => {
                let filter =
                    filter.map(|f| self.draft.field(f).text());
                ctx.lookup(list, filter.as_deref())
            }
            _ => None,
        };
        let limit =
            matches!(spec.kind, Kind::Lookup { limit: true, .. });
        if focused && !editing {
            self.start_edit(i, list);
            // Arriving by keyboard selects the whole field, as Access
            // does, so typing replaces it. A click places the cursor.
            let clicked = ui.input(|inp| inp.pointer.any_pressed());
            if let (false, Some(e)) = (clicked, &self.edit) {
                let mut state = TextEditState::load(ui.ctx(), wid)
                    .unwrap_or_default();
                let all = CCursorRange::select_all_str(&e.buffer);
                state.cursor.set_char_range(Some(all));
                state.store(ui.ctx(), wid);
            }
        }

        if spec.kind == Kind::Check {
            return self.check_ui(ui, i, ctx, label);
        }

        let mut edit = self.edit.take();
        let mut shown = self.shown_text(i, list);
        let at_focus = edit
            .as_ref()
            .map(|e| e.buffer_at_focus.clone())
            .unwrap_or_default();
        let buffer = match &mut edit {
            Some(e) if e.field == i => &mut e.buffer,
            _ => &mut shown,
        };
        let width = match (spec.width, spec.kind) {
            (w, _) if w > 0.0 => w,
            (_, Kind::Lookup { .. }) => 200.0,
            (_, Kind::Text) => 200.0,
            _ => 100.0,
        };
        let enter_or_tab = ui.input(|inp| {
            inp.modifiers.is_none()
                && (inp.key_pressed(Key::Enter)
                    || inp.key_pressed(Key::Tab))
        });
        let placeholder = self.draft.placeholder(i, ctx);
        let mut accepted = None;
        let mut widget = |ui: &mut Ui| -> Response {
            let text_edit = if spec.lines > 1 {
                TextEdit::multiline(buffer)
                    .desired_rows(spec.lines.into())
            } else {
                // Up and down are the form's to handle (next field, or the
                // row above and below in a datasheet), not egui's.
                TextEdit::singleline(buffer).event_filter(
                    EventFilter {
                        vertical_arrows: true,
                        horizontal_arrows: true,
                        ..Default::default()
                    },
                )
            }
            .id(wid)
            .interactive(!spec.read_only)
            .desired_width(width);
            let text_edit = match &placeholder {
                Some(text) => text_edit.hint_text(text.as_str()),
                None => text_edit,
            };
            // A locked field reads as part of the form, as Access drew it.
            let text_edit = match spec.read_only {
                true => text_edit
                    .background_color(ui.visuals().panel_fill),
                false => text_edit,
            };
            match list {
                Some(list) if !spec.read_only => {
                    let ctx = ui.ctx().clone();
                    let output = unwrapped(&ctx, || {
                        CompletionPopup::new(wid)
                            .word_boundary(|_| false)
                            .show(ui, text_edit, |q| {
                                if q.text == at_focus {
                                    return Vec::new();
                                }
                                list.rank(q.text, 12)
                                    .into_iter()
                                    .map(suggestion)
                                    .collect()
                            })
                    });
                    accepted = output.accepted.map(|s| s.insert);
                    output.text_edit.response.response
                }
                _ => ui.add(text_edit),
            }
        };
        let mut response = match in_sheet {
            false => {
                let report =
                    self.report.as_mut().expect("begin() first");
                FormField::new(report, FieldName(spec.name))
                    .ui(ui, widget)
                    .labelled_by(label)
            }
            true => {
                let response = widget(ui).labelled_by(label);
                let shown = self.refused || self.is_dirty();
                match self.error(i).filter(|_| shown) {
                    Some(err) => {
                        let color = ui.visuals().error_fg_color;
                        ui.painter().rect_stroke(
                            response.rect,
                            2.0,
                            Stroke::new(1.0, color),
                            StrokeKind::Inside,
                        );
                        response.on_hover_text(err)
                    }
                    None => response,
                }
            }
        };
        if !spec.hint.is_empty() {
            response = response.on_hover_text(spec.hint);
        }
        let typed = edit
            .as_ref()
            .filter(|e| e.field == i)
            .map(|e| e.buffer.clone());
        self.edit = edit;

        if let Some(key) = accepted {
            // A row picked from the list: AfterUpdate now, as V1's combo
            // fired on selection.
            let _ = self.draft.field_mut(i).set_text(&key);
            if let (Some(e), Some(list)) = (&mut self.edit, list) {
                e.buffer = list
                    .by_key(&key)
                    .map(|r| r.text.clone())
                    .unwrap_or_default();
                e.buffer_at_focus = e.buffer.clone();
            }
            self.changed(i, ctx);
            return enter_or_tab.then_some(Nav::Next);
        }
        // Typing sets the value as it goes, except in a lookup limited
        // to its list, which waits for a pick.
        if let (Some(text), true, false) =
            (&typed, response.changed(), limit)
        {
            match self.draft.field_mut(i).set_text(text) {
                Ok(()) => self.bad_text[i] = None,
                Err(msg) => {
                    self.bad_text[i] = Some((text.clone(), msg))
                }
            }
            self.mark_inserted(ctx);
        }
        let still_focused = ui.memory(|m| m.has_focus(wid));
        if still_focused && self.focus == Some(i) {
            self.focus = None;
        }
        if typed.is_some() && !still_focused {
            let (escape, enter) = ui.input(|inp| {
                (
                    inp.key_pressed(Key::Escape),
                    inp.key_pressed(Key::Enter),
                )
            });
            return self.finish_edit(ui, i, ctx, escape, enter);
        }
        if still_focused && spec.lines <= 1 {
            let (up, down) = ui.input(|inp| {
                (
                    inp.key_pressed(Key::ArrowUp),
                    inp.key_pressed(Key::ArrowDown),
                )
            });
            if down {
                return Some(Nav::Down);
            }
            if up {
                return Some(Nav::Up);
            }
        }
        None
    }

    fn check_ui(
        &mut self,
        ui: &mut Ui,
        i: usize,
        ctx: &T::Context,
        label: Id,
    ) -> Option<Nav> {
        let mut value = self.draft.field(i).text() == "true";
        let response = ui.checkbox(&mut value, "").labelled_by(label);
        if self.focus == Some(i) && !self.focus_wait {
            response.request_focus();
            if response.has_focus() {
                self.focus = None;
            }
        }
        if response.changed() {
            let _ =
                self.draft.field_mut(i).set_text(&value.to_string());
            self.changed(i, ctx);
        }
        let enter = ui.input(|inp| inp.key_pressed(Key::Enter));
        (response.has_focus() && enter).then_some(Nav::Next)
    }

    /// What field `i` shows when it isn't being typed in.
    fn shown_text(
        &self,
        i: usize,
        list: Option<&LookupList>,
    ) -> String {
        if let Some((text, _)) = &self.bad_text[i] {
            return text.clone();
        }
        let value = self.draft.field(i).text();
        match T::FIELDS[i].kind {
            Kind::Lookup { .. } => list
                .and_then(|l| l.by_key(&value))
                .map(|r| r.text.clone())
                .unwrap_or(value),
            kind => display(kind, &value),
        }
    }

    fn start_edit(&mut self, i: usize, list: Option<&LookupList>) {
        let buffer = match (&self.bad_text[i], T::FIELDS[i].kind) {
            (Some((text, _)), _) => text.clone(),
            (None, Kind::Lookup { .. }) => self.shown_text(i, list),
            (None, _) => self.draft.field(i).text(),
        };
        self.edit = Some(Edit {
            field: i,
            buffer_at_focus: buffer.clone(),
            buffer,
            value_at_focus: self.draft.field(i).text(),
        });
    }

    /// Leaving field `i`: Esc undoes, anything else commits.
    fn finish_edit(
        &mut self,
        ui: &mut Ui,
        i: usize,
        ctx: &T::Context,
        escape: bool,
        enter: bool,
    ) -> Option<Nav> {
        let edit = self.edit.take()?;
        let value = self.draft.field(i).text();
        if escape {
            let touched = edit.buffer != edit.buffer_at_focus
                || value != edit.value_at_focus;
            if touched {
                let _ = self
                    .draft
                    .field_mut(i)
                    .set_text(&edit.value_at_focus);
                self.bad_text[i] = None;
            } else {
                self.undo_record();
            }
            self.focus = Some(i);
            self.focus_wait = true;
            ui.ctx().request_repaint();
            return None;
        }
        if let Kind::Lookup {
            list,
            filter,
            limit: true,
        } = T::FIELDS[i].kind
        {
            // Limit to list: typed text must name exactly one row.
            if edit.buffer != edit.buffer_at_focus {
                let filter =
                    filter.map(|f| self.draft.field(f).text());
                let key = match ctx.lookup(list, filter.as_deref()) {
                    _ if edit.buffer.trim().is_empty() => {
                        Some(String::new())
                    }
                    Some(l) => {
                        l.exact(&edit.buffer).map(|r| r.key.clone())
                    }
                    None => None,
                };
                if let Some(key) = key {
                    let _ = self.draft.field_mut(i).set_text(&key);
                }
            }
        }
        if self.draft.field(i).text() != edit.value_at_focus {
            self.changed(i, ctx);
        }
        enter.then_some(Nav::Next)
    }

    /// Field `i` has a new value: run its AfterUpdate.
    fn changed(&mut self, i: usize, ctx: &T::Context) {
        self.mark_inserted(ctx);
        let cmds = self.draft.after_update(i, ctx);
        self.commands.extend(cmds);
    }

    fn mark_inserted(&mut self, ctx: &T::Context) {
        if self.is_new
            && !self.inserted
            && self.draft != self.original
        {
            self.inserted = true;
            let cmds = self.draft.before_insert(ctx);
            self.commands.extend(cmds);
        }
    }
}
