//! A `BTreeMap` index over a lookup's rows (Chapter 13).
//!
//! The list's `rank` reads every row on every keystroke. This index
//! keeps the search terms, sorted, so a prefix is one range scan: the
//! keys that start with the query are next to each other. Every term is
//! a lowercased word of a row's text, detail or keywords, plus the whole
//! text (a SKU such as `li-05000` is one word, but `cream co` is also
//! findable as `cream co`).
//!
//! What it cannot do is find the middle of a word. `rank` finds
//! `ateway` inside `Gateway`; the index, which looks up where terms
//! start, does not. Candidates come from the index and are scored by
//! the same function `rank` uses, so everything the index returns is
//! ordered exactly as `rank` would order it.

use crate::lookup::{score, LookupRow};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct LookupIndex {
    rows: Vec<LookupRow>,
    /// Term to the rows that have it, in list order.
    terms: BTreeMap<String, Vec<u32>>,
}

impl LookupIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_rows(
        rows: impl IntoIterator<Item = LookupRow>,
    ) -> Self {
        let mut index = Self::new();
        for row in rows {
            index.push(row);
        }
        index
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Add a row at the end of the list, and its terms to the index.
    /// Nothing else is touched: this is the whole cost of a new product.
    pub fn push(&mut self, row: LookupRow) {
        let at = self.rows.len() as u32;
        let mut mine: Vec<String> = Vec::new();
        mine.push(row.text.to_lowercase());
        for field in [&row.text, &row.detail, &row.keywords] {
            mine.extend(
                field.split_whitespace().map(|w| w.to_lowercase()),
            );
        }
        mine.sort();
        mine.dedup();
        for term in mine {
            self.terms.entry(term).or_default().push(at);
        }
        self.rows.push(row);
    }

    /// Rows matching `query`, best first, as `LookupList::rank` orders
    /// them, for every query that starts at the start of some word.
    pub fn rank(&self, query: &str, limit: usize) -> Vec<&LookupRow> {
        let q = query.trim().to_lowercase();
        let Some(first) = q.split_whitespace().next() else {
            return Vec::new();
        };
        // The query's first word is where a match has to start. Walk the
        // keys from there for as long as they begin with it.
        let mut candidates: Vec<u32> = self
            .terms
            .range(first.to_string()..)
            .take_while(|(term, _)| term.starts_with(first))
            .flat_map(|(_, rows)| rows.iter().copied())
            .collect();
        candidates.sort_unstable();
        candidates.dedup();
        let mut scored: Vec<(u8, u32)> = candidates
            .into_iter()
            .filter_map(|i| {
                Some((score(&self.rows[i as usize], &q)?, i))
            })
            .collect();
        scored.sort_unstable();
        scored
            .into_iter()
            .take(limit)
            .map(|(_, i)| &self.rows[i as usize])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lookup::LookupList;

    fn row(
        key: usize,
        text: &str,
        detail: &str,
        keywords: &str,
    ) -> LookupRow {
        LookupRow {
            key: key.to_string(),
            text: text.into(),
            detail: detail.into(),
            keywords: keywords.into(),
        }
    }

    fn rows() -> Vec<LookupRow> {
        vec![
            row(0, "CS-TH050", "Temperature Sensor", "Sensor 1.2.0"),
            row(1, "CS-GW200", "LoRaWAN Gateway", "Gateway uplink"),
            row(2, "Cream Co", "", ""),
            row(3, "Bay Creamery", "", ""),
            row(4, "CS-TH0500", "Humidity Sensor", "Sensor 2.0.0"),
        ]
    }

    fn keys(rows: Vec<&LookupRow>) -> Vec<String> {
        rows.iter().map(|r| r.key.clone()).collect()
    }

    #[test]
    fn it_orders_a_prefix_as_the_scan_does() {
        let list = LookupList::new(rows());
        let index = LookupIndex::from_rows(rows());
        for q in
            ["cs-", "cs-th050", "cream", "sensor", "uplink", "lo"]
        {
            assert_eq!(
                keys(index.rank(q, 9)),
                keys(list.rank(q, 9)),
                "query {q:?}"
            );
        }
    }

    #[test]
    fn it_finds_a_phrase_that_starts_a_word() {
        let index = LookupIndex::from_rows(rows());
        assert_eq!(keys(index.rank("lorawan gate", 9)), ["1"]);
        assert_eq!(keys(index.rank("cream co", 9)), ["2"]);
    }

    #[test]
    fn it_does_not_find_the_middle_of_a_word() {
        let list = LookupList::new(rows());
        let index = LookupIndex::from_rows(rows());
        assert_eq!(keys(list.rank("ateway", 9)), ["1"]);
        assert!(index.rank("ateway", 9).is_empty());
    }

    #[test]
    fn a_pushed_row_is_found_at_once_and_after_the_rest() {
        let mut index = LookupIndex::from_rows(rows());
        assert_eq!(keys(index.rank("cs-th0", 9)), ["0", "4"]);
        index.push(row(5, "CS-TH051", "Probe", ""));
        assert_eq!(keys(index.rank("cs-th0", 9)), ["0", "4", "5"]);
        assert_eq!(index.len(), 6);
    }

    #[test]
    fn nothing_for_an_empty_or_unmatched_query() {
        let index = LookupIndex::from_rows(rows());
        assert!(index.rank("   ", 9).is_empty());
        assert!(index.rank("zzzzq", 9).is_empty());
    }
}
