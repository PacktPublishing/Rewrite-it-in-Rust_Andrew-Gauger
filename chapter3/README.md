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

## Measure on a real device

Bind the server to your network so a phone or tablet on the same Wi-Fi can
reach it:

```sh
bin/rails server -b 0.0.0.0 -p 3737
```

Then open `http://<your computer's IP>:3737/?engine=wasm` and
`?engine=legacy` on the device. Both engines show the same three readouts:

- the existing per-update timing,
- a frames-per-second line (`paint fps`, `playback updates/s`,
  `worst frame`),
- under **Time playback**, a **Run full speed** button that steps through
  every reading timestamp in order, one per animation frame, and reports the
  wall time for the whole playback plus the total render work.

Use **Run full speed** to compare engines. The ordinary **Play** button
ticks at different rates in the two engines (legacy every 150 ms, one
timestamp per tick; Wasm every 100 ms, two minutes per tick), so its speed
is not a fair comparison.

## Run the tests and the benchmark

```sh
cd viz-core && cargo test
```

See [`wasm-bench/README.md`](wasm-bench/README.md) for the benchmark.
