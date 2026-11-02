//! The quote form without a mouse: V1's keystrokes, checked with
//! `egui_kittest`. A small snapshot built here, so no database.

mod common;

use cairn_app::context::AppContext;
use cairn_app::screens::document::QuoteScreen;
use cairn_app::screens::Screen;
use cairn_core::{ContactId, CustomerId, ProductId, RepId};
use cairn_forms::Form;
use common::context;
use egui::accesskit::Role;
use egui::Key;
use egui_kittest::kittest::{by, Queryable};
use egui_kittest::Harness;
use rust_decimal::Decimal;

fn harness(ctx: &AppContext) -> Harness<'_, QuoteScreen> {
    let screen = QuoteScreen::open(ctx, None, None);
    let mut h = Harness::builder()
        .with_size([1180.0, 760.0])
        .build_ui_state(
            move |ui, screen: &mut QuoteScreen| {
                let _ = screen.ui(ui, ctx);
            },
            screen,
        );
    h.run();
    h
}

fn press(h: &mut Harness<'_, QuoteScreen>, key: Key) {
    h.key_press(key);
    h.run();
}

/// The text box labelled `label` (the totals have labels too).
fn field<'h>(
    h: &'h Harness<'_, QuoteScreen>,
    label: &'h str,
) -> egui_kittest::Node<'h> {
    h.get_by_role_and_label(Role::TextInput, label)
}

fn type_text(
    h: &mut Harness<'_, QuoteScreen>,
    label: &str,
    text: &str,
) {
    field(h, label).type_text(text);
    h.run();
}

#[test]
fn tab_follows_v1s_field_order_and_skips_the_locked_number() {
    let ctx = context();
    let mut h = harness(&ctx);
    // Quote # is locked, so the form opens on Quote Date.
    assert!(h.get_by_label("Quote Date").is_focused());
    for next in ["Valid Until", "Status", "Sales Rep", "Customer"] {
        press(&mut h, Key::Tab);
        assert!(h.get_by_label(next).is_focused(), "{next}");
    }
}

#[test]
fn picking_a_customer_fills_rep_contact_and_tax_like_v1() {
    let ctx = context();
    let mut h = harness(&ctx);
    h.get_by_label("Customer").focus();
    h.run();
    type_text(&mut h, "Customer", "shas");
    press(&mut h, Key::Enter);
    let q = &h.state().form.draft;
    assert_eq!(q.customer, Some(CustomerId(766)));
    assert_eq!(q.rep, Some(RepId(12)));
    assert_eq!(q.contact, Some(ContactId(1)));
    assert!((q.tax_rate - 0.0825).abs() < 1e-12);
    // Enter accepted the match and moved on.
    assert!(h.get_by_label("Contact").is_focused());
}

#[test]
fn a_name_not_in_the_list_is_refused() {
    let ctx = context();
    let mut h = harness(&ctx);
    h.get_by_label("Customer").focus();
    h.run();
    type_text(&mut h, "Customer", "Nobody Ltd");
    press(&mut h, Key::Tab);
    assert_eq!(h.state().form.draft.customer, None);
}

#[test]
fn esc_undoes_the_field_then_the_record() {
    let ctx = context();
    let mut h = harness(&ctx);
    h.get_by_label("Customer").focus();
    h.run();
    type_text(&mut h, "Customer", "bay");
    press(&mut h, Key::Enter);
    field(&h, "Shipping").focus();
    h.run();
    type_text(&mut h, "Shipping", "25");
    assert_eq!(h.state().form.draft.shipping, Decimal::new(25, 0));

    press(&mut h, Key::Escape);
    assert_eq!(h.state().form.draft.shipping, Decimal::ZERO);
    assert_eq!(h.state().form.draft.customer, Some(CustomerId(767)));

    press(&mut h, Key::Escape);
    assert_eq!(h.state().form.draft.customer, None);
    assert!(!h.state().is_dirty());
}

#[test]
fn enter_moves_to_the_next_field() {
    let ctx = context();
    let mut h = harness(&ctx);
    press(&mut h, Key::Enter);
    assert!(h.get_by_label("Valid Until").is_focused());
}

#[test]
fn a_bad_tax_rate_is_outlined_and_blocks_the_save() {
    let ctx = context();
    let mut h = harness(&ctx);
    h.get_by_label("Tax Rate").focus();
    h.run();
    type_text(&mut h, "Tax Rate", "825");
    press(&mut h, Key::Tab);
    h.get_by_label("Must be between 0% and 100%");
    h.key_press_modifiers(egui::Modifiers::SHIFT, Key::Enter);
    h.run();
    // Refused: the first bad field (Sales Rep, required) gets the focus.
    assert!(h.get_by_label("Sales Rep").is_focused());
    assert!(h.state().form.is_new());
}

/// Type where the keyboard is.
fn type_here(h: &mut Harness<'_, QuoteScreen>, text: &str) -> String {
    let node = h
        .root()
        .query_all(by().predicate(|n| n.is_focused()))
        .next()
        .expect("something has the focus");
    let label = format!("{node:?}");
    node.type_text(text);
    h.run();
    label
}

fn focused_label(h: &Harness<'_, QuoteScreen>) -> String {
    let nodes: Vec<String> = h
        .root()
        .query_all(by().predicate(|n| n.is_focused()))
        .map(|n| {
            let s = format!("{n:?}");
            let label = s.split("label: \"").nth(1).unwrap_or("");
            label.split('"').next().unwrap_or("").to_string()
        })
        .collect();
    assert_eq!(nodes.len(), 1, "{nodes:?}");
    nodes[0].clone()
}

#[test]
fn a_whole_quote_line_goes_in_without_the_mouse() {
    let ctx = context();
    let mut h = harness(&ctx);
    // The blank row's Product cell. Column headers name the cells.
    field(&h, "Product").focus();
    h.run();
    type_here(&mut h, "th05");
    press(&mut h, Key::Enter); // pick CS-TH050
    assert_eq!(focused_label(&h), "Description");
    press(&mut h, Key::Tab);
    assert_eq!(focused_label(&h), "Qty");
    type_here(&mut h, "12"); // replaces the selected 1
    press(&mut h, Key::Tab);
    assert_eq!(focused_label(&h), "Unit Price");
    press(&mut h, Key::Tab);
    type_here(&mut h, "5");
    press(&mut h, Key::Enter); // off the row: the next blank row

    let lines: Vec<_> = h.state().lines.records().cloned().collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].product, Some(ProductId(11)));
    assert_eq!(lines[0].quantity, 12);
    assert_eq!(lines[0].unit_price, Decimal::new(7900, 2));
    assert!((lines[0].discount - 0.05).abs() < 1e-12);
    assert_eq!(
        cairn_core::line_total(&lines[0].doc_line()),
        Decimal::new(90060, 2)
    );
    assert_eq!(focused_label(&h), "Product");

    // The description was left blank, so it shows what will print: the
    // product's name (R02).
    let blank = lines[0].placeholder(1, &ctx);
    assert_eq!(blank.as_deref(), Some("Temperature/Humidity Sensor"));

    // Up goes back to the line just entered, same column.
    press(&mut h, Key::ArrowUp);
    let product = h
        .root()
        .query_all(by().predicate(|n| n.is_focused()))
        .next()
        .unwrap()
        .value();
    assert_eq!(product.as_deref(), Some("CS-TH050"));
}
