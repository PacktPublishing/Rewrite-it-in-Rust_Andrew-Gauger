//! Screenshots of every form over the placeholder rows (`dev-ipsum.sh`),
//! in the light theme (it prints), at the desktop size and at 1024 × 768,
//! an iPad in landscape. They read
//! the dev database, so they're ignored by default:
//!
//!   UPDATE_SNAPSHOTS=1 cargo test -p cairn-app --test screens -- --ignored

use cairn_app::app::CairnApp;
use cairn_app::context::{AppContext, Target};
use cairn_app::data;
use cairn_app::screens::sheet::SheetOf;
use egui_kittest::{Harness, SnapshotResults};

fn app(target: Target) -> CairnApp {
    let url = std::env::var("CAIRN_DATABASE_URL")
        .unwrap_or_else(|_| data::DEV_DATABASE_URL.into());
    let snapshot = data::load(&url).expect("dev database");
    let mut app = CairnApp::new(AppContext::new(snapshot));
    app.open(target);
    app
}

#[test]
#[ignore = "reads the dev database"]
fn every_form() {
    let mut results = SnapshotResults::new();
    let first = app(Target::Menu);
    let d = &first.ctx.data;
    let quote = d.quotes.first().and_then(|q| q.id);
    let order = d.orders.first().and_then(|o| o.id);
    let shots = [
        ("main-menu", Target::Menu),
        ("customers", Target::Customer(None)),
        ("quote", Target::Quote(quote)),
        ("order", Target::Order(order)),
        ("order-datasheet", Target::OrderList),
        ("quote-datasheet", Target::Sheet(SheetOf::Quotes)),
        ("activity-datasheet", Target::Sheet(SheetOf::Activities)),
        ("products", Target::Product(None)),
        ("sales-reps", Target::Rep(None)),
        ("company-info", Target::Company),
        ("activity", Target::Activity(None)),
        ("phone-message", Target::Message(None)),
        ("rma", Target::Rma(None)),
    ];
    for (name, target) in shots {
        for (size, suffix) in [
            ([1180.0, 760.0], "desktop"),
            ([1024.0, 768.0], "tablet"),
        ] {
            let mut h = Harness::builder()
                .with_size(size)
                .with_theme(egui::Theme::Light)
                .build_ui_state(
                    |ui, app: &mut CairnApp| app.show(ui),
                    app(target),
                );
            h.run();
            h.snapshot(format!("{name}-{suffix}"));
            results.extend_harness(&mut h);
        }
    }
}

/// The lookup open on a new quote line, "li-00" typed: egui 0.37's
/// `CompletionPopup`, with the kit's ranked rows.
#[test]
#[ignore = "reads the dev database"]
fn lookup_open() {
    use egui::accesskit::Role;
    use egui_kittest::kittest::Queryable;
    let first = app(Target::Menu);
    let quote = first.ctx.data.quotes.first().and_then(|q| q.id);
    let mut h = Harness::builder()
        .with_size([1180.0, 760.0])
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            |ui, app: &mut CairnApp| app.show(ui),
            app(Target::Quote(quote)),
        );
    h.run();
    // The blank row's Product cell is the last one.
    let cell = h
        .get_all_by_role_and_label(Role::TextInput, "Product")
        .last()
        .unwrap();
    cell.focus();
    h.run();
    let cell = h
        .get_all_by_role_and_label(Role::TextInput, "Product")
        .last()
        .unwrap();
    cell.type_text("li-00");
    h.run();
    h.snapshot("lookup-open");
    // Picked: the price fills in, and the blank description shows,
    // faint, the product name it will print.
    h.key_press(egui::Key::Enter);
    h.run();
    h.snapshot("lookup-picked");
}

/// Sales Orders in datasheet view: the Status heading's menu open, then
/// Closed and Cancelled unticked and the list sorted by Total.
#[test]
#[ignore = "reads the dev database"]
fn datasheet_filter() {
    use egui::accesskit::Role;
    use egui_kittest::kittest::Queryable;
    let mut h = Harness::builder()
        .with_size([1180.0, 760.0])
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            |ui, app: &mut CairnApp| app.show(ui),
            app(Target::OrderList),
        );
    h.run();
    h.get_by_role_and_label(Role::Button, "Status").click();
    h.run();
    h.snapshot("datasheet-filter-menu");
    for status in ["Closed", "Cancelled"] {
        h.get_by_role_and_label(Role::CheckBox, status).click();
        h.run();
    }
    h.key_press(egui::Key::Escape);
    h.run();
    h.get_by_role_and_label(Role::Button, "Total").click();
    h.run();
    h.get_by_role_and_label(Role::Button, "Sort Largest to Smallest")
        .click();
    h.run();
    h.snapshot("datasheet-filtered");
}

/// Company Info, derived, refusing a tax rate of 825%: `egui_form`'s
/// outline and message, and the focus back on the field. The figure H4
/// sets beside H2's hand-built form keeping "8.25x" and saying nothing.
#[test]
#[ignore = "reads the dev database"]
fn company_refused() {
    use egui::accesskit::Role;
    use egui::Key;
    use egui_kittest::kittest::Queryable;
    let mut h = Harness::builder()
        .with_size([900.0, 420.0])
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            |ui, app: &mut CairnApp| app.show(ui),
            app(Target::Company),
        );
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Default Tax Rate")
        .focus();
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Default Tax Rate")
        .type_text("825");
    h.run();
    h.key_press(Key::Tab);
    h.run();
    h.key_press_modifiers(egui::Modifiers::SHIFT, Key::Enter);
    h.run();
    h.snapshot("company-refused");
}
