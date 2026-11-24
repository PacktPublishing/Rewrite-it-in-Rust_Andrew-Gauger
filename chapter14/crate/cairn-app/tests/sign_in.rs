//! A login that runs out mid-edit, one test per way it runs out: the
//! draft stays, "sign in again" shows, the sign-in runs again, the pool
//! is rebuilt with the new login, and the save that was refused is sent
//! again and lands. The sign-in is a stand-in for the browser: it issues
//! the login the way `cairn-auth` does (`session::issue`), with a real
//! lifetime a few seconds long. Against the dev database, so ignored by
//! default:
//!
//!   cargo test -p cairn-app --test sign_in -- --ignored --nocapture --test-threads 1

mod common;

use cairn_app::app::CairnApp;
use cairn_app::context::{AppContext, Saved, Target};
use cairn_app::data::{self, Snapshot};
use cairn_app::orchestrator::Orchestrator;
use cairn_app::save;
use cairn_app::sign_in::{Login, Refused, SignIn};
use cairn_app::worker::{self, Workers, CAPACITY};
use cairn_auth::desktop::Expired;
use cairn_auth::session::{self, Identity};
use common::rep::{superuser, Cleanup, Rep, PORT};
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use rust_decimal::Decimal;
use sqlx::postgres::{PgConnectOptions, PgPool};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use uuid::Uuid;

/// The first login's lifetime: long enough to open the form.
const FIRST: f64 = 4.0;

/// A sign-in with no browser: each call issues a new login for the
/// same person, the first short, the rest an hour. A sign-in after the
/// first waits a moment, as a person at a browser would, so the screen
/// can be seen asking.
struct Issued {
    auth: PgPool,
    who: Identity,
    base: PgConnectOptions,
    issued: Mutex<Vec<(Instant, PgConnectOptions)>>,
}

impl SignIn for Issued {
    fn sign_in(&self) -> Login<'_> {
        Box::pin(async move {
            let n = self.issued.lock().unwrap().len();
            if n > 0 {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            let lifetime = if n == 0 { FIRST } else { 3600.0 };
            let c = session::issue(&self.auth, &self.who, lifetime)
                .await
                .map_err(|e| Refused(e.to_string()))?;
            let login = c.connect_options(&self.base);
            self.issued
                .lock()
                .unwrap()
                .push((Instant::now(), login.clone()));
            Ok(login)
        })
    }
}

fn base() -> PgConnectOptions {
    PgConnectOptions::new()
        .host("127.0.0.1")
        .port(PORT)
        .database("cairn")
}

/// The words on screen that contain `part`.
fn text(h: &Harness<'_, CairnApp>, part: &str) -> String {
    let node = h.get_by_label_contains(part);
    let node = node.accesskit_node();
    node.label()
        .or(node.value())
        .unwrap_or_default()
        .to_string()
}

fn price(h: &Harness<'_, CairnApp>) -> String {
    let field =
        h.get_by_role_and_label(Role::TextInput, "Unit Price");
    let node = field.accesskit_node();
    node.value().unwrap_or_default().to_string()
}

