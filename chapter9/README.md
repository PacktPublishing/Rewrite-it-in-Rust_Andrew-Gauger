# Chapter 9: Building the Replacement: Strategy, Identity, and Schema

The workspace this chapter builds is in [`crate/`](crate/), as it stood at
the end of the chapter: `cairn-core` from Chapter 8, plus `migration`, the
Postgres schema, and `cairn-auth`, sign-in through Entra ID. Later chapters
grow the workspace in [`../part 2/crate/`](../part%202/crate/); this copy
stays as Chapter 9 printed it.

| Folder | What it is |
|---|---|
| `crate/` | The Cairn V2 workspace as of this chapter. Its README says how to run it |
| `cairn-auth/` | Sets up a test Microsoft Entra tenant for the sign-in service, with the Azure CLI |
| `outputs/` | Captured output quoted in the chapter. Its README says how each was made |
| `listings/` | `capture-errors.sh`, which reproduces the compiler error, and `schema-figure.sh`, which draws Figure 9.3 |

The chapter needs PostgreSQL, and setting up the tenant needs the Azure CLI:

```sh
brew install postgresql@17     # captured output used 17.11
brew install azure-cli
```

Then follow [`cairn-auth/README.md`](cairn-auth/README.md) for the tenant,
and run the workspace:

```sh
cd crate
sh dev-db.sh
export DATABASE_URL=postgres://cairn_owner:cairn_owner_dev@127.0.0.1:5433/cairn
cargo run -p migration -- up
sh dev-seed.sh yournamegmail.onmicrosoft.com
cargo test --workspace
```

The schema diagrams need `tbls` and Graphviz (`brew install tbls graphviz`).
