//! frmCustomers: the customer, and four tabs of subforms. Contacts and
//! Activities are edited in place; Quotes and Orders are lists that open
//! the document on a double-click.

use super::{buttons, date_text, nav_bar, save_key, Screen};
use crate::context::{AppContext, Command, Saved, Target};
use crate::forms::*;
use crate::screens::sheet::SheetOf;
use cairn_core::{CustomerId, OrderId, QuoteId};
use cairn_forms::{
    find_box, money, widgets, Datasheet, ListColumn, ListView,
    RecordForm,
};
use egui::Ui;
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Contacts,
    Activities,
    Quotes,
    Orders,
}

/// A row of the Quotes tab: `sfrmCustomerQuotes` over `qryQuoteTotals`.
struct QuoteRow {
    id: QuoteId,
    number: String,
    date: String,
    status: String,
    total: Decimal,
}

/// A row of the Orders tab: `sfrmCustomerOrders` over `qryOrderBalances`.
struct OrderRow {
    id: OrderId,
    number: String,
    date: String,
    status: String,
    total: Decimal,
    paid: Decimal,
    due: Decimal,
}

pub struct CustomerScreen {
    pub form: RecordForm<CustomerDraft>,
    pub contacts: Datasheet<ContactDraft>,
    pub activities: Datasheet<ActivityLine>,
    tab: Tab,
    quotes: Vec<QuoteRow>,
    orders: Vec<OrderRow>,
    quote_list: ListView,
    order_list: ListView,
    pending: Option<Option<i32>>,
}

impl CustomerScreen {
    pub fn open(ctx: &AppContext, id: Option<CustomerId>) -> Self {
        let d = &ctx.data;
        let customer = id.and_then(|id| ctx.customer(id));
        let form = match customer {
            Some(c) => {
                RecordForm::new(("customer", c.id), c.clone(), ctx)
            }
            None => RecordForm::blank(
                ("customer", "new"),
                CustomerDraft::blank(ctx),
            ),
        };
        let mine = |c: Option<CustomerId>| id.is_some() && c == id;
        let contacts = d
            .contacts
            .iter()
            .filter(|c| mine(c.customer))
            .cloned()
            .collect();
        let activities = d
            .activities
            .iter()
            .filter(|a| mine(a.customer))
            .map(ActivityDraft::line)
            .collect();
        // V1's `ActivityDefaults`: a new activity takes the customer's
        // rep. It's also dated now, and for this customer.
        let mut blank_activity = ActivityDraft::blank(ctx, id).line();
        blank_activity.rep = customer.and_then(|c| c.rep);
        let mut quotes: Vec<QuoteRow> = d
            .quotes
            .iter()
            .filter(|q| mine(q.customer))
            .filter_map(|q| {
                let qid = q.id?;
                Some(QuoteRow {
                    id: qid,
                    number: q.number.clone(),
                    date: date_text(q.date),
                    status: q.status.to_string(),
                    total: ctx.quote_totals.get(&qid)?.total,
                })
            })
            .collect();
        quotes.sort_by(|a, b| b.number.cmp(&a.number));
        let mut orders: Vec<OrderRow> = d
            .orders
            .iter()
            .filter(|o| mine(o.customer))
            .filter_map(|o| {
                let oid = o.id?;
                let b = ctx.balances.get(&oid)?;
                Some(OrderRow {
                    id: oid,
                    number: o.number.clone(),
                    date: date_text(o.date),
                    status: o.status.to_string(),
                    total: b.totals.total,
                    paid: b.paid,
                    due: b.due(),
                })
            })
            .collect();
        orders.sort_by(|a, b| b.number.cmp(&a.number));
        let mut screen = CustomerScreen {
            form,
            contacts: Datasheet::new(
                ("contacts", id),
                contacts,
                ContactDraft::blank(id),
                Vec::new(),
                ctx,
            ),
            activities: Datasheet::new(
                ("activities", id),
                activities,
                blank_activity,
                Vec::new(),
                ctx,
            ),
            tab: Tab::Contacts,
            quotes,
            orders,
            quote_list: ListView::default(),
            order_list: ListView::default(),
            pending: None,
        };
        screen.form.focus_first();
        screen
    }
}

const QUOTE_COLUMNS: [ListColumn<QuoteRow>; 4] = [
    ListColumn {
        label: "Quote #",
        width: 120.0,
        value: |q| q.number.clone(),
    },
    ListColumn {
        label: "Date",
        width: 90.0,
        value: |q| q.date.clone(),
    },
    ListColumn {
        label: "Status",
        width: 90.0,
        value: |q| q.status.clone(),
    },
    ListColumn {
        label: "Total",
        width: 110.0,
        value: |q| money(q.total),
    },
];

