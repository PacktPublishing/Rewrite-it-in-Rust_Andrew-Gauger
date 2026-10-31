use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// A calendar date. V1 stores dates as `Date/Time`; every rule here uses the
/// date part only.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct Date {
    year: i32,
    month: u32,
    day: u32,
}

impl Date {
    /// `None` if the day doesn't exist.
    pub fn new(year: i32, month: u32, day: u32) -> Option<Date> {
        if !(1..=12).contains(&month)
            || day == 0
            || day > days_in_month(year, month)
        {
            return None;
        }
        Some(Date { year, month, day })
    }

    pub fn year(self) -> i32 {
        self.year
    }

    pub fn month(self) -> u32 {
        self.month
    }

    pub fn day(self) -> u32 {
        self.day
    }

    /// Whole days from `earlier` to `self`, as Access subtracts two dates.
    pub fn days_since(self, earlier: Date) -> i64 {
        self.day_number() - earlier.day_number()
    }

    pub fn add_days(self, days: i64) -> Date {
        from_day_number(self.day_number() + days)
    }

    /// R23: a month report takes any date and covers that calendar month,
    /// from the 1st up to the 1st of the next.
    pub fn in_same_month(self, any_day_in_month: Date) -> bool {
        self.year == any_day_in_month.year
            && self.month == any_day_in_month.month
    }

    // Days since 1970-01-01 (Howard Hinnant's days_from_civil).
    fn day_number(self) -> i64 {
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.month);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5
            + i64::from(self.day)
            - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }
}

fn from_day_number(z: i64) -> Date {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
    Date { year, month, day }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        2 if year % 4 == 0
            && (year % 100 != 0 || year % 400 == 0) =>
        {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadDate(pub String);

impl fmt::Display for BadDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a YYYY-MM-DD date: {:?}", self.0)
    }
}

impl std::error::Error for BadDate {}

impl FromStr for Date {
    type Err = BadDate;

    /// `YYYY-MM-DD`; anything after the date (a time) is ignored.
    fn from_str(s: &str) -> Result<Date, BadDate> {
        let bad = || BadDate(s.to_string());
        let d = s.get(..10).ok_or_else(bad)?;
        let mut parts = d.split('-');
        let mut next = || parts.next().and_then(|p| p.parse().ok());
        let (y, m, dd) = (
            next().ok_or_else(bad)?,
            next().ok_or_else(bad)?,
            next().ok_or_else(bad)?,
        );
        Date::new(y as i32, m, dd).ok_or_else(bad)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    #[test]
    fn days_since_counts_across_months_and_leap_years() {
        assert_eq!(d("2026-10-02").days_since(d("2026-09-23")), 9);
        assert_eq!(d("2024-03-01").days_since(d("2024-02-28")), 2);
        assert_eq!(d("2023-03-01").days_since(d("2023-02-28")), 1);
        assert_eq!(d("2017-01-30").days_since(d("2016-01-30")), 366);
    }

    #[test]
    fn add_days_round_trips() {
        assert_eq!(d("2026-12-31").add_days(1), d("2027-01-01"));
        assert_eq!(d("2026-10-09").add_days(-90), d("2026-07-11"));
    }

    #[test]
    fn rejects_dates_that_dont_exist() {
        assert!("2026-02-29".parse::<Date>().is_err());
        assert!("09/23/26".parse::<Date>().is_err());
    }
}
