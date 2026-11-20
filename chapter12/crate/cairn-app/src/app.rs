//! The eframe shell: a header, V1's main menu, and the screen it opened.

use crate::builder::{Builder, Built, Change, Done};
use crate::conflict::{Settle, Settled};
use crate::context::{AppContext, Command, Saved, Target};
use crate::data::{Rows, Table};
use crate::forms::*;
use crate::manufacturer::Sent;
use crate::manufacturer_orchestrator::ManufacturerOrchestrator;
use crate::orchestrator::{Answer, Orchestrator, ScreenId};
use crate::screens::customer::CustomerScreen;
use crate::screens::document::{OrderScreen, QuoteScreen};
use crate::screens::record::{RecordScreen, Table as _};
use crate::screens::sheet::Sheet;
use crate::screens::{date_text, later, thousands, Screen};
use crate::worker::{Response, ToManufacturer};
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
    /// What the view shows, and its number for the orchestrator.
    target: Target,
    screen: ScreenId,
    /// The workers, through the orchestrator. Without one (the tests
    /// with no database) a save goes into the snapshot only.
    pub orchestrator: Option<Orchestrator>,
    /// Where the workers' answers become a new context, off this
    /// thread. With the orchestrator only.
    builder: Option<Builder>,
    /// The manufacturer's own orchestrator: its channel, its worker,
    /// and the retry prompt. The app routes to it. Without one a send
    /// says it needs the database.
    pub manufacturer: Option<ManufacturerOrchestrator>,
    /// Where to go once the unsaved-changes question is answered.
    leaving_to: Option<Target>,
    /// A refused save being settled: another rep saved first.
    conflict: Option<Box<dyn Settle>>,
    /// For waking the UI from the builder thread: the first frame's.
    egui: Option<egui::Context>,
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
            target: Target::Menu,
            screen: ScreenId::default(),
            orchestrator: None,
            builder: None,
            manufacturer: None,
            leaving_to: None,
            conflict: None,
            egui: None,
            messages: ListView::default(),
            follow_ups: ListView::default(),
            orders: ListView::default(),
            status: String::new(),
            styled: false,
        }
    }

    /// The app over the workers. The window opens before any rows
    /// arrive; `read(Rows::All)` is the first request.
    pub fn with(mut self, orchestrator: Orchestrator) -> Self {
        self.orchestrator = Some(orchestrator);
        self.builder = Some(Builder::spawn(self.ctx.data.clone()));
        self
    }

    /// The manufacturer's side: sends go to it, not to the records
    /// workers.
    pub fn with_manufacturer(
        mut self,
        m: ManufacturerOrchestrator,
    ) -> Self {
        self.manufacturer = Some(m);
        self
    }

    /// The Retry/Cancel question, if the manufacturer didn't answer.
    pub fn prompt(&self) -> Option<&ToManufacturer> {
        self.manufacturer.as_ref().and_then(|m| m.prompt())
    }

    /// Sign in, then read everything: what `main` asks first.
    pub fn sign_in(&mut self) {
        if let Some(o) = &mut self.orchestrator {
            o.sign_in();
        }
    }

    /// Ask the workers for `rows`. The screen that's open hears back.
    pub fn read(&mut self, rows: Rows) {
        if let Some(o) = &mut self.orchestrator {
            o.read(rows, self.screen);
        }
    }

    /// Go to `target`, asking first if the open form has unsaved
    /// changes.
    pub fn open(&mut self, target: Target) {
        match &self.view {
            View::Form(f) if f.is_dirty() => {
                self.leaving_to = Some(target)
            }
            _ => self.show_view(target),
        }
    }

    /// Open `target` from the snapshot at once, and ask for its record
    /// as Postgres has it now. The form redraws when that arrives.
    fn show_view(&mut self, target: Target) {
        self.view = self.view_for(target);
        self.target = target;
        self.screen.0 += 1;
        if let Some(m) = &mut self.manufacturer {
            m.clear_prompt();
        }
        self.ask_if_unconfirmed();
        if let Some(rows) = Rows::of_target(target) {
            self.read(rows);
        }
    }

    /// Opening an order the manufacturer never confirmed asks again.
    fn ask_if_unconfirmed(&mut self) {
        let Target::Order(Some(order_id)) = self.target else {
            return;
        };
        let Some(m) = &mut self.manufacturer else {
            return;
        };
        if m.asked(self.screen) || m.sending(order_id) {
            return;
        }
        if let Some(order) = self.ctx.data.unconfirmed.get(&order_id)
        {
            m.ask(
                Box::new(ToManufacturer {
                    order_id,
                    order: order.clone(),
                }),
                self.screen,
            );
        }
    }

    fn is_dirty(&self) -> bool {
        matches!(&self.view, View::Form(f) if f.is_dirty())
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
                let mut sheet = Sheet::build(of, ctx);
                if let Some(o) = &self.orchestrator {
                    sheet.mark_pending(&o.pending_keys(of.table()));
                }
                return View::Sheet(Box::new(sheet));
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
            Command::SendToManufacturer(id) => self.send(id),
        }
    }

    /// Send the saved order. One already in doubt goes with the same
    /// correlation ID.
    fn send(&mut self, order_id: OrderId) {
        let Some(m) = &mut self.manufacturer else {
            self.status = "Sending needs the database".into();
            return;
        };
        if m.sending(order_id) {
            return;
        }
        match self.ctx.manufacturer_order(order_id) {
            Ok(order) => {
                m.send(
                    ToManufacturer { order_id, order },
                    self.screen,
                );
            }
            Err(why) => self.status = format!("Not sent: {why}"),
        }
    }

    /// What the attempt came to. Only the rep retries.
    fn sent(&mut self, send: Box<ToManufacturer>, sent: Sent) {
        let number = self
            .ctx
            .order(send.order_id)
            .map(|o| o.number.clone())
            .unwrap_or_default();
        match sent {
            Sent::Confirmed {
                mfr_order_id,
                repeat,
            } => {
                let had =
                    if repeat { ", already on file" } else { "" };
                self.status =
                    format!("{number} sent: {mfr_order_id}{had}");
            }
            Sent::Unconfirmed { .. } => {
                self.status = format!("{number}: no answer");
                if let Some(m) = &mut self.manufacturer {
                    m.ask(send.clone(), self.screen);
                }
            }
            Sent::Failed { reason } => {
                self.status = format!("{number} not sent: {reason}");
            }
        }
        // The order's attempts as Postgres has them now.
        self.read(Rows::Record(Table::Orders, send.order_id.0));
    }

    /// Send the save to the workers; the form waits for the answer.
    /// With no workers, save into the snapshot now.
    fn save(&mut self, saved: Saved) {
        match &mut self.orchestrator {
            Some(o) => {
                // An existing record shows at once, marked pending, and
                // goes back if Postgres doesn't take it. A new record
                // can't: its key and number come from Postgres.
                match (
                    saved.target(),
                    self.builder.as_mut(),
                    self.egui.as_ref(),
                ) {
                    (Some((table, key)), Some(b), Some(egui)) => {
                        let last_good = Box::new(
                            self.ctx.data.record(table, key),
                        );
                        o.save_optimistic(
                            saved.clone(),
                            self.screen,
                            last_good,
                        );
                        b.change(
                            self.screen,
                            Change::Pending(Box::new(saved)),
                            egui,
                        );
                    }
                    _ => {
                        o.save(saved, self.screen);
                    }
                }
            }
            None => {
                let key = self.ctx.apply(saved);
                if let View::Form(f) = &mut self.view {
                    f.saved(key, &self.ctx);
                }
                self.status = "Saved".into();
            }
        }
    }

    /// Take the workers' answers: rows and saves go to the builder,
    /// and whatever context it has finished comes in.
    fn answers(&mut self, egui: &egui::Context) {
        let Some(o) = &mut self.orchestrator else {
            return;
        };
        for answer in o.poll() {
            self.answer(answer, egui);
        }
        // The manufacturer's side answers on its own channel.
        let (heard, expired) = match &mut self.manufacturer {
            Some(m) => (m.poll(), m.take_expired()),
            None => (Vec::new(), false),
        };
        for (send, sent) in heard {
            self.sent(send, sent);
        }
        if expired {
            // The login ran out under an attempt: the records side
            // signs in, and Try again sends it.
            if let Some(o) = &mut self.orchestrator {
                o.sign_in();
            }
        }
        while let Some(built) =
            self.builder.as_mut().and_then(Builder::try_take)
        {
            self.built(built);
        }
    }

    fn answer(
        &mut self,
        Answer {
            screen,
            response,
            rolled_back,
        }: Answer,
        egui: &egui::Context,
    ) {
        let change = match response {
            Response::Read { rows, data, .. } => {
                Change::Read(rows, data)
            }
            Response::Saved { stored, saved, .. } => {
                Change::Saved(saved, stored.number)
            }
            // Someone else saved the record first. Their version goes
            // on the list; the three-way dialog is the next step, so
            // for now the rep is told and the draft stays in the form.
            Response::Conflict {
                table,
                key,
                mine,
                theirs,
                ..
            } => {
                self.status = "Not saved: someone else changed this \
                               record first"
                    .into();
                // Settle it on the screen that asked, if it is still
                // the one open.
                if screen == self.screen {
                    if let View::Form(f) = &mut self.view {
                        self.conflict = f.settle(&mine, &theirs);
                    }
                }
                Change::Read(Rows::Record(table, key), theirs)
            }
            // Sends have a channel of their own, polled in `answers`.
            Response::Sent { send, sent, .. } => {
                return self.sent(send, sent)
            }
            // The orchestrator keeps failures for the status line,
            // and answers sign-ins itself.
            Response::Failed { .. } | Response::SignedIn { .. } => {
                return
            }
        };
        if rolled_back {
            // The list goes back. The form the rep is typing in does
            // not: it is unsaved again, with its draft as typed.
            if let (true, View::Form(f)) =
                (screen == self.screen, &mut self.view)
            {
                f.not_saved();
            }
            if let Some(b) = &mut self.builder {
                b.change(ScreenId(u64::MAX), change, egui);
            }
            return;
        }
        if let Some(b) = &mut self.builder {
            b.change(screen, change, egui);
        }
    }

    /// Swap in the context the builder finished, then do what each
    /// change asked for.
    fn built(&mut self, Built { ctx, done }: Built) {
        if let Some(ctx) = ctx {
            let old = std::mem::replace(&mut self.ctx, *ctx);
            if let Some(b) = &self.builder {
                b.retire(Box::new(old));
            }
        }
        // A datasheet built while a save was out shows it marked. Once
        // anything changes, build it again, so the mark goes.
        if let (View::Sheet(s), true) = (&self.view, !done.is_empty())
        {
            if s.marked {
                self.view = self.view_for(self.target);
            }
        }
        for d in done {
            match d {
                Done::Pending => {}
                // A form opened before its rows arrived draws them
                // now, unless the rep has started typing.
                Done::Read {
                    screen,
                    rows,
                    changed: true,
                } => {
                    let here = screen == self.screen;
                    if (here || rows == Rows::All) && !self.is_dirty()
                    {
                        self.view = self.view_for(self.target);
                    }
                    self.ask_if_unconfirmed();
                }
                Done::Read { .. } => {}
                Done::Saved {
                    screen,
                    key,
                    number,
                } => {
                    if let (true, View::Form(f)) =
                        (screen == self.screen, &mut self.view)
                    {
                        f.saved(key, &self.ctx);
                    }
                    self.status = match number {
                        Some(n) => format!("Saved {n}"),
                        None => "Saved".into(),
                    };
                }
            }
        }
    }

    /// Wait for the builder to finish everything sent to it, and take
    /// it in: for tests, which can't wait a frame at a time.
    pub fn settle(&mut self) {
        while let Some(built) =
            self.builder.as_mut().and_then(Builder::wait)
        {
            self.built(built);
        }
    }

    /// Answers merged and not yet swapped in.
    pub fn building(&self) -> bool {
        self.builder.as_ref().is_some_and(|b| b.pending() > 0)
    }

    /// The status line: what's in flight, and what failed, with Try
    /// again.
    fn status_line(&mut self, ui: &mut Ui) {
        // The manufacturer's side: its own wait and its own failures.
        let (mut mfr_retry, mut mfr_dismiss) = (None, None);
        if let Some(m) = &self.manufacturer {
            for f in m.failures() {
                if ui.small_button("Dismiss").clicked() {
                    mfr_dismiss = Some(f.id);
                }
                if ui.small_button("Try again").clicked() {
                    mfr_retry = Some(f.id);
                }
                ui.label(
                    RichText::new(f.describe())
                        .color(ui.visuals().error_fg_color),
                );
            }
            if let Some(text) = m.status() {
                ui.label(text);
                ui.spinner();
            }
        }
        if let Some(m) = &mut self.manufacturer {
            if let Some(id) = mfr_retry {
                m.retry(id);
            }
            if let Some(id) = mfr_dismiss {
                m.dismiss(id);
            }
        }
        let Some(o) = &mut self.orchestrator else {
            return;
        };
        let (mut retry, mut dismiss) = (None, None);
        for f in o.failures() {
            if ui.small_button("Dismiss").clicked() {
                dismiss = Some(f.id);
            }
            if ui.small_button("Try again").clicked() {
                retry = Some(f.id);
            }
            ui.label(
                RichText::new(f.describe())
                    .color(ui.visuals().error_fg_color),
            );
        }
        if let Some(text) = o.status() {
            ui.label(text);
            ui.spinner();
        }
        if let Some(id) = retry {
            o.retry(id);
        }
        if let Some(id) = dismiss {
            o.dismiss(id);
        }
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
                    match self.ctx.data.role.as_str() {
                        "" if self.orchestrator.is_some() => {
                            ui.label("Connecting…")
                        }
                        role => {
                            ui.label(format!("Connected as {role}"))
                        }
                    };
                    ui.label(
                        RichText::new("Placeholder data")
                            .color(ui.visuals().warn_fg_color),
                    )
                    .on_hover_text(
                        "Generated rows for laying out screens; \
                         V1's data arrives in Chapter 15",
                    );
                    ui.label(RichText::new(&self.status).weak());
                    self.status_line(ui);
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
    /// V2 asks. Save sends the save and goes: the answer merges into the
    /// snapshot whichever screen is open by then.
    /// A refused save: what came in, what stays, and a question for each
    /// field both reps changed. Save sends the merged record over the
    /// version Postgres has now.
    fn conflict_dialog(&mut self, ui: &mut Ui) {
        let Some(c) = &mut self.conflict else {
            return;
        };
        match c.ui(ui, &self.ctx) {
            Some(Settled::Save(saved)) => {
                self.conflict = None;
                self.run(Command::Save(saved));
            }
            Some(Settled::Keep) => self.conflict = None,
            None => {}
        }
    }

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
                match &mut self.orchestrator {
                    Some(o) => {
                        o.save(saved, self.screen);
                    }
                    None => {
                        self.ctx.apply(saved);
                    }
                }
                self.show_view(target);
            }
        } else if discard {
            self.leaving_to = None;
            self.show_view(target);
        } else if stay || modal.should_close() {
            self.leaving_to = None;
        }
    }

    /// The manufacturer didn't answer. Never retried by itself: the
    /// rep decides, and Retry carries the same correlation ID, which
    /// the manufacturer refuses to place twice.
    fn manufacturer_question(&mut self, ui: &mut Ui) {
        let Some(send) = self.prompt() else {
            return;
        };
        let number = self
            .ctx
            .order(send.order_id)
            .map(|o| o.number.clone())
            .unwrap_or_default();
        let modal = egui::Modal::new(egui::Id::unique(
            "manufacturer",
        ))
        .show(ui.ctx(), |ui| {
            ui.set_width(380.0);
            ui.heading("Send to Manufacturer");
            ui.add_space(4.0);
            ui.label(format!("{number} is unconfirmed."));
            ui.label(
                "The manufacturer didn't answer. Retry is safe: \
                     they won't place this order twice.",
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                (
                    ui.button("Retry").clicked(),
                    ui.button("Cancel").clicked(),
                )
            })
            .inner
        });
        let (retry, cancel) = modal.inner;
        if retry {
            let again = self.manufacturer.as_mut().and_then(|m| {
                m.take_prompt().map(|send| ToManufacturer {
                    order_id: send.order_id,
                    order: send.order.retry(),
                })
            });
            if let (Some(again), Some(m)) =
                (again, &mut self.manufacturer)
            {
                m.send(again, self.screen);
            }
        } else if cancel || modal.should_close() {
            // The order stays unconfirmed: its latest attempt has no
            // answer, and opening it asks again.
            if let Some(m) = &mut self.manufacturer {
                m.clear_prompt();
            }
            self.status = format!("{number} is unconfirmed");
        }
    }

    /// One frame of the whole app. It takes whatever answers have
    /// arrived, and never waits for one.
    pub fn show(&mut self, ui: &mut Ui) {
        self.egui = Some(ui.ctx().clone());
        if !self.styled {
            cairn_forms::widgets::form_style(ui.ctx());
            self.styled = true;
        }
        self.answers(&ui.ctx().clone());
        egui::Panel::top("header").show(ui, |ui| self.header(ui));
        let saving = self
            .orchestrator
            .as_ref()
            .is_some_and(|o| o.saving(self.screen));
        let mut cmds = Vec::new();
        egui::CentralPanel::default().show(ui, |ui| {
            cmds = match &mut self.view {
                View::Menu => self.menu(ui),
                View::Sheet(sheet) => sheet.ui(ui),
                View::OrderList(rows) => {
                    Self::order_list(ui, rows, &mut self.orders)
                }
                // The form waits for its save: drawn, but not taking
                // input, so nothing typed now is lost when it reloads.
                View::Form(f) if saving => {
                    ui.disable();
                    f.ui(ui, &self.ctx);
                    Vec::new()
                }
                View::Form(f) => f.ui(ui, &self.ctx),
            };
            self.unsaved_question(ui);
            self.conflict_dialog(ui);
            self.manufacturer_question(ui);
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
