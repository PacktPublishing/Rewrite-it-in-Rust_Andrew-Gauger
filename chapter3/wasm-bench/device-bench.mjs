// Drives the real app on a real device (or desktop Chrome) through each
// scenario, both engines, several attempts, and records what the page's
// own readouts measured: Play's frames per second and worst frame, and the
// full-speed run's wall time and per-frame timings.
//
//   Android, over USB (see README.md for the one-time tablet setup):
//     node device-bench.mjs --android --label sm-t290
//
//   Desktop Chrome, optionally throttled, to try the script out:
//     node device-bench.mjs --throttle 4 --label desktop-4x
//
// Options:
//   --base URL        site to test (default http://localhost:3737; with
//                     --android the port is reversed over adb, so the
//                     tablet reaches the Mac's server as localhost)
//   --attempts N      fresh page loads per engine and scenario (default 3)
//   --play-seconds N  how long Play runs before it is sampled (default 10)
//   --label NAME      used in the output file name
//
// Writes results-<label>-<timestamp>.json (every sample, every frame) and
// prints one Markdown table per scenario, median across attempts.

import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { chromium, _android as android } from "playwright";

const args = process.argv.slice(2);
const flag = (name) => args.includes(`--${name}`);
const opt = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i === -1 ? fallback : args[i + 1];
};

const onAndroid = flag("android");
const base = opt("base", "http://localhost:3737");
const attempts = Number(opt("attempts", 3));
const playSeconds = Number(opt("play-seconds", 10));
const throttle = Number(opt("throttle", 1));
const label = opt("label", onAndroid ? "android" : `desktop-${throttle}x`);

// The cases the chapter reports: the agriculture cluster, and the container
// fleet both at the default zoom and zoomed in two levels.
const scenarios = [
  { name: "cluster", zoomIn: 0, title: "Agricultural sensors (cluster)" },
  { name: "highway", zoomIn: 0, title: "Container fleet (highway), default zoom" },
  { name: "highway", zoomIn: 2, title: "Container fleet (highway), zoomed in 2 levels" }
];
const engines = ["legacy", "wasm"];

async function openBrowser() {
  if (onAndroid) {
    const port = new URL(base).port || "80";
    execFileSync("adb", ["reverse", `tcp:${port}`, `tcp:${port}`]);
    const [device] = await android.devices();
    if (!device) throw new Error("no Android device found: check `adb devices`");
    const model = device.model();
    await device.shell("am force-stop com.android.chrome");
    const context = await device.launchBrowser();
    const userAgent = await (await context.newPage()).evaluate(() => navigator.userAgent);
    return { context, describe: `${model} · ${userAgent}`, close: () => device.close() };
  }
  const browser = await chromium.launch({ channel: "chrome", headless: true });
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  return { context, describe: `desktop Chrome ${browser.version()}, ${throttle}x CPU throttle`, close: () => browser.close() };
}

async function runOnce(context, scenario, engine) {
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  if (!onAndroid && throttle > 1) {
    const cdp = await context.newCDPSession(page);
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: throttle });
  }

  await page.goto(`${base}/?scenario=${scenario.name}&engine=${engine}`);
  await page.waitForFunction(() => document.getElementById("perf-readout").textContent, null, {
    timeout: 120_000
  });
  for (let i = 0; i < scenario.zoomIn; i++) {
    await page.click(".leaflet-control-zoom-in");
    await page.waitForTimeout(1500);
  }
  await page.check('input[value="playback"]');

  // Play: let it settle for a second, then keep only the samples taken
  // while it ran.
  await page.click("#play-pause");
  await page.waitForTimeout(1000);
  const firstSample = await page.evaluate(() => window.fpsSamples.length);
  await page.waitForTimeout(playSeconds * 1000);
  const play = await page.evaluate((from) => window.fpsSamples.slice(from), firstSample);
  await page.click("#play-pause");
  await page.waitForTimeout(500);

  await page.click("#run-full-speed");
  await page.waitForFunction(() => window.fullSpeedRun, null, { timeout: 300_000 });
  const fullSpeed = await page.evaluate(() => window.fullSpeedRun);

  await page.close();
  return { play, fullSpeed, errors };
}

const median = (xs) => {
  const s = [...xs].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};
const mean = (xs) => xs.reduce((a, b) => a + b, 0) / xs.length;

// The six rows of the chapter's table, from one attempt's raw results.
const metrics = [
  ["Play: average FPS", (r) => mean(r.play.map((s) => s.fps)), 1],
  ["Play: worst frame (ms)", (r) => Math.max(...r.play.map((s) => s.worstFrameMs)), 1],
  ["Full speed: 360 frames (s)", (r) => r.fullSpeed.wallMs / 1000, 2],
  ["Full speed: p99 frame (ms)", (r) => r.fullSpeed.frame.p99, 1],
  ["Full speed: worst frame (ms)", (r) => r.fullSpeed.frame.worst, 1],
  ["Full speed: avg render work (ms)", (r) => r.fullSpeed.work.avg, 2]
];

const { context, describe, close } = await openBrowser();
console.log(`# ${describe}\n# ${base}, ${attempts} attempts, Play sampled for ${playSeconds} s\n`);

const results = [];
for (const scenario of scenarios) {
  for (const engine of engines) {
    for (let attempt = 1; attempt <= attempts; attempt++) {
      process.stderr.write(`${scenario.title} · ${engine} · attempt ${attempt}… `);
      const r = await runOnce(context, scenario, engine);
      process.stderr.write(`${r.fullSpeed.fps.toFixed(1)} fps full speed\n`);
      results.push({ scenario: scenario.title, engine, attempt, ...r });
    }
  }

  console.log(`## ${scenario.title}, median of ${attempts}\n`);
  console.log("| metric | JavaScript | Wasm |\n|---|---|---|");
  for (const [name, pick, digits] of metrics) {
    const cell = (engine) =>
      median(results.filter((r) => r.scenario === scenario.title && r.engine === engine).map(pick)).toFixed(digits);
    console.log(`| ${name} | ${cell("legacy")} | ${cell("wasm")} |`);
  }
  console.log();
}

const errors = results.flatMap((r) => r.errors);
if (errors.length) console.log(`Page errors: ${errors.join("; ")}`);

const file = `results-${label}-${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
writeFileSync(file, JSON.stringify({ describe, base, attempts, playSeconds, throttle, results }, null, 2));
console.log(`Raw results: ${file}`);
await close();
