use num_bigint::BigUint;
use std::env;
use std::thread;
use std::time::Instant;

const N: u32 = 300_000;

fn fib(n: u32) -> BigUint {
    let mut a = BigUint::from(0u32);
    let mut b = BigUint::from(1u32);
    for _ in 0..n {
        let next = &a + &b;
        a = b;
        b = next;
    }
    a
}

fn bench_serial(jobs: u32) -> f64 {
    let start = Instant::now();
    for _ in 0..jobs {
        fib(N);
    }
    start.elapsed().as_secs_f64()
}

fn bench_threaded(jobs: u32) -> f64 {
    let start = Instant::now();
    let handles: Vec<_> = (0..jobs).map(|_| thread::spawn(|| fib(N))).collect();
    for h in handles {
        h.join().unwrap();
    }
    start.elapsed().as_secs_f64()
}

fn main() {
    let mode = env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "one" => {
            let start = Instant::now();
            fib(N);
            println!("{:.3}", start.elapsed().as_secs_f64());
        }
        "serial4" => println!("{:.3}", bench_serial(4)),
        "threaded4" => println!("{:.3}", bench_threaded(4)),
        _ => eprintln!("usage: rustfib <one|serial4|threaded4>"),
    }
}
