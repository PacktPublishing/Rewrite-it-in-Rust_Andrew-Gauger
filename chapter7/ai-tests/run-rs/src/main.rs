//! Chapter 7 AI test-suite run: local model + Access tool-calling harness.
//!
//! Started as a port of `run.py` and now fixes the defects its 2026-10-21 run
//! exposed (see `../run-py-issues.md`). Same commands, flags, tools, briefs and
//! output files:
//!
//!     cargo run --release -- setup  --backend mlx
//!     cargo run --release -- selftest              # build the unit graph, exercise the tools
//!     cargo run --release -- run    --backend mlx
//!
//! Built for a small machine, not for a model that can hold the whole system in
//! context. Each round gets a fixed context budget (--context-budget, default
//! 16,384 tokens) and holds only what that round needs:
//!
//! 0. Survey. The model sees the inventory (unit names only) and writes an
//!    overview, naming the units to start from.
//! 1. Notes, breadth first. The harness splits the database into units (each VBA
//!    procedure, each saved query, each table) and finds the references between
//!    them. Each round pops the next unit off the queue and shows its source,
//!    the notes of the unit that led to it, and the references found in it. The
//!    model writes that unit's notes and names the references worth following;
//!    new ones join the back of the queue.
//! 2. Tests, in the same order. One round per unit whose notes found rules; the
//!    model writes that unit's Rust test file.
//!
//! The harness, not the model, keeps the memory: the queue, what was visited,
//! and every saved file. A round that would overflow the budget is retried from
//! its brief; a unit that fails twice is skipped and recorded. `walk.md` and
//! `run-metadata.md` are rewritten between rounds, so a run that dies keeps them.
//!
//! Every tool reads the Access database file itself. Tables, schema, saved
//! queries and data come through mdbtools; the VBA modules are decoded from the
//! file's own MSysAccessStorage table by `access_vba.py`, run with the MLX
//! virtualenv's Python exactly as `run.py` does. No exported `.bas` file and no
//! `part 2/README.md` (the answer key) is ever given to the model.

mod tools;
mod units;
mod walk;

use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};
use serde_json::{json, Map, Value};

use tools::run_cmd;
use units::Kind;
use walk::{Config, Harness, Phase, Walk, OVERVIEW};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const DEFAULT_MLX_REPO: &str = "mlx-community/Qwen3.8-27B-4bit";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(1800);
const SERVER_STARTUP: Duration = Duration::from_secs(900);
const WARN_AT: f64 = 0.75; // of a round's room: tell the model to save
const MAX_NUDGES: u32 = 3; // replies in a row with no tool call before the unit is skipped
const MAX_ATTEMPTS: u32 = 2; // budget overflows before the unit is skipped

const SYSTEM: &str = "You are reverse-engineering a small Microsoft Access sales system whose \
    author is gone and which has no documentation. You can read its Visual Basic for \
    Applications (VBA) modules, its saved queries, its schema and its data through the \
    tools provided. Explore before you write. Do not guess behavior the code does not show.";

#[derive(Clone, Copy, ValueEnum)]
enum Step {
    Setup,
    Run,
    Selftest,
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
enum Backend {
    Mlx,
    Llama,
}

impl Backend {
    fn name(self) -> &'static str {
        match self {
            Backend::Mlx => "mlx",
            Backend::Llama => "llama",
        }
    }
}

/// Chapter 7 AI test-suite run: a breadth-first walk of the Access system (Rust port of run.py).
#[derive(Parser)]
struct Args {
    #[arg(value_enum)]
    command: Step,
    #[arg(long, value_enum, default_value_t = Backend::Mlx)]
    backend: Backend,
    /// Hugging Face repo (MLX), or GGUF repo[:quant] for llama
    #[arg(long, default_value = DEFAULT_MLX_REPO)]
    repo: String,
    /// serve with mlx_vlm instead of mlx_lm
    #[arg(long)]
    vlm: bool,
    /// extra flags for the MLX server, e.g. memory limits; recorded in run-metadata.md
    #[arg(long, default_value = "", allow_hyphen_values = true)]
    server_args: String,
    /// prompt file [default: prompt.md beside run.py]
    #[arg(long)]
    prompt: Option<PathBuf>,
    #[arg(long, default_value_t = 7)]
    seed: i64,
    /// tokens a round may use, prompt and reply together; the llama.cpp context size
    #[arg(long, default_value_t = 16384)]
    context_budget: u32,
    /// longest single reply
    #[arg(long, default_value_t = 4096)]
    max_tokens: u32,
    /// model requests across the whole run
    #[arg(long, default_value_t = 3000)]
    max_turns: u32,
    #[arg(long, default_value_t = 8080)]
    port: u16,
    /// use a model server already listening on --port instead of starting one
    #[arg(long)]
    no_server: bool,
    /// where outputs/, logs/, crate/ and run-metadata.md go [default: beside run.py]
    #[arg(long)]
    workdir: Option<PathBuf>,
    /// Python that creates the MLX virtualenv during setup (run.py uses whichever Python runs it)
    #[arg(long, default_value = "python3")]
    python: String,
}

