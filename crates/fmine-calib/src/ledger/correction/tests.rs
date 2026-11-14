//! Determinism: DeterministicPerIsa

use super::*;
use crate::ledger::correction::CorrectionMethod;
use crate::ledger::{AssessmentPolicy, CohortSpec, MissingReason, OutcomeCitation, OutcomeKind};

fn metadata() -> ForecastMetadata {
    ForecastMetadata {
        family: "synthetic-quarterly-production".into(),
        author_role: "synthetic-model".into(),
        domain: "same-domain".into(),
        horizon: "one-quarter".into(),
        site: "synthetic-site".into(),
        quantity: "gold-production-100pct".into(),
        unit: "koz".into(),
    }
}

fn citation(index: usize, vintage: &str) -> OutcomeCitation {
    OutcomeCitation {
        source: "synthetic".into(),
        vintage: vintage.into(),
        locator: format!("synthetic-outcome-{index}"),
        sha256: fmine_core::hash::sha256(format!("{index}:{vintage}").as_bytes()).0,
    }
}

fn gaussian_payload(mean: f64, std_dev: f64) -> ForecastPayload {
    ForecastPayload::Gaussian { mean, std_dev }
}

fn setup(
    historical: &[(f64, f64, Option<f64>)],
    target: ForecastPayload,
    target_metadata: ForecastMetadata,
) -> (ForecastLedger, Digest32, ObjectId, Vec<ObjectId>) {
    let mut ledger = ForecastLedger::new();
    let ids = historical
        .iter()
        .enumerate()
        .map(|(i, &(mean, std_dev, _))| {
            ledger
                .register_payload_with_metadata(
                    "synthetic-model-v1",
                    i as i64 + 1,
                    100,
                    gaussian_payload(mean, std_dev),
                    metadata(),
                )
                .expect("training forecast")
        })
        .collect::<Vec<_>>();
    let cohort = ledger
        .freeze_cohort(CohortSpec {
            name: "complete-training-population".into(),
            frozen_at_ns: 50,
            members: ids.clone(),
            resolution_rule: "same synthetic quarterly gold production".into(),
            eligible_sources: vec!["synthetic".into()],
            policy: AssessmentPolicy::default(),
        })
        .expect("cohort");
    for (i, (&id, (_, _, value))) in ids.iter().zip(historical).enumerate() {
        if let Some(value) = value {
            ledger
                .append_outcome(
                    id,
                    101,
                    102,
                    OutcomeKind::Published {
                        value: *value,
                        citation: citation(i, "original"),
                    },
                )
                .expect("outcome");
        }
    }
    let assessment = ledger
        .assess(cohort, 110, OutcomeSelector::FirstPublished)
        .expect("saved")
        .digest;
    let target = ledger
        .register_payload_with_metadata("synthetic-model-v1", 120, 200, target, target_metadata)
        .expect("later forecast");
    (ledger, assessment, target, ids)
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-12 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

#[test]
fn constant_ratio_bias_is_visible_and_zero_log_dispersion_never_collapses_intervals() {
    let (mut ledger, assessment, target, _) = setup(
        &[
            (10.0, 1.0, Some(20.0)),
            (20.0, 2.0, Some(40.0)),
            (40.0, 4.0, Some(80.0)),
        ],
        gaussian_payload(50.0, 5.0),
        metadata(),
    );
    let raw_before = ledger.forecast(target).expect("target").clone();
    let prior = ledger
        .correction_proposal(assessment, target)
        .expect("initial proposal");
    let saved_before = ledger
        .assess(prior.cohort_digest, prior.training_as_of_ns, prior.selector)
        .expect("public saved-assessment snapshot")
        .clone();
    let proposal = ledger
        .correction_proposal(assessment, target)
        .expect("proposal");
    assert_eq!(
        proposal.digest.to_hex(),
        "25f3ef710a725a8f84f405c6ecf8c7d777d732cb42305385b730727c89ab01ad",
        "FACT: captured from the unchanged log implementation before additive support"
    );
    assert_eq!(proposal.method, CorrectionMethod::MultiplicativeLog);
    assert_eq!(
        proposal,
        ledger
            .correction_proposal_with_method(
                assessment,
                target,
                CorrectionMethod::MultiplicativeLog,
            )
            .expect("explicit legacy method")
    );
    close(proposal.adjustment, 2.0);
    close(proposal.mean_error, std::f64::consts::LN_2);
    close(proposal.sample_error_std_dev, 0.0);
    assert_eq!(proposal.inflation, 1.0);
    let ForecastPayload::Gaussian { mean, std_dev } = proposal.corrected else {
        panic!("Gaussian")
    };
    close(mean, 100.0);
    close(std_dev, 10.0);
    assert_eq!(proposal.raw_training_hits, 0);
    assert_eq!(proposal.corrected_training_hits, 3);
    assert_eq!(proposal.required_training_hits, 3);
    assert_eq!(ledger.forecast(target), Some(&raw_before));
    assert_eq!(
        ledger
            .assess(prior.cohort_digest, prior.training_as_of_ns, prior.selector)
            .expect("public saved-assessment replay"),
        &saved_before
    );
    assert_eq!(
        proposal.digest,
        ledger
            .correction_proposal(assessment, target)
            .expect("rerun")
            .digest
    );
}

#[test]
fn overprediction_bias_rescales_width_before_the_inflation_floor() {
    let (ledger, assessment, target, _) = setup(
        &[(20.0, 2.0, Some(10.0)), (40.0, 4.0, Some(20.0))],
        gaussian_payload(50.0, 5.0),
        metadata(),
    );
    let report = ledger
        .correction_proposal(assessment, target)
        .expect("downward correction");
    close(report.adjustment, 0.5);
    assert_eq!(report.inflation, 1.0);
    let ForecastPayload::Gaussian { mean, std_dev } = report.corrected else {
        panic!("Gaussian")
    };
    close(mean, 25.0);
    close(std_dev, 2.5);
    assert!(
        report.corrected_interval.1 - report.corrected_interval.0
            < report.raw_interval.1 - report.raw_interval.0
    );
}

#[test]
fn reciprocal_ratio_known_answer_restores_training_coverage_without_claiming_validation() {
    // Independent calculation: ratios {1/2,1,2} have geometric mean 1 and sample log SD ln(2).
    // The 80% inverse empirical rank is 3; max absolute residual is 10, so kappa=10/z_0.9.
    let (ledger, assessment, target, _) = setup(
        &[
            (10.0, 1.0, Some(5.0)),
            (10.0, 1.0, Some(10.0)),
            (10.0, 1.0, Some(20.0)),
        ],
        gaussian_payload(30.0, 3.0),
        metadata(),
    );
    let proposal = ledger
        .correction_proposal(assessment, target)
        .expect("proposal");
    close(proposal.adjustment, 1.0);
    close(proposal.sample_error_std_dev, std::f64::consts::LN_2);
    close(proposal.inflation, 10.0 / 1.281_551_565_544_600_4);
    close(proposal.corrected_interval.0, 0.0);
    close(proposal.corrected_interval.1, 60.0);
    assert_eq!(
        (proposal.raw_training_hits, proposal.corrected_training_hits),
        (1, 3)
    );
    assert!(NO_CLAIM.contains("not out-of-sample"));
}

#[test]
fn saved_selection_is_stable_after_later_training_restatements() {
    let (mut ledger, assessment, target, ids) = setup(
        &[(10.0, 1.0, Some(20.0)), (20.0, 2.0, Some(40.0))],
        gaussian_payload(50.0, 5.0),
        metadata(),
    );
    let original = ledger
        .correction_proposal(assessment, target)
        .expect("original");
    let predecessor = ledger.outcomes(ids[0])[0].seq;
    ledger
        .append_outcome(
            ids[0],
            130,
            131,
            OutcomeKind::Restated {
                value: 200.0,
                citation: citation(0, "restatement"),
                supersedes: predecessor,
            },
        )
        .expect("later restatement");
    assert_eq!(
        original,
        ledger
            .correction_proposal(assessment, target)
            .expect("saved selection")
    );
    let later = ledger
        .assess(original.cohort_digest, 140, OutcomeSelector::LatestAsOf)
        .expect("new saved report")
        .digest;
    assert!(
        ledger.correction_proposal(later, target).is_err(),
        "new cut is later than target registration"
    );
}

#[test]
fn incomplete_nonpositive_and_non_gaussian_populations_refuse_without_filtering() {
    for historical in [
        vec![(10.0, 1.0, Some(20.0)), (20.0, 2.0, None)],
        vec![(10.0, 1.0, Some(20.0)), (20.0, 2.0, Some(0.0))],
        vec![(10.0, 1.0, Some(20.0)), (-20.0, 2.0, Some(40.0))],
        vec![(10.0, 1.0, Some(20.0))],
    ] {
        let (ledger, assessment, target, _) =
            setup(&historical, gaussian_payload(50.0, 5.0), metadata());
        assert!(ledger.correction_proposal(assessment, target).is_err());
    }
    for target in [
        gaussian_payload(0.0, 1.0),
        ForecastPayload::Interval {
            lo: 40.0,
            hi: 60.0,
            nominal: Some(0.8),
        },
    ] {
        let (ledger, assessment, target, _) = setup(
            &[(10.0, 1.0, Some(20.0)), (20.0, 2.0, Some(40.0))],
            target,
            metadata(),
        );
        assert!(ledger.correction_proposal(assessment, target).is_err());
    }
}

#[test]
fn every_segment_dimension_is_required_and_outcomes_prevent_retrospective_proposals() {
    for field in 0..7 {
        let mut changed = metadata();
        match field {
            0 => changed.family.push_str("-other"),
            1 => changed.author_role.push_str("-other"),
            2 => changed.domain.push_str("-other"),
            3 => changed.horizon.push_str("-other"),
            4 => changed.site.push_str("-other"),
            5 => changed.quantity.push_str("-other"),
            _ => changed.unit.push_str("-other"),
        }
        let (ledger, assessment, target, _) = setup(
            &[(10.0, 1.0, Some(20.0)), (20.0, 2.0, Some(40.0))],
            gaussian_payload(50.0, 5.0),
            changed,
        );
        for method in [
            CorrectionMethod::MultiplicativeLog,
            CorrectionMethod::AdditiveResidual,
        ] {
            assert!(
                ledger
                    .correction_proposal_with_method(assessment, target, method)
                    .is_err()
            );
        }
    }
    let (mut ledger, assessment, target, _) = setup(
        &[(10.0, 1.0, Some(20.0)), (20.0, 2.0, Some(40.0))],
        gaussian_payload(50.0, 5.0),
        metadata(),
    );
    ledger
        .append_outcome(
            target,
            201,
            202,
            OutcomeKind::Missing {
                reason: MissingReason::NotPublished,
            },
        )
        .expect("missing event");
    for method in [
        CorrectionMethod::MultiplicativeLog,
        CorrectionMethod::AdditiveResidual,
    ] {
        assert!(
            ledger
                .correction_proposal_with_method(assessment, target, method)
                .is_err(),
            "missing is also an outcome event"
        );
    }
}

#[test]
fn unseen_assessments_early_targets_and_conflicting_alpha_refuse() {
    let (mut ledger, assessment, target, ids) = setup(
        &[(10.0, 1.0, Some(20.0)), (20.0, 2.0, Some(40.0))],
        gaussian_payload(50.0, 5.0),
        metadata(),
    );
    for method in [
        CorrectionMethod::MultiplicativeLog,
        CorrectionMethod::AdditiveResidual,
    ] {
        assert!(
            ledger
                .correction_proposal_with_method(Digest32([7; 32]), target, method)
                .is_err()
        );
        assert!(
            ledger
                .correction_proposal_with_method(assessment, ids[0], method)
                .is_err()
        );
    }
    ledger
        .freeze_cohort(CohortSpec {
            name: "future-at-another-level".into(),
            frozen_at_ns: 121,
            members: vec![target],
            resolution_rule: "same synthetic quantity".into(),
            eligible_sources: vec!["synthetic".into()],
            policy: AssessmentPolicy {
                alpha: 0.1,
                ..AssessmentPolicy::default()
            },
        })
        .expect("future cohort");
    for method in [
        CorrectionMethod::MultiplicativeLog,
        CorrectionMethod::AdditiveResidual,
    ] {
        assert!(
            ledger
                .correction_proposal_with_method(assessment, target, method)
                .is_err()
        );
    }
}

#[test]
fn additive_constant_residual_keeps_signed_and_zero_values_and_predictive_width() {
    let (mut ledger, assessment, target, ids) = setup(
        &[
            (-10.0, 1.0, Some(-8.0)),
            (-2.0, 1.0, Some(0.0)),
            (0.0, 1.0, Some(2.0)),
        ],
        gaussian_payload(-5.0, 1.0),
        metadata(),
    );
    assert!(
        ledger.correction_proposal(assessment, target).is_err(),
        "the default log method cannot silently fall back"
    );
    let raw_before = ledger.forecast(target).expect("raw").clone();
    let proposal = ledger
        .correction_proposal_with_method(assessment, target, CorrectionMethod::AdditiveResidual)
        .expect("signed additive proposal");
    assert_eq!(proposal.method, CorrectionMethod::AdditiveResidual);
    close(proposal.mean_error, 2.0);
    close(proposal.adjustment, 2.0);
    close(proposal.sample_error_std_dev, 0.0);
    assert_eq!(proposal.inflation, 1.0);
    assert_eq!(proposal.corrected, gaussian_payload(-3.0, 1.0));
    assert_eq!(
        (
            proposal.raw_training_hits,
            proposal.corrected_training_hits,
            proposal.required_training_hits,
        ),
        (0, 3, 3)
    );
    assert_eq!(proposal.members.len(), 3);
    for member in &proposal.members {
        close(member.error, 2.0);
        close(
            member.corrected_interval.1 - member.corrected_interval.0,
            2.0 * 1.281_551_565_544_600_4,
        );
    }
    assert_eq!(ledger.forecast(target), Some(&raw_before));
    let saved_before = ledger
        .assess(
            proposal.cohort_digest,
            proposal.training_as_of_ns,
            proposal.selector,
        )
        .expect("saved assessment")
        .clone();
    let predecessor = ledger.outcomes(ids[0])[0].seq;
    ledger
        .append_outcome(
            ids[0],
            130,
            131,
            OutcomeKind::Restated {
                value: -100.0,
                citation: citation(0, "later"),
                supersedes: predecessor,
            },
        )
        .expect("later restatement");
    assert_eq!(
        proposal,
        ledger
            .correction_proposal_with_method(assessment, target, CorrectionMethod::AdditiveResidual)
            .expect("unchanged historical selection")
    );
    assert_eq!(
        ledger
            .assess(
                proposal.cohort_digest,
                proposal.training_as_of_ns,
                proposal.selector,
            )
            .expect("immutable saved assessment"),
        &saved_before
    );
    assert!(proposal.method.no_claim().contains("target's declared unit"));
    assert!(proposal.method.no_claim().contains("not out-of-sample"));
}

#[test]
fn additive_residual_dispersion_inflates_against_the_complete_empirical_rank() {
    // Independent calculation: residuals {-4, 0, 4} have mean 0 and sample SD 4.
    // The 80% empirical rank is 3, so kappa=4/z_0.9; target half-width is 2*4=8.
    let (ledger, assessment, target, _) = setup(
        &[
            (-10.0, 1.0, Some(-14.0)),
            (0.0, 1.0, Some(0.0)),
            (10.0, 1.0, Some(14.0)),
        ],
        gaussian_payload(-5.0, 2.0),
        metadata(),
    );
    let proposal = ledger
        .correction_proposal_with_method(assessment, target, CorrectionMethod::AdditiveResidual)
        .expect("additive dispersion");
    close(proposal.mean_error, 0.0);
    close(proposal.adjustment, 0.0);
    close(proposal.sample_error_std_dev, 4.0);
    close(proposal.inflation, 4.0 / 1.281_551_565_544_600_4);
    close(proposal.corrected_interval.0, -13.0);
    close(proposal.corrected_interval.1, 3.0);
    assert_eq!(
        (proposal.raw_training_hits, proposal.corrected_training_hits),
        (1, 3)
    );
    assert!(proposal.inflation > 1.0);
    let ForecastPayload::Gaussian { mean, std_dev } = proposal.corrected else {
        panic!("Gaussian")
    };
    close(mean, -5.0);
    assert!(std_dev > 2.0);
}

#[test]
fn additive_downward_shift_crosses_zero_without_narrowing_or_changing_log_identities() {
    let (ledger, assessment, target, _) = setup(
        &[(2.0, 1.0, Some(0.0)), (4.0, 2.0, Some(2.0))],
        gaussian_payload(0.0, 3.0),
        metadata(),
    );
    let proposal = ledger
        .correction_proposal_with_method(assessment, target, CorrectionMethod::AdditiveResidual)
        .expect("negative corrected mean");
    close(proposal.adjustment, -2.0);
    assert_eq!(proposal.corrected, gaussian_payload(-2.0, 3.0));
    assert_eq!(proposal.inflation, 1.0);

    let (ledger, assessment, target, _) = setup(
        &[(10.0, 1.0, Some(12.0)), (20.0, 2.0, Some(22.0))],
        gaussian_payload(30.0, 3.0),
        metadata(),
    );
    let log = ledger
        .correction_proposal(assessment, target)
        .expect("positive log proposal");
    let additive = ledger
        .correction_proposal_with_method(assessment, target, CorrectionMethod::AdditiveResidual)
        .expect("positive additive proposal");
    assert_ne!(log.digest, additive.digest);
    assert_ne!(log.method.single_name(), additive.method.single_name());
    assert_eq!(additive.corrected, gaussian_payload(32.0, 3.0));
}

#[test]
fn additive_still_refuses_incomplete_non_gaussian_and_unrepresentable_proposals() {
    for historical in [
        vec![(-10.0, 1.0, Some(-8.0))],
        vec![(-10.0, 1.0, Some(-8.0)), (0.0, 1.0, None)],
    ] {
        let (ledger, assessment, target, _) =
            setup(&historical, gaussian_payload(-5.0, 1.0), metadata());
        assert!(
            ledger
                .correction_proposal_with_method(
                    assessment,
                    target,
                    CorrectionMethod::AdditiveResidual,
                )
                .is_err()
        );
    }
    for target in [
        ForecastPayload::Interval {
            lo: -10.0,
            hi: 0.0,
            nominal: Some(0.8),
        },
        gaussian_payload(-5.0, f64::from_bits(1)),
    ] {
        let (ledger, assessment, target, _) = setup(
            &[(-10.0, 1.0, Some(-8.0)), (0.0, 1.0, Some(2.0))],
            target,
            metadata(),
        );
        let before = ledger.forecast(target).expect("raw").clone();
        assert!(
            ledger
                .correction_proposal_with_method(
                    assessment,
                    target,
                    CorrectionMethod::AdditiveResidual,
                )
                .is_err()
        );
        assert_eq!(ledger.forecast(target), Some(&before));
    }
    let (ledger, assessment, target, _) = setup(
        &[
            (0.0, 1e307, Some(1e307)),
            (0.0, 1e307, Some(1e307)),
        ],
        gaussian_payload(f64::MAX / 1.005, 1e303),
        metadata(),
    );
    let before = ledger.forecast(target).expect("raw").clone();
    assert!(
        ledger
            .correction_proposal_with_method(assessment, target, CorrectionMethod::AdditiveResidual)
            .is_err(),
        "a finite raw mean plus a finite shift can still overflow"
    );
    assert_eq!(ledger.forecast(target), Some(&before));
}

#[test]
fn correction_overflow_refuses_without_mutating_raw_forecast() {
    let (ledger, assessment, target, _) = setup(
        &[(1.0, 1.0, Some(4.0)), (2.0, 1.0, Some(8.0))],
        gaussian_payload(f64::MAX / 2.0, 1.0),
        metadata(),
    );
    let before = ledger.forecast(target).expect("raw").clone();
    assert!(ledger.correction_proposal(assessment, target).is_err());
    assert_eq!(ledger.forecast(target), Some(&before));
}

#[test]
fn nonzero_gaussian_spread_cannot_be_rendered_as_a_collapsed_interval() {
    let (ledger, assessment, target, _) = setup(
        &[(10.0, 1.0, Some(20.0)), (20.0, 2.0, Some(40.0))],
        gaussian_payload(50.0, f64::from_bits(1)),
        metadata(),
    );
    let before = ledger.forecast(target).expect("raw").clone();
    let error = ledger
        .correction_proposal(assessment, target)
        .expect_err("sub-ULP Gaussian width refuses");
    assert!(error.what.contains("not representable"));
    assert_eq!(ledger.forecast(target), Some(&before));
}
