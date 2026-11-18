//! Datasheet view: any form's records as rows, as Access's View >
//! Datasheet showed them. The columns are the form's own fields, shown as
//! the form shows them: names for lookups, `$1,234.50` for money. Built
//! once when the view opens; sorting and filtering are the headings'.

use super::thousands;
use crate::context::{AppContext, Command, Target};
use cairn_core::{CustomerId, QuoteId};
use cairn_forms::{display, money, Form, Kind, ListView};
use egui::Ui;

/// Which form's records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetOf {
    Quotes,
    Customers,
    Products,
    Reps,
    Activities,
    Messages,
    Rmas,
}

pub struct Sheet {
    of: SheetOf,
    title: &'static str,
    columns: Vec<(String, f32)>,
    rows: Vec<Vec<String>>,
    keys: Vec<Option<i32>>,
    list: ListView,
}

/// A field as its form shows it.
fn cell<T: Form<Context = AppContext>>(
    r: &T,
    i: usize,
    ctx: &AppContext,
) -> String {
    use cairn_forms::FormContext;
    let value = r.field(i).text();
    match T::FIELDS[i].kind {
        Kind::Lookup { list, filter, .. } => {
            let filter = filter.map(|f| r.field(f).text());
            ctx.lookup(list, filter.as_deref())
                .and_then(|l| l.by_key(&value))
                .map_or(value, |row| row.text.clone())
        }
        Kind::Check => match value.as_str() {
            "true" => "Yes".into(),
            _ => String::new(),
        },
        kind => display(kind, &value).replace('\n', " "),
    }
}

/// The form's fields as columns, and its records as rows.
fn text<T: Form<Context = AppContext>>(
    rows: &[T],
    ctx: &AppContext,
) -> (Vec<(String, f32)>, Vec<Vec<String>>) {
    let fields: Vec<usize> = (0..T::FIELDS.len())
        .filter(|&i| !T::FIELDS[i].hidden)
        .collect();
    let columns = fields
        .iter()
        .map(|&i| {
            let f = &T::FIELDS[i];
            let width = match (f.lines, f.width, f.kind) {
                (2.., _, _) => 240.0,
                (_, w, _) if w > 0.0 => w.min(240.0),
                (_, _, Kind::Text | Kind::Lookup { .. }) => 160.0,
                _ => 90.0,
            };
            (f.label.to_string(), width)
        })
        .collect();
    let rows = rows
        .iter()
        .map(|r| fields.iter().map(|&i| cell(r, i, ctx)).collect())
        .collect();
    (columns, rows)
}

impl Sheet {
    pub fn build(of: SheetOf, ctx: &AppContext) -> Self {
        let d = &ctx.data;
        let (title, (mut columns, mut rows), keys): (_, _, Vec<_>) =
            match of {
                SheetOf::Quotes => (
                    "Quotes",
                    text(&d.quotes, ctx),
                    d.quotes
                        .iter()
                        .map(|q| q.id.map(|i| i.0))
                        .collect(),
                ),
                SheetOf::Customers => (
                    "Customers",
                    text(&d.customers, ctx),
                    d.customers
                        .iter()
                        .map(|c| c.id.map(|i| i.0))
                        .collect(),
                ),
                SheetOf::Products => (
                    "Products",
                    text(&d.products, ctx),
                    d.products
                        .iter()
                        .map(|p| p.id.map(|i| i.0))
                        .collect(),
                ),
                SheetOf::Reps => (
                    "Sales Reps",
                    text(&d.reps, ctx),
                    d.reps
                        .iter()
                        .map(|r| r.id.map(|i| i.0))
                        .collect(),
                ),
                SheetOf::Activities => (
                    "Activities",
                    text(&d.activities, ctx),
                    d.activities.iter().map(|a| a.id).collect(),
                ),
                SheetOf::Messages => (
                    "Phone Messages",
                    text(&d.messages, ctx),
                    d.messages.iter().map(|m| m.id).collect(),
                ),
                SheetOf::Rmas => (
                    "Returns (RMAs)",
                    text(&d.rmas, ctx),
                    d.rmas.iter().map(|r| r.id).collect(),
                ),
            };
        // frmQuotes' Quote Total box was a calculated control; its
        // datasheet view showed it as a column.
        if of == SheetOf::Quotes {
            columns.push(("Quote Total".into(), 110.0));
            for (row, q) in rows.iter_mut().zip(&d.quotes) {
                let total =
                    q.id.and_then(|id| ctx.quote_totals.get(&id));
                row.push(
                    total.map(|t| money(t.total)).unwrap_or_default(),
                );
            }
        }
        Sheet {
            of,
            title,
            columns,
            rows,
            keys,
            list: ListView::default(),
        }
    }

    /// The record to open in its form, for a row's key.
    fn target(&self, key: i32) -> Target {
        match self.of {
            SheetOf::Quotes => Target::Quote(Some(QuoteId(key))),
            SheetOf::Customers => {
                Target::Customer(Some(CustomerId(key)))
            }
            SheetOf::Products => Target::Product(Some(key)),
            SheetOf::Reps => Target::Rep(Some(key)),
            SheetOf::Activities => Target::Activity(Some(key)),
            SheetOf::Messages => Target::Message(Some(key)),
            SheetOf::Rmas => Target::Rma(Some(key)),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) -> Vec<Command> {
        ui.horizontal(|ui| {
            let (shown, total) = self.list.counts();
            let count = match self.list.is_filtered() {
                true => format!(
                    "{} of {} (Filtered)",
                    thousands(shown),
                    thousands(total)
                ),
                false => thousands(self.rows.len()),
            };
            ui.heading(format!("{}: {count}", self.title));
            ui.label(
                "A heading's menu sorts and filters. \
                 Double-click or Enter opens the form.",
            );
        });
        // Wider than the window, as most forms' datasheets are: scroll
        // sideways.
        let open = egui::ScrollArea::horizontal()
            .show(ui, |ui| {
                self.list.ui_text(
                    ui,
                    self.title,
                    &self.rows,
                    &self.columns,
                )
            })
            .inner;
        open.and_then(|i| self.keys[i])
            .map(|k| Command::Open(self.target(k)))
            .into_iter()
            .collect()
    }
}
