//! frmQuotes and frmOrders: a document, its lines, and its totals. Orders
//! add payments and the balance due.

use super::{buttons, later, nav_bar, save_key, Move, Screen};
use crate::context::{AppContext, Command, Saved, Target};
use crate::forms::*;
use crate::screens::sheet::SheetOf;
use cairn_core::{totals, CustomerId, DocLine, OrderId, QuoteId};
use cairn_forms::{
    find_box, widgets, Computed, Datasheet, FormEvents, RecordForm,
};
use egui::Ui;

fn money_col<L>(value: fn(&L) -> String) -> Computed<L> {
    Computed {
        label: "Line Total",
        width: 90.0,
        value,
    }
}

fn quote_line_total(l: &QuoteLineDraft) -> String {
    cairn_forms::money(cairn_core::line_total(&l.doc_line()))
}

fn order_line_total(l: &OrderLineDraft) -> String {
    cairn_forms::money(cairn_core::line_total(&l.doc_line()))
}

/// A subform in a group box, `height` points tall.
fn subform<L: FormEvents>(
    ui: &mut Ui,
    id: &str,
    height: f32,
    sheet: &mut Datasheet<L>,
    ctx: &L::Context,
) {
    ui.push_id(id, |ui| {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_height(height);
            sheet.ui(ui, ctx);
        });
    });
}

/// Where a navigation-bar move or the find box goes, by key.
fn target<T>(
    rows: &[T],
    key: impl Fn(&T) -> Option<i32>,
    at: Option<i32>,
    mv: Move,
) -> Option<i32> {
    let i =
        at.and_then(|k| rows.iter().position(|r| key(r) == Some(k)));
    mv.from(i, rows.len()).and_then(|i| key(&rows[i]))
}

/// The header shared by both documents: title, find box, and whether
/// the find box picked a record.
fn header(
    ui: &mut Ui,
    ctx: &AppContext,
    title: &str,
    dirty: bool,
    find: &str,
) -> Option<i32> {
    let mut picked = None;
    ui.horizontal(|ui| {
        widgets::title(ui, title, dirty);
        ui.add_space(40.0);
        if let Some(list) = ctx.lookup_list(find) {
            picked = find_box(ui, "find", list, 160.0)
                .and_then(|k| k.parse().ok());
        }
    });
    ui.separator();
    picked
}

pub struct QuoteScreen {
    pub form: RecordForm<QuoteDraft>,
    pub lines: Datasheet<QuoteLineDraft>,
    /// Where to go once the save goes through.
    pending: Option<Option<i32>>,
}

impl QuoteScreen {
    /// The quote with `id`, or a new one, for `customer` if given.
    pub fn open(
        ctx: &AppContext,
        id: Option<QuoteId>,
        customer: Option<CustomerId>,
    ) -> Self {
        let quote = id.and_then(|id| {
            ctx.data.quotes.iter().find(|q| q.id == Some(id))
        });
        let (form, lines) = match quote {
            Some(q) => (
                RecordForm::new(("quote", q.id), q.clone(), ctx),
                id.and_then(|id| ctx.data.quote_lines.get(&id))
                    .cloned()
                    .unwrap_or_default(),
            ),
            None => (
                RecordForm::blank(
                    ("quote", "new"),
                    QuoteDraft::blank(ctx, customer),
                ),
                Vec::new(),
            ),
        };
        let mut screen = QuoteScreen {
            form,
            lines: Datasheet::new(
                ("quote-lines", id),
                lines,
                QuoteLineDraft::blank(),
                vec![money_col(quote_line_total)],
                ctx,
            ),
            pending: None,
        };
        screen.form.focus_first();
        screen
    }

    fn records(&self) -> Saved {
        Saved::Quote {
            quote: self.form.draft.clone(),
            lines: self.lines.records().cloned().collect(),
        }
    }
}

