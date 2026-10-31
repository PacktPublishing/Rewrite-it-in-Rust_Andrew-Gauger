# Chapter 3: Mounting a Rust Core Compiled to WebAssembly

| Folder | What it is |
|---|---|
| `cairn-systems/website/` | The Rails site as Cairn inherited it, filtering readings in JavaScript |
| `cairn-systems/website-wasm/` | The same site with `viz-core` mounted behind a feature flag. The crate is in `rust/viz-core/` and its build output in `public/wasm/` |
| `viz-core/` | The `viz-core` crate as its own project |
| `wasm-bench/` | The Playwright benchmark behind the chapter's timings, with its captured `bench-output.json` |

## Build the Wasm module

```sh
cd cairn-systems/website-wasm/rust/viz-core
wasm-pack build --target web --out-dir pkg --out-name viz_core --no-opt
cp pkg/viz_core.js pkg/viz_core_bg.wasm ../../public/wasm/
```

## Run the site

Both sites are Rails 8 apps on Ruby 3.4.1 with no database.

```sh
cd cairn-systems/website-wasm
bundle install
bin/rails server
```

## Run the tests and the benchmark

```sh
cd viz-core && cargo test
```

See [`wasm-bench/README.md`](wasm-bench/README.md) for the benchmark.
