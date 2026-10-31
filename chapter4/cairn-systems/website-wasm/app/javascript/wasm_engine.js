// The whole point of this engine: the point cloud crosses into WASM linear
// memory exactly once (the Float64Array built below, handed to
// PointCloud::new). Every later interaction -- dragging the time slider,
// toggling a sensor, panning or zooming the map -- calls back into WASM
// with a handful of scalars and gets back an already-filtered slice. The
// map itself (tiles, chrome, Leaflet's own event wiring) stays JS on
// purpose; only the filtering/interpolation math moved to Rust.

function buildSensorPicker(sensorIds, onChange) {
  const container = document.getElementById("sensor-picker");
  container.hidden = false;
  container.innerHTML = "";

  const allLabel = document.createElement("label");
  const allCheckbox = document.createElement("input");
  allCheckbox.type = "checkbox";
  allCheckbox.checked = true;
  allLabel.appendChild(allCheckbox);
  allLabel.append(" All sensors");
  container.appendChild(allLabel);

  const checkboxes = sensorIds.map((id, idx) => {
    const label = document.createElement("label");
    const cb = document.createElement("input");
    cb.type = "checkbox";
    cb.checked = true;
    label.appendChild(cb);
    label.append(" " + id);
    container.appendChild(label);
    cb.addEventListener("change", () => {
      allCheckbox.checked = checkboxes.every((c) => c.checked);
      onChange();
    });
    return cb;
  });

  allCheckbox.addEventListener("change", () => {
    checkboxes.forEach((cb) => (cb.checked = allCheckbox.checked));
    onChange();
  });

  return () =>
    checkboxes.reduce((mask, cb, idx) => (cb.checked ? mask | (1 << idx) : mask), 0) >>> 0;
}

(async function () {
  const mapEl = document.getElementById("map");
  const scenario = mapEl.dataset.scenario;
  const perfEl = document.getElementById("perf-readout");
  perfEl.hidden = false;

  const [res, wasmModule] = await Promise.all([
    fetch(`/readings.json?scenario=${scenario}`),
    import("/wasm/viz_core.js")
  ]);
  const readings = await res.json();
  await wasmModule.default();

  const sensorIds = Array.from(new Set(readings.map((r) => r.sensor_id))).sort();
  const idxOf = new Map(sensorIds.map((id, i) => [id, i]));

  let minTs = Infinity;
  let maxTs = -Infinity;
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

  // The one bulk copy: this Float64Array crosses into WASM linear memory
  // exactly once. Everything from here on is scalars in, a filtered slice
  // out.
  const cloud = new wasmModule.PointCloud(flat, sensorIds.length);

  document.getElementById("reading-count-text").textContent =
    `${readings.length.toLocaleString()} raw readings from ${sensorIds.length} sensors, ` +
    "resident in WASM linear memory after a single bulk copy.";

  const center = readings.length ? [readings[0].lat, readings[0].lng] : [41.05, -98.35];
  const map = L.map("map").setView(center, 11);

  L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png", {
    maxZoom: 19,
    attribution: "&copy; OpenStreetMap contributors"
  }).addTo(map);

  const markerLayer = L.layerGroup().addTo(map);

  function colorFor(vibrationG) {
    return vibrationG > 1.0 ? "#d33" : vibrationG > 0.3 ? "#e8a33d" : "#3d8fe8";
  }

  function renderRows(rows) {
    markerLayer.clearLayers();
    for (let i = 0; i < rows.length; i += 6) {
      const sensorIdx = rows[i];
      const lat = rows[i + 1];
      const lng = rows[i + 2];
      const tempF = rows[i + 3];
      const vibrationG = rows[i + 4];
      const ts = rows[i + 5];
      const color = colorFor(vibrationG);

      const marker = L.circleMarker([lat, lng], {
        radius: 5,
        color,
        weight: 1,
        fillColor: color,
        fillOpacity: 0.6
      });
      marker.bindPopup(
        `<strong>${sensorIds[sensorIdx]}</strong><br>${tempF.toFixed(1)}&deg;F &middot; ` +
          `${vibrationG.toFixed(3)}g<br>${new Date(ts).toLocaleString()}`
      );
      markerLayer.addLayer(marker);
    }
  }

  function currentBounds() {
    const b = map.getBounds();
    return [b.getSouth(), b.getWest(), b.getNorth(), b.getEast()];
  }

  const getMask = buildSensorPicker(sensorIds, () => renderCurrent());

  const slider = document.getElementById("time-slider");
  const timeLabel = document.getElementById("time-label");
  const playPauseBtn = document.getElementById("play-pause");
  const playbackControls = document.querySelector(".playback-controls");
  const durationMs = maxTs - minTs;
  slider.min = 0;
  slider.max = durationMs;
  slider.step = 60000;
  slider.value = 0;

  let mode = "all";
  let playing = false;
  let timer = null;

  function renderCurrent() {
    const [minLat, minLng, maxLat, maxLng] = currentBounds();
    const mask = getMask();
    const start = performance.now();
    let rows;

    if (mode === "playback") {
      const atTs = minTs + parseInt(slider.value, 10);
      rows = cloud.snapshot_at(atTs, mask, minLat, minLng, maxLat, maxLng);
      timeLabel.textContent = new Date(atTs).toLocaleString();
    } else {
      rows = cloud.range(mask, minTs, maxTs, minLat, minLng, maxLat, maxLng);
    }

    renderRows(rows);
    const elapsed = (performance.now() - start).toFixed(2);
    perfEl.textContent =
      `WASM query: ${elapsed} ms · ${(rows.length / 6).toLocaleString()} points rendered of ` +
      `${readings.length.toLocaleString()} resident (rough client-side timing, not a device benchmark)`;
  }

  map.on("moveend zoomend", () => renderCurrent());

  function stopPlayback() {
    playing = false;
    playPauseBtn.textContent = "Play";
    clearInterval(timer);
  }

  function startPlayback() {
    playing = true;
    playPauseBtn.textContent = "Pause";
    timer = setInterval(() => {
      let next = parseInt(slider.value, 10) + 120000;
      if (next > durationMs) next = 0;
      slider.value = next;
      renderCurrent();
    }, 100);
  }

  playPauseBtn.addEventListener("click", () => (playing ? stopPlayback() : startPlayback()));

  slider.addEventListener("input", () => {
    stopPlayback();
    renderCurrent();
  });

  document.querySelectorAll('input[name="view-mode"]').forEach((radio) => {
    radio.addEventListener("change", () => {
      stopPlayback();
      if (radio.value === "playback" && radio.checked) {
        mode = "playback";
        playbackControls.hidden = false;
        slider.value = 0;
        renderCurrent();
      } else if (radio.value === "all" && radio.checked) {
        mode = "all";
        playbackControls.hidden = true;
        renderCurrent();
      }
    });
  });

  renderCurrent();
})();
