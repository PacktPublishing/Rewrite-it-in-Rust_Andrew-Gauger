// Real before/after benchmark for chapter3 Heading 4, run against the live
// Rails app at cairn-systems/website-wasm (must already be running on
// http://127.0.0.1:3737). Two measurements, both executed inside an actual
// Chromium page (not Node) so CPU throttling via CDP applies to both sides
// equally:
//
//   A) Isolated call cost: N calls to the real wasm-bindgen PointCloud.range
//      / .snapshot_at, vs. an equivalent hand-written JS implementation of
//      the same algorithm (bitmask+timerange+bbox filter; per-sensor binary
//      search + lerp for snapshot_at) run over the same data as plain JS
//      objects. No DOM, no Leaflet -- isolates the boundary/runtime cost
//      from marker rendering.
//   B) Full on-page cost: the app's own already-instrumented #perf-readout
//      text, read after real interactions against the real legacy_engine.js
//      and wasm_engine.js pages -- includes Leaflet marker DOM churn, i.e.
//      what a customer's phone actually pays.
//
// Usage: node bench.mjs
import { chromium } from "playwright";

const BASE = "http://127.0.0.1:3737";
const SCENARIO = "cluster"; // 20 sensors x 360 readings/sensor = 7,200 readings
const THROTTLES = [1, 4, 6]; // CPU slowdown multipliers (1x = unthrottled)
const N_CALLS = 2000;

async function isolatedBenchmark(page, throttle) {
  return await page.evaluate(
    async ({ scenario, nCalls }) => {
      const res = await fetch(`/readings.json?scenario=${scenario}`);
      const readings = await res.json();

      const sensorIds = Array.from(new Set(readings.map((r) => r.sensor_id))).sort();
      const idxOf = new Map(sensorIds.map((id, i) => [id, i]));
      let minTs = Infinity, maxTs = -Infinity;
      const flat = new Float64Array(readings.length * 6);
      let cursor = 0;
      for (const r of readings) {
        const ts = Date.parse(r.recorded_at);
        if (ts < minTs) minTs = ts;
        if (ts > maxTs) maxTs = ts;
        flat[cursor++] = idxOf.get(r.sensor_id);
        flat[cursor++] = r.lat;
        flat[cursor++] = r.lng;
        flat[cursor++] = r.temperature_f;
        flat[cursor++] = r.vibration_g;
        flat[cursor++] = ts;
      }

      const wasmModule = await import("/wasm/viz_core.js");
      await wasmModule.default();
      const cloud = new wasmModule.PointCloud(flat, sensorIds.length);

      // Plain-JS equivalent of the same two algorithms, over the same
      // readings as ordinary JS objects: the "before" baseline. Per-sensor
      // sorted index built once, same as viz-core's `by_sensor`, so this
      // isolates runtime/dispatch cost, not algorithmic complexity. Same
      // fairness bar as `flat`/`data.push` above: parse each ISO timestamp
      // and resolve each sensor index ONCE, at load, into a flat array of
      // plain objects with scalar fields -- the JS-side equivalent of one
      // bulk copy into resident memory. A JS baseline that re-parsed
      // `recorded_at` inside the hot filter loop would be measuring
      // `Date.parse` cost, not runtime/dispatch cost, and would overstate
      // the WASM side's advantage.
      const rows = readings.map((r) => ({
        idx: idxOf.get(r.sensor_id),
        lat: r.lat,
        lng: r.lng,
        temp_f: r.temperature_f,
        vibration_g: r.vibration_g,
        ts: Date.parse(r.recorded_at),
      }));
      const bySensor = sensorIds.map((_, s) =>
        rows.filter((r) => r.idx === s).sort((a, b) => a.ts - b.ts)
      );

      function jsRange(mask, startTs, endTs, minLat, minLng, maxLat, maxLng) {
        const out = [];
        for (const r of rows) {
          if ((mask & (1 << r.idx)) === 0) continue;
          if (r.ts < startTs || r.ts > endTs) continue;
          if (r.lat < minLat || r.lat > maxLat || r.lng < minLng || r.lng > maxLng) continue;
          out.push([r.idx, r.lat, r.lng, r.temp_f, r.vibration_g, r.ts]);
        }
        return out;
      }

      function bracket(rows, atTs) {
        let lo = 0, hi = rows.length - 1;
        if (atTs <= rows[0].ts) return [rows[0], rows[0]];
        if (atTs >= rows[hi].ts) return [rows[hi], rows[hi]];
        while (lo < hi) {
          const mid = (lo + hi) >> 1;
          if (rows[mid].ts <= atTs) lo = mid + 1;
          else hi = mid;
        }
        return [rows[lo - 1], rows[lo]];
      }

      function jsSnapshotAt(atTs, mask, minLat, minLng, maxLat, maxLng) {
        const out = [];
        for (let s = 0; s < bySensor.length; s++) {
          if ((mask & (1 << s)) === 0) continue;
          const rows = bySensor[s];
          if (rows.length === 0) continue;
          const [a, b] = bracket(rows, atTs);
          const f = b.ts > a.ts ? Math.min(1, Math.max(0, (atTs - a.ts) / (b.ts - a.ts))) : 0;
          const lerp = (x, y) => x + (y - x) * f;
          const lat = lerp(a.lat, b.lat);
          const lng = lerp(a.lng, b.lng);
          if (lat < minLat || lat > maxLat || lng < minLng || lng > maxLng) continue;
          out.push([s, lat, lng, lerp(a.temp_f, b.temp_f), lerp(a.vibration_g, b.vibration_g), atTs]);
        }
        return out;
      }

      const fullMask = (1 << sensorIds.length) - 1;
      const bbox = [-90, -180, 90, 180];

      // Warm up the JIT / wasm call path before timing, and consume the
      // result the same way the real app does: index into the typed array
      // in place (`rows[i]`), never materialize it via Array.from -- that
      // conversion isn't part of the real call site in wasm_engine.js and
      // would unfairly tax the WASM side.
      function timeit(fn, warmupRuns = 20) {
        for (let i = 0; i < warmupRuns; i++) fn(i);
        const t0 = performance.now();
        let lastLen = 0;
        for (let i = 0; i < nCalls; i++) lastLen = fn(i).length;
        const t1 = performance.now();
        return { totalMs: t1 - t0, perCallMs: (t1 - t0) / nCalls, lastLen };
      }

      const wasmRange = timeit((i) => cloud.range(fullMask, minTs, maxTs, ...bbox));
      const jsRangeResult = timeit((i) => jsRange(fullMask, minTs, maxTs, ...bbox));

      const wasmSnapshot = timeit((i) => {
        const atTs = minTs + ((maxTs - minTs) * i) / nCalls;
        return cloud.snapshot_at(atTs, fullMask, ...bbox);
      });
      const jsSnapshot = timeit((i) => {
        const atTs = minTs + ((maxTs - minTs) * i) / nCalls;
        return jsSnapshotAt(atTs, fullMask, ...bbox);
      });

      return {
        readingCount: readings.length,
        sensorCount: sensorIds.length,
        nCalls,
        wasmRange,
        jsRange: jsRangeResult,
        wasmSnapshot,
        jsSnapshot,
      };
    },
    { scenario: SCENARIO, nCalls: N_CALLS }
  );
}

