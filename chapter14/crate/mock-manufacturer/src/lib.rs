//! The manufacturer's order API after Chapter 11's negotiation. The
//! same modes, ledger and endpoints as V1's PowerShell mock, plus one
//! rule: an order whose `correlationId` it already holds is refused
//! with `409 Conflict` and the number it holds. An order with no
//! `correlationId` (every V1 call) is accepted every time.

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::header::{CONNECTION, CONTENT_TYPE};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Create the order and answer 201.
    Normal,
    /// Create the order, then close the connection without answering.
    Drop,
    /// Wait, then create and answer.
    Slow,
    /// Answer 500 and create nothing.
    Error,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Mode> {
        match s {
            "normal" => Some(Mode::Normal),
            "drop" => Some(Mode::Drop),
            "slow" => Some(Mode::Slow),
            "error" => Some(Mode::Error),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Mode::Normal => "normal",
            Mode::Drop => "drop",
            Mode::Slow => "slow",
            Mode::Error => "error",
        }
    }
}

pub struct Settings {
    pub mode: Mode,
    /// Go back to normal after this many orders (0 = stay).
    pub times: u32,
    pub delay: Duration,
    pub ledger: PathBuf,
    pub first_order_number: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Order {
    mfr_order_id: String,
    received_at: String,
    #[serde(default)]
    correlation_id: Option<Uuid>,
    dealer_po: Option<String>,
    customer: Option<String>,
    lines: Vec<Line>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Line {
    sku: String,
    qty: i64,
}

/// What the ledger file holds: the order and the request, byte for
/// byte, so a double order can be compared with the first.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    #[serde(flatten)]
    order: Order,
    raw_request: String,
}

/// The fields the manufacturer reads. Anything else is kept only in
/// the raw request.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Placed {
    correlation_id: Option<Uuid>,
    po: Option<String>,
    ship_to: Option<ShipTo>,
    #[serde(default)]
    lines: Vec<Line>,
}

#[derive(Deserialize)]
struct ShipTo {
    name: Option<String>,
}

struct Ledger {
    mode: Mode,
    left: u32,
    next: u32,
    entries: Vec<Entry>,
    path: PathBuf,
}

impl Ledger {
    fn open(s: &Settings) -> std::io::Result<Ledger> {
        let entries: Vec<Entry> = match std::fs::read(&s.ledger) {
            Ok(b) => serde_json::from_slice(&b)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Vec::new()
            }
            Err(e) => return Err(e),
        };
        let held = entries
            .iter()
            .filter_map(|e| number(&e.order.mfr_order_id))
            .max();
        let next = held.map_or(s.first_order_number, |n| {
            (n + 1).max(s.first_order_number)
        });
        Ok(Ledger {
            mode: s.mode,
            left: s.times,
            next,
            entries,
            path: s.ledger.clone(),
        })
    }

    fn holding(&self, id: Option<Uuid>) -> Option<&Order> {
        let id = id?;
        self.entries
            .iter()
            .map(|e| &e.order)
            .find(|o| o.correlation_id == Some(id))
    }

    /// The mode for this order, counting it against `--times`.
    fn take_mode(&mut self) -> Mode {
        let mode = self.mode;
        if self.left > 0 {
            self.left -= 1;
            if self.left == 0 {
                self.mode = Mode::Normal;
            }
        }
        mode
    }

    fn record(
        &mut self,
        placed: Placed,
        raw: &[u8],
    ) -> std::io::Result<Order> {
        let order = Order {
            mfr_order_id: format!("MFR-{}", self.next),
            received_at: chrono::Local::now()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
            correlation_id: placed.correlation_id,
            dealer_po: placed.po,
            customer: placed.ship_to.and_then(|s| s.name),
            lines: placed.lines,
        };
        self.next += 1;
        self.entries.push(Entry {
            order: order.clone(),
            raw_request: String::from_utf8_lossy(raw).into_owned(),
        });
        let json = serde_json::to_string_pretty(&self.entries)?;
        std::fs::write(&self.path, json)?;
        Ok(order)
    }
}

