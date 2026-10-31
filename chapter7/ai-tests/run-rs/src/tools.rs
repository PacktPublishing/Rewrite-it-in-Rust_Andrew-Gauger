//! Tools the model can call. The readers are read-only; the two savers write only the
//! current unit's file into the run's own outputs.

use std::error::Error;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use regex::{Regex, RegexBuilder};
use serde_json::{json, Map, Value};

use crate::walk::{Harness, Item, Phase, OVERVIEW};

const MAX_READ_LINES: i64 = 300;
pub const MAX_SQL_ROWS: usize = 20;
const MAX_SEARCH_HITS: usize = 40;
pub const NOTE_CHARS: usize = 2000;
pub const TEST_CHARS: usize = 6000;
/// mdb-sql can loop forever (COUNT(*) with a WHERE clause does); no query gets longer than this.
const SQL_TIMEOUT: Duration = Duration::from_secs(30);

/// What mdb-sql understands, measured against mdbtools on the Cairn file. run.py's sql tool
/// says none of this, and mdb-sql exits 0 with empty output when it can't parse a query.
const SQL_HELP: &str = "mdb-sql reads one table per query: SELECT [TOP n] columns, * or COUNT(*) FROM \
    table [WHERE comparisons with =, <>, <, >, <=, >=, LIKE, joined by AND, OR, NOT] [LIMIT n]. It has \
    no joins, AS aliases, DISTINCT, GROUP BY, ORDER BY, IN, IS NULL, functions or arithmetic, and \
    COUNT(*) cannot take a WHERE. Rows come back in table order.";

/// A finished command's output.
pub struct Captured {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

/// Run a command, feeding it `stdin`, and collect stdout and stderr. With a timeout, a
/// command still running at the deadline is killed and reported as an error.
pub fn capture(
    program: &str,
    args: &[&str],
    stdin: Option<&str>,
    timeout: Option<Duration>,
) -> Result<Captured, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("error: {program}: {e}"))?;
    // Read both pipes on their own threads so a chatty child can't block on a full pipe.
    let reader = |pipe: Option<Box<dyn Read + Send>>| {
        thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut buf);
            }
            String::from_utf8_lossy(&buf).into_owned()
        })
    };
    let out = reader(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let err = reader(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    if let (Some(text), Some(mut pipe)) = (stdin, child.stdin.take()) {
        let _ = pipe.write_all(text.as_bytes()); // dropping the pipe closes it
    }
    let deadline = timeout.map(|t| Instant::now() + t);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => return Err(format!("error: {program}: {e}")),
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            let _ = child.kill();
            let _ = child.wait();
            let secs = timeout.unwrap_or_default().as_secs();
            return Err(format!(
                "error: {program} did not finish within {secs} seconds and was stopped"
            ));
        }
        thread::sleep(Duration::from_millis(20));
    };
    Ok(Captured {
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
        success: status.success(),
    })
}

/// Run a command and return its stdout, or "error: ..." the way the model sees it.
pub fn run_cmd(program: &str, args: &[&str], stdin: Option<&str>) -> String {
    match capture(program, args, stdin, None) {
        Err(e) => e,
        Ok(c) if !c.success => {
            let msg = if c.stderr.trim().is_empty() {
                c.stdout.trim()
            } else {
                c.stderr.trim()
            };
            format!("error: {msg}")
        }
        Ok(c) => c.stdout,
    }
}

