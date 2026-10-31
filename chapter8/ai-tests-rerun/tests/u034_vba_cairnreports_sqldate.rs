// Tests for vba:CairnReports.SqlDate (CairnReports L512-514)
//
//   Private Function SqlDate(ByVal d As Variant) As String
//       SqlDate = Format(d, "\#yyyy\-mm\-dd hh\:nn\:ss\#")
//   End Function
//
// The backslashes escape #, -, : to literals, so the output is the value
// wrapped in #...# delimiters:  #<yyyy>-<mm>-<dd> <hh>:<nn>:<ss>#
//   yyyy = 4-digit year, mm = 2-digit month, dd = 2-digit day,
//   hh   = 2-digit 24-hour hour (00-23), nn = 2-digit minute, ss = 2-digit second.
//
// We assume the system exposes:  fn sql_date(d: NaiveDateTime) -> String
// (VBA Date == date-time without timezone -> chrono NaiveDateTime).

use chrono::NaiveDateTime;

fn dt(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
}

// Rule 2: every sub-month/sub-day component is zero-padded to two digits.
// 2024-01-05 09:05:03 -> "#2024-01-05 09:05:03#"
#[test]
fn sql_date_zero_pads_single_digit_components() {
    assert_eq!(sql_date(dt("2024-01-05 09:05:03")), "#2024-01-05 09:05:03#");
}

// Rule 3: the time is 24-hour (hh = 00-23), no AM/PM.
// 2024-03-05 14:30:45 -> "#2024-03-05 14:30:45#"
#[test]
fn sql_date_uses_24_hour_clock() {
    assert_eq!(sql_date(dt("2024-03-05 14:30:45")), "#2024-03-05 14:30:45#");
}

// Rule 3 (boundary): midnight renders a zero hour, not 12.
// 2024-03-01 00:00:00 -> "#2024-03-01 00:00:00#"
#[test]
fn sql_date_midnight_is_zero_hour() {
    assert_eq!(sql_date(dt("2024-03-01 00:00:00")), "#2024-03-01 00:00:00#");
}

// Rule 2 (boundary): already-two-digit components are not over-padded.
// 2024-12-31 23:59:59 -> "#2024-12-31 23:59:59#"
#[test]
fn sql_date_double_digit_components_stay_two_digits() {
    assert_eq!(sql_date(dt("2024-12-31 23:59:59")), "#2024-12-31 23:59:59#");
}

// Rule 1: the value is wrapped in #...# delimiters with the exact
// separators: '-' between date parts, a single space between date and time,
// ':' between time parts.
#[test]
fn sql_date_wraps_in_hash_delimiters_with_expected_separators() {
    let out = sql_date(dt("2024-06-15 08:09:10"));
    assert!(out.starts_with('#'), "must open with #");
    assert!(out.ends_with('#'), "must close with #");
    assert_eq!(out, "#2024-06-15 08:09:10#");
}

// ---- Added for the book (2026-10-28). Not part of the AI's output. ----
// Stubs with the signatures the tests above assume, so the file compiles.
// Each body is todo!() until the rule is implemented.
fn sql_date(_d: NaiveDateTime) -> String { todo!() }
