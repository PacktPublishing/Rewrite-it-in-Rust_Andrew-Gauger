//! What every form can see: the snapshot, the lookups built from it, and
//! the totals V1's saved queries computed.

use crate::data::Snapshot;
use crate::forms::*;
use cairn_core::{
    totals, CompanyInfo, ContactId, CustomerId, CustomerRow,
    CustomerStatus, Date, OrderId, OrderStatus, ProductId, QuoteId,
    QuoteStatus, RepId, Totals,
};
use cairn_forms::{DateTime, FormContext, LookupList, LookupRow};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Where a button or a double-click goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    Menu,
    Customer(Option<CustomerId>),
    Quote(Option<QuoteId>),
    Order(Option<OrderId>),
    OrderList,
    /// A form in datasheet view.
    Sheet(crate::screens::sheet::SheetOf),
    Product(Option<i32>),
    Rep(Option<i32>),
    Company,
    Activity(Option<i32>),
    Message(Option<i32>),
    Rma(Option<i32>),
    /// V1's "New Quote", "New Order" and "Log Activity" on a customer.
    NewQuote(Option<CustomerId>),
    NewOrder(Option<CustomerId>),
    NewActivity(Option<CustomerId>),
    NewMessage,
}

/// A record to save, with its subform rows.
pub enum Saved {
    Company(CompanyDraft),
    Rep(RepDraft),
    Product(ProductDraft),
    Activity(ActivityDraft),
    Message(MessageDraft),
    Rma(RmaDraft),
    Customer {
        customer: CustomerDraft,
        contacts: Vec<ContactDraft>,
        activities: Vec<ActivityLine>,
    },
    Quote {
        quote: QuoteDraft,
        lines: Vec<QuoteLineDraft>,
    },
    Order {
        order: OrderDraft,
        lines: Vec<OrderLineDraft>,
        payments: Vec<PaymentDraft>,
    },
}

/// Work a form asks the app to do. Handlers and buttons return these
/// instead of doing it; Chapter 11 sends the saves to workers.
pub enum Command {
    Open(Target),
    Save(Box<Saved>),
}

impl Command {
    pub fn save(saved: Saved) -> Command {
        Command::Save(Box::new(saved))
    }
}

/// `qryOrderBalances`, per order.
#[derive(Debug, Clone, Copy)]
pub struct Balance {
    pub totals: Totals,
    pub paid: Decimal,
}

impl Balance {
    pub fn due(&self) -> Decimal {
        self.totals.total - self.paid
    }
}

pub struct AppContext {
    pub data: Snapshot,
    pub company: CompanyInfo,
    pub today: Date,
    lookups: HashMap<&'static str, LookupList>,
    /// Lookups narrowed by a customer: its contacts, quotes and orders.
    by_customer: HashMap<&'static str, HashMap<String, LookupList>>,
    customer_index: HashMap<CustomerId, usize>,
    product_index: HashMap<ProductId, usize>,
    order_index: HashMap<OrderId, usize>,
    pub quote_totals: HashMap<QuoteId, Totals>,
    pub balances: HashMap<OrderId, Balance>,
    /// The main menu's two lists, as indexes into the snapshot.
    pub open_messages: Vec<usize>,
    pub follow_ups: Vec<usize>,
}

impl FormContext for AppContext {
    type Command = Command;

    fn lookup(
        &self,
        list: &str,
        filter: Option<&str>,
    ) -> Option<&LookupList> {
        match filter {
            Some(key) => self.by_customer.get(list)?.get(key),
            None => self.lookups.get(list),
        }
    }
}

fn row(key: impl ToString, text: &str, detail: &str) -> LookupRow {
    LookupRow {
        key: key.to_string(),
        text: text.to_string(),
        detail: detail.to_string(),
    }
}

fn index<K: std::hash::Hash + Eq, T>(
    rows: &[T],
    key: impl Fn(&T) -> Option<K>,
) -> HashMap<K, usize> {
    rows.iter()
        .enumerate()
        .filter_map(|(i, r)| key(r).map(|k| (k, i)))
        .collect()
}

