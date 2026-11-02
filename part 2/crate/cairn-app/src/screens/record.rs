//! A single-table form: its fields, Access's navigation bar, a find box,
//! and Save, Undo and Close. Moving to another record saves this one
//! first, as Access does, and stays put if it can't.

use super::{buttons, nav_bar, save_key, Move, Screen};
use crate::context::{AppContext, Command, Saved, Target};
use crate::data::Snapshot;
use crate::screens::sheet::SheetOf;
use cairn_forms::{find_box, widgets, FormEvents, RecordForm};
use egui::Ui;

/// A V1 form over one table.
pub trait Table: FormEvents<Context = AppContext> {
    const TITLE: &'static str;
    /// The "Go to:" box's lookup, if V1's form had one.
    const FIND: Option<&'static str> = None;
    /// One record, no new ones (Company Info).
    const SINGLE: bool = false;
    /// Its datasheet view, if it has one.
    const SHEET: Option<SheetOf> = None;

    fn key(&self) -> Option<i32>;
    /// The records, in the order V1's form listed them.
    fn rows(data: &Snapshot) -> &[Self];
    /// A new record with the table's defaults.
    fn blank(ctx: &AppContext) -> Self;
    fn saved(self) -> Saved;

    /// What V1's form had beside its fields: buttons, locked boxes.
    /// Returns true to save the record.
    fn extras(
        _form: &mut RecordForm<Self>,
        _ui: &mut Ui,
        _ctx: &AppContext,
    ) -> bool {
        false
    }
}

pub struct RecordScreen<T: Table> {
    /// Which record, by position; `None` is a new one.
    at: Option<usize>,
    form: RecordForm<T>,
    /// Where to go once this record's save goes through.
    pending: Option<Go>,
}

/// Where to go next: a move in the navigation bar, or the record with a
/// key, picked in the find box.
#[derive(Debug, Clone, Copy)]
enum Go {
    Move(Move),
    Key(i32),
}

impl<T: Table> RecordScreen<T> {
    /// The record with `key`, or the first.
    pub fn open(ctx: &AppContext, key: Option<i32>) -> Self {
        let rows = T::rows(&ctx.data);
        let at = key
            .and_then(|k| {
                rows.iter().position(|r| r.key() == Some(k))
            })
            .or((!rows.is_empty()).then_some(0));
        let mut s = RecordScreen {
            at,
            form: Self::form(ctx, at),
            pending: None,
        };
        s.form.focus_first();
        s
    }

    /// A new record, already filled in (a message for a rep, an activity
    /// for a customer).
    pub fn new_record(record: T) -> Self {
        let mut form = RecordForm::blank((T::TITLE, "new"), record);
        form.focus_first();
        RecordScreen {
            at: None,
            form,
            pending: None,
        }
    }

    fn form(ctx: &AppContext, at: Option<usize>) -> RecordForm<T> {
        match at.map(|i| &T::rows(&ctx.data)[i]) {
            Some(r) => {
                RecordForm::new((T::TITLE, r.key()), r.clone(), ctx)
            }
            None => {
                RecordForm::blank((T::TITLE, "new"), T::blank(ctx))
            }
        }
    }

    fn go(&mut self, ctx: &AppContext, go: Go) {
        let rows = T::rows(&ctx.data);
        self.at = match go {
            Go::Move(m) => m.from(self.at, rows.len()),
            Go::Key(k) => {
                rows.iter().position(|r| r.key() == Some(k))
            }
        };
        self.form = Self::form(ctx, self.at);
        self.form.focus_first();
    }
}

impl<T: Table> Screen for RecordScreen<T> {
    fn ui(&mut self, ui: &mut Ui, ctx: &AppContext) -> Vec<Command> {
        let mut cmds = Vec::new();
        let mut save = save_key(ui);
        let mut go = None;
        ui.horizontal(|ui| {
            widgets::title(ui, T::TITLE, self.form.is_dirty());
            if let Some(list) =
                T::FIND.and_then(|l| ctx.lookup_list(l))
            {
                ui.add_space(40.0);
                if let Some(key) = find_box(ui, "find", list, 200.0) {
                    go = key.parse().ok().map(Go::Key);
                }
            }
        });
        ui.separator();
        self.form.ui(ui, ctx);
        cmds.extend(self.form.take_commands());
        ui.add_space(6.0);
        save |= T::extras(&mut self.form, ui, ctx);
        ui.add_space(6.0);
        let b = buttons(ui, T::SHEET.is_some());
        if let (true, Some(of)) = (b.sheet, T::SHEET) {
            cmds.push(Command::Open(Target::Sheet(of)));
        }
        if !T::SINGLE {
            ui.add_space(4.0);
            let count = T::rows(&ctx.data).len();
            if let Some(m) = nav_bar(ui, self.at, count, true) {
                go = Some(Go::Move(m));
            }
        }
        if b.undo {
            self.form.undo_record();
        }
        if b.close {
            cmds.push(Command::Open(Target::Menu));
        }

        if let Some(go) = go {
            if self.form.is_dirty() {
                // Access saves a record when you leave it.
                if let Some(rec) = self.form.try_save(ui) {
                    self.pending = Some(go);
                    cmds.push(Command::save(rec.saved()));
                }
            } else {
                self.go(ctx, go);
            }
        } else if save {
            if let Some(rec) = self.form.try_save(ui) {
                cmds.push(Command::save(rec.saved()));
            }
        }
        cmds
    }

    fn is_dirty(&self) -> bool {
        self.form.is_dirty()
    }

    fn try_save(&mut self, ui: &mut Ui) -> Option<Saved> {
        self.form.try_save(ui).map(Table::saved)
    }

    fn saved(&mut self, key: i32, ctx: &AppContext) {
        let rows = T::rows(&ctx.data);
        self.at = rows.iter().position(|r| r.key() == Some(key));
        match self.pending.take() {
            Some(go) => self.go(ctx, go),
            None => {
                self.form = Self::form(ctx, self.at);
            }
        }
    }
}
