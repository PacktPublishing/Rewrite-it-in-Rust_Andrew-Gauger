# Chapter 5: Painting the Heatmap Onto a Moving Map

| Folder | What it is |
|---|---|
| `viz-core/` | `viz-core` with the RGBA palette (`palette.rs`) and canvas painting through `web-sys`. The same change is [viz-core pull request #3](https://github.com/AndyGauge/viz-core/pull/3) |
| `website-heatmap/` | The finished site: heatmap and labels painted from Rust, with its own copy of the crate in `rust/viz-core/` |
| `heatmap-bench/` | The paint pipeline benchmarked against a hand-written JavaScript baseline, with its captured `bench-output.json` |
| `float-formatting-size-measurement/` | What `format!` on floats costs in Wasm binary size |

```sh
cd viz-core && cargo test
```

Build the Wasm module and run the site the same way as in [Chapter 3](../chapter3/README.md). See [`heatmap-bench/README.md`](heatmap-bench/README.md) for the benchmark.
