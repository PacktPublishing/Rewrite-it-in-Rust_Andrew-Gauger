// Full-speed playback, shared by both engines so they do identical work:
// every distinct reading timestamp, in order, one per animation frame, with
// no timer between them. Each frame renders the markers for one instant,
// then yields so the browser can paint it before the next one starts.
//
// What comes out of it:
//
//   wall time    first frame to last painted frame: what a person watching
//                the whole playback waits. On a fast machine this bottoms
//                out at the display's refresh rate (60 Hz = 16.7 ms/frame).
//   frame time   the gap between consecutive animation frames: how long
//                each step stayed on screen. Its p99 and worst are the
//                stutter a person sees.
//   work time    the render calls alone (query + Leaflet markers), which
//                the refresh rate does not cap.
//
// The finished run is also left on window.fullSpeedRun, every frame
// included, so a script driving the page can collect it.

function percentile(sorted, p) {
  if (sorted.length === 0) return 0;
  const idx = Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1);
  return sorted[Math.max(idx, 0)];
}

function summarize(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const total = values.reduce((sum, v) => sum + v, 0);
  return {
    total,
    avg: total / (values.length || 1),
    p50: percentile(sorted, 50),
    p95: percentile(sorted, 95),
    p99: percentile(sorted, 99),
    worst: sorted[sorted.length - 1] ?? 0
  };
}

export function attachFullSpeedRun({ button, readout, frameCount, renderFrame, beforeRun, afterRun = () => {} }) {
  readout.hidden = false;
  readout.textContent = `full-speed run: ${frameCount} frames, press "Run full speed"`;

  button.addEventListener("click", () => {
    beforeRun();
    button.disabled = true;
    window.fullSpeedRun = null;

    let i = 0;
    let started = null;
    let lastFrame = null;
    const workMs = [];
    const frameMs = [];

    function step(now) {
      if (started === null) started = now;
      if (lastFrame !== null) frameMs.push(now - lastFrame);
      lastFrame = now;

      if (i < frameCount) {
        const t0 = performance.now();
        renderFrame(i);
        workMs.push(performance.now() - t0);
        i += 1;
        if (i % 30 === 0) readout.textContent = `full-speed run: ${i} / ${frameCount} frames…`;
        requestAnimationFrame(step);
        return;
      }

      // This callback is the first frame after the last render was painted.
      const wallMs = now - started;
      const work = summarize(workMs);
      const frame = summarize(frameMs);
      window.fullSpeedRun = {
        frames: frameCount,
        wallMs,
        fps: (frameCount * 1000) / wallMs,
        work,
        frame,
        workMs,
        frameMs
      };
      readout.textContent =
        `full-speed run: ${frameCount} frames in ${(wallMs / 1000).toFixed(2)} s ` +
        `(${window.fullSpeedRun.fps.toFixed(1)} fps) · frame p99 ${frame.p99.toFixed(1)} ms, ` +
        `worst ${frame.worst.toFixed(1)} ms · work ${work.total.toFixed(0)} ms total, ` +
        `${work.avg.toFixed(2)} ms avg, ${work.worst.toFixed(1)} ms worst`;
      afterRun();
      button.disabled = false;
    }
    requestAnimationFrame(step);
  });
}
