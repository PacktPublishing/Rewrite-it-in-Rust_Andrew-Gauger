//! A trigram index over a lookup's rows, with early exit (Chapter 13).
//!
//! The index only chooses which rows to look at. Every row is cut into
//! overlapping three-byte pieces, lowercased, one field at a time, and
//! each piece points at the rows that contain it. A query of three
//! bytes or more must have every one of its pieces inside one field of
//! any row that contains it, so the rows holding all of them are a
//! superset of the true matches. [`score`] then decides, with the same
//! function `LookupList::rank` uses, so the answer and its order are
//! `rank`'s, for every query.
//!
//! Two lists of pieces are kept apart: the text field's, and the detail
//! and keywords'. That is what allows an early exit. `rank` orders by
//! tier and then by list position, and the tiers split by field: 0, 1
//! and 2 are matches in the text, 3 a match in the other fields. So the
//! text's candidates are walked first, in list order, and the others
//! only if the text did not fill the list. Walking in list order means
//! a later row can never displace an earlier one of the same tier, so
//! the walk stops the moment the list is full.
//!
//! A query shorter than three bytes has no pieces. The index walks the
//! rows instead, with the same early exit, and says so in its
//! [`Profile`].

use crate::lookup::{score_folded, LookupRow};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// Where the time and the rows went for one query.
#[derive(Debug, Default, Clone)]
pub struct Profile {
    /// The query had fewer than three bytes: nothing to look up, so the
    /// rows were walked instead.
    pub fell_back: bool,
    /// Distinct trigrams in the query.
    pub trigrams: usize,
    /// Rows checked with `score`. The scan checks every row.
    pub checked: usize,
    /// Of those, the rows `score` accepted and the index kept.
    pub kept: usize,
    /// The walk stopped with rows left unchecked.
    pub exited_early: bool,
    /// Walking the posting lists and checking each row, together: the
    /// two are one loop, so there is no honest way to split them.
    pub walk: Duration,
    /// Putting the kept rows in order.
    pub order: Duration,
}

/// What the index holds, for sizing.
#[derive(Debug, Default, Clone)]
pub struct Stats {
    pub rows: usize,
    /// Distinct trigrams in the text field's lists and in the others'.
    pub text_trigrams: usize,
    pub other_trigrams: usize,
    pub postings: usize,
    pub longest: usize,
    /// Bytes in the posting lists themselves (4 per posting).
    pub posting_bytes: usize,
}

#[derive(Debug, Clone, Default)]
pub struct TrigramIndex {
    rows: Vec<LookupRow>,
    /// Each row's text, detail and keywords, lowercased once, here,
    /// so checking a candidate allocates nothing.
    folded: Vec<[String; 3]>,
    /// Trigram to the rows whose text has it, ascending.
    text: BTreeMap<[u8; 3], Vec<u32>>,
    /// Trigram to the rows whose detail or keywords have it, ascending.
    other: BTreeMap<[u8; 3], Vec<u32>>,
}

fn trigrams(field: &str, out: &mut Vec<[u8; 3]>) {
    let bytes = field.as_bytes();
    out.extend(bytes.windows(3).map(|w| [w[0], w[1], w[2]]));
}

fn distinct(mut grams: Vec<[u8; 3]>) -> Vec<[u8; 3]> {
    grams.sort_unstable();
    grams.dedup();
    grams
}

/// The rows in every one of `lists`, in list order, one at a time. It
/// walks the shortest list and looks each row up in the others, from
/// where the last look-up ended, so stopping early costs nothing for
/// the rows it never reached.
struct Common<'a> {
    first: &'a [u32],
    at: usize,
    rest: Vec<(&'a [u32], usize)>,
}

impl<'a> Common<'a> {
    fn new(mut lists: Vec<&'a [u32]>) -> Self {
        lists.sort_by_key(|l| l.len());
        let first = lists.remove(0);
        Common {
            first,
            at: 0,
            rest: lists.into_iter().map(|l| (l, 0)).collect(),
        }
    }
}

impl Iterator for Common<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        'rows: loop {
            let row = *self.first.get(self.at)?;
            self.at += 1;
            for (list, from) in &mut self.rest {
                match list[*from..].binary_search(&row) {
                    Ok(i) => *from += i,
                    Err(i) => {
                        *from += i;
                        if *from >= list.len() {
                            // Past the end of this list: no later row
                            // can be in it.
                            return None;
                        }
                        continue 'rows;
                    }
                }
            }
            return Some(row);
        }
    }
}

/// The best `limit` rows by tier and then list position, kept as rows
/// arrive in list position order. A row of a given tier can only be
/// displaced by a better tier, never by a later row, so a full bucket
/// ignores the rows that come after it.
struct Best {
    limit: usize,
    tiers: [Vec<u32>; 4],
}

impl Best {
    fn new(limit: usize) -> Self {
        Best {
            limit,
            tiers: Default::default(),
        }
    }