const ORDER_COLUMNS: [ListColumn<OrderRow>; 6] = [
    ListColumn {
        label: "Order #",
        width: 120.0,
        value: |o| o.number.clone(),
    },
    ListColumn {
        label: "Date",
        width: 90.0,
        value: |o| o.date.clone(),
    },
    ListColumn {
        label: "Status",
        width: 100.0,
        value: |o| o.status.clone(),
    },
    ListColumn {
        label: "Total",
        width: 100.0,
        value: |o| money(o.total),
    },
    ListColumn {
        label: "Paid",
        width: 100.0,
        value: |o| money(o.paid),
    },
    ListColumn {
        label: "Balance",
        width: 100.0,
        value: |o| money(o.due),
    },
];

impl Screen for CustomerScreen {
    fn ui(&mut self, ui: &mut Ui, ctx: &AppContext) -> Vec<Command> {
        let mut cmds = Vec::new();
        let mut save = save_key(ui);
        let mut picked = None;
        ui.horizontal(|ui| {
            widgets::title(ui, "Customers", self.is_dirty());
            ui.add_space(40.0);
            if let Some(list) = ctx.lookup_list("customers") {
                picked = find_box(ui, "find", list, 200.0)
                    .and_then(|k| k.parse::<i32>().ok());
            }
        });
        ui.separator();
        self.form.ui(ui, ctx);
        cmds.extend(self.form.take_commands());

        let id = self.form.draft.id;
        ui.horizontal(|ui| {
            // V1's buttons open a new document with this customer filled
            // in. A customer not yet saved has nothing to fill in.
            ui.add_enabled_ui(id.is_some(), |ui| {
                if ui.button("New Quote").clicked() {
                    cmds.push(Command::Open(Target::NewQuote(id)));
                }
                if ui.button("New Order").clicked() {
                    cmds.push(Command::Open(Target::NewOrder(id)));
                }
                if ui.button("Log Activity").clicked() {
                    cmds.push(Command::Open(Target::NewActivity(id)));
                }
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for (tab, caption) in [
                (Tab::Contacts, "Contacts"),
                (Tab::Activities, "Activities"),
                (Tab::Quotes, "Quotes"),
                (Tab::Orders, "Orders"),
            ] {
                ui.selectable_value(&mut self.tab, tab, caption);
            }
        });
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_height(170.0);
            ui.set_width(ui.available_width());
            match self.tab {
                Tab::Contacts => self.contacts.ui(ui, ctx),
                Tab::Activities => self.activities.ui(ui, ctx),
                Tab::Quotes => {
                    let open = self.quote_list.ui(
                        ui,
                        "customer-quotes",
                        &self.quotes,
                        &QUOTE_COLUMNS,
                    );
                    if let Some(i) = open {
                        let q =
                            Target::Quote(Some(self.quotes[i].id));
                        cmds.push(Command::Open(q));
                    }
                }
                Tab::Orders => {
                    let open = self.order_list.ui(
                        ui,
                        "customer-orders",
                        &self.orders,
                        &ORDER_COLUMNS,
                    );
                    if let Some(i) = open {
                        let o =
                            Target::Order(Some(self.orders[i].id));
                        cmds.push(Command::Open(o));
                    }
                }
            }
        });
        cmds.extend(self.contacts.take_commands());
        cmds.extend(self.activities.take_commands());

        let b = buttons(ui, true);
        if b.sheet {
            cmds.push(Command::Open(Target::Sheet(
                SheetOf::Customers,
            )));
        }
        let rows = &ctx.data.customers;
        let key = |c: &CustomerDraft| c.id.map(|i| i.0);
        let at = id.map(|i| i.0);
        let i = rows.iter().position(|c| key(c) == at);
        let mv = nav_bar(ui, i, rows.len(), true);
        save |= b.save;
        if b.undo {
            self.form.undo_record();
        }
        if b.close {
            cmds.push(Command::Open(Target::Menu));
        }
        let go = match (picked, mv) {
            (Some(k), _) => Some(Some(k)),
            (None, Some(m)) => Some(
                m.from(i, rows.len()).and_then(|i| key(&rows[i])),
            ),
            _ => None,
        };
        if let Some(go) = go {
            if self.is_dirty() {
                if let Some(rec) = self.try_save(ui) {
                    self.pending = Some(go);
                    cmds.push(Command::save(rec));
                }
            } else {
                let target = Target::Customer(go.map(CustomerId));
                cmds.push(Command::Open(target));
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
            || self.contacts.is_dirty()
            || self.activities.is_dirty()
    }

    fn try_save(&mut self, ui: &mut Ui) -> Option<Saved> {
        if self.contacts.refuse_bad_row() {
            self.tab = Tab::Contacts;
            return None;
        }
        if self.activities.refuse_bad_row() {
            self.tab = Tab::Activities;
            return None;
        }
        self.form.try_save(ui)?;
        Some(Saved::Customer {
            customer: self.form.draft.clone(),
            contacts: self.contacts.records().cloned().collect(),
            activities: self.activities.records().cloned().collect(),
        })
    }

    fn saved(&mut self, key: i32, ctx: &AppContext) {
        let go = self.pending.take().unwrap_or(Some(key));
        let tab = self.tab;
        *self = CustomerScreen::open(ctx, go.map(CustomerId));
        self.tab = tab;
    }
}
