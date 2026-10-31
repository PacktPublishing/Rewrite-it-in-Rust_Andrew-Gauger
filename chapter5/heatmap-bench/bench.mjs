// Extends chapter3/wasm-bench to the chapter-5 paint pipeline. Same method
// as that bench's "isolated call cost" pass: run everything inside a real
// Chromium page under Chrome DevTools Protocol CPU throttling, warmed up,
// and compare the real wasm-bindgen call against a hand-written JS
// implementation of the identical algorithm. No Leaflet, no app chrome.
//
// What's new here vs. chapter 3:
//   - density_grid   : the KDE compute alone (Float64Array out, no canvas)
//   - paint_heatmap  : the whole chapter-5 pipeline -- filter, project,
//                      KDE splat, palette lookup, ImageData, put_image_data
//   - paint_heatmap_labels : the float-formatting text layer
//   - jsPaintHeatmap : the "before" chapter 5 says nobody wrote -- the same
//                      splat + palette LUT + putImageData, in plain JS
//
// range / snapshot_at are carried over unchanged so this bench stands on
// its own and the two chapters' numbers stay comparable.
//
// Usage:  node bench.mjs          (writes JSON to stdout)
//         node bench.mjs --quick  (one throttle, fewer iterations -- smoke test)

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join, normalize } from "node:path";
import { chromium } from "playwright";

const HERE = dirname(fileURLToPath(import.meta.url));
const QUICK = process.argv.includes("--quick");

const SCENARIOS = ["cluster", "highway"];
const THROTTLES = QUICK ? [4] : [1, 4, 6]; // CPU slowdown multipliers (1x = unthrottled)
const N_LIGHT = QUICK ? 400 : 2000; // range / snapshot_at
const N_HEAVY = QUICK ? 60 : 300; // density_grid / paint_heatmap / labels
const TARGET_VIEWPORT_PX = 1024; // fit each scenario's data to ~this wide, then snap zoom to an integer
const CELL_SIZE_PX = 4; // the value the app passes to paint_heatmap

// ---------------------------------------------------------------------------
// static file server: harness.html at /, plus /wasm/* and /fixtures/*
// ---------------------------------------------------------------------------
const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
  ".json": "application/json; charset=utf-8",
};

function startServer() {
  const server = createServer(async (req, res) => {
    try {
      const urlPath = decodeURIComponent(req.url.split("?")[0]);
      const rel = urlPath === "/" ? "/harness.html" : urlPath;
      const abs = normalize(join(HERE, rel));
      if (!abs.startsWith(HERE)) {
        res.writeHead(403).end("no");
        return;
      }
      const body = await readFile(abs);
      const ext = abs.slice(abs.lastIndexOf("."));
      res.writeHead(200, { "content-type": TYPES[ext] || "application/octet-stream" });
      res.end(body);
    } catch {
      res.writeHead(404).end("not found");
    }
  });
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      resolve({ url: `http://127.0.0.1:${port}`, close: () => server.close() });
    });
  });
}