impl Screen for QuoteScreen {
    fn ui(&mut self, ui: &mut Ui, ctx: &AppContext) -> Vec<Command> {
        let mut cmds = Vec::new();
        let mut save = save_key(ui);
        let dirty = self.is_dirty();
        let picked =
            header(ui, ctx, "Quotes", dirty, "quote_numbers");

        self.form.ui(ui, ctx);
        cmds.extend(self.form.take_commands());
        ui.add_space(8.0);
        subform(ui, "lines", 200.0, &mut self.lines, ctx);
        cmds.extend(self.lines.take_commands());

        let lines: Vec<DocLine> = self
            .lines
            .records()
            .map(QuoteLineDraft::doc_line)
            .collect();
        let d = &self.form.draft;
        let t = totals(&lines, d.tax_rate, d.shipping);
        ui.add_space(8.0);
        let mut b = None;
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    later(ui, "Create Order from Quote", 13);
                    later(ui, "Print Quote", 14);
                });
                b = Some(buttons(ui, true));
            });
            ui.add_space(120.0);
            egui::Grid::new("totals").num_columns(2).show(ui, |ui| {
                widgets::total(ui, "Subtotal", t.subtotal, false);
                widgets::total(ui, "Tax", t.tax, false);
                widgets::total(ui, "Shipping", t.shipping, false);
                widgets::total(ui, "Quote Total", t.total, true);
            });
        });
        let b = b.expect("drawn");
        if b.sheet {
            cmds.push(Command::Open(Target::Sheet(SheetOf::Quotes)));
        }
        let rows = &ctx.data.quotes;
        let key = |q: &QuoteDraft| q.id.map(|i| i.0);
        let at = self.form.draft.id.map(|i| i.0);
        let mv = nav_bar(
            ui,
            rows.iter().position(|q| key(q) == at),
            rows.len(),
            true,
        );
        save |= b.save;
        if b.undo {
            self.form.undo_record();
        }
        if b.close {
            cmds.push(Command::Open(Target::Menu));
        }
        let go = match (picked, mv) {
            (Some(k), _) => Some(Some(k)),
            (None, Some(m)) => Some(target(rows, key, at, m)),
            _ => None,
        };
        if let Some(go) = go {
            if self.is_dirty() {
                if let Some(rec) = self.try_save(ui) {
                    self.pending = Some(go);
                    cmds.push(Command::save(rec));
                }
            } else {
                cmds.push(Command::Open(Target::Quote(
                    go.map(QuoteId),
                )));
            }
        } else if save {
            if let Some(rec) = self.try_save(ui) {
                cmds.push(Command::save(rec));
            }
        }
        cmds
    }

    fn is_dirty(&self) -> bool {
        self.form.is_dirty() || self.lines.is_dirty()
    }

    fn try_save(&mut self, ui: &mut Ui) -> Option<Saved> {
        if self.lines.refuse_bad_row() {
            return None;
        }
        self.form.try_save(ui)?;
        Some(self.records())
    }

    fn saved(&mut self, key: i32, ctx: &AppContext) {
        let go = self.pending.take().unwrap_or(Some(key));
        *self = QuoteScreen::open(ctx, go.map(QuoteId), None);
    }
}

pub struct OrderScreen {
    pub form: RecordForm<OrderDraft>,
    pub lines: Datasheet<OrderLineDraft>,
    pub payments: Datasheet<PaymentDraft>,
    pending: Option<Option<i32>>,
}

impl OrderScreen {
    pub fn open(
        ctx: &AppContext,
        id: Option<OrderId>,
        customer: Option<CustomerId>,
    ) -> Self {
        let order = id.and_then(|id| {
            ctx.data.orders.iter().find(|o| o.id == Some(id))
        });
        let (form, lines, payments) = match order {
            Some(o) => (
                RecordForm::new(("order", o.id), o.clone(), ctx),
                id.and_then(|id| ctx.data.order_lines.get(&id))
                    .cloned()
                    .unwrap_or_default(),
                id.and_then(|id| ctx.data.payments.get(&id))
                    .cloned()
                    .unwrap_or_default(),
            ),
            None => (
                RecordForm::blank(
                    ("order", "new"),
                    OrderDraft::blank(ctx, customer),
                ),
                Vec::new(),
                Vec::new(),
            ),
        };
        let mut screen = OrderScreen {
            form,
            lines: Datasheet::new(
                ("order-lines", id),
                lines,
                OrderLineDraft::blank(),
                vec![money_col(order_line_total)],
                ctx,
            ),
            payments: Datasheet::new(
                ("payments", id),
                payments,
                PaymentDraft::blank(ctx.today),
                Vec::new(),
                ctx,
            ),
            pending: None,
        };
        screen.form.focus_first();
        screen
    }
}

