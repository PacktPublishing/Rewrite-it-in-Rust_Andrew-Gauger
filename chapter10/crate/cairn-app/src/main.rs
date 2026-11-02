use cairn_app::context::AppContext;
use cairn_app::{app::CairnApp, data};

fn main() -> eframe::Result {
    let url = std::env::var("CAIRN_DATABASE_URL")
        .unwrap_or_else(|_| data::DEV_DATABASE_URL.into());
    let snapshot = match data::load(&url) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Couldn't read the database: {e}");
            std::process::exit(1);
        }
    };
    let ctx = AppContext::new(snapshot);
    eframe::run_native(
        "Cairn",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1180.0, 760.0])
                .with_min_inner_size([1024.0, 700.0]),
            ..Default::default()
        },
        Box::new(|_cc| Ok(Box::new(CairnApp::new(ctx)))),
    )
}
