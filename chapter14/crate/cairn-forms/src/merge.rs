//! The three-way comparison behind a conflict (Chapter 12). Three copies
//! of one record: `original`, as the rep read it; `mine`, as the rep has
//! it now; `theirs`, as Postgres has it now, after another rep saved.
//! Field by field, using the labels `#[derive(Form)]` already knows:
//!
//! - only they changed it: it comes in;
//! - only the rep changed it: it stays;
//! - both changed it to the same thing: nothing to settle;
//! - both changed it, differently: the rep is asked, and only about that
//!   field.
//!
//! The merged record starts as theirs, so its key and version are the
//! ones Postgres has now, and the save that sends it is checked against
//! them.

use crate::field::{FieldSpec, Kind};
use crate::record::Form;

/// Which copy of a field's value to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Mine,
    Theirs,
}

/// A field both reps changed, differently.
#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    pub field: usize,
    pub label: String,
    pub kind: Kind,
    pub original: String,
    pub mine: String,
    pub theirs: String,
    pub choice: Option<Side>,
}

/// A field only one side changed, and what the merge did with it.
#[derive(Debug, Clone, PartialEq)]
pub struct Settled {
    pub label: String,
    pub kind: Kind,
    pub text: String,
    /// `Theirs`: it came in. `Mine`: it stays.
    pub side: Side,
}

#[derive(Debug, Clone)]
pub struct Merge<T: Form> {
    /// Theirs, with every field only the rep changed put back, and each
    /// field asked about set as it was chosen.
    pub merged: T,
    pub settled: Vec<Settled>,
    pub asks: Vec<Ask>,
}

/// Compare `mine` and `theirs` against `original`.
pub fn three_way<T: Form>(
    original: &T,
    mine: &T,
    theirs: &T,
) -> Merge<T> {
    let mut merged = theirs.clone();
    let (mut settled, mut asks) = (Vec::new(), Vec::new());
    for (i, spec) in T::FIELDS.iter().enumerate() {
        let text = |r: &T| r.field(i).text();
        let (o, m, t) = (text(original), text(mine), text(theirs));
        if m == t {
            continue;
        }
        let side = if m == o {
            Some(Side::Theirs)
        } else if t == o {
            Some(Side::Mine)
        } else {
            None
        };
        match side {
            Some(Side::Mine) => {
                let _ = merged.field_mut(i).set_text(&m);
                settled.push(settled_of(spec, m, Side::Mine));
            }
            Some(Side::Theirs) => {
                settled.push(settled_of(spec, t, Side::Theirs));
            }
            // A key the rep never sees: the rep's own copy stands.
            None if spec.hidden => {
                let _ = merged.field_mut(i).set_text(&m);
            }
            None => asks.push(Ask {
                field: i,
                label: spec.label.to_string(),
                kind: spec.kind,
                original: o,
                mine: m,
                theirs: t,
                choice: None,
            }),
        }
    }
    // A hidden field that only they changed isn't news to the rep.
    settled.retain(|s| !s.label.is_empty());
    Merge {
        merged,
        settled,
        asks,
    }
}

fn settled_of(spec: &FieldSpec, text: String, side: Side) -> Settled {
    Settled {
        label: if spec.hidden {
            String::new()
        } else {
            spec.label.to_string()
        },
        kind: spec.kind,
        text,
        side,
    }
}

impl<T: Form> Merge<T> {
    /// Keep one side's value for the field asked about.
    pub fn choose(&mut self, ask: usize, side: Side) {
        let a = &mut self.asks[ask];
        a.choice = Some(side);
        let text = match side {
            Side::Mine => a.mine.clone(),
            Side::Theirs => a.theirs.clone(),
        };
        let _ = self.merged.field_mut(a.field).set_text(&text);
    }

    /// Whether every question has an answer: the merged record can be
    /// saved.
    pub fn resolved(&self) -> bool {
        self.asks.iter().all(|a| a.choice.is_some())
    }
}

// ---------------------------------------------------------------------
// Rows. A record's subforms (lines, contacts, activities, payments) are
// rows with a key and a version of their own, and the same three copies:
// as the rep read them, as the rep has them, as Postgres has them now.

/// A subform row with a key and a version of its own.
pub trait Keyed: Form {
    fn key(&self) -> Option<i32>;
    fn version(&self) -> i32;
    fn set_version(&mut self, v: i32);
    /// The row as a new one: no key, no version.
    fn forget(&mut self);
}

/// What a question is about, and what each answer means, in the words
/// the dialog prints on its buttons.
#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    pub label: String,
    pub mine: String,
    pub theirs: String,
    /// What the field was when the rep read it, when it has one.
    pub original: Option<String>,
    pub choice: Option<Side>,
}

