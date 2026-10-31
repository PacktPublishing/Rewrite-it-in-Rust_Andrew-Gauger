// Tests for vba:CairnForms.NextNumber (CairnForms L670-681).
//
// Rule under test: NextNumber(prefix) returns  prefix & "-" & year & "-" & seq
// where seq = Format(Nz(maxSeq, 0) + 1, "0000").
//   - The prefix and the current calendar year are echoed verbatim into the
//     output (L678: stem = prefix & "-" & Year(Date) & "-").
//   - The sequence is the largest numeric sequence already used this year,
//     parsed with Val (L679), plus one.
//   - Format "0000" zero-pads to AT LEAST 4 digits and does not truncate
//     longer values (L680).
//   - If no row matches this year, Nz(n,0) -> 0, so the first number is 0001.
//
// The prefix routing (Q->Quotes, RMA->RMAs, else->Orders, L673-677) only
// selects which table's max is read; because the prefix is echoed into the
// output it is not independently observable, so the tests below pin the
// observable format rule for each of the three prefixes.
//
// Assume: fn next_number(prefix: &str, year: i32, max_seq: i32) -> String

#[test]
fn quote_2026_max_1699_yields_1700() {
    // Q-2026-1699 exists; next is 1699+1 = 1700, already 4 digits.
    assert_eq!(next_number("Q", 2026, 1699), "Q-2026-1700");
}

#[test]
fn order_2026_max_999_yields_1000() {
    // 999+1 = 1000, exactly 4 digits, no padding needed.
    assert_eq!(next_number("SO", 2026, 999), "SO-2026-1000");
}

#[test]
fn rma_2026_max_69_yields_0070() {
    // 69+1 = 70, padded to 4 digits -> 0070.
    assert_eq!(next_number("RMA", 2026, 69), "RMA-2026-0070");
}

#[test]
fn quote_2026_no_rows_yields_0001() {
    // max_seq 0 means no row matched this year; Nz(0)+1 = 1 -> 0001.
    assert_eq!(next_number("Q", 2026, 0), "Q-2026-0001");
}

#[test]
fn order_2026_no_rows_yields_0001() {
    assert_eq!(next_number("SO", 2026, 0), "SO-2026-0001");
}

#[test]
fn rma_2026_no_rows_yields_0001() {
    assert_eq!(next_number("RMA", 2026, 0), "RMA-2026-0001");
}

#[test]
fn quote_2025_max_83_yields_0084() {
    // Year is part of the stem; 83+1 = 84 padded -> 0084.
    assert_eq!(next_number("Q", 2025, 83), "Q-2025-0084");
}

#[test]
fn order_2016_no_rows_yields_0001() {
    // Older year stem; first number of that year is 0001.
    assert_eq!(next_number("SO", 2016, 0), "SO-2016-0001");
}

#[test]
fn quote_2026_max_999_yields_1000() {
    // 999+1 = 1000, exactly 4 digits.
    assert_eq!(next_number("Q", 2026, 999), "Q-2026-1000");
}

#[test]
fn quote_2026_max_10000_yields_10001() {
    // Format "0000" pads to at least 4 digits but does NOT truncate:
    // 10000+1 = 10001 stays 5 digits.
    assert_eq!(next_number("Q", 2026, 10000), "Q-2026-10001");
}
