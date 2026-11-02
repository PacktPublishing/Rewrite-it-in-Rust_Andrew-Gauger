//! One searchable lookup for every combo box: type-ahead, ranked
//! matches, and limited to its list, the way V1's combos were
//! (`LimitToList = True`).
//!
//! egui 0.37's `CompletionPopup` does the popup, the arrow keys, and
//! accepting with Enter, Tab or a click. This module adds the list, the
//! ranking, and the rule that only a row in the list can be picked.

/// One row a lookup offers.
#[derive(Debug, Clone, PartialEq)]
pub struct LookupRow {
    /// What the field stores: an ID, or a status's text.
    pub key: String,
    /// What the field shows.
    pub text: String,
    /// Shown faint beside the text, and searched too (a city, a SKU's
    /// product name).
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LookupList {
    pub rows: Vec<LookupRow>,
    /// Row by key: a datasheet of 68,257 activities looks up a customer
    /// for each.
    index: std::collections::HashMap<String, usize>,
}

impl LookupList {
    pub fn new(rows: Vec<LookupRow>) -> Self {
        let index = rows
            .iter()
            .enumerate()
            .rev()
            .map(|(i, r)| (r.key.clone(), i))
            .collect();
        LookupList { rows, index }
    }

    /// A value list, where each value is its own key.
    pub fn values<T: ToString>(values: &[T]) -> Self {
        LookupList::new(
            values
                .iter()
                .map(|v| LookupRow {
                    key: v.to_string(),
                    text: v.to_string(),
                    detail: String::new(),
                })
                .collect(),
        )
    }

    pub fn by_key(&self, key: &str) -> Option<&LookupRow> {
        self.index.get(key).map(|&i| &self.rows[i])
    }

    /// The row whose text is exactly `text`, ignoring case, if there's
    /// only one.
    pub fn exact(&self, text: &str) -> Option<&LookupRow> {
        let t = text.trim().to_lowercase();
        let mut found =
            self.rows.iter().filter(|r| r.text.to_lowercase() == t);
        match (found.next(), found.next()) {
            (Some(row), None) => Some(row),
            _ => None,
        }
    }

    /// Rows matching `query`, best first: the text starts with it, then a
    /// word in the text does, then the text contains it, then the detail
    /// does. Ties keep the list's own order.
    pub fn rank(&self, query: &str, limit: usize) -> Vec<&LookupRow> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(u8, usize, &LookupRow)> = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, row)| {
                let text = row.text.to_lowercase();
                let score = if text.starts_with(&q) {
                    0
                } else if text
                    .split_whitespace()
                    .any(|w| w.starts_with(&q))
                {
                    1
                } else if text.contains(&q) {
                    2
                } else if row.detail.to_lowercase().contains(&q) {
                    3
                } else {
                    return None;
                };
                Some((score, i, row))
            })
            .collect();
        scored.sort_by_key(|&(score, i, _)| (score, i));
        scored.into_iter().take(limit).map(|(.., r)| r).collect()
    }
}

/// One row in the popup: its text, a gap, then its detail, faint. The
/// key goes in as the suggestion's `insert`; the field shows the text.
pub(crate) fn suggestion(row: &LookupRow) -> egui::Suggestion {
    let gap =
        egui::Atom::custom(egui::IdSalt::new("gap"), [64.0, 0.0]);
    egui::Suggestion::new(row.key.clone())
        .content((row.text.clone(), gap))
        .description(row.detail.clone())
}

/// Show a popup with text wrapping off, so it's as wide as its longest
/// row instead of the field under it. The popup takes the app's style,
/// not the field's, so the switch is app-wide for this one call.
pub(crate) fn unwrapped<R>(
    ctx: &egui::Context,
    show: impl FnOnce() -> R,
) -> R {
    let before = ctx.global_style().wrap_mode;
    ctx.all_styles_mut(|s| {
        s.wrap_mode = Some(egui::TextWrapMode::Extend);
    });
    let shown = show();
    ctx.all_styles_mut(|s| s.wrap_mode = before);
    shown
}

/// V1's `FindBox`: "Go to:" a record by picking it from `list`. Not bound
/// to a field; returns the key picked, once.
pub fn find_box(
    ui: &mut egui::Ui,
    id_salt: impl egui::AsIdSalt,
    list: &LookupList,
    width: f32,
) -> Option<String> {
    let id = ui.make_persistent_id(id_salt);
    let label = ui.label("Go to:");
    let mut text: String = ui.data_mut(|d| {
        d.get_temp(id.with("text")).unwrap_or_default()
    });
    let ctx = ui.ctx().clone();
    let out = unwrapped(&ctx, || {
        egui::CompletionPopup::new(id)
            .word_boundary(|_| false)
            .show(
                ui,
                egui::TextEdit::singleline(&mut text)
                    .desired_width(width),
                |q| {
                    list.rank(q.text, 12)
                        .into_iter()
                        .map(suggestion)
                        .collect()
                },
            )
    });
    let _ = out.text_edit.response.response.labelled_by(label.id);
    let picked = out.accepted.map(|s| s.insert);
    if picked.is_some() {
        text.clear();
    }
    ui.data_mut(|d| d.insert_temp(id.with("text"), text));
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(names: &[&str]) -> LookupList {
        LookupList::new(
            names
                .iter()
                .enumerate()
                .map(|(i, n)| LookupRow {
                    key: i.to_string(),
                    text: n.to_string(),
                    detail: String::new(),
                })
                .collect(),
        )
    }

    #[test]
    fn a_prefix_beats_a_word_beats_a_substring() {
        let l =
            list(&["Bay Creamery", "Shasta Creamery", "Cream Co"]);
        let texts: Vec<&str> = l
            .rank("cream", 9)
            .iter()
            .map(|r| r.text.as_str())
            .collect();
        assert_eq!(
            texts,
            ["Cream Co", "Bay Creamery", "Shasta Creamery"]
        );
    }

    #[test]
    fn an_exact_match_must_be_the_only_one() {
        let l = list(&["Acme", "acme", "Bolt"]);
        assert!(l.exact("ACME").is_none());
        assert_eq!(l.exact("bolt").unwrap().key, "2");
    }
}
