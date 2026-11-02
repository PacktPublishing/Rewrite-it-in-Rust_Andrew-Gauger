# riir-wasm-battery-pack

Part 1 of the [riir battery pack](../README.md) -- compiling a Rust core to
WebAssembly with `wasm-bindgen`, the crate curation behind Chapter 3 of
*Rewrite It in Rust*, Part I: The Dashboard (companion repo:
[AndyGauge/viz-core](https://github.com/AndyGauge/viz-core)).

## Quick Start

```sh
cargo bp new wasm-core my-viz-core
```

Add it to an existing crate instead:

```sh
cargo bp add riir-wasm
```

Want DOM/canvas bindings for painting a density grid onto the map, or
readable panic messages in the browser devtools console instead of an
opaque WASM trap?

```sh
cargo bp add riir-wasm -F dom,panic-hook
```

## Why these crates, and not others

`wasm-bindgen` and `js-sys` are the two required crates: the proc-macro
bridge and the typed-array/JS-builtin bindings a bulk-load constructor
needs to accept a `Float64Array` in one copy, the pattern this pack's
`wasm-core` template scaffolds and `viz-core/src/lib.rs`'s `PointCloud::new`
uses for real. See the Cargo.toml walkthrough in `riir-battery-pack.md` at
the repo root for the reasoning behind the exact-pinned `wasm-bindgen`
version and the `opt-level = "s"` / `lto = true` release profile.

Deliberately **not** included: `serde_wasm_bindgen`. Chapter 3's own
research (`RESEARCH-v2.md`) found it measurably slower than
`wasm-bindgen`'s structural getter access for this book's access pattern --
recommending it here would work against the chapter this pack exists to
support.

`web-sys` and `console_error_panic_hook` are opt-in (`dom`, `panic-hook`)
rather than defaults: neither is needed for Chapter 3's boundary-crossing
argument, but `web-sys` is the natural next dependency once Chapter 5
starts painting to a `<canvas>`, and `console_error_panic_hook` is
standard practice for any WASM module while it's still being debugged.

## License

Licensed under either of:

- [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
- [MIT license](https://opensource.org/license/mit)

at your option.
