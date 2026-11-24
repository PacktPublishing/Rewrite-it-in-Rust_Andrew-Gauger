//! The manufacturer's own orchestrator (Chapter 12). A custom order goes
//! out on a channel of its own, to a worker of its own with a limit of
//! its own, and the retry prompt's state lives here, not in the records
//! side. Nothing the manufacturer takes twenty seconds over can hold a
//! customer lookup that needs two milliseconds, because the two never
//! share a permit.
//!
//! The records side keeps reads, saves, conflicts and sign-in. The app
//! is the router: a command goes to the side that owns it, and each
//! side's answers come back to the frame the same way, with `try_recv`.

use crate::manufacturer::Sent;
use crate::orchestrator::ScreenId;
use crate::worker::{
    Failure, Request, RequestId, Response, ToManufacturer,
};
use cairn_core::OrderId;
use std::collections::{HashMap, VecDeque};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{Receiver, Sender};

/// How many attempts may be out at once. A send waits on the
/// manufacturer, not on the pool, so this is the manufacturer's limit
/// and nobody else's.
pub const LIMIT: usize = 2;

/// Request numbers on this side start here, so a number in the log
/// names one side or the other.
const FIRST: u64 = 1 << 32;

/// An attempt that failed before the manufacturer could say anything
/// (the order's attempt couldn't be recorded, the login ran out), kept
/// for the status line and Try again.
#[derive(Debug)]
pub struct SendFailed {
    pub id: RequestId,
    pub send: Box<ToManufacturer>,
    pub error: Failure,
}

impl SendFailed {
    pub fn describe(&self) -> String {
        format!("Couldn't send to the manufacturer: {}", self.error)
    }
}

pub struct ManufacturerOrchestrator {
    requests: Sender<Request>,
    responses: Receiver<Response>,
    next: u64,
    /// Sent, not yet answered.
    in_flight: HashMap<RequestId, (Box<ToManufacturer>, ScreenId)>,
    /// What the channel had no room for, oldest first.
    waiting: VecDeque<(Request, Box<ToManufacturer>, ScreenId)>,
    failed: Vec<SendFailed>,
    /// An order the manufacturer didn't confirm: the Retry/Cancel
    /// question, until the rep answers it.
    prompt: Option<Box<ToManufacturer>>,
    /// The view the question was last asked on, so Cancel holds until
    /// the order is opened again.
    prompted: ScreenId,
    /// The login ran out under an attempt: the records side signs in.
    expired: bool,
}

impl ManufacturerOrchestrator {
    pub fn new(
        requests: Sender<Request>,
        responses: Receiver<Response>,
    ) -> Self {
        ManufacturerOrchestrator {
            requests,
            responses,
            next: FIRST,
            in_flight: HashMap::new(),
            waiting: VecDeque::new(),
            failed: Vec::new(),
            prompt: None,
            prompted: ScreenId::default(),
            expired: false,
        }
    }

    /// One attempt at the manufacturer. Never dropped: if the channel
    /// is full it waits here, in order.
    pub fn send(
        &mut self,
        send: ToManufacturer,
        screen: ScreenId,
    ) -> RequestId {
        self.next += 1;
        let id = RequestId(self.next);
        let send = Box::new(send);
        let request = Request::Send {
            id,
            send: send.clone(),
        };
        self.waiting.push_back((request, send, screen));
        self.pump();
        id
    }

    fn pump(&mut self) {
        while let Some((request, send, screen)) =
            self.waiting.pop_front()
        {
            let id = request.id();
            match self.requests.try_send(request) {
                Ok(()) => {
                    self.in_flight.insert(id, (send, screen));
                }
                Err(TrySendError::Full(request)) => {
                    self.waiting.push_front((request, send, screen));
                    break;
                }
                Err(TrySendError::Closed(_)) => {
                    self.failed.push(SendFailed {
                        id,
                        send,
                        error: Failure::Stopped(
                            "the manufacturer's worker has stopped"
                                .into(),
                        ),
                    });
                }
            }
        }
    }

    /// Once a frame: send what's waiting, and take the answers that
    /// have come in. Never waits.
    pub fn poll(&mut self) -> Vec<(Box<ToManufacturer>, Sent)> {
        self.pump();
        let mut heard = Vec::new();
        while let Ok(response) = self.responses.try_recv() {
            let id = response.id();
            let Some((send, _)) = self.in_flight.remove(&id) else {
                continue;
            };
            match response {
                Response::Sent { sent, .. } => {
                    heard.push((send, sent))
                }
                Response::Failed { error, .. } => {
                    self.expired |= error.session_expired().is_some();
                    self.failed.push(SendFailed { id, send, error });
                }
                _ => {}
            }
        }
        self.pump();
        heard
    }

    /// An attempt for sales order `order` hasn't been answered.
    pub fn sending(&self, order: OrderId) -> bool {
        self.in_flight
            .values()
            .map(|(s, _)| s)
            .chain(self.waiting.iter().map(|(_, s, _)| s))
            .any(|s| s.order_id == order)
    }

    /// Whether the login ran out under an attempt since last asked.
    pub fn take_expired(&mut self) -> bool {
        std::mem::take(&mut self.expired)
    }

    pub fn failures(&self) -> &[SendFailed] {
        &self.failed
    }

    /// Send a failed attempt again, as itself.
    pub fn retry(&mut self, id: RequestId) -> Option<RequestId> {
        let at = self.failed.iter().position(|f| f.id == id)?;
        let f = self.failed.remove(at);
        Some(self.send(*f.send, ScreenId::default()))
    }

    pub fn dismiss(&mut self, id: RequestId) {
        self.failed.retain(|f| f.id != id);
    }

    /// What the status line says while attempts are out.
    pub fn status(&self) -> Option<String> {
        let n = self.in_flight.len() + self.waiting.len();
        (n > 0)
            .then(|| format!("Waiting for the manufacturer ({n})…"))
    }

    /// The Retry/Cancel question, if one is open.
    pub fn prompt(&self) -> Option<&ToManufacturer> {
        self.prompt.as_deref()
    }

    /// Ask about `send`, on view `screen`.
    pub fn ask(
        &mut self,
        send: Box<ToManufacturer>,
        screen: ScreenId,
    ) {
        self.prompt = Some(send);
        self.prompted = screen;
    }

    /// Whether `screen` has been asked already.
    pub fn asked(&self, screen: ScreenId) -> bool {
        self.prompt.is_some() || self.prompted == screen
    }

    /// The question answered or left behind.
    pub fn take_prompt(&mut self) -> Option<Box<ToManufacturer>> {
        self.prompt.take()
    }

    pub fn clear_prompt(&mut self) {
        self.prompt = None;
    }
}
