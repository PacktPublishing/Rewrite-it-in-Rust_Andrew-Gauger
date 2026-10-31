//! Walk state (the queue, what was visited, what was saved) and the brief a round starts from.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;

use regex::Regex;

use crate::tools::Module;
use crate::units::{inventory, slug, Kind, Units};

pub const OVERVIEW: &str = "overview";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Notes,
    Tests,
    Finished,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Notes => "notes",
            Phase::Tests => "tests",
            Phase::Finished => "finished",
        }
    }
}

/// A queue entry: the unit, the unit whose notes led to it, and its breadth-first level.
#[derive(Clone)]
pub struct Item {
    pub unit: String,
    pub parent: Option<String>,
    pub level: usize,
}

/// What the walk recorded about one unit.
#[derive(Default)]
pub struct Info {
    pub parent: Option<String>,
    pub level: usize,
    pub follow: Vec<String>,
    pub has_rules: bool,
    /// Position in the notes order; set once the unit's notes are saved.
    pub index: Option<usize>,
    pub tests: Option<usize>,
    pub skipped: Option<String>,
}

pub struct Walk {
    pub phase: Phase,
    pub queue: VecDeque<Item>,
    pub seen: HashSet<String>,
    /// Units in the order their notes were saved.
    pub order: Vec<String>,
    info: HashMap<String, Info>,
    info_order: Vec<String>,
    pub current: Option<Item>,
    pub attempts: u32,
    /// The current unit's file was saved this round.
    pub saved: bool,
    /// Over-long notes refused this round; the same text sent again is cut and saved.
    pub rejected_notes: Option<String>,
    pub rounds: usize,
    pub test_queue: VecDeque<Item>,
}

impl Walk {
    pub fn new() -> Self {
        Walk {
            phase: Phase::Notes,
            queue: VecDeque::from([Item {
                unit: OVERVIEW.into(),
                parent: None,
                level: 0,
            }]),
            seen: HashSet::from([OVERVIEW.to_string()]),
            order: Vec::new(),
            info: HashMap::new(),
            info_order: Vec::new(),
            current: None,
            attempts: 0,
            saved: false,
            rejected_notes: None,
            rounds: 0,
            test_queue: VecDeque::new(),
        }
    }

    pub fn info(&self, unit: &str) -> Option<&Info> {
        self.info.get(unit)
    }

    /// The unit's record, created on first use (Python's setdefault).
    pub fn entry(&mut self, unit: &str) -> &mut Info {
        if !self.info.contains_key(unit) {
            self.info_order.push(unit.to_string());
        }
        self.info.entry(unit.to_string()).or_default()
    }

    /// Records in the order they were created.
    pub fn records(&self) -> impl Iterator<Item = (&String, &Info)> {
        self.info_order.iter().map(|u| (u, &self.info[u]))
    }

    /// Pop the next unit for the current phase, moving to tests (then finished) when a queue runs dry.
    pub fn next_unit(&mut self) -> Option<Item> {
        if self.phase == Phase::Notes {
            if let Some(item) = self.queue.pop_front() {
                return Some(item);
            }
            self.phase = Phase::Tests;
            self.test_queue = self
                .order
                .iter()
                .filter(|u| self.info[*u].has_rules)
                .map(|u| Item {
                    unit: u.clone(),
                    parent: self.info[u].parent.clone(),
                    level: self.info[u].level,
                })
                .collect();
        }
        if self.phase == Phase::Tests {
            if let Some(item) = self.test_queue.pop_front() {
                return Some(item);
            }
        }
        self.phase = Phase::Finished;
        None
    }

    pub fn skip_current(&mut self, reason: String) {
        let item = self.current.clone().expect("a round has a unit");
        let is_new = self.info(&item.unit).is_none();
        let info = self.entry(&item.unit);
        if is_new {
            info.parent = item.parent;
            info.level = item.level;
        }
        info.skipped = Some(reason);
    }
}

pub struct Config {
    pub db: PathBuf,
    pub out: PathBuf,
    pub crate_dir: PathBuf,
    pub max_tool_chars: usize,
    pub context_budget: u32,
}

/// Everything a run works on: the decoded VBA, the unit graph and the walk.
pub struct Harness {
    pub cfg: Config,
    pub vba: Vec<Module>,
    pub units: Units,
    pub walk: Walk,
}

impl Harness {
    pub fn notes_path(&self, unit: &str) -> PathBuf {
        let index = self
            .walk
            .info(unit)
            .and_then(|i| i.index)
            .expect("notes were saved");
        self.cfg
            .out
            .join("notes")
            .join(format!("{index:03}-{}.md", slug(unit)))
    }

    pub fn test_path(&self, unit: &str) -> PathBuf {
        let index = self
            .walk
            .info(unit)
            .and_then(|i| i.index)
            .expect("notes were saved");
        self.cfg
            .crate_dir
            .join("tests")
            .join(format!("u{index:03}_{}.rs", slug(unit)))
    }

