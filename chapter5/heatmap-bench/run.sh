#!/usr/bin/env bash
# End-to-end: regenerate the real dataset, (re)build the wasm from the
# current viz-core source, install the Playwright package if needed, run
# the benchmark. Writes bench-output.json.
#
#   ./run.sh            full run  (throttles 1x/4x/6x, both scenarios)
#   ./run.sh --quick    smoke test (4x only, fewer iterations)
set -euo pipefail
cd "$(dirname "$0")"

echo "==> regenerating fixtures from the real SensorDataGenerator"
ruby gen-readings.rb cluster > fixtures/readings.cluster.json
ruby gen-readings.rb highway > fixtures/readings.highway.json

echo "==> building viz-core -> wasm"
if command -v wasm-pack >/dev/null 2>&1; then
  ( cd ../viz-core && wasm-pack build --target web --out-dir pkg --out-name viz_core --no-opt >/dev/null )
  cp ../viz-core/pkg/viz_core.js ../viz-core/pkg/viz_core_bg.wasm wasm/
else
  echo "    wasm-pack not found; falling back to the committed ../viz-core/pkg"
  cp ../viz-core/pkg/viz_core.js ../viz-core/pkg/viz_core_bg.wasm wasm/
fi

if [ ! -d node_modules ]; then
  echo "==> installing playwright (browser download skipped; uses system Chrome)"
  PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 npm install --no-audit --no-fund
fi

echo "==> running bench"
node bench.mjs "$@" > bench-output.json
echo "==> wrote bench-output.json"
