//! frmQuotes and frmOrders: a document, its lines, and its totals. Orders
//! add payments and the balance due.

use super::{
    buttons, later, nav_bar, print_button, save_key, Move, Screen,
};
use crate::conflict::{Rows, Settle, Settling};
use crate::context::{
    AppContext, Command, Gone, Saved, Target, Touched,
};
use crate::data::Snapshot;
use crate::form_state::{Action, Event, Phase, Then};
use crate::forms::*;
use crate::reports::Report;
use crate::screens::sheet::SheetOf;
use cairn_core::{totals, CustomerId, DocLine, OrderId, QuoteId};
use cairn_forms::{
    find_box, three_way, widgets, Computed, Datasheet, FormEvents,
    RecordForm,
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

/// What a document form (a quote, an order) adds to `Screen`: one save
/// lifecycle, `Phase`, and the one place that says where the form goes
/// when a save ends (Chapter 13). A form supplies its phase and four
/// facts about itself; the routing is written once, here.
trait Document: Screen + Sized {
    /// Where the form's save has got to.
    fn phase_mut(&mut self) -> &mut Phase;
    /// The form for record `key` (a new one for `None`), from the
    /// snapshot.
    fn reopen(ctx: &AppContext, key: Option<i32>) -> Self;
    /// The target that opens record `key`.
    fn record(key: Option<i32>) -> Target;
    /// Save & New opens this.
    const NEW: Target;
    /// Save & Close opens this.
    const LIST: Target;

    fn phase_event(&mut self, event: Event) -> Action {
        let (phase, action) = self.phase_mut().next(event);
        *self.phase_mut() = phase;
        action
    }

    /// Typing after a failed save clears the failure.
    fn note_typing(&mut self, dirty: bool) {
        if dirty && matches!(self.phase_mut(), Phase::Error(_)) {
            self.phase_event(Event::Edited);
        }
    }

    /// Save, then do what `then` says once Postgres answers. `go` is a
    /// navigation-bar move or find-box pick: it saves first if the form
    /// is dirty, and opens at once if not.
    fn route(
        &mut self,
        ui: &mut Ui,
        go: Option<Option<i32>>,
        then: Option<Then>,
        save: bool,
    ) -> Vec<Command> {
        let mut cmds = Vec::new();
        let then = match go {
            Some(go) if !self.is_dirty() => {
                cmds.push(Command::Open(Self::record(go)));
                None
            }
            Some(go) => Some(Then::Go(go)),
            None => then.or(save.then_some(Then::Stay)),
        };
        if let Some(then) = then {
            if let Some(rec) = self.try_save(ui) {
                self.phase_event(Event::Save { then });
                cmds.push(Command::save(rec));
            }
        }
        if let Phase::Error(why) = self.phase_mut() {
            ui.colored_label(ui.visuals().error_fg_color, &*why);
        }
        cmds
    }

    /// One arm for every way a save can end: what the rep asked for
    /// when they pressed the button is in the phase.
    fn finish(
        &mut self,
        key: i32,
        ctx: &AppContext,
    ) -> Option<Target> {
        let then = match self.phase_event(Event::Saved) {
            Action::Leave(then) => then,
            _ => Then::Stay,
        };
        match then {
            Then::Stay => {
                *self = Self::reopen(ctx, Some(key));
                None
            }
            Then::Go(go) => {
                *self = Self::reopen(ctx, go);
                None
            }
            Then::New => Some(Self::NEW),
            Then::Close => Some(Self::LIST),
        }
    }

    /// The save failed and the list went back.
    fn failed(&mut self) {
        self.phase_event(Event::Failed(
            "Not saved. Your changes are still here.".into(),
        ));
    }
}

/// Save & Close and Save & New, drawn in the buttons' row.
fn save_and(ui: &mut Ui) -> Option<Then> {
    let mut then = None;
    if ui.button("Save & Close").clicked() {
        then = Some(Then::Close);
    }
    if ui.button("Save & New").clicked() {
        then = Some(Then::New);
    }
    then
}

pub struct QuoteScreen {
    pub form: RecordForm<QuoteDraft>,
    pub lines: Datasheet<QuoteLineDraft>,
    /// Where this form is in a save (Chapter 13).
    phase: Phase,
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
            phase: Phase::Editing,
        };
        screen.form.focus_first();
        screen
    }

    fn records(&self, touched: Touched) -> Saved {
        Saved::Quote {
            quote: self.form.draft.clone(),
            lines: self.lines.records().cloned().collect(),
            removed_lines: self
                .lines
                .deleted
                .iter()
                .filter_map(|l| {
                    l.id.map(|id| Gone {
                        id,
                        version: l.version,
                    })
                })
                .collect(),
            touched: Some(touched),
        }
    }
}

