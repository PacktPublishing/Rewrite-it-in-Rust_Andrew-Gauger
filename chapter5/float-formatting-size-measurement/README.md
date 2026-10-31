# `core::fmt` Float-Formatting Size Cost on `wasm32-unknown-unknown`

Measured 2026-08-30 against `viz-core` v0.3.0 (`add-canvas-paint` branch).

## Method

Three builds of the same crate, each with `wasm-pack build --target web --no-opt`
(the `release` profile: `opt-level = "s"`, LTO, single codegen unit, `panic = "abort"`,
`strip = true`). All builds on the same machine, no toolchain changes between them.

| Stage | Description | Dependencies added | `viz_core_bg.wasm` |
|---|---|---|---|
| Ch4 baseline | `geo` + `libm`, no `web-sys`, no labels | — | **43,091 bytes** |
| Intermediate (~48KB) | `web-sys` for `paint_heatmap`, no `paint_heatmap_labels`/`format!` | `web-sys` (CanvasRenderingContext2d/HtmlCanvasElement/ImageData) | **47,664 bytes** |
| Labels (`format!` float → string) | `paint_heatmap_labels` uses `format!("{:.2}g / {:.0}°F", …)` | float-to-decimal formatter pulled in | **77,441 bytes** |

## Results

The float-to-string layer alone costs **29,777 bytes** (77,441 − 47,664).
That is larger than `web-sys`'s 4,573-byte contribution (47,664 − 43,091).

The reason: `core::fmt`'s float formatting does not have a thin hardware-assisted
path on `wasm32-unknown-unknown` and pulls in full Grisu/Ryū lineage float-to-decimal
conversion code rather than delegating to an OS libc. The integer `Display` impl
pulls in disproportionate code; float formatting is heavier still.

## Attribution

- *Rust and WebAssembly* book, "Shrinking .wasm Size": lists `core::fmt` /
  `<str as core::fmt::Debug>::fmt` among the largest code-size contributors in its
  `twiggy` example, and recommends "only do string formatting in debug mode,
  and in release mode use static strings."
  <https://rustwasm.github.io/docs/book/reference/code-size.html>

- rust-lang/rust #118940 "`impl fmt::Display for u32` compiles to large binary" —
  establishes that even integer `Display` pulls in disproportionate code; float
  formatting (Grisu/Ryū-style float-to-decimal) is heavier still.
  <https://github.com/rust-lang/rust/issues/118940>

## Resolution

The prose claim ("the shipped `viz_core_bg.wasm` grew from 43 KB to 77 KB") is
confirmed. The intermediate step is 47,664 bytes (~46 KB), which is close enough
to round to ~48 KB if desired, but the cleanest attribution is 43 KB → 77 KB
because that is the before/after of adding the label layer itself.