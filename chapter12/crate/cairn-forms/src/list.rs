//! Read-only datasheets, as Access drew them: rows in view are built each
//! frame, so 13,531 orders cost what 30 do, and every column header is a
//! menu that sorts and filters, the way Access's header drop-downs did.
//!
//! The sort and the filters belong to the screen, in a [`ListView`], not
//! to the widget: immediate mode keeps state where the app can see it.

use crate::field::parse_date;
use cairn_core::Date;
use egui::{Color32, Key, Sense, Stroke, Ui};
use egui_extras::{Column, TableBuilder};
use std::cmp::Ordering;
use std::collections::BTreeSet;

/// A column of a read-only list.
pub struct ListColumn<R> {
    pub label: &'static str,
    pub width: f32,
    pub value: fn(&R) -> String,
}

/// What a list draws: rows of cells under labelled columns.
trait Cells {
    fn rows(&self) -> usize;
    fn columns(&self) -> usize;
    fn label(&self, col: usize) -> &str;
    fn width(&self, col: usize) -> f32;
    fn cell(&self, row: usize, col: usize) -> String;
}

/// Rows of any type, with a function per column.
struct ByColumn<'a, R> {
    rows: &'a [R],
    columns: &'a [ListColumn<R>],
}

impl<R> Cells for ByColumn<'_, R> {
    fn rows(&self) -> usize {
        self.rows.len()
    }
    fn columns(&self) -> usize {
        self.columns.len()
    }
    fn label(&self, col: usize) -> &str {
        self.columns[col].label
    }
    fn width(&self, col: usize) -> f32 {
        self.columns[col].width
    }
    fn cell(&self, row: usize, col: usize) -> String {
        (self.columns[col].value)(&self.rows[row])
    }
}

/// Rows already turned into text.
struct Text<'a> {
    rows: &'a [Vec<String>],
    columns: &'a [(String, f32)],
}

impl Cells for Text<'_> {
    fn rows(&self) -> usize {
        self.rows.len()
    }
    fn columns(&self) -> usize {
        self.columns.len()
    }
    fn label(&self, col: usize) -> &str {
        &self.columns[col].0
    }
    fn width(&self, col: usize) -> f32 {
        self.columns[col].1
    }
    fn cell(&self, row: usize, col: usize) -> String {
        self.rows[row][col].clone()
    }
}

/// A column offers its values as a checklist when it has this many or
/// fewer: statuses and reps, not order numbers.
const CHECKLIST_MAX: usize = 60;

/// One column's filter: text it must contain, and the values ticked in
/// its checklist (`None` is all of them).
#[derive(Debug, Clone, Default, PartialEq)]
struct Filter {
    contains: String,
    only: Option<BTreeSet<String>>,
}

impl Filter {
    fn is_on(&self) -> bool {
        !self.contains.is_empty() || self.only.is_some()
    }

    fn keeps(&self, value: &str) -> bool {
        let contains = self.contains.to_lowercase();
        (contains.is_empty()
            || value.to_lowercase().contains(&contains))
            && self
                .only
                .as_ref()
                .is_none_or(|only| only.contains(value))
    }
}

/// A list's state: the selected row, the sort, the filters, and the rows
/// they leave, in order.
#[derive(Debug, Default)]
pub struct ListView {
    selected: Option<usize>,
    sort: Option<(usize, bool)>,
    filters: Vec<Filter>,
    /// Row indexes, filtered and sorted.
    shown: Vec<usize>,
    /// What `shown` was worked out for.
    shown_for: Option<Settings>,
}

/// The row count, the sort (column, ascending) and the filters.
type Settings = (usize, Option<(usize, bool)>, Vec<Filter>);

/// How a column's values compare: numbers as numbers, dates as dates, so
/// $900 sorts before $1,000 and 9/1 before 10/1.
#[derive(Debug, PartialEq, PartialOrd)]
enum SortKey {
    Blank,
    Number(f64),
    Date(Date),
    Text(String),
}

fn sort_key(value: &str) -> SortKey {
    let v = value.trim();
    if v.is_empty() {
        return SortKey::Blank;
    }
    let numeric: String = v
        .chars()
        .filter(|c| !matches!(c, '$' | ',' | '%'))
        .collect();
    if let Ok(n) = numeric.parse::<f64>() {
        return SortKey::Number(n);
    }
    // A date, or a date and a time: sort by the date, then the text.
    let first = v.split(' ').next().unwrap_or(v);
    if first.contains('/') {
        if let Ok(d) = parse_date(first) {
            return SortKey::Date(d);
        }
    }
    SortKey::Text(v.to_lowercase())
}

