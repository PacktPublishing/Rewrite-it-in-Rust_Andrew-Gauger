//! Chapter 13: the product lookup at 10,000 products. The list is a
//! cache in the app, so a keystroke is `rank` over rows already in
//! memory. No database, no network, no runtime. The mission's bar is
//! 100 ms.
//!
//!   cargo test --release -p cairn-app --test lookup_timing -- --nocapture

mod common;

use cairn_app::context::AppContext;
use cairn_app::forms::ProductDraft;
use cairn_core::ProductId;
use cairn_forms::{LookupIndex, LookupRow};
use rust_decimal::Decimal;
use std::time::Instant;

const PRODUCTS: i32 = 10_000;

/// The same words every run: a fixed generator, no `rand`.
struct Words(u64);

impl Words {
    fn pick<'a>(&mut self, list: &[&'a str]) -> &'a str {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        list[(self.0 >> 33) as usize % list.len()]
    }
}

const NOUNS: [&str; 12] = [
    "Sensor",
    "Gateway",
    "Probe",
    "Module",
    "Logger",
    "Antenna",
    "Relay",
    "Bracket",
    "Cable",
    "Enclosure",
    "Display",
    "Charger",
];
const KINDS: [&str; 10] = [
    "Temperature",
    "Humidity",
    "Pressure",
    "Vibration",
    "LoRaWAN",
    "Wired",
    "Solar",
    "Outdoor",
    "Compact",
    "Industrial",
];
const CATEGORIES: [&str; 6] =
    ["Sensor", "Gateway", "Module", "Accessory", "Service", "Kit"];
const WORDS: [&str; 16] = [
    "calibrated",
    "rugged",
    "battery",
    "powered",
    "reports",
    "every",
    "minute",
    "over",
    "uplink",
    "mounts",
    "on",
    "a",
    "pipe",
    "rail",
    "with",
    "alarm",
];

fn product(n: i32, w: &mut Words) -> ProductDraft {
    let mut p = common::product(
        n,
        &format!("LI-{n:05}"),
        &format!("{} {}", w.pick(&KINDS), w.pick(&NOUNS)),
        7900,
    );
    p.category = w.pick(&CATEGORIES).into();
    p.firmware = format!("{}.{}.0", 1 + n % 3, n % 10);
    p.description = (0..14)
        .map(|_| w.pick(&WORDS))
        .collect::<Vec<_>>()
        .join(" ");
    p.unit_price = Decimal::new(100 + i64::from(n), 2);
    p.id = Some(ProductId(n));
    p
}

/// What a rep might type: an exact SKU, a SKU prefix, the start of a
/// name, a word inside one, a word only the description has, a
/// category, a firmware version, one letter, and a miss.
const QUERIES: [&str; 20] = [
    "LI-05000",
    "LI-0500",
    "LI-05",
    "li-",
    "temp",
    "tempe",
    "sensor",
    "ateway",
    "lorawan sensor",
    "calibrated",
    "uplink",
    "rail",
    "Accessory",
    "2.4",
    "s",
    "te",
    "a",
    "pipe rail",
    "zzzzq",
    "LI-9999",
];

#[test]
fn ranking_ten_thousand_products_takes_milliseconds() {
    let mut data = common::snapshot();
    let mut w = Words(7);
    data.products =
        (1..=PRODUCTS).map(|n| product(n, &mut w)).collect();

    let t = Instant::now();
    let ctx = AppContext::new(data);
    let build = t.elapsed();
    let list = ctx.lookup_list("products").expect("a products list");
    assert_eq!(list.rows.len(), PRODUCTS as usize);

    let mut ms: Vec<(f64, &str, usize)> = QUERIES
        .iter()
        .map(|q| {
            // Best of five: the machine's noise is not the lookup's.
            let mut best = f64::MAX;
            let mut found = 0;
            for _ in 0..5 {
                let t = Instant::now();
                let rows = list.rank(q, 12);
                best = best.min(t.elapsed().as_secs_f64() * 1000.0);
                found = rows.len();
            }
            (best, *q, found)
        })
        .collect();
    ms.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (t, q, found) in &ms {
        println!("{t:8.3} ms  {q:<16} {found} shown");
    }
    let median = ms[ms.len() / 2].0;
    let worst = ms.last().unwrap().0;
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!(
        "{PRODUCTS} products ({profile}): rows built in {:.1} ms, \
         median {median:.3} ms, worst {worst:.3} ms",
        build.as_secs_f64() * 1000.0
    );
    assert!(worst < 100.0, "worst {worst} ms is over the 100 ms bar");
}

