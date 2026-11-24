use cairn_app::app::CairnApp;
use cairn_app::context::AppContext;
use cairn_app::data::{self, Rows, Snapshot};
use cairn_app::manufacturer_orchestrator::{
    ManufacturerOrchestrator, LIMIT as MANUFACTURER_LIMIT,
};
use cairn_app::orchestrator::Orchestrator;
use cairn_app::report_orchestrator::{
    ReportOrchestrator, LIMIT as REPORT_LIMIT,
};
use cairn_app::reports::{Renderer, System};
use cairn_app::sign_in::{Entra, Fixed};
use cairn_app::worker::{self, Workers, CAPACITY};
use sqlx::postgres::PgConnectOptions;
use std::sync::Arc;
use tokio::sync::mpsc;

/// With `CAIRN_AUTH_URL` (`cairn-auth`'s address), the rep signs in
/// through Entra, and `CAIRN_DATABASE_URL` names only the server and
/// database. Without it, `CAIRN_DATABASE_URL` is the login itself: the
/// dev database as `cairn_owner` by default, for a machine with no
/// Entra tenant. `CAIRN_MANUFACTURER_URL` stands in for Company Info's
/// `manufacturer_api_url`, for the mock.
fn workers() -> Result<Workers, sqlx::Error> {
    let url = std::env::var("CAIRN_DATABASE_URL")
        .unwrap_or_else(|_| data::DEV_DATABASE_URL.into());
    let database: PgConnectOptions = url.parse()?;
    let workers = match std::env::var("CAIRN_AUTH_URL") {
        Ok(auth) => Workers::new(Entra { auth, database }),
        Err(_) => Workers::new(Fixed(database)),
    };
    // Templates beside the app by default, so an analyst can edit
    // them. `CAIRN_WEASYPRINT` names the program if it is not on PATH.
    let templates =
        std::env::var("CAIRN_TEMPLATES").unwrap_or_else(|_| {
            // Beside the app if it is there, else the source tree's.
            let here = std::path::Path::new("templates");
            if here.is_dir() {
                "templates".into()
            } else {
                concat!(env!("CARGO_MANIFEST_DIR"), "/templates")
                    .into()
            }
        });
    let out = std::env::temp_dir().join("cairn-reports");
    let mut renderer = Renderer::new(templates, out);
    if let Ok(program) = std::env::var("CAIRN_WEASYPRINT") {
        renderer = renderer.with_weasyprint(program);
    }
    let workers = workers.with_renderer(renderer);
    Ok(match std::env::var("CAIRN_MANUFACTURER_URL") {
        Ok(mfr) => workers.with_manufacturer(&mfr),
        Err(_) => workers,
    })
}

fn main() -> eframe::Result {
    // The log sits next to the app. The guard lives as long as `main`.
    let _log = cairn_app::logging::init(std::path::Path::new("logs"));
    let workers = match workers() {
        Ok(w) => Arc::new(w),
        Err(e) => {
            eprintln!("CAIRN_DATABASE_URL: {e}");
            std::process::exit(1);
        }
    };
    // The workers' runtime, on threads of its own. It lives as long as
    // `main`: dropped, it would cancel every request in flight.
    let rt = tokio::runtime::Runtime::new().expect("a tokio runtime");
    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    // The manufacturer's own channels, worker and limit.
    let (mfr_requests, mfr_from_ui) = mpsc::channel(CAPACITY);
    let (mfr_to_ui, mfr_responses) = mpsc::channel(CAPACITY);
    // And the reports': a channel, a worker and a limit of their own.
    let (rpt_requests, rpt_from_ui) = mpsc::channel(CAPACITY);
    let (rpt_to_ui, rpt_responses) = mpsc::channel(CAPACITY);
    let runtime = rt.handle().clone();
    eframe::run_native(
        "Cairn",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1180.0, 760.0])
                .with_min_inner_size([1024.0, 700.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            // A worker that answers wakes the UI thread, so the answer
            // is drawn even if nobody moves the mouse.
            let egui = cc.egui_ctx.clone();
            let (records_workers, mfr_workers, rpt_workers) =
                (workers.clone(), workers.clone(), workers);
            let wake = egui.clone();
            let rpt_wake = egui.clone();
            runtime.spawn(worker::run(
                from_ui,
                to_ui,
                data::POOL_SIZE as usize,
                move || egui.request_repaint(),
                move |request| {
                    worker::handle(records_workers.clone(), request)
                },
            ));
            runtime.spawn(worker::run(
                mfr_from_ui,
                mfr_to_ui,
                MANUFACTURER_LIMIT,
                move || wake.request_repaint(),
                move |request| {
                    worker::handle(mfr_workers.clone(), request)
                },
            ));
            runtime.spawn(worker::run(
                rpt_from_ui,
                rpt_to_ui,
                REPORT_LIMIT,
                move || rpt_wake.request_repaint(),
                move |request| {
                    worker::handle(rpt_workers.clone(), request)
                },
            ));
            let ctx = AppContext::new(Snapshot::default());
            let mut app = CairnApp::new(ctx)
                .with(Orchestrator::new(requests, responses))
                .with_manufacturer(ManufacturerOrchestrator::new(
                    mfr_requests,
                    mfr_responses,
                ))
                .with_reports(
                    ReportOrchestrator::new(
                        rpt_requests,
                        rpt_responses,
                    ),
                    System,
                );
            // The sign-in first: the read waits for its login.
            app.sign_in();
            app.read(Rows::All);
            Ok(Box::new(app))
        }),
    )
}
