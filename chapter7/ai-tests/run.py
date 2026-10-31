#!/usr/bin/env python3
"""Chapter 7 AI test-suite run: local model + Access tool-calling harness.

Sets up a local model server (MLX or llama.cpp), then walks the Cairn Access
system breadth first with the model, one unit per round, writing one notes
file per unit and then one Rust test file per unit that has rules.
Everything is logged so the chapter can show a real run.

    python3 run.py setup  --backend mlx
    python3 run.py selftest                 # build the unit graph, exercise the tools, no model
    python3 run.py run    --backend mlx

Built for a small machine, not for a model that can hold the whole system in
context. Each round gets a fixed context budget (--context-budget, default
16,384 tokens) and holds only what that round needs:

0. Survey. The model sees the inventory (unit names only) and writes an
   overview, naming the units to start from.
1. Notes, breadth first. The harness splits the database into units (each VBA
   procedure, each saved query, each table) and finds the references between
   them. Each round pops the next unit off the queue and shows its source,
   the notes of the unit that led to it, and the references found in it. The
   model writes that unit's notes and names the references worth following;
   new ones join the back of the queue.
2. Tests, in the same order. One round per unit whose notes found rules; the
   model writes that unit's Rust test file.

The harness, not the model, keeps the memory: the queue, what was visited,
and every saved file. A round that would overflow the budget is retried from
its brief; a unit that fails twice is skipped and recorded.

Every tool reads the Access database file itself. Tables, schema, saved
queries and data come through mdbtools; the VBA modules are decoded from the
file's own MSysAccessStorage table by ``access_vba.py``. No exported ``.bas``
file and no ``part 2/README.md`` (the answer key) is ever given to the model.

Only the standard library is needed to drive the loop. ``setup`` creates a
virtualenv for ``mlx-lm`` (or installs llama.cpp with Homebrew) and downloads
the model from Hugging Face. See SPEC.md for the reasoning.
"""

import argparse
import collections
import datetime
import json
import pathlib
import platform
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent
DB = REPO / "part 2" / "CairnSystems_History.accdb"

VENV = pathlib.Path.home() / "venvs" / "mlx"
MODELS = pathlib.Path.home() / "models"
DEFAULT_MLX_REPO = "mlx-community/Qwen3.8-27B-4bit"

DEFAULT_BUDGET = 16384
MAX_READ_LINES = 300
MAX_SQL_ROWS = 20
MAX_SEARCH_HITS = 40
NOTE_CHARS = 2000
TEST_CHARS = 6000
WARN_AT = 0.75            # of a round's room: tell the model to save
MAX_NUDGES = 3            # replies in a row with no tool call before the unit is skipped
MAX_ATTEMPTS = 2          # budget overflows before the unit is skipped
OVERVIEW = "overview"

# Set by configure() from the command line.
WORK = OUT = LOGS = VBA_DIR = CRATE = None
BASE_URL = None
MAX_TOOL_CHARS = None