/// The words Access uses for sorting this kind of column.
fn sort_words(
    sample: Option<&SortKey>,
) -> (&'static str, &'static str) {
    match sample {
        Some(SortKey::Number(_)) => {
            ("Sort Smallest to Largest", "Sort Largest to Smallest")
        }
        Some(SortKey::Date(_)) => {
            ("Sort Oldest to Newest", "Sort Newest to Oldest")
        }
        _ => ("Sort A to Z", "Sort Z to A"),
    }
}

impl ListView {
    /// Recompute the rows shown, if anything they depend on changed. A
    /// short list is recomputed every frame, since its rows may have
    /// changed in place.
    fn refresh(&mut self, t: &dyn Cells) {
        self.filters.resize(t.columns(), Filter::default());
        let key = (t.rows(), self.sort, self.filters.clone());
        if t.rows() > 2_000 && self.shown_for.as_ref() == Some(&key) {
            return;
        }
        let mut shown: Vec<usize> = (0..t.rows())
            .filter(|&i| {
                self.filters.iter().enumerate().all(|(c, f)| {
                    !f.is_on() || f.keeps(&t.cell(i, c))
                })
            })
            .collect();
        if let Some((col, ascending)) = self.sort {
            let mut keyed: Vec<(SortKey, usize)> = shown
                .iter()
                .map(|&i| (sort_key(&t.cell(i, col)), i))
                .collect();
            keyed.sort_by(|a, b| {
                let o =
                    a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal);
                if ascending {
                    o
                } else {
                    o.reverse()
                }
            });
            shown = keyed.into_iter().map(|(_, i)| i).collect();
        }
        self.shown = shown;
        self.shown_for = Some(key);
    }

    /// The column header: a menu that sorts and filters, marked when it
    /// does either.
    fn header(&mut self, ui: &mut Ui, col: usize, t: &dyn Cells) {
        let name = t.label(col);
        let filtered = self.filters[col].is_on();
        let mut text = egui::RichText::new(name).strong();
        if filtered {
            text = text.color(ui.visuals().selection.stroke.color);
        }
        // The heading is a heading: its name, then marks for a sort or a
        // filter, then at the column's right edge the menu, where
        // Access put its drop-down arrow.
        let label = ui.label(text);
        // Room for both marks always, so ticking a value doesn't move
        // anything in the heading while its menu is open.
        let (marks, _) = ui.allocate_exact_size(
            egui::vec2(22.0, 10.0),
            Sense::hover(),
        );
        if let Some((_, ascending)) = self.sort.filter(|s| s.0 == col)
        {
            arrow(
                ui,
                marks.left_center() + egui::vec2(5.0, 0.0),
                ascending,
            );
        }
        if filtered {
            funnel(ui, marks.right_center() - egui::vec2(5.0, 0.0));
        }
        // Ticking values doesn't close it, as Access's filter list
        // stayed open; a click outside or Esc does.
        let config = egui::containers::menu::MenuConfig::new()
            .close_behavior(
                egui::PopupCloseBehavior::CloseOnClickOutside,
            );
        let icon = egui::Button::new("")
            .frame(false)
            .min_size(egui::vec2(18.0, 18.0));
        let layout = egui::Layout::right_to_left(egui::Align::Center);
        // A fixed id: the marks before it come and go, and an id that
        // moved with them would close the menu as soon as a tick filtered.
        ui.push_id(("heading-menu", col), |ui| {
            ui.with_layout(layout, |ui| {
                let (response, _) =
                    egui::containers::menu::MenuButton::from_button(
                        icon,
                    )
                    .config(config)
                    .ui(ui, |ui| {
                        ui.set_min_width(220.0);
                        let sample = (t.rows() > 0)
                            .then(|| sort_key(&t.cell(0, col)));
                        let (up, down) = sort_words(sample.as_ref());
                        if ui.button(up).clicked() {
                            self.sort = Some((col, true));
                            ui.close();
                        }
                        if ui.button(down).clicked() {
                            self.sort = Some((col, false));
                            ui.close();
                        }
                        ui.separator();
                        let clear =
                            format!("Clear filter from {name}");
                        if ui
                            .add_enabled(
                                filtered,
                                egui::Button::new(clear),
                            )
                            .clicked()
                        {
                            self.filters[col] = Filter::default();
                            ui.close();
                        }
                        ui.horizontal(|ui| {
                            let label = ui.label("Contains");
                            ui.add(
                                egui::TextEdit::singleline(
                                    &mut self.filters[col].contains,
                                )
                                .desired_width(140.0),
                            )
                            .labelled_by(label.id);
                        });
                        self.checklist(ui, col, t);
                    });
                // Named by its heading, for screen readers and tests.
                let response = response
                    .labelled_by(label.id)
                    .on_hover_text("Sort and filter");
                hamburger(ui, &response);
            })
        });
    }

    /// The column's values as a checklist, as Access offered them.
    fn checklist(&mut self, ui: &mut Ui, col: usize, t: &dyn Cells) {
        let mut values = BTreeSet::new();
        for i in 0..t.rows() {
            values.insert(t.cell(i, col));
            if values.len() > CHECKLIST_MAX {
                return;
            }
        }
        ui.separator();
        let f = &mut self.filters[col];
        let mut all = f.only.is_none();
        if ui.checkbox(&mut all, "(Select All)").changed() {
            f.only = (!all).then(BTreeSet::new);
        }
        egui::ScrollArea::vertical().max_height(220.0).show(
            ui,
            |ui| {
                for v in &values {
                    let mut on =
                        f.only.as_ref().is_none_or(|o| o.contains(v));
                    let shown =
                        if v.is_empty() { "(Blanks)" } else { v };
                    if ui.checkbox(&mut on, shown).changed() {
                        let only = f
                            .only
                            .get_or_insert_with(|| values.clone());
                        if on {
                            only.insert(v.clone());
                        } else {
                            only.remove(v);
                        }
                        if only.len() == values.len() {
                            f.only = None;
                        }
                    }
                }
            },
        );
    }

    /// Draw the list. Click selects; double-click or Enter opens. Returns
    /// the row to open, as an index into `rows`.
    pub fn ui<R>(
        &mut self,
        ui: &mut Ui,
        id_salt: impl egui::AsIdSalt,
        rows: &[R],
        columns: &[ListColumn<R>],
    ) -> Option<usize> {
        self.show(ui, id_salt, &ByColumn { rows, columns })
    }

    /// The same, over rows already turned into text, one string per
    /// column: a datasheet view built from any form's fields.
    pub fn ui_text(
        &mut self,
        ui: &mut Ui,
        id_salt: impl egui::AsIdSalt,
        rows: &[Vec<String>],
        columns: &[(String, f32)],
    ) -> Option<usize> {
        self.show(ui, id_salt, &Text { rows, columns })
    }

    fn show(
        &mut self,
        ui: &mut Ui,
        id_salt: impl egui::AsIdSalt,
        t: &dyn Cells,
    ) -> Option<usize> {
        self.refresh(t);
        let n = self.shown.len();
        // `selected` is a position in the rows shown.
        if self.selected.is_some_and(|s| s >= n) {
            self.selected = None;
        }
        let mut open = None;
        let (up, down, enter) = ui.input(|i| {
            (
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::Enter),
            )
        });
        // The keys are the list's unless a text field is taking them.
        // (A clicked row takes the focus itself, so "nothing focused"
        // would ignore Enter right after a click.)
        let focus_free = !ui.ctx().text_edit_focused();
        if focus_free && n > 0 {
            if down {
                self.selected = Some(
                    self.selected.map_or(0, |s| (s + 1).min(n - 1)),
                );
            }
            if up {
                self.selected = Some(
                    self.selected.map_or(0, |s| s.saturating_sub(1)),
                );
            }
            if enter {
                open = self.selected.map(|s| self.shown[s]);
            }
        }
        let mut table = TableBuilder::new(ui)
            .id_salt(id_salt)
            .striped(true)
            .sense(Sense::click());
        for c in 0..t.columns() {
            table = table
                .column(Column::initial(t.width(c)).resizable(true));
        }
        if focus_free && (up || down) {
            if let Some(s) = self.selected {
                table = table.scroll_to_row(s, None);
            }
        }
        let mut clicked = None;
        table
            .header(22.0, |mut header| {
                for col in 0..t.columns() {
                    header.col(|ui| {
                        // Name, marks and menu on one line: [Status =].
                        ui.horizontal(|ui| {
                            self.header(ui, col, t);
                        });
                    });
                }
            })
            .body(|body| {
                body.rows(20.0, n, |mut row| {
                    let at = row.index();
                    let i = self.shown[at];
                    row.set_selected(self.selected == Some(at));
                    for c in 0..t.columns() {
                        row.col(|ui| {
                            // One line, cut off with an ellipsis, as
                            // Access's datasheet did; not selectable, so a
                            // click on the text is a click on the row.
                            ui.add(
                                egui::Label::new(t.cell(i, c))
                                    .selectable(false)
                                    .truncate(),
                            );
                        });
                    }
                    let response = row.response();
                    if response.clicked() {
                        clicked = Some(at);
                    }
                    if response.double_clicked() {
                        open = Some(i);
                    }
                });
            });
        if let Some(at) = clicked {
            self.selected = Some(at);
        }
        open
    }

    /// How many rows the filters leave, of how many.
    pub fn counts(&self) -> (usize, usize) {
        let total = self.shown_for.as_ref().map_or(0, |k| k.0);
        (self.shown.len(), total)
    }

    pub fn is_filtered(&self) -> bool {
        self.filters.iter().any(Filter::is_on)
    }
}