/// Where everything lives. `here` is `chapter7/ai-tests/`, this crate's parent.
struct Paths {
    here: PathBuf,
    work: PathBuf,
    db: PathBuf,
    out: PathBuf,
    logs: PathBuf,
    vba_dir: PathBuf,
    crate_dir: PathBuf,
    venv: PathBuf,
    models: PathBuf,
}

impl Paths {
    fn new(a: &Args) -> Self {
        let here = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate sits in ai-tests/")
            .to_path_buf();
        let work = a.workdir.clone().unwrap_or_else(|| here.clone());
        let work = fs::create_dir_all(&work)
            .and_then(|_| work.canonicalize())
            .unwrap_or(work);
        let home = PathBuf::from(std::env::var_os("HOME").expect("HOME is set"));
        let out = work.join("outputs");
        Paths {
            db: here.join("../../part 2/CairnSystems_History.accdb"),
            vba_dir: out.join("vba-from-db"), // what access_vba.py decoded, kept as evidence
            logs: work.join("logs"),
            crate_dir: work.join("crate"),
            venv: home.join("venvs").join("mlx"),
            models: home.join("models"),
            out,
            work,
            here,
        }
    }

    fn venv_bin(&self, name: &str) -> PathBuf {
        self.venv.join("bin").join(name)
    }
}

fn s(path: &Path) -> &str {
    path.to_str().expect("path is UTF-8")
}

/// Rough count for text the server hasn't measured yet: about 3 characters per token for code.
fn estimate_tokens(chars: usize) -> u64 {
    (chars / 3 + 1) as u64
}

// ---------------------------------------------------------------------------
// Setup: install runtime, download model
// ---------------------------------------------------------------------------
fn require_apple_silicon() -> Result<()> {
    if std::env::consts::ARCH != "aarch64" {
        let machine = run_cmd("uname", &["-m"], None);
        return Err(format!(
            "This is {}, not Apple Silicon. Run it on the M3.",
            machine.trim()
        )
        .into());
    }
    Ok(())
}

