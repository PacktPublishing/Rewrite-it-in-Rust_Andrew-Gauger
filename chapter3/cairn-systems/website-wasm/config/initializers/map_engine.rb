# Feature flag: which frontend renders the map. The Rails app, its data,
# and its deploy process are identical either way -- this only decides
# whether the browser loads app/javascript/legacy_engine.js or the
# WASM-backed app/javascript/wasm_engine.js.
Rails.application.config.x.map_engine = ENV.fetch("MAP_ENGINE", "legacy")
