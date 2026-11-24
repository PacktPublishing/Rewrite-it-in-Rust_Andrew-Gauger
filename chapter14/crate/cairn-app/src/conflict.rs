//! The conflict dialog (Chapter 12). Another rep saved the record first.
//! The dialog lists what came in from them, what stays from this rep,
//! and asks only about a field both changed. No button overwrites
//! without showing what it overwrites.

use crate::context::Gone;
use crate::context::{AppContext, Saved};
use cairn_forms::{
    merge_rows, Datasheet, Form, FormEvents, Keyed, Merge, Question,
    RowsMerge, Section, Side,
};
use egui::{Id, Modal, Ui};
use std::cell::RefCell;
use std::rc::Rc;

/// What the rep did with the dialog.
pub enum Settled {
    /// Save the merged record, over the version Postgres has now.
    Save(Box<Saved>),
    /// Back to the form, with the draft as typed.
    Keep,
}

/// A conflict being settled, whatever the record's type.
pub trait Settle {
    /// Draw the dialog; `Some` once the rep has answered.
    fn ui(
        &mut self,
        ui: &mut Ui,
        ctx: &AppContext,
    ) -> Option<Settled>;
}

/// The three-way comparison of one record, any subforms' rows, and how
/// to put the merged record back into a save.
pub struct Settling<T: Form> {
    /// The form's name, for the title of the dialog's window.
    pub title: &'static str,
    pub merge: Merge<T>,
    /// The subforms' rows: lines, contacts, activities, payments.
    pub sections: Vec<Box<dyn Section>>,
    pub wrap: Box<dyn Fn(T) -> Saved>,
    id: Id,
}

impl<T: Form> Settling<T> {
    pub fn new(
        title: &'static str,
        merge: Merge<T>,
        wrap: impl Fn(T) -> Saved + 'static,
    ) -> Self {
        Settling {
            title,
            merge,
            sections: Vec::new(),
            wrap: Box::new(wrap),
            id: Id::unique("conflict"),
        }
    }

    pub fn with_section(mut self, s: impl Section + 'static) -> Self {
        self.sections.push(Box::new(s));
        self
    }

    fn resolved(&self) -> bool {
        self.merge.resolved()
            && self.sections.iter().all(|s| s.resolved())
    }

    /// The dialog's body, so a snapshot test can draw it without a
    /// modal around it. Returns (save, keep).
    pub fn body(&mut self, ui: &mut Ui) -> (bool, bool) {
        ui.heading("Someone else changed this record");
        ui.label("Another rep saved it after you opened it.");
        ui.add_space(6.0);
        let asking = !self.merge.asks.is_empty()
            || self
                .sections
                .iter()
                .any(|s| !s.questions().is_empty());
        if asking {
            ui.label("Pick one for each question below.");
        }
        section_ui(ui, &mut self.merge);
        for s in &mut self.sections {
            section_ui(ui, s.as_mut());
        }
        ui.add_space(8.0);
        let resolved = self.resolved();
        ui.horizontal(|ui| {
            let save = ui
                .add_enabled(
                    resolved,
                    egui::Button::new("Save merged record"),
                )
                .clicked();
            let keep = ui.button("Keep editing").clicked();
            (save, keep)
        })
        .inner
    }
}

/// One section: what was settled without asking, then its questions.
fn section_ui(ui: &mut Ui, s: &mut dyn Section) {
    let (lines, questions) = (s.settled(), s.questions());
    if lines.is_empty() && questions.is_empty() {
        return;
    }
    let name = s.name();
    if !name.is_empty() {
        ui.add_space(4.0);
        ui.strong(name);
    }
    for l in lines {
        ui.label(l);
    }
    let mut chosen = Vec::new();
    for (n, q) in questions.iter().enumerate() {
        ui.add_space(4.0);
        ui.strong(&q.label);
        ui.horizontal(|ui| {
            if ui
                .selectable_label(
                    q.choice == Some(Side::Mine),
                    &q.mine,
                )
                .clicked()
            {
                chosen.push((n, Side::Mine));
            }
            if ui
                .selectable_label(
                    q.choice == Some(Side::Theirs),
                    &q.theirs,
                )
                .clicked()
            {
                chosen.push((n, Side::Theirs));
            }
        });
        if let Some(o) = &q.original {
            ui.label(format!("It was: {o}"));
        }
    }
    for (n, side) in chosen {
        s.answer(n, side);
    }
}

impl<T: Form> Settle for Settling<T> {
    fn ui(
        &mut self,
        ui: &mut Ui,
        _ctx: &AppContext,
    ) -> Option<Settled> {
        let modal =
            Modal::new(self.id).show(ui.ctx(), |ui| self.body(ui));
        let (save, keep) = modal.inner;
        if save {
            Some(Settled::Save(Box::new((self.wrap)(
                self.merge.merged.clone(),
            ))))
        } else if keep || modal.should_close() {
            Some(Settled::Keep)
        } else {
            None
        }
    }
}

/// One subform's rows, compared, and shared with the save that will
/// send them once the rep has answered.
pub struct Rows<R: Keyed>(Rc<RefCell<RowsMerge<R>>>);

impl<R: Keyed> Clone for Rows<R> {
    fn clone(&self) -> Self {
        Rows(self.0.clone())
    }
}

impl<R: Keyed> Rows<R> {
    /// The datasheet's rows against `theirs`, the rows as Postgres has
    /// them now.
    pub fn compare(
        name: &'static str,
        sheet: &Datasheet<R>,
        theirs: Vec<R>,
        describe: fn(&R) -> String,
    ) -> Self
    where
        R: FormEvents,
    {
        let mine: Vec<R> = sheet.records().cloned().collect();
        let mut originals: Vec<R> =
            sheet.originals().cloned().collect();
        originals.extend(sheet.deleted.iter().cloned());
        Rows(Rc::new(RefCell::new(merge_rows(
            name,
            &originals,
            &mine,
            &sheet.deleted,
            &theirs,
            describe,
        ))))
    }

    /// The rows as they will stand, the ones to delete (at the version
    /// Postgres has now), and the keys of the saved rows to write.
    pub fn result(&self) -> (Vec<R>, Vec<Gone>, Vec<i32>) {
        let m = self.0.borrow();
        let gone = m
            .removed()
            .iter()
            .filter_map(|r| {
                r.key().map(|id| Gone {
                    id,
                    version: r.version(),
                })
            })
            .collect();
        (m.rows(), gone, m.written())
    }
}

impl<R: Keyed> Section for Rows<R> {
    fn name(&self) -> String {
        self.0.borrow().name()
    }
    fn settled(&self) -> Vec<String> {
        self.0.borrow().settled()
    }
    fn questions(&self) -> Vec<Question> {
        self.0.borrow().questions()
    }
    fn answer(&mut self, question: usize, side: Side) {
        self.0.borrow_mut().answer(question, side)
    }
}
