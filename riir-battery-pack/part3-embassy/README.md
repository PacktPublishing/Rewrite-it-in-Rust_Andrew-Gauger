# riir-embassy-battery-pack

Part 3 of the [riir battery pack](../README.md) -- `no_std` async firmware
on ARM Cortex-M STM32 with the Embassy runtime, the crate curation behind
Part III of *Rewrite It in Rust*: The Devices (a fleet of sensors moving
off C onto Rust without a debugger attached in the field).

## Quick Start

```sh
cargo bp new stm32-core my-firmware
```

Add it to an existing crate instead:

```sh
cargo bp add riir-embassy
```

Need async delays/timeouts driven by the executor instead of a busy-wait
loop?

```sh
cargo bp add riir-embassy -F time
```

**You still have to pick your chip.** `embassy-stm32` is generic across
the whole STM32 line; a real firmware crate enables exactly one
chip-specific feature (e.g. `stm32f411ce`) plus a matching
`time-driver-*` feature on top of what this pack adds. That choice is the
project's, not this pack's -- see the
[Embassy book's STM32 setup chapter](https://embassy.dev/book/) for the
current feature name for your part number.

## Why these crates, and not others

`embassy-executor` + `embassy-stm32` are the required pair: Embassy's
async executor as the task model (suspending the CPU between readings
instead of a busy-wait loop keeping it awake), running on the
chip-specific HAL that actually owns the STM32's peripherals.
`embassy-time` (`time`) is opt-in rather than default because not every
firmware needs software timers/delays beyond what the HAL already gives
peripheral drivers directly -- but most will want it.

`heapless` replaces `Vec` and friends with fixed-capacity collections:
there is no allocator by default on this target, and a fleet device
bricked by an unexpected heap exhaustion is exactly the failure class this
rewrite exists to close off. `critical-section` replaces the C original's
raw `volatile` globals with an interrupt-safe primitive for state actually
shared between an interrupt handler and the main task. `embedded-hal`
supplies the portable async traits (GPIO, SPI, I2C, ...) a driver crate
codes against, instead of each HAL inventing its own.

`defmt` + `defmt-rtt` + `panic-probe` are the debugging trio: `defmt` is
the wire format, `defmt-rtt` carries it over RTT instead of a UART
println (no wire, no baud rate to get wrong, works over the same
`probe-rs` session already flashing the chip), and `panic-probe` routes an
unhandled panic through the same `defmt` channel instead of silently
resetting the device. All three are default, not opt-in: a firmware
image with no way to see its own panic is the exact hazard this Part
of the book is about closing.

**Not curated here:** `probe-rs` itself. It is the flashing/debugging CLI
this pack's logging trio talks to, invoked from the command line or a
`.cargo/config.toml` runner, not a dependency the firmware crate links
against.

## License

Licensed under either of:

- [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
- [MIT license](https://opensource.org/license/mit)

at your option.
