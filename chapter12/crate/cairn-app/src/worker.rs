//! The far side of the channels: requests in, responses out, a tokio
//! task for each request. The UI thread sends with `try_send` and drains
//! with `try_recv`, so only this side ever waits.

use crate::context::Saved;
use crate::data::{self, Rows, Snapshot, Table};
use crate::manufacturer::{self, Manufacturer, Sent};
use crate::save::{self, Rejected, Stored};
use crate::sign_in::{Fixed, Refused, SignIn};
use cairn_auth::desktop::{session_expired, Expired};
use cairn_core::manufacturer::ManufacturerOrder;
use cairn_core::OrderId;
use sqlx::PgPool;
use std::fmt;
use std::future::Future;
use std::sync::{Arc, RwLock};
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinError;
use tracing::Instrument;

/// Room in each channel. A burst bigger than this waits in the
/// orchestrator's own queue, where the screen can say so.
pub const CAPACITY: usize = 8;

/// Every request's number, so its answer can find it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct RequestId(pub u64);

/// What the UI asks the workers for.
#[derive(Debug)]
pub enum Request {
    /// Rows as Postgres has them now: everything when the window
    /// opens, a table for a refresh, a record when a form opens it.
    Read { id: RequestId, rows: Rows },
    /// A record and its subform rows, to write. A form's
    /// `Command::Save` arrives here.
    Save { id: RequestId, saved: Box<Saved> },
    /// One attempt to send a custom order. A Retry sends
    /// `order.retry()`: the same correlation ID.
    Send {
        id: RequestId,
        send: Box<ToManufacturer>,
    },
    /// A login for the rep: when the window opens, and again when
    /// Postgres says the last one has run out.
    SignIn { id: RequestId },
}

/// A sales order's custom order, for the manufacturer.
#[derive(Debug, Clone, PartialEq)]
pub struct ToManufacturer {
    pub order_id: OrderId,
    pub order: ManufacturerOrder,
}

/// What comes back, one for every request.
#[derive(Debug)]
pub enum Response {
    Read {
        id: RequestId,
        rows: Rows,
        data: Box<Snapshot>,
    },
    /// The record as stored: keys, numbers, version.
    Saved {
        id: RequestId,
        stored: Stored,
        saved: Box<Saved>,
    },
    /// The save wrote nothing: another rep saved the record first.
    /// `mine` is what this rep tried to save; `theirs` is the record
    /// as Postgres has it now (a record that is gone reads back empty).
    Conflict {
        id: RequestId,
        table: Table,
        key: i32,
        mine: Box<Saved>,
        theirs: Box<Snapshot>,
    },
    /// What the attempt came to, `Unconfirmed` included: an answer
    /// that never came is an outcome, not a failure.
    Sent {
        id: RequestId,
        send: Box<ToManufacturer>,
        sent: Sent,
    },
    /// The pool now connects with the new login.
    SignedIn {
        id: RequestId,
    },
    Failed {
        id: RequestId,
        error: Failure,
    },
}

/// Why a request has no answer but a failure.
#[derive(Debug)]
pub enum Failure {
    Database(sqlx::Error),
    SignIn(Refused),
    /// The task panicked, or the runtime stopped it.
    Stopped(String),
}

impl Failure {
    /// Postgres refused the rep's login (SQLSTATE class 28): sign in
    /// again and resend.
    pub fn session_expired(&self) -> Option<Expired> {
        match self {
            Failure::Database(e) => session_expired(e),
            _ => None,
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::Database(e) => write!(f, "{e}"),
            Failure::SignIn(why) => write!(f, "{why}"),
            Failure::Stopped(why) => write!(f, "{why}"),
        }
    }
}

impl From<JoinError> for Failure {
    fn from(e: JoinError) -> Self {
        Failure::Stopped(e.to_string())
    }
}

