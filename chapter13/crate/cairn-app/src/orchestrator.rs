//! The App Orchestrator: the one place that knows what's in flight. A
//! widget lasts one frame; this lasts between them. It owns both channel
//! ends, numbers every request, drops an answer a newer request has
//! overtaken, holds what the channel has no room for, and keeps every
//! failure, with the save that failed, until the rep has dealt with it.
//! When Postgres says the rep's login has run out, it holds the
//! request, signs the rep in again, and sends it again.

use crate::context::Saved;
use crate::data::{Rows, Snapshot, Table};
use crate::worker::{Failure, Request, RequestId, Response};
use cairn_auth::desktop::Expired;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{Receiver, Sender};

/// Which screen asked: the app numbers each view it opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ScreenId(pub u64);

/// The kinds of request, each with its own backpressure policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Never dropped: it waits in the orchestrator's queue.
    Save,
    /// A new login. Everything else waits until it's in.
    SignIn,
    /// Every row of a table (or of everything): one already waiting
    /// covers a second.
    Refresh,
    /// One record, for the form that opens it: a new one replaces the
    /// one waiting.
    Lookup,
}

/// A request sent and not yet answered, or waiting to be sent.
#[derive(Debug)]
pub struct InFlight {
    pub kind: Kind,
    /// What it reads or writes, for staleness.
    pub rows: Option<Rows>,
    pub screen: ScreenId,
    pub since: Instant,
    /// A save's record, kept until it's stored, so a failure loses no
    /// edit.
    pub saved: Option<Box<Saved>>,
    /// Sent again after a new login. Refused again, it's a failure
    /// to show, not a reason to sign in a third time.
    pub again: bool,
    /// An optimistic save's way back: the record as the list had it.
    /// Taken when the save fails, as a read that puts it back.
    pub last_good: Option<Box<Snapshot>>,
}

/// A request that failed, kept for the status line and for Try again.
#[derive(Debug)]
pub struct Failed {
    pub id: RequestId,
    pub request: InFlight,
    pub error: Failure,
}

/// A fresh answer, for the app to merge.
#[derive(Debug)]
pub struct Answer {
    pub screen: ScreenId,
    pub response: Response,
    /// A read that puts back a row the list showed too soon, not news
    /// from Postgres: it never rebuilds the form the rep is typing in.
    pub rolled_back: bool,
}

pub struct Orchestrator {
    requests: Sender<Request>,
    responses: Receiver<Response>,
    next: u64,
    /// Sent, not yet answered.
    in_flight: HashMap<RequestId, InFlight>,
    /// The newest request for each set of rows. An answer to an older
    /// one is stale.
    latest: HashMap<Rows, RequestId>,
    /// What the channel had no room for, oldest first.
    waiting: VecDeque<(Request, InFlight)>,
    failed: Vec<Failed>,
    /// The sign-in out now, if one is.
    signing_in: Option<RequestId>,
    /// Held saves already logged, so the log says it once.
    held_logged: std::collections::HashSet<RequestId>,
    /// Requests refused because the login ran out, to send again once
    /// the rep has signed in again.
    expired: Vec<(InFlight, Failure)>,
    /// Answers dropped as stale.
    pub stale: usize,
    /// Sign-ins sent, the first one included.
    pub sign_ins: usize,
    /// How the login last ran out.
    pub expired_by: Option<Expired>,
    /// One save per record in flight: a second save of a record waits
    /// until the first is stored, then goes with the first's version.
    /// Always on, except in the test that shows what happens without
    /// it.
    ordered_saves: bool,
}

impl Orchestrator {
    pub fn new(
        requests: Sender<Request>,
        responses: Receiver<Response>,
    ) -> Self {
        Orchestrator {
            requests,
            responses,
            next: 0,
            in_flight: HashMap::new(),
            latest: HashMap::new(),
            waiting: VecDeque::new(),
            failed: Vec::new(),
            signing_in: None,
            held_logged: Default::default(),
            expired: Vec::new(),
            stale: 0,
            sign_ins: 0,
            expired_by: None,
            ordered_saves: true,
        }
    }

    /// Send every save as soon as there is room, even a second save of
    /// a record whose first is still out. Both then carry the version
    /// the rep read, and the second is refused. Only for the test that
    /// shows it; the app never calls this.
    pub fn with_unordered_saves(mut self) -> Self {
        self.ordered_saves = false;
        self
    }

