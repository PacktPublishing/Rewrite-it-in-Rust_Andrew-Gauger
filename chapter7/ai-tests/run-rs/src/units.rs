//! The unit graph the harness walks: each VBA procedure, each module's declarations,
//! each saved query and each table, with the references between them.

use std::cmp::Reverse;
use std::collections::HashMap;

use regex::Regex;

use crate::tools::{run_cmd, strip_banner, Module};

/// Shown under every saved query. mdb-queries prints `FROM [qryOrderTotals],[qryOrderPayments]`
/// for a LEFT JOIN with an ON clause, and drops GROUP BY and AS. Without this, run.py's model
/// read the comma list as an inner join and wrote a rule the database doesn't have.
const QUERY_NOTE: &str = "-- Rebuilt by mdbtools from the saved query's definition. It leaves out join \
    types and ON conditions (joined tables are listed with commas), GROUP BY, HAVING, and AS aliases \
    for tables and columns, so the query Access runs may join, group and name things differently.";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Vba,
    Query,
    Table,
}

pub struct Unit {
    pub id: String,
    pub kind: Kind,
    /// The VBA module a procedure or declarations unit lives in; None for queries and tables.
    pub module: Option<String>,
    pub label: String,
    /// Last source line, for VBA units.
    pub last: usize,
    pub text: String,
    /// Candidate edges: other units named in this one.
    pub refs: Vec<String>,
}

/// Units in discovery order, with lookup by id.
#[derive(Default)]
pub struct Units {
    list: Vec<Unit>,
    index: HashMap<String, usize>,
}

impl Units {
    pub fn get(&self, id: &str) -> Option<&Unit> {
        self.index.get(id).map(|&i| &self.list[i])
    }

