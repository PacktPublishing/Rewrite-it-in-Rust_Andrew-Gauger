use rust_decimal::Decimal;

/// V1's `Round(x, 2)`.
///
/// Access rounds a `Double`, not an exact decimal: it multiplies by 100 in
/// floating point and sends an exact half to the even cent. Tax rates,
/// discounts and commission rates are `Double` columns, so every money
/// figure V1 prints went through this.
// FLAG: R05 V1 rounds binary floating-point values half to even, so some printed cents differ from exact arithmetic (528 of 13,531 orders' tax).
pub fn access_round(x: f64) -> Decimal {
    let cents = (x * 100.0).round_ties_even();
    Decimal::new(cents as i64, 2)
}

/// V1's `Round(amount * rate, 2)`, where `amount` is a `Currency` and `rate`
/// a `Double`. Access converts the amount to the nearest `Double` first.
pub fn access_round_product(amount: Decimal, rate: f64) -> Decimal {
    access_round(to_f64(amount) * rate)
}

/// The nearest `f64` to a decimal, as Access converts `Currency` to `Double`.
/// Parsing the text is correctly rounded; arithmetic on the mantissa is not.
pub(crate) fn to_f64(d: Decimal) -> f64 {
    d.to_string()
        .parse()
        .expect("a Decimal always prints as a valid f64")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn exact_halves_go_to_the_even_cent() {
        assert_eq!(access_round(0.125), dec("0.12"));
        assert_eq!(access_round(0.375), dec("0.38"));
        assert_eq!(access_round(-0.025), dec("-0.02"));
    }

    #[test]
    fn so_2026_1053_tax_is_877_10_not_877_11() {
        // 12098.00 x 0.0725 is exactly 877.105, but as doubles it is
        // 877.1049999999999, so Access prints 877.10.
        assert_eq!(
            access_round_product(dec("12098.00"), 0.0725),
            dec("877.10")
        );
    }
}