    fn read_notes_file(&self, unit: &str) -> String {
        fs::read_to_string(self.notes_path(unit))
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    /// A unit's source for the brief, cut at the tool-result size with a pointer to the rest.
    pub fn unit_source(&self, unit: &str) -> String {
        let u = self.units.get(unit).expect("queued units exist");
        let text = if u.text.is_empty() {
            "(empty)"
        } else {
            &u.text
        };
        if text.chars().count() <= self.cfg.max_tool_chars {
            return text.to_string();
        }
        let cut: String = text.chars().take(self.cfg.max_tool_chars).collect();
        let head = cut.rsplit_once('\n').map_or(cut.as_str(), |(h, _)| h);
        let numbered = Regex::new(r"(?m)^(\d+): ").unwrap();
        let shown: Option<usize> = if u.kind == Kind::Vba {
            numbered
                .captures_iter(head)
                .last()
                .and_then(|c| c[1].parse().ok())
        } else {
            None
        };
        let more = match (shown, &u.module) {
            (Some(n), Some(module)) if n > 0 => {
                format!(
                    "read_vba module={module} start={} for the rest, up to line {}",
                    n + 1,
                    u.last
                )
            }
            _ => "the rest is cut".to_string(),
        };
        format!("{head}\n... [unit continues; {more}]")
    }

    /// Everything a round starts from.
    pub fn brief(&self, note: &str) -> String {
        let item = self.walk.current.as_ref().expect("a round has a unit");
        let phase = self.walk.phase;
        let mut parts = vec![format!(
            "Round {}. Phase: {}. Context budget: {} tokens per round.",
            self.walk.rounds,
            phase.name(),
            self.cfg.context_budget
        )];
        if !note.is_empty() {
            parts.push(note.to_string());
        }
        if item.unit == OVERVIEW {
            parts.push(format!(
                "This is the survey round. Inventory of the system (names only):\n{}",
                inventory(&self.units, &self.vba)
            ));
            parts.push("Next: save_notes with an overview of what this system appears to do, and in follow \
                        list the units to start from, most important first. Every other unit is reached \
                        from those, breadth first.".into());
            return parts.join("\n\n");
        }
        let u = self.units.get(&item.unit).expect("queued units exist");
        let waiting = if phase == Phase::Notes {
            self.walk.queue.len()
        } else {
            self.walk.test_queue.len()
        };
        parts.push(format!("Unit: {} ({}). Breadth-first level {}, reached from {}. {waiting} units wait after this one.",
                           item.unit, u.label, item.level, item.parent.as_deref().unwrap_or("(start)")));
        if phase == Phase::Notes {
            if let Some(parent) = &item.parent {
                parts.push(format!(
                    "Notes of {parent}, which led here:\n{}",
                    self.read_notes_file(parent)
                ));
            }
            let new: Vec<&str> = u
                .refs
                .iter()
                .filter(|r| !self.walk.seen.contains(*r))
                .map(String::as_str)
                .collect();
            let refs = if u.refs.is_empty() {
                "none".to_string()
            } else {
                u.refs.join(", ")
            };
            let candidates = if new.is_empty() {
                String::new()
            } else {
                format!("\nNot yet seen (candidates to follow): {}", new.join(", "))
            };
            parts.push(format!("References found in this unit: {refs}{candidates}"));
            parts.push(format!("Source:\n{}", self.unit_source(&item.unit)));
            parts.push("Next: check anything you need with the tools, then save_notes for this unit: the \
                        rules it implements, each as a sentence with its evidence. In follow, name the \
                        references worth visiting. Set has_rules if there is something to test.".into());
        } else {
            parts.push(format!(
                "Your notes for this unit:\n{}",
                self.read_notes_file(&item.unit)
            ));
            parts.push(format!("Source:\n{}", self.unit_source(&item.unit)));
            parts.push(
                "Next: check anything you need, then save_tests with one Rust test file for this \
                        unit's rules, or skip with a reason if there is nothing to test."
                    .into(),
            );
        }
        parts.join("\n\n")
    }

    /// The breadth-first order as it happened, plus every unit the walk never reached.
    pub fn walk_log(&self) -> String {
        let mut rows = vec![
            "| # | Level | Unit | Reached from | Rules | Tests | Note |".to_string(),
            "|---|---|---|---|---|---|---|".to_string(),
        ];
        for unit in &self.walk.order {
            let i = self.walk.info(unit).expect("ordered units have records");
            rows.push(format!(
                "| {} | {} | {unit} | {} | {} | {} | {} |",
                i.index.unwrap_or(0),
                i.level,
                i.parent.as_deref().unwrap_or(""),
                if i.has_rules { "yes" } else { "" },
                i.tests.map(|n| n.to_string()).unwrap_or_default(),
                i.skipped.as_deref().unwrap_or("")
            ));
        }
        let mut text = format!("# Breadth-first walk\n\n{}\n", rows.join("\n"));
        let skipped: Vec<String> = self
            .walk
            .records()
            .filter(|(_, i)| i.index.is_none())
            .map(|(u, i)| format!("- {u}: {}", i.skipped.as_deref().unwrap_or("")))
            .collect();
        if !skipped.is_empty() {
            text += &format!("\n## Skipped\n\n{}\n", skipped.join("\n"));
        }
        let queued: Vec<&str> = self.walk.queue.iter().map(|i| i.unit.as_str()).collect();
        if !queued.is_empty() {
            text += &format!(
                "\n## Still queued when the run stopped ({})\n\n{}\n",
                queued.len(),
                queued.join(", ")
            );
        }
        let never: Vec<&str> = self
            .units
            .iter()
            .map(|u| u.id.as_str())
            .filter(|u| self.walk.info(u).is_none() && !self.walk.seen.contains(*u))
            .collect();
        text += &format!(
            "\n## Never reached ({} of {} units)\n\n{}\n",
            never.len(),
            self.units.len(),
            if never.is_empty() {
                "none".to_string()
            } else {
                never.join(", ")
            }
        );
        text
    }
}
