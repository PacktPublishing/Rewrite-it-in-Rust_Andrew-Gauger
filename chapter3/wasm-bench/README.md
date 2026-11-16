# wasm-bench

Source for the numbers in `chapter3/4-shipping-and-measuring.md`: a
Playwright script that drives the real `cairn-systems/website-wasm` app
(both `legacy_engine.js` and `wasm_engine.js`) inside an actual Chromium
page, under Chrome DevTools Protocol CPU throttling, and measures two
things:

1. **Isolated call cost** — `page.evaluate()` fetches `/readings.json`,
   builds the real flat `Float64Array`, constructs the real `PointCloud`
   via the real `/wasm/viz_core.js`, and times 2,000 warmed-up calls to
   `range`/`snapshot_at`, against a hand-written JS equivalent of the same
   two algorithms (bitmask + timestamp range + bbox filter; per-sensor
   binary search + lerp). Both sides parse each ISO timestamp and resolve
   each sensor index exactly once, up front — the JS-side equivalent of
   `PointCloud::new`'s one bulk copy — so the comparison isolates
   runtime/dispatch/boundary cost, not `Date.parse` or `Map.get` cost
   inside the hot loop. No DOM involved.
2. **On-page cost** — real navigation to `?engine=legacy` and
   `?engine=wasm`, reading the app's own already-instrumented
   `#perf-readout` text after the initial "show all" render and across a
   sweep of the time slider in playback mode. Includes real Leaflet marker
   DOM churn, i.e. what a customer's browser actually pays end to end.

Run 2026-08-22 on an Intel Core i5-1038NG7 (8 cores), macOS 14.6.1, Node
v23.4.0, Playwright's bundled Chromium (chrome-headless-shell 151.0.7922.34),
against the Rails app running locally with `bin/rails server`. CPU
throttling stands in for a physical budget Android device, per Chrome
DevTools' own `Emulation.setCPUThrottlingRate` — no such device was
available for this run.

## Running it

```sh
# from cairn-systems/website-wasm/
bin/rails server -p 3737 -d

# from this directory
npm install playwright
npx playwright install chromium
node bench.mjs
```

`bench-output.json` is the verbatim output of the run the chapter's numbers
come from. `bench.mjs` prints the same shape fresh each time; run-to-run
noise on a shared dev machine (not an isolated benchmark rig) should be
expected, especially under throttling.

## On a real device: `device-bench.mjs`

CPU throttling scales a desktop processor; it does not reproduce a
phone's. `device-bench.mjs` drives the real app in Chrome on a real
Android device over USB, through each scenario (the agriculture cluster,
and the container fleet at the default zoom and zoomed in two levels),
both engines, three fresh page loads each. It records the page's own
readouts: Play's frames per second and worst frame, and the **Run full
speed** wall time, p99 and worst frame time, and average render work. It
prints one table per scenario (median of the attempts) and writes every
sample and every frame to `results-<label>-<timestamp>.json`.

An Android emulator or an Android Docker image cannot stand in for this:
the emulator runs on the host's processor, so its numbers describe the
host.

One-time setup:

1. On the device, open **Settings → About tablet → Software information**
   and tap **Build number** seven times. In **Settings → Developer
   options**, turn on **USB debugging**.
2. In the device's Chrome, open `chrome://flags`, enable **Enable command
   line on non-rooted devices**, and relaunch Chrome.
3. On the computer: `brew install android-platform-tools`. Connect the
   USB cable, accept the **Allow USB debugging?** prompt on the device,
   and check that `adb devices` lists it.

Then:

```sh
# from cairn-systems/website-wasm/
bin/rails server -p 3737

# from this directory
npm install playwright
node device-bench.mjs --android --label sm-t290
```

The script runs `adb reverse`, so the device reaches the computer's server
as `localhost` over the cable; no Wi-Fi needed. Leave the device plugged
in, other apps closed, and screen on. Without `--android` it drives desktop
Chrome instead (`--throttle 4` for 4x), which is useful for trying the
script out.