/// A small sort arrow: up for ascending.
fn arrow(ui: &Ui, c: egui::Pos2, ascending: bool) {
    let (h, w) = (4.0, 4.0);
    let points = if ascending {
        vec![
            c + egui::vec2(-w, h),
            c + egui::vec2(w, h),
            c + egui::vec2(0.0, -h),
        ]
    } else {
        vec![
            c + egui::vec2(-w, -h),
            c + egui::vec2(w, -h),
            c + egui::vec2(0.0, h),
        ]
    };
    let color: Color32 = ui.visuals().strong_text_color();
    ui.painter().add(egui::Shape::convex_polygon(
        points,
        color,
        Stroke::NONE,
    ));
}

/// A small funnel, Access's mark for a filtered column.
fn funnel(ui: &Ui, c: egui::Pos2) {
    let color = ui.visuals().selection.stroke.color;
    let top = vec![
        c + egui::vec2(-5.0, -4.0),
        c + egui::vec2(5.0, -4.0),
        c + egui::vec2(1.0, 1.0),
        c + egui::vec2(-1.0, 1.0),
    ];
    ui.painter().add(egui::Shape::convex_polygon(
        top,
        color,
        Stroke::NONE,
    ));
    let stem = egui::Rect::from_min_max(
        c + egui::vec2(-1.0, 1.0),
        c + egui::vec2(1.0, 5.0),
    );
    ui.painter().rect_filled(stem, 0.0, color);
}

