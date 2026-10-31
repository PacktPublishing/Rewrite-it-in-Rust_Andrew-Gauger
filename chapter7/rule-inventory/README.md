# Rule Inventory

**`rules.md` is the ground-truth rule list** (52 rules: 46 in the code, 6
only people hold, plus 10 data defects), each with its status and the AI
run's coverage. It's H5's count. `inventory.md` below is the raw listing of
every query and routine, generator and form builders included, and isn't a
rule count. `commission_rounding.py` and `data_checks.py` reproduce the
numbers `rules.md` cites.

## The inventory script

This script scans the Cairn Access system in the `part 2/` directory and extracts:

* Saved queries, with their SQL, read from `CairnSystems_History.accdb` using
  `mdb-queries` (part of [mdbtools](https://github.com/mdbtools/mdbtools)).
  The line in the build module where each query is defined is recorded when found.
* VBA routine names (`Sub` / `Function`) from the exported `.bas` modules.

It writes a markdown table `inventory.md` with columns **Rule**, **Kind**,
**Source**, and **Evidence** (the query SQL, or the nearest preceding comment
for a routine).

## Requirements

* Python 3
* mdbtools, which provides `mdb-queries` (`brew install mdbtools`)

The script exits with an error if `mdb-queries` is not on the `PATH`.

## Usage
```sh
cd chapter7/rule-inventory
python3 inventory.py
```
The script produces `inventory.md` in the same folder.

## Limits

Queries embedded in form Row Source properties live inside the form objects,
which mdbtools cannot read, so they are not listed. The script surfaces
identifiers and SQL. It does not interpret VBA logic.