/// One part of a conflict the dialog settles: the record's own fields,
/// or one subform's rows.
pub trait Section {
    fn name(&self) -> String;
    /// What was settled without asking, as lines of text.
    fn settled(&self) -> Vec<String>;
    fn questions(&self) -> Vec<Question>;
    fn answer(&mut self, question: usize, side: Side);
    fn resolved(&self) -> bool {
        self.questions().iter().all(|q| q.choice.is_some())
    }
}

fn buttons(kind: Kind, a: &Ask) -> Question {
    let show = |t: &str| {
        let t = crate::field::display(kind, t);
        if t.is_empty() {
            "(blank)".to_string()
        } else {
            t
        }
    };
    Question {
        label: a.label.clone(),
        mine: format!("Keep mine: {}", show(&a.mine)),
        theirs: format!("Take theirs: {}", show(&a.theirs)),
        original: Some(show(&a.original)),
        choice: a.choice,
    }
}

fn settled_line(s: &Settled) -> String {
    let t = crate::field::display(s.kind, &s.text);
    let t = if t.is_empty() { "(blank)".into() } else { t };
    let what = match s.side {
        Side::Theirs => "came in from the other rep",
        Side::Mine => "stays as you changed it",
    };
    format!("{}: {t} ({what})", s.label)
}

impl<T: Form> Section for Merge<T> {
    fn name(&self) -> String {
        String::new()
    }

    fn settled(&self) -> Vec<String> {
        self.settled
            .iter()
            .filter(|s| !s.label.is_empty())
            .map(settled_line)
            .collect()
    }

    fn questions(&self) -> Vec<Question> {
        self.asks.iter().map(|a| buttons(a.kind, a)).collect()
    }

    fn answer(&mut self, question: usize, side: Side) {
        self.choose(question, side);
    }
}

/// Whether two rows hold the same field values: the key and version
/// aren't fields, and a version moved by someone else's save doesn't
/// make the row different.
pub fn same_fields<R: Form>(a: &R, b: &R) -> bool {
    (0..R::FIELDS.len())
        .all(|i| a.field(i).text() == b.field(i).text())
}

enum Entry<R: Keyed> {
    /// A row nobody here touched: whatever Postgres has.
    Taken(R),
    /// A row the rep added.
    New(R),
    /// A row the other rep added.
    TheirNew(R),
    /// The rep edited it, the other rep didn't: written, at the version
    /// Postgres has now.
    Edited(R),
    /// Both edited it: the fields are compared.
    Fields(Merge<R>),
    /// The rep edited it and the other rep deleted it.
    EditedThenDeleted {
        label: String,
        mine: R,
        choice: Option<Side>,
    },
    /// The rep deleted it and the other rep changed it.
    DeletedThenChanged {
        label: String,
        theirs: R,
        choice: Option<Side>,
    },
    /// The rep deleted it, nobody changed it: the delete stands.
    Removed(R),
}

/// One subform's rows, three ways.
pub struct RowsMerge<R: Keyed> {
    name: &'static str,
    entries: Vec<Entry<R>>,
}

/// Compare a subform's rows. `originals` are the rows as the rep read
/// them (the ones the rep deleted included), `mine` the rows now, `gone`
/// the rows the rep deleted, `theirs` the rows as Postgres has them.
/// `describe` names a row the rep no longer sees by position.
pub fn merge_rows<R: Keyed>(
    name: &'static str,
    originals: &[R],
    mine: &[R],
    gone: &[R],
    theirs: &[R],
    describe: fn(&R) -> String,
) -> RowsMerge<R> {
    let original =
        |k: i32| originals.iter().find(|r| r.key() == Some(k));
    let other = |k: i32| theirs.iter().find(|r| r.key() == Some(k));
    let mut entries = Vec::new();
    for (n, row) in mine.iter().enumerate() {
        let pos = format!("{name} {}", n + 1);
        let Some(k) = row.key() else {
            entries.push(Entry::New(row.clone()));
            continue;
        };
        let (Some(o), t) = (original(k), other(k)) else {
            entries.push(Entry::New(row.clone()));
            continue;
        };
        let edited = !same_fields(row, o);
        match (t, edited) {
            (None, false) => {} // they deleted what the rep left alone
            (None, true) => entries.push(Entry::EditedThenDeleted {
                label: format!("{pos}: the other rep deleted it"),
                mine: row.clone(),
                choice: None,
            }),
            (Some(t), false) => entries.push(Entry::Taken(t.clone())),
            (Some(t), true) if same_fields(t, o) => {
                let mut r = row.clone();
                r.set_version(t.version());
                entries.push(Entry::Edited(r));
            }
            (Some(t), true) => {
                let mut merge = three_way(o, row, t);
                for a in &mut merge.asks {
                    a.label = format!("{pos}, {}", a.label);
                }
                for s in &mut merge.settled {
                    s.label = format!("{pos}, {}", s.label);
                }
                entries.push(Entry::Fields(merge));
            }
        }
    }
    for d in gone {
        let Some(k) = d.key() else { continue };
        let Some(t) = other(k) else { continue };
        if same_fields(t, original(k).unwrap_or(d)) {
            let mut r = d.clone();
            r.set_version(t.version());
            entries.push(Entry::Removed(r));
        } else {
            entries.push(Entry::DeletedThenChanged {
                label: format!(
                    "{name} {}: you deleted it, the other rep changed it",
                    describe(d)
                ),
                theirs: t.clone(),
                choice: None,
            });
        }
    }
    for t in theirs {
        let Some(k) = t.key() else { continue };
        let known = originals.iter().any(|r| r.key() == Some(k))
            || mine.iter().any(|r| r.key() == Some(k));
        if !known {
            entries.push(Entry::TheirNew(t.clone()));
        }
    }
    RowsMerge { name, entries }
}

