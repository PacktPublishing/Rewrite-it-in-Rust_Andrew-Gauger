//! The App Orchestrator: the one place that knows what's in flight. A
//! widget lasts one frame; this lasts between them. It owns both channel
//! ends, numbers every request, drops an answer a newer request has
//! overtaken, holds what the channel has no room for, and keeps every
//! failure, with the save that failed, until the rep has dealt with it.
//! When Postgres says the rep's login has run out, it holds the
//! request, signs the rep in again, and sends it again.

use crate::context::Saved;
use crate::data::{Rows, Table};
use crate::worker::{
    Failure, Request, RequestId, Response, ToManufacturer,
};
use cairn_auth::desktop::Expired;
use cairn_core::OrderId;
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
    /// A custom order to the manufacturer: a save's policy.
    Send,
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
    /// A send's order, kept the same way: a resend carries its
    /// correlation ID.
    pub send: Option<Box<ToManufacturer>>,
    /// Sent again after a new login. Refused again, it's a failure
    /// to show, not a reason to sign in a third time.
    pub again: bool,
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
    /// Requests refused because the login ran out, to send again once
    /// the rep has signed in again.
    expired: Vec<(InFlight, Failure)>,
    /// Answers dropped as stale.
    pub stale: usize,
    /// Sign-ins sent, the first one included.
    pub sign_ins: usize,
    /// How the login last ran out.
    pub expired_by: Option<Expired>,
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
            expired: Vec::new(),
            stale: 0,
            sign_ins: 0,
            expired_by: None,
        }
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

    /// One attempt at the manufacturer.
    pub fn send(
        &mut self,
        send: ToManufacturer,
        screen: ScreenId,
    ) -> RequestId {
        let id = self.id();
        let mut info = self.info(Kind::Send, None, screen);
        info.send = Some(Box::new(send.clone()));
        let request = Request::Send {
            id,
            send: Box::new(send),
        };
        self.submit(request, info)
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
            send: None,
            again: false,
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
            Kind::Save | Kind::Send | Kind::SignIn => {}
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
        while let Some((request, info)) = self.waiting.pop_front() {
            let id = request.id();
            if self.signing_in.is_some_and(|s| s != id) {
                self.waiting.push_front((request, info));
                break;
            }
            match self.requests.try_send(request) {
                Ok(()) => {
                    self.in_flight.insert(id, info);
                }
                Err(TrySendError::Full(request)) => {
                    self.waiting.push_front((request, info));
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
                    self.failed.push(Failed {
                        id,
                        request: info,
                        error,
                    })
                }
                Response::SignedIn { .. } => self.signed_in(None),
                response => answers.push(Answer {
                    screen: info.screen,
                    response,
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
                send: Some(send), ..
            } => self.send(*send, screen),
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

    /// An attempt for sales order `order` hasn't been answered.
    pub fn sending(&self, order: OrderId) -> bool {
        self.pending().any(|w| {
            w.send.as_ref().is_some_and(|s| s.order_id == order)
        })
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
        if self.pending().any(|w| w.kind == Kind::Send) {
            return Some("Sending to the manufacturer…".into());
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
            Kind::Send => "send to the manufacturer",
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
