//! What the hand-built form does, and what it leaves to you.

use company_by_hand::{CompanyInfo, CompanyScreen};
use egui::accesskit::Role;
use egui::Key;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

/// Click into a box and type, as a person would.
fn type_into(
    h: &mut Harness<'_, CompanyScreen>,
    label: &str,
    text: &str,
) {
    h.get_by_role_and_label(Role::TextInput, label).focus();
    h.run();
    h.get_by_role_and_label(Role::TextInput, label)
        .type_text(text);
    h.run();
}

fn harness() -> Harness<'static, CompanyScreen> {
    let mut h = Harness::builder()
        .with_size([731.0, 330.0])
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            |ui, s: &mut CompanyScreen| s.ui(ui),
            CompanyScreen::new(CompanyInfo::v1()),
        );
    h.run();
    h
}

#[test]
fn the_screen_as_drawn() {
    let mut h = harness();
    h.snapshot("company-by-hand");
}

#[test]
fn tab_goes_in_the_order_the_widgets_were_drawn() {
    let mut h = harness();
    h.get_by_role_and_label(Role::TextInput, "Country").focus();
    h.run();
    h.key_press(Key::Tab);
    h.run();
    // The second grid was drawn after the first, so Tab leaves Country
    // for Phone.
    assert!(h
        .get_by_role_and_label(Role::TextInput, "Phone")
        .is_focused());
}

#[test]
fn typing_makes_it_unsaved_and_undo_puts_it_back() {
    let mut h = harness();
    type_into(&mut h, "City", "ville");
    assert_eq!(h.state().draft.city, "[City]ville");
    assert!(h.query_by_label("(unsaved)").is_some());
    h.get_by_role_and_label(Role::Button, "Undo").click();
    h.run();
    assert_eq!(h.state().draft, CompanyInfo::v1());
}

#[test]
fn a_rate_that_isnt_a_number_is_kept_as_text_and_said_nowhere() {
    let mut h = harness();
    type_into(&mut h, "Default Tax Rate", "x");
    // "8.25x" isn't a rate, so the rate is unchanged, and nothing on
    // screen says why. Chapter 10's form kit shows the error.
    assert_eq!(h.state().tax_text, "8.25x");
    assert_eq!(h.state().draft.default_tax_rate, 0.0825);
    h.snapshot("company-by-hand-bad-rate");
}
