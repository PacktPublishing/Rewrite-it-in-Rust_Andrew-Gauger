//! The forms' own behaviour, through the whole app: V1's record
//! navigation, its buttons, its handlers. No database.

mod common;

use cairn_app::app::CairnApp;
use cairn_app::context::Target;
use cairn_core::CustomerId;
use common::context;
use egui::accesskit::Role;
use egui::Key;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use rust_decimal::Decimal;

fn harness(target: Target) -> Harness<'static, CairnApp> {
    let mut app = CairnApp::new(context());
    app.open(target);
    let mut h = Harness::builder()
        .with_size([1180.0, 760.0])
        .build_ui_state(|ui, app: &mut CairnApp| app.show(ui), app);
    h.run();
    h
}

fn click(h: &mut Harness<'_, CairnApp>, label: &str) {
    h.get_by_role_and_label(Role::Button, label).click();
    h.run();
}

#[test]
fn moving_to_the_next_record_saves_this_one_as_access_did() {
    let mut h = harness(Target::Product(None));
    let price =
        h.get_by_role_and_label(Role::TextInput, "Unit Price");
    price.focus();
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Unit Price")
        .type_text("85");
    h.run();
    h.key_press(Key::PageDown);
    h.run();
    let p = &h.state().ctx.data.products;
    let sensor = p.iter().find(|p| p.sku == "CS-TH050").unwrap();
    assert_eq!(sensor.unit_price, Decimal::new(85, 0));
    assert!(h.query_by_label("2 of 2").is_some());
}

#[test]
fn new_quote_on_a_customer_fills_in_the_customer_like_doc_load() {
    let mut h = harness(Target::Customer(Some(CustomerId(767))));
    click(&mut h, "New Quote");
    // V1 opened frmQuotes with the customer as OpenArgs, and DocLoad ran
    // DocCustomerChanged: the rep, the primary contact, and no tax for a
    // tax-exempt customer.
    let customer =
        h.get_by_role_and_label(Role::TextInput, "Customer");
    assert_eq!(customer.value().as_deref(), Some("Bay Dairy"));
    let rep = h.get_by_role_and_label(Role::TextInput, "Sales Rep");
    assert_eq!(rep.value().as_deref(), Some("Maya Torres"));
    let contact = h.get_by_role_and_label(Role::TextInput, "Contact");
    assert_eq!(contact.value().as_deref(), Some("Joe Park"));
    let tax = h.get_by_role_and_label(Role::TextInput, "Tax Rate");
    assert_eq!(tax.value().as_deref(), Some("0%"));
}

#[test]
fn mark_returned_stamps_the_message_and_saves_it() {
    let mut h = harness(Target::Message(Some(1)));
    click(&mut h, "Mark Returned");
    let m = &h.state().ctx.data.messages[0];
    assert_eq!(m.status, "Returned");
    assert!(m.returned_at.is_some());
    // The open-messages list on the main menu no longer has it.
    assert!(h.state().ctx.open_messages.is_empty());
}

/// The order numbers in the datasheet, top to bottom.
fn order_numbers(h: &Harness<'_, CairnApp>) -> Vec<String> {
    let mut rows: Vec<(f32, String)> = h
        .root()
        .query_all(
            egui_kittest::kittest::by().role(Role::Label).predicate(
                |n| n.value().is_some_and(|v| v.starts_with("SO-")),
            ),
        )
        .map(|n| (n.rect().top(), n.value().unwrap()))
        .collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    rows.into_iter().map(|(_, v)| v).collect()
}

#[test]
fn datasheet_headings_sort_and_filter_as_access_did() {
    let mut h = harness(Target::OrderList);
    assert_eq!(order_numbers(&h).len(), 3);

    // Total: "Sort Largest to Smallest". $9,480 > $790 > $158.
    click(&mut h, "Total");
    click(&mut h, "Sort Largest to Smallest");
    assert_eq!(
        order_numbers(&h),
        ["SO-2026-0003", "SO-2026-0001", "SO-2026-0002"]
    );

    // Status: untick Closed in the checklist.
    click(&mut h, "Status");
    h.get_by_role_and_label(Role::CheckBox, "Closed").click();
    h.run();
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(order_numbers(&h), ["SO-2026-0003", "SO-2026-0002"]);
    assert!(h.query_by_label_contains("2 of 3 (Filtered)").is_some());
}

#[test]
fn datasheet_view_lists_the_records_and_opens_one() {
    let mut h = harness(Target::Customer(None));
    click(&mut h, "Datasheet View");
    assert!(h.query_by_label("Customers: 2").is_some());
    // Customers' Sales Rep column shows names, not IDs.
    assert!(h.query_by_label("Lena Brandt").is_some());
    // Select the row, and Enter opens it (as a double-click does).
    h.get_by_label("Bay Dairy").click();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    let company = h.get_by_role_and_label(Role::TextInput, "Company");
    assert_eq!(company.value().as_deref(), Some("Bay Dairy"));
}
