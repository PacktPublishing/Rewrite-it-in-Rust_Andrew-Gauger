//! The three routes. `cairn-auth` keeps nothing but sign-ins in progress,
//! in memory, for a few minutes at most. A restart signs nobody out.

use crate::entra::{Entra, Outbound};
use crate::session::{self, Credential, Identity};
use crate::token::{random_token, verifies};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a browser has to come back from Entra.
const SIGN_IN_TTL: Duration = Duration::from_secs(600);
/// How long the app has to trade its one-time code.
const CODE_TTL: Duration = Duration::from_secs(60);

pub struct AppState {
    pub db: PgPool,
    pub entra: Entra,
    pub lifetime_secs: f64,
    signing_in: Mutex<HashMap<String, SigningIn>>,
    codes: Mutex<HashMap<String, Issued>>,
}

struct SigningIn {
    outbound: Outbound,
    app_port: u16,
    app_challenge: String,
    started: Instant,
}

struct Issued {
    identity: Identity,
    app_challenge: String,
    issued: Instant,
}

pub fn router(
    db: PgPool,
    entra: Entra,
    lifetime_secs: f64,
) -> Router {
    let state = Arc::new(AppState {
        db,
        entra,
        lifetime_secs,
        signing_in: Mutex::default(),
        codes: Mutex::default(),
    });
    Router::new()
        .route("/auth/start", get(start))
        .route("/auth/callback", get(callback))
        .route("/session", post(exchange))
        .with_state(state)
}

#[derive(Deserialize)]
struct Start {
    port: u16,
    challenge: String,
}

/// Step 1: the app has opened and sent the browser here.
async fn start(
    State(s): State<Arc<AppState>>,
    Query(q): Query<Start>,
) -> Response {
    if q.port < 1024 || q.challenge.len() != 43 {
        return refuse(
            StatusCode::BAD_REQUEST,
            "Bad sign-in request.",
        );
    }
    let (url, state, outbound) = s.entra.start();
    let mut signing_in = s.signing_in.lock().unwrap();
    signing_in.retain(|_, v| v.started.elapsed() < SIGN_IN_TTL);
    signing_in.insert(
        state.secret().clone(),
        SigningIn {
            outbound,
            app_port: q.port,
            app_challenge: q.challenge,
            started: Instant::now(),
        },
    );
    Redirect::to(url.as_str()).into_response()
}

#[derive(Deserialize)]
struct Callback {
    code: Option<String>,
    state: Option<String>,
    error_description: Option<String>,
}

/// Step 2: Entra sends the browser back. Nothing changes in Postgres
/// here; the app gets a one-time code on its loopback port.
async fn callback(
    State(s): State<Arc<AppState>>,
    Query(q): Query<Callback>,
) -> Response {
    if let Some(e) = q.error_description {
        return refuse(StatusCode::UNAUTHORIZED, &e);
    }
    let (Some(code), Some(state)) = (q.code, q.state) else {
        return refuse(StatusCode::BAD_REQUEST, "Bad callback.");
    };
    let Some(pending) = s.signing_in.lock().unwrap().remove(&state)
    else {
        return refuse(
            StatusCode::BAD_REQUEST,
            "Unknown or expired sign-in.",
        );
    };
    if pending.started.elapsed() > SIGN_IN_TTL {
        return refuse(
            StatusCode::BAD_REQUEST,
            "Sign-in took too long.",
        );
    }
    let who = match s.entra.finish(code, pending.outbound).await {
        Ok(who) => who,
        Err(e) => {
            tracing::warn!("Entra sign-in failed: {e}");
            return refuse(
                StatusCode::UNAUTHORIZED,
                "Entra sign-in failed.",
            );
        }
    };
    let admitted = session::admit(
        &s.db,
        who.oid,
        &who.upn,
        &who.display_name,
        &who.roles,
    )
    .await;
    let identity = match admitted {
        Ok(Some(identity)) => identity,
        Ok(None) => {
            tracing::info!("{} isn't set up for Cairn", who.upn);
            return refuse(
                StatusCode::FORBIDDEN,
                &format!("{} isn't set up for Cairn.", who.upn),
            );
        }
        Err(e) => return internal(e),
    };
    tracing::info!("{} signed in with Entra", identity.upn);
    let one_time = random_token();
    let mut codes = s.codes.lock().unwrap();
    codes.retain(|_, v| v.issued.elapsed() < CODE_TTL);
    codes.insert(
        one_time.clone(),
        Issued {
            identity,
            app_challenge: pending.app_challenge,
            issued: Instant::now(),
        },
    );
    let to = format!(
        "http://127.0.0.1:{}/callback?code={one_time}",
        pending.app_port
    );
    Redirect::to(&to).into_response()
}

#[derive(Deserialize)]
struct Exchange {
    code: String,
    verifier: String,
}

/// Step 3: the app trades the code and its verifier for a credential.
/// This is the only place a password is set.
async fn exchange(
    State(s): State<Arc<AppState>>,
    Json(q): Json<Exchange>,
) -> Response {
    let issued = s.codes.lock().unwrap().remove(&q.code);
    let Some(issued) = issued else {
        return refuse(StatusCode::UNAUTHORIZED, "Unknown code.");
    };
    if issued.issued.elapsed() > CODE_TTL
        || !verifies(&q.verifier, &issued.app_challenge)
    {
        return refuse(StatusCode::UNAUTHORIZED, "Code refused.");
    }
    let issue =
        session::issue(&s.db, &issued.identity, s.lifetime_secs);
    let credential: Credential = match issue.await {
        Ok(credential) => credential,
        Err(e) => return internal(e),
    };
    tracing::info!(
        "{} has a login until {}",
        credential.user,
        credential.expires_at
    );
    Json(credential).into_response()
}

fn refuse(status: StatusCode, why: &str) -> Response {
    let why = why.replace('<', "&lt;");
    (status, Html(format!("<p>Cairn sign-in: {why}</p>")))
        .into_response()
}

fn internal(e: sqlx::Error) -> Response {
    tracing::error!("database: {e}");
    refuse(StatusCode::INTERNAL_SERVER_ERROR, "Database error.")
}
