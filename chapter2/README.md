# Chapter 2: Choosing a Strategy, Finding the Bottleneck

`gil-demo/` is the "Under Load" benchmark: an arbitrary-precision Fibonacci calculation to n=300,000, run as one job, four jobs in series, and four jobs at once, in Python, Ruby, JavaScript and Rust.

```sh
cd gil-demo
python3 fib.py one; python3 fib.py serial4; python3 fib.py threaded4
cd rustfib && cargo build --release && /usr/bin/time -l ./target/release/rustfib threaded4
```

See [`gil-demo/README.md`](gil-demo/README.md) for the full set of runs and the toolchain versions behind the chapter's numbers.

## ARM optimization

On ARM64 machines such as Apple Silicon, the Rust build runs slower than Node on this benchmark. The cause is one character in `num-bigint`'s add-with-carry for non-x86 CPUs: `b || d` where `b | d` would do. `gil-demo/rustfib-arm/` is the same program linked against a copy of `num-bigint` 0.5.1 with that character changed. On an Apple M3 Pro it is about 1.8 times as fast as `rustfib`, and about 1.4 times as fast as Node. See [ARM optimization](gil-demo/README.md#arm-optimization) for the change, the machine code, the numbers, and where the slowdown came from.

```sh
cd gil-demo/rustfib-arm && cargo build --release && ./target/release/rustfib-arm one
```