async function onPageReadout(page, url) {
  await page.goto(url, { waitUntil: "networkidle" });
  await page.waitForFunction(() => {
    const el = document.getElementById("perf-readout");
    return el && !el.hidden && el.textContent.trim().length > 0;
  }, { timeout: 30000 });
  return await page.textContent("#perf-readout");
}

async function playbackSweep(page, url, ticks = 8) {
  await page.goto(url, { waitUntil: "networkidle" });
  await page.click('input[name="view-mode"][value="playback"]');
  const max = await page.locator("#time-slider").getAttribute("max");
  const readouts = [];
  for (let i = 0; i <= ticks; i++) {
    const value = Math.round((Number(max) * i) / ticks);
    await page.evaluate((v) => {
      const el = document.getElementById("time-slider");
      el.value = String(v);
      el.dispatchEvent(new Event("input", { bubbles: true }));
    }, value);
    await page.waitForTimeout(30);
    readouts.push(await page.textContent("#perf-readout"));
  }
  return readouts;
}

async function main() {
  const browser = await chromium.launch();
  const results = {};

  for (const throttle of THROTTLES) {
    const page = await browser.newPage();
    const client = await page.context().newCDPSession(page);
    await client.send("Emulation.setCPUThrottlingRate", { rate: throttle });

    await page.goto(`${BASE}/?scenario=${SCENARIO}&engine=wasm`, { waitUntil: "networkidle" });
    const isolated = await isolatedBenchmark(page, throttle);

    const wasmAllReadout = await onPageReadout(page, `${BASE}/?scenario=${SCENARIO}&engine=wasm`);
    const wasmPlayback = await playbackSweep(page, `${BASE}/?scenario=${SCENARIO}&engine=wasm`);

    const legacyAllReadout = await onPageReadout(page, `${BASE}/?scenario=${SCENARIO}&engine=legacy`);
    const legacyPlayback = await playbackSweep(page, `${BASE}/?scenario=${SCENARIO}&engine=legacy`);

    results[`${throttle}x`] = {
      isolated,
      onPage: { wasmAllReadout, wasmPlayback, legacyAllReadout, legacyPlayback },
    };

    await page.close();
  }

  await browser.close();
  console.log(JSON.stringify(results, null, 2));
}

main();
