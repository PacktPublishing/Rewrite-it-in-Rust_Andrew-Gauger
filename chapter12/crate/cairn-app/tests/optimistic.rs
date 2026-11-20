//! A save shown before Postgres has answered (Chapter 12): the list
//! takes the edited row at once, marked, and puts it back if the save
//! fails. The form keeps what the rep typed. Over a fake worker, no
//! database.
//!
//!   cargo test -p cairn-app --test optimistic -- --nocapture

mod common;

use cairn_app::app::CairnApp;
use cairn_app::context::{Saved, Target};
use cairn_app::data::Table;
use cairn_app::optimistic::Shown;
use cairn_app::orchestrator::Orchestrator;
use cairn_app::save::Stored;
use cairn_app::screens::sheet::SheetOf;
use cairn_app::worker::{
    Failure, Request, RequestId, Response, CAPACITY,
};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use rust_decimal::Decimal;
use tokio::sync::mpsc::{self, Receiver, Sender};

struct Fake {
    requests: Receiver<Request>,
    responses: Sender<Response>,
}

impl Fake {
    fn take(&mut self) -> Vec<Request> {
        std::iter::from_fn(|| self.requests.try_recv().ok()).collect()
    }
}

fn price(h: &Harness<'_, CairnApp>) -> Decimal {
    let d = &h.state().ctx.data;
    d.products
        .iter()
        .find(|p| p.sku == "CS-TH050")
        .unwrap()
        .unit_price
}

/// The Products form on the sensor, $79.00, with $85 typed in and Save
/// pressed. The save is in flight; nothing has answered.
fn saving() -> (Harness<'static, CairnApp>, Fake, RequestId) {
    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    let mut app = CairnApp::new(common::context())
        .with(Orchestrator::new(requests, responses));
    app.open(Target::Product(Some(11)));
    let mut h = Harness::builder()
        .with_size([1400.0, 300.0])
        .with_pixels_per_point(2.0)
        .with_theme(egui::Theme::Light)
        .build_ui_state(|ui, app: &mut CairnApp| app.show(ui), app);
    h.run_steps(4);
    h.get_by_role_and_label(Role::TextInput, "Unit Price")
        .focus();
    h.run_steps(4);
    h.get_by_role_and_label(Role::TextInput, "Unit Price")
        .type_text("85");
    h.run_steps(4);
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
    h.run_steps(4);
    let mut fake = Fake {
        requests: from_ui,
        responses: to_ui,
    };
    let id = fake
        .take()
        .into_iter()
        .find_map(|r| match r {
            Request::Save { id, .. } => Some(id),
            _ => None,
        })
        .expect("a save");
    h.state_mut().settle();
    h.run_steps(2);
    (h, fake, id)
}

#[test]
fn the_list_shows_a_save_at_once_and_marks_it() {
    let (mut h, _worker, _) = saving();
    // Postgres hasn't answered. The snapshot the lists are built from
    // already has the new price.
    assert_eq!(price(&h), Decimal::new(8500, 2));
    let pending = h
        .state()
        .orchestrator
        .as_ref()
        .unwrap()
        .pending_keys(Table::Products);
    assert_eq!(pending, vec![11]);

    // A datasheet opened now shows the row, with a dot.
    h.state_mut().open(Target::Sheet(SheetOf::Products));
    h.run_steps(4);
    assert!(h.query_by_label_contains("• CS-TH050").is_some());
    assert!(h.query_by_label_contains("$85.00").is_some());
    println!("pending: • CS-TH050 at $85.00, request out");
    h.snapshot("optimistic-row-pending");
}

#[test]
fn a_failed_save_puts_the_row_back_and_keeps_the_draft() {
    let (mut h, worker, id) = saving();
    assert_eq!(price(&h), Decimal::new(8500, 2));
    worker
        .responses
        .try_send(Response::Failed {
            id,
            error: Failure::Database(sqlx::Error::PoolTimedOut),
        })
        .unwrap();
    h.run_steps(4);
    h.state_mut().settle();
    h.run_steps(4);

    // The list is true again.
    assert_eq!(price(&h), Decimal::new(7900, 2));
    // The form still holds what the rep typed, and says it isn't saved.
    assert!(h.query_by_label_contains("Couldn't save").is_some());
    assert!(h.query_by_label("(unsaved)").is_some());
    let field =
        h.get_by_role_and_label(Role::TextInput, "Unit Price");
    assert_eq!(field.value().as_deref(), Some("$85.00"));
    println!("rolled back: list $79.00, form still $85.00, unsaved");
}

#[test]
fn a_rolled_back_datasheet_loses_its_mark() {
    let (mut h, worker, id) = saving();
    h.state_mut().open(Target::Sheet(SheetOf::Products));
    h.run_steps(4);
    assert!(h.query_by_label_contains("• CS-TH050").is_some());
    worker
        .responses
        .try_send(Response::Failed {
            id,
            error: Failure::Database(sqlx::Error::PoolTimedOut),
        })
        .unwrap();
    h.run_steps(4);
    h.state_mut().settle();
    h.run_steps(4);
    assert!(h.query_by_label_contains("• CS-TH050").is_none());
    assert!(h.query_by_label_contains("$79.00").is_some());
    h.snapshot("optimistic-row-rolled-back");
}

#[test]
fn a_stored_save_confirms_the_row() {
    let (mut h, worker, id) = saving();
    let mut sensor = common::product(
        11,
        "CS-TH050",
        "Temperature/Humidity Sensor",
        8500,
    );
    sensor.version = 2;
    worker
        .responses
        .try_send(Response::Saved {
            id,
            stored: Stored {
                key: 11,
                number: None,
                version: 2,
            },
            saved: Box::new(Saved::Product(sensor)),
        })
        .unwrap();
    h.run_steps(4);
    h.state_mut().settle();
    h.run_steps(4);
    assert_eq!(price(&h), Decimal::new(8500, 2));
    assert!(h
        .state()
        .orchestrator
        .as_ref()
        .unwrap()
        .pending_keys(Table::Products)
        .is_empty());
}

#[test]
fn shown_goes_back_to_the_last_good_row() {
    let row = Shown::pending(85, 79, RequestId(1));
    assert!(row.is_pending());
    assert_eq!(*row.shown(), 85);
    let back = row.clone().rolled_back();
    assert_eq!((*back.shown(), back.is_pending()), (79, false));
    let kept = row.confirmed(86);
    assert_eq!((*kept.shown(), kept.is_pending()), (86, false));
}