/// A V1 value list (`VL_TERMS` and the rest), `;`-separated.
fn values(list: &str) -> LookupList {
    LookupList::values(&list.split(';').collect::<Vec<_>>())
}

/// Group lookup rows by customer.
fn by_customer(
    rows: impl Iterator<Item = (Option<CustomerId>, LookupRow)>,
) -> HashMap<String, LookupList> {
    let mut grouped: HashMap<String, Vec<LookupRow>> = HashMap::new();
    for (customer, row) in rows {
        let key = customer.map_or(0, |c| c.0).to_string();
        grouped.entry(key).or_default().push(row);
    }
    grouped
        .into_iter()
        .map(|(k, rows)| (k, LookupList::new(rows)))
        .collect()
}

fn key<I: Copy>(id: Option<I>, raw: impl Fn(I) -> i32) -> i32 {
    id.map_or(0, raw)
}

impl AppContext {
    pub fn new(data: Snapshot) -> Self {
        let mut ctx = AppContext {
            company: CompanyInfo {
                default_tax_rate: data.company.default_tax_rate,
            },
            today: today(),
            lookups: HashMap::new(),
            by_customer: HashMap::new(),
            customer_index: HashMap::new(),
            product_index: HashMap::new(),
            order_index: HashMap::new(),
            quote_totals: HashMap::new(),
            balances: HashMap::new(),
            open_messages: Vec::new(),
            follow_ups: Vec::new(),
            data,
        };
        ctx.rebuild();
        ctx
    }

