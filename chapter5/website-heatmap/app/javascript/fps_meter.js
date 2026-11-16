// Frames-per-second readout, shared by both engines so the two report in
// the same units. Three numbers, refreshed once a second:
//
//   paint fps    requestAnimationFrame callbacks per second: how often the
//                browser could put a new frame on screen
//   updates/s    playback ticks that finished rendering, per second: how
//                many map updates the engine delivered
//   worst frame  the longest gap between two animation frames in that
//                second: the stutter a person sees
//
// performance.now() around one tick measures one function. These measure
// what reaches the screen.

export function startFpsMeter(el) {
  el.hidden = false;

  let frames = 0;
  let updates = 0;
  let worstGap = 0;
  let lastFrame = performance.now();
  let windowStart = lastFrame;

  function onFrame(now) {
    frames += 1;
    worstGap = Math.max(worstGap, now - lastFrame);
    lastFrame = now;

    const elapsed = now - windowStart;
    if (elapsed >= 1000) {
      const fps = (frames * 1000) / elapsed;
      const ups = (updates * 1000) / elapsed;
      el.textContent =
        `paint ${fps.toFixed(1)} fps · playback ${ups.toFixed(1)} updates/s · ` +
        `worst frame ${worstGap.toFixed(0)} ms`;
      frames = 0;
      updates = 0;
      worstGap = 0;
      windowStart = now;
    }
    requestAnimationFrame(onFrame);
  }
  requestAnimationFrame(onFrame);

  return {
    // Call once after each playback tick has rendered its markers.
    update() {
      updates += 1;
    }
  };
}