    /// Sign the rep in: the first request when the window opens.
    /// Nothing else is sent until the login is in.
    pub fn sign_in(&mut self) -> RequestId {
        if let Some(id) = self.signing_in {
            return id;
        }
        let id = self.id();
        self.signing_in = Some(id);
        self.sign_ins += 1;
        let info = self.info(Kind::SignIn, None, ScreenId::default());
        self.waiting.push_front((Request::SignIn { id }, info));
        self.pump();
        id
    }

    /// Read `rows`: a refresh, or a lookup for one record.
    pub fn read(
        &mut self,
        rows: Rows,
        screen: ScreenId,
    ) -> RequestId {
        let id = self.id();
        let kind = match rows {
            Rows::Record(..) => Kind::Lookup,
            _ => Kind::Refresh,
        };
        let request = Request::Read { id, rows };
        self.submit(request, self.info(kind, Some(rows), screen))
    }

    pub fn save(
        &mut self,
        saved: Saved,
        screen: ScreenId,
    ) -> RequestId {
        let id = self.id();
        let mut info = self.info(Kind::Save, record(&saved), screen);
        info.saved = Some(Box::new(saved.clone()));
        let request = Request::Save {
            id,
            saved: Box::new(saved),
        };
        self.submit(request, info)
    }

    /// A save the list already shows (`Change::Pending`): if it fails,
    /// `last_good` comes back as a read of the same record.
    pub fn save_optimistic(
        &mut self,
        saved: Saved,
        screen: ScreenId,
        last_good: Box<Snapshot>,
    ) -> RequestId {
        let id = self.id();
        let mut info = self.info(Kind::Save, record(&saved), screen);
        info.saved = Some(Box::new(saved.clone()));
        // A second save of a record keeps the first's way back: the
        // list already shows the first, and if both fail, the row that
        // comes back is the one Postgres had.
        let earlier = self
            .pending()
            .find(|w| w.rows == info.rows && w.last_good.is_some())
            .and_then(|w| w.last_good.clone());
        info.last_good = Some(earlier.unwrap_or(last_good));
        let request = Request::Save {
            id,
            saved: Box::new(saved),
        };
        self.submit(request, info)
    }

    /// The keys in `table` with a save the list shows and Postgres
    /// hasn't answered.
    pub fn pending_keys(&self, table: Table) -> Vec<i32> {
        self.pending()
            .filter(|w| w.last_good.is_some())
            .filter_map(|w| match w.rows {
                Some(Rows::Record(t, k)) if t == table => Some(k),
                _ => None,
            })
            .collect()
    }

    fn id(&mut self) -> RequestId {
        self.next += 1;
        RequestId(self.next)
    }

    fn info(
        &self,
        kind: Kind,
        rows: Option<Rows>,
        screen: ScreenId,
    ) -> InFlight {
        InFlight {
            kind,
            rows,
            screen,
            since: Instant::now(),
            saved: None,
            again: false,
            last_good: None,
        }
    }

    /// Send now if the channel has room; otherwise apply the request's
    /// policy and queue it.
    fn submit(
        &mut self,
        request: Request,
        info: InFlight,
    ) -> RequestId {
        let id = request.id();
        match info.kind {
            Kind::Save | Kind::SignIn => {}
            Kind::Refresh => {
                let same = |w: &InFlight| {
                    w.kind == Kind::Refresh && w.rows == info.rows
                };
                if let Some((r, _)) =
                    self.waiting.iter().find(|(_, w)| same(w))
                {
                    return r.id();
                }
            }
            Kind::Lookup => {
                self.waiting.retain(|(_, w)| w.kind != Kind::Lookup)
            }
        }
        if let Some(rows) = info.rows {
            self.latest.insert(rows, id);
        }
        self.waiting.push_back((request, info));
        self.pump();
        id
    }