    /// Rebuild the lookups and totals after the snapshot changes.
    pub fn rebuild(&mut self) {
        self.company = CompanyInfo {
            default_tax_rate: self.data.company.default_tax_rate,
        };
        let d = &self.data;
        self.customer_index = index(&d.customers, |c| c.id);
        self.product_index = index(&d.products, |p| p.id);
        self.order_index = index(&d.orders, |o| o.id);
        let names: HashMap<CustomerId, &str> = d
            .customers
            .iter()
            .filter_map(|c| Some((c.id?, c.company_name.as_str())))
            .collect();
        let name = |c: Option<CustomerId>| {
            c.and_then(|c| names.get(&c).copied()).unwrap_or("")
        };

        // V1's row sources: RS_CUSTOMERS, RS_REPS, RS_PRODUCTS, the find
        // boxes, and the VL_ value lists.
        let customers = d
            .customers
            .iter()
            .map(|c| {
                let place =
                    format!("{}, {}", c.bill_city, c.bill_state);
                row(key(c.id, |i| i.0), &c.company_name, &place)
            })
            .collect();
        // V1 offered every rep, departed ones too; V2 marks them.
        let reps = d
            .reps
            .iter()
            .map(|r| {
                let detail = if r.active { "" } else { "inactive" };
                row(key(r.id, |i| i.0), &r.name(), detail)
            })
            .collect();
        // V1's line combos offered active products only, so an old line's
        // retired product showed blank. V2 lists it, marked.
        let products = d
            .products
            .iter()
            .map(|p| {
                let detail = if p.active {
                    p.name.clone()
                } else {
                    format!("{} (retired)", p.name)
                };
                row(key(p.id, |i| i.0), &p.sku, &detail)
            })
            .collect();
        let quote_numbers = d
            .quotes
            .iter()
            .map(|q| {
                row(key(q.id, |i| i.0), &q.number, name(q.customer))
            })
            .collect();
        let order_numbers = d
            .orders
            .iter()
            .map(|o| {
                row(key(o.id, |i| i.0), &o.number, name(o.customer))
            })
            .collect();
        let rma_numbers = d
            .rmas
            .iter()
            .map(|r| {
                row(r.id.unwrap_or(0), &r.number, name(r.customer))
            })
            .collect();
        self.lookups = HashMap::from([
            ("customers", LookupList::new(customers)),
            ("reps", LookupList::new(reps)),
            ("products", LookupList::new(products)),
            ("quote_numbers", LookupList::new(quote_numbers)),
            ("order_numbers", LookupList::new(order_numbers)),
            ("rma_numbers", LookupList::new(rma_numbers)),
            ("quote_status", LookupList::values(QuoteStatus::ALL)),
            ("order_status", LookupList::values(OrderStatus::ALL)),
            ("customer_status", LookupList::values(CustomerStatus::ALL)),
            (
                "rma_reasons",
                values(
                    "Dead on Arrival;Failed in Warranty;Damaged in Shipping;\
                     Wrong Item Shipped;Intermittent Fault;Customer Damage",
                ),
            ),
            (
                "rma_status",
                values(
                    "Open;Received;In Repair;Replaced;Credited;Closed;\
                     Rejected",
                ),
            ),
            (
                "terms",
                values("Due on Receipt;Net 15;Net 30;Net 45;Net 60"),
            ),
            ("activity_types", values("Note;Call;Email;Meeting;Task")),
            ("payment_methods", values("Check;ACH;Wire;Credit Card;Cash")),
            ("message_status", values("New;Returned;Closed")),
            (
                "categories",
                values("Sensor;Gateway;Module;Accessory;Service;Kit"),
            ),
        ]);

        // Lookups narrowed by customer.
        self.by_customer = HashMap::from([
            (
                "contacts",
                by_customer(d.contacts.iter().map(|c| {
                    let detail =
                        if c.is_primary { "primary" } else { "" };
                    (
                        c.customer,
                        row(key(c.id, |i| i.0), &c.name(), detail),
                    )
                })),
            ),
            (
                "customer_quotes",
                by_customer(d.quotes.iter().map(|q| {
                    (
                        q.customer,
                        row(key(q.id, |i| i.0), &q.number, ""),
                    )
                })),
            ),
            (
                "customer_orders",
                by_customer(d.orders.iter().map(|o| {
                    let date = crate::screens::date_text(o.date);
                    (
                        o.customer,
                        row(key(o.id, |i| i.0), &o.number, &date),
                    )
                })),
            ),
        ]);

        // Totals: `qryQuoteTotals` and `qryOrderBalances`.
        self.quote_totals = d
            .quotes
            .iter()
            .filter_map(|q| {
                let id = q.id?;
                let lines: Vec<_> = d
                    .quote_lines
                    .get(&id)
                    .into_iter()
                    .flatten()
                    .map(QuoteLineDraft::doc_line)
                    .collect();
                Some((id, totals(&lines, q.tax_rate, q.shipping)))
            })
            .collect();
        self.balances = d
            .orders
            .iter()
            .filter_map(|o| {
                let id = o.id?;
                let lines: Vec<_> = d
                    .order_lines
                    .get(&id)
                    .into_iter()
                    .flatten()
                    .map(OrderLineDraft::doc_line)
                    .collect();
                let paid = d
                    .payments
                    .get(&id)
                    .into_iter()
                    .flatten()
                    .map(|p| p.amount)
                    .sum();
                let totals = totals(&lines, o.tax_rate, o.shipping);
                Some((id, Balance { totals, paid }))
            })
            .collect();

        // The main menu's lists: `qryOpenPhoneMessages` and
        // `qryFollowUpsDue`.
        let mut open: Vec<usize> = (0..d.messages.len())
            .filter(|&i| d.messages[i].status == "New")
            .collect();
        open.sort_by_key(|&i| {
            let m = &d.messages[i];
            (!m.urgent, std::cmp::Reverse(m.received))
        });
        let (from, to) = (self.today, self.today.add_days(7));
        let mut due: Vec<usize> = (0..d.activities.len())
            .filter(|&i| {
                let a = &d.activities[i];
                !a.completed
                    && a.follow_up
                        .is_some_and(|f| f >= from && f <= to)
            })
            .collect();
        due.sort_by_key(|&i| d.activities[i].follow_up);
        self.open_messages = open;
        self.follow_ups = due;
    }

