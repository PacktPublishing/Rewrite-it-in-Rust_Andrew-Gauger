//! Sending a custom order to the manufacturer, and keeping a row for
//! every attempt in `manufacturer_orders`.
//!
//! The call is the one write in the app that can't be put back. An
//! answer that never arrives doesn't say whether the order exists, so
//! it is its own outcome, [`Sent::Unconfirmed`], and the rep decides
//! whether to retry. A retry carries the same correlation ID, and the
//! manufacturer refuses a repeat with `409` and the number it holds.

use cairn_core::manufacturer::ManufacturerOrder;
use cairn_core::OrderId;
use reqwest::StatusCode;
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

/// What one attempt to send an order came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sent {
    /// The manufacturer holds the order. `repeat`: an earlier
    /// attempt placed it, and this one got its number back (409).
    Confirmed { mfr_order_id: String, repeat: bool },
    /// No answer: a timeout, or the connection closed after the order
    /// may have been written. The order may or may not exist.
    Unconfirmed { reason: String },
    /// An answer that created nothing (a 500, a 400), or a connection
    /// that never opened.
    Failed { reason: String },
}

/// How long the app waits for the manufacturer's answer. Past it, the
/// order is in doubt and the rep decides.
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// The status and body, or why there were none.
pub type Answer = Result<(StatusCode, String), reqwest::Error>;

/// What the 201 and the 409 both carry.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Number {
    mfr_order_id: String,
}

/// One order the manufacturer holds, as `GET /orders` returns it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Held {
    pub mfr_order_id: String,
    pub received_at: String,
    pub correlation_id: Option<Uuid>,
    pub dealer_po: Option<String>,
    pub customer: Option<String>,
    pub lines: Vec<HeldLine>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HeldLine {
    pub sku: String,
    pub qty: i64,
}

pub struct Manufacturer {
    client: reqwest::Client,
    /// Company Info's `manufacturer_api_url`, the `/orders` endpoint.
    orders_url: String,
}

impl Manufacturer {
    pub fn new(
        orders_url: &str,
        timeout: Duration,
    ) -> reqwest::Result<Manufacturer> {
        let client = reqwest::Client::builder()
            // reqwest has no default timeout: no answer, no outcome.
            .timeout(timeout)
            // Never resend an order. The rep decides, with one ID.
            .retry(reqwest::retry::never())
            // No reuse: V1's silent resend ordered twice.
            .pool_max_idle_per_host(0)
            .build()?;
        Ok(Manufacturer {
            client,
            orders_url: orders_url.to_string(),
        })
    }

    /// `POST /orders`, once.
    pub async fn post(&self, order: &ManufacturerOrder) -> Answer {
        let resp = self
            .client
            .post(&self.orders_url)
            .json(order)
            .send()
            .await?;
        let status = resp.status();
        Ok((status, resp.text().await?))
    }

    /// `GET /orders?correlationId=`: what the manufacturer holds for
    /// one ID. For the admin's `mfr-status`, not the rep's screen.
    pub async fn lookup(
        &self,
        correlation_id: Uuid,
    ) -> reqwest::Result<Vec<Held>> {
        self.client
            .get(&self.orders_url)
            .query(&[("correlationId", correlation_id.to_string())])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }
}

/// The manufacturer's answers, and the one that isn't an answer.
pub fn outcome(answer: &Answer) -> Sent {
    match answer {
        Ok((StatusCode::CREATED, body)) => confirmed(body, false),
        Ok((StatusCode::CONFLICT, body)) => confirmed(body, true),
        // Nothing was written: the connection never opened.
        Err(e) if e.is_connect() => Sent::Failed { reason: why(e) },
        Err(e) => Sent::Unconfirmed { reason: why(e) },
        Ok((status, body)) => Sent::Failed {
            reason: format!("{status}: {body}"),
        },
    }
}

/// A 201 or 409 without a number says the order exists but not which
/// one, so it isn't confirmed.
fn confirmed(body: &str, repeat: bool) -> Sent {
    match serde_json::from_str::<Number>(body) {
        Ok(n) => Sent::Confirmed {
            mfr_order_id: n.mfr_order_id,
            repeat,
        },
        Err(e) => Sent::Unconfirmed {
            reason: format!("no mfrOrderId in the answer: {e}"),
        },
    }
}

/// The error and its causes: reqwest's own text names only the URL.
fn why(e: &reqwest::Error) -> String {
    let mut text = e.to_string();
    let mut cause = std::error::Error::source(e);
    while let Some(c) = cause {
        text = format!("{text}: {c}");
        cause = c.source();
    }
    text
}

/// One row of `manufacturer_orders`.
#[derive(Debug, Clone)]
pub struct AttemptRow {
    pub id: i32,
    pub order_id: i32,
    pub correlation_id: Uuid,
    pub attempt: i32,
    pub sent_at: String,
    pub http_status: Option<i32>,
    pub error_text: Option<String>,
    pub manufacturer_ref: Option<String>,
}

/// The row an attempt is recorded in, written before the request goes
/// out, so an attempt that never finishes is still on record.
#[derive(Debug, Clone, Copy)]
pub struct Begun {
    pub id: i32,
    pub attempt: i32,
}

