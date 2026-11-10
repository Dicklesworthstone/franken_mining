//! DESIGN (BD-048/049): independent supplied-quantile loss answers and immutable source-bound projections.
//! Every forecast, source and outcome in these controls is synthetic.

#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::panic)]

use fmine_calib::ledger::diagnostics::quantiles::{
    QuantileDiagnostics, QuantileLossBounds, QuantileLossMember,
};
use fmine_calib::{
    Assessment, AssessmentPolicy, CohortSpec, ForecastLedger, ForecastPayload, MemberOutcome,
    MissingReason, OutcomeCitation, OutcomeKind, OutcomeSelector,
};
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_core::id::ObjectId;
use fmine_units::ExactQty;

fn rat(num: i128, den: i128) -> ExactQty {
    ExactQty::new(num, den).unwrap()
}

fn table(values: [f64; 3]) -> ForecastPayload {
    ForecastPayload::Quantiles {
        levels: vec![0.25, 0.5, 0.75],
        values: values.to_vec(),
    }
}

fn ledger(
    payloads: Vec<ForecastPayload>,
    bounds: Option<(f64, f64)>,
    alpha: f64,
) -> (ForecastLedger, Vec<ObjectId>, Digest32) {
    let mut ledger = ForecastLedger::new();
    let ids = payloads
        .into_iter()
        .enumerate()
        .map(|(index, payload)| {
            ledger
                .register_payload(format!("synthetic-quantile-{index}"), 1, 99, payload)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let cohort = ledger
        .freeze_cohort(CohortSpec {
            name: "synthetic quantile scores".into(),
            frozen_at_ns: 2,
            members: ids.clone(),
            resolution_rule: "Synthetic output in fixed units".into(),
            eligible_sources: vec!["synthetic".into()],
            policy: AssessmentPolicy {
                alpha,
                outcome_bounds: bounds,
                ..AssessmentPolicy::default()
            },
        })
        .unwrap();
    (ledger, ids, cohort)
}

fn citation(vintage: &str) -> OutcomeCitation {
    OutcomeCitation {
        source: "synthetic".into(),
        vintage: vintage.into(),
        locator: "Synthetic!A1".into(),
        sha256: sha256(vintage.as_bytes()).0,
    }
}

fn publish(ledger: &mut ForecastLedger, id: ObjectId, value: f64) {
    ledger
        .append_outcome(
            id,
            100,
            100,
            OutcomeKind::Published {
                value,
                citation: citation("original"),
            },
        )
        .unwrap();
}

fn assess(ledger: &mut ForecastLedger, cohort: Digest32) -> (Assessment, QuantileDiagnostics) {
    let parent = ledger
        .assess(cohort, 101, OutcomeSelector::LatestAsOf)
        .unwrap()
        .clone();
    let report = ledger.quantile_diagnostics(parent.digest).unwrap();
    (parent, report)
}

fn member(report: &QuantileDiagnostics, id: ObjectId) -> &QuantileLossMember {
    report
        .members
        .iter()
        .find(|m| m.member.forecast_id == id)
        .unwrap()
}

#[test]
fn every_supplied_level_scores_and_a_changed_median_leaves_the_frozen_interval_score_alone() {
    let (mut ledger, ids, cohort) = ledger(
        vec![table([10.0, 15.0, 20.0]), table([10.0, 12.0, 20.0])],
        None,
        0.5,
    );
    for id in &ids {
        publish(&mut ledger, *id, 18.0);
    }
    let (parent, report) = assess(&mut ledger, cohort);
    assert_eq!(
        parent.summary.interval_score.unwrap().observed_mean,
        Some(10.0)
    );
    assert_eq!(
        report
            .levels
            .iter()
            .map(|s| s.observed_mean)
            .collect::<Vec<_>>(),
        vec![Some(rat(4, 1)), Some(rat(9, 2)), Some(rat(1, 1))]
    );
    for (id, expected) in [(ids[0], [4, 3, 1]), (ids[1], [4, 6, 1])] {
        let q = &member(&report, id).quantiles;
        assert_eq!(
            q.iter().map(|q| q.loss).collect::<Vec<_>>(),
            expected.map(|value| Some(rat(value, 1)))
        );
        // Independent central pair identity at exactly complementary binary levels .25 and .75.
        assert_eq!(
            q[0].loss
                .unwrap()
                .checked_add(q[2].loss.unwrap())
                .unwrap()
                .checked_div(rat(1, 2))
                .unwrap(),
            rat(10, 1)
        );
    }
    assert_eq!(report.assessment_digest, parent.digest);
    assert_eq!(blake3_hash(&report.canonical_bytes()), report.digest);
    assert_eq!(
        ledger
            .assess(cohort, 101, OutcomeSelector::LatestAsOf)
            .unwrap(),
        &parent
    );
}

#[test]
fn loss_uses_exact_frozen_binary_probabilities_and_preserves_a_one_ulp_error() {
    let payload = ForecastPayload::Quantiles {
        levels: vec![0.1, 0.5, 0.9],
        values: vec![1.0, 1.0, 2.0],
    };
    let (mut ledger, ids, cohort) = ledger(vec![payload], None, 0.2);
    publish(&mut ledger, ids[0], f64::from_bits(1.0_f64.to_bits() + 1));
    let (_, report) = assess(&mut ledger, cohort);
    let q = &member(&report, ids[0]).quantiles;
    // 0.1's exact frozen significand is 3602879701896397 / 2^55, and the error is 2^-52.
    assert_eq!(q[0].loss, Some(rat(3_602_879_701_896_397, 1_i128 << 106)));
    assert_eq!(q[1].loss, Some(rat(1, 1_i128 << 52)));
    assert_ne!(q[0].loss, Some(rat(1, 5_i128 << 52)));
    let json = report.to_json();
    assert!(
        json.contains("\"num\":\"3602879701896397\",\"den\":\"81129638414606681695789005144064\""),
        "{json}"
    );
    assert!(json.contains("\"level_bits\":\"3fb999999999999a\""));
    assert!(
        report
            .to_markdown()
            .contains("3602879701896397/81129638414606681695789005144064")
    );
}

#[test]
fn missing_members_have_sharp_frozen_domain_bounds_and_stay_in_every_supplied_denominator() {
    let (mut ledger, ids, cohort) =
        ledger(vec![table([10.0, 15.0, 20.0]); 2], Some((0.0, 40.0)), 0.5);
    publish(&mut ledger, ids[0], 18.0);
    ledger
        .append_outcome(
            ids[1],
            100,
            100,
            OutcomeKind::Missing {
                reason: MissingReason::NotPublished,
            },
        )
        .unwrap();
    let (_, report) = assess(&mut ledger, cohort);
    for (index, (lower, upper)) in [
        (rat(2, 1), rat(19, 2)),
        (rat(3, 2), rat(14, 1)),
        (rat(1, 2), rat(31, 2)),
    ]
    .into_iter()
    .enumerate()
    {
        let level = report.levels[index];
        assert_eq!(
            (
                level.n_supplied,
                level.n_not_supplied,
                level.n_resolved,
                level.n_unresolved,
                level.n_scored
            ),
            (2, 0, 1, 1, 1)
        );
        assert_eq!(
            level.whole_supplied,
            Some(QuantileLossBounds {
                lower,
                upper: Some(upper)
            })
        );
    }
    let missing = member(&report, ids[1]);
    assert!(missing.member.selected_event.is_some());
    assert!(matches!(
        missing.member.outcome,
        MemberOutcome::Unresolved {
            reason: MissingReason::NotPublished
        }
    ));
    for (entry, upper) in missing.quantiles.iter().zip([15, 25, 30]) {
        assert_eq!(entry.loss, None);
        assert_eq!(entry.status(false), "unresolved");
        assert_eq!(
            entry.completion,
            Some(QuantileLossBounds {
                lower: rat(0, 1),
                upper: Some(rat(upper, 1))
            })
        );
    }
    let json = report.to_json();
    assert!(json.contains("\"outcome_bounds\":[0,40]"));
    assert!(json.contains("\"lower\":{\"num\":\"2\",\"den\":\"1\"},\"upper\":{\"num\":\"19\",\"den\":\"2\"},\"upper_unbounded\":false"));
    assert!(
        report
            .to_markdown()
            .contains("| 0.25 | 2 / 2 | 1 | 1 | 1 | 0 | 4 | [2, 19/2] |")
    );
}

#[test]
fn missing_loss_minima_respect_outside_domain_quantiles_and_unbounded_domains() {
    let payload = table([-10.0, 15.0, 50.0]);
    let (mut bounded, ids, cohort) = ledger(vec![payload.clone()], Some((0.0, 40.0)), 0.5);
    let (_, report) = assess(&mut bounded, cohort);
    for (q, lower) in member(&report, ids[0]).quantiles.iter().zip([5, 0, 5]) {
        assert_eq!(
            q.completion,
            Some(QuantileLossBounds {
                lower: rat(lower, 1),
                upper: Some(rat(25, 1))
            })
        );
    }
    let (mut unbounded, _, cohort) = ledger(vec![payload], None, 0.5);
    let (_, report) = assess(&mut unbounded, cohort);
    for level in report.levels {
        assert_eq!(level.observed_mean, None);
        assert_eq!(level.observed_status(), "no_resolved_outcomes");
        assert_eq!(
            level.whole_supplied,
            Some(QuantileLossBounds {
                lower: rat(0, 1),
                upper: None
            })
        );
    }
}

#[test]
fn sparse_tables_and_other_distributions_do_not_acquire_invented_quantiles() {
    let sparse = ForecastPayload::Quantiles {
        levels: vec![0.25, 0.75],
        values: vec![10.0, 20.0],
    };
    let gaussian = ForecastPayload::Gaussian {
        mean: 15.0,
        std_dev: 2.0,
    };
    let (mut ledger, ids, cohort) =
        ledger(vec![table([10.0, 15.0, 20.0]), sparse, gaussian], None, 0.5);
    publish(&mut ledger, ids[1], 18.0);
    let (_, report) = assess(&mut ledger, cohort);
    assert_eq!(
        report
            .levels
            .iter()
            .map(|s| (
                s.level,
                s.n_supplied,
                s.n_not_supplied,
                s.n_resolved,
                s.n_unresolved
            ))
            .collect::<Vec<_>>(),
        vec![(0.25, 2, 1, 1, 1), (0.5, 1, 2, 0, 1), (0.75, 2, 1, 1, 1)]
    );
    assert_eq!(member(&report, ids[1]).quantiles.len(), 2);
    assert!(member(&report, ids[2]).quantiles.is_empty());
    assert_eq!(report.members.len(), 3);
    assert!(
        report
            .to_json()
            .contains("\"n_quantile_members\":2,\"n_unsupported_distributions\":1")
    );
    assert!(
        report
            .to_json()
            .contains("\"distribution\":\"gaussian\",\"status\":\"unsupported_distribution\"")
    );
    assert!(
        report
            .to_markdown()
            .contains("unsupported distribution (gaussian)")
    );
}

#[test]
fn unrepresentable_losses_never_hide_a_member_or_block_the_parent_assessment() {
    let payload = |first| ForecastPayload::Quantiles {
        levels: vec![1.0e-300, 0.25, 0.5, 0.75],
        values: vec![first, 18.0, 18.0, 20.0],
    };
    let (mut ledger, ids, cohort) = ledger(vec![payload(0.0), payload(18.0)], None, 0.5);
    for id in &ids {
        publish(&mut ledger, *id, 18.0);
    }
    let (parent, report) = assess(&mut ledger, cohort);
    assert_eq!(
        parent.summary.interval_score.unwrap().observed_mean,
        Some(2.0)
    );
    let smallest = report.levels[0];
    assert_eq!(
        (
            smallest.n_supplied,
            smallest.n_resolved,
            smallest.n_scored,
            smallest.n_arithmetic_indeterminate
        ),
        (2, 2, 1, 1)
    );
    assert_eq!(smallest.observed_mean, None);
    assert_eq!(smallest.whole_supplied, None);
    assert_eq!(smallest.observed_status(), "arithmetic_indeterminate");
    assert_eq!(
        member(&report, ids[0]).quantiles[0].status(true),
        "arithmetic_indeterminate"
    );
    assert_eq!(member(&report, ids[1]).quantiles[0].loss, Some(rat(0, 1)));
    assert_eq!(report.levels[1].observed_mean, Some(rat(0, 1)));
    assert_eq!(
        ledger
            .assess(cohort, 101, OutcomeSelector::LatestAsOf)
            .unwrap(),
        &parent
    );
    assert!(report.to_json().contains("\"observed_status\":\"arithmetic_indeterminate\",\"observed_mean\":null,\"completion_status\":\"arithmetic_indeterminate\",\"whole_supplied\":null"));

    let (mut bounded, _, cohort) = ledger_for_tiny_missing(Some((0.0, 40.0)));
    let (_, report) = assess(&mut bounded, cohort);
    assert_eq!(report.levels[0].n_unresolved, 1);
    assert_eq!(report.levels[0].n_arithmetic_indeterminate, 1);
    assert_eq!(report.levels[0].whole_supplied, None);
    let (mut unbounded, _, cohort) = ledger_for_tiny_missing(None);
    let (_, report) = assess(&mut unbounded, cohort);
    assert_eq!(
        report.levels[0].whole_supplied,
        Some(QuantileLossBounds {
            lower: rat(0, 1),
            upper: None
        })
    );
}

fn ledger_for_tiny_missing(
    bounds: Option<(f64, f64)>,
) -> (ForecastLedger, Vec<ObjectId>, Digest32) {
    ledger(
        vec![ForecastPayload::Quantiles {
            levels: vec![1.0e-300, 0.25, 0.75],
            values: vec![0.0, 10.0, 20.0],
        }],
        bounds,
        0.5,
    )
}

#[test]
fn aggregate_capacity_failure_is_explicit_even_when_every_member_loss_fits() {
    let value = -2.0_f64.powi(126);
    let (mut ledger, ids, cohort) = ledger(vec![table([value; 3]); 2], None, 0.5);
    for id in &ids {
        publish(&mut ledger, *id, 0.0);
    }
    let (_, report) = assess(&mut ledger, cohort);
    let median = report.levels[1];
    assert_eq!(
        (
            median.n_supplied,
            median.n_scored,
            median.n_arithmetic_indeterminate
        ),
        (2, 2, 0)
    );
    assert_eq!(median.observed_mean, None);
    assert_eq!(median.observed_status(), "arithmetic_indeterminate");
    assert_eq!(median.whole_supplied, None);
    assert_eq!(median.completion_status(), "arithmetic_indeterminate");
    assert!(report.to_markdown().contains(
        "| 0.5 | 2 / 2 | 2 | 0 | 2 | 0 | arithmetic indeterminate | arithmetic indeterminate |"
    ));
}

#[test]
fn source_binding_and_saved_quantile_results_survive_restatements_and_detect_tampering() {
    let (mut ledger, ids, cohort) = ledger(vec![table([10.0, 15.0, 20.0])], Some((0.0, 40.0)), 0.5);
    publish(&mut ledger, ids[0], 18.0);
    let (parent, old) = assess(&mut ledger, cohort);
    let parent_bytes = parent.canonical_bytes();
    let prior_diagnostics = ledger.assessment_diagnostics(parent.digest).unwrap();
    let prior_coverage = ledger.coverage_diagnostics(parent.digest).unwrap();
    ledger
        .append_outcome(
            ids[0],
            102,
            102,
            OutcomeKind::Restated {
                value: 12.0,
                citation: citation("restatement"),
                supersedes: 1,
            },
        )
        .unwrap();
    let next_parent = ledger
        .assess(cohort, 103, OutcomeSelector::LatestAsOf)
        .unwrap()
        .clone();
    let changed = ledger.quantile_diagnostics(next_parent.digest).unwrap();
    assert_ne!(old.digest, changed.digest);
    assert_eq!(
        changed
            .levels
            .iter()
            .map(|s| s.observed_mean)
            .collect::<Vec<_>>(),
        vec![Some(rat(1, 1)), Some(rat(3, 1)), Some(rat(4, 1))]
    );
    assert_ne!(
        old.members[0].member.selected_event,
        changed.members[0].member.selected_event
    );
    let MemberOutcome::Resolved {
        citation: selected, ..
    } = &old.members[0].member.outcome
    else {
        panic!("resolved fixture")
    };
    assert_eq!(selected, &citation("original"));
    assert_eq!(ledger.quantile_diagnostics(parent.digest).unwrap(), old);
    assert_eq!(
        ledger
            .assess(cohort, 101, OutcomeSelector::LatestAsOf)
            .unwrap()
            .canonical_bytes(),
        parent_bytes
    );
    assert_eq!(
        ledger.assessment_diagnostics(parent.digest).unwrap(),
        prior_diagnostics
    );
    assert_eq!(
        ledger.coverage_diagnostics(parent.digest).unwrap(),
        prior_coverage
    );
    assert!(ledger.quantile_diagnostics(Digest32::default()).is_err());
    let first = ledger
        .assess(cohort, 103, OutcomeSelector::FirstPublished)
        .unwrap()
        .digest;
    assert_eq!(
        ledger.quantile_diagnostics(first).unwrap().members[0].quantiles,
        old.members[0].quantiles
    );
    let mut tampered = old.clone();
    if let MemberOutcome::Resolved { citation, .. } = &mut tampered.members[0].member.outcome {
        citation.sha256[0] ^= 1;
    }
    assert_ne!(blake3_hash(&tampered.canonical_bytes()), old.digest);
    let mut tampered = old.clone();
    tampered.members[0].quantiles[1].value = 14.0;
    assert_ne!(blake3_hash(&tampered.canonical_bytes()), old.digest);
    let mut tampered = old.clone();
    tampered.outcome_bounds = None;
    assert_ne!(blake3_hash(&tampered.canonical_bytes()), old.digest);
    assert_eq!(ledger.quantile_diagnostics(parent.digest).unwrap(), old);
}