    /// A lookup by name, for a find box.
    pub fn lookup_list(&self, list: &str) -> Option<&LookupList> {
        self.lookups.get(list)
    }

    pub fn customer(&self, id: CustomerId) -> Option<&CustomerDraft> {
        self.customer_index
            .get(&id)
            .map(|&i| &self.data.customers[i])
    }

    pub fn product(&self, id: ProductId) -> Option<&ProductDraft> {
        self.product_index.get(&id).map(|&i| &self.data.products[i])
    }

    pub fn order(&self, id: OrderId) -> Option<&OrderDraft> {
        self.order_index.get(&id).map(|&i| &self.data.orders[i])
    }

    pub fn customer_name(&self, id: Option<CustomerId>) -> String {
        id.and_then(|c| self.customer(c))
            .map(|c| c.company_name.clone())
            .unwrap_or_default()
    }

    pub fn rep_name(&self, id: Option<RepId>) -> String {
        self.data
            .reps
            .iter()
            .find(|r| r.id.is_some() && r.id == id)
            .map(RepDraft::name)
            .unwrap_or_default()
    }

    pub fn primary_contact(
        &self,
        customer: CustomerId,
    ) -> Option<ContactId> {
        self.data
            .contacts
            .iter()
            .find(|c| c.customer == Some(customer) && c.is_primary)
            .and_then(|c| c.id)
    }

    /// What Chapter 8's `on_customer_picked` reads about a customer. A
    /// customer with no rep keeps the document's rep.
    pub fn customer_row(
        &self,
        customer: CustomerId,
        doc_rep: Option<RepId>,
    ) -> Option<CustomerRow> {
        let c = self.customer(customer)?;
        Some(CustomerRow {
            tax_exempt: c.tax_exempt,
            assigned_rep: c.rep.or(doc_rep)?,
            primary_contact: self.primary_contact(customer),
        })
    }

    /// V1's `Now()`, to the minute (UTC until Chapter 11 brings the
    /// server's clock).
    pub fn now(&self) -> DateTime {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        DateTime {
            date: today(),
            minute: ((secs % 86_400) / 60) as u16,
        }
    }

