//! Chapter 13: three ways to search the same rows, profiled. The scan
//! (`LookupList::rank`, the oracle), a `BTreeMap` of word prefixes
//! (`LookupIndex`), and a trigram index (`TrigramIndex`) that prunes
//! candidates and lets the scan's own `score` decide.
//!
//!   cargo test -q --release -p cairn-app --test lookup_profile -- --nocapture
//!   cargo test -q --release -p cairn-app --test lookup_profile -- --ignored --nocapture

use cairn_forms::{LookupIndex, LookupList, LookupRow, TrigramIndex};
use std::time::Instant;

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

/// The rows the app builds for products: the SKU, the name beside it,
/// and the rest of the record searched but never shown.
fn rows(count: usize) -> Vec<LookupRow> {
    let mut w = Words(7);
    (1..=count)
        .map(|n| {
            let description = (0..14)
                .map(|_| w.pick(&WORDS))
                .collect::<Vec<_>>()
                .join(" ");
            LookupRow {
                key: n.to_string(),
                text: format!("LI-{n:07}"),
                detail: format!(
                    "{} {}",
                    w.pick(&KINDS),
                    w.pick(&NOUNS)
                ),
                keywords: format!(
                    "{} {}.{}.0 {description}",
                    w.pick(&CATEGORIES),
                    1 + n % 3,
                    n % 10
                ),
            }
        })
        .collect()
}

/// What a rep might type. Under three bytes the trigram index has
/// nothing to look up; from three up it serves everything.
const QUERIES: [&str; 24] = [
    "LI-0050000",
    "LI-005000",
    "LI-0050",
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
    "gat",
    "ens",
    "ura",
    "wire",
    "LI-9999999",
];

fn ms(t: std::time::Duration) -> f64 {
    t.as_secs_f64() * 1000.0
}

fn keys(rows: Vec<&LookupRow>) -> Vec<String> {
    rows.iter().map(|r| r.key.clone()).collect()
}

fn median_worst(mut v: Vec<f64>) -> (f64, f64) {
    v.sort_by(|a, b| a.total_cmp(b));
    (v[v.len() / 2], *v.last().unwrap())
}

fn profile(count: usize) {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!("== {count} products ({profile})");
    let data = rows(count);

    let t = Instant::now();
    let list = LookupList::new(data.clone());
    println!("scan list built      {:>9.1} ms", ms(t.elapsed()));
    let t = Instant::now();
    let prefix = LookupIndex::from_rows(data.clone());
    println!("prefix index built   {:>9.1} ms", ms(t.elapsed()));
    let t = Instant::now();
    let mut tri = TrigramIndex::from_rows(data.clone());
    println!("trigram index built  {:>9.1} ms", ms(t.elapsed()));
    let s = tri.stats();
    println!(
        "trigram index: {}+{} distinct trigrams (text, other), {} postings, \
         longest list {}, posting lists {:.1} MB ({:.0} bytes a row)",
        s.text_trigrams,
        s.other_trigrams,
        s.postings,
        s.longest,
        s.posting_bytes as f64 / 1_048_576.0,
        s.posting_bytes as f64 / s.rows as f64
    );
    let extra = LookupRow {
        key: "x".into(),
        text: "LI-9999998".into(),
        detail: "Late Arrival".into(),
        keywords: "Sensor 1.0.0 added after the build".into(),
    };
    let t = Instant::now();
    tri.push(extra);
    println!("one product pushed   {:>9.4} ms", ms(t.elapsed()));
    // Put the table back to the rows the others hold.
    let tri = TrigramIndex::from_rows(data);

    println!(
        "{:<15} {:>8} {:>8} {:>8} | {:>8} {:>6} | {:>7} {:>7} | same",
        "query",
        "scan",
        "prefix",
        "trigram",
        "checked",
        "kept",
        "walk",
        "order"
    );
    let (mut scan, mut pre, mut gram) = (vec![], vec![], vec![]);
    let (mut same_tri, mut same_pre) = (0, 0);
    let (mut served, mut served_gram) = (vec![], vec![]);
    for q in QUERIES {
        let mut best = [f64::MAX; 3];
        for _ in 0..5 {
            let t = Instant::now();
            let _ = list.rank(q, 12);
            best[0] = best[0].min(ms(t.elapsed()));
            let t = Instant::now();
            let _ = prefix.rank(q, 12);
            best[1] = best[1].min(ms(t.elapsed()));
        }
        let mut best_profile = None;
        for _ in 0..5 {
            let t = Instant::now();
            let (_, p) = tri.rank_profiled(q, 12);
            let took = ms(t.elapsed());
            if took < best[2] {
                best[2] = took;
                best_profile = Some(p);
            }
        }
        let p = best_profile.unwrap();
        let want = keys(list.rank(q, 12));
        let tri_same = keys(tri.rank(q, 12)) == want;
        let pre_same = keys(prefix.rank(q, 12)) == want;
        same_tri += tri_same as usize;
        same_pre += pre_same as usize;
        let note = match (tri_same, p.fell_back, p.exited_early) {
            (false, _, _) => "NO",
            (_, true, true) => "yes, walk (<3 bytes), early",
            (_, true, false) => "yes, walk (<3 bytes)",
            (_, false, true) => "yes, early",
            _ => "yes",
        };
        println!(
            "{q:<15} {:>8.3} {:>8.3} {:>8.3} | {:>8} {:>6} | {:>7.3} {:>7.3} | {note}",
            best[0], best[1], best[2], p.checked, p.kept,
            ms(p.walk), ms(p.order)
        );
        scan.push(best[0]);
        pre.push(best[1]);
        gram.push(best[2]);
        if !p.fell_back {
            served.push(best[0]);
            served_gram.push(best[2]);
        }
    }
    let (sm, sw) = median_worst(scan);
    let (pm, pw) = median_worst(pre);
    let (gm, gw) = median_worst(gram);
    println!("scan    median {sm:.3} ms, worst {sw:.3} ms");
    println!("prefix  median {pm:.3} ms, worst {pw:.3} ms, same results on {same_pre} of {}", QUERIES.len());
    println!("trigram median {gm:.3} ms, worst {gw:.3} ms, same results on {same_tri} of {}", QUERIES.len());
    let (a, _) = median_worst(served);
    let (b, w) = median_worst(served_gram);
    println!(
        "queries of three bytes or more only: scan median {a:.3} ms, \
         trigram median {b:.3} ms, worst {w:.3} ms"
    );
    assert_eq!(
        same_tri,
        QUERIES.len(),
        "the trigram index must agree"
    );
}

