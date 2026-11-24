//! A form field: what `#[derive(Form)]` records about it, and how its
//! value turns into text and back.

use cairn_core::status::{CustomerStatus, OrderStatus, QuoteStatus};
use cairn_core::{
    ContactId, CustomerId, Date, OrderId, PaymentId, ProductId,
    QuoteId, RepId,
};
use rust_decimal::Decimal;
use std::str::FromStr;

/// Which widget a field gets. V1's helpers were `Txt` (with a `Format`),
/// `Chk` and `Cbo`; these are the same three, split by format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    Integer,
    /// `Format = "Currency"`: edited as `1234.5`, shown as `$1,234.50`.
    Money,
    /// `Format = "Percent"`: edited as `8.25`, shown as `8.25%`, stored
    /// as the fraction `0.0825`.
    Percent,
    /// Edited and shown as `10/31/2026`, the way V1's users type it.
    Date,
    Check,
    /// A combo box: `list` names a lookup the form's context supplies.
    /// `filter` is the field whose value narrows it (V1's contacts combo
    /// shows only the customer's contacts). With `limit` (V1's
    /// `LimitToList`, the default), only a row in the list can be picked;
    /// without it, the list only suggests.
    Lookup {
        list: &'static str,
        filter: Option<usize>,
        limit: bool,
    },
}

/// Everything the form needs to know about one field, written by
/// `#[derive(Form)]` from the field's `#[field(...)]` attribute.
#[derive(Debug, Clone, Copy)]
pub struct FieldSpec {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    /// Which column of the form the field sits in, from 0.
    pub column: u8,
    /// Locked, and out of the Tab order (V1's `Locked`, `TabStop = False`).
    pub read_only: bool,
    pub required: bool,
    /// More than 1 makes a multi-line text box, where Enter is a newline.
    pub lines: u8,
    /// Width in points; 0 for the default.
    pub width: f32,
    /// V1's `ControlTipText`.
    pub hint: &'static str,
    /// On the record but not on screen: a key another field's lookup is
    /// narrowed by.
    pub hidden: bool,
}

/// A value a form field can edit.
pub trait FieldValue {
    /// The value as the user edits it. For a lookup, the row's key.
    fn text(&self) -> String;

    /// Set the value from what the user typed, or say why it isn't one.
    fn set_text(&mut self, text: &str) -> Result<(), String>;

    fn is_empty(&self) -> bool {
        self.text().trim().is_empty()
    }

    /// Check boxes edit a `bool` directly.
    fn checkbox(&mut self) -> Option<&mut bool> {
        None
    }
}

/// A date and a time of day, to the minute (V1's `Now()` stamps:
/// `ReceivedAt`, `ActivityDate`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct DateTime {
    pub date: Date,
    /// Minutes since midnight.
    pub minute: u16,
}

impl DateTime {
    pub fn new(date: Date, hour: u16, minute: u16) -> Self {
        DateTime {
            date,
            minute: hour * 60 + minute,
        }
    }
}

impl std::fmt::Display for DateTime {
    /// `10/31/2026 3:05 PM`
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        let (h, m) = (self.minute / 60, self.minute % 60);
        let (h12, ampm) = match h {
            0 => (12, "AM"),
            1..=11 => (h, "AM"),
            12 => (12, "PM"),
            _ => (h - 12, "PM"),
        };
        write!(f, "{} {h12}:{m:02} {ampm}", date_text(self.date))
    }
}

impl FromStr for DateTime {
    type Err = String;

    /// `10/31/2026`, `10/31/26 3:05 PM`, `10/31/2026 15:05`, or
    /// Postgres's `2026-10-31 15:05`.
    fn from_str(text: &str) -> Result<Self, String> {
        let bad = || format!("{text:?} isn't a date and time");
        let t = text.trim();
        let (date, rest) = t.split_once(' ').unwrap_or((t, ""));
        let date = parse_date(date)?;
        let rest = rest.trim().to_uppercase();
        if rest.is_empty() {
            return Ok(DateTime { date, minute: 0 });
        }
        let (clock, ampm) = match rest.split_once(' ') {
            Some((c, a)) => {
                (c.to_string(), Some(a.trim().to_string()))
            }
            None => match rest
                .strip_suffix("AM")
                .or(rest.strip_suffix("PM"))
            {
                Some(c) => {
                    (c.to_string(), Some(rest[c.len()..].to_string()))
                }
                None => (rest.clone(), None),
            },
        };
        let (h, m) = clock.split_once(':').unwrap_or((&clock, "0"));
        let (mut h, m): (u16, u16) = (
            h.trim().parse().map_err(|_| bad())?,
            m.trim().parse().map_err(|_| bad())?,
        );
        match ampm.as_deref() {
            Some("AM") if h == 12 => h = 0,
            Some("PM") if h < 12 => h += 12,
            Some("AM" | "PM") | None => {}
            Some(_) => return Err(bad()),
        }
        if h > 23 || m > 59 {
            return Err(bad());
        }
        Ok(DateTime::new(date, h, m))
    }
}

