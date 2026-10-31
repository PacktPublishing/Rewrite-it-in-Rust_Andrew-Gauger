//! Wired for Chapter 8 (2026-10-30). Not part of the AI's output.
//!
//! Conversions between the types the AI's tests assumed and cairn-core's,
//! plus a reader for the History database where the AI assumed a lookup
//! (V1's DLookup/DSum). No business rule lives here: every answer the tests
//! check comes from cairn-core.
#![allow(dead_code)]

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;

pub fn dec(x: f64) -> Decimal {
    Decimal::from_str(&x.to_string()).expect("a finite f64")
}

pub fn f64_of(d: Decimal) -> f64 {
    d.to_string().parse().expect("a Decimal prints as an f64")
}

pub fn date(d: NaiveDate) -> cairn_core::Date {
    cairn_core::Date::new(d.year(), d.month(), d.day()).expect("a real date")
}

/// cairn-core's route text, as the `&'static str` the AI's signature wants.
pub fn route_text(r: cairn_core::Route) -> &'static str {
    Box::leak(r.to_string().into_boxed_str())
}

/// The AI's input can't be built as a cairn-core value. The test fails
/// here, and the run's README counts it apart from wrong answers.
pub fn not_representable(why: &str) -> ! {
    panic!("not representable in cairn-core: {why}")
}

/// One table of the History database, keyed by its first column.
pub fn table(name: &str) -> HashMap<i32, HashMap<String, String>> {
    let db = std::env::var("CAIRN_DB").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../part 2/CairnSystems_History.accdb").to_string()
    });
    let out = std::process::Command::new("mdb-export")
        .args(["-Q", "-d", "\x1f", "-R", "\x1e", &db, name])
        .output()
        .expect("mdb-export must be installed");
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.split('\x1e').filter(|l| !l.is_empty());
    let header: Vec<String> = lines.next().unwrap().split('\x1f').map(str::to_string).collect();
    lines
        .map(|line| {
            let row: HashMap<String, String> =
                header.iter().cloned().zip(line.split('\x1f').map(str::to_string)).collect();
            (row[&header[0]].parse().unwrap(), row)
        })
        .collect()
}

/// mdb-export's `MM/DD/YY HH:MM:SS`.
pub fn v1_date(s: &str) -> cairn_core::Date {
    cairn_core::Date::new(2000 + s[6..8].parse::<i32>().unwrap(), s[0..2].parse().unwrap(), s[3..5].parse().unwrap())
        .unwrap()
}

/// V1's DLookup("DefaultTaxRate", "CompanyInfo").
pub fn company() -> cairn_core::CompanyInfo {
    let t = table("CompanyInfo");
    let row = t.values().next().expect("one CompanyInfo row");
    cairn_core::CompanyInfo { default_tax_rate: row["DefaultTaxRate"].parse().unwrap() }
}