/// What the sql tool shows for mdb-sql's output. mdb-sql exits 0 whatever happens, so its
/// stderr is the only sign that a query wasn't understood or was only partly evaluated.
pub fn sql_result(stdout: &str, stderr: &str) -> String {
    if stderr.contains("syntax error") || stderr.contains("Got no result") {
        return format!("error: mdb-sql could not parse this query, so nothing ran. {SQL_HELP}");
    }
    if stderr.contains("unknown operator") {
        return "error: mdb-sql cannot evaluate part of this WHERE clause and would have returned wrong \
                rows, so nothing is shown. IS NULL and IS NOT NULL are among what it can't evaluate."
            .into();
    }
    // mdb-sql starts its output with a blank line, then the header, then one line per row.
    let body = stdout.strip_prefix('\n').unwrap_or(stdout);
    let lines: Vec<&str> = body.lines().collect();
    let Some((header, rows)) = lines.split_first() else {
        return "error: mdb-sql printed nothing for this query.".into();
    };
    let mut out = if rows.len() > MAX_SQL_ROWS {
        format!(
            "{header}\n{}\n... [{} more rows not shown, {} in all; add a WHERE clause]",
            rows[..MAX_SQL_ROWS].join("\n"),
            rows.len() - MAX_SQL_ROWS,
            rows.len()
        )
    } else if rows.is_empty() {
        format!("{header}\n(0 rows)")
    } else {
        body.trim_end_matches('\n').to_string()
    };
    if let Some(warning) = stderr.lines().map(str::trim).find(|l| !l.is_empty()) {
        out += &format!("\n[mdb-sql also printed: {warning}]");
    }
    out
}

/// Cut a tool result to `max` characters and say how much was dropped.
pub fn clip(text: String, max: usize) -> String {
    let n = text.chars().count();
    if n <= max {
        return text;
    }
    let head: String = text.chars().take(max).collect();
    format!("{head}\n... [truncated, {} more characters]", n - max)
}

/// mdbtools prefixes every schema with a licence banner; it costs tokens and says nothing.
pub fn strip_banner(sql: &str) -> String {
    let kept: Vec<&str> = sql
        .lines()
        .filter(|l| !l.starts_with("--") && !l.starts_with("SET "))
        .collect();
    format!("{}\n", kept.join("\n").trim())
}

