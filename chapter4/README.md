# Chapter 4: From Bounding Box to Density Grid

| Folder | What it is |
|---|---|
| `viz-core/` | `viz-core` with Web Mercator projection (`mercator.rs`) and kernel density estimation (`density.rs`). The same change is [viz-core pull request #2](https://github.com/AndyGauge/viz-core/pull/2) |
| `cairn-systems/website/` | The inherited site, unchanged from Chapter 3 |
| `cairn-systems/website-wasm/` | Chapter 3's Wasm site |
| `cairn-systems/website-heatmap/` | The site wired for the heatmap overlay, with its own copy of the crate in `rust/viz-core/` |

```sh
cd viz-core
cargo test        # includes the mercator:: and density:: tests the chapter prints
```

Build the Wasm module and run a site the same way as in [Chapter 3](../chapter3/README.md). Both sites carry the same device-measurement readouts and **Run full speed** button; see [Measure on a real device](../chapter3/README.md#measure-on-a-real-device). In `website-heatmap/`, Play and **Run full speed** also drive the Heatmap view, one sample per repaint.
