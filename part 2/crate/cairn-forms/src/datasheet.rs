//! Datasheets. V1 had nine; three are editable (quote lines, order lines,
//! payments) and the rest are lists that open a record on double-click.
//! Both are `egui_extras::TableBuilder`, which builds only the rows in
//! view.

use crate::record::{Cmd, Form, FormEvents, Nav, RecordForm};
use egui::{Id, Key, Modifiers, Ui};
use egui_extras::{Column, TableBuilder};

/// A column worked out from the row, not stored (V1's `Line Total`).
pub struct Computed<L> {
    pub label: &'static str,
    pub width: f32,
    pub value: fn(&L) -> String,
}

/// An editable datasheet: one `RecordForm` per row, and a blank row at
/// the end for a new one (Access's `*` row). Arrows move up and down a
/// column, Enter and Tab move along the row and on to the next, Esc
/// undoes, and Ctrl+Minus deletes the row, as in Access.
pub struct Datasheet<L: Form> {
    id: Id,
    pub rows: Vec<RecordForm<L>>,
    new_row: RecordForm<L>,
    blank: L,
    computed: Vec<Computed<L>>,
    added: usize,
    /// Rows deleted since the last save.
    pub deleted: Vec<L>,
}

impl<L: FormEvents> Datasheet<L> {
    pub fn new(
        id_salt: impl egui::AsIdSalt,
        rows: Vec<L>,
        blank: L,
        computed: Vec<Computed<L>>,
        ctx: &L::Context,
    ) -> Self {
        let id = Id::unique(id_salt);
        let rows = rows
            .into_iter()
            .enumerate()
            .map(|(n, row)| RecordForm::new(id.with(n), row, ctx))
            .collect::<Vec<_>>();
        let added = rows.len();
        Datasheet {
            id,
            new_row: RecordForm::blank(
                id.with(("new", added)),
                blank.clone(),
            ),
            rows,
            blank,
            computed,
            added,
            deleted: Vec::new(),
        }
    }

    pub fn records(&self) -> impl Iterator<Item = &L> {
        self.rows.iter().map(|r| &r.draft)
    }

    pub fn is_dirty(&self) -> bool {
        !self.deleted.is_empty()
            || self.rows.iter().any(|r| r.is_dirty())
    }

    pub fn take_commands(&mut self) -> Vec<Cmd<L>> {
        let mut cmds: Vec<_> = self
            .rows
            .iter_mut()
            .flat_map(|r| r.take_commands())
            .collect();
        cmds.extend(self.new_row.take_commands());
        cmds
    }

    /// The first row and field that can't be saved, focused.
    pub fn refuse_bad_row(&mut self) -> bool {
        self.rows.iter_mut().any(|row| row.refuse())
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &L::Context) {
        let fields = L::FIELDS;
        let mut table = TableBuilder::new(ui)
            .id_salt(self.id)
            .striped(true)
            .column(Column::exact(14.0));
        // Hidden fields (a key a lookup is narrowed by) get no column.
        let shown: Vec<usize> = (0..fields.len())
            .filter(|&i| !fields[i].hidden)
            .collect();
        for f in shown.iter().map(|&i| &fields[i]) {
            let w = if f.width > 0.0 { f.width } else { 110.0 };
            table = table.column(Column::initial(w).resizable(true));
        }
        for c in &self.computed {
            table = table.column(Column::initial(c.width));
        }
        let mut moves: Vec<(usize, usize, Nav)> = Vec::new();
        let mut headers = vec![Id::NULL; shown.len()];
        let mut delete = None;
        let rows = self.rows.len();
        table
            .header(20.0, |mut header| {
                header.col(|_| {});
                for (&i, id) in shown.iter().zip(&mut headers) {
                    let f = &fields[i];
                    header.col(|ui| {
                        *id = ui.strong(f.label).id;
                    });
                }
                for c in &self.computed {
                    header.col(|ui| {
                        ui.strong(c.label);
                    });
                }
            })
            .body(|mut body| {
                for r in 0..=rows {
                    let form = if r < rows {
                        &mut self.rows[r]
                    } else {
                        &mut self.new_row
                    };
                    form.begin();
                    body.row(22.0, |mut row| {
                        // The record selector: a dot while unsaved (Access drew a pencil).
                        row.col(|ui| {
                            let mark =
                                match (form.is_dirty(), r == rows) {
                                    (true, _) => "•",
                                    (false, true) => "*",
                                    _ => "",
                                };
                            ui.label(mark);
                        });
                        for (&i, &header) in
                            shown.iter().zip(&headers)
                        {
                            row.col(|ui| {
                                if let Some(nav) = form.field_ui(
                                    ui, i, ctx, header, true,
                                ) {
                                    moves.push((r, i, nav));
                                }
                                let wid = form.id().with(i);
                                let del = ui
                                    .memory(|m| m.has_focus(wid))
                                    && ui.input_mut(|inp| {
                                        inp.consume_key(
                                            Modifiers::COMMAND,
                                            Key::Minus,
                                        )
                                    });
                                if del && r < rows {
                                    delete = Some(r);
                                }
                            });
                        }
                        for c in &self.computed {
                            row.col(|ui| {
                                ui.label((c.value)(&form.draft));
                            });
                        }
                    });
                }
            });
        if let Some(r) = delete {
            let row = self.rows.remove(r);
            if !row.is_new() {
                self.deleted.push(row.draft);
            }
            if let Some(next) = self.rows.get_mut(r) {
                next.focus_first();
            }
        }
        for (r, i, nav) in moves {
            self.navigate(r, i, nav);
        }
        // Typing in the blank row makes it a row, and a new blank appears.
        if self.new_row.is_dirty() {
            self.added += 1;
            let blank = RecordForm::blank(
                self.id.with(("new", self.added)),
                self.blank.clone(),
            );
            let row = std::mem::replace(&mut self.new_row, blank);
            self.rows.push(row);
        }
    }

    /// Row `r`, where one past the last is the blank row.
    fn row_mut(&mut self, r: usize) -> Option<&mut RecordForm<L>> {
        if r == self.rows.len() {
            Some(&mut self.new_row)
        } else {
            self.rows.get_mut(r)
        }
    }

    fn navigate(&mut self, r: usize, i: usize, nav: Nav) {
        match nav {
            Nav::Up | Nav::Down => {
                let target = match nav {
                    Nav::Up => r.checked_sub(1),
                    _ => Some(r + 1),
                };
                if let Some(form) =
                    target.and_then(|t| self.row_mut(t))
                {
                    form.focus_field(i);
                }
            }
            Nav::Next | Nav::Prev => {
                let Some(form) = self.row_mut(r) else { return };
                if form.navigate(i, nav).is_some() && nav == Nav::Next
                {
                    // Off the end of the row: the next row's first field.
                    if let Some(next) = self.row_mut(r + 1) {
                        next.focus_first();
                    }
                }
            }
        }
    }
}
