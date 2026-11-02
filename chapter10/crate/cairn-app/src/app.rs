//! The eframe shell: a header, V1's main menu, and the screen it opened.

use crate::context::{AppContext, Command, Saved, Target};
use crate::forms::*;
use crate::screens::customer::CustomerScreen;
use crate::screens::document::{OrderScreen, QuoteScreen};
use crate::screens::record::{RecordScreen, Table as _};
use crate::screens::sheet::Sheet;
use crate::screens::{date_text, later, thousands, Screen};
use cairn_core::OrderId;
use cairn_forms::{money, ListColumn, ListView};
use egui::{RichText, Ui};
use rust_decimal::Decimal;

pub enum View {
    Menu,
    /// Sales Orders in datasheet view: every order, a row each, built
    /// once when the view opens. Building 13,531 rows every frame would
    /// undo what drawing only the visible ones saves.
    OrderList(Vec<OrderRow>),
    /// Any other form in datasheet view.
    Sheet(Box<Sheet>),
    Form(Box<dyn Screen>),
}

pub struct CairnApp {
    pub ctx: AppContext,
    pub view: View,
    /// Where to go once the unsaved-changes question is answered.
    leaving_to: Option<Target>,
    messages: ListView,
    follow_ups: ListView,
    orders: ListView,
    status: String,
    styled: bool,
}

impl CairnApp {
    pub fn new(ctx: AppContext) -> Self {
        CairnApp {
            ctx,
            view: View::Menu,
            leaving_to: None,
            messages: ListView::default(),
            follow_ups: ListView::default(),
            orders: ListView::default(),
            status: String::new(),
            styled: false,
        }
    }

    /// Go to `target`, asking first if the open form has unsaved
    /// changes.
    pub fn open(&mut self, target: Target) {
        match &self.view {
            View::Form(f) if f.is_dirty() => {
                self.leaving_to = Some(target)
            }
            _ => self.view = self.view_for(target),
        }
    }

    fn view_for(&self, target: Target) -> View {
        let ctx = &self.ctx;
        let first_customer =
            ctx.data.customers.first().and_then(|c| c.id);
        let screen: Box<dyn Screen> = match target {
            Target::Menu => return View::Menu,
            Target::OrderList => {
                return View::OrderList(self.order_rows())
            }
            Target::Sheet(of) => {
                return View::Sheet(Box::new(Sheet::build(of, ctx)))
            }
            Target::Customer(id) => Box::new(CustomerScreen::open(
                ctx,
                id.or(first_customer),
            )),
            Target::Quote(id) => {
                Box::new(QuoteScreen::open(ctx, id, None))
            }
            Target::Order(id) => {
                Box::new(OrderScreen::open(ctx, id, None))
            }
            Target::NewQuote(c) => {
                Box::new(QuoteScreen::open(ctx, None, c))
            }
            Target::NewOrder(c) => {
                Box::new(OrderScreen::open(ctx, None, c))
            }
            Target::Product(k) => {
                Box::new(RecordScreen::<ProductDraft>::open(ctx, k))
            }
            Target::Rep(k) => {
                Box::new(RecordScreen::<RepDraft>::open(ctx, k))
            }
            Target::Company => Box::new(
                RecordScreen::<CompanyDraft>::open(ctx, None),
            ),
            Target::Rma(k) => {
                Box::new(RecordScreen::<RmaDraft>::open(ctx, k))
            }
            Target::Activity(id) => {
                Box::new(RecordScreen::<ActivityDraft>::open(ctx, id))
            }
            Target::Message(id) => {
                Box::new(RecordScreen::<MessageDraft>::open(ctx, id))
            }
            Target::NewActivity(c) => {
                Box::new(RecordScreen::new_record(
                    ActivityDraft::blank(ctx, c),
                ))
            }
            Target::NewMessage => Box::new(RecordScreen::new_record(
                MessageDraft::blank(ctx),
            )),
        };
        View::Form(screen)
    }

    pub fn run(&mut self, cmd: Command) {
        match cmd {
            Command::Open(target) => self.open(target),
            Command::Save(saved) => self.save(*saved),
        }
    }

    /// Chapter 10 saves into the snapshot only; Chapter 11 sends the
    /// save to Postgres.
    fn save(&mut self, saved: Saved) {
        let key = self.ctx.apply(saved);
        if let View::Form(f) = &mut self.view {
            f.saved(key, &self.ctx);
        }
        self.status = "Saved (in this window only)".into();
    }