/// A string quoted the way Python's repr() quotes it, so messages read the same in both harnesses.
pub fn py_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::from(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// A VBA module decoded from the database: its name and its lines.
pub type Module = (String, Vec<String>);

/// Decode the VBA project out of the database with access_vba.py (needs oletools,
/// installed by setup), keep the decoded files in `vba_dir` as evidence, and load them.
pub fn load_vba(
    python: &Path,
    script: &Path,
    db: &Path,
    vba_dir: &Path,
) -> Result<Vec<Module>, Box<dyn Error>> {
    if vba_dir.exists() {
        fs::remove_dir_all(vba_dir)?;
    }
    let out = Command::new(python)
        .arg(script)
        .arg(db)
        .arg(vba_dir)
        .output()
        .map_err(|e| format!("{}: {e}", python.display()))?;
    if !out.status.success() {
        let name = db.file_name().unwrap_or_default().to_string_lossy();
        return Err(format!(
            "could not read VBA from {name}:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    println!("{}", String::from_utf8_lossy(&out.stdout).trim());

    let mut files: Vec<PathBuf> = fs::read_dir(vba_dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "bas"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|path| {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let lines = fs::read_to_string(path)?
                .lines()
                .map(String::from)
                .collect();
            Ok((name, lines))
        })
        .collect()
}

struct Param {
    name: &'static str,
    ty: &'static str,
    desc: &'static str,
    required: bool,
}

struct Spec {
    name: &'static str,
    desc: String,
    params: &'static [Param],
}

fn spec_list() -> Vec<Spec> {
    vec![
        Spec { name: "get_schema", desc: "Get the CREATE TABLE statement for one table.".into(),
               params: &[Param { name: "table", ty: "string", desc: "Table name", required: true }] },
        Spec { name: "get_saved_query", desc: "Get the SQL of one saved query.".into(),
               params: &[Param { name: "name", ty: "string", desc: "Saved query name, e.g. qryQuoteTotals", required: true }] },
        Spec { name: "read_vba",
               desc: "Read lines of a VBA module. Stops early at a size limit and says where to continue.".into(),
               params: &[Param { name: "module", ty: "string", desc: "Module name", required: true },
                         Param { name: "start", ty: "integer", desc: "First line, 1-based", required: false },
                         Param { name: "end", ty: "integer", desc: "Last line", required: false }] },
        Spec { name: "search_vba", desc: "Regex search (case-insensitive) across the VBA modules.".into(),
               params: &[Param { name: "pattern", ty: "string", desc: "Regular expression", required: true },
                         Param { name: "module", ty: "string", desc: "Optional: limit to one module", required: false }] },
        Spec { name: "sql",
               desc: format!("Run one read-only SELECT against the database data. Returns at most {MAX_SQL_ROWS} rows \
                              and says how many more matched. {SQL_HELP}"),
               params: &[Param { name: "query", ty: "string", desc: "A single SELECT statement", required: true }] },
        Spec { name: "read_notes", desc: "Read the notes saved for another unit.".into(),
               params: &[Param { name: "unit", ty: "string", desc: "Unit id, e.g. vba:CairnForms.NextNumber", required: true }] },
        Spec { name: "save_notes",
               desc: format!("Save this round's unit notes (at most {NOTE_CHARS} characters) and name the units to \
                              visit later. Ends the round."),
               params: &[Param { name: "text", ty: "string",
                                 desc: "What the unit does: each rule as a sentence, with line numbers or SQL as evidence",
                                 required: true },
                         Param { name: "follow", ty: "array",
                                 desc: "Unit ids worth visiting, from the references in the brief", required: false },
                         Param { name: "has_rules", ty: "boolean",
                                 desc: "true if this unit implements a business rule worth testing", required: false }] },
        Spec { name: "save_tests",
               desc: format!("Save this round's unit Rust test file (at most {TEST_CHARS} characters). Ends the round."),
               params: &[Param { name: "code", ty: "string", desc: "One Rust test file", required: true },
                         Param { name: "skip", ty: "string",
                                 desc: "Instead of code: why this unit has nothing to test", required: false }] },
    ]
}

/// The tool list in the OpenAI-compatible shape the model server expects.
pub fn specs() -> Value {
    spec_list()
        .iter()
        .map(|spec| {
            let mut properties = Map::new();
            for p in spec.params {
                let mut prop = json!({"type": p.ty, "description": p.desc});
                if p.ty == "array" {
                    prop["items"] = json!({"type": "string"});
                }
                properties.insert(p.name.into(), prop);
            }
            let required: Vec<&str> = spec
                .params
                .iter()
                .filter(|p| p.required)
                .map(|p| p.name)
                .collect();
            json!({"type": "function", "function": {
                "name": spec.name,
                "description": spec.desc,
                "parameters": {"type": "object", "properties": properties, "required": required},
            }})
        })
        .collect()
}

/// A string argument as text, whatever JSON type the model sent it as.
fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// An integer argument, accepting a number or a numeric string.
fn int(args: &Map<String, Value>, key: &str, default: i64) -> Result<i64, String> {
    match args.get(key) {
        None => Ok(default),
        Some(Value::Number(n)) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f as i64))
            .ok_or_else(|| format!("error: bad arguments: {key} must be an integer")),
        Some(Value::String(s)) => s.trim().parse().map_err(|_| {
            format!(
                "error: bad arguments: {key} must be an integer, got {}",
                py_repr(s)
            )
        }),
        Some(other) => Err(format!(
            "error: bad arguments: {key} must be an integer, got {other}"
        )),
    }
}

/// Unit ids from a list, or from a comma- or space-separated string, without repeats.
fn ids(value: Option<&Value>) -> Vec<String> {
    let list: Vec<String> = match value {
        None | Some(Value::Null) => vec![],
        Some(Value::String(s)) => s
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|v| !v.is_empty())
            .map(String::from)
            .collect(),
        Some(Value::Array(items)) => items.iter().map(|v| text(v).trim().to_string()).collect(),
        Some(other) => vec![text(other).trim().to_string()],
    };
    let mut unique = Vec::new();
    for id in list {
        if !unique.contains(&id) {
            unique.push(id);
        }
    }
    unique
}

/// Notes cut to `max` characters, marker included, at the last line or sentence end that fits.
fn cut_notes(text: &str, max: usize) -> String {
    let n = text.chars().count();
    let marker_room = 60;
    let head: String = text.chars().take(max.saturating_sub(marker_room)).collect();
    let end = head
        .rfind('\n')
        .or_else(|| head.rfind(". ").map(|i| i + 1))
        .filter(|&i| i > head.len() / 2)
        .unwrap_or(head.len());
    let kept = head[..end].trim_end();
    format!(
        "{kept}\n[cut by the harness: {} of {n} characters kept]",
        kept.chars().count()
    )
}

fn flag(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::Null) | None => false,
        Some(other) => matches!(
            text(other).trim().to_lowercase().as_str(),
            "true" | "1" | "yes"
        ),
    }
}

