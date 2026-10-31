// A plain top-level async IIFE, not a DOMContentLoaded listener: this file
// is loaded as an ES module after the surrounding HTML has already parsed,
// and each engine switch is a full page load (see the `data-turbo="false"`
// links in the view), so there's no second DOMContentLoaded to wait for.
(async function () {
  const mapEl = document.getElementById("map");
  const scenario = mapEl.dataset.scenario;
  const perfEl = document.getElementById("perf-readout");
  perfEl.hidden = false;

  const res = await fetch(`/readings.json?scenario=${scenario}`);
  const readings = await res.json();

  document.getElementById("reading-count-text").textContent =
    `${readings.length.toLocaleString()} raw readings from ` +
    `${new Set(readings.map((r) => r.sensor_id)).size} sensors over the last 6 hours, ` +
    "one marker per reading — no clustering, no aggregation.";

  const center = readings.length ? [readings[0].lat, readings[0].lng] : [41.05, -98.35];
  const map = L.map("map").setView(center, 11);

  L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png", {
    maxZoom: 19,
    attribution: "&copy; OpenStreetMap contributors"
  }).addTo(map);

  const markers = L.layerGroup().addTo(map);

  function colorFor(vibrationG) {
    return vibrationG > 1.0 ? "#d33" : vibrationG > 0.3 ? "#e8a33d" : "#3d8fe8";
  }

  function addMarker(r) {
    const color = colorFor(r.vibration_g);
    const marker = L.circleMarker([r.lat, r.lng], {
      radius: 5,
      color,
      weight: 1,
      fillColor: color,
      fillOpacity: 0.6
    });
    marker.bindPopup(
      `<strong>${r.sensor_id}</strong><br>${r.temperature_f}&deg;F &middot; ${r.vibration_g}g<br>` +
        new Date(r.recorded_at).toLocaleString()
    );
    markers.addLayer(marker);
  }

  // Same measurement window as the WASM engine's perf readout: JS-side
  // scan/filter plus the Leaflet DOM marker churn it triggers, so the two
  // numbers land in the same units and can be compared directly.
  function reportPerf(start, label, rowCount) {
    const elapsed = (performance.now() - start).toFixed(2);
    perfEl.textContent =
      `${label}: ${elapsed} ms · ${rowCount.toLocaleString()} markers rendered of ` +
      `${readings.length.toLocaleString()} total (rough client-side timing, not a device benchmark)`;
  }

  // Deliberately naive: every reading gets its own Leaflet marker, added
  // straight to the map with no clustering, no canvas renderer, no
  // virtualization. This is the "legacy" rendering path the book replaces
  // with WASM/Rust.
  function renderAll() {
    const start = performance.now();
    markers.clearLayers();
    readings.forEach(addMarker);
    reportPerf(start, "Legacy JS (show all)", readings.length);
  }

  const framesByTime = {};
  readings.forEach((r) => {
    (framesByTime[r.recorded_at] = framesByTime[r.recorded_at] || []).push(r);
  });
  const timestamps = Object.keys(framesByTime).sort();

  const slider = document.getElementById("time-slider");
  const timeLabel = document.getElementById("time-label");
  const playPauseBtn = document.getElementById("play-pause");
  const playbackControls = document.querySelector(".playback-controls");
  slider.max = Math.max(timestamps.length - 1, 0);

  let currentIdx = 0;
  let playing = false;
  let timer = null;

  function renderFrame(idx) {
    const start = performance.now();
    markers.clearLayers();
    const frame = framesByTime[timestamps[idx]] || [];
    frame.forEach(addMarker);
    timeLabel.textContent = timestamps[idx] ? new Date(timestamps[idx]).toLocaleString() : "";
    reportPerf(start, "Legacy JS (playback)", frame.length);
  }

  function stopPlayback() {
    playing = false;
    playPauseBtn.textContent = "Play";
    clearInterval(timer);
  }

  function startPlayback() {
    playing = true;
    playPauseBtn.textContent = "Pause";
    timer = setInterval(() => {
      currentIdx = (currentIdx + 1) % timestamps.length;
      slider.value = currentIdx;
      renderFrame(currentIdx);
    }, 150);
  }

  playPauseBtn.addEventListener("click", () => (playing ? stopPlayback() : startPlayback()));

  slider.addEventListener("input", () => {
    stopPlayback();
    currentIdx = parseInt(slider.value, 10);
    renderFrame(currentIdx);
  });

  document.querySelectorAll('input[name="view-mode"]').forEach((radio) => {
    radio.addEventListener("change", () => {
      stopPlayback();
      if (radio.value === "playback" && radio.checked) {
        playbackControls.hidden = false;
        currentIdx = 0;
        slider.value = 0;
        renderFrame(0);
      } else if (radio.value === "all" && radio.checked) {
        playbackControls.hidden = true;
        renderAll();
      }
    });
  });

  renderAll();
})();