impl Screen for OrderScreen {
    fn ui(&mut self, ui: &mut Ui, ctx: &AppContext) -> Vec<Command> {
        let mut cmds = Vec::new();
        let mut save = save_key(ui);
        let dirty = self.is_dirty();
        let picked =
            header(ui, ctx, "Sales Orders", dirty, "order_numbers");

        self.form.ui(ui, ctx);
        cmds.extend(self.form.take_commands());
        ui.add_space(6.0);
        subform(ui, "lines", 150.0, &mut self.lines, ctx);
        cmds.extend(self.lines.take_commands());

        let lines: Vec<DocLine> = self
            .lines
            .records()
            .map(OrderLineDraft::doc_line)
            .collect();
        let d = &self.form.draft;
        let t = totals(&lines, d.tax_rate, d.shipping);
        let paid = self.payments.records().map(|p| p.amount).sum();
        let mut b = None;
        ui.add_space(6.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.strong("Payments");
                    later(ui, "Send to Manufacturer", 11);
                    later(ui, "Print Receipt", 14);
                });
                ui.set_width(640.0);
                subform(
                    ui,
                    "payments",
                    110.0,
                    &mut self.payments,
                    ctx,
                );
            });
            ui.add_space(16.0);
            egui::Grid::new("totals").num_columns(2).show(ui, |ui| {
                widgets::total(ui, "Subtotal", t.subtotal, false);
                widgets::total(ui, "Tax", t.tax, false);
                widgets::total(ui, "Shipping", t.shipping, false);
                widgets::total(ui, "Order Total", t.total, true);
                widgets::total(ui, "Paid", paid, false);
                widgets::total(
                    ui,
                    "Balance Due",
                    t.total - paid,
                    true,
                );
            });
        });
        cmds.extend(self.payments.take_commands());
        ui.horizontal(|ui| {
            b = Some(buttons(ui, true));
            later(ui, "Print Invoice", 14);
        });
        let b = b.expect("drawn");
        if b.sheet {
            cmds.push(Command::Open(Target::OrderList));
        }
        let rows = &ctx.data.orders;
        let key = |o: &OrderDraft| o.id.map(|i| i.0);
        let at = self.form.draft.id.map(|i| i.0);
        let mv = nav_bar(
            ui,
            rows.iter().position(|o| key(o) == at),
            rows.len(),
            true,
        );
        save |= b.save;
        if b.undo {
            self.form.undo_record();
        }
        if b.close {
            cmds.push(Command::Open(Target::Menu));
        }
        let go = match (picked, mv) {
            (Some(k), _) => Some(Some(k)),
            (None, Some(m)) => Some(target(rows, key, at, m)),
            _ => None,
        };
        if let Some(go) = go {
            if self.is_dirty() {
                if let Some(rec) = self.try_save(ui) {
                    self.pending = Some(go);
                    cmds.push(Command::save(rec));
                }
            } else {
                cmds.push(Command::Open(Target::Order(
                    go.map(OrderId),
                )));
            }
        } else if save {
            if let Some(rec) = self.try_save(ui) {
                cmds.push(Command::save(rec));
            }
        }
        cmds
    }

    fn is_dirty(&self) -> bool {
        self.form.is_dirty()
            || self.lines.is_dirty()
            || self.payments.is_dirty()
    }

    fn try_save(&mut self, ui: &mut Ui) -> Option<Saved> {
        if self.lines.refuse_bad_row()
            || self.payments.refuse_bad_row()
        {
            return None;
        }
        self.form.try_save(ui)?;
        Some(Saved::Order {
            order: self.form.draft.clone(),
            lines: self.lines.records().cloned().collect(),
            payments: self.payments.records().cloned().collect(),
        })
    }

    fn saved(&mut self, key: i32, ctx: &AppContext) {
        let go = self.pending.take().unwrap_or(Some(key));
        *self = OrderScreen::open(ctx, go.map(OrderId), None);
    }
}