#[test]
fn ten_thousand_and_a_hundred_thousand() {
    profile(10_000);
    profile(100_000);
}

#[test]
#[ignore = "builds three copies of a million rows: release only"]
fn one_million() {
    profile(1_000_000);
}

/// Where does plain scanning stop being enough? The same lookup at
/// growing sizes: the scan against the trigram index with early exit,
/// the worst of 24 queries each. Two budgets: the mission's 100 ms,
/// and one frame (16.7 ms at 60 frames a second), because the popup
/// ranks inside the frame that draws it.
///
///   cargo test -q --release -p cairn-app --test lookup_profile -- --ignored --nocapture sweep
#[test]
#[ignore = "builds lists up to a million rows: release only"]
fn sweep() {
    let sizes = [
        10_000, 25_000, 50_000, 100_000, 200_000, 300_000, 500_000,
        750_000, 1_000_000,
    ];
    println!(
        "{:>9} | {:>9} {:>9} | {:>9} {:>9} | {:>9}",
        "products",
        "scan med",
        "scan worst",
        "tri med",
        "tri worst",
        "tri build"
    );
    let mut curve = Vec::new();
    for count in sizes {
        let data = rows(count);
        let list = LookupList::new(data.clone());
        let t = Instant::now();
        let tri = TrigramIndex::from_rows(data);
        let built = ms(t.elapsed());
        let (mut a, mut b) = (vec![], vec![]);
        for q in QUERIES {
            let (mut sa, mut sb) = (f64::MAX, f64::MAX);
            for _ in 0..5 {
                let t = Instant::now();
                let _ = list.rank(q, 12);
                sa = sa.min(ms(t.elapsed()));
                let t = Instant::now();
                let _ = tri.rank(q, 12);
                sb = sb.min(ms(t.elapsed()));
            }
            a.push(sa);
            b.push(sb);
            assert_eq!(
                keys(list.rank(q, 12)),
                keys(tri.rank(q, 12)),
                "{q:?} at {count}"
            );
        }
        let (am, aw) = median_worst(a);
        let (bm, bw) = median_worst(b);
        println!(
            "{count:>9} | {am:>9.3} {aw:>9.3} | {bm:>9.3} {bw:>9.3} | {built:>7.0} ms"
        );
        curve.push((count, aw, bw));
    }
    for budget in [16.7, 100.0] {
        let crossing = |pick: fn(&(usize, f64, f64)) -> f64| {
            curve
                .windows(2)
                .find(|w| {
                    pick(&w[0]) <= budget && pick(&w[1]) > budget
                })
                .map(|w| {
                    let (x0, y0) = (w[0].0 as f64, pick(&w[0]));
                    let (x1, y1) = (w[1].0 as f64, pick(&w[1]));
                    (x0 + (budget - y0) / (y1 - y0) * (x1 - x0))
                        as usize
                })
        };
        println!(
            "worst case passes {budget} ms: scan at about {:?} products, trigram at {:?}",
            crossing(|c| c.1),
            crossing(|c| c.2)
        );
    }
}

/// Chapter 13: the same rows and the scan's answers, written out so a
/// Postgres `pg_trgm` index can be held to them. `rows.tsv` is key,
/// text, detail, keywords; `expected.tsv` is each query and the keys
/// `rank(q, 12)` returns, in order.
///
///   PGTRGM_DIR=/tmp/pgtrgm cargo test -q --release -p cairn-app --test lookup_profile -- --ignored dump_for_postgres
#[test]
#[ignore = "writes files under $PGTRGM_DIR"]
fn dump_for_postgres() {
    use std::io::Write;
    let dir = std::env::var("PGTRGM_DIR").expect("set PGTRGM_DIR");
    let data = rows(100_000);
    let mut f =
        std::fs::File::create(format!("{dir}/rows.tsv")).unwrap();
    for r in &data {
        writeln!(f, "{}\t{}\t{}\t{}", r.key, r.text, r.detail, r.keywords)
            .unwrap();
    }
    let list = LookupList::new(data);
    let mut e =
        std::fs::File::create(format!("{dir}/expected.tsv")).unwrap();
    for q in QUERIES {
        let k = keys(list.rank(q, 12)).join(",");
        writeln!(e, "{q}\t{k}").unwrap();
    }
}