    /// Send what's waiting, oldest first, until the channel is full.
    /// While a sign-in is out, only the sign-in goes.
    fn pump(&mut self) {
        let mut at = 0;
        while at < self.waiting.len() {
            let (request, info) = &self.waiting[at];
            if self.signing_in.is_some_and(|s| s != request.id()) {
                break;
            }
            // One save per record in flight: this one waits for the
            // first, and the others behind it go on.
            if self.ordered_saves && self.save_is_out(info) {
                if self.held_logged.insert(request.id()) {
                    tracing::info!(
                        id = request.id().0,
                        "held: this record's save is still out"
                    );
                }
                at += 1;
                continue;
            }
            let Some((request, info)) = self.waiting.remove(at)
            else {
                break;
            };
            let id = request.id();
            match self.requests.try_send(request) {
                Ok(()) => {
                    tracing::info!(id = id.0, "sent to the worker");
                    self.in_flight.insert(id, info);
                }
                Err(TrySendError::Full(request)) => {
                    self.waiting.insert(at, (request, info));
                    break;
                }
                Err(TrySendError::Closed(_)) => {
                    let error = Failure::Stopped(
                        "the worker has stopped".into(),
                    );
                    self.failed.push(Failed {
                        id,
                        request: info,
                        error,
                    });
                }
            }
        }
    }

    /// A save of the same record is out, unanswered.
    fn save_is_out(&self, info: &InFlight) -> bool {
        info.kind == Kind::Save
            && info.rows.is_some()
            && self
                .in_flight
                .values()
                .any(|f| f.kind == Kind::Save && f.rows == info.rows)
    }

    /// A save of `rows` was stored: the saves waiting on it go with the
    /// versions Postgres gave, not the ones the rep read.
    fn rebase_waiting(
        &mut self,
        rows: Rows,
        stored: &Saved,
        version: i32,
    ) {
        for (request, info) in &mut self.waiting {
            if info.kind != Kind::Save || info.rows != Some(rows) {
                continue;
            }
            if let Request::Save { saved, .. } = request {
                saved.rebase(stored, version);
            }
            if let Some(saved) = &mut info.saved {
                saved.rebase(stored, version);
            }
        }
    }

    /// Once a frame: send what's waiting, and take the answers that
    /// have come in. Never waits.
    pub fn poll(&mut self) -> Vec<Answer> {
        self.pump();
        let mut answers = Vec::new();
        while let Ok(response) = self.responses.try_recv() {
            let id = response.id();
            let Some(info) = self.in_flight.remove(&id) else {
                continue;
            };
            // A save always counts. A read that a newer one for the
            // same rows has overtaken doesn't, even if it failed.
            let overtaken = info
                .rows
                .is_some_and(|r| self.latest.get(&r) != Some(&id));
            if info.kind != Kind::Save && overtaken {
                self.stale += 1;
                continue;
            }
            match response {
                Response::Failed { error, .. }
                    if info.kind == Kind::SignIn =>
                {
                    self.signed_in(Some((id, info, error)))
                }
                // The login ran out (SQLSTATE class 28). Postgres did
                // nothing with the request, so it's kept, and sent
                // again once the rep has signed in again.
                Response::Failed { error, .. }
                    if !info.again
                        && error.session_expired().is_some() =>
                {
                    self.expired_by = error.session_expired();
                    self.expired.push((info, error));
                    self.sign_in();
                }
                Response::Failed { error, .. } => {
                    let mut info = info;
                    // The list showed the save before Postgres had
                    // it. It didn't take it, so the row goes back.
                    if let (Some(last_good), Some(rows)) =
                        (info.last_good.take(), info.rows)
                    {
                        answers.push(Answer {
                            screen: info.screen,
                            response: Response::Read {
                                id,
                                rows,
                                data: last_good,
                            },
                            rolled_back: true,
                        });
                    }
                    self.failed.push(Failed {
                        id,
                        request: info,
                        error,
                    })
                }
                Response::SignedIn { .. } => self.signed_in(None),
                Response::Saved {
                    ref stored,
                    ref saved,
                    ..
                } => {
                    if let Some(rows) = info.rows {
                        self.rebase_waiting(
                            rows,
                            saved,
                            stored.version,
                        );
                    }
                    answers.push(Answer {
                        screen: info.screen,
                        response,
                        rolled_back: false,
                    });
                }
                response => answers.push(Answer {
                    screen: info.screen,
                    response,
                    rolled_back: false,
                }),
            }
        }
        self.pump();
        answers
    }

