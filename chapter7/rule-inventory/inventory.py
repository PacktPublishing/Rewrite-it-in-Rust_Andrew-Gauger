#!/usr/bin/env python3
"""Rule inventory generator for Chapter 7.

Surfaces the places business rules live in the Cairn Access system:

* Saved queries: listed and exported from the .accdb with ``mdb-queries``,
  so the SQL (the rule itself) is the evidence. The line where the build
  module defines each query (``Q "qryName", s``) is recorded when found.
* VBA routines: every ``Sub`` / ``Function`` in the exported ``.bas`` modules,
  with the comment on the line above as its description.

It writes ``inventory.md`` with columns Rule, Kind, Source, Evidence. It does
not interpret the logic. Queries embedded in form Row Source properties are
not visible to mdbtools and are not listed.
"""

import pathlib
import re
import shutil
import subprocess
import sys
from typing import List, Tuple

BASE_DIR = pathlib.Path(__file__).resolve().parent.parent.parent  # repo root
PART2_DIR = BASE_DIR / "part 2"
BAS_FILES = sorted(PART2_DIR.glob("*.bas"))
ACCDB = PART2_DIR / "CairnSystems_History.accdb"
OUTPUT_MD = pathlib.Path(__file__).with_name("inventory.md")

ROUTINE_PAT = re.compile(
    r"^\s*(?:Public\s+|Private\s+)?(?:Sub|Function)\s+([A-Za-z0-9_]+)", re.IGNORECASE
)
QUERY_DEF_PAT = re.compile(r'\bQ\s+"(qry[A-Za-z0-9_]+)"')

Row = Tuple[str, str, str, str]  # rule, kind, source, evidence


def preceding_comment(lines: List[str], idx: int) -> str:
    """Comment on the line above ``lines[idx]`` (0-based), or empty."""
    if idx == 0:
        return ""
    prev = lines[idx - 1].strip()
    if prev.startswith("'"):
        return prev.lstrip("'").strip()
    if prev.lower().startswith("rem "):
        return prev[4:].strip()
    return ""


def scan_bas(path: pathlib.Path) -> Tuple[List[Row], dict]:
    """Return routine rows and a {query name: "file:line"} map."""
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    rows: List[Row] = []
    query_defs = {}
    for i, line in enumerate(lines):
        m = ROUTINE_PAT.match(line)
        if m:
            rows.append((m.group(1), "VBA routine", f"{path.name}:{i + 1}",
                         preceding_comment(lines, i)))
        m = QUERY_DEF_PAT.search(line)
        if m:
            query_defs.setdefault(m.group(1), f"{path.name}:{i + 1}")
    return rows, query_defs


def mdb(*args: str) -> str:
    return subprocess.run(["mdb-queries", *args], capture_output=True, text=True,
                          check=True).stdout


def saved_queries(query_defs: dict) -> List[Row]:
    if not shutil.which("mdb-queries"):
        sys.exit("mdb-queries not found; install mdbtools (brew install mdbtools)")
    names = [n for n in mdb("-1", str(ACCDB)).splitlines() if n.strip()]
    rows = []
    for name in sorted(names):
        sql = " ".join(mdb(str(ACCDB), name).split())
        where = f"{ACCDB.name}" + (f" ({query_defs[name]})" if name in query_defs else "")
        rows.append((name, "Saved query", where, sql))
    return rows


def cell(text: str) -> str:
    return text.replace("|", "\\|")


def main() -> None:
    routines: List[Row] = []
    query_defs: dict = {}
    for bas in BAS_FILES:
        r, q = scan_bas(bas)
        routines.extend(r)
        for k, v in q.items():
            query_defs.setdefault(k, v)
    rows = saved_queries(query_defs) + routines

    seen = set()
    unique = []
    for row in rows:
        key = (row[0], row[2])
        if key not in seen:
            seen.add(key)
            unique.append(row)

    with OUTPUT_MD.open("w", encoding="utf-8") as f:
        f.write("| Rule | Kind | Source | Evidence |\n")
        f.write("|------|------|--------|----------|\n")
        for rule, kind, src, ev in unique:
            f.write(f"| {cell(rule)} | {kind} | {cell(src)} | {cell(ev)} |\n")
    nq = sum(1 for r in unique if r[1] == "Saved query")
    print(f"Wrote {len(unique)} rows ({nq} saved queries, {len(unique) - nq} VBA routines) to {OUTPUT_MD}")


if __name__ == "__main__":
    main()
