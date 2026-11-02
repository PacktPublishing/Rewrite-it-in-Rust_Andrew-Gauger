# Chapter 10 outputs

Captured from the workspace in `../crate` on 2026-10-31 (Apple M3 Pro, macOS 15.7.9),
against the placeholder rows `dev-ipsum.sh` loads (13,531 orders). None
of the rows are Cairn's data.

| File | How it was made |
|---|---|
| `cargo-expand-company.txt` | 2026-11-01, `cargo expand -p cairn-app --lib forms::company` (cargo-expand 1.0.127): the `Form` impl `#[derive(Form)]` writes for `CompanyDraft`, verbatim; the derived `Debug`, `Clone` and `PartialEq` impls are left out |
| `keyboard-tests.txt` | 2026-11-01, `cargo test -p cairn-app --test keyboard`, the build lines removed. The tests need no database |
| `cargo-bp-new.txt` | 2026-11-01, `cargo-bp` 0.6.1, `riir-gui-battery-pack` 0.1.0 from crates.io, in an empty folder, non-interactive (`-N`, both template questions answered no). The command prints nothing when it succeeds; the file shows the command and the `Cargo.toml` it wrote |
| `scroll-timing.txt` | `cargo test --release -p cairn-app --test scroll_timing -- --ignored --nocapture`, 2026-11-01. Time to build each frame (`CairnApp::show`) while the mouse wheel flings Sales Orders in datasheet view (seven columns) from the top to the oldest order, 600 points a frame; the test fails if the oldest order isn't on screen at the end. Drawing on the GPU isn't counted. |

**A run worth keeping for the third section (2026-11-01).** When the
datasheet view first rebuilt its 13,531 row structs every frame, the same
test printed `13531 orders, 520 frames: median 3.377 ms, 99th percentile
3.798 ms, max 5.078 ms`. Building the rows once, when the view opens,
brought it back to the numbers in `scroll-timing.txt`. `TableBuilder`
draws only the rows in view; the app still has to not build the rest.
(The first measurement, 2026-10-31, was 0.62 ms median with six
columns.)

The screenshots in `../crate/cairn-app/tests/snapshots/` are `egui_kittest` snapshots of
every form in the light theme, which prints (2026-11-01, after a fresh
`dev-ipsum.sh`, with `form_style` giving every field an outline):
`UPDATE_SNAPSHOTS=1 cargo test -p cairn-app --test screens -- --ignored`,
each at 1180 × 760 (desktop) and 1024 × 768 (an iPad in landscape).
Reload the placeholder rows first: `cairn-auth`'s Postgres tests add rows
("Test Co") to the dev database.
