//! The read-only pieces of a form.

use egui::{Align, Layout, RichText, Ui};
use rust_decimal::Decimal;

/// V1's `TotalBox`: a locked, right-aligned amount, out of the Tab order.
pub fn total(ui: &mut Ui, label: &str, amount: Decimal, bold: bool) {
    ui.label(label);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let text = RichText::new(crate::money(amount)).monospace();
        ui.label(if bold { text.strong() } else { text });
    });
    ui.end_row();
}

/// A form's title bar: its caption, and "(unsaved)" while it is.
pub fn title(ui: &mut Ui, caption: &str, dirty: bool) {
    ui.horizontal(|ui| {
        ui.heading(caption);
        if dirty {
            ui.label(RichText::new("(unsaved)").weak());
        }
    });
}

/// Contrast for forms, in both themes. egui's defaults draw a text box as
/// a borderless fill barely off the background, so on a form full of them
/// you can't see where one field ends. Access drew every box with an
/// outline on a grey form; this does the same.
pub fn form_style(ctx: &egui::Context) {
    use egui::{Color32, Stroke, Theme};
    let tune = |s: &mut egui::Style,
                panel: u8,
                field: u8,
                line: u8,
                edge: u8| {
        let v = &mut s.visuals;
        v.panel_fill = Color32::from_gray(panel);
        v.window_fill = Color32::from_gray(panel);
        v.text_edit_bg_color = Some(Color32::from_gray(field));
        v.extreme_bg_color = Color32::from_gray(field);
        // Separators, group boxes and table lines.
        v.widgets.noninteractive.bg_stroke =
            Stroke::new(1.0, Color32::from_gray(line));
        // Every field and button gets an outline, darker on hover.
        v.widgets.inactive.bg_stroke =
            Stroke::new(1.0, Color32::from_gray(edge));
        let hover = if panel > 128 { edge - 60 } else { edge + 60 };
        v.widgets.hovered.bg_stroke =
            Stroke::new(1.0, Color32::from_gray(hover));
    };
    ctx.style_mut_of(Theme::Light, |s| tune(s, 232, 255, 175, 150));
    ctx.style_mut_of(Theme::Dark, |s| tune(s, 32, 12, 70, 95));
}