// ---------------------------------------------------------------------------
// the measurement, executed inside the page (has DOM, canvas, wasm)
// ---------------------------------------------------------------------------
async function measureInPage(page, { readings, nLight, nHeavy, targetPx, cellPx }) {
  return page.evaluate(
    async ({ readings, nLight, nHeavy, targetPx, cellPx }) => {
      // --- load data into wasm, exactly like wasm_engine.js does ----------
      const sensorIds = Array.from(new Set(readings.map((r) => r.sensor_id))).sort();
      const idxOf = new Map(sensorIds.map((id, i) => [id, i]));
      let minTs = Infinity;
      let maxTs = -Infinity;
      const flat = new Float64Array(readings.length * 6);
      let c = 0;
      for (const r of readings) {
        const ts = Date.parse(r.recorded_at);
        if (ts < minTs) minTs = ts;
        if (ts > maxTs) maxTs = ts;
        flat[c++] = idxOf.get(r.sensor_id);
        flat[c++] = r.lat;
        flat[c++] = r.lng;
        flat[c++] = r.temperature_f;
        flat[c++] = r.vibration_g;
        flat[c++] = ts;
      }

      const wasm = await import("/wasm/viz_core.js");
      await wasm.default();
      const cloud = new wasm.PointCloud(flat, sensorIds.length);

      const fullMask = sensorIds.length >= 32 ? 0xffffffff : ((1 << sensorIds.length) - 1) >>> 0;

      // --- pick a representative viewport: fit the data, snap zoom to int -
      let minLat = Infinity, minLng = Infinity, maxLat = -Infinity, maxLng = -Infinity;
      for (const r of readings) {
        if (r.lat < minLat) minLat = r.lat;
        if (r.lat > maxLat) maxLat = r.lat;
        if (r.lng < minLng) minLng = r.lng;
        if (r.lng > maxLng) maxLng = r.lng;
      }
      // pad 8% so edge readings aren't sitting exactly on the bbox line
      const padLat = (maxLat - minLat) * 0.08 || 0.001;
      const padLng = (maxLng - minLng) * 0.08 || 0.001;
      minLat -= padLat; maxLat += padLat; minLng -= padLng; maxLng += padLng;
      const rawZoom = Math.log2((targetPx * 360) / ((maxLng - minLng) * 256));
      const zoom = Math.max(1, Math.min(20, Math.round(rawZoom)));
      const bbox = [minLat, minLng, maxLat, maxLng];

      // ================= plain-JS baseline (the "before") ================
      // per-sensor rows, chronological -- the JS side of PointCloud::new's
      // by_sensor. Parse/resolve once, up front: same fairness bar as
      // chapter 3's bench.
      const bySensor = sensorIds.map(() => []);
      for (const r of readings) {
        bySensor[idxOf.get(r.sensor_id)].push({
          ts: Date.parse(r.recorded_at),
          lat: r.lat, lng: r.lng, temp: r.temperature_f, vib: r.vibration_g,
        });
      }
      for (const rows of bySensor) rows.sort((a, b) => a.ts - b.ts);

      // -- density.rs constants + fns (verbatim) --
      const TEMP_BASELINE_F = 70.0, TEMP_SPAN_F = 50.0, VIBRATION_SATURATION_G = 1.0;
      const TEMP_WEIGHT = 0.35, VIBRATION_WEIGHT = 0.65;
      const MIN_SIGMA_PX = 8.0, MAX_SIGMA_PX = 48.0;
      const jsIntensity = (tempF, vibG) => {
        const tempScore = Math.min(Math.abs(tempF - TEMP_BASELINE_F) / TEMP_SPAN_F, 1.0);
        const vibScore = Math.min(Math.abs(vibG) / VIBRATION_SATURATION_G, 1.0);
        return TEMP_WEIGHT * tempScore + VIBRATION_WEIGHT * vibScore;
      };
      const jsSigma = (vibG) => {
        const t = Math.min(Math.abs(vibG) / VIBRATION_SATURATION_G, 1.0);
        return MIN_SIGMA_PX + t * (MAX_SIGMA_PX - MIN_SIGMA_PX);
      };

      // -- mercator.rs project (verbatim) --
      const jsProject = (lng, lat, z) => {
        const size = 256 * 2 ** z;
        const x = ((lng + 180) / 360) * size;
        const siny = Math.min(0.9999, Math.max(-0.9999, Math.sin((lat * Math.PI) / 180)));
        const y = (0.5 - Math.log((1 + siny) / (1 - siny)) / (4 * Math.PI)) * size;
        return [x, y];
      };

      // -- palette.rs (verbatim: STOPS, ramp, 256-entry LUT, rgba_for) --
      const STOPS = [
        [0.0, 0, 0, 255, 0],
        [0.25, 0, 255, 255, 90],
        [0.5, 0, 255, 0, 150],
        [0.75, 255, 255, 0, 200],
        [1.0, 255, 0, 0, 255],
      ];
      const clamp01 = (t) => (t < 0 ? 0 : t > 1 ? 1 : t);
      const lerpU8 = (a, b, f) => (a + (b - a) * f + 0.5) | 0;
      const ramp = (t) => {
        t = clamp01(t);
        for (let i = 0; i < STOPS.length - 1; i++) {
          const [t0, r0, g0, b0, a0] = STOPS[i];
          const [t1, r1, g1, b1, a1] = STOPS[i + 1];
          if (t >= t0 && t <= t1) {
            const f = t1 > t0 ? (t - t0) / (t1 - t0) : 0;
            return [lerpU8(r0, r1, f), lerpU8(g0, g1, f), lerpU8(b0, b1, f), lerpU8(a0, a1, f)];
          }
        }
        const s = STOPS[STOPS.length - 1];
        return [s[1], s[2], s[3], s[4]];
      };
      const PALETTE = Array.from({ length: 256 }, (_, i) => ramp(i / 255));
      const MAX_CELL_DENSITY = 1.0;
      const rgbaFor = (density) => PALETTE[(clamp01(density / MAX_CELL_DENSITY) * 255) | 0];

      // -- lib.rs filtered_snapshot (bracket + interpolate + bbox) --
      const jsFilteredSnapshot = (atTs, mask, [bMinLat, bMinLng, bMaxLat, bMaxLng]) => {
        const out = [];
        for (let s = 0; s < bySensor.length; s++) {
          const rows = bySensor[s];
          if (rows.length === 0 || (mask & (1 << s)) === 0) continue;
          let lo = 0, hi = rows.length;
          while (lo < hi) {
            const mid = (lo + hi) >>> 1;
            if (rows[mid].ts <= atTs) lo = mid + 1;
            else hi = mid;
          }
          const split = lo;
          const a = rows[split > 0 ? split - 1 : 0];
          const b = rows[Math.min(split, rows.length - 1)];
          const f = b.ts > a.ts ? Math.min(1, Math.max(0, (atTs - a.ts) / (b.ts - a.ts))) : 0;
          const lat = a.lat + (b.lat - a.lat) * f;
          const lng = a.lng + (b.lng - a.lng) * f;
          if (lng < bMinLng || lng > bMaxLng || lat < bMinLat || lat > bMaxLat) continue;
          out.push({
            sensorIdx: s, lat, lng,
            temp: a.temp + (b.temp - a.temp) * f,
            vib: a.vib + (b.vib - a.vib) * f,
          });
        }
        return out;
      };

      // -- lib.rs compute_density_grid + density.rs compute_grid --
      const jsDensityGrid = (atTs, mask, bb, z, cell) => {
        const rd = jsFilteredSnapshot(atTs, mask, bb);
        const [ox, oy] = jsProject(bb[1], bb[2], z); // (minLng, maxLat)
        const [mx] = jsProject(bb[3], bb[2], z); // (maxLng, maxLat)
        const [, my] = jsProject(bb[1], bb[0], z); // (minLng, minLat)
        const widthPx = mx - ox;
        const heightPx = my - oy;
        const cols = Math.max(Math.ceil(widthPx / cell), 1);
        const rows = Math.max(Math.ceil(heightPx / cell), 1);
        const cells = new Float64Array(cols * rows);
        for (const r of rd) {
          const [px, py] = jsProject(r.lng, r.lat, z);
          const x = px - ox, y = py - oy;
          const sigma = jsSigma(r.vib);
          const inten = jsIntensity(r.temp, r.vib);
          const cutoff = 3.0 * sigma;
          if (x + cutoff < 0 || x - cutoff > widthPx || y + cutoff < 0 || y - cutoff > heightPx) continue;
          const colLo = Math.max(Math.floor((x - cutoff) / cell), 0);
          const colHi = Math.min(Math.ceil((x + cutoff) / cell), cols - 1);
          const rowLo = Math.max(Math.floor((y - cutoff) / cell), 0);
          const rowHi = Math.min(Math.ceil((y + cutoff) / cell), rows - 1);
          const twoSigmaSq = 2.0 * sigma * sigma;
          const cutoffSq = cutoff * cutoff;
          for (let row = rowLo; row <= rowHi; row++) {
            const dy = (row + 0.5) * cell - y;
            for (let col = colLo; col <= colHi; col++) {
              const dx = (col + 0.5) * cell - x;
              const d2 = dx * dx + dy * dy;
              if (d2 > cutoffSq) continue;
              cells[row * cols + col] += inten * Math.exp(-d2 / twoSigmaSq);
            }
          }
        }
        return { cols, rows, cells };
      };

      const jsCanvas = document.createElement("canvas");
      const jsPaintHeatmap = (atTs, mask, bb, z, cell) => {
        const { cols, rows, cells } = jsDensityGrid(atTs, mask, bb, z, cell);
        const px = new Uint8ClampedArray(cols * rows * 4);
        // Mirror of paint_heatmap's Rust loop: one palette lookup per cell,
        // four bytes written per cell. Plain scalar writes, not px.set(),
        // which is the fair equivalent of Rust's copy_from_slice(&[u8; 4]).
        for (let i = 0; i < cells.length; i++) {
          const c = rgbaFor(cells[i]);
          const o = i * 4;
          px[o] = c[0];
          px[o + 1] = c[1];
          px[o + 2] = c[2];
          px[o + 3] = c[3];
        }
        jsCanvas.width = cols;
        jsCanvas.height = rows;
        const ctx = jsCanvas.getContext("2d");
        ctx.putImageData(new ImageData(px, cols, rows), 0, 0);
        return cols;
      };

      // -- lib.rs range / snapshot_at (plain-JS equivalents, from ch3) --
      const rowsFlat = readings.map((r) => ({
        idx: idxOf.get(r.sensor_id),
        lat: r.lat, lng: r.lng, temp: r.temperature_f, vib: r.vibration_g,
        ts: Date.parse(r.recorded_at),
      }));
      const jsRange = (mask, sTs, eTs, [bMinLat, bMinLng, bMaxLat, bMaxLng]) => {
        const out = [];
        for (const r of rowsFlat) {
          if ((mask & (1 << r.idx)) === 0) continue;
          if (r.ts < sTs || r.ts > eTs) continue;
          if (r.lat < bMinLat || r.lat > bMaxLat || r.lng < bMinLng || r.lng > bMaxLng) continue;
          out.push(r.idx, r.lat, r.lng, r.temp, r.vib, r.ts);
        }
        return out;
      };
      const jsSnapshotAt = (atTs, mask, bb) =>
        jsFilteredSnapshot(atTs, mask, bb).flatMap((r) => [r.sensorIdx, r.lat, r.lng, r.temp, r.vib, atTs]);

      // ============================ timing =============================
      function timeit(fn, n, warmup = 20) {
        for (let i = 0; i < warmup; i++) fn(i);
        const t0 = performance.now();
        let acc = 0;
        for (let i = 0; i < n; i++) acc += fn(i) | 0;
        const t1 = performance.now();
        return { totalMs: +(t1 - t0).toFixed(3), perCallMs: +((t1 - t0) / n).toFixed(5), n };
      }

      const heatmapCanvas = document.getElementById("heatmap");
      const labelsCanvas = document.getElementById("labels");
      const sweptTs = (i) => minTs + ((maxTs - minTs) * (i % 512)) / 512;

      // shape / sanity, measured once (not timed)
      const dgHeader = cloud.density_grid(minTs + (maxTs - minTs) / 2, fullMask, ...bbox, zoom, cellPx);
      const wasmCols = dgHeader[0], wasmRows = dgHeader[1];
      const jsGrid = jsDensityGrid(minTs + (maxTs - minTs) / 2, fullMask, bbox, zoom, cellPx);
      const activeSensors = jsFilteredSnapshot(minTs + (maxTs - minTs) / 2, fullMask, bbox).length;

      // one guarded real call each, so a thrown JsValue shows up clearly
      cloud.paint_heatmap(heatmapCanvas, minTs, fullMask, ...bbox, zoom, cellPx);
      cloud.paint_heatmap_labels(labelsCanvas, minTs, fullMask, ...bbox, zoom, sensorIds);

      return {
        shape: {
          readingCount: readings.length,
          sensorCount: sensorIds.length,
          zoom,
          rawZoom: +rawZoom.toFixed(2),
          bbox: bbox.map((v) => +v.toFixed(5)),
          cellSizePx: cellPx,
          activeSensors,
          wasmGrid: [wasmCols, wasmRows],
          jsGrid: [jsGrid.cols, jsGrid.rows],
          gridCellsMatch: wasmCols === jsGrid.cols && wasmRows === jsGrid.rows,
        },
        wasmRange: timeit(() => cloud.range(fullMask, minTs, maxTs, ...bbox).length, nLight),
        jsRange: timeit(() => jsRange(fullMask, minTs, maxTs, bbox).length, nLight),
        wasmSnapshot: timeit((i) => cloud.snapshot_at(sweptTs(i), fullMask, ...bbox).length, nLight),
        jsSnapshot: timeit((i) => jsSnapshotAt(sweptTs(i), fullMask, bbox).length, nLight),
        wasmDensityGrid: timeit((i) => cloud.density_grid(sweptTs(i), fullMask, ...bbox, zoom, cellPx).length, nHeavy),
        jsDensityGrid: timeit((i) => jsDensityGrid(sweptTs(i), fullMask, bbox, zoom, cellPx).cells.length, nHeavy),
        wasmPaintHeatmap: timeit((i) => {
          cloud.paint_heatmap(heatmapCanvas, sweptTs(i), fullMask, ...bbox, zoom, cellPx);
          return heatmapCanvas.width;
        }, nHeavy),
        jsPaintHeatmap: timeit((i) => jsPaintHeatmap(sweptTs(i), fullMask, bbox, zoom, cellPx), nHeavy),
        wasmPaintLabels: timeit((i) => {
          cloud.paint_heatmap_labels(labelsCanvas, sweptTs(i), fullMask, ...bbox, zoom, sensorIds);
          return labelsCanvas.width;
        }, nHeavy),
      };
    },
    { readings, nLight, nHeavy, targetPx, cellPx }
  );
}

