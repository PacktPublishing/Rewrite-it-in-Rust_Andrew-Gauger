//! The reports' own orchestrator (Chapter 14). A print goes out on a
//! channel of its own, to a worker of its own with a limit of its own,
//! the way the manufacturer's sends do. A WeasyPrint run that takes
//! seconds cannot hold a customer lookup that needs two milliseconds,
//! because the two never share a permit.
//!
//! What it keeps is small: which prints are out, which came back as
//! PDFs, and which failed, with the failure's own words.

use crate::reports::Report;
use crate::worker::{Failure, Request, RequestId, Response};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{Receiver, Sender};

/// How many reports may render at once. Each one is a WeasyPrint
/// process, so this is a limit on processes, not on the pool.
pub const LIMIT: usize = 2;

/// Request numbers on this side start here, so a number in the log
/// names one side or the other. The manufacturer's start at 1 << 32.
const FIRST: u64 = 1 << 33;

/// What came back for a print.
#[derive(Debug)]
pub enum Printed {
    Ready { report: Report, path: PathBuf },
    Failed { report: Report, error: Failure },
}

pub struct ReportOrchestrator {
    requests: Sender<Request>,
    responses: Receiver<Response>,
    next: u64,
    in_flight: HashMap<RequestId, Report>,
    /// What the channel had no room for, oldest first.
    waiting: VecDeque<(Request, Report)>,
    /// Prints that could not even be sent.
    stopped: Vec<Printed>,
    /// The login ran out under a render: the records side signs in.
    expired: bool,
}

impl ReportOrchestrator {
    pub fn new(
        requests: Sender<Request>,
        responses: Receiver<Response>,
    ) -> Self {
        ReportOrchestrator {
            requests,
            responses,
            next: FIRST,
            in_flight: HashMap::new(),
            waiting: VecDeque::new(),
            stopped: Vec::new(),
            expired: false,
        }
    }

    /// Ask for `report`. A report already out is not asked for twice:
    /// a double click makes one PDF.
    pub fn print(&mut self, report: Report) -> Option<RequestId> {
        if self.printing(report) {
            return None;
        }
        self.next += 1;
        let id = RequestId(self.next);
        self.waiting
            .push_back((Request::Render { id, report }, report));
        self.pump();
        Some(id)
    }

    fn pump(&mut self) {
        while let Some((request, report)) = self.waiting.pop_front() {
            let id = request.id();
            match self.requests.try_send(request) {
                Ok(()) => {
                    self.in_flight.insert(id, report);
                }
                Err(TrySendError::Full(request)) => {
                    self.waiting.push_front((request, report));
                    break;
                }
                Err(TrySendError::Closed(_)) => {
                    self.stopped.push(Printed::Failed {
                        report,
                        error: Failure::Stopped(
                            "the report worker has stopped".into(),
                        ),
                    });
                }
            }
        }
    }

    /// Once a frame: send what is waiting, take what has come back.
    /// Never waits.
    pub fn poll(&mut self) -> Vec<Printed> {
        self.pump();
        let mut done = std::mem::take(&mut self.stopped);
        while let Ok(response) = self.responses.try_recv() {
            let Some(report) = self.in_flight.remove(&response.id())
            else {
                continue;
            };
            match response {
                Response::Rendered { path, .. } => {
                    done.push(Printed::Ready { report, path })
                }
                Response::Failed { error, .. } => {
                    self.expired |= error.session_expired().is_some();
                    done.push(Printed::Failed { report, error });
                }
                _ => {}
            }
        }
        self.pump();
        done
    }

    pub fn printing(&self, report: Report) -> bool {
        self.in_flight
            .values()
            .chain(self.waiting.iter().map(|(_, r)| r))
            .any(|r| *r == report)
    }

    pub fn take_expired(&mut self) -> bool {
        std::mem::take(&mut self.expired)
    }

    /// What the status line says while prints are out.
    pub fn status(&self) -> Option<String> {
        let n = self.in_flight.len() + self.waiting.len();
        (n > 0).then(|| format!("Making {n} PDF(s)…"))
    }
}