    fn header(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.button("Main Menu").clicked() {
                self.open(Target::Menu);
            }
            ui.strong(&self.ctx.data.company.company_name);
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.label(format!(
                        "Connected as {}",
                        self.ctx.data.role
                    ));
                    ui.label(
                        RichText::new("Placeholder data")
                            .color(ui.visuals().warn_fg_color),
                    )
                    .on_hover_text(
                        "Generated rows for laying out screens; \
                         V1's data arrives in Chapter 15",
                    );
                    ui.label(RichText::new(&self.status).weak());
                },
            );
        });
    }

    /// frmMainMenu: V1's buttons, and its two lists.
    fn menu(&mut self, ui: &mut Ui) -> Vec<Command> {
        let mut cmds = Vec::new();
        let ctx = &self.ctx;
        let d = &ctx.data;
        ui.heading("Cairn Systems - Orders & CRM");
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(170.0);
                ui.strong("Records");
                let first_quote = d.quotes.first().and_then(|q| q.id);
                let first_order = d.orders.first().and_then(|o| o.id);
                for (caption, target) in [
                    ("Customers", Target::Customer(None)),
                    ("Quotes", Target::Quote(first_quote)),
                    ("Sales Orders", Target::Order(first_order)),
                    ("Products", Target::Product(None)),
                    ("Sales Reps", Target::Rep(None)),
                    ("Company Info", Target::Company),
                    ("Returns (RMAs)", Target::Rma(None)),
                    ("Take Phone Message", Target::NewMessage),
                    ("Log Activity", Target::NewActivity(None)),
                ] {
                    let b = egui::Button::new(caption)
                        .min_size([160.0, 24.0].into());
                    if ui.add(b).clicked() {
                        cmds.push(Command::Open(target));
                    }
                }
            });
            ui.vertical(|ui| {
                ui.set_width(170.0);
                ui.strong("Month-end");
                for caption in [
                    "Orders",
                    "Sales by Rep",
                    "Sales by Product",
                    "Cash Received",
                    "Quote Pipeline",
                    "Open Receivables",
                    "Commission Statements",
                ] {
                    later(ui, caption, 14);
                }
            });
            ui.vertical(|ui| {
                let rows: Vec<MessageRow> = ctx
                    .open_messages
                    .iter()
                    .map(|&i| {
                        let m = &d.messages[i];
                        MessageRow {
                            id: m.id,
                            urgent: m.urgent,
                            received: m.received.to_string(),
                            for_rep: ctx.rep_name(m.for_rep),
                            caller: m.caller_name.clone(),
                            company: m.caller_company.clone(),
                        }
                    })
                    .collect();
                ui.strong(format!(
                    "Open phone messages ({}): double-click to open",
                    rows.len()
                ));
                ui.push_id("messages", |ui| {
                    ui.set_height(200.0);
                    let open = self.messages.ui(
                        ui,
                        "messages",
                        &rows,
                        &MESSAGE_COLUMNS,
                    );
                    if let Some(i) = open {
                        let target = Target::Message(rows[i].id);
                        cmds.push(Command::Open(target));
                    }
                });
                ui.add_space(12.0);
                let rows: Vec<FollowUpRow> = ctx
                    .follow_ups
                    .iter()
                    .map(|&i| {
                        let a = &d.activities[i];
                        FollowUpRow {
                            id: a.id,
                            due: a
                                .follow_up
                                .map(date_text)
                                .unwrap_or_default(),
                            rep: ctx.rep_name(a.rep),
                            customer: ctx.customer_name(a.customer),
                            subject: a.subject.clone(),
                        }
                    })
                    .collect();
                ui.strong(format!(
                    "Follow-ups due in the next 7 days ({}): \
                     double-click to open",
                    rows.len()
                ));
                ui.push_id("follow-ups", |ui| {
                    ui.set_height(200.0);
                    let open = self.follow_ups.ui(
                        ui,
                        "follow-ups",
                        &rows,
                        &FOLLOW_UP_COLUMNS,
                    );
                    if let Some(i) = open {
                        let target = Target::Activity(rows[i].id);
                        cmds.push(Command::Open(target));
                    }
                });
            });
        });
        cmds
    }

    fn order_rows(&self) -> Vec<OrderRow> {
        let ctx = &self.ctx;
        ctx.data
            .orders
            .iter()
            .map(|o| {
                let b = o.id.and_then(|id| ctx.balances.get(&id));
                OrderRow {
                    id: o.id.unwrap_or(OrderId(0)),
                    number: o.number.clone(),
                    date: date_text(o.date),
                    customer: ctx.customer_name(o.customer),
                    status: o.status.to_string(),
                    po: o.customer_po.clone(),
                    total: b
                        .map(|b| b.totals.total)
                        .unwrap_or_default(),
                    due: b.map(|b| b.due()).unwrap_or_default(),
                }
            })
            .collect()
    }

    /// Sales Orders in datasheet view.
    fn order_list(
        ui: &mut Ui,
        rows: &[OrderRow],
        list: &mut ListView,
    ) -> Vec<Command> {
        ui.horizontal(|ui| {
            // Access's navigation bar said "Filtered" too.
            let (shown, total) = list.counts();
            let count = match list.is_filtered() {
                true => format!(
                    "{} of {} (Filtered)",
                    thousands(shown),
                    thousands(total)
                ),
                false => thousands(rows.len()),
            };
            ui.heading(format!("Sales Orders: {count}"));
            ui.label("A heading's menu sorts and filters. Double-click or Enter opens.");
        });
        let open = list.ui(ui, "orders", rows, &ORDER_COLUMNS);
        open.map(|i| Command::Open(Target::Order(Some(rows[i].id))))
            .into_iter()
            .collect()
    }

    /// Leaving a form with unsaved changes. Access saved without asking;
    /// V2 asks, because Chapter 10 saves only into this window.
    fn unsaved_question(&mut self, ui: &mut Ui) {
        let Some(target) = self.leaving_to else {
            return;
        };
        let modal = egui::Modal::new(egui::Id::unique("unsaved"))
            .show(ui.ctx(), |ui| {
                ui.label("This record has unsaved changes.");
                ui.horizontal(|ui| {
                    (
                        ui.button("Save").clicked(),
                        ui.button("Discard changes").clicked(),
                        ui.button("Keep editing").clicked(),
                    )
                })
                .inner
            });
        let (save, discard, stay) = modal.inner;
        if save {
            let saved = match &mut self.view {
                View::Form(f) => f.try_save(ui),
                _ => None,
            };
            self.leaving_to = None;
            if let Some(saved) = saved {
                self.ctx.apply(saved);
                self.view = self.view_for(target);
            }
        } else if discard {
            self.leaving_to = None;
            self.view = self.view_for(target);
        } else if stay || modal.should_close() {
            self.leaving_to = None;
        }
    }

    /// One frame of the whole app.
    pub fn show(&mut self, ui: &mut Ui) {
        if !self.styled {
            cairn_forms::widgets::form_style(ui.ctx());
            self.styled = true;
        }
        egui::Panel::top("header").show(ui, |ui| self.header(ui));
        let mut cmds = Vec::new();
        egui::CentralPanel::default().show(ui, |ui| {
            cmds = match &mut self.view {
                View::Menu => self.menu(ui),
                View::Sheet(sheet) => sheet.ui(ui),
                View::OrderList(rows) => {
                    Self::order_list(ui, rows, &mut self.orders)
                }
                View::Form(f) => f.ui(ui, &self.ctx),
            };
            self.unsaved_question(ui);
        });
        for cmd in cmds {
            self.run(cmd);
        }
    }
}