    /// Save into the snapshot, and return the record's key. New quotes,
    /// orders and RMAs are numbered as Postgres's `next_document_number`
    /// does (Chapter 9). Chapter 11 sends the save to Postgres instead.
    pub fn apply(&mut self, saved: Saved) -> i32 {
        let d = &mut self.data;
        let k = match saved {
            Saved::Company(c) => {
                d.company = c;
                1
            }
            Saved::Rep(mut r) => {
                let id = r.id.map(|i| i.0);
                upsert(
                    &mut d.reps,
                    |x| x.id.map(|i| i.0),
                    id,
                    |n| {
                        r.id = Some(RepId(n));
                        r
                    },
                )
            }
            Saved::Product(mut p) => {
                let id = p.id.map(|i| i.0);
                upsert(
                    &mut d.products,
                    |x| x.id.map(|i| i.0),
                    id,
                    |n| {
                        p.id = Some(ProductId(n));
                        p
                    },
                )
            }
            Saved::Activity(mut a) => upsert(
                &mut d.activities,
                |x| x.id,
                a.id,
                |n| {
                    a.id = Some(n);
                    a
                },
            ),
            Saved::Message(mut m) => upsert(
                &mut d.messages,
                |x| x.id,
                m.id,
                |n| {
                    m.id = Some(n);
                    m
                },
            ),
            Saved::Rma(mut r) => {
                if r.id.is_none() {
                    let numbers =
                        d.rmas.iter().map(|x| x.number.as_str());
                    r.number = next_number(numbers, "RMA", r.opened);
                }
                upsert(
                    &mut d.rmas,
                    |x| x.id,
                    r.id,
                    |n| {
                        r.id = Some(n);
                        r
                    },
                )
            }
            Saved::Customer {
                mut customer,
                contacts,
                activities,
            } => {
                let id = customer.id.map(|i| i.0);
                let k = upsert(
                    &mut d.customers,
                    |x| x.id.map(|i| i.0),
                    id,
                    |n| {
                        customer.id = Some(CustomerId(n));
                        customer
                    },
                );
                let cid = Some(CustomerId(k));
                d.contacts.retain(|c| c.customer != cid);
                let mut next = max_key(
                    d.contacts.iter().map(|c| c.id.map(|i| i.0)),
                );
                for mut c in contacts {
                    c.customer = cid;
                    if c.id.is_none() {
                        next += 1;
                        c.id = Some(ContactId(next));
                    }
                    d.contacts.push(c);
                }
                d.activities.retain(|a| a.customer != cid);
                let mut next =
                    max_key(d.activities.iter().map(|a| a.id));
                for line in activities {
                    let mut a = line.draft();
                    a.customer = cid;
                    if a.id.is_none() {
                        next += 1;
                        a.id = Some(next);
                    }
                    d.activities.push(a);
                }
                d.activities
                    .sort_by_key(|a| std::cmp::Reverse(a.date));
                k
            }
            Saved::Quote { mut quote, lines } => {
                if quote.id.is_none() {
                    let numbers =
                        d.quotes.iter().map(|q| q.number.as_str());
                    quote.number =
                        next_number(numbers, "Q", quote.date);
                }
                let id = quote.id.map(|i| i.0);
                let k = upsert(
                    &mut d.quotes,
                    |x| x.id.map(|i| i.0),
                    id,
                    |n| {
                        quote.id = Some(QuoteId(n));
                        quote
                    },
                );
                d.quote_lines.insert(QuoteId(k), lines);
                k
            }
            Saved::Order {
                mut order,
                lines,
                payments,
            } => {
                if order.id.is_none() {
                    let numbers =
                        d.orders.iter().map(|o| o.number.as_str());
                    order.number =
                        next_number(numbers, "SO", order.date);
                }
                let id = order.id.map(|i| i.0);
                let k = upsert(
                    &mut d.orders,
                    |x| x.id.map(|i| i.0),
                    id,
                    |n| {
                        order.id = Some(OrderId(n));
                        order
                    },
                );
                d.order_lines.insert(OrderId(k), lines);
                d.payments.insert(OrderId(k), payments);
                k
            }
        };
        self.rebuild();
        k
    }
}

fn max_key(keys: impl Iterator<Item = Option<i32>>) -> i32 {
    keys.flatten().max().unwrap_or(0)
}

/// Replace the row with key `id`, or add one, first, with the next key.
fn upsert<T>(
    rows: &mut Vec<T>,
    key: impl Fn(&T) -> Option<i32>,
    id: Option<i32>,
    with_key: impl FnOnce(i32) -> T,
) -> i32 {
    let at = id
        .and_then(|id| rows.iter().position(|r| key(r) == Some(id)));
    match (at, id) {
        (Some(i), Some(id)) => {
            rows[i] = with_key(id);
            id
        }
        _ => {
            let next = max_key(rows.iter().map(&key)) + 1;
            rows.insert(0, with_key(next));
            next
        }
    }
}

/// `Q-2026-1556`: the year's next number.
fn next_number<'a>(
    numbers: impl Iterator<Item = &'a str>,
    prefix: &str,
    date: Date,
) -> String {
    let stem = format!("{prefix}-{}-", date.year());
    let last = numbers
        .filter_map(|n| n.strip_prefix(&stem)?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("{stem}{:04}", last + 1)
}

/// Today, from the system clock (UTC).
fn today() -> Date {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400);
    Date::new(1970, 1, 1)
        .expect("a real date")
        .add_days(days as i64)
}
