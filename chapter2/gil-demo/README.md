# gil-demo

Source for the "Under Load" benchmark in `chapter2/6-under-load.md`: an
iterative, arbitrary-precision Fibonacci calculation to n=300,000, run as
1 job, 4 jobs serial, and 4 jobs concurrent, in four languages.

Originally run July 2026 (see `v1/RESEARCH.md`'s "Interpreter-lock demo, 4
languages" entry), but the source files from that session were never
committed. Re-run and re-verified 2026-08-21 on the same machine/toolchain
(Python 3.14.3, Ruby 3.4.1, Node v23.4.0, Rust 1.95, 8-core), producing
numbers within run-to-run noise of the original. These files and the numbers
in the chapter come from that re-run, not the original session.

## Running it

```sh
python3 fib.py one          # or serial4 / threaded4
ruby fib.rb one
node fib.js one
cd rustfib && cargo build --release && ./target/release/rustfib one
```

Memory follow-up (peak resident set size during the `threaded4` run):

```sh
/usr/bin/time -l python3 fib.py threaded4
/usr/bin/time -l ruby fib.rb threaded4
/usr/bin/time -l node fib.js threaded4
/usr/bin/time -l ./rustfib/target/release/rustfib threaded4
```

`/usr/bin/time -l` is macOS-specific; use `/usr/bin/time -v` (GNU time) on
Linux for the equivalent `Maximum resident set size` line.

## ARM optimization

On ARM64 (Apple Silicon, AWS Graviton, Ampere), `rustfib` loses to Node. The
Rust code isn't at fault. It's one character in the `num-bigint` crate it
uses.

`num-bigint` adds big numbers one 64-bit digit at a time, passing a carry from
each digit to the next. On x86 it uses the CPU's add-with-carry intrinsic. On
every other architecture it uses this fallback:

```rust
fn adc(carry: u8, lhs: BigDigit, rhs: BigDigit, out: &mut BigDigit) -> u8 {
    let (a, b) = lhs.overflowing_add(rhs);
    let (c, d) = a.overflowing_add(carry as BigDigit);
    *out = c;
    u8::from(b || d)
}
```

`rustfib-arm/` is the same program (its `main.rs` is identical to
`rustfib/`'s) linked against `num-bigint-arm/`, a copy of `num-bigint` 0.5.1
with that last line changed from `||` to `|`:

```rust
    u8::from(b | d)
```

That is the only difference from the published crate. It's in
`num-bigint-arm/src/biguint/addition.rs`. x86 is unaffected, because it never
uses the fallback.

```sh
cd rustfib-arm && cargo build --release && ./target/release/rustfib-arm one
```

### Why one character matters

Each digit's addition can't finish until the previous digit's carry is
known, so the cost per digit is set by the chain of instructions the carry
passes through. These are the per-digit loops of `BigUint` addition from the
two release builds on an Apple M3 Pro.

With `||`, the carry goes through `adds`, `cset`, `orr` and `and` before the
next digit can use it. That's four dependent instructions:

```
ldr   x14, [x11]
ldr   x15, [x21], #0x8
adds  x14, x15, x14
cset  w15, hs
adds  x12, x14, x12        ; carry in
cset  w14, hs
str   x12, [x11], #0x8
orr   w14, w15, w14
and   x12, x14, #0x1       ; carry out
subs  x13, x13, #0x1
b.ne  loop
```

With `|`, the two digits are added independently of the carry, and the carry
goes through only `adds` and `csinc`:

```
ldr   x14, [x11]
ldr   x15, [x21], #0x8
adds  x14, x15, x14
cset  w15, hs
adds  x12, x14, x12        ; carry in
str   x12, [x11], #0x8
csinc w12, w15, wzr, lo    ; carry out
subs  x13, x13, #0x1
b.ne  loop
```

With `||`, the compiler turns each overflow flag into its own 0-or-1 value
(the two `cset`s), ORs them, and masks the result back to one bit. With `|`,
it folds the second flag straight into a single conditional increment
(`csinc`).

### Results

Apple M3 Pro (12 cores), macOS, Rust 1.95.0, Node v23.4.0, 2026-10-31.
Seconds, three runs each:

| Mode | `rustfib` | `rustfib-arm` | Node |
|---|---|---|---|
| `one` | 0.533 0.529 0.530 | 0.311 0.297 0.292 | 0.421 0.412 0.413 |
| `serial4` | 2.126 2.118 2.117 | 1.171 1.181 1.165 | 1.637 1.637 1.633 |
| `threaded4` | 0.591 0.591 0.590 | 0.327 0.328 0.331 | 0.509 0.493 0.494 |

`rustfib-arm` is about 1.8 times as fast as `rustfib` in every mode. Where
`rustfib` lost to Node, `rustfib-arm` beats it: about 1.4 times as fast on one
job and 1.5 times as fast on four threads. Peak resident set during
`threaded4` is 4.1 MB, against Node's 218 MB.

Both builds compute the same F(300,000): 208,272 bits and 62,696 decimal
digits, beginning `87617325329163457942`. `num-bigint`'s own test suite (202
tests) passes on `num-bigint-arm`. The change still builds for x86_64 and
wasm32, and uses nothing newer than the crate's minimum Rust version, 1.60.

### Where the slowdown came from

`num-bigint` 0.4.5 and earlier computed the fallback carry by widening each
digit to 128 bits. [PR #308](https://github.com/rust-num/num-bigint/pull/308)
replaced that in 0.4.6 with the version above, copied from the standard
library's `carrying_add`, to avoid the widening. Every release since carries
it, including 0.5.1. The unmodified `rustfib`, built against each version on
the same M3 Pro, measures the difference:

| `num-bigint` | `one` | `threaded4` |
|---|---|---|
| 0.4.5 (widening) | 0.437 0.419 0.421 | 0.470 0.470 0.469 |
| 0.4.6 (`\|\|`) | 0.550 0.531 0.531 | 0.598 0.598 0.593 |
| 0.5.1 with `\|` (`rustfib-arm`) | 0.311 0.297 0.292 | 0.327 0.328 0.331 |

So `|` beats both earlier versions. Calling the standard library's
`u64::carrying_add` directly measures the same as `|` (0.291 seconds on one
job), but it needs a newer Rust than `num-bigint` supports.

The chapter's numbers were measured with the unmodified `rustfib`, which is
the code the chapter prints.