/// Best of five, in milliseconds, with what it found.
fn best_of_five<'a>(
    mut run: impl FnMut() -> Vec<&'a LookupRow>,
) -> (f64, Vec<String>) {
    let mut best = f64::MAX;
    let mut keys = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        let rows = run();
        best = best.min(t.elapsed().as_secs_f64() * 1000.0);
        keys = rows.iter().map(|r| r.key.clone()).collect();
    }
    (best, keys)
}

/// The same lookup over the same rows two ways: `rank`, which reads
/// every row, and a `BTreeMap` of search terms, where a prefix is one
/// range scan. At 10,000 products and at ten times that.
#[test]
fn a_btree_index_against_the_scan() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    for count in [10_000, 100_000] {
        let mut data = common::snapshot();
        let mut w = Words(7);
        data.products =
            (1..=count).map(|n| product(n, &mut w)).collect();
        let ctx = AppContext::new(data);
        let list = ctx.lookup_list("products").unwrap();

        let t = Instant::now();
        let mut index = LookupIndex::from_rows(list.rows.clone());
        let build = t.elapsed().as_secs_f64() * 1000.0;

        // One new product: the whole cost of adding it.
        let extra = LookupRow {
            key: "999999".into(),
            text: "LI-99999".into(),
            detail: "Late Arrival".into(),
            keywords: "Sensor 1.0.0 added after the build".into(),
        };
        let t = Instant::now();
        index.push(extra);
        let push = t.elapsed().as_secs_f64() * 1000.0;
        // The scan's version of the same news: rebuild the lookups.
        let t = Instant::now();
        let _ = AppContext::new(ctx.data.clone());
        let rebuild = t.elapsed().as_secs_f64() * 1000.0;
        // Take the extra row out again: queries compare the same rows.
        let index = LookupIndex::from_rows(list.rows.clone());

        println!("{count} products ({profile})");
        println!("  index built in {build:.1} ms; one product added in {push:.4} ms; lookups rebuilt in {rebuild:.1} ms");
        println!(
            "  {:<16} {:>10} {:>10}  same results",
            "query", "scan ms", "index ms"
        );
        let (mut scan_ms, mut index_ms, mut differ) =
            (Vec::new(), Vec::new(), Vec::new());
        for q in QUERIES {
            let (a, scan_keys) = best_of_five(|| list.rank(q, 12));
            let (b, index_keys) = best_of_five(|| index.rank(q, 12));
            let same = scan_keys == index_keys;
            println!(
                "  {q:<16} {a:>10.3} {b:>10.3}  {}",
                if same { "yes" } else { "NO" }
            );
            scan_ms.push(a);
            index_ms.push(b);
            if !same {
                differ.push(q);
            }
        }
        let stats = |v: &mut Vec<f64>| {
            v.sort_by(|a, b| a.total_cmp(b));
            (v[v.len() / 2], *v.last().unwrap())
        };
        let (sm, sw) = stats(&mut scan_ms);
        let (im, iw) = stats(&mut index_ms);
        println!("  scan  : median {sm:.3} ms, worst {sw:.3} ms");
        println!("  index : median {im:.3} ms, worst {iw:.3} ms");
        println!(
            "  the index differs on {} of {}: {differ:?}",
            differ.len(),
            QUERIES.len()
        );
    }
}