    fn keep(&mut self, tier: u8, row: u32) {
        let bucket = &mut self.tiers[tier as usize];
        if bucket.len() < self.limit {
            bucket.push(row);
        }
    }

    /// Nothing can beat a full list of tier 0.
    fn full(&self) -> bool {
        self.tiers[0].len() >= self.limit
    }

    /// Rows already in the first three tiers, which the other fields
    /// cannot outrank.
    fn in_text_tiers(&self) -> usize {
        self.tiers[..3].iter().map(Vec::len).sum()
    }

    fn kept(&self) -> usize {
        self.tiers.iter().map(Vec::len).sum()
    }

    fn ordered(self) -> impl Iterator<Item = u32> {
        let limit = self.limit;
        self.tiers.into_iter().flatten().take(limit)
    }
}

impl TrigramIndex {
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

    /// Add a row at the end of the list. Its trigrams go onto the end
    /// of their lists, so every list stays ascending with no sorting.
    pub fn push(&mut self, row: LookupRow) {
        let at = self.rows.len() as u32;
        let folded = [
            row.text.to_lowercase(),
            row.detail.to_lowercase(),
            row.keywords.to_lowercase(),
        ];
        let mut text = Vec::new();
        trigrams(&folded[0], &mut text);
        for g in distinct(text) {
            self.text.entry(g).or_default().push(at);
        }
        let mut other = Vec::new();
        trigrams(&folded[1], &mut other);
        trigrams(&folded[2], &mut other);
        for g in distinct(other) {
            self.other.entry(g).or_default().push(at);
        }
        self.folded.push(folded);
        self.rows.push(row);
    }

    pub fn stats(&self) -> Stats {
        let lists = || self.text.values().chain(self.other.values());
        let postings: usize = lists().map(Vec::len).sum();
        Stats {
            rows: self.rows.len(),
            text_trigrams: self.text.len(),
            other_trigrams: self.other.len(),
            postings,
            longest: lists().map(Vec::len).max().unwrap_or(0),
            posting_bytes: postings * std::mem::size_of::<u32>(),
        }
    }

    pub fn rank(&self, query: &str, limit: usize) -> Vec<&LookupRow> {
        self.rank_profiled(query, limit).0
    }

    fn check(&self, row: u32, q: &str) -> Option<u8> {
        let [text, detail, keywords] = &self.folded[row as usize];
        score_folded(text, detail, keywords, q)
    }

    /// The lists for `wanted` in `map`, or `None` if some piece is in
    /// no row at all (then nothing can match).
    fn lists<'a>(
        map: &'a BTreeMap<[u8; 3], Vec<u32>>,
        wanted: &[[u8; 3]],
    ) -> Option<Vec<&'a [u32]>> {
        wanted
            .iter()
            .map(|g| map.get(g).map(Vec::as_slice))
            .collect()
    }

    pub fn rank_profiled(
        &self,
        query: &str,
        limit: usize,
    ) -> (Vec<&LookupRow>, Profile) {
        let mut p = Profile::default();
        let q = query.trim().to_lowercase();
        if q.is_empty() || limit == 0 {
            return (Vec::new(), p);
        }
        let mut best = Best::new(limit);

        let t = Instant::now();
        let mut wanted = Vec::new();
        trigrams(&q, &mut wanted);
        let wanted = distinct(wanted);
        if wanted.is_empty() {
            // Under three bytes: walk every row, in list order.
            p.fell_back = true;
            for row in 0..self.rows.len() as u32 {
                p.checked += 1;
                if let Some(tier) = self.check(row, &q) {
                    best.keep(tier, row);
                    if best.full() {
                        break;
                    }
                }
            }
            p.exited_early = p.checked < self.rows.len();
        } else {
            p.trigrams = wanted.len();
            // Tiers 0 to 2: the text field, in list order.
            if let Some(lists) = Self::lists(&self.text, &wanted) {
                for row in Common::new(lists) {
                    p.checked += 1;
                    if let Some(tier @ 0..=2) = self.check(row, &q) {
                        best.keep(tier, row);
                        if best.full() {
                            break;
                        }
                    }
                }
            }
            // Tier 3: only if the text left room, and only as many as
            // there is room for.
            let room = limit.saturating_sub(best.in_text_tiers());
            if !best.full() && room > 0 {
                if let Some(lists) = Self::lists(&self.other, &wanted)
                {
                    let mut taken = 0;
                    for row in Common::new(lists) {
                        p.checked += 1;
                        // A row the text matched is already counted.
                        if let Some(3) = self.check(row, &q) {
                            best.keep(3, row);
                            taken += 1;
                            if taken == room {
                                p.exited_early = true;
                                break;
                            }
                        }
                    }
                }
            } else {
                p.exited_early = true;
            }
        }
        p.walk = t.elapsed();
        p.kept = best.kept();

        let t = Instant::now();
        let rows =
            best.ordered().map(|i| &self.rows[i as usize]).collect();
        p.order = t.elapsed();
        (rows, p)
    }
}