impl FieldValue for DateTime {
    fn text(&self) -> String {
        self.to_string()
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        *self = text.parse()?;
        Ok(())
    }
}

impl FieldValue for Option<DateTime> {
    fn text(&self) -> String {
        self.map(|d| d.to_string()).unwrap_or_default()
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        *self = match text.trim() {
            "" => None,
            t => Some(t.parse()?),
        };
        Ok(())
    }
}

/// How a field reads when it isn't being edited.
pub fn display(kind: Kind, text: &str) -> String {
    match kind {
        Kind::Money => Decimal::from_str(text)
            .map(money)
            .unwrap_or_else(|_| text.to_string()),
        Kind::Percent if !text.is_empty() => format!("{text}%"),
        _ => text.to_string(),
    }
}

/// `$1,234.50`, as Access's Currency format prints it.
pub fn money(d: Decimal) -> String {
    let d = d.round_dp(2);
    let sign = if d.is_sign_negative() { "-" } else { "" };
    let s = format!("{:.2}", d.abs());
    let (whole, cents) = s.split_once('.').unwrap_or((&s, "00"));
    let mut grouped = String::new();
    for (i, c) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("{sign}${grouped}.{cents}")
}

fn parse_decimal(text: &str) -> Result<Decimal, String> {
    let cleaned: String =
        text.chars().filter(|c| !matches!(c, '$' | ',')).collect();
    Decimal::from_str(cleaned.trim())
        .map_err(|_| format!("{text:?} isn't an amount"))
}

pub(crate) fn parse_date(text: &str) -> Result<Date, String> {
    let bad = || format!("{text:?} isn't a date (use 10/31/2026)");
    let t = text.trim();
    if t.contains('-') {
        return Date::from_str(t).map_err(|_| bad());
    }
    let parts: Vec<&str> = t.split('/').collect();
    let [m, d, y] = parts[..] else {
        return Err(bad());
    };
    let (m, d, mut y): (u32, u32, i32) = (
        m.parse().map_err(|_| bad())?,
        d.parse().map_err(|_| bad())?,
        y.parse().map_err(|_| bad())?,
    );
    if y < 100 {
        // Access follows Windows' two-digit-year window, 1950-2049 by
        // default: on V1's machine 10/31/29 and 10/31/30 both became
        // 20xx (the Windows keystroke run, 2026-11-01).
        y += if y < 50 { 2000 } else { 1900 };
    }
    Date::new(y, m, d).ok_or_else(bad)
}

fn date_text(d: Date) -> String {
    format!("{}/{}/{}", d.month(), d.day(), d.year())
}

impl FieldValue for String {
    fn text(&self) -> String {
        self.clone()
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        text.clone_into(self);
        Ok(())
    }
}

impl FieldValue for Option<String> {
    fn text(&self) -> String {
        self.clone().unwrap_or_default()
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        *self = (!text.is_empty()).then(|| text.to_string());
        Ok(())
    }
}

impl FieldValue for bool {
    fn text(&self) -> String {
        self.to_string()
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        *self = text == "true";
        Ok(())
    }
    fn is_empty(&self) -> bool {
        false
    }
    fn checkbox(&mut self) -> Option<&mut bool> {
        Some(self)
    }
}

impl FieldValue for i32 {
    fn text(&self) -> String {
        self.to_string()
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        *self = text
            .trim()
            .parse()
            .map_err(|_| format!("{text:?} isn't a whole number"))?;
        Ok(())
    }
}

impl FieldValue for Option<i32> {
    fn text(&self) -> String {
        self.map(|n| n.to_string()).unwrap_or_default()
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        *self = match text.trim() {
            "" => None,
            t => Some(t.parse().map_err(|_| {
                format!("{text:?} isn't a whole number")
            })?),
        };
        Ok(())
    }
}