/// Inserts the next attempt for this correlation ID (1 for a new
/// order), before sending. With no rep given, the signed-in rep's.
pub async fn begin_attempt(
    pool: &PgPool,
    order_id: i32,
    sent_by_rep_id: Option<i32>,
    order: &ManufacturerOrder,
) -> sqlx::Result<Begun> {
    sqlx::query_as!(
        Begun,
        r#"INSERT INTO manufacturer_orders
             (order_id, correlation_id, attempt, sent_by_rep_id,
              request_body)
           SELECT $1, $2, COALESCE(MAX(attempt), 0) + 1,
                  COALESCE($3, (SELECT rep_id FROM users
                                 WHERE role_name = session_user)),
                  $4
             FROM manufacturer_orders
            WHERE correlation_id = $2
           RETURNING id, attempt"#,
        order_id,
        order.correlation_id(),
        sent_by_rep_id,
        order.to_json(),
    )
    .fetch_one(pool)
    .await
}

/// Writes what the attempt came to onto its row.
pub async fn finish_attempt(
    pool: &PgPool,
    begun: Begun,
    answer: &Answer,
    sent: &Sent,
) -> sqlx::Result<()> {
    let (status, body, error) = match answer {
        Ok((s, b)) => {
            (Some(i32::from(s.as_u16())), Some(b.as_str()), None)
        }
        Err(e) => (None, None, Some(why(e))),
    };
    let reference = match sent {
        Sent::Confirmed { mfr_order_id, .. } => {
            Some(mfr_order_id.as_str())
        }
        Sent::Unconfirmed { .. } | Sent::Failed { .. } => None,
    };
    // error_text is varchar(255).
    let error: Option<String> =
        error.map(|e| e.chars().take(255).collect());
    sqlx::query!(
        "UPDATE manufacturer_orders
            SET http_status = $2, response_body = $3, error_text = $4,
                manufacturer_ref = $5, version = version + 1
          WHERE id = $1",
        begun.id,
        status,
        body,
        error,
        reference,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// One attempt, start to finish: the row, the call, the row's answer.
/// The worker's `SendToManufacturer` calls this, and a Retry calls it
/// again with `order.retry()`.
pub async fn send(
    pool: &PgPool,
    mfr: &Manufacturer,
    order_id: i32,
    sent_by_rep_id: Option<i32>,
    order: &ManufacturerOrder,
) -> sqlx::Result<Sent> {
    let begun =
        begin_attempt(pool, order_id, sent_by_rep_id, order).await?;
    let answer = mfr.post(order).await;
    let sent = outcome(&answer);
    finish_attempt(pool, begun, &answer, &sent).await?;
    Ok(sent)
}

/// Every attempt for one correlation ID, in order.
pub async fn attempts(
    pool: &PgPool,
    correlation_id: Uuid,
) -> sqlx::Result<Vec<AttemptRow>> {
    sqlx::query_as!(
        AttemptRow,
        r#"SELECT id, order_id, correlation_id, attempt,
                  to_char(sent_at AT TIME ZONE 'UTC',
                          'YYYY-MM-DD HH24:MI:SS') AS "sent_at!",
                  http_status, error_text, manufacturer_ref
             FROM manufacturer_orders
            WHERE correlation_id = $1
            ORDER BY attempt"#,
        correlation_id,
    )
    .fetch_all(pool)
    .await
}

/// Whether an attempt leaves its order unconfirmed: it got no
/// answer at all (a timeout, a dropped connection, an attempt still
/// out), so it says nothing either way. Any answer is on its row as a
/// status: a 500 or a 400 created nothing, and a 201 or 409 carries
/// the number.
pub fn in_doubt(
    http_status: Option<i32>,
    manufacturer_ref: Option<&str>,
) -> bool {
    http_status.is_none() && manufacturer_ref.is_none()
}

/// True when the latest attempt for this ID is [`in_doubt`]. Cancel
/// leaves an order this way. Never sent is false.
pub async fn is_unconfirmed(
    pool: &PgPool,
    correlation_id: Uuid,
) -> sqlx::Result<bool> {
    let latest = sqlx::query!(
        "SELECT http_status, manufacturer_ref FROM manufacturer_orders
          WHERE correlation_id = $1
          ORDER BY attempt DESC LIMIT 1",
        correlation_id,
    )
    .fetch_optional(pool)
    .await?;
    Ok(latest.is_some_and(|r| {
        in_doubt(r.http_status, r.manufacturer_ref.as_deref())
    }))
}

/// Every sales order (or the one, `order_id`) whose latest attempt is in
/// doubt, with the request as it was sent: what a Retry sends again,
/// with the correlation ID the manufacturer may already hold.
pub async fn unconfirmed(
    pool: &PgPool,
    order_id: Option<i32>,
) -> sqlx::Result<HashMap<OrderId, ManufacturerOrder>> {
    let latest = sqlx::query!(
        r#"SELECT DISTINCT ON (order_id)
                  order_id, request_body, http_status, manufacturer_ref
             FROM manufacturer_orders
            WHERE $1::int4 IS NULL OR order_id = $1
            ORDER BY order_id, sent_at DESC, attempt DESC"#,
        order_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(latest
        .into_iter()
        .filter(|r| {
            in_doubt(r.http_status, r.manufacturer_ref.as_deref())
        })
        .filter_map(|r| {
            let order =
                serde_json::from_str(&r.request_body?).ok()?;
            Some((OrderId(r.order_id), order))
        })
        .collect())
}

/// The sales order's unconfirmed order, if it has one: the one a Retry
/// must send again.
pub async fn unconfirmed_for_order(
    pool: &PgPool,
    order_id: i32,
) -> sqlx::Result<Option<ManufacturerOrder>> {
    let mut one = unconfirmed(pool, Some(order_id)).await?;
    Ok(one.remove(&OrderId(order_id)))
}