async function measure(page, opts) {
  await page.goto(opts.url, { waitUntil: "load" });
  return measureInPage(page, opts);
}

// ---------------------------------------------------------------------------
async function main() {
  const srv = await startServer();
  const fixtures = Object.fromEntries(
    await Promise.all(
      SCENARIOS.map(async (s) => [
        s,
        JSON.parse(await readFile(join(HERE, `fixtures/readings.${s}.json`), "utf8")),
      ])
    )
  );

  let browser;
  try {
    browser = await chromium.launch({ channel: "chrome" });
  } catch {
    browser = await chromium.launch(); // fall back to bundled chromium if present
  }

  const results = { meta: {}, runs: {} };
  results.meta = {
    started: new Date().toISOString(),
    node: process.version,
    scenarios: SCENARIOS,
    throttles: THROTTLES,
    nLight: N_LIGHT,
    nHeavy: N_HEAVY,
    targetViewportPx: TARGET_VIEWPORT_PX,
    cellSizePx: CELL_SIZE_PX,
    browser: browser.version?.() ?? "unknown",
    note:
      "Isolated call cost only (chapter 3 bench's pass 1), extended to the " +
      "chapter 5 paint pipeline. No Leaflet / no Rails app; served from a " +
      "static harness. Data is the real SensorDataGenerator output.",
  };

  for (const throttle of THROTTLES) {
    const page = await browser.newPage();
    const client = await page.context().newCDPSession(page);
    await client.send("Emulation.setCPUThrottlingRate", { rate: throttle });
    for (const scenario of SCENARIOS) {
      const out = await measure(page, {
        url: srv.url,
        readings: fixtures[scenario],
        nLight: N_LIGHT,
        nHeavy: N_HEAVY,
        targetPx: TARGET_VIEWPORT_PX,
        cellPx: CELL_SIZE_PX,
      });
      results.runs[`${scenario}@${throttle}x`] = out;
      process.stderr.write(
        `  ${scenario} @ ${throttle}x  grid ${out.shape.wasmGrid.join("x")}  ` +
          `paint wasm ${out.wasmPaintHeatmap.perCallMs}ms  js ${out.jsPaintHeatmap.perCallMs}ms\n`
      );
    }
    await page.close();
  }

  await browser.close();
  srv.close();
  results.meta.finished = new Date().toISOString();
  console.log(JSON.stringify(results, null, 2));
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