impl Request {
    pub fn id(&self) -> RequestId {
        match self {
            Request::Read { id, .. }
            | Request::Save { id, .. }
            | Request::Send { id, .. }
            | Request::SignIn { id } => *id,
        }
    }
}

impl Response {
    pub fn id(&self) -> RequestId {
        match self {
            Response::Read { id, .. }
            | Response::Saved { id, .. }
            | Response::Conflict { id, .. }
            | Response::Sent { id, .. }
            | Response::SignedIn { id }
            | Response::Failed { id, .. } => *id,
        }
    }
}

/// What every worker task shares: the pool for the rep's current
/// login, and the way to get the next one.
pub struct Workers {
    pool: RwLock<Option<PgPool>>,
    sign_in: Box<dyn SignIn>,
    /// The manufacturer's `/orders`, when it isn't Company Info's
    /// `manufacturer_api_url` (a test's mock).
    manufacturer_url: Option<String>,
}

impl Workers {
    /// Workers with no login yet: the first request is a sign-in.
    pub fn new(sign_in: impl SignIn) -> Self {
        Workers {
            pool: RwLock::new(None),
            sign_in: Box::new(sign_in),
            manufacturer_url: None,
        }
    }

    /// Signed in with `pool`'s login, which a sign-in gives again.
    pub fn signed_in(pool: PgPool) -> Self {
        let login = (*pool.connect_options()).clone();
        Workers::new(Fixed(login)).with_pool(pool)
    }

    /// Already signed in, with `pool`.
    pub fn with_pool(self, pool: PgPool) -> Self {
        *self.pool.write().expect("not poisoned") = Some(pool);
        self
    }

    pub fn with_manufacturer(self, orders_url: &str) -> Self {
        Workers {
            manufacturer_url: Some(orders_url.into()),
            ..self
        }
    }

    fn pool(&self) -> Option<PgPool> {
        self.pool.read().expect("not poisoned").clone()
    }

    /// Sign in, check the login with one connection, and swap the
    /// pool. Requests already out finish on the old one.
    async fn sign_in(&self) -> Result<(), Failure> {
        let login =
            self.sign_in.sign_in().await.map_err(Failure::SignIn)?;
        let pool = data::pool(login);
        pool.acquire().await.map_err(Failure::Database)?;
        let old =
            self.pool.write().expect("not poisoned").replace(pool);
        if let Some(old) = old {
            tokio::spawn(async move { old.close().await });
        }
        Ok(())
    }

    /// One attempt at the manufacturer, recorded in
    /// `manufacturer_orders`.
    async fn send(
        &self,
        db: &PgPool,
        send: &ToManufacturer,
    ) -> sqlx::Result<Sent> {
        let url = match &self.manufacturer_url {
            Some(url) => Some(url.clone()),
            None => sqlx::query_scalar!(
                "SELECT manufacturer_api_url FROM company_info
                  ORDER BY id LIMIT 1"
            )
            .fetch_one(db)
            .await?
            .filter(|u| !u.is_empty()),
        };
        let Some(url) = url else {
            return Ok(Sent::Failed {
                reason: "No manufacturer address is set up in \
                         Company Info."
                    .into(),
            });
        };
        let mfr = match Manufacturer::new(&url, manufacturer::TIMEOUT)
        {
            Ok(mfr) => mfr,
            Err(e) => {
                return Ok(Sent::Failed {
                    reason: e.to_string(),
                })
            }
        };
        let order_id = send.order_id.0;
        manufacturer::send(db, &mfr, order_id, None, &send.order)
            .await
    }
}

/// A refused save's answer: the record as Postgres has it now, beside
/// the one the rep tried to save.
async fn conflict(
    db: &PgPool,
    id: RequestId,
    mine: Box<Saved>,
) -> Result<Response, sqlx::Error> {
    let (table, key) = mine.target().ok_or_else(|| {
        sqlx::Error::Protocol(
            "a new record has no version to refuse".into(),
        )
    })?;
    let theirs = data::read(db, Rows::Record(table, key)).await?;
    Ok(Response::Conflict {
        id,
        table,
        key,
        mine,
        theirs: Box::new(theirs),
    })
}

