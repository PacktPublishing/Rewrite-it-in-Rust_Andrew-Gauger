//! The scan is the oracle. `LookupList::rank` defines the answer; the
//! trigram index is only allowed to reach it faster. For thousands of
//! generated queries the two must return the same keys in the same
//! order. A failure here is a bug in the index, never in the scan.
//!
//!   cargo test -p cairn-forms --test trigram_oracle -- --nocapture

use cairn_forms::{LookupList, LookupRow, TrigramIndex};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: usize) -> usize {
        self.next() as usize % n
    }
    fn pick<'a>(&mut self, list: &[&'a str]) -> &'a str {
        list[self.below(list.len())]
    }
}

const NOUNS: [&str; 10] = [
    "Sensor",
    "Gateway",
    "Probe",
    "Module",
    "Logger",
    "Antenna",
    "Relay",
    "Café",
    "Zürich",
    "İstanbul",
];
const KINDS: [&str; 8] = [
    "Temperature",
    "Humidity",
    "Pressure",
    "LoRaWAN",
    "Wired",
    "Solar",
    "Outdoor",
    "Industrial",
];
const WORDS: [&str; 12] = [
    "calibrated",
    "rugged",
    "battery",
    "uplink",
    "mounts",
    "on",
    "a",
    "pipe",
    "rail",
    "with",
    "alarm",
    "naïve",
];

fn rows(n: usize, rng: &mut Rng) -> Vec<LookupRow> {
    (0..n)
        .map(|i| LookupRow {
            key: i.to_string(),
            // Shapes that match "cream" at every tier: it starts the
            // text, starts a word, or is inside one.
            text: match rng.below(5) {
                0 => format!("LI-{i:05}"),
                1 => format!("Cream Co {i}"),
                2 => format!("Shasta Cream {i}"),
                3 => format!("Icecream Plant {i}"),
                _ => format!("Creamery {i}"),
            },
            detail: format!(
                "{} {}",
                rng.pick(&KINDS),
                rng.pick(&NOUNS)
            ),
            keywords: (0..1 + rng.below(8))
                .map(|_| rng.pick(&WORDS))
                .collect::<Vec<_>>()
                .join(" "),
        })
        .collect()
}

/// A query a rep might type, or might not: a piece of a real field of
/// a real row (so most queries match something), at a random length,
/// with letters flipped to upper case, padded with spaces; or noise.
fn query(rows: &[LookupRow], rng: &mut Rng) -> String {
    if rng.below(10) == 0 {
        let alphabet: Vec<char> = "abcxyz -.0159é".chars().collect();
        return (0..1 + rng.below(6))
            .map(|_| alphabet[rng.below(alphabet.len())])
            .collect();
    }
    let row = &rows[rng.below(rows.len())];
    let field = match rng.below(3) {
        0 => &row.text,
        1 => &row.detail,
        _ => &row.keywords,
    };
    let chars: Vec<char> = field.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let start = rng.below(chars.len());
    let len = 1 + rng.below(14);
    let piece: String = chars[start..].iter().take(len).collect();
    let piece: String = piece
        .chars()
        .map(|c| {
            if rng.below(4) == 0 {
                c.to_uppercase().next().unwrap_or(c)
            } else {
                c
            }
        })
        .collect();
    match rng.below(5) {
        0 => format!("  {piece}"),
        1 => format!("{piece}  "),
        _ => piece,
    }
}

fn keys(rows: Vec<&LookupRow>) -> Vec<String> {
    rows.iter().map(|r| r.key.clone()).collect()
}

#[test]
fn the_index_returns_what_the_scan_returns() {
    let mut rng = Rng(11);
    let data = rows(3_000, &mut rng);
    let list = LookupList::new(data.clone());
    let index = TrigramIndex::from_rows(data.clone());

    let (mut served, mut fell_back, mut empty) = (0, 0, 0);
    for n in 0..2_000 {
        let q = query(&data, &mut rng);
        for limit in [1, 12, 1_000] {
            let want = keys(list.rank(&q, limit));
            let (got, profile) = index.rank_profiled(&q, limit);
            assert_eq!(keys(got), want, "query {q:?}, limit {limit}");
            if limit == 12 && n < 2_000 {
                if q.trim().is_empty() {
                    empty += 1;
                } else if profile.fell_back {
                    fell_back += 1;
                } else {
                    served += 1;
                }
            }
        }
    }
    println!(
        "2,000 queries x 3 limits agree: {served} served by the index, \
         {fell_back} under three bytes fell back to the scan, \
         {empty} empty"
    );
    assert!(served > 1_000, "the test should mostly use the index");
}

#[test]
fn a_pushed_row_is_the_same_as_a_row_built_in() {
    let mut rng = Rng(5);
    let data = rows(2_000, &mut rng);
    let (first, later) = data.split_at(1_500);
    let built = TrigramIndex::from_rows(data.clone());
    let mut pushed = TrigramIndex::from_rows(first.to_vec());
    for row in later {
        pushed.push(row.clone());
    }
    let list = LookupList::new(data.clone());
    for _ in 0..1_000 {
        let q = query(&data, &mut rng);
        let want = keys(list.rank(&q, 12));
        assert_eq!(keys(built.rank(&q, 12)), want, "built, {q:?}");
        assert_eq!(keys(pushed.rank(&q, 12)), want, "pushed, {q:?}");
    }
}

#[test]
fn it_finds_the_middle_of_a_word_which_the_prefix_index_cannot() {
    let data = vec![LookupRow {
        key: "1".into(),
        text: "CS-GW200".into(),
        detail: "LoRaWAN Gateway".into(),
        keywords: String::new(),
    }];
    let index = TrigramIndex::from_rows(data);
    assert_eq!(keys(index.rank("ateway", 9)), ["1"]);
    assert_eq!(keys(index.rank("raw", 9)), ["1"]);
    assert!(index.rank("zzz", 9).is_empty());
}

/// The point of the early exit: a query that matches most rows is
/// answered after checking about as many rows as the list holds.
#[test]
fn a_common_query_stops_once_the_list_is_full() {
    let mut rng = Rng(3);
    let data = rows(5_000, &mut rng);
    let list = LookupList::new(data.clone());
    let index = TrigramIndex::from_rows(data);
    for (q, limit) in [("li-", 12), ("sensor", 12), ("rugged", 12)] {
        let (got, p) = index.rank_profiled(q, limit);
        assert_eq!(keys(got), keys(list.rank(q, limit)), "{q:?}");
        println!(
            "{q:?}: checked {} of 5,000 rows, kept {}, early {}",
            p.checked, p.kept, p.exited_early
        );
        assert!(p.exited_early, "{q:?} should stop early");
        assert!(p.checked < 1_000, "{q:?} checked {}", p.checked);
    }
}
