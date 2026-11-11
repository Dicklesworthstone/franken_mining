#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticated_upper_bound_is_strict_at_nanosecond_boundaries_and_integer_extremes() {
        assert!(strictly_before(2, 2_000_000_001, "deadline").is_ok());
        for boundary in [1_999_999_999, 2_000_000_000] {
            assert_eq!(
                strictly_before(2, boundary, "deadline").unwrap_err().code,
                "FMINE-REF-206"
            );
        }
        assert!(strictly_before(i64::MIN, i64::MIN, "deadline").is_ok());
        assert_eq!(
            strictly_before(i64::MAX, i64::MAX, "deadline")
                .unwrap_err()
                .code,
            "FMINE-REF-206"
        );
        assert!(strictly_before(9_223_372_036, i64::MAX, "deadline").is_ok());
        assert!(strictly_before(9_223_372_037, i64::MAX, "deadline").is_err());
    }
}