fn number(mfr_order_id: &str) -> Option<u32> {
    mfr_order_id.strip_prefix("MFR-")?.parse().ok()
}

struct Mock {
    ledger: Mutex<Ledger>,
    delay: Duration,
    stopping: AtomicBool,
    stopped: Notify,
}

impl Mock {
    fn lock(&self) -> MutexGuard<'_, Ledger> {
        self.ledger.lock().expect("no handler panics holding it")
    }
}

/// Returned instead of a response to make hyper close the connection
/// without writing anything.
#[derive(Debug)]
struct HangUp;

impl fmt::Display for HangUp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("hung up without answering")
    }
}

impl std::error::Error for HangUp {}

type Reply = Response<Full<Bytes>>;

/// Serves until `POST /control?mode=stop`.
pub async fn serve(
    listener: TcpListener,
    settings: Settings,
) -> std::io::Result<()> {
    let mock = Arc::new(Mock {
        ledger: Mutex::new(Ledger::open(&settings)?),
        delay: settings.delay,
        stopping: AtomicBool::new(false),
        stopped: Notify::new(),
    });
    loop {
        let (stream, _) = tokio::select! {
            accepted = listener.accept() => accepted?,
            _ = mock.stopped.notified() => return Ok(()),
        };
        let mock = mock.clone();
        tokio::spawn(async move {
            let svc = service_fn(|req| handle(mock.clone(), req));
            // The error is the hang-up, or the client going away.
            let _ = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), svc)
                .await;
            if mock.stopping.load(Ordering::SeqCst) {
                mock.stopped.notify_one();
            }
        });
    }
}

async fn handle(
    mock: Arc<Mock>,
    req: Request<Incoming>,
) -> Result<Reply, HangUp> {
    let method = req.method().clone();
    let path = req.uri().path().trim_end_matches('/').to_string();
    let query = req.uri().query().unwrap_or_default().to_string();
    let body = req
        .into_body()
        .collect()
        .await
        .map_err(|_| HangUp)?
        .to_bytes();
    Ok(match (method, path.as_str()) {
        (Method::POST, "/orders") => return place(&mock, body).await,
        (Method::GET, "/orders") => list(&mock, &query),
        (Method::POST, "/control") => control(&mock, &query),
        (_, "/health") => {
            let l = mock.lock();
            reply(
                StatusCode::OK,
                json!({
                    "ok": true,
                    "mode": l.mode.name(),
                    "orders": l.entries.len(),
                }),
            )
        }
        _ => reply(
            StatusCode::NOT_FOUND,
            json!({"error": "not found"}),
        ),
    })
}