/// Three lines: the menu icon, drawn on the frameless button.
fn hamburger(ui: &Ui, response: &egui::Response) {
    let color = ui.style().interact(response).fg_stroke.color;
    let c = response.rect.center();
    for dy in [-4.0, 0.0, 4.0] {
        ui.painter().line_segment(
            [c + egui::vec2(-5.0, dy), c + egui::vec2(5.0, dy)],
            Stroke::new(1.5, color),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(values: &[&str]) -> Vec<String> {
        let mut v: Vec<&str> = values.to_vec();
        v.sort_by(|a, b| {
            sort_key(a)
                .partial_cmp(&sort_key(b))
                .unwrap_or(Ordering::Equal)
        });
        v.into_iter().map(String::from).collect()
    }

    #[test]
    fn money_and_dates_sort_by_value_not_by_text() {
        assert_eq!(
            sorted(&["$1,000.00", "$900.00", "$15.50"]),
            ["$15.50", "$900.00", "$1,000.00"]
        );
        assert_eq!(
            sorted(&["10/1/2026", "9/1/2026", "1/15/2027"]),
            ["9/1/2026", "10/1/2026", "1/15/2027"]
        );
        assert_eq!(sorted(&["b", "", "A"]), ["", "A", "b"]);
    }

    #[test]
    fn a_filter_needs_its_text_and_a_ticked_value() {
        let mut f = Filter {
            contains: "creamery".into(),
            only: None,
        };
        assert!(f.keeps("Shasta Creamery"));
        assert!(!f.keeps("Bay Dairy"));
        f.only = Some(BTreeSet::from(["Bay Creamery".to_string()]));
        assert!(!f.keeps("Shasta Creamery"));
        assert!(f.keeps("Bay Creamery"));
    }
}
