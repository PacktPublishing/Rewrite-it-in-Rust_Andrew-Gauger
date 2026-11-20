//! Field rules the forms check before a save. Each mirrors a constraint
//! the Chapter 9 schema enforces, so the form refuses what Postgres
//! would, with a message a person can act on. V1 had none: Access
//! checked only types and required fields.

/// A rate or a discount: `0.0825` for 8.25%. The schema's `fraction`
/// columns are checked between 0 and 1.
pub fn fraction(value: &f64) -> Result<(), String> {
    if (0.0..=1.0).contains(value) {
        Ok(())
    } else {
        Err("Must be between 0% and 100%".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fraction_is_zero_to_one() {
        assert!(fraction(&0.0825).is_ok());
        assert!(fraction(&1.0).is_ok());
        assert!(fraction(&8.25).is_err());
        assert!(fraction(&-0.01).is_err());
    }
}
