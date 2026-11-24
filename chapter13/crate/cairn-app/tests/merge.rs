//! The three-way comparison (Chapter 12), over a real form: the
//! customer's, with its labels from `#[derive(Form)]`. No database.

mod common;

use cairn_forms::{three_way, Side};

fn customer() -> cairn_app::forms::CustomerDraft {
    common::customer(7, "Westside Dairy", false, 1)
}

#[test]
fn a_field_only_they_changed_comes_in() {
    let original = customer();
    let mine = original.clone();
    let mut theirs = original.clone();
    theirs.ship_address = "9 New Dock Rd".into();
    theirs.version = 4;
    let m = three_way(&original, &mine, &theirs);
    assert!(m.asks.is_empty());
    assert_eq!(m.merged.ship_address, "9 New Dock Rd");
    assert_eq!(m.merged.version, 4, "the merge starts from theirs");
    assert_eq!(m.settled.len(), 1);
    assert_eq!(m.settled[0].label, "Ship Address");
    assert_eq!(m.settled[0].side, Side::Theirs);
}

#[test]
fn a_field_only_the_rep_changed_stays() {
    let original = customer();
    let mut mine = original.clone();
    mine.notes = "Called back".into();
    let mut theirs = original.clone();
    theirs.version = 4;
    let m = three_way(&original, &mine, &theirs);
    assert!(m.asks.is_empty());
    assert_eq!(m.merged.notes, "Called back");
    assert_eq!(m.merged.version, 4);
    assert_eq!(m.settled[0].side, Side::Mine);
}

#[test]
fn different_fields_both_land_and_nothing_is_asked() {
    let original = customer();
    let mut mine = original.clone();
    mine.notes = "Called back".into();
    let mut theirs = original.clone();
    theirs.ship_address = "9 New Dock Rd".into();
    let m = three_way(&original, &mine, &theirs);
    assert!(m.asks.is_empty());
    assert!(m.resolved());
    assert_eq!(m.merged.notes, "Called back");
    assert_eq!(m.merged.ship_address, "9 New Dock Rd");
}

#[test]
fn the_same_change_on_both_sides_is_not_a_question() {
    let original = customer();
    let mut mine = original.clone();
    mine.phone = "555-0199".into();
    let theirs = mine.clone();
    let m = three_way(&original, &mine, &theirs);
    assert!(m.asks.is_empty());
    assert!(m.settled.is_empty());
}

#[test]
fn a_field_both_changed_differently_asks_about_that_field_only() {
    let original = customer();
    let mut mine = original.clone();
    mine.phone = "555-0100".into();
    mine.notes = "Called back".into();
    let mut theirs = original.clone();
    theirs.phone = "555-0199".into();
    let mut m = three_way(&original, &mine, &theirs);
    assert_eq!(m.asks.len(), 1);
    assert_eq!(m.asks[0].label, "Phone");
    assert_eq!(m.asks[0].mine, "555-0100");
    assert_eq!(m.asks[0].theirs, "555-0199");
    assert!(!m.resolved());

    m.choose(0, Side::Mine);
    assert!(m.resolved());
    assert_eq!(m.merged.phone, "555-0100");
    assert_eq!(m.merged.notes, "Called back");

    m.choose(0, Side::Theirs);
    assert_eq!(m.merged.phone, "555-0199");
}

// Rows (H1 P5a, P7): a record's lines are compared the same three ways,
// each by its own key and version.

use cairn_app::context::{AppContext, Saved, Touched};
use cairn_app::forms::OrderLineDraft;
use cairn_app::screens::document::OrderScreen;
use cairn_app::screens::Screen;
use cairn_core::{OrderId, ProductId};
use cairn_forms::{merge_rows, Keyed, Section};
use rust_decimal::Decimal;

fn line(id: i32, version: i32, qty: i32) -> OrderLineDraft {
    OrderLineDraft {
        id: Some(id),
        version,
        product: Some(ProductId(11)),
        description: format!("line {id}"),
        quantity: qty,
        unit_price: Decimal::new(7900, 2),
        discount: 0.0,
        serial_numbers: String::new(),
    }
}

fn describe(l: &OrderLineDraft) -> String {
    l.description.clone()
}

#[test]
fn two_reps_on_line_2s_quantity_ask_about_that_one_field() {
    let read = vec![line(1, 1, 12), line(2, 1, 1)];
    let mut mine = read.clone();
    mine[1].quantity = 3;
    let mut theirs = read.clone();
    theirs[1].quantity = 5;
    theirs[1].version = 2;
    let m = merge_rows("Line", &read, &mine, &[], &theirs, describe);
    let qs = m.questions();
    assert_eq!(qs.len(), 1);
    assert_eq!(qs[0].label, "Line 2, Qty");
    assert_eq!(qs[0].mine, "Keep mine: 3");
    assert_eq!(qs[0].theirs, "Take theirs: 5");
    println!("{}: {} / {}", qs[0].label, qs[0].mine, qs[0].theirs);
    // Line 1 is nobody's question: it is theirs, untouched.
    assert_eq!(m.rows().len(), 2);
}

