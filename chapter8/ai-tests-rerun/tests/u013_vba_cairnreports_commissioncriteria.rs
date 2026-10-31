//! Tests for vba:CairnReports.CommissionCriteria (CairnReports L490-492).
//!
//! The VBA unit:
//!   Public Function CommissionCriteria(ByVal monthStart As Date) As String
//!       CommissionCriteria = "OrderDate >= " & SqlDate(monthStart) & _
//!           " AND OrderDate < " & SqlDate(DateAdd("m", 1, monthStart))
//!   End Function
//!
//! SqlDate (L512-514) is: Format(d, "\#yyyy\-mm\-dd hh\:nn\:ss\#")
//! i.e. an Access date literal "#yyyy-mm-dd hh:nn:ss#" (backslashes escape # and :).
//!
//! Rules pinned:
//!  R1  lower bound = monthStart itself, formatted as the Access date literal.
//!  R2  upper bound = first day of the NEXT month (VBA DateAdd("m",1,monthStart)),
//!      compared with strictly "<"  =>  half-open window [monthStart, nextMonthStart).
//!  R3  the window spans exactly one calendar month, including the Dec->Jan year rollover.
//!  R4  caller (PrintCommission L470-488) normalizes any entered date to the first of
//!      its month via DateSerial(Year, Month, 1) before calling, so the criteria always
//!      covers a whole month.

use chrono::{Datelike, NaiveDate, NaiveDateTime};

/// Access date literal as produced by SqlDate: "#yyyy-mm-dd hh:nn:ss#".
fn sql_date(d: NaiveDate) -> String {
    format!("#{} 00:00:00#", d.format("%Y-%m-%d"))
}

/// Assumed Rust binding of CommissionCriteria(monthStart).
fn commission_criteria(month_start: NaiveDate) -> String {
    format!(
        "OrderDate >= {} AND OrderDate < {}",
        sql_date(month_start),
        sql_date(month_start
            .checked_add_months(chrono::Months::new(1))
            .unwrap())
    )
}

/// Assumed Rust binding of the caller's normalization:
/// DateSerial(Year(CDate(s)), Month(CDate(s)), 1)
fn normalize_month_start(d: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap()
}

#[test]
fn criteria_mid_year_month_is_half_open_window() {
    // monthStart = 2024-03-01 00:00:00
    let got = commission_criteria(NaiveDate::from_ymd_opt(2024, 3, 1).unwrap());
    assert_eq!(
        got,
        "OrderDate >= #2024-03-01 00:00:00# AND OrderDate < #2024-04-01 00:00:00#"
    );
}

#[test]
fn criteria_uses_ge_lower_and_strict_lt_upper() {
    // R1 + R2: lower bound is ">=", upper bound is strictly "<" (half-open).
    let got = commission_criteria(NaiveDate::from_ymd_opt(2024, 6, 1).unwrap());
    assert!(got.starts_with("OrderDate >= #2024-06-01 00:00:00#"));
    assert!(got.ends_with("AND OrderDate < #2024-07-01 00:00:00#"));
    // The window must not be closed: no ">=" on the upper bound.
    assert!(!got.contains("OrderDate >=") || got.matches("OrderDate >=").count() == 1);
    assert!(got.contains("OrderDate <"));
}

#[test]
fn criteria_december_rolls_over_to_next_january() {
    // R3: DateAdd("m", 1, 2024-12-01) = 2025-01-01, so the year increments.
    let got = commission_criteria(NaiveDate::from_ymd_opt(2024, 12, 1).unwrap());
    assert_eq!(
        got,
        "OrderDate >= #2024-12-01 00:00:00# AND OrderDate < #2025-01-01 00:00:00#"
    );
}

#[test]
fn criteria_spans_exactly_one_calendar_month() {
    // R3: for any month the upper bound is the first of the following month.
    for (y, m) in [(2024, 1), (2024, 2), (2024, 11), (2023, 12)] {
        let start = NaiveDate::from_ymd_opt(y, m, 1).unwrap();
        let got = commission_criteria(start);
        let next = start.checked_add_months(chrono::Months::new(1)).unwrap();
        assert_eq!(
            got,
            format!(
                "OrderDate >= {} AND OrderDate < {}",
                sql_date(start),
                sql_date(next)
            )
        );
    }
}

#[test]
fn caller_normalizes_any_date_in_month_to_first_of_month() {
    // R4: the user may enter any date in the target month; the caller reduces it
    // to the first of that month before building the criteria.
    let entered = NaiveDate::from_ymd_opt(2024, 3, 15).unwrap();
    let norm = normalize_month_start(entered);
    assert_eq!(norm, NaiveDate::from_ymd_opt(2024, 3, 1).unwrap());

    // And the resulting criteria covers the whole month.
    let got = commission_criteria(norm);
    assert_eq!(
        got,
        "OrderDate >= #2024-03-01 00:00:00# AND OrderDate < #2024-04-01 00:00:00#"
    );
}
