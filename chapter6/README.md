# Chapter 6: The File Nobody Could Touch

`export-demo/` runs the naive "just export it to a real database" path for real: it copies the Access history database into a throwaway PostgreSQL cluster and runs the chapter's queries against it.

```sh
export-demo/run.sh                      # uses ../part 2/CairnSystems_History.accdb
export-demo/run.sh path/to/other.accdb
python3 export-demo/rounding.py
```

It needs `mdbtools` and a local PostgreSQL install (`initdb`, `pg_ctl` and `psql` on your `PATH`). The cluster starts in a temporary directory on port 55432 and is removed at the end. `output.txt` and `rounding-output.txt` are the captured runs.
