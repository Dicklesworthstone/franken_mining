#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::expect_used)]
    fn inspection_refuses_bad_bodies_and_clearance_and_exposes_missing_ancestry() {
        let mut report = fmine_public::recovery::run_synthetic().expect("typed model fixture");
        let root = report.quarters[0]
            .outcome
            .as_ref()
            .expect("model")
            .constant_tails
            .claim_id;
        let mut nodes = inventory(&report).expect("validated model inventory");
        let absent = report.training[0].head.claim_id;
        nodes.remove(&absent);
        let json = trace(root, &nodes);
        assert!(json.contains("\"complete_for_claim\":false"));
        assert!(json.contains(&format!("\"unresolved_parents\":[\"{absent}\"]")));
        assert!(!json.contains(&format!("\"claim_id\":\"{absent}\"")));
        let original_key = report.fit.metric_key.clone();
        report.fit.metric_key.push_str(".mutated");
        assert_eq!(
            inventory(&report)
                .err()
                .expect("mutated canonical body")
                .code,
            "FMINE-REF-507"
        );
        report.fit.metric_key = original_key;
        report.fit.confidentiality = Confidentiality::internal();
        report.fit.claim_id = fmine_core::id::ObjectId::from_canonical_bytes(
            fmine_core::id::PrefixKind::Claim,
            &report.fit.canonical_body(),
        );
        report.fit.check_id().expect("valid private Claim");
        assert_eq!(
            inventory(&report)
                .err()
                .expect("private Claim cannot be inspected publicly")
                .code,
            "FMINE-REF-301"
        );
    }
}