#[test]
fn lines_on_different_rows_both_land_and_nothing_is_asked() {
    let read = vec![line(1, 1, 12), line(2, 1, 1)];
    let mut mine = read.clone();
    mine[0].quantity = 20;
    let mut theirs = read.clone();
    theirs[1].quantity = 5;
    theirs[1].version = 2;
    let m = merge_rows("Line", &read, &mine, &[], &theirs, describe);
    assert!(m.questions().is_empty());
    let rows = m.rows();
    assert_eq!((rows[0].quantity, rows[1].quantity), (20, 5));
    assert_eq!(
        m.written(),
        vec![1],
        "only the rep's own edit is written"
    );
    assert_eq!(
        rows[1].version(),
        2,
        "line 2 is theirs, at their version"
    );
}

#[test]
fn a_line_the_rep_edited_and_the_other_rep_deleted_asks() {
    let read = vec![line(1, 1, 12), line(2, 1, 1)];
    let mut mine = read.clone();
    mine[1].quantity = 3;
    let theirs = vec![read[0].clone()];
    let mut m =
        merge_rows("Line", &read, &mine, &[], &theirs, describe);
    let qs = m.questions();
    assert_eq!(qs.len(), 1);
    assert_eq!(qs[0].label, "Line 2: the other rep deleted it");
    assert!(!m.resolved());
    assert_eq!(m.rows().len(), 1, "until answered, it stays deleted");

    m.answer(0, Side::Mine);
    let rows = m.rows();
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[1].key(), rows[1].version()), (None, 0));
    assert_eq!(rows[1].quantity, 3, "added again, as a new line");
}

#[test]
fn a_line_the_rep_deleted_and_the_other_rep_changed_asks() {
    let read = vec![line(1, 1, 12), line(2, 1, 1)];
    let mine = vec![read[0].clone()];
    let gone = vec![read[1].clone()];
    let mut theirs = read.clone();
    theirs[1].quantity = 5;
    theirs[1].version = 2;
    let mut m =
        merge_rows("Line", &read, &mine, &gone, &theirs, describe);
    assert_eq!(m.questions().len(), 1);
    assert!(m.questions()[0].label.contains("you deleted it"));

    m.answer(0, Side::Mine);
    assert_eq!(m.removed().len(), 1);
    assert_eq!(
        m.removed()[0].version(),
        2,
        "deleted at their version"
    );
    assert_eq!(m.rows().len(), 1);

    m.answer(0, Side::Theirs);
    assert!(m.removed().is_empty());
    assert_eq!(m.rows().len(), 2, "their line stays");
}

#[test]
fn a_line_the_rep_deleted_and_nobody_touched_stays_deleted() {
    let read = vec![line(1, 1, 12), line(2, 1, 1)];
    let mine = vec![read[0].clone()];
    let m = merge_rows(
        "Line",
        &read,
        &mine,
        &[read[1].clone()],
        &read,
        describe,
    );
    assert!(m.questions().is_empty());
    assert_eq!(m.removed().len(), 1);
}

#[test]
fn a_line_the_other_rep_added_comes_in() {
    let read = vec![line(1, 1, 12)];
    let mine = read.clone();
    let mut theirs = read.clone();
    theirs.push(line(9, 1, 4));
    let m = merge_rows("Line", &read, &mine, &[], &theirs, describe);
    assert!(m.questions().is_empty());
    assert_eq!(m.rows().len(), 2);
    assert!(m.settled()[0].contains("line"));
}

/// The order screen's settle: lines and header together.
#[test]
fn the_order_screen_settles_lines_with_the_header() {
    let mut data = common::snapshot();
    data.order_lines
        .insert(OrderId(2), vec![line(21, 1, 12), line(22, 1, 1)]);
    let ctx = AppContext::new(data.clone());
    let mut screen = OrderScreen::open(&ctx, Some(OrderId(2)), None);
    // This rep reworked line 2's quantity.
    screen.lines.rows[1].draft.quantity = 3;
    let mine = Saved::Order {
        order: screen.form.draft.clone(),
        lines: screen.lines.records().cloned().collect(),
        payments: Vec::new(),
        removed_lines: Vec::new(),
        removed_payments: Vec::new(),
        touched: Some(Touched {
            lines: vec![22],
            ..Touched::default()
        }),
    };
    // The other rep changed line 2's quantity too, and the notes.
    let mut theirs = data.clone();
    theirs.order_lines.get_mut(&OrderId(2)).unwrap()[1].quantity = 5;
    theirs.order_lines.get_mut(&OrderId(2)).unwrap()[1].version = 2;
    theirs
        .orders
        .iter_mut()
        .find(|o| o.id == Some(OrderId(2)))
        .unwrap()
        .notes = "Dock closed Friday".into();
    theirs
        .orders
        .iter_mut()
        .find(|o| o.id == Some(OrderId(2)))
        .unwrap()
        .version = 2;

    let settle = screen.settle(&mine, &theirs).expect("a dialog");
    let mut h = egui_kittest::Harness::builder()
        .with_size([700.0, 420.0])
        .with_pixels_per_point(2.0)
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            |ui, s: &mut Box<dyn cairn_app::conflict::Settle>| {
                s.ui(ui, &common::context());
            },
            settle,
        );
    h.run_steps(3);
    h.snapshot("conflict-dialog-lines");
}