/// A row of the Sales Orders datasheet.
pub struct OrderRow {
    id: OrderId,
    number: String,
    date: String,
    customer: String,
    status: String,
    po: String,
    total: Decimal,
    due: Decimal,
}

pub struct MessageRow {
    id: Option<i32>,
    urgent: bool,
    received: String,
    for_rep: String,
    caller: String,
    company: String,
}

pub struct FollowUpRow {
    id: Option<i32>,
    due: String,
    rep: String,
    customer: String,
    subject: String,
}

const ORDER_COLUMNS: [ListColumn<OrderRow>; 7] = [
    ListColumn {
        label: "Order #",
        width: 110.0,
        value: |o| o.number.clone(),
    },
    ListColumn {
        label: "Date",
        width: 90.0,
        value: |o| o.date.clone(),
    },
    ListColumn {
        label: "Customer",
        width: 220.0,
        value: |o| o.customer.clone(),
    },
    ListColumn {
        label: "Status",
        width: 100.0,
        value: |o| o.status.clone(),
    },
    ListColumn {
        label: "Customer PO",
        width: 100.0,
        value: |o| o.po.clone(),
    },
    ListColumn {
        label: "Total",
        width: 100.0,
        value: |o| money(o.total),
    },
    ListColumn {
        label: "Balance",
        width: 100.0,
        value: |o| money(o.due),
    },
];

/// `sfrmOpenMessages`, urgent ones first.
const MESSAGE_COLUMNS: [ListColumn<MessageRow>; 5] = [
    ListColumn {
        label: "Urgent",
        width: 50.0,
        value: |m| {
            if m.urgent {
                "Yes".into()
            } else {
                String::new()
            }
        },
    },
    ListColumn {
        label: "Received",
        width: 140.0,
        value: |m| m.received.clone(),
    },
    ListColumn {
        label: "For",
        width: 110.0,
        value: |m| m.for_rep.clone(),
    },
    ListColumn {
        label: "Caller",
        width: 120.0,
        value: |m| m.caller.clone(),
    },
    ListColumn {
        label: "Company",
        width: 160.0,
        value: |m| m.company.clone(),
    },
];

/// `sfrmFollowUps`.
const FOLLOW_UP_COLUMNS: [ListColumn<FollowUpRow>; 4] = [
    ListColumn {
        label: "Due",
        width: 90.0,
        value: |f| f.due.clone(),
    },
    ListColumn {
        label: "Rep",
        width: 110.0,
        value: |f| f.rep.clone(),
    },
    ListColumn {
        label: "Customer",
        width: 160.0,
        value: |f| f.customer.clone(),
    },
    ListColumn {
        label: "Subject",
        width: 200.0,
        value: |f| f.subject.clone(),
    },
];

impl eframe::App for CairnApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}