impl Document for QuoteScreen {
    fn phase_mut(&mut self) -> &mut Phase {
        &mut self.phase
    }
    fn reopen(ctx: &AppContext, key: Option<i32>) -> Self {
        QuoteScreen::open(ctx, key.map(QuoteId), None)
    }
    fn record(key: Option<i32>) -> Target {
        Target::Quote(key.map(QuoteId))
    }
    const NEW: Target = Target::NewQuote(None);
    const LIST: Target = Target::Sheet(SheetOf::Quotes);
}

impl Screen for QuoteScreen {
    fn ui(&mut self, ui: &mut Ui, ctx: &AppContext) -> Vec<Command> {
        let mut cmds = Vec::new();
        let mut save = save_key(ui);
        let dirty = self.is_dirty();
        self.note_typing(dirty);
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
        let mut then = None;
        let saved_quote = self.form.draft.id.filter(|_| !dirty);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    later(ui, "Create Order from Quote", 13);
                    if print_button(
                        ui,
                        "Print Quote",
                        saved_quote.is_some(),
                    ) {
                        if let Some(q) = saved_quote {
                            cmds.push(Command::Print(Report::Quote(
                                q,
                            )));
                        }
                    }
                });
                ui.horizontal(|ui| {
                    b = Some(buttons(ui, true));
                    then = save_and(ui);
                });
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
        cmds.extend(self.route(ui, go, then, save));
        cmds
    }

    fn is_dirty(&self) -> bool {
        self.form.is_dirty() || self.lines.is_dirty()
    }

    fn try_save(&mut self, ui: &mut Ui) -> Option<Saved> {
        if self.lines.refuse_bad_row() {
            return None;
        }
        // Read before the form's save takes the draft as the original.
        let touched = Touched {
            header: self.form.edited(),
            lines: self.lines.edited().filter_map(|l| l.id).collect(),
            ..Touched::default()
        };
        self.form.try_save(ui)?;
        Some(self.records(touched))
    }

    fn not_saved(&mut self) {
        self.form.refused_save();
        self.failed();
    }

    fn settle(
        &mut self,
        mine: &Saved,
        theirs: &Snapshot,
    ) -> Option<Box<dyn Settle>> {
        let key = self.form.draft.id?;
        let other =
            theirs.quotes.iter().find(|q| q.id == Some(key))?;
        self.form.refused_save();
        self.phase_event(Event::Conflict);
        let merge =
            three_way(self.form.as_read(), &self.form.draft, other);
        let lines = Rows::compare(
            "Line",
            &self.lines,
            theirs.quote_lines.get(&key).cloned().unwrap_or_default(),
            |l| l.description.clone(),
        );
        let header_edited = match mine {
            Saved::Quote { touched, .. } => {
                touched.as_ref().is_none_or(|t| t.header)
            }
            _ => true,
        };
        let l = lines.clone();
        Some(Box::new(
            Settling::new("Quote", merge, move |header| {
                let (lines, removed_lines, wrote) = l.result();
                Saved::Quote {
                    quote: header,
                    lines,
                    removed_lines,
                    touched: Some(Touched {
                        header: header_edited,
                        lines: wrote,
                        ..Touched::default()
                    }),
                }
            })
            .with_section(lines),
        ))
    }

    fn saved(&mut self, key: i32, ctx: &AppContext) {
        self.finish(key, ctx);
    }

    fn after_save(
        &mut self,
        key: i32,
        ctx: &AppContext,
    ) -> Option<Target> {
        self.finish(key, ctx)
    }
}

pub struct OrderScreen {
    pub form: RecordForm<OrderDraft>,
    pub lines: Datasheet<OrderLineDraft>,
    pub payments: Datasheet<PaymentDraft>,
    /// Where this form is in a save (Chapter 13).
    phase: Phase,
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
            phase: Phase::Editing,
        };
        screen.form.focus_first();
        screen
    }
}

impl Document for OrderScreen {
    fn phase_mut(&mut self) -> &mut Phase {
        &mut self.phase
    }
    fn reopen(ctx: &AppContext, key: Option<i32>) -> Self {
        OrderScreen::open(ctx, key.map(OrderId), None)
    }
    fn record(key: Option<i32>) -> Target {
        Target::Order(key.map(OrderId))
    }
    const NEW: Target = Target::NewOrder(None);
    const LIST: Target = Target::OrderList;
}