    pub fn contains(&self, id: &str) -> bool {
        self.index.contains_key(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Unit> {
        self.list.iter()
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    fn insert(&mut self, unit: Unit) {
        match self.index.get(&unit.id) {
            Some(&i) => self.list[i] = unit,
            None => {
                self.index.insert(unit.id.clone(), self.list.len());
                self.list.push(unit);
            }
        }
    }
}

/// A unit id as a file name that is also a valid Rust test-crate name.
pub fn slug(unit: &str) -> String {
    let lower = unit.to_lowercase();
    let runs = Regex::new(r"[^a-z0-9]+").unwrap();
    runs.replace_all(&lower, "_").trim_matches('_').to_string()
}

/// One unit per procedure (with the comment block just above it), plus one for the
/// module's declarations: everything outside a procedure. Indexes are 0-based lines.
pub fn split_module(module: &str, lines: &[String]) -> Vec<(String, Vec<usize>)> {
    let proc_start = Regex::new(
        r"(?i)^\s*(?:(?:Public|Private|Friend)\s+)?(?:Static\s+)?(?:Function|Sub|Property\s+(?:Get|Let|Set))\s+(\w+)",
    ).unwrap();
    let proc_end = Regex::new(r"(?i)^\s*End\s+(?:Function|Sub|Property)\b").unwrap();
    let mut procs: Vec<(String, usize, usize)> = Vec::new();
    let mut open: Option<(usize, String)> = None;
    for (i, line) in lines.iter().enumerate() {
        match &open {
            None => {
                if let Some(c) = proc_start.captures(line) {
                    let mut start = i;
                    while start > 0 && lines[start - 1].trim_start().starts_with('\'') {
                        start -= 1; // the comment block above belongs to the procedure
                    }
                    open = Some((start, c[1].to_string()));
                }
            }
            Some((start, name)) => {
                if proc_end.is_match(line) {
                    procs.push((name.clone(), *start, i));
                    open = None;
                }
            }
        }
    }
    let mut inside = vec![false; lines.len()];
    for (_, a, b) in &procs {
        inside[*a..=*b].iter_mut().for_each(|x| *x = true);
    }
    let decl = (0..lines.len()).filter(|&i| !inside[i]).collect();
    let mut units = vec![(format!("vba:{module}"), decl)];
    units.extend(
        procs
            .into_iter()
            .map(|(name, a, b)| (format!("vba:{module}.{name}"), (a..=b).collect())),
    );
    units
}

pub fn build(vba: &[Module], db: &str) -> Units {
    let mut units = Units::default();
    for (module, lines) in vba {
        for (id, idx) in split_module(module, lines) {
            let text = idx
                .iter()
                .map(|&i| format!("{}: {}", i + 1, lines[i]))
                .collect::<Vec<_>>()
                .join("\n");
            let label = match (idx.first(), idx.last()) {
                _ if id == format!("vba:{module}") => {
                    format!("{module} declarations, everything outside its procedures")
                }
                (Some(a), Some(b)) => format!("{module} lines {}-{}", a + 1, b + 1),
                _ => module.clone(),
            };
            let last = idx.last().map_or(0, |i| i + 1);
            units.insert(Unit {
                id,
                kind: Kind::Vba,
                module: Some(module.clone()),
                label,
                last,
                text,
                refs: vec![],
            });
        }
    }
    for name in run_cmd("mdb-queries", &["-1", db], None).split_whitespace() {
        let text = format!(
            "{}\n{QUERY_NOTE}",
            run_cmd("mdb-queries", &[db, name], None).trim()
        );
        units.insert(Unit {
            id: format!("query:{name}"),
            kind: Kind::Query,
            module: None,
            label: "saved query".into(),
            last: 0,
            text,
            refs: vec![],
        });
    }
    for name in run_cmd("mdb-tables", &["-1", db], None).split_whitespace() {
        let text = strip_banner(&run_cmd("mdb-schema", &["-T", name, db, "postgres"], None))
            .trim()
            .to_string();
        units.insert(Unit {
            id: format!("table:{name}"),
            kind: Kind::Table,
            module: None,
            label: "table".into(),
            last: 0,
            text,
            refs: vec![],
        });
    }
    let modules: Vec<&str> = vba.iter().map(|(m, _)| m.as_str()).collect();
    find_references(&mut units, &modules);
    units
}

/// The name a unit is referred to by: the last segment of its id.
fn short_name(id: &str) -> &str {
    let rest = id.split_once(':').map_or(id, |(_, r)| r);
    rest.rsplit('.').next().unwrap_or(rest)
}

/// Candidate edges: every other unit whose name appears as a word in this one.
fn find_references(units: &mut Units, modules: &[&str]) {
    let mut names: Vec<&str> = Vec::new();
    let mut by_name: HashMap<&str, Vec<String>> = HashMap::new();
    for unit in &units.list {
        let name = short_name(&unit.id);
        if !by_name.contains_key(name) {
            names.push(name);
        }
        by_name.entry(name).or_default().push(unit.id.clone());
    }
    // Module names are too common to mean a reference, and one- or two-letter helpers match every SQL alias.
    names.retain(|n| !modules.contains(n) && n.chars().count() >= 3);
    names.sort_by_key(|n| Reverse(n.chars().count())); // stable: longest first, then discovery order
    let alternatives: Vec<String> = names.iter().map(|n| regex::escape(n)).collect();
    let word = Regex::new(&format!(r"\b({})\b", alternatives.join("|"))).unwrap();
    let numbering = Regex::new(r"(?m)^\d+: ").unwrap();
    let module_of: HashMap<String, Option<String>> = units
        .list
        .iter()
        .map(|u| (u.id.clone(), u.module.clone()))
        .collect();

    let mut all_refs = Vec::with_capacity(units.list.len());
    for unit in &units.list {
        let body = numbering.replace_all(&unit.text, "");
        let mut found: Vec<&str> = Vec::new();
        for m in word.find_iter(&body) {
            if !found.contains(&m.as_str()) {
                found.push(m.as_str());
            }
        }
        let mut refs: Vec<String> = Vec::new();
        for name in found {
            let targets: Vec<&String> = by_name[name].iter().filter(|t| **t != unit.id).collect();
            let same_module: Vec<&String> = targets
                .iter()
                .copied()
                .filter(|t| module_of[*t] == unit.module)
                .collect();
            let chosen = if same_module.is_empty() {
                targets
            } else {
                same_module
            };
            for t in chosen {
                if !refs.contains(t) {
                    refs.push(t.clone());
                }
            }
        }
        all_refs.push(refs);
    }
    for (unit, refs) in units.list.iter_mut().zip(all_refs) {
        unit.refs = refs;
    }
}

/// Names only, grouped, for the survey round.
pub fn inventory(units: &Units, vba: &[Module]) -> String {
    let mut lines = Vec::new();
    for (module, src) in vba {
        let prefix = format!("vba:{module}.");
        let procs: Vec<&str> = units
            .iter()
            .filter_map(|u| u.id.strip_prefix(&prefix))
            .collect();
        lines.push(format!("VBA module {module} ({} lines, {} procedures; unit vba:{module} is its declarations): {}",
                           src.len(), procs.len(), procs.join(", ")));
    }
    let names = |prefix: &str| {
        units
            .iter()
            .filter_map(|u| u.id.strip_prefix(prefix))
            .collect::<Vec<_>>()
            .join(", ")
    };
    lines.push(format!("Saved queries: {}", names("query:")));
    lines.push(format!("Tables: {}", names("table:")));
    lines.push("Unit ids look like vba:CairnForms.NextNumber, vba:CairnForms, query:qryQuoteTotals, table:Orders.".into());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(String::from).collect()
    }

    #[test]
    fn slug_makes_crate_safe_names() {
        assert_eq!(
            slug("vba:CairnForms.NextNumber"),
            "vba_cairnforms_nextnumber"
        );
        assert_eq!(slug("query:qryRMARouting"), "query_qryrmarouting");
    }

    #[test]
    fn split_module_keeps_comment_blocks_with_their_procedure() {
        let src = lines("Option Explicit\n' next number\n' per year\nPublic Function NextNumber()\n  x = 1\nEnd Function\nPrivate x As Long");
        let units = split_module("CairnForms", &src);
        assert_eq!(units[0], ("vba:CairnForms".to_string(), vec![0, 6]));
        assert_eq!(
            units[1],
            ("vba:CairnForms.NextNumber".to_string(), vec![1, 2, 3, 4, 5])
        );
    }

    #[test]
    fn split_module_reads_every_procedure_kind() {
        let src = lines("Sub A()\nEnd Sub\nPrivate Static Function B()\nEnd Function\nPublic Property Get C()\nEnd Property");
        let ids: Vec<String> = split_module("M", &src)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(ids, ["vba:M", "vba:M.A", "vba:M.B", "vba:M.C"]);
    }

    #[test]
    fn short_name_is_the_last_segment() {
        assert_eq!(short_name("vba:CairnForms.NextNumber"), "NextNumber");
        assert_eq!(short_name("query:qryQuoteTotals"), "qryQuoteTotals");
        assert_eq!(short_name("vba:CairnForms"), "CairnForms");
    }
}