fn which(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

fn sh(cmd: &[&str]) -> Result<()> {
    println!("+ {}", cmd.join(" "));
    let status = Command::new(cmd[0]).args(&cmd[1..]).status()?;
    if !status.success() {
        return Err(format!("command failed ({status}): {}", cmd.join(" ")).into());
    }
    Ok(())
}

fn model_dir(a: &Args, p: &Paths) -> PathBuf {
    p.models.join(a.repo.rsplit('/').next().unwrap_or(&a.repo))
}

fn setup(a: &Args, p: &Paths) -> Result<()> {
    require_apple_silicon()?;
    if !which("mdb-queries") {
        sh(&["brew", "install", "mdbtools"])?;
    }
    match a.backend {
        Backend::Mlx => {
            if !p.venv_bin("python").exists() {
                sh(&[&a.python, "-m", "venv", s(&p.venv)])?;
            }
            sh(&[
                s(&p.venv_bin("pip")),
                "install",
                "-U",
                "mlx-lm",
                "mlx-vlm",
                "huggingface_hub[cli]",
                "oletools",
            ])?;
            let dest = model_dir(a, p);
            sh(&[
                s(&p.venv_bin("hf")),
                "download",
                &a.repo,
                "--local-dir",
                s(&dest),
            ])?;
            println!("\nModel at {}", dest.display());
        }
        Backend::Llama => {
            if !which("llama-server") {
                sh(&["brew", "install", "llama.cpp"])?;
            }
            println!("llama.cpp installed. The model downloads on first `run` via -hf.");
        }
    }
    Ok(())
}

fn mlx_command(a: &Args, p: &Paths) -> Vec<String> {
    // Qwen3.8 builds are tagged image-text; mlx_vlm.server accepts the same API.
    let module = if a.vlm {
        "mlx_vlm.server"
    } else {
        "mlx_lm.server"
    };
    let mut cmd: Vec<String> = [
        s(&p.venv_bin("python")),
        "-m",
        module,
        "--model",
        s(&model_dir(a, p)),
        "--port",
    ]
    .iter()
    .map(|x| x.to_string())
    .collect();
    cmd.push(a.port.to_string());
    cmd.extend(a.server_args.split_whitespace().map(String::from));
    cmd
}

fn llama_command(a: &Args) -> Result<Vec<String>> {
    if a.repo.is_empty() || a.repo == DEFAULT_MLX_REPO {
        return Err("--backend llama needs --repo <org>/<GGUF repo>[:quant]".into());
    }
    // The server's context is the round budget: the run never needs more.
    Ok(vec![
        "llama-server".into(),
        "-hf".into(),
        a.repo.clone(),
        "--jinja".into(),
        "--port".into(),
        a.port.to_string(),
        "-c".into(),
        a.context_budget.to_string(),
        "-ngl".into(),
        "99".into(),
    ])
}

// ---------------------------------------------------------------------------
// Preparing a run: decode the VBA, build the unit graph, start from nothing
// ---------------------------------------------------------------------------
fn clean_outputs(p: &Paths) -> Result<()> {
    for dir in [p.out.join("notes"), p.crate_dir.join("tests")] {
        if dir.exists() {
            fs::remove_dir_all(dir)?;
        }
    }
    Ok(())
}

/// The crate the saved test files land in; save_tests writes crate/tests/uNNN_<unit>.rs.
fn write_crate_skeleton(p: &Paths) -> Result<()> {
    fs::create_dir_all(p.crate_dir.join("tests"))?;
    if !p.crate_dir.join("Cargo.toml").exists() {
        fs::write(
            p.crate_dir.join("Cargo.toml"),
            "[package]\nname = \"cairn-ai-tests\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )?;
        fs::create_dir_all(p.crate_dir.join("src"))?;
        fs::write(p.crate_dir.join("src").join("lib.rs"), "")?;
    }
    Ok(())
}

fn prepare(a: &Args, p: &Paths) -> Result<Harness> {
    fs::create_dir_all(&p.out)?;
    fs::create_dir_all(&p.logs)?;
    clean_outputs(p)?;
    let vba = tools::load_vba(
        &p.venv_bin("python"),
        &p.here.join("access_vba.py"),
        &p.db,
        &p.vba_dir,
    )?;
    let units = units::build(&vba, s(&p.db));
    write_crate_skeleton(p)?;
    Ok(Harness {
        cfg: Config {
            db: p.db.clone(),
            out: p.out.clone(),
            crate_dir: p.crate_dir.clone(),
            max_tool_chars: 2000.max(a.context_budget as usize / 4), // about a twelfth of the budget in tokens
            context_budget: a.context_budget,
        },
        vba,
        units,
        walk: Walk::new(),
    })
}

// ---------------------------------------------------------------------------
// Server lifecycle and the walk
// ---------------------------------------------------------------------------

/// The model server. Dropping it stops the server, however the run ends.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        // SIGTERM first so the server can exit cleanly, then SIGKILL after 30 seconds.
        let _ = Command::new("kill").arg(self.0.id().to_string()).status();
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.0.try_wait() {
                return;
            }
            sleep(Duration::from_millis(200));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn http(base: &str, path: &str, payload: Option<&Value>, timeout: Duration) -> Result<Value> {
    let url = format!("{base}{path}");
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let response = match payload {
        None => agent.get(&url).call(),
        Some(body) => agent.post(&url).send_json(body),
    }
    .map_err(|e| format!("{url}: {e}"))?;
    Ok(response.into_json()?)
}

fn wait_for_server(base: &str, server: Option<&mut Server>) -> Result<()> {
    let mut server = server;
    let deadline = Instant::now() + SERVER_STARTUP;
    while Instant::now() < deadline {
        if let Some(s) = server.as_deref_mut() {
            if let Some(status) = s.0.try_wait()? {
                return Err(format!("model server exited early with {status}").into());
            }
        }
        if http(base, "/models", None, Duration::from_secs(5)).is_ok() {
            return Ok(());
        }
        sleep(Duration::from_secs(3));
    }
    Err("model server did not come up in time".into())
}

#[derive(Default)]
struct Stats {
    prompt_tokens: u64,
    completion_tokens: u64,
    max_prompt: u64,
    calls: u32,
    overflows: u32,
    turns: u32,
}

/// The transcript: one JSON record per line, flushed so it can be tailed during a run.
struct Transcript(File);

impl Transcript {
    fn log(&mut self, record: Value) -> Result<()> {
        writeln!(self.0, "{record}")?;
        self.0.flush()?;
        Ok(())
    }
}

/// What a round holds while it runs.
struct Round {
    messages: Vec<Value>,
    known: u64,           // tokens the server has measured for the current messages
    pending_chars: usize, // text appended since that measurement
    warned: bool,
    nudges: u32,
    cut_off: bool, // the last reply hit max_tokens before calling a tool
}

/// How much of a cut-off reply's reasoning to hand back, so the next reply can pick it up.
const REASONING_TAIL: usize = 1200;

/// The assistant message as it goes back to the server: without its reasoning, which the
/// server leaves out of the next prompt anyway, so the token count matches what is sent.
fn history_message(msg: &Value) -> Value {
    let mut kept = msg.clone();
    if let Some(m) = kept.as_object_mut() {
        m.remove("reasoning");
        m.remove("reasoning_content");
    }
    kept
}

/// Characters of a history message that the next prompt will carry.
fn message_chars(msg: &Value) -> usize {
    let content = msg["content"].as_str().map_or(0, |c| c.chars().count());
    let calls = msg
        .get("tool_calls")
        .map_or(0, |c| c.to_string().chars().count());
    content + calls
}

/// The nudge after a reply with no tool call. A reply cut off at max_tokens gets told so, with
/// the end of its reasoning: in run.py every such reply looked like a plain refusal, the model
/// started its reasoning over, hit the limit again, and the unit was skipped.
fn nudge_text(phase: Phase, msg: &Value, cut_off: bool, max_tokens: u32) -> String {
    let save = if phase == Phase::Notes {
        "save_notes"
    } else {
        "save_tests"
    };
    if !cut_off {
        return format!("Use the tools, then {save} for this unit.");
    }
    let reasoning = msg["reasoning"]
        .as_str()
        .or(msg["reasoning_content"].as_str())
        .filter(|r| !r.trim().is_empty())
        .or(msg["content"].as_str())
        .unwrap_or("");
    let n = reasoning.chars().count();
    let tail: String = reasoning
        .chars()
        .skip(n.saturating_sub(REASONING_TAIL))
        .collect();
    format!("Your last reply reached the {max_tokens}-token limit while still reasoning, before it called a \
             tool, so nothing happened, and that reasoning is not kept. Where it stopped:\n...{}\n\nKeep the \
             reasoning short and call {save} now.", tail.trim())
}

fn run(a: &Args, p: &Paths) -> Result<()> {
    if !a.no_server {
        require_apple_silicon()?;
    }
    let mut h = prepare(a, p)?;
    let count = |kind| h.units.iter().filter(|u| u.kind == kind).count();
    println!(
        "{} units: {} VBA, {} queries, {} tables",
        h.units.len(),
        count(Kind::Vba),
        count(Kind::Query),
        count(Kind::Table)
    );
    let prompt_path = a.prompt.clone().unwrap_or_else(|| p.here.join("prompt.md"));
    let prompt = fs::read_to_string(&prompt_path)
        .map_err(|e| format!("{}: {e}", prompt_path.display()))?
        .trim()
        .to_string();
    let room = a.context_budget as i64 - a.max_tokens as i64; // what a round may fill; the rest is the reply's
    if room < (a.context_budget / 4) as i64 {
        return Err(format!(
            "--max-tokens {} leaves too little of --context-budget {}",
            a.max_tokens, a.context_budget
        )
        .into());
    }
    let room = room as u64;
    let mut server = None;
    if !a.no_server {
        let cmd = match a.backend {
            Backend::Mlx => mlx_command(a, p),
            Backend::Llama => llama_command(a)?,
        };
        println!("+ {}", cmd.join(" "));
        let log = File::create(p.logs.join("server.log"))?;
        let child = Command::new(&cmd[0])
            .args(&cmd[1..])
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()
            .map_err(|e| format!("{}: {e}", cmd[0]))?;
        server = Some(Server(child));
    }
    let mut transcript = Transcript(File::create(p.out.join("transcript.jsonl"))?);
    let base = format!("http://127.0.0.1:{}/v1", a.port);
    let specs = tools::specs();
    let specs_chars = serde_json::to_string(&specs)?.chars().count();

    let mut stats = Stats::default();
    wait_for_server(&base, server.as_mut())?;
    let models = http(&base, "/models", None, REQUEST_TIMEOUT)?;
    let model_id = models["data"][0]["id"]
        .as_str()
        .map(String::from)
        .unwrap_or_else(|| a.repo.clone());
    let versions = server_versions(a, p);
    let started = Instant::now();
    let mut round: Option<Round> = None;
    let mut note = String::new();
    let mut turn = 0u32;
    let mut finished_walk = false;
    while turn < a.max_turns {
        if round.is_none() {
            // Between rounds, so a run that dies still leaves its walk and metadata behind.
            // run.py writes both only at the very end.
            stats.turns = turn;
            fs::write(p.out.join("walk.md"), h.walk_log())?;
            write_metadata(
                a,
                p,
                &h,
                &model_id,
                &stats,
                started.elapsed().as_secs_f64(),
                &versions,
                true,
            )?;
            if note.is_empty() {
                h.walk.current = h.walk.next_unit();
                h.walk.attempts = 0;
                if h.walk.current.is_none() {
                    finished_walk = true;
                    break;
                }
            }
            h.walk.rounds += 1;
            h.walk.saved = false;
            h.walk.rejected_notes = None;
            let brief = format!("{prompt}\n\n{}", h.brief(&note));
            let unit = h
                .walk
                .current
                .as_ref()
                .map(|i| i.unit.clone())
                .unwrap_or_default();
            transcript.log(json!({"round": h.walk.rounds, "phase": h.walk.phase.name(), "unit": unit, "brief": brief}))?;
            println!("round {} {}: {unit}", h.walk.rounds, h.walk.phase.name());
            // Nothing measured yet: estimate the system prompt, the brief and the tool list.
            let pending_chars = SYSTEM.chars().count() + brief.chars().count() + specs_chars;
            round = Some(Round {
                messages: vec![
                    json!({"role": "system", "content": SYSTEM}),
                    json!({"role": "user", "content": brief}),
                ],
                known: 0,
                pending_chars,
                warned: false,
                nudges: 0,
                cut_off: false,
            });
            note.clear();
        }
        let r = round.as_mut().expect("a round is open");
        if r.known + estimate_tokens(r.pending_chars) > room {
            stats.overflows += 1;
            h.walk.attempts += 1;
            round = None;
            if h.walk.attempts >= MAX_ATTEMPTS {
                h.walk
                    .skip_current(format!("ran out of context {MAX_ATTEMPTS} times"));
                println!(
                    "  skipped {}: out of context",
                    h.walk.current.as_ref().unwrap().unit
                );
            } else {
                note = "The last attempt at this unit ran out of context before saving. Read less and save sooner.".into();
            }
            continue;
        }
        turn += 1;
        // A reply cut off last time may use the room the round has left, up to twice max_tokens.
        // Prompt and reply together still never pass the budget: used <= room = budget - max_tokens.
        let used = r.known + estimate_tokens(r.pending_chars);
        let reply_cap = if r.cut_off {
            (a.context_budget as u64)
                .saturating_sub(used)
                .min(2 * a.max_tokens as u64)
                .max(a.max_tokens as u64)
        } else {
            a.max_tokens as u64
        };
        let resp = http(
            &base,
            "/chat/completions",
            Some(&json!({
                "model": model_id, "messages": r.messages, "tools": specs,
                "temperature": 0, "seed": a.seed, "max_tokens": reply_cap,
            })),
            REQUEST_TIMEOUT,
        )?;
        let prompt_tokens = resp["usage"]["prompt_tokens"].as_u64();
        let completion_tokens = resp["usage"]["completion_tokens"].as_u64().unwrap_or(0);
        stats.prompt_tokens += prompt_tokens.unwrap_or(0);
        stats.completion_tokens += completion_tokens;
        stats.max_prompt = stats.max_prompt.max(prompt_tokens.unwrap_or(0));
        let msg = resp["choices"][0]["message"].clone();
        if msg.is_null() {
            return Err(format!("no message in the server's reply: {resp}").into());
        }
        let finish_reason = resp["choices"][0]["finish_reason"]
            .as_str()
            .unwrap_or("")
            .to_string();
        // The measured prompt covers everything sent so far. What carries into the next prompt is
        // the reply's content and tool calls, not its reasoning. run.py added the whole completion,
        // reasoning included, which made rounds look full and skipped units as out of context.
        if let Some(measured) = prompt_tokens {
            r.known = measured;
            r.pending_chars = 0;
        }
        let kept = history_message(&msg);
        r.pending_chars += message_chars(&kept);
        r.messages.push(kept);
        transcript.log(json!({"turn": turn, "round": h.walk.rounds, "prompt_tokens": prompt_tokens,
                              "completion_tokens": completion_tokens, "finish_reason": finish_reason,
                              "assistant": msg}))?;

        let tool_calls = msg["tool_calls"].as_array().cloned().unwrap_or_default();
        if tool_calls.is_empty() {
            r.nudges += 1;
            if r.nudges > MAX_NUDGES {
                let nudges = r.nudges;
                round = None;
                h.walk
                    .skip_current(format!("{nudges} replies without a tool call"));
                println!(
                    "  skipped {}: no tool calls",
                    h.walk.current.as_ref().unwrap().unit
                );
                continue;
            }
            r.cut_off = finish_reason == "length";
            let nudge = nudge_text(h.walk.phase, &msg, r.cut_off, reply_cap as u32);
            if r.cut_off {
                println!("  [turn {turn}] reply cut off at {reply_cap} tokens with no tool call");
            }
            r.pending_chars += nudge.chars().count();
            r.messages.push(json!({"role": "user", "content": nudge}));
            continue;
        }
        r.nudges = 0;
        r.cut_off = false;
        for tc in &tool_calls {
            let name = tc["function"]["name"].as_str().unwrap_or("");
            let (args, result) = match &tc["function"]["arguments"] {
                Value::String(raw) => match serde_json::from_str::<Value>(raw) {
                    Ok(parsed) => {
                        let result = call(&mut h, name, &parsed);
                        (parsed, result)
                    }
                    Err(e) => (
                        Value::String(raw.clone()),
                        format!("error: arguments are not valid JSON ({e}); call {name} again"),
                    ),
                },
                other => (other.clone(), call(&mut h, name, other)),
            };
            stats.calls += 1;
            let shown: String = args.to_string().chars().take(80).collect();
            println!(
                "  [turn {turn}] {name}({shown}) -> {} chars",
                result.chars().count()
            );
            r.messages.push(json!({"role": "tool", "tool_call_id": tc["id"].as_str().unwrap_or(""), "content": result}));
            r.pending_chars += result.chars().count();
            transcript.log(json!({"turn": turn, "tool": name, "args": args, "result": result}))?;
        }
        let used = r.known + estimate_tokens(r.pending_chars);
        if h.walk.saved {
            round = None;
        } else if !r.warned && used as f64 > WARN_AT * room as f64 {
            let save = if h.walk.phase == Phase::Notes {
                "save_notes"
            } else {
                "save_tests"
            };
            let warning =
                format!("Context: about {used} of {room} tokens used. {save} for this unit now.");
            r.pending_chars += warning.chars().count();
            r.messages.push(json!({"role": "user", "content": warning}));
            r.warned = true;
        }
    }
    if !finished_walk {
        println!("Stopped after {} turns without finishing.", a.max_turns);
    }
    stats.turns = turn;
    let elapsed = started.elapsed().as_secs_f64();
    fs::write(p.out.join("walk.md"), h.walk_log())?;
    write_metadata(a, p, &h, &model_id, &stats, elapsed, &versions, false)?;
    println!("Done in {elapsed:.0}s: phase {}, {} rounds, {} tool calls, largest prompt {} tokens. See {}",
             h.walk.phase.name(), h.walk.rounds, stats.calls, stats.max_prompt, p.out.display());
    Ok(())
}

/// A tool call with Python's `a or {}`: empty arguments mean none.
fn call(h: &mut Harness, name: &str, args: &Value) -> String {
    let empty = match args {
        Value::Null => true,
        Value::Object(m) => m.is_empty(),
        Value::Array(v) => v.is_empty(),
        Value::String(s) => s.is_empty(),
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64() == Some(0.0),
    };
    let none = Value::Object(Map::new());
    h.call(name, if empty { &none } else { args })
}

/// The runtime's package versions, read once per run.
fn server_versions(a: &Args, p: &Paths) -> String {
    if a.no_server {
        return format!("(external server at http://127.0.0.1:{}/v1)", a.port);
    }
    match a.backend {
        Backend::Mlx => run_cmd(s(&p.venv_bin("pip")), &["freeze"], None)
            .lines()
            .filter(|l| {
                let l = l.to_lowercase();
                l.starts_with("mlx") || l.starts_with("huggingface")
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Backend::Llama => run_cmd("llama-server", &["--version"], None),
    }
}

#[allow(clippy::too_many_arguments)]
fn write_metadata(
    a: &Args,
    p: &Paths,
    h: &Harness,
    model_id: &str,
    stats: &Stats,
    elapsed: f64,
    versions: &str,
    running: bool,
) -> Result<()> {
    let chip = run_cmd("sysctl", &["-n", "machdep.cpu.brand_string"], None)
        .trim()
        .to_string();
    let mem = run_cmd("sysctl", &["-n", "hw.memsize"], None)
        .trim()
        .parse::<u64>()
        .unwrap_or(0)
        >> 30;
    let macos = run_cmd("sw_vers", &["-productVersion"], None)
        .trim()
        .to_string();
    let date = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S");
    let server_flags = if a.server_args.is_empty() {
        "(none)"
    } else {
        &a.server_args
    };
    let visited = h.walk.order.len() - usize::from(h.walk.order.iter().any(|u| u == OVERVIEW));
    let with_rules = h.walk.records().filter(|(_, i)| i.has_rules).count();
    let mut test_files: Vec<PathBuf> = fs::read_dir(p.crate_dir.join("tests"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|x| x == "rs"))
        .collect();
    test_files.sort();
    let tests: usize = test_files
        .iter()
        .map(|f| {
            fs::read_to_string(f)
                .map(|t| t.matches("#[test]").count())
                .unwrap_or(0)
        })
        .sum();
    let finished = match (h.walk.phase, running) {
        (Phase::Finished, _) => "True",
        (_, true) => "False (still running; this file is rewritten after every round)",
        _ => "False",
    };
    fs::write(
        p.work.join("run-metadata.md"),
        format!(
            "# Run metadata

- Date: {date}
- Backend: {backend}
- Model repo: {repo}
- Model id reported by server: {model_id}
- Chip / memory: {chip}, {mem} GB
- macOS: {macos}
- Server flags: {server_flags}
- Settings: temperature 0, seed {seed}, max_tokens {max_tokens}, max_turns {max_turns}
- Context budget: {budget} tokens per round; tool results capped at {cap} characters
- Largest prompt sent: {max_prompt} tokens
- Units in the database: {units}; notes saved for {visited}; {with_rules} with rules
- Test files: {files}, {tests} tests
- Rounds: {rounds} ({overflows} ran out of context); turns: {turns}; tool calls: {calls}
- Tokens: {prompt_tokens} prompt, {completion_tokens} completion
- Wall time: {elapsed:.0}s
- Finished: {finished} (last phase: {phase})
- Walk: outputs/walk.md
- Revision hash: fill in from the model page or `{DEFAULT_MLX_REPO}` download log

```
{versions}
```
",
            backend = a.backend.name(),
            repo = a.repo,
            seed = a.seed,
            max_tokens = a.max_tokens,
            max_turns = a.max_turns,
            budget = a.context_budget,
            cap = h.cfg.max_tool_chars,
            max_prompt = stats.max_prompt,
            units = h.units.len(),
            files = test_files.len(),
            rounds = h.walk.rounds,
            overflows = stats.overflows,
            turns = stats.turns,
            calls = stats.calls,
            prompt_tokens = stats.prompt_tokens,
            completion_tokens = stats.completion_tokens,
            phase = h.walk.phase.name(),
            versions = versions.trim()
        ),
    )?;
    Ok(())
}

/// Build the unit graph and exercise every tool and refusal. No model needed.
fn selftest(a: &Args, p: &Paths) -> Result<()> {
    let mut h = prepare(a, p)?;
    let count = |kind| h.units.iter().filter(|u| u.kind == kind).count();
    println!(
        "--- units: {} ({{'vba': {}, 'query': {}, 'table': {}}})",
        h.units.len(),
        count(Kind::Vba),
        count(Kind::Query),
        count(Kind::Table)
    );
    let sample = "vba:CairnForms.NextNumber";
    let u = h.units.get(sample).ok_or("sample unit missing")?;
    println!("--- {sample}: {}; refs: {}", u.label, u.refs.join(", "));
    let routing = h
        .units
        .get("query:qryRMARouting")
        .ok_or("qryRMARouting missing")?;
    println!("--- query:qryRMARouting refs: {}", routing.refs.join(", "));
    let biggest = h
        .units
        .iter()
        .fold(None, |best: Option<&units::Unit>, u| match best {
            Some(b) if b.text.chars().count() >= u.text.chars().count() => Some(b),
            _ => Some(u),
        })
        .ok_or("no units")?;
    println!(
        "--- largest unit: {} ({} characters)",
        biggest.label,
        biggest.text.chars().count()
    );
    h.walk.current = h.walk.next_unit();
    let survey = h.brief("");
    println!(
        "--- survey brief: {} characters, about {} tokens",
        survey.chars().count(),
        estimate_tokens(survey.chars().count())
    );
    let checks = [
        ("get_saved_query", json!({"name": "qryQuoteTotals"})),
        ("get_schema", json!({"table": "RMAs"})),
        (
            "read_vba",
            json!({"module": "CairnForms", "start": 1, "end": 3}),
        ),
        ("search_vba", json!({"pattern": "Function NextNumber"})),
        (
            "sql",
            json!({"query": "SELECT OrderNumber FROM Orders WHERE OrderID < 3"}),
        ),
        ("sql", json!({"query": "DELETE FROM Orders"})),
        // Each of these came back blank, with no error, from run.py's sql tool.
        (
            "sql",
            json!({"query": "SELECT OrderNumber FROM Orders ORDER BY OrderNumber DESC"}),
        ),
        (
            "sql",
            json!({"query": "SELECT CustomerID FROM Customers WHERE Notes IS NULL"}),
        ),
        (
            "sql",
            json!({"query": "SELECT COUNT(*) FROM Orders WHERE OrderID < 3"}),
        ),
        (
            "sql",
            json!({"query": "SELECT OrderNumber FROM Orders WHERE OrderID = -1"}),
        ),
        (
            "sql",
            json!({"query": "SELECT OrderNumber FROM Orders WHERE OrderNumber LIKE 'SO-2026-12%'"}),
        ),
        ("read_vba", json!({"module": "README.md"})),
        ("read_vba", json!({"module": "CairnHistory"})),
        ("get_saved_query", json!({"name": "x; rm -rf /"})),
        ("save_tests", json!({"code": "#[test] fn t() {}"})),
        (
            "save_notes",
            json!({"text": "x", "follow": ["vba:Nope.Nothing"]}),
        ),
        (
            "save_notes",
            json!({"text": "x".repeat(tools::NOTE_CHARS + 1)}),
        ),
        (
            "save_notes",
            json!({"text": "Survey: a quoting and order system.", "follow": [sample, "query:qryRMARouting"]}),
        ),
        ("save_notes", json!({"text": "again"})),
    ];
    for (name, args) in checks {
        let shown: Map<String, Value> = args
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                let v = match v.as_str() {
                    Some(t) if t.chars().count() > 40 => {
                        Value::String(format!("{}...", t.chars().take(40).collect::<String>()))
                    }
                    _ => v.clone(),
                };
                (k.clone(), v)
            })
            .collect();
        let out: String = h.call(name, &args).chars().take(300).collect();
        println!("--- {name} {}\n{out}", Value::Object(shown));
    }
    h.walk.current = h.walk.next_unit();
    let unit_brief = h.brief("");
    println!(
        "--- next unit {}: brief {} characters, about {} tokens",
        h.walk.current.as_ref().map_or("", |i| i.unit.as_str()),
        unit_brief.chars().count(),
        estimate_tokens(unit_brief.chars().count())
    );
    clean_outputs(p)?;
    write_crate_skeleton(p)
}

fn main() {
    let args = Args::parse();
    let paths = Paths::new(&args);
    let result = match args.command {
        Step::Setup => setup(&args, &paths),
        Step::Run => run(&args, &paths),
        Step::Selftest => selftest(&args, &paths),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod parity {
    /// The system prompt must match run.py's word for word.
    #[test]
    fn system_prompt_matches_run_py() {
        let fixture: String =
            serde_json::from_str(include_str!("../tests/fixtures/system-prompt.json")).unwrap();
        assert_eq!(super::SYSTEM, fixture);
    }
}
