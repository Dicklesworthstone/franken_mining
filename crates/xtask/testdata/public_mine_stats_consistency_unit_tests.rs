#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn member_order_is_commutative_but_equation_roles_are_not() {
        assert_eq!(
            role(Family::SitesToRegion, 0, 3),
            role(Family::SitesToRegion, 1, 3)
        );
        assert_ne!(
            role(Family::QuarterSum, 0, 3),
            role(Family::QuarterSum, 1, 3)
        );
        assert_ne!(
            role(Family::SoldWeightedRate, 0, 5),
            role(Family::SoldWeightedRate, 1, 5)
        );
        assert!(role(Family::Ownership, 0, 1).is_none());
    }
    #[test]
    fn displayed_precision_is_retained_but_hidden_storage_digits_are_not() {
        let number = |stored: &str, shown: &str| PublicValue::Number {
            stored: stored.into(),
            displayed: Ok(shown.into()),
            under_status: None,
        };
        assert_eq!(
            displayed(&number("100.001", "100")),
            displayed(&number("100.002", "100"))
        );
        assert_ne!(
            displayed(&number("100", "100")),
            displayed(&number("100", "100.0"))
        );
        assert_ne!(
            displayed(&number("0", "0")),
            displayed(&PublicValue::Nil {
                token: "0".into(),
                reason: crate::key::NilReason::Withheld
            })
        );
    }
}
