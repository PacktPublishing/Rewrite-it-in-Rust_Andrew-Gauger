#!/usr/bin/env bash
# End-to-end: regenerate the real dataset, (re)build the wasm from the
# current viz-core source, install the Playwright package if needed, run
# the benchmark. Writes bench-output.json.
#
#   ./run.sh            full run  (throttles 1x/4x/6x, both scenarios)
#   ./run.sh --quick    smoke test (4x only, fewer iterations)
#   ./run.sh --android --attempts 3
#                       Chrome on a USB-connected Android device, no
#                       throttle, mean of 3; writes bench-output-android.json
set -euo pipefail
cd "$(dirname "$0")"

mkdir -p fixtures wasm
echo "==> regenerating fixtures from the real SensorDataGenerator"
ruby gen-readings.rb cluster > fixtures/readings.cluster.json
ruby gen-readings.rb highway > fixtures/readings.highway.json

echo "==> building viz-core -> wasm"
if command -v wasm-pack >/dev/null 2>&1; then
  ( cd ../viz-core && wasm-pack build --target web --out-dir pkg --out-name viz_core --no-opt >/dev/null )
  cp ../viz-core/pkg/viz_core.js ../viz-core/pkg/viz_core_bg.wasm wasm/
elif command -v wasm-bindgen >/dev/null 2>&1; then
  # what `wasm-pack build --target web --no-opt` does: a release build,
  # then wasm-bindgen (its version must match the crate's dependency)
  echo "    wasm-pack not found; using cargo + wasm-bindgen"
  ( cd ../viz-core && cargo build --release --target wasm32-unknown-unknown --lib >/dev/null 2>&1 )
  wasm-bindgen --target web --out-dir wasm --out-name viz_core \
    ../viz-core/target/wasm32-unknown-unknown/release/viz_core.wasm
else
  echo "    neither wasm-pack nor wasm-bindgen found; falling back to the committed ../viz-core/pkg"
  cp ../viz-core/pkg/viz_core.js ../viz-core/pkg/viz_core_bg.wasm wasm/
fi

if [ ! -d node_modules ]; then
  echo "==> installing playwright (browser download skipped; uses system Chrome)"
  PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 npm install --no-audit --no-fund
fi

out=bench-output.json
case " $* " in *" --android "*) out=bench-output-android.json ;; esac
echo "==> running bench"
node bench.mjs "$@" > "$out.tmp" && mv "$out.tmp" "$out"
echo "==> wrote $out"