async fn place(mock: &Mock, body: Bytes) -> Result<Reply, HangUp> {
    let placed: Placed = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            log("400 malformed JSON");
            return Ok(reply(
                StatusCode::BAD_REQUEST,
                json!({"error": "malformed JSON", "detail": e.to_string()}),
            ));
        }
    };
    let po = placed.po.clone().unwrap_or_default();
    // The repeat check comes before the mode: the order exists.
    let mode = {
        let mut l = mock.lock();
        if let Some(held) = l.holding(placed.correlation_id) {
            return Ok(refuse(held, &po));
        }
        l.take_mode()
    };
    match mode {
        Mode::Error => {
            log(format!("500 (mode error) {po}"));
            return Ok(reply(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"error": "internal error"}),
            ));
        }
        Mode::Slow => {
            log(format!(
                "slow: waiting {} s before answering {po}",
                mock.delay.as_secs()
            ));
            tokio::time::sleep(mock.delay).await;
        }
        Mode::Normal | Mode::Drop => {}
    }
    let mut l = mock.lock();
    // A repeat can land while a slow order waits.
    if let Some(held) = l.holding(placed.correlation_id) {
        return Ok(refuse(held, &po));
    }
    let id = placed.correlation_id;
    let order = match l.record(placed, &body) {
        Ok(o) => o,
        Err(e) => {
            log(format!("error: {e}"));
            return Ok(reply(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"error": e.to_string()}),
            ));
        }
    };
    let tag = id.map(|id| format!(" ({id})")).unwrap_or_default();
    if mode == Mode::Drop {
        log(format!(
            "created {} for {po}{tag}, then DROPPED the connection",
            order.mfr_order_id
        ));
        return Err(HangUp);
    }
    let same_po = l
        .entries
        .iter()
        .filter(|e| e.order.dealer_po.as_deref() == Some(&po))
        .count();
    let note = if same_po > 1 {
        format!("  ({same_po} orders for this PO)")
    } else {
        String::new()
    };
    log(format!("201 {} for {po}{tag}{note}", order.mfr_order_id));
    Ok(reply(
        StatusCode::CREATED,
        json!({"mfrOrderId": order.mfr_order_id, "status": "accepted"}),
    ))
}

fn refuse(held: &Order, po: &str) -> Reply {
    let id = held.correlation_id.expect("held by its correlationId");
    log(format!(
        "409 {po} ({id}) is already {}, created nothing",
        held.mfr_order_id
    ));
    reply(
        StatusCode::CONFLICT,
        json!({
            "error": "duplicate correlationId",
            "mfrOrderId": held.mfr_order_id,
        }),
    )
}

fn list(mock: &Mock, query: &str) -> Reply {
    let l = mock.lock();
    let all = l.entries.iter().map(|e| &e.order);
    let Some(wanted) = param(query, "correlationId") else {
        return reply(StatusCode::OK, all.collect::<Vec<_>>());
    };
    let Ok(wanted) = Uuid::parse_str(wanted) else {
        return reply(
            StatusCode::BAD_REQUEST,
            json!({"error": format!("bad correlationId '{wanted}'")}),
        );
    };
    let found: Vec<_> =
        all.filter(|o| o.correlation_id == Some(wanted)).collect();
    log(format!("lookup {wanted}: {} order(s)", found.len()));
    reply(StatusCode::OK, found)
}

fn control(mock: &Mock, query: &str) -> Reply {
    let m = param(query, "mode").unwrap_or_default();
    if m == "stop" {
        mock.stopping.store(true, Ordering::SeqCst);
        let mut r = reply(
            StatusCode::OK,
            json!({"ok": true, "stopping": true}),
        );
        // Close after answering, so the server can stop.
        r.headers_mut()
            .insert(CONNECTION, "close".parse().expect("valid"));
        return r;
    }
    let Some(mode) = Mode::parse(m) else {
        return reply(
            StatusCode::BAD_REQUEST,
            json!({"error": format!("unknown mode '{m}'")}),
        );
    };
    let times = param(query, "times")
        .and_then(|t| t.parse().ok())
        .unwrap_or(0);
    let mut l = mock.lock();
    l.mode = mode;
    l.left = times;
    log(format!("mode -> {}{}", mode.name(), for_times(times)));
    reply(
        StatusCode::OK,
        json!({"ok": true, "mode": mode.name(), "times": times}),
    )
}

pub fn for_times(times: u32) -> String {
    if times > 0 {
        format!(" for {times} order(s)")
    } else {
        String::new()
    }
}

fn param<'q>(query: &'q str, key: &str) -> Option<&'q str> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then_some(v)
    })
}

fn reply(status: StatusCode, body: impl Serialize) -> Reply {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(
            serde_json::to_string(&body).expect("plain JSON data"),
        )))
        .expect("a status and a valid header")
}

pub fn log(msg: impl fmt::Display) {
    println!("{} {msg}", chrono::Local::now().format("%H:%M:%S"));
}