impl Request {
    fn kind(&self) -> &'static str {
        match self {
            Request::Read { .. } => "read",
            Request::Save { .. } => "save",
            Request::Send { .. } => "send",
            Request::SignIn { .. } => "sign_in",
        }
    }
}

/// One request, inside a span keyed by its id, so every line it logs
/// carries `request{id=7 kind="save"}`.
pub async fn handle(w: Arc<Workers>, request: Request) -> Response {
    let span = tracing::info_span!(
        "request",
        id = request.id().0,
        kind = request.kind()
    );
    async {
        tracing::info!("started");
        let started = std::time::Instant::now();
        let response = work(w, request).await;
        let ms = started.elapsed().as_millis();
        match &response {
            Response::Conflict { table, key, .. } => {
                tracing::warn!(?table, key, ms, "stale: conflict")
            }
            Response::Failed { error, .. } => {
                tracing::warn!(%error, ms, "failed")
            }
            _ => tracing::info!(ms, "finished"),
        }
        response
    }
    .instrument(span)
    .await
}

/// A sign-in, or work against the current login's pool.
async fn work(w: Arc<Workers>, request: Request) -> Response {
    let id = request.id();
    let failed = |error| Response::Failed { id, error };
    if let Request::SignIn { .. } = request {
        return match w.sign_in().await {
            Ok(()) => Response::SignedIn { id },
            Err(error) => failed(error),
        };
    }
    let Some(db) = w.pool() else {
        return failed(Failure::Stopped("not signed in".into()));
    };
    let answer = match request {
        Request::Read { rows, .. } => {
            data::read(&db, rows).await.map(|d| Response::Read {
                id,
                rows,
                data: Box::new(d),
            })
        }
        Request::Save { saved, .. } => {
            let mine = saved.clone();
            match save::save(&db, *saved).await {
                Ok((stored, saved)) => Ok(Response::Saved {
                    id,
                    stored,
                    saved: Box::new(saved),
                }),
                Err(Rejected::Db(e)) => Err(e),
                Err(Rejected::Stale) => conflict(&db, id, mine).await,
            }
        }
        Request::Send { send, .. } => w
            .send(&db, &send)
            .await
            .map(|sent| Response::Sent { id, send, sent }),
        Request::SignIn { .. } => unreachable!("answered above"),
    };
    answer.unwrap_or_else(|e| failed(Failure::Database(e)))
}

/// The worker loop: a task per request, at most `limit` running at
/// once, so a burst waits in the bounded channel instead of piling up
/// on the pool. Each task's `JoinHandle` is awaited, so a task that
/// panics is answered too: tokio hands the panic to the handle.
pub async fn run<H, F>(
    mut requests: mpsc::Receiver<Request>,
    responses: mpsc::Sender<Response>,
    limit: usize,
    wake: impl Fn() + Clone + Send + 'static,
    handle: H,
) where
    H: Fn(Request) -> F,
    F: Future<Output = Response> + Send + 'static,
{
    let running = Arc::new(Semaphore::new(limit));
    loop {
        let Ok(permit) = running.clone().acquire_owned().await else {
            break;
        };
        let Some(request) = requests.recv().await else {
            break;
        };
        let id = request.id();
        let work = handle(request);
        let task = tokio::spawn(async move {
            let response = work.await;
            drop(permit);
            response
        });
        let (responses, wake) = (responses.clone(), wake.clone());
        tokio::spawn(async move {
            let response = match task.await {
                Ok(response) => response,
                Err(e) => Response::Failed {
                    id,
                    error: e.into(),
                },
            };
            if responses.send(response).await.is_ok() {
                wake();
            }
        });
    }
}
