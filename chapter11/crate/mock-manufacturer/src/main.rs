use mock_manufacturer::{for_times, log, serve, Mode, Settings};
use std::path::PathBuf;
use std::time::Duration;

const USAGE: &str = "\
usage: mock-manufacturer [--port 8765] [--mode normal|drop|slow|error]
                         [--times N] [--delay-sec 20]
                         [--ledger ledger.json]
                         [--first-order-number 200001]";

struct Args {
    port: u16,
    settings: Settings,
}

fn parse(
    mut args: impl Iterator<Item = String>,
) -> Result<Args, String> {
    let mut a = Args {
        port: 8765,
        settings: Settings {
            mode: Mode::Normal,
            times: 0,
            delay: Duration::from_secs(20),
            ledger: PathBuf::from("ledger.json"),
            first_order_number: 200001,
        },
    };
    while let Some(flag) = args.next() {
        let value =
            args.next().ok_or(format!("{flag} needs a value"))?;
        let bad = || format!("{flag}: '{value}' is not valid");
        let s = &mut a.settings;
        match flag.as_str() {
            "--port" => a.port = value.parse().map_err(|_| bad())?,
            "--mode" => {
                s.mode = Mode::parse(&value).ok_or_else(bad)?
            }
            "--times" => {
                s.times = value.parse().map_err(|_| bad())?
            }
            "--delay-sec" => {
                let secs = value.parse().map_err(|_| bad())?;
                s.delay = Duration::from_secs(secs);
            }
            "--ledger" => s.ledger = PathBuf::from(value),
            "--first-order-number" => {
                s.first_order_number =
                    value.parse().map_err(|_| bad())?;
            }
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    Ok(a)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = match parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            std::process::exit(2);
        }
    };
    let s = args.settings;
    let listener =
        tokio::net::TcpListener::bind(("127.0.0.1", args.port))
            .await?;
    println!(
        "Mock manufacturer API on http://localhost:{}/  \
         (mode: {}{}, ledger: {})",
        args.port,
        s.mode.name(),
        for_times(s.times),
        s.ledger.display()
    );
    println!("POST /control?mode=stop or Ctrl+C to stop.");
    serve(listener, s).await?;
    log("Mock manufacturer API stopped.");
    Ok(())
}
