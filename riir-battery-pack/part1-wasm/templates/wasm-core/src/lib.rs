use wasm_bindgen::prelude::*;

/// Sanity-check export: confirms the toolchain (wasm-pack, wasm-bindgen's
/// generated JS glue) is wired up end to end before writing real WASM
/// linear-memory code against it.
#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    format!("Hello, {name}, from Wasm!")
}