impl<R: Keyed> RowsMerge<R> {
    /// The rows as they will stand: the rep's edits, the other rep's
    /// changes, and the answers so far.
    pub fn rows(&self) -> Vec<R> {
        let mut out = Vec::new();
        for e in &self.entries {
            match e {
                Entry::Taken(r)
                | Entry::New(r)
                | Entry::TheirNew(r)
                | Entry::Edited(r) => out.push(r.clone()),
                Entry::Fields(merge) => {
                    out.push(merge.merged.clone())
                }
                Entry::EditedThenDeleted {
                    mine,
                    choice: Some(Side::Mine),
                    ..
                } => {
                    // Put back as a new row.
                    let mut r = mine.clone();
                    r.forget();
                    out.push(r);
                }
                Entry::DeletedThenChanged {
                    theirs,
                    choice: Some(Side::Theirs),
                    ..
                } => out.push(theirs.clone()),
                _ => {}
            }
        }
        out
    }

    /// The rows to delete, each at the version Postgres has now.
    pub fn removed(&self) -> Vec<R> {
        let mut out = Vec::new();
        for e in &self.entries {
            match e {
                Entry::Removed(r) => out.push(r.clone()),
                Entry::DeletedThenChanged {
                    theirs,
                    choice: Some(Side::Mine),
                    ..
                } => out.push(theirs.clone()),
                _ => {}
            }
        }
        out
    }

    /// The keys of the saved rows to write.
    pub fn written(&self) -> Vec<i32> {
        self.entries
            .iter()
            .filter_map(|e| match e {
                Entry::Edited(r) => r.key(),
                Entry::Fields(merge) => merge.merged.key(),
                _ => None,
            })
            .collect()
    }
}

impl<R: Keyed> Section for RowsMerge<R> {
    fn name(&self) -> String {
        format!("{}s", self.name)
    }

    fn settled(&self) -> Vec<String> {
        let mut out = Vec::new();
        for e in &self.entries {
            match e {
                Entry::TheirNew(_) => out.push(format!(
                    "A {} the other rep added comes in",
                    self.name.to_lowercase()
                )),
                Entry::Removed(_) => out.push(format!(
                    "A {} you deleted stays deleted",
                    self.name.to_lowercase()
                )),
                Entry::Fields(merge) => {
                    out.extend(Section::settled(merge))
                }
                _ => {}
            }
        }
        out
    }

    fn questions(&self) -> Vec<Question> {
        let mut out = Vec::new();
        for e in &self.entries {
            match e {
                Entry::Fields(merge) => out.extend(merge.questions()),
                Entry::EditedThenDeleted {
                    label, choice, ..
                } => out.push(Question {
                    label: label.clone(),
                    mine: "Keep mine: add it again".into(),
                    theirs: "Take theirs: leave it deleted".into(),
                    original: None,
                    choice: *choice,
                }),
                Entry::DeletedThenChanged {
                    label, choice, ..
                } => out.push(Question {
                    label: label.clone(),
                    mine: "Keep mine: delete it".into(),
                    theirs: "Take theirs: keep their version".into(),
                    original: None,
                    choice: *choice,
                }),
                _ => {}
            }
        }
        out
    }

    fn answer(&mut self, mut question: usize, side: Side) {
        for e in &mut self.entries {
            match e {
                Entry::Fields(merge) => {
                    let n = merge.asks.len();
                    if question < n {
                        merge.choose(question, side);
                        return;
                    }
                    question -= n;
                }
                Entry::EditedThenDeleted { choice, .. }
                | Entry::DeletedThenChanged { choice, .. } => {
                    if question == 0 {
                        *choice = Some(side);
                        return;
                    }
                    question -= 1;
                }
                _ => {}
            }
        }
    }
}