impl Screen for OrderScreen {
    fn ui(&mut self, ui: &mut Ui, ctx: &AppContext) -> Vec<Command> {
        let mut cmds = Vec::new();
        let mut save = save_key(ui);
        let dirty = self.is_dirty();
        self.note_typing(dirty);
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
        // Sends the order as saved, so an edit is saved first.
        let saved = self.form.draft.id.filter(|_| !dirty);
        let mut send = false;
        let mut receipt = false;
        // The latest payment on the order: its date, then its number.
        let latest = self
            .payments
            .records()
            .filter_map(|p| Some((p.date, p.id?)))
            .max()
            .map(|(_, id)| id);
        ui.add_space(6.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.strong("Payments");
                    send = ui
                        .add_enabled(
                            saved.is_some(),
                            egui::Button::new("Send to Manufacturer"),
                        )
                        .on_disabled_hover_text(
                            "Save the order first",
                        )
                        .clicked();
                    receipt = print_button(
                        ui,
                        "Print Receipt",
                        saved.is_some() && latest.is_some(),
                    );
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
        if let (true, Some(id)) = (send, saved) {
            cmds.push(Command::SendToManufacturer(id));
        }
        if let (true, Some(order), Some(payment)) =
            (receipt, saved, latest)
        {
            cmds.push(Command::Print(Report::Receipt {
                order,
                payment,
            }));
        }
        let mut invoice = false;
        let mut then = None;
        ui.horizontal(|ui| {
            b = Some(buttons(ui, true));
            then = save_and(ui);
            invoice =
                print_button(ui, "Print Invoice", saved.is_some());
        });
        if let (true, Some(id)) = (invoice, saved) {
            cmds.push(Command::Print(Report::Invoice(id)));
        }
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
        cmds.extend(self.route(ui, go, then, save));
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
        let touched = Touched {
            header: self.form.edited(),
            lines: self.lines.edited().filter_map(|l| l.id).collect(),
            payments: self
                .payments
                .edited()
                .filter_map(|p| p.id.map(|k| k.0))
                .collect(),
            ..Touched::default()
        };
        self.form.try_save(ui)?;
        Some(Saved::Order {
            order: self.form.draft.clone(),
            lines: self.lines.records().cloned().collect(),
            payments: self.payments.records().cloned().collect(),
            removed_lines: self
                .lines
                .deleted
                .iter()
                .filter_map(|l| {
                    l.id.map(|id| Gone {
                        id,
                        version: l.version,
                    })
                })
                .collect(),
            removed_payments: self
                .payments
                .deleted
                .iter()
                .filter_map(|p| {
                    p.id.map(|k| Gone {
                        id: k.0,
                        version: p.version,
                    })
                })
                .collect(),
            touched: Some(touched),
        })
    }

    fn not_saved(&mut self) {
        self.form.refused_save();
        self.failed();
    }

    fn settle(
        &mut self,
        mine: &Saved,
        theirs: &Snapshot,
    ) -> Option<Box<dyn Settle>> {
        let key = self.form.draft.id?;
        let other =
            theirs.orders.iter().find(|o| o.id == Some(key))?;
        self.form.refused_save();
        self.phase_event(Event::Conflict);
        let merge =
            three_way(self.form.as_read(), &self.form.draft, other);
        let lines = Rows::compare(
            "Line",
            &self.lines,
            theirs.order_lines.get(&key).cloned().unwrap_or_default(),
            |l| l.description.clone(),
        );
        let payments = Rows::compare(
            "Payment",
            &self.payments,
            theirs.payments.get(&key).cloned().unwrap_or_default(),
            |p| p.reference.clone(),
        );
        let header_edited = match mine {
            Saved::Order { touched, .. } => {
                touched.as_ref().is_none_or(|t| t.header)
            }
            _ => true,
        };
        let (l, p) = (lines.clone(), payments.clone());
        Some(Box::new(
            Settling::new("Order", merge, move |header| {
                let (lines, removed_lines, wrote_lines) = l.result();
                let (payments, removed_payments, wrote_payments) =
                    p.result();
                Saved::Order {
                    order: header,
                    lines,
                    payments,
                    removed_lines,
                    removed_payments,
                    touched: Some(Touched {
                        header: header_edited,
                        lines: wrote_lines,
                        payments: wrote_payments,
                        ..Touched::default()
                    }),
                }
            })
            .with_section(lines)
            .with_section(payments),
        ))
    }

    fn saved(&mut self, key: i32, ctx: &AppContext) {
        self.finish(key, ctx);
    }

    fn after_save(
        &mut self,
        key: i32,
        ctx: &AppContext,
    ) -> Option<Target> {
        self.finish(key, ctx)
    }
}