    /// The sign-in is answered: send what it held up, or, if it
    /// failed, show that and keep each held request for Try again.
    fn signed_in(
        &mut self,
        failed: Option<(RequestId, InFlight, Failure)>,
    ) {
        self.signing_in = None;
        let held = std::mem::take(&mut self.expired);
        match failed {
            None => {
                for (request, _) in held {
                    let Some(id) = self.resend(request) else {
                        continue;
                    };
                    let waiting = self
                        .waiting
                        .iter_mut()
                        .find(|(r, _)| r.id() == id)
                        .map(|(_, w)| w);
                    if let Some(w) =
                        waiting.or(self.in_flight.get_mut(&id))
                    {
                        w.again = true;
                    }
                }
            }
            Some((id, request, error)) => {
                self.failed.push(Failed { id, request, error });
                for (request, error) in held {
                    let id = self.id();
                    self.failed.push(Failed { id, request, error });
                }
            }
        }
        self.pump();
    }

    /// The same request again, with a new number: a save with the
    /// record it kept, a send with its order, a read for the same
    /// rows.
    fn resend(&mut self, request: InFlight) -> Option<RequestId> {
        let screen = request.screen;
        Some(match request {
            InFlight {
                saved: Some(saved), ..
            } => self.save(*saved, screen),
            InFlight {
                kind: Kind::SignIn, ..
            } => self.sign_in(),
            InFlight {
                rows: Some(rows), ..
            } => self.read(rows, screen),
            _ => return None,
        })
    }

    /// Send a failed request again.
    pub fn retry(&mut self, id: RequestId) -> Option<RequestId> {
        let at = self.failed.iter().position(|f| f.id == id)?;
        let f = self.failed.remove(at);
        self.resend(f.request)
    }

    pub fn dismiss(&mut self, id: RequestId) {
        self.failed.retain(|f| f.id != id);
    }

    pub fn failures(&self) -> &[Failed] {
        &self.failed
    }

    fn pending(&self) -> impl Iterator<Item = &InFlight> {
        self.in_flight
            .values()
            .chain(self.waiting.iter().map(|(_, w)| w))
            .chain(self.expired.iter().map(|(w, _)| w))
    }

    /// Requests held until the rep has signed in again.
    pub fn expired(&self) -> usize {
        self.expired.len()
    }

    /// A save from `screen` hasn't been answered: the form waits.
    pub fn saving(&self, screen: ScreenId) -> bool {
        self.pending()
            .any(|w| w.kind == Kind::Save && w.screen == screen)
    }

    /// What the status line says while requests are out.
    pub fn status(&self) -> Option<String> {
        if self.signing_in.is_some() && !self.expired.is_empty() {
            return Some(
                "Your sign-in ran out. Sign in again to carry on…"
                    .into(),
            );
        }
        if self.signing_in.is_some() {
            return Some("Signing in…".into());
        }
        let saves =
            self.pending().filter(|w| w.kind == Kind::Save).count();
        match saves {
            0 if self.pending().next().is_some() => {
                Some("Reading…".into())
            }
            0 => None,
            1 => Some("Saving…".into()),
            n => Some(format!("Saving {n} records…")),
        }
    }

    /// Requests sent and not answered.
    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }

    /// Requests in the channel, sent and not yet taken by the worker.
    pub fn in_channel(&self) -> usize {
        self.requests.max_capacity() - self.requests.capacity()
    }

    /// Requests the channel had no room for yet.
    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }
}

impl Failed {
    /// The status line's words for it.
    pub fn describe(&self) -> String {
        let what = match self.request.kind {
            Kind::Save => "save",
            Kind::SignIn => "sign in",
            Kind::Refresh | Kind::Lookup => "read",
        };
        format!("Couldn't {what}: {}", self.error)
    }
}

/// The record a save writes, once it has a key.
fn record(saved: &Saved) -> Option<Rows> {
    let (table, key) = match saved {
        Saved::Company(c) => (Table::Company, c.id),
        Saved::Rep(r) => (Table::Reps, r.id.map(|i| i.0)),
        Saved::Product(p) => (Table::Products, p.id.map(|i| i.0)),
        Saved::Activity(a) => (Table::Activities, a.id),
        Saved::Message(m) => (Table::Messages, m.id),
        Saved::Rma(r) => (Table::Rmas, r.id),
        Saved::Customer { customer, .. } => {
            (Table::Customers, customer.id.map(|i| i.0))
        }
        Saved::Quote { quote, .. } => {
            (Table::Quotes, quote.id.map(|i| i.0))
        }
        Saved::Order { order, .. } => {
            (Table::Orders, order.id.map(|i| i.0))
        }
    };
    match table {
        Table::Company => Some(Rows::Table(Table::Company)),
        _ => Some(Rows::Record(table, key?)),
    }
}