def configure(args):
    global WORK, OUT, LOGS, VBA_DIR, CRATE, BASE_URL, MAX_TOOL_CHARS
    WORK = pathlib.Path(args.workdir).resolve()
    OUT = WORK / "outputs"
    LOGS = WORK / "logs"
    VBA_DIR = OUT / "vba-from-db"     # what access_vba.py decoded, kept as evidence
    CRATE = WORK / "crate"
    BASE_URL = f"http://127.0.0.1:{args.port}/v1"
    MAX_TOOL_CHARS = max(2000, args.context_budget // 4)   # about a twelfth of the budget in tokens


def _run(cmd, stdin=None):
    p = subprocess.run(cmd, input=stdin, capture_output=True, text=True, errors="replace")
    if p.returncode != 0:
        return f"error: {p.stderr.strip() or p.stdout.strip()}"
    return p.stdout


def _clip(text):
    if len(text) <= MAX_TOOL_CHARS:
        return text
    return text[:MAX_TOOL_CHARS] + f"\n... [truncated, {len(text) - MAX_TOOL_CHARS} more characters]"


def _strip_banner(sql):
    """mdbtools prefixes every schema with a licence banner; it costs tokens and says nothing."""
    lines = [l for l in sql.splitlines() if not l.startswith("--") and not l.startswith("SET ")]
    return "\n".join(lines).strip() + "\n"


def slug(unit):
    """A unit id as a file name that is also a valid Rust test-crate name."""
    return re.sub(r"[^a-z0-9]+", "_", unit.lower()).strip("_")


# ---------------------------------------------------------------------------
# The unit graph: what the harness walks
# ---------------------------------------------------------------------------
_VBA = {}       # module -> lines
UNITS = {}      # id -> {"kind", "label", "module", "first", "last", "text", "refs"}

PROC = re.compile(r"^\s*(?:(?:Public|Private|Friend)\s+)?(?:Static\s+)?"
                  r"(?:Function|Sub|Property\s+(?:Get|Let|Set))\s+(\w+)", re.I)
END_PROC = re.compile(r"^\s*End\s+(?:Function|Sub|Property)\b", re.I)


def load_vba():
    """Decode the VBA project out of the database (needs oletools, installed by setup)."""
    if VBA_DIR.exists():
        shutil.rmtree(VBA_DIR)
    p = subprocess.run([str(VENV / "bin" / "python"), str(HERE / "access_vba.py"), str(DB), str(VBA_DIR)],
                       capture_output=True, text=True)
    if p.returncode != 0:
        sys.exit(f"could not read VBA from {DB.name}:\n{p.stderr}")
    print(p.stdout.strip())
    _VBA.clear()
    _VBA.update({f.stem: f.read_text().splitlines() for f in sorted(VBA_DIR.glob("*.bas"))})


def split_module(module, lines):
    """One unit per procedure (with the comment block just above it), plus one for the
    module's declarations: everything outside a procedure."""
    procs, start, name = [], None, None
    for i, line in enumerate(lines):
        if start is None:
            m = PROC.match(line)
            if m:
                start, name = i, m.group(1)
                while start > 0 and lines[start - 1].lstrip().startswith("'"):
                    start -= 1          # the comment block above belongs to the procedure
        elif END_PROC.match(line):
            procs.append((name, start, i))
            start = None
    inside = set()
    for _, a, b in procs:
        inside.update(range(a, b + 1))
    decl = [i for i in range(len(lines)) if i not in inside]
    units = [(f"vba:{module}", decl)]
    units += [(f"vba:{module}.{name}", list(range(a, b + 1))) for name, a, b in procs]
    return units


def build_units():
    UNITS.clear()
    for module, lines in _VBA.items():
        for uid, idx in split_module(module, lines):
            text = "\n".join(f"{i + 1}: {lines[i]}" for i in idx)
            label = (f"{module} declarations, everything outside its procedures" if uid == f"vba:{module}"
                     else f"{module} lines {idx[0] + 1}-{idx[-1] + 1}")
            UNITS[uid] = {"kind": "vba", "module": module, "first": idx[0] + 1 if idx else 0,
                          "last": idx[-1] + 1 if idx else 0, "text": text, "label": label}
    for name in _run(["mdb-queries", "-1", str(DB)]).split():
        UNITS[f"query:{name}"] = {"kind": "query", "label": "saved query",
                                  "text": _run(["mdb-queries", str(DB), name]).strip()}
    for name in _run(["mdb-tables", "-1", str(DB)]).split():
        UNITS[f"table:{name}"] = {"kind": "table", "label": "table",
                                  "text": _strip_banner(_run(["mdb-schema", "-T", name, str(DB), "postgres"])).strip()}
    find_references()


def find_references():
    """Candidate edges: every other unit whose name appears as a word in this one."""
    by_name = collections.defaultdict(list)
    for uid in UNITS:
        by_name[uid.split(":", 1)[1].rsplit(".", 1)[-1]].append(uid)
    # Module names are too common to mean a reference, and one- or two-letter helpers match every SQL alias.
    by_name = {n: ids for n, ids in by_name.items() if n not in _VBA and len(n) >= 3}
    word = re.compile(r"\b(" + "|".join(sorted(map(re.escape, by_name), key=len, reverse=True)) + r")\b")
    for uid, unit in UNITS.items():
        refs = []
        body = re.sub(r"^\d+: ", "", unit["text"], flags=re.M)
        for name in dict.fromkeys(word.findall(body)):
            targets = [t for t in by_name[name] if t != uid]
            same_module = [t for t in targets if UNITS[t].get("module") == unit.get("module")]
            refs += same_module or targets
        unit["refs"] = list(dict.fromkeys(refs))


def inventory():
    """Names only, grouped, for the survey round."""
    lines = []
    for module, src in _VBA.items():
        procs = [u.split(".", 1)[1] for u in UNITS if u.startswith(f"vba:{module}.")]
        lines.append(f"VBA module {module} ({len(src)} lines, {len(procs)} procedures; unit vba:{module} "
                     f"is its declarations): " + ", ".join(procs))
    lines.append("Saved queries: " + ", ".join(u.split(":", 1)[1] for u in UNITS if u.startswith("query:")))
    lines.append("Tables: " + ", ".join(u.split(":", 1)[1] for u in UNITS if u.startswith("table:")))
    lines.append("Unit ids look like vba:CairnForms.NextNumber, vba:CairnForms, query:qryQuoteTotals, table:Orders.")
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Walk state: the queue, what was visited, what was saved
# ---------------------------------------------------------------------------
STATE = {}


def reset_state():
    STATE.clear()
    STATE.update({
        "phase": "notes",           # notes -> tests -> finished
        "queue": collections.deque([(OVERVIEW, None, 0)]),   # (unit, parent, level)
        "seen": {OVERVIEW},
        "order": [],                # units in the order their notes were saved
        "info": {},                 # unit -> {"parent", "level", "follow", "has_rules", "file", "tests", "skipped"}
        "current": None,            # (unit, parent, level) being worked on
        "attempts": 0,
        "saved": False,             # the current unit's file was saved this round
        "rounds": 0,
        "test_queue": collections.deque(),
    })


def notes_path(unit):
    return OUT / "notes" / f"{STATE['info'][unit]['index']:03d}-{slug(unit)}.md"


def test_path(unit):
    return CRATE / "tests" / f"u{STATE['info'][unit]['index']:03d}_{slug(unit)}.rs"


# ---------------------------------------------------------------------------
# Tools the model can call. The readers are read-only; the two savers write
# only the current unit's file into the run's own outputs.
# ---------------------------------------------------------------------------
def _module(name):
    name = name.removesuffix(".bas")
    if name not in _VBA:
        return None, f"error: no module {name!r}. Modules: {', '.join(_VBA)}"
    return _VBA[name], None


def tool_get_schema(table):
    unit = UNITS.get(f"table:{table}")
    return unit["text"] if unit else f"error: no table {table!r}"


def tool_get_saved_query(name):
    unit = UNITS.get(f"query:{name}")
    return unit["text"] if unit else f"error: no saved query {name!r}"


def tool_read_vba(module, start=1, end=200):
    lines, err = _module(module)
    if err:
        return err
    start = max(1, int(start))
    end = min(len(lines), int(end), start + MAX_READ_LINES - 1)
    out, size, room = [], 0, MAX_TOOL_CHARS - 100
    for i in range(start, end + 1):
        line = f"{i}: {lines[i - 1]}"
        if out and size + len(line) + 1 > room:
            out.append(f"... [stopped after line {i - 1} to fit; continue with start={i}]")
            break
        out.append(line)
        size += len(line) + 1
    return "\n".join(out)


def tool_search_vba(pattern, module=None):
    try:
        pat = re.compile(pattern, re.IGNORECASE)
    except re.error as e:
        return f"error: bad regex: {e}"
    names = [module] if module else list(_VBA)
    hits = []
    for name in names:
        lines, err = _module(name)
        if err:
            return err
        for i, line in enumerate(lines, 1):
            if pat.search(line):
                hits.append(f"{name.removesuffix('.bas')}:{i}: {line.strip()}")
    if len(hits) > MAX_SEARCH_HITS:
        return "\n".join(hits[:MAX_SEARCH_HITS]) + f"\n... [{len(hits) - MAX_SEARCH_HITS} more matches; narrow the pattern]"
    return "\n".join(hits) or "no matches"


def tool_sql(query):
    q = query.strip().rstrip(";")
    if not re.match(r"(?is)^select\b", q) or ";" in q:
        return "error: only a single SELECT statement is allowed"
    out = _run(["mdb-sql", "-P", "-F", "-d", "|", str(DB)], stdin=q + "\n")
    lines = out.splitlines()
    if len(lines) > MAX_SQL_ROWS + 1:
        out = "\n".join(lines[:MAX_SQL_ROWS + 1]) + f"\n... [{len(lines) - MAX_SQL_ROWS - 1} more rows not shown; add a WHERE clause]"
    return out


def tool_read_notes(unit):
    info = STATE["info"].get(unit)
    if not info or "index" not in info:
        return f"error: no notes for {unit!r} yet"
    return notes_path(unit).read_text()


def _ids(value):
    if value is None:
        return []
    if isinstance(value, str):
        value = [v for v in re.split(r"[,\s]+", value) if v]
    return [str(v).strip() for v in value]


def _flag(v):
    return v is True or str(v).strip().lower() in ("true", "1", "yes")


def tool_save_notes(text, follow=None, has_rules=False):
    if STATE["phase"] != "notes":
        return "error: the notes phase is over; save_tests for this unit."
    if STATE["saved"]:
        return "error: this unit's notes are already saved."
    unit, parent, level = STATE["current"]
    text = text.strip()
    if not text:
        return "error: empty notes"
    if len(text) > NOTE_CHARS:
        return f"error: {len(text)} characters; a unit's notes hold at most {NOTE_CHARS}. Tighten them."
    follow = list(dict.fromkeys(_ids(follow)))
    unknown = [f for f in follow if f not in UNITS]
    if unknown:
        return f"error: unknown unit ids: {', '.join(unknown)}. Nothing saved; use ids from the brief."
    info = STATE["info"].setdefault(unit, {})
    info.update({"parent": parent, "level": level, "follow": follow,
                 "has_rules": unit != OVERVIEW and _flag(has_rules), "index": len(STATE["order"])})
    STATE["order"].append(unit)
    path = notes_path(unit)
    path.parent.mkdir(parents=True, exist_ok=True)
    header = f"<!-- unit: {unit} | reached from: {parent or '(start)'} | level {level} | follow: {', '.join(follow) or '(none)'} -->\n"
    path.write_text(header + text + "\n")
    queued = [f for f in follow if f not in STATE["seen"]]
    for f in queued:
        STATE["seen"].add(f)
        STATE["queue"].append((f, unit, level + 1))
    STATE["saved"] = True
    return (f"saved {path.name}. Queued {len(queued)} new units"
            + (f" ({len(follow) - len(queued)} already seen)" if len(follow) > len(queued) else "")
            + f"; {len(STATE['queue'])} waiting. This round is done.")


def tool_save_tests(code, skip=None):
    if STATE["phase"] != "tests":
        return "error: tests come after the notes; save_notes for this unit."
    if STATE["saved"]:
        return "error: this unit's tests are already saved."
    unit, _, _ = STATE["current"]
    code = (code or "").strip("\n")
    if not code.strip():
        if not skip:
            return "error: empty test file. Pass skip with a reason if this unit has nothing to test."
        STATE["info"][unit]["skipped"] = f"no tests: {skip}"
        STATE["saved"] = True
        return "recorded: no tests for this unit. This round is done."
    if len(code) > TEST_CHARS:
        return f"error: {len(code)} characters; a unit's test file holds at most {TEST_CHARS}. Keep the clearest tests."
    path = test_path(unit)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(code + "\n")
    count = len(re.findall(r"#\[test\]", code))
    STATE["info"][unit]["tests"] = count
    STATE["saved"] = True
    return f"saved {path.name} ({count} tests). This round is done."


# Parameters are (type, description, required).
TOOLS = {
    "get_schema": (tool_get_schema, "Get the CREATE TABLE statement for one table.",
                   {"table": ("string", "Table name", True)}),
    "get_saved_query": (tool_get_saved_query, "Get the SQL of one saved query.",
                        {"name": ("string", "Saved query name, e.g. qryQuoteTotals", True)}),
    "read_vba": (tool_read_vba, "Read lines of a VBA module. Stops early at a size limit and says where to continue.",
                 {"module": ("string", "Module name", True),
                  "start": ("integer", "First line, 1-based", False),
                  "end": ("integer", "Last line", False)}),
    "search_vba": (tool_search_vba, "Regex search (case-insensitive) across the VBA modules.",
                   {"pattern": ("string", "Regular expression", True),
                    "module": ("string", "Optional: limit to one module", False)}),
    "sql": (tool_sql, f"Run one read-only SELECT against the database data. Returns at most {MAX_SQL_ROWS} rows.",
            {"query": ("string", "A single SELECT statement", True)}),
    "read_notes": (tool_read_notes, "Read the notes saved for another unit.",
                   {"unit": ("string", "Unit id, e.g. vba:CairnForms.NextNumber", True)}),
    "save_notes": (tool_save_notes,
                   f"Save this round's unit notes (at most {NOTE_CHARS} characters) and name the units to "
                   "visit later. Ends the round.",
                   {"text": ("string", "What the unit does: each rule as a sentence, with line numbers or SQL as evidence", True),
                    "follow": ("array", "Unit ids worth visiting, from the references in the brief", False),
                    "has_rules": ("boolean", "true if this unit implements a business rule worth testing", False)}),
    "save_tests": (tool_save_tests,
                   f"Save this round's unit Rust test file (at most {TEST_CHARS} characters). Ends the round.",
                   {"code": ("string", "One Rust test file", True),
                    "skip": ("string", "Instead of code: why this unit has nothing to test", False)}),
}


def tool_specs():
    specs = []
    for name, (_, desc, params) in TOOLS.items():
        props = {}
        for k, (t, d, _) in params.items():
            props[k] = {"type": t, "description": d}
            if t == "array":
                props[k]["items"] = {"type": "string"}
        specs.append({"type": "function", "function": {
            "name": name, "description": desc,
            "parameters": {"type": "object", "properties": props,
                           "required": [k for k, (_, _, req) in params.items() if req]}}})
    return specs


def call_tool(name, args):
    if name not in TOOLS:
        return f"error: no such tool {name!r}"
    try:
        return _clip(str(TOOLS[name][0](**args)))
    except (TypeError, ValueError) as e:
        return f"error: bad arguments: {e}"


# ---------------------------------------------------------------------------
# The brief: everything a round starts from
# ---------------------------------------------------------------------------
def unit_source(unit):
    u = UNITS[unit]
    text = u["text"] or "(empty)"
    if len(text) <= MAX_TOOL_CHARS:
        return text
    head = text[:MAX_TOOL_CHARS].rsplit("\n", 1)[0]
    shown = int(re.findall(r"^(\d+): ", head, re.M)[-1]) if u["kind"] == "vba" else None
    more = (f"read_vba module={u['module']} start={shown + 1} for the rest, up to line {u['last']}"
            if shown else "the rest is cut")
    return head + f"\n... [unit continues; {more}]"


def brief(args, note=""):
    unit, parent, level = STATE["current"]
    phase = STATE["phase"]
    parts = [f"Round {STATE['rounds']}. Phase: {phase}. Context budget: {args.context_budget} tokens per round."]
    if note:
        parts.append(note)
    if unit == OVERVIEW:
        parts.append("This is the survey round. Inventory of the system (names only):\n" + inventory())
        parts.append("Next: save_notes with an overview of what this system appears to do, and in follow "
                     "list the units to start from, most important first. Every other unit is reached "
                     "from those, breadth first.")
        return "\n\n".join(parts)
    u = UNITS[unit]
    waiting = len(STATE["queue"]) if phase == "notes" else len(STATE["test_queue"])
    parts.append(f"Unit: {unit} ({u['label']}). Breadth-first level {level}, reached from "
                 f"{parent or '(start)'}. {waiting} units wait after this one.")
    if phase == "notes":
        if parent:
            parts.append(f"Notes of {parent}, which led here:\n" + notes_path(parent).read_text().strip())
        refs = u["refs"]
        new = [r for r in refs if r not in STATE["seen"]]
        parts.append("References found in this unit: " + (", ".join(refs) or "none")
                     + (f"\nNot yet seen (candidates to follow): {', '.join(new)}" if new else ""))
        parts.append("Source:\n" + unit_source(unit))
        parts.append("Next: check anything you need with the tools, then save_notes for this unit: the "
                     "rules it implements, each as a sentence with its evidence. In follow, name the "
                     "references worth visiting. Set has_rules if there is something to test.")
    else:
        parts.append("Your notes for this unit:\n" + notes_path(unit).read_text().strip())
        parts.append("Source:\n" + unit_source(unit))
        parts.append("Next: check anything you need, then save_tests with one Rust test file for this "
                     "unit's rules, or skip with a reason if there is nothing to test.")
    return "\n\n".join(parts)


# ---------------------------------------------------------------------------
# Setup: install runtime, download model
# ---------------------------------------------------------------------------
def require_apple_silicon():
    if platform.machine() != "arm64":
        sys.exit(f"This is {platform.machine()}, not Apple Silicon. Run it on the M3.")


def sh(cmd, **kw):
    print("+", " ".join(map(str, cmd)))
    subprocess.run(cmd, check=True, **kw)


def setup(args):
    require_apple_silicon()
    if not shutil.which("mdb-queries"):
        sh(["brew", "install", "mdbtools"])
    if args.backend == "mlx":
        if not (VENV / "bin" / "python").exists():
            sh([sys.executable, "-m", "venv", str(VENV)])
        pip = [str(VENV / "bin" / "pip")]
        sh(pip + ["install", "-U", "mlx-lm", "mlx-vlm", "huggingface_hub[cli]", "oletools"])
        dest = MODELS / args.repo.split("/")[-1]
        sh([str(VENV / "bin" / "hf"), "download", args.repo, "--local-dir", str(dest)])
        print(f"\nModel at {dest}")
    else:
        if not shutil.which("llama-server"):
            sh(["brew", "install", "llama.cpp"])
        print("llama.cpp installed. The model downloads on first `run` via -hf.")


def mlx_command(args):
    dest = MODELS / args.repo.split("/")[-1]
    # Qwen3.8 builds are tagged image-text; mlx_vlm.server accepts the same API.
    module = "mlx_vlm.server" if args.vlm else "mlx_lm.server"
    return ([str(VENV / "bin" / "python"), "-m", module, "--model", str(dest), "--port", str(args.port)]
            + args.server_args.split())


def llama_command(args):
    if not args.repo or args.repo == DEFAULT_MLX_REPO:
        sys.exit("--backend llama needs --repo <org>/<GGUF repo>[:quant]")
    # The server's context is the round budget: the run never needs more.
    return ["llama-server", "-hf", args.repo, "--jinja", "--port", str(args.port),
            "-c", str(args.context_budget), "-ngl", "99"]


# ---------------------------------------------------------------------------
# Server lifecycle and the walk
# ---------------------------------------------------------------------------
def http(path, payload=None, timeout=1800):
    req = urllib.request.Request(
        BASE_URL + path,
        data=None if payload is None else json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.load(r)


def wait_for_server(proc, seconds=900):
    deadline = time.time() + seconds
    while time.time() < deadline:
        if proc is not None and proc.poll() is not None:
            sys.exit(f"model server exited early with code {proc.returncode}")
        try:
            http("/models", timeout=5)
            return
        except (urllib.error.URLError, ConnectionError, OSError):
            time.sleep(3)
    sys.exit("model server did not come up in time")


SYSTEM = """You are reverse-engineering a small Microsoft Access sales system whose \
author is gone and which has no documentation. You can read its Visual Basic for \
Applications (VBA) modules, its saved queries, its schema and its data through the \
tools provided. Explore before you write. Do not guess behavior the code does not show."""


def load_prompt(path):
    return pathlib.Path(path).read_text().strip()


def estimate_tokens(text):
    """Rough count for text the server hasn't measured yet: about 3 characters per token for code."""
    return len(text) // 3 + 1


def next_unit():
    """Pop the next unit for the current phase, moving to tests (then finished) when a queue runs dry."""
    if STATE["phase"] == "notes":
        if STATE["queue"]:
            return STATE["queue"].popleft()
        STATE["phase"] = "tests"
        STATE["test_queue"] = collections.deque(
            (u, STATE["info"][u]["parent"], STATE["info"][u]["level"])
            for u in STATE["order"] if STATE["info"][u].get("has_rules"))
    if STATE["phase"] == "tests" and STATE["test_queue"]:
        return STATE["test_queue"].popleft()
    STATE["phase"] = "finished"
    return None


def skip_unit(reason):
    unit, parent, level = STATE["current"]
    info = STATE["info"].setdefault(unit, {"parent": parent, "level": level})
    info["skipped"] = reason


def clean_outputs():
    """A run starts from nothing: drop the previous run's notes and tests."""
    for d in [OUT / "notes", CRATE / "tests"]:
        if d.exists():
            shutil.rmtree(d)


def write_crate_skeleton():
    """The crate the saved test files land in; save_tests writes crate/tests/uNNN_<unit>.rs."""
    (CRATE / "tests").mkdir(parents=True, exist_ok=True)
    if not (CRATE / "Cargo.toml").exists():
        (CRATE / "Cargo.toml").write_text(
            '[package]\nname = "cairn-ai-tests"\nversion = "0.1.0"\nedition = "2021"\n')
        (CRATE / "src").mkdir(exist_ok=True)
        (CRATE / "src" / "lib.rs").write_text("")


def prepare():
    OUT.mkdir(parents=True, exist_ok=True)
    LOGS.mkdir(parents=True, exist_ok=True)
    clean_outputs()
    reset_state()
    load_vba()
    build_units()
    write_crate_skeleton()


def run(args):
    if not args.no_server:
        require_apple_silicon()
    prepare()
    print(f"{len(UNITS)} units: {sum(u['kind'] == 'vba' for u in UNITS.values())} VBA, "
          f"{sum(u['kind'] == 'query' for u in UNITS.values())} queries, "
          f"{sum(u['kind'] == 'table' for u in UNITS.values())} tables")
    prompt = load_prompt(args.prompt)
    room = args.context_budget - args.max_tokens   # what a round may fill; the rest is the reply's
    if room < args.context_budget // 4:
        sys.exit(f"--max-tokens {args.max_tokens} leaves too little of --context-budget {args.context_budget}")
    proc = server_log = None
    if not args.no_server:
        cmd = mlx_command(args) if args.backend == "mlx" else llama_command(args)
        print("+", " ".join(cmd))
        server_log = (LOGS / "server.log").open("w")
        proc = subprocess.Popen(cmd, stdout=server_log, stderr=subprocess.STDOUT)
    transcript = (OUT / "transcript.jsonl").open("w")

    def log(record):
        transcript.write(json.dumps(record) + "\n")
        transcript.flush()

    stats = {"prompt_tokens": 0, "completion_tokens": 0, "max_prompt": 0, "calls": 0,
             "overflows": 0, "turns": 0}
    try:
        wait_for_server(proc)
        models = http("/models")
        model_id = models["data"][0]["id"] if models.get("data") else args.repo
        started = time.time()
        messages = None
        note = ""
        turn = 0
        while turn < args.max_turns:
            if messages is None:                       # start a round
                if not note:
                    STATE["current"] = next_unit()
                    STATE["attempts"] = 0
                    if STATE["current"] is None:
                        break
                STATE["rounds"] += 1
                STATE["saved"] = False
                messages = [{"role": "system", "content": SYSTEM},
                            {"role": "user", "content": prompt + "\n\n" + brief(args, note)}]
                log({"round": STATE["rounds"], "phase": STATE["phase"], "unit": STATE["current"][0],
                     "brief": messages[1]["content"]})
                print(f"round {STATE['rounds']} {STATE['phase']}: {STATE['current'][0]}")
                # Nothing measured yet: estimate the system prompt, the brief and the tool list.
                specs = json.dumps(tool_specs(), separators=(",", ":"))
                known, pending, warned, nudges, note = 0, SYSTEM + messages[1]["content"] + specs, False, 0, ""
            if known + estimate_tokens(pending) > room:
                stats["overflows"] += 1
                STATE["attempts"] += 1
                messages = None
                if STATE["attempts"] >= MAX_ATTEMPTS:
                    skip_unit(f"ran out of context {MAX_ATTEMPTS} times")
                    print(f"  skipped {STATE['current'][0]}: out of context")
                else:
                    note = ("The last attempt at this unit ran out of context before saving. "
                            "Read less and save sooner.")
                continue
            turn += 1
            resp = http("/chat/completions", {
                "model": model_id, "messages": messages, "tools": tool_specs(),
                "temperature": 0, "seed": args.seed, "max_tokens": args.max_tokens})
            usage = resp.get("usage", {})
            stats["prompt_tokens"] += usage.get("prompt_tokens", 0)
            stats["completion_tokens"] += usage.get("completion_tokens", 0)
            stats["max_prompt"] = max(stats["max_prompt"], usage.get("prompt_tokens", 0))
            known = usage.get("prompt_tokens", 0) + usage.get("completion_tokens", 0)
            pending = ""
            msg = resp["choices"][0]["message"]
            messages.append(msg)
            log({"turn": turn, "round": STATE["rounds"], "prompt_tokens": usage.get("prompt_tokens"),
                 "assistant": msg})
            tool_calls = msg.get("tool_calls") or []
            if not tool_calls:
                nudges += 1
                if nudges > MAX_NUDGES:
                    skip_unit(f"{nudges} replies without a tool call")
                    print(f"  skipped {STATE['current'][0]}: no tool calls")
                    messages = None
                    continue
                nudge = ("Use the tools, then save_notes for this unit." if STATE["phase"] == "notes"
                         else "Use the tools, then save_tests for this unit.")
                messages.append({"role": "user", "content": nudge})
                pending += nudge
                continue
            nudges = 0
            for tc in tool_calls:
                fn = tc["function"]
                a = fn["arguments"]
                try:
                    a = json.loads(a) if isinstance(a, str) else a
                    result = call_tool(fn["name"], a or {})
                except json.JSONDecodeError as e:
                    result = f"error: arguments are not valid JSON ({e}); call {fn['name']} again"
                stats["calls"] += 1
                print(f"  [turn {turn}] {fn['name']}({json.dumps(a)[:80]}) -> {len(result)} chars")
                messages.append({"role": "tool", "tool_call_id": tc.get("id", ""), "content": result})
                pending += result
                log({"turn": turn, "tool": fn["name"], "args": a, "result": result})
            if STATE["saved"]:
                messages = None
            elif not warned and known + estimate_tokens(pending) > WARN_AT * room:
                used = known + estimate_tokens(pending)
                save = "save_notes" if STATE["phase"] == "notes" else "save_tests"
                warning = f"Context: about {used} of {room} tokens used. {save} for this unit now."
                messages.append({"role": "user", "content": warning})
                pending += warning
                warned = True
        else:
            print(f"Stopped after {args.max_turns} turns without finishing.")
        stats["turns"] = turn
        elapsed = time.time() - started
        write_walk_log()
        write_metadata(args, model_id, stats, elapsed)
        print(f"Done in {elapsed:.0f}s: phase {STATE['phase']}, {STATE['rounds']} rounds, "
              f"{stats['calls']} tool calls, largest prompt {stats['max_prompt']} tokens. See {OUT}")
    finally:
        transcript.close()
        if proc is not None:
            proc.terminate()
            try:
                proc.wait(30)
            except subprocess.TimeoutExpired:
                proc.kill()


def write_walk_log():
    """The breadth-first order as it happened, plus every unit the walk never reached."""
    rows = ["| # | Level | Unit | Reached from | Rules | Tests | Note |", "|---|---|---|---|---|---|---|"]
    for unit in STATE["order"]:
        i = STATE["info"][unit]
        rows.append(f"| {i['index']} | {i['level']} | {unit} | {i['parent'] or ''} | "
                    f"{'yes' if i.get('has_rules') else ''} | {i.get('tests', '')} | {i.get('skipped', '')} |")
    skipped = [u for u, i in STATE["info"].items() if "index" not in i]
    never = [u for u in UNITS if u not in STATE["info"] and u not in STATE["seen"]]
    queued = [u for u, _, _ in STATE["queue"]]
    text = "# Breadth-first walk\n\n" + "\n".join(rows) + "\n"
    if skipped:
        text += "\n## Skipped\n\n" + "\n".join(f"- {u}: {STATE['info'][u]['skipped']}" for u in skipped) + "\n"
    if queued:
        text += f"\n## Still queued when the run stopped ({len(queued)})\n\n" + ", ".join(queued) + "\n"
    text += f"\n## Never reached ({len(never)} of {len(UNITS)} units)\n\n" + (", ".join(never) or "none") + "\n"
    (OUT / "walk.md").write_text(text)


def write_metadata(args, model_id, stats, elapsed):
    if args.no_server:
        versions = f"(external server at {BASE_URL})"
    elif args.backend == "mlx":
        versions = _run([str(VENV / "bin" / "pip"), "freeze"])
        versions = "\n".join(l for l in versions.splitlines() if re.match(r"(mlx|huggingface)", l, re.I))
    else:
        versions = _run(["llama-server", "--version"])
    chip = _run(["sysctl", "-n", "machdep.cpu.brand_string"]).strip()
    mem = int(_run(["sysctl", "-n", "hw.memsize"]).strip() or 0) // 2**30
    visited = len(STATE["order"]) - (1 if OVERVIEW in STATE["order"] else 0)
    with_rules = sum(1 for i in STATE["info"].values() if i.get("has_rules"))
    test_files = sorted((CRATE / "tests").glob("*.rs"))
    tests = sum(len(re.findall(r"#\[test\]", p.read_text())) for p in test_files)
    (WORK / "run-metadata.md").write_text(f"""# Run metadata

- Date: {datetime.datetime.now().isoformat(timespec='seconds')}
- Backend: {args.backend}
- Model repo: {args.repo}
- Model id reported by server: {model_id}
- Chip / memory: {chip}, {mem} GB
- macOS: {platform.mac_ver()[0]}
- Server flags: {args.server_args or "(none)"}
- Settings: temperature 0, seed {args.seed}, max_tokens {args.max_tokens}, max_turns {args.max_turns}
- Context budget: {args.context_budget} tokens per round; tool results capped at {MAX_TOOL_CHARS} characters
- Largest prompt sent: {stats['max_prompt']} tokens
- Units in the database: {len(UNITS)}; notes saved for {visited}; {with_rules} with rules
- Test files: {len(test_files)}, {tests} tests
- Rounds: {STATE['rounds']} ({stats['overflows']} ran out of context); turns: {stats['turns']}; tool calls: {stats['calls']}
- Tokens: {stats['prompt_tokens']} prompt, {stats['completion_tokens']} completion
- Wall time: {elapsed:.0f}s
- Finished: {STATE['phase'] == 'finished'} (last phase: {STATE['phase']})
- Walk: outputs/walk.md
- Revision hash: fill in from the model page or `{DEFAULT_MLX_REPO}` download log

```
{versions.strip()}
```
""")


def selftest(args):
    """Build the unit graph and exercise every tool and refusal. No model needed."""
    prepare()
    kinds = collections.Counter(u["kind"] for u in UNITS.values())
    print(f"--- units: {len(UNITS)} ({dict(kinds)})")
    sample = "vba:CairnForms.NextNumber"
    print(f"--- {sample}: {UNITS[sample]['label']}; refs: {', '.join(UNITS[sample]['refs'])}")
    print(f"--- query:qryRMARouting refs: {', '.join(UNITS['query:qryRMARouting']['refs'])}")
    biggest = max(UNITS.values(), key=lambda u: len(u["text"]))
    print(f"--- largest unit: {biggest['label']} ({len(biggest['text'])} characters)")
    STATE["current"] = next_unit()
    survey = brief(args)
    print(f"--- survey brief: {len(survey)} characters, about {estimate_tokens(survey)} tokens")
    checks = [
        ("get_saved_query", {"name": "qryQuoteTotals"}),
        ("get_schema", {"table": "RMAs"}),
        ("read_vba", {"module": "CairnForms", "start": 1, "end": 3}),
        ("search_vba", {"pattern": r"Function NextNumber"}),
        ("sql", {"query": "SELECT OrderNumber FROM Orders WHERE OrderID < 3"}),
        ("sql", {"query": "DELETE FROM Orders"}),
        ("read_vba", {"module": "README.md"}),
        ("read_vba", {"module": "CairnHistory"}),
        ("get_saved_query", {"name": "x; rm -rf /"}),
        ("save_tests", {"code": "#[test] fn t() {}"}),
        ("save_notes", {"text": "x", "follow": ["vba:Nope.Nothing"]}),
        ("save_notes", {"text": "x" * (NOTE_CHARS + 1)}),
        ("save_notes", {"text": "Survey: a quoting and order system.", "follow": [sample, "query:qryRMARouting"]}),
        ("save_notes", {"text": "again"}),
    ]
    for name, a in checks:
        shown = {k: (v[:40] + "...") if isinstance(v, str) and len(v) > 40 else v for k, v in a.items()}
        print(f"--- {name} {json.dumps(shown)}\n{call_tool(name, a)[:300]}")
    STATE["current"] = next_unit()
    unit_brief = brief(args)
    print(f"--- next unit {STATE['current'][0]}: brief {len(unit_brief)} characters, "
          f"about {estimate_tokens(unit_brief)} tokens")
    clean_outputs()
    write_crate_skeleton()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["setup", "run", "selftest"])
    ap.add_argument("--backend", choices=["mlx", "llama"], default="mlx")
    ap.add_argument("--repo", default=DEFAULT_MLX_REPO,
                    help="Hugging Face repo (MLX), or GGUF repo[:quant] for llama")
    ap.add_argument("--vlm", action="store_true", help="serve with mlx_vlm instead of mlx_lm")
    ap.add_argument("--server-args", default="",
                    help="extra flags for the MLX server, e.g. memory limits; recorded in run-metadata.md")
    ap.add_argument("--prompt", default=str(HERE / "prompt.md"))
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--context-budget", type=int, default=DEFAULT_BUDGET,
                    help="tokens a round may use, prompt and reply together; the llama.cpp context size")
    ap.add_argument("--max-tokens", type=int, default=4096, help="longest single reply")
    ap.add_argument("--max-turns", type=int, default=3000, help="model requests across the whole run")
    ap.add_argument("--port", type=int, default=8080)
    ap.add_argument("--no-server", action="store_true",
                    help="use a model server already listening on --port instead of starting one")
    ap.add_argument("--workdir", default=str(HERE),
                    help="where outputs/, logs/, crate/ and run-metadata.md go")
    args = ap.parse_args()
    configure(args)
    {"setup": setup, "run": run, "selftest": selftest}[args.command](args)


if __name__ == "__main__":
    main()