impl FieldValue for Decimal {
    fn text(&self) -> String {
        self.round_dp(2).to_string()
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        *self = parse_decimal(text)?;
        Ok(())
    }
}

impl FieldValue for Option<Decimal> {
    fn text(&self) -> String {
        self.map(|d| d.round_dp(2).to_string()).unwrap_or_default()
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        *self = match text.trim() {
            "" => None,
            t => Some(parse_decimal(t)?),
        };
        Ok(())
    }
}

/// A percentage, stored as the fraction `cairn-core` computes with (an
/// `f64`, like V1's `Double`).
impl FieldValue for f64 {
    fn text(&self) -> String {
        let pct = format!("{:.4}", self * 100.0);
        pct.trim_end_matches('0').trim_end_matches('.').to_string()
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        let t = text.trim().trim_end_matches('%').trim();
        let pct: f64 = t
            .parse()
            .map_err(|_| format!("{text:?} isn't a percentage"))?;
        *self = pct / 100.0;
        Ok(())
    }
}

impl FieldValue for Date {
    fn text(&self) -> String {
        date_text(*self)
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        *self = parse_date(text)?;
        Ok(())
    }
}

impl FieldValue for Option<Date> {
    fn text(&self) -> String {
        self.map(date_text).unwrap_or_default()
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        *self = match text.trim() {
            "" => None,
            t => Some(parse_date(t)?),
        };
        Ok(())
    }
}

/// Keys picked from a lookup: the text is the key.
macro_rules! id_field {
    ($($id:ident),*) => {$(
        impl FieldValue for Option<$id> {
            fn text(&self) -> String {
                self.map(|id| id.0.to_string()).unwrap_or_default()
            }
            fn set_text(&mut self, text: &str) -> Result<(), String> {
                *self = match text {
                    "" => None,
                    t => Some($id(t.parse().map_err(|_| {
                        format!("{t:?} isn't in the list")
                    })?)),
                };
                Ok(())
            }
        }
    )*};
}

id_field!(
    CustomerId, ContactId, RepId, ProductId, QuoteId, OrderId,
    PaymentId
);

/// V1's value lists: the text is the value.
macro_rules! status_field {
    ($($status:ident),*) => {$(
        impl FieldValue for $status {
            fn text(&self) -> String {
                self.to_string()
            }
            fn set_text(&mut self, text: &str) -> Result<(), String> {
                *self = $status::try_from(text)
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
        }
    )*};
}

status_field!(QuoteStatus, OrderStatus, CustomerStatus);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_prints_like_access_currency() {
        assert_eq!(money(Decimal::new(123450, 2)), "$1,234.50");
        assert_eq!(money(Decimal::new(-5, 1)), "-$0.50");
        assert_eq!(
            money(Decimal::new(100000000, 0)),
            "$100,000,000.00"
        );
    }

    #[test]
    fn percent_is_typed_in_percent_and_stored_as_a_fraction() {
        let mut rate = 0.0;
        rate.set_text("8.25%").unwrap();
        assert!((rate - 0.0825).abs() < 1e-12);
        assert_eq!(rate.text(), "8.25");
    }

    #[test]
    fn times_are_typed_twelve_or_twenty_four_hour() {
        let d = Date::new(2026, 10, 31).unwrap();
        let pm = DateTime::new(d, 15, 5);
        assert_eq!(pm.to_string(), "10/31/2026 3:05 PM");
        assert_eq!("10/31/26 3:05 pm".parse::<DateTime>(), Ok(pm));
        assert_eq!("2026-10-31 15:05".parse::<DateTime>(), Ok(pm));
        assert_eq!(
            "10/31/2026 12:00 AM".parse::<DateTime>(),
            Ok(DateTime::new(d, 0, 0))
        );
        assert!("10/31/2026 25:00".parse::<DateTime>().is_err());
    }

    #[test]
    fn dates_are_typed_month_first_with_two_digit_years() {
        let mut d = Date::new(2000, 1, 1).unwrap();
        d.set_text("10/31/26").unwrap();
        assert_eq!(d, Date::new(2026, 10, 31).unwrap());
        d.set_text("10/31/30").unwrap();
        assert_eq!(d.year(), 2030);
        d.set_text("10/31/50").unwrap();
        assert_eq!(d.year(), 1950);
        assert!(d.set_text("2/30/2026").is_err());
    }
}
