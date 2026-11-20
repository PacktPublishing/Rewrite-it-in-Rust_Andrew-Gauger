//! The conflict dialog, end to end over a fake worker (Chapter 12): a
//! refused save comes back as `Response::Conflict`, the dialog lists
//! what came in and asks about the one field both reps changed, and
//! Save sends the merged record over the version Postgres has now. No
//! database.
//!
//!   cargo test -p cairn-app --test conflict_dialog -- --nocapture

mod common;

use cairn_app::app::CairnApp;
use cairn_app::context::{Saved, Target};
use cairn_app::data::{Snapshot, Table};
use cairn_app::orchestrator::Orchestrator;
use cairn_app::worker::{Request, Response, CAPACITY};
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use rust_decimal::Decimal;
use tokio::sync::mpsc;

#[test]
fn the_dialog_asks_only_about_the_field_both_changed() {
    let (requests, from_ui) = mpsc::channel(CAPACITY);
    let (to_ui, responses) = mpsc::channel(CAPACITY);
    let mut worker = from_ui;
    let mut app = CairnApp::new(common::context())
        .with(Orchestrator::new(requests, responses));
    // The sensor, as this rep read it: version 1, $79.00.
    app.open(Target::Product(Some(11)));
    let mut h = Harness::builder()
        .with_size([820.0, 560.0])
        .with_pixels_per_point(2.0)
        .with_theme(egui::Theme::Light)
        .build_ui_state(|ui, app: &mut CairnApp| app.show(ui), app);
    h.run_steps(4);

    // This rep changes the price. Save.
    h.get_by_role_and_label(Role::TextInput, "Unit Price")
        .focus();
    h.run_steps(4);
    h.get_by_role_and_label(Role::TextInput, "Unit Price")
        .type_text("85");
    h.run_steps(4);
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
    h.run_steps(4);
    let sent = std::iter::from_fn(|| worker.try_recv().ok())
        .find_map(|r| match r {
            Request::Save { id, saved } => Some((id, saved)),
            _ => None,
        })
        .expect("a save");
    let (id, mine) = sent;
    let Saved::Product(sent_product) = &*mine else {
        panic!("a product")
    };
    assert_eq!(sent_product.version, 1);

    // Meanwhile another rep renamed it and also changed the price.
    let mut theirs =
        common::product(11, "CS-TH050", "Sensor, rev B", 9000);
    theirs.version = 2;
    to_ui
        .try_send(Response::Conflict {
            id,
            table: Table::Products,
            key: 11,
            mine,
            theirs: Box::new(Snapshot {
                products: vec![theirs],
                ..Snapshot::default()
            }),
        })
        .unwrap();
    h.run_steps(2);
    h.state_mut().settle();
    h.run_steps(4);

    assert!(h
        .query_by_label("Someone else changed this record")
        .is_some());
    // The name came in from the other rep. Only the price is asked.
    assert!(h
        .query_by_label_contains("Product Name: Sensor, rev B")
        .is_some());
    assert!(h.query_by_label_contains("Keep mine: $85.00").is_some());
    assert!(h
        .query_by_label_contains("Take theirs: $90.00")
        .is_some());
    h.snapshot("conflict-dialog");

    // Save waits for an answer.
    assert!(h
        .get_by_label("Save merged record")
        .accesskit_node()
        .is_disabled());
    h.get_by_label_contains("Keep mine: $85.00").click();
    h.run_steps(4);
    h.get_by_label("Save merged record").click();
    h.run_steps(4);

    let resent = std::iter::from_fn(|| worker.try_recv().ok())
        .find_map(|r| match r {
            Request::Save { saved, .. } => Some(saved),
            _ => None,
        })
        .expect("the merged save");
    let Saved::Product(p) = *resent else {
        panic!("a product")
    };
    println!(
        "resent: {:?} at ${}, version {}",
        p.name, p.unit_price, p.version
    );
    assert_eq!(p.name, "Sensor, rev B");
    assert_eq!(p.unit_price, Decimal::new(8500, 2));
    assert_eq!(p.version, 2, "checked against what Postgres has now");
}