/// Sign in, open a product, type a price, let the login run out the
/// way `path` says, then save.
fn runs_out_mid_edit(path: Expired) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let who = Identity {
        oid: Uuid::new_v4(),
        upn: "test@cairn.example".into(),
        display_name: "Test Rep".into(),
        rep_id: Some(1),
        is_admin: false,
    };
    let _cleanup = Cleanup(Rep {
        role: session::role_name(&who.oid),
        password: String::new(),
        oid: who.oid.to_string(),
        rep_id: 1,
    });
    let auth = rt
        .block_on(PgPool::connect(&format!(
            "postgres://cairn_auth:cairn_auth_dev@127.0.0.1:{PORT}/cairn"
        )))
        .unwrap();
    let sign_in = Arc::new(Issued {
        auth,
        who: who.clone(),
        base: base(),
        issued: Mutex::new(Vec::new()),
    });
    let workers = Arc::new(Workers::new(Shared(sign_in.clone())));
    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    rt.spawn(worker::run(
        from_ui,
        to_ui,
        4,
        || {},
        move |r| worker::handle(workers.clone(), r),
    ));
    let mut app = CairnApp::new(AppContext::new(Snapshot::default()))
        .with(Orchestrator::new(requests, responses));
    app.sign_in();
    let mut h = Harness::builder()
        .with_size([1180.0, 760.0])
        .build_ui_state(|ui, app: &mut CairnApp| app.show(ui), app);
    let start = Instant::now();
    let t = || format!("{:5.1} s", start.elapsed().as_secs_f64());
    while sign_in.issued.lock().unwrap().is_empty() {
        assert!(start.elapsed() < Duration::from_secs(10));
        h.step();
        std::thread::sleep(Duration::from_millis(16));
    }
    let first = sign_in.issued.lock().unwrap()[0].clone();
    println!("{} signed in, the login good for {FIRST} s", t());

    // A product of the rep's own, to edit.
    let db =
        rt.block_on(data::connect_with(first.1.clone())).unwrap();
    let mut product =
        common::product(0, "", "Sign-in test sensor", 7900);
    product.id = None;
    product.sku = format!("T-{}", &who.oid.simple().to_string()[..8]);
    let (stored, _) = rt
        .block_on(save::save(&db, Saved::Product(product)))
        .unwrap();
    rt.block_on(db.close());
    h.state_mut().open(Target::Product(Some(stored.key)));
    common::until(&mut h, "Sign-in test sensor");
    h.get_by_role_and_label(Role::TextInput, "Unit Price")
        .focus();
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Unit Price")
        .type_text("85");
    h.run();
    println!("{} Unit Price typed: {}", t(), price(&h));

    let su = rt.block_on(superuser());
    let role = session::role_name(&who.oid);
    if path == Expired::NewConnection {
        // Only the role's VALID UNTIL runs out; the users row doesn't,
        // so the open connection would carry on. The connection drops
        // (as a network blip or the pool's 30-minute recycle would do
        // it), and the pool has to log in again.
        rt.block_on(
            sqlx::query(
                "UPDATE users SET expires_at = now() + interval '1 hour'
                  WHERE role_name = $1",
            )
            .bind(&role)
            .execute(&su),
        )
        .unwrap();
    }
    let ends = first.0 + Duration::from_secs_f64(FIRST + 1.0);
    std::thread::sleep(
        ends.saturating_duration_since(Instant::now()),
    );
    if path == Expired::NewConnection {
        rt.block_on(
            sqlx::query(
                "SELECT pg_terminate_backend(pid) FROM pg_stat_activity
                  WHERE usename = $1",
            )
            .bind(&role)
            .execute(&su),
        )
        .unwrap();
        println!(
            "{} the login has run out; its connection dropped",
            t()
        );
    } else {
        println!(
            "{} the login has run out; its connection is open",
            t()
        );
    }

    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
    common::until(&mut h, "Sign in again");
    let o = h.state().orchestrator.as_ref().unwrap();
    println!(
        "{} Shift+Enter: refused ({:?}); status line: \"{}\"; held: {}",
        t(),
        o.expired_by.unwrap(),
        text(&h, "Sign in again"),
        o.expired()
    );
    let draft = price(&h);
    println!(
        "{} the draft while signing in: Unit Price {draft}",
        t()
    );
    assert_eq!(o.expired_by, Some(path));
    common::until(&mut h, "Saved");
    let o = h.state().orchestrator.as_ref().unwrap();
    println!(
        "{} signed in again ({} sign-ins), save sent again: \"{}\"",
        t(),
        o.sign_ins,
        text(&h, "Saved")
    );
    let (sign_ins, failures) = (o.sign_ins, o.failures().len());

    let saved: Decimal = rt
        .block_on(
            sqlx::query_scalar(
                "SELECT unit_price FROM products WHERE id = $1",
            )
            .bind(stored.key)
            .fetch_one(&su),
        )
        .unwrap();
    println!("{} in Postgres: unit_price {saved}", t());
    drop(h);
    rt.block_on(su.close());

    assert_eq!(draft, "$85.00");
    assert_eq!(sign_ins, 2);
    assert_eq!(failures, 0);
    assert_eq!(saved, Decimal::new(85, 0));
}

/// The test keeps a handle on the sign-in to see what it issued.
struct Shared(Arc<Issued>);

impl SignIn for Shared {
    fn sign_in(&self) -> Login<'_> {
        self.0.sign_in()
    }
}

#[test]
#[ignore = "writes to the dev database"]
fn a_new_connection_after_valid_until_signs_in_again_and_saves() {
    runs_out_mid_edit(Expired::NewConnection);
}

#[test]
#[ignore = "writes to the dev database"]
fn an_open_connection_after_expires_at_signs_in_again_and_saves() {
    runs_out_mid_edit(Expired::OpenConnection);
}
