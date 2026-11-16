// Full-speed playback, shared by both engines so they do identical work:
// every distinct reading timestamp, in order, one per animation frame, with
// no timer between them. Each frame renders the markers for one instant,
// then yields so the browser can paint it before the next one starts.
//
// Two totals come out of it:
//
//   wall time   first frame to last painted frame: what a person watching
//               the whole playback waits. On a fast machine this bottoms
//               out at the display's refresh rate (60 Hz = 16.7 ms/frame).
//   work time   the sum of the render calls alone (query + Leaflet
//               markers), which the refresh rate does not cap.

export function attachFullSpeedRun({ button, readout, frameCount, renderFrame, beforeRun }) {
  readout.hidden = false;
  readout.textContent = `full-speed run: ${frameCount} frames, press "Run full speed"`;

  button.addEventListener("click", () => {
    beforeRun();
    button.disabled = true;

    let i = 0;
    let workMs = 0;
    let worstWorkMs = 0;
    let started = null;

    function step(now) {
      if (started === null) started = now;

      if (i < frameCount) {
        const t0 = performance.now();
        renderFrame(i);
        const dt = performance.now() - t0;
        workMs += dt;
        worstWorkMs = Math.max(worstWorkMs, dt);
        i += 1;
        if (i % 30 === 0) readout.textContent = `full-speed run: ${i} / ${frameCount} frames…`;
        requestAnimationFrame(step);
        return;
      }

      // This callback is the first frame after the last render was painted.
      const wallMs = now - started;
      readout.textContent =
        `full-speed run: ${frameCount} frames in ${(wallMs / 1000).toFixed(2)} s ` +
        `(${((frameCount * 1000) / wallMs).toFixed(1)} fps) · work ${workMs.toFixed(0)} ms total, ` +
        `${(workMs / frameCount).toFixed(2)} ms avg, ${worstWorkMs.toFixed(1)} ms worst`;
      button.disabled = false;
    }
    requestAnimationFrame(step);
  });
}
