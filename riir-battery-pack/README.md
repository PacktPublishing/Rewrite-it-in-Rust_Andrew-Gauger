# riir battery pack

Three [battery packs](https://battery-pack-rs.github.io/battery-pack/),
one per hardware target *Rewrite It in Rust* rewrites onto, in the order
the book covers them. Each is its own crate with its own `Cargo.toml`,
since the three platforms don't share a target triple, let alone a
dependency graph.

| Part | Directory | Pack name | Book section |
|---|---|---|---|
| 1 -- WASM | [`part1-wasm/`](part1-wasm/) | `riir-wasm` | Part I: The Dashboard (Ch. 3-5) |
| 2 -- GUI | [`part2-gui/`](part2-gui/) | `riir-gui` | Part II: The Database |
| 3 -- Embassy/STM32 | [`part3-embassy/`](part3-embassy/) | `riir-embassy` | Part III: The Devices |

```sh
cargo bp add riir-wasm      # Part 1: wasm-bindgen + js-sys
cargo bp add riir-gui       # Part 2: egui + eframe + sqlx + tokio
cargo bp add riir-embassy   # Part 3: embassy-executor + embassy-stm32 + defmt
```

See each part's own `README.md` for its full crate list, feature flags,
and the reasoning behind what's included and what's deliberately left out.
