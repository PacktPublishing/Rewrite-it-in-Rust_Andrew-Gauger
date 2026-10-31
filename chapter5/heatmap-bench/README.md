# heatmap-bench

Chapter 5's answer to `chapter3/wasm-bench`: does moving the *paint* step
into Rust actually pay, the way Chapter 3 asked whether moving the *filter*
step did?

It reuses Chapter 3's method for the "isolated call cost" pass verbatim:
run everything inside a real Chromium page under Chrome DevTools Protocol
CPU throttling, warm the JIT and the wasm call path, then time the real
`wasm-bindgen` call against a hand-written JS implementation of the exact
same algorithm. Same fairness bar (`Date.parse` and sensor-index lookup
done once, up front, on both sides). No DOM framework, no Leaflet.

## What it measures

| op | wasm | JS baseline | what it isolates |
|----|------|-------------|------------------|
| `range` | `PointCloud.range` | `jsRange` | carried over from Chapter 3, unchanged |
| `snapshot_at` | `PointCloud.snapshot_at` | `jsSnapshotAt` | carried over from Chapter 3, unchanged |
| `density_grid` | `PointCloud.density_grid` | `jsDensityGrid` | KDE compute **plus** marshalling the whole grid back as a `Float64Array` |
| `paint_heatmap` | `PointCloud.paint_heatmap` | `jsPaintHeatmap` | the whole Chapter 5 pipeline: filter, project, KDE splat, palette LUT, `ImageData`, `put_image_data` — grid never crosses the boundary |
| `paint_heatmap_labels` | `PointCloud.paint_heatmap_labels` | *(none)* | the `format!`/`core::fmt` float-to-text layer |

`jsPaintHeatmap` is the "before" Chapter 5 says nobody wrote: the same
unnormalized-Gaussian splat, the same 256-entry palette lookup table built
once, the same four-bytes-per-cell fill, the same single `putImageData`.
Every constant (`VIBRATION_SATURATION_G`, the palette stops, `MIN/MAX_SIGMA_PX`,
the Web Mercator math) is copied straight out of `density.rs` / `palette.rs`
/ `mercator.rs`. The bench asserts `gridCellsMatch` each run — the JS port
and the Rust must agree on grid dimensions or the comparison is void.

## How this differs from `chapter3/wasm-bench`

Chapter 3's bench had a second pass ("on-page cost") that drove the real
Rails app and read its `#perf-readout` after real Leaflet marker churn.
This bench has **only** the isolated pass:

- The Chapter 5 work is a paint pipeline, not a DOM-marker pipeline. There
  is no `legacy_engine.js` heatmap to be an on-page "before" — the legacy
  engine has no heatmap mode at all. The meaningful measurement is the
  per-call cost of the paint, which the isolated pass gives more precisely
  than scraping a readout string.
- It serves a bare `harness.html` from a throwaway static server instead of
  booting Rails. The data is still the real thing: `gen-readings.rb` runs
  the app's actual `SensorDataGenerator` (with three one-line ActiveSupport
  shims) so `fixtures/*.json` is byte-for-byte what `GET /readings.json`
  returns.
- It runs both scenarios (`cluster` and `highway`); Chapter 3 only ran
  `cluster`.

Viewport per scenario: the data's own bounding box, padded 8%, with the
zoom snapped to the integer that fits it to ~1024 projected px. That
decides grid size — `cluster` is tight and tall (~198x202 cells at zoom
17), `highway` is a wide, short route (~215x30 cells at zoom 8).

## Running it

```sh
./run.sh           # full: throttles 1x / 4x / 6x, both scenarios
./run.sh --quick   # smoke test: 4x only, fewer iterations
```

`run.sh` regenerates the fixtures, rebuilds the wasm from the current
`../viz-core` source (`wasm-pack build --no-opt`, same flags as Chapter 3),
`npm install`s Playwright once (browser download skipped — it drives the
system Google Chrome via `channel: "chrome"`), and writes
`bench-output.json`. Node 20+; Ruby 3.x for the generator.

`bench-output.json` is the committed output of the run of record.

## The run of record

2026-08-30, Intel Core i5 (x86_64 darwin), Node v23.4.0, system Chrome
151.0.7922.175, `wasm-pack build --no-opt`. `ms/call`, warmed up, 2,000
iterations for `range`/`snapshot_at`, 300 for the heatmap ops.

```
cluster, 198x202 grid (39,996 cells), 20 active sensors
                   wasm      js       wasm/js
  range          0.82 ms   0.93 ms    0.88x     tie (Chapter 3's finding)
  snapshot_at    0.020 ms  0.031 ms   0.65x
  density_grid   1.53 ms   1.03 ms    1.49x     JS wins: grid marshals back
  paint_heatmap  1.90 ms   3.53 ms    0.54x     Rust wins: grid never crosses
  paint_labels   0.84 ms     -          -
  (4x CPU throttle)

highway, 215x30 grid (6,450 cells), 20 active sensors
                   wasm      js       wasm/js
  range          0.73 ms   0.91 ms    0.80x
  snapshot_at    0.020 ms  0.023 ms   0.86x
  density_grid   2.99 ms   3.04 ms    0.99x     tie
  paint_heatmap  2.81 ms   3.74 ms    0.75x     Rust wins
  paint_labels   0.80 ms     -          -
  (4x CPU throttle)
```

The pattern holds at 1x and 6x too (see `bench-output.json`): the boundary,
not the math, decides the winner. `density_grid` returns the whole grid as
a `Float64Array` and JS matches or beats it — the same result Chapter 3 got
for `range`. `paint_heatmap` runs the identical KDE and then returns
*nothing* (it paints from Rust, one `put_image_data`), and Rust pulls
ahead by 1.3x–1.9x. "No JavaScript touches a pixel" isn't only an
architecture note; deleting the grid-sized return payload is what makes the
Rust path the faster one.

`paint_heatmap_labels` has no JS baseline on purpose — it exists to price
the `core::fmt` float-formatting the chapter flags, not to race anything.
