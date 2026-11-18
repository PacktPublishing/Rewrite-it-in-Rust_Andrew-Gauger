//! A TCP proxy that adds latency, for testing Cairn against a slow
//! network on a fast machine.
//!
//! Every chunk read from one side is held for `--delay-ms` (plus up
//! to `--jitter-ms` more) before it is written to the other side.
//! Each direction has its own queue, so bytes leave in the order they
//! arrived, and a round trip costs twice the delay.
//!
//! ```text
//! cargo run -p cairn-app --bin dev-latency -- --delay-ms 300
//! ```

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::{sleep_until, Instant};

const USAGE: &str = "\
usage: dev-latency [--listen ADDR] [--upstream ADDR]
                   [--delay-ms N] [--jitter-ms N]

  --listen     address to accept on   (default 127.0.0.1:5434)
  --upstream   address to forward to  (default 127.0.0.1:5433)
  --delay-ms   delay added each way   (default 0)
  --jitter-ms  up to N ms more, random per chunk (default 0)";

#[derive(Clone, Copy)]
struct Delay {
    fixed: Duration,
    jitter_ms: u64,
}

impl Delay {
    fn next(&self) -> Duration {
        if self.jitter_ms == 0 {
            return self.fixed;
        }
        // A fresh RandomState is randomly seeded: enough for jitter.
        let r = RandomState::new().build_hasher().finish();
        self.fixed + Duration::from_millis(r % (self.jitter_ms + 1))
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let mut listen = "127.0.0.1:5434".to_string();
    let mut upstream = "127.0.0.1:5433".to_string();
    let mut delay = Delay {
        fixed: Duration::ZERO,
        jitter_ms: 0,
    };

    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == "--help" || flag == "-h" {
            println!("{USAGE}");
            return Ok(());
        }
        let Some(value) = args.next() else {
            eprintln!("{flag} needs a value\n\n{USAGE}");
            std::process::exit(2);
        };
        let ms = || {
            value.parse::<u64>().unwrap_or_else(|_| {
                eprintln!("{flag}: not a number: {value}");
                std::process::exit(2);
            })
        };
        match flag.as_str() {
            "--listen" => listen = value.clone(),
            "--upstream" => upstream = value.clone(),
            "--delay-ms" => delay.fixed = Duration::from_millis(ms()),
            "--jitter-ms" => delay.jitter_ms = ms(),
            _ => {
                eprintln!("unknown flag {flag}\n\n{USAGE}");
                std::process::exit(2);
            }
        }
    }

    let listener = TcpListener::bind(&listen).await?;
    println!(
        "dev-latency: {listen} -> {upstream}, +{} ms each way \
         (jitter up to {} ms)",
        delay.fixed.as_millis(),
        delay.jitter_ms
    );

    loop {
        let (client, peer) = listener.accept().await?;
        let upstream = upstream.clone();
        tokio::spawn(async move {
            match TcpStream::connect(&upstream).await {
                Ok(server) => {
                    let _ = client.set_nodelay(true);
                    let _ = server.set_nodelay(true);
                    let (cr, cw) = client.into_split();
                    let (sr, sw) = server.into_split();
                    tokio::join!(
                        pipe(cr, sw, delay),
                        pipe(sr, cw, delay)
                    );
                }
                Err(e) => eprintln!("{peer}: upstream: {e}"),
            }
        });
    }
}

/// Copy one direction. The reader stamps each chunk with the time it
/// may leave; the writer holds it until then. Reading never waits on
/// the delay, so a burst arrives as a burst, just later.
async fn pipe(
    mut from: OwnedReadHalf,
    mut to: OwnedWriteHalf,
    delay: Delay,
) {
    let (tx, mut rx) =
        mpsc::unbounded_channel::<(Instant, Vec<u8>)>();

    let reader = async move {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            match from.read(&mut buf).await {
                Ok(0) | Err(_) => break, // dropping tx ends the writer
                Ok(n) => {
                    let due = Instant::now() + delay.next();
                    if tx.send((due, buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
            }
        }
    };

    let writer = async move {
        while let Some((due, chunk)) = rx.recv().await {
            sleep_until(due).await;
            if to.write_all(&chunk).await.is_err() {
                return;
            }
        }
        let _ = to.shutdown().await; // pass the close along
    };

    tokio::join!(reader, writer);
}
