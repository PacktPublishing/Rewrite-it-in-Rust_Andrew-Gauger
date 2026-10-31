#!/usr/bin/env python3
"""Read the VBA modules out of an Access database file, with no Access and no Windows.

Access keeps its VBA project inside the .accdb, in the hidden system table
MSysAccessStorage: a tree of named streams (VBA/VBAProject/PROJECT,
VBA/VBAProject/VBA/dir, one compressed stream per module). mdb-export reads
the table; oletools decodes the streams (MS-OVBA compression and the dir
records that name each module).

    python access_vba.py <database.accdb> <out-dir>   # writes <Module>.bas per module

Needs mdbtools on PATH and oletools in the running Python.
"""

import csv
import io
import pathlib
import subprocess
import sys

from oletools import olevba

ROOT = "VBA/VBAProject/"

# Edits made to the module source in `part 2/` that the .accdb hasn't picked up
# yet: rebuilding the file needs Access on Windows. Each is applied to the
# decoded text so the model reads what the source now says. Delete an entry
# once the database is regenerated with it. {module: [(old, new), ...]}
PENDING_SOURCE_FIXES = {
    "CairnForms": [(
        "'  Sign-in. Passwords are stored and compared as plain text, and the SQL is\n"
        "'  built by pasting in what was typed. That is how it was written; it is kept\n"
        "'  that way on purpose so the replacement has something real to retire.\n",
        "'  Sign-in. Checks the name and password against the Users table and keeps\n"
        "'  who signed in (and whether they're an admin) in TempVars for the forms.\n"
        "'  It only lives on the S: drive, so nothing fancy.\n",
    )],
}


def storage_streams(db):
    """Every stream in MSysAccessStorage, keyed by its path in the tree."""
    out = subprocess.run(["mdb-export", "-b", "hex", str(db), "MSysAccessStorage"],
                         capture_output=True, text=True, check=True).stdout
    csv.field_size_limit(sys.maxsize)
    rows = list(csv.DictReader(io.StringIO(out)))
    by_id = {r["Id"]: r for r in rows}

    def path(r):
        parts = []
        while r["Name"] != "MSysAccessStorage_ROOT":
            parts.append(r["Name"])
            r = by_id[r["ParentId"]]
        return "/".join(reversed(parts))

    return {path(r): bytes.fromhex(r["Lv"]) for r in rows if r["Type"] == "2"}


class StorageAsOle:
    """The slice of olefile's interface that olevba.VBA_Project uses."""

    def __init__(self, streams):
        self.streams = streams

    def _key(self, p):
        return (p if isinstance(p, str) else "/".join(p)).replace("//", "/")

    def openstream(self, p):
        return io.BytesIO(self.streams[self._key(p)])

    def exists(self, p):
        return self._key(p) in self.streams

    def get_type(self, p):
        return 2


def modules(db):
    """{module name: source text} for every module in the database's VBA project."""
    project = olevba.VBA_Project(StorageAsOle(storage_streams(db)), ROOT,
                                 ROOT + "PROJECT", ROOT + "VBA/dir", relaxed=True)
    project.parse_project_stream()
    found = {}
    for _, filename, code in project.parse_modules():
        text = code.decode("latin-1") if isinstance(code, bytes) else code
        found[pathlib.Path(filename).stem] = text.replace("\r\n", "\n")
    for name, fixes in PENDING_SOURCE_FIXES.items():
        for old, new in fixes:
            if old in found.get(name, ""):
                found[name] = found[name].replace(old, new)
            elif new not in found.get(name, ""):
                sys.exit(f"{name}: pending source fix matches neither the old nor the new text")
    return found


if __name__ == "__main__":
    db, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    for name, text in modules(db).items():
        (out / f"{name}.bas").write_text(text)
        print(f"{name}: {len(text.splitlines())} lines")