impl Harness {
    fn db(&self) -> &str {
        self.cfg.db.to_str().expect("database path is UTF-8")
    }

    fn module(&self, name: &str) -> Result<&[String], String> {
        let name = name.strip_suffix(".bas").unwrap_or(name);
        self.vba
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, lines)| lines.as_slice())
            .ok_or_else(|| {
                let names: Vec<&str> = self.vba.iter().map(|(n, _)| n.as_str()).collect();
                format!(
                    "error: no module {}. Modules: {}",
                    py_repr(name),
                    names.join(", ")
                )
            })
    }

    pub fn get_schema(&self, table: &str) -> String {
        match self.units.get(&format!("table:{table}")) {
            Some(u) => u.text.clone(),
            None => format!("error: no table {}", py_repr(table)),
        }
    }

    pub fn get_saved_query(&self, name: &str) -> String {
        match self.units.get(&format!("query:{name}")) {
            Some(u) => u.text.clone(),
            None => format!("error: no saved query {}", py_repr(name)),
        }
    }

    /// Whole lines from `start`, stopping before the result would pass the size limit.
    pub fn read_vba(&self, module: &str, start: i64, end: i64) -> String {
        let lines = match self.module(module) {
            Ok(lines) => lines,
            Err(e) => return e,
        };
        let start = start.max(1);
        let end = end.min(lines.len() as i64).min(start + MAX_READ_LINES - 1);
        let room = self.cfg.max_tool_chars.saturating_sub(100);
        let (mut out, mut size) = (Vec::new(), 0usize);
        for i in start..=end {
            let line = format!("{i}: {}", lines[i as usize - 1]);
            let len = line.chars().count();
            if !out.is_empty() && size + len + 1 > room {
                out.push(format!(
                    "... [stopped after line {} to fit; continue with start={i}]",
                    i - 1
                ));
                break;
            }
            out.push(line);
            size += len + 1;
        }
        out.join("\n")
    }

    pub fn search_vba(&self, pattern: &str, module: Option<&str>) -> String {
        let re = match RegexBuilder::new(pattern).case_insensitive(true).build() {
            Ok(re) => re,
            Err(e) => return format!("error: bad regex: {e}"),
        };
        let names: Vec<&str> = match module {
            Some(m) if !m.is_empty() => vec![m],
            _ => self.vba.iter().map(|(n, _)| n.as_str()).collect(),
        };
        let mut hits = Vec::new();
        for name in names {
            let lines = match self.module(name) {
                Ok(lines) => lines,
                Err(e) => return e,
            };
            let shown = name.strip_suffix(".bas").unwrap_or(name);
            for (i, line) in lines.iter().enumerate() {
                if re.is_match(line) {
                    hits.push(format!("{shown}:{}: {}", i + 1, line.trim()));
                }
            }
        }
        if hits.len() > MAX_SEARCH_HITS {
            let more = hits.len() - MAX_SEARCH_HITS;
            hits.truncate(MAX_SEARCH_HITS);
            return format!(
                "{}\n... [{more} more matches; narrow the pattern]",
                hits.join("\n")
            );
        }
        if hits.is_empty() {
            "no matches".into()
        } else {
            hits.join("\n")
        }
    }

    pub fn sql(&self, query: &str) -> String {
        let q = query.trim().trim_end_matches(';');
        let select = Regex::new(r"(?is)^select\b").unwrap();
        if !select.is_match(q) || q.contains(';') {
            return "error: only a single SELECT statement is allowed".into();
        }
        let count_where = Regex::new(r"(?is)\bcount\s*\(\s*\*\s*\).*\bwhere\b").unwrap();
        if count_where.is_match(q) {
            return "error: mdb-sql never returns from COUNT(*) with a WHERE clause. Select one column with \
                    the same WHERE instead; the result says how many rows matched."
                .into();
        }
        match capture(
            "mdb-sql",
            &["-P", "-F", "-d", "|", self.db()],
            Some(&format!("{q}\n")),
            Some(SQL_TIMEOUT),
        ) {
            Err(e) => format!("{e}. Narrow the WHERE clause."),
            Ok(c) if !c.success => format!("error: {}", c.stderr.trim()),
            Ok(c) => sql_result(&c.stdout, &c.stderr),
        }
    }

    pub fn read_notes(&self, unit: &str) -> String {
        match self.walk.info(unit).and_then(|i| i.index) {
            Some(_) => fs::read_to_string(self.notes_path(unit)).unwrap_or_default(),
            None => format!("error: no notes for {} yet", py_repr(unit)),
        }
    }

    pub fn save_notes(
        &mut self,
        text: &str,
        follow: Option<&Value>,
        has_rules: Option<&Value>,
    ) -> String {
        if self.walk.phase != Phase::Notes {
            return "error: the notes phase is over; save_tests for this unit.".into();
        }
        if self.walk.saved {
            return "error: this unit's notes are already saved.".into();
        }
        let Item {
            unit,
            parent,
            level,
        } = self.walk.current.clone().expect("a round has a unit");
        let text = text.trim();
        if text.is_empty() {
            return "error: empty notes".into();
        }
        let n = text.chars().count();
        let mut text = text.to_string();
        let mut cut_note = String::new();
        if n > NOTE_CHARS {
            // A small model can't count characters: in run.py it sent the same over-long text
            // back until the round ran out of context. The second identical try is cut and saved.
            if self.walk.rejected_notes.as_deref() != Some(text.as_str()) {
                self.walk.rejected_notes = Some(text);
                return format!("error: {n} characters; a unit's notes hold at most {NOTE_CHARS}, so cut at \
                                least {} of them. Nothing saved. Sending the same text again saves only \
                                what fits.", n - NOTE_CHARS);
            }
            text = cut_notes(&text, NOTE_CHARS);
            cut_note = format!(
                " The notes came back unchanged at {n} characters, so they were cut to {}.",
                text.chars().count()
            );
        }
        let follow = ids(follow);
        let unknown: Vec<&str> = follow
            .iter()
            .filter(|f| !self.units.contains(f))
            .map(String::as_str)
            .collect();
        if !unknown.is_empty() {
            return format!(
                "error: unknown unit ids: {}. Nothing saved; use ids from the brief.",
                unknown.join(", ")
            );
        }
        let index = self.walk.order.len();
        let info = self.walk.entry(&unit);
        info.parent = parent.clone();
        info.level = level;
        info.follow = follow.clone();
        info.has_rules = unit != OVERVIEW && flag(has_rules);
        info.index = Some(index);
        self.walk.order.push(unit.clone());

        let path = self.notes_path(&unit);
        let header = format!(
            "<!-- unit: {unit} | reached from: {} | level {level} | follow: {} -->\n",
            parent.as_deref().unwrap_or("(start)"),
            if follow.is_empty() {
                "(none)".to_string()
            } else {
                follow.join(", ")
            }
        );
        if let Err(e) = path
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|_| fs::write(&path, format!("{header}{text}\n")))
        {
            return format!("error: could not save notes: {e}");
        }
        let queued: Vec<String> = follow
            .iter()
            .filter(|f| !self.walk.seen.contains(*f))
            .cloned()
            .collect();
        for f in &queued {
            self.walk.seen.insert(f.clone());
            self.walk.queue.push_back(Item {
                unit: f.clone(),
                parent: Some(unit.clone()),
                level: level + 1,
            });
        }
        self.walk.saved = true;
        let already = if follow.len() > queued.len() {
            format!(" ({} already seen)", follow.len() - queued.len())
        } else {
            String::new()
        };
        format!(
            "saved {}.{cut_note} Queued {} new units{already}; {} waiting. This round is done.",
            path.file_name().unwrap_or_default().to_string_lossy(),
            queued.len(),
            self.walk.queue.len()
        )
    }

    pub fn save_tests(&mut self, code: &str, skip: Option<&Value>) -> String {
        if self.walk.phase != Phase::Tests {
            return "error: tests come after the notes; save_notes for this unit.".into();
        }
        if self.walk.saved {
            return "error: this unit's tests are already saved.".into();
        }
        let unit = self
            .walk
            .current
            .as_ref()
            .expect("a round has a unit")
            .unit
            .clone();
        let code = code.trim_matches('\n');
        if code.trim().is_empty() {
            let Some(reason) = skip.map(text).filter(|s| !s.is_empty()) else {
                return "error: empty test file. Pass skip with a reason if this unit has nothing to test.".into();
            };
            self.walk.entry(&unit).skipped = Some(format!("no tests: {reason}"));
            self.walk.saved = true;
            return "recorded: no tests for this unit. This round is done.".into();
        }
        let n = code.chars().count();
        if n > TEST_CHARS {
            return format!("error: {n} characters; a unit's test file holds at most {TEST_CHARS}, so cut at least {}. \
                            Keep the clearest tests.", n - TEST_CHARS);
        }
        let path = self.test_path(&unit);
        if let Err(e) = path
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|_| fs::write(&path, format!("{code}\n")))
        {
            return format!("error: could not save tests: {e}");
        }
        let count = code.matches("#[test]").count();
        self.walk.entry(&unit).tests = Some(count);
        self.walk.saved = true;
        format!(
            "saved {} ({count} tests). This round is done.",
            path.file_name().unwrap_or_default().to_string_lossy()
        )
    }

    /// Run one tool call from the model and return what the model sees.
    pub fn call(&mut self, name: &str, args: &Value) -> String {
        let specs = spec_list();
        let Some(spec) = specs.iter().find(|s| s.name == name) else {
            return format!("error: no such tool {}", py_repr(name));
        };
        let empty = Map::new();
        let args = match args {
            Value::Object(map) => map,
            Value::Null => &empty,
            _ => return "error: bad arguments: arguments must be a JSON object".into(),
        };
        // The model often names the unit it is saving. run.py rejected that 48 times in one
        // run; here a unit argument naming the current unit is accepted and dropped.
        let without_unit;
        let args = match args.get("unit") {
            Some(given) if matches!(name, "save_notes" | "save_tests") => {
                let current = self.walk.current.as_ref().map_or("", |i| i.unit.as_str());
                let given = text(given);
                if given.trim() != current {
                    return format!(
                        "error: this round is for {current}, not {}. Save this unit's file; \
                                    the unit argument isn't needed.",
                        py_repr(given.trim())
                    );
                }
                let mut map = args.clone();
                map.remove("unit");
                without_unit = map;
                &without_unit
            }
            _ => args,
        };
        if let Some(key) = args
            .keys()
            .find(|k| !spec.params.iter().any(|p| p.name == k.as_str()))
        {
            return format!(
                "error: bad arguments: {name}() got an unexpected argument {}",
                py_repr(key)
            );
        }
        if let Some(p) = spec
            .params
            .iter()
            .find(|p| p.required && !args.contains_key(p.name))
        {
            return format!(
                "error: bad arguments: {name}() missing required argument {}",
                py_repr(p.name)
            );
        }
        let arg = |key: &str| args.get(key).map(text).unwrap_or_default();
        let result = match name {
            "get_schema" => self.get_schema(&arg("table")),
            "get_saved_query" => self.get_saved_query(&arg("name")),
            "read_vba" => match (int(args, "start", 1), int(args, "end", 200)) {
                (Ok(start), Ok(end)) => self.read_vba(&arg("module"), start, end),
                (Err(e), _) | (_, Err(e)) => e,
            },
            "search_vba" => {
                self.search_vba(&arg("pattern"), args.get("module").map(text).as_deref())
            }
            "sql" => self.sql(&arg("query")),
            "read_notes" => self.read_notes(&arg("unit")),
            "save_notes" => {
                let (text, follow, has_rules) = (
                    arg("text"),
                    args.get("follow").cloned(),
                    args.get("has_rules").cloned(),
                );
                self.save_notes(&text, follow.as_ref(), has_rules.as_ref())
            }
            "save_tests" => {
                let (code, skip) = (arg("code"), args.get("skip").cloned());
                self.save_tests(&code, skip.as_ref())
            }
            _ => unreachable!("every name in spec_list has a match arm"),
        };
        clip(result, self.cfg.max_tool_chars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Units;
    use crate::walk::{Config, Walk};

    /// A harness with its own output directories, so tests running in parallel don't share files.
    fn harness() -> Harness {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("run-rs-unit-tests-{}-{n}", std::process::id()));
        let lines = (1..=500).map(|i| format!("line {i}")).collect();
        Harness {
            cfg: Config {
                db: PathBuf::from("/nonexistent.accdb"),
                out: dir.join("outputs"),
                crate_dir: dir.join("crate"),
                max_tool_chars: 4096,
                context_budget: 16384,
            },
            vba: vec![("CairnForms".into(), lines)],
            units: Units::default(),
            walk: Walk::new(),
        }
    }

    #[test]
    fn clip_keeps_short_text_and_counts_what_it_cuts() {
        assert_eq!(clip("short".into(), 10), "short");
        assert!(clip("x".repeat(15), 10).ends_with("... [truncated, 5 more characters]"));
    }

    #[test]
    fn read_vba_stops_at_whole_lines_and_says_where_to_continue() {
        let mut h = harness();
        h.cfg.max_tool_chars = 200;
        let out = h.read_vba("CairnForms", 1, 300);
        assert!(out.starts_with("1: line 1\n"));
        assert!(out.ends_with("continue with start=10]"), "{out}");
    }

    #[test]
    fn read_vba_clamps_start_and_end() {
        let h = harness();
        assert_eq!(h.read_vba("CairnForms.bas", -4, 2), "1: line 1\n2: line 2");
        assert_eq!(
            h.read_vba("CairnForms", 499, 900),
            "499: line 499\n500: line 500"
        );
    }

    #[test]
    fn unknown_module_names_the_ones_that_exist() {
        assert_eq!(
            harness().read_vba("CairnHistory", 1, 3),
            "error: no module 'CairnHistory'. Modules: CairnForms"
        );
    }

    #[test]
    fn sql_refuses_anything_but_one_select() {
        let h = harness();
        for q in [
            "DELETE FROM Orders",
            "SELECT 1; DROP TABLE Orders",
            "selectx FROM t",
        ] {
            assert_eq!(
                h.sql(q),
                "error: only a single SELECT statement is allowed",
                "{q}"
            );
        }
    }

    #[test]
    fn call_checks_arguments_against_the_spec() {
        let mut h = harness();
        assert_eq!(h.call("nope", &json!({})), "error: no such tool 'nope'");
        assert!(h
            .call("read_vba", &json!({}))
            .contains("missing required argument 'module'"));
        assert!(h
            .call("read_vba", &json!({"module": "CairnForms", "x": 1}))
            .contains("unexpected argument 'x'"));
        assert_eq!(
            h.call(
                "read_vba",
                &json!({"module": "CairnForms", "start": "2", "end": 2})
            ),
            "2: line 2"
        );
    }

    #[test]
    fn savers_respect_the_phase_and_limits() {
        let mut h = harness();
        h.walk.current = h.walk.next_unit();
        assert!(h
            .call("save_tests", &json!({"code": "x"}))
            .starts_with("error: tests come after the notes"));
        assert!(h
            .call("save_notes", &json!({"text": "x".repeat(NOTE_CHARS + 1)}))
            .contains("at most 2000"));
        assert!(h
            .call("save_notes", &json!({"text": "x", "follow": ["vba:Nope"]}))
            .starts_with("error: unknown unit ids"));
    }

    #[test]
    fn follow_accepts_lists_and_strings_and_drops_repeats() {
        assert_eq!(ids(Some(&json!(["a", " b ", "a"]))), ["a", "b"]);
        assert_eq!(ids(Some(&json!("a, b c"))), ["a", "b", "c"]);
        assert!(ids(None).is_empty());
    }

    #[test]
    fn sql_result_reports_what_mdb_sql_did() {
        let syntax =
            "Error at Line : syntax error\nsyntax error\nGot no result for 'SELECT 1' command\n";
        assert!(sql_result("\n", syntax).starts_with("error: mdb-sql could not parse this query"));
        let unknown = "Calling mdb_test_sarg on unknown operator.  Add code to mdb_test_string() for operator 10\n";
        assert!(sql_result("\nCustomerID\n", unknown).starts_with("error: mdb-sql cannot evaluate"));
        assert_eq!(sql_result("\nOrderNumber\n", ""), "OrderNumber\n(0 rows)");
        assert_eq!(
            sql_result("\nOrderNumber\nSO-2026-0920\n", ""),
            "OrderNumber\nSO-2026-0920"
        );
        let many: String = (1..=25).map(|i| format!("{i}\n")).collect();
        let out = sql_result(&format!("\nOrderID\n{many}"), "");
        assert!(
            out.ends_with("... [5 more rows not shown, 25 in all; add a WHERE clause]"),
            "{out}"
        );
        assert_eq!(out.lines().count(), MAX_SQL_ROWS + 2);
    }

    #[test]
    fn sql_refuses_count_with_where_before_running_it() {
        let h = harness();
        assert!(h
            .sql("SELECT COUNT(*) FROM Orders WHERE OrderID < 3")
            .starts_with("error: mdb-sql never returns"));
    }

    #[test]
    fn capture_stops_a_command_at_its_timeout() {
        let err = capture("sleep", &["5"], None, Some(Duration::from_millis(200)))
            .err()
            .unwrap();
        assert_eq!(
            err,
            "error: sleep did not finish within 0 seconds and was stopped"
        );
        let ok = capture("cat", &[], Some("hello"), Some(Duration::from_secs(5))).unwrap();
        assert_eq!((ok.stdout.as_str(), ok.success), ("hello", true));
    }

    #[test]
    fn identical_over_long_notes_are_cut_and_saved_the_second_time() {
        let mut h = harness();
        h.walk.current = h.walk.next_unit();
        let long = "A rule. ".repeat(300);
        let first = h.call("save_notes", &json!({"text": long}));
        assert!(
            first.contains("so cut at least") && first.contains("Nothing saved"),
            "{first}"
        );
        let second = h.call("save_notes", &json!({"text": long}));
        assert!(
            second.starts_with("saved 000-overview.md. The notes came back unchanged"),
            "{second}"
        );
        let saved = fs::read_to_string(h.notes_path(OVERVIEW)).unwrap();
        let body = saved.split_once("-->\n").unwrap().1;
        assert!(body.trim_end().ends_with("characters kept]"));
        assert!(
            body.trim_end().chars().count() <= NOTE_CHARS,
            "{}",
            body.chars().count()
        );
    }

    #[test]
    fn savers_accept_a_unit_argument_naming_the_current_unit() {
        let mut h = harness();
        h.walk.current = h.walk.next_unit();
        let wrong = h.call(
            "save_notes",
            &json!({"text": "x", "unit": "vba:CairnForms.Nope"}),
        );
        assert!(
            wrong.starts_with("error: this round is for overview"),
            "{wrong}"
        );
        let right = h.call(
            "save_notes",
            &json!({"text": "Overview.", "unit": "overview"}),
        );
        assert!(right.starts_with("saved 000-overview.md"), "{right}");
    }

    #[test]
    fn py_repr_matches_python() {
        assert_eq!(py_repr("x; rm -rf /"), "'x; rm -rf /'");
        assert_eq!(py_repr("it's"), "\"it's\"");
    }
}

#[cfg(test)]
mod parity {
    /// The specs the model sees must match run.py's byte for byte, key order included, except
    /// the sql tool's description, which here says what mdb-sql can't do (run-py-issues.md).
    /// Regenerate the fixture from run.py with the command in README.md.
    #[test]
    fn tool_specs_match_run_py_except_the_sql_description() {
        let mut fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/tool-specs.json")).unwrap();
        let ours = super::specs();
        let sql = |specs: &serde_json::Value| {
            specs
                .as_array()
                .unwrap()
                .iter()
                .position(|s| s["function"]["name"] == "sql")
                .unwrap()
        };
        let (i, j) = (sql(&fixture), sql(&ours));
        assert_eq!(
            fixture[i]["function"]["description"],
            format!(
                "Run one read-only SELECT against the database data. Returns at most {} rows.",
                super::MAX_SQL_ROWS
            )
        );
        fixture[i]["function"]["description"] = ours[j]["function"]["description"].clone();
        assert_eq!(
            serde_json::to_string(&ours).unwrap(),
            serde_json::to_string(&fixture).unwrap()
        );
    }
}
