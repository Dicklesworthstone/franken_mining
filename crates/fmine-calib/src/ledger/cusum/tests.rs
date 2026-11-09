//! DESIGN: known-answer residual and durable-cohort controls for BD-051.
//! Determinism: DeterministicPerIsa.

use super::*;
use crate::{
    Assessment, AssessmentPolicy, CalibrationFlag, CohortSpec, CoverageMonitorPolicy,
    ForecastLedger, ForecastMetadata, MissingReason, OutcomeCitation, OutcomeKind, OutcomeSelector,
};
use fmine_core::hash::blake3_hash;

fn gaussian(mean: f64, std_dev: f64) -> ForecastPayload {
    ForecastPayload::Gaussian { mean, std_dev }
}

fn citation(vintage: &str) -> OutcomeCitation {
    OutcomeCitation {
        source: "synthetic".into(),
        vintage: vintage.into(),
        locator: "synthetic:cusum".into(),
        sha256: fmine_core::hash::sha256(vintage.as_bytes()).0,
    }
}

fn published(value: f64) -> OutcomeKind {
    OutcomeKind::Published {
        value,
        citation: citation("v1"),
    }
}

fn setup(payloads: Vec<ForecastPayload>) -> (ForecastLedger, CohortSpec, Vec<ObjectId>) {
    setup_with_metadata(payloads, false)
}

fn setup_with_metadata(
    payloads: Vec<ForecastPayload>,
    metadata: bool,
) -> (ForecastLedger, CohortSpec, Vec<ObjectId>) {
    let mut ledger = ForecastLedger::new();
    let ids: Vec<_> = payloads
        .into_iter()
        .enumerate()
        .map(|(i, payload)| {
            if metadata {
                ledger
                    .register_payload_with_metadata(
                        format!("cusum:{i}"),
                        1,
                        100,
                        payload,
                        ForecastMetadata {
                            family: "production".into(),
                            author_role: "model:synthetic".into(),
                            domain: "synthetic-domain".into(),
                            horizon: "one-month".into(),
                            site: "synthetic-site".into(),
                            quantity: "synthetic-production".into(),
                            unit: "synthetic-unit".into(),
                        },
                    )
                    .unwrap()
            } else {
                ledger
                    .register_payload(format!("cusum:{i}"), 1, 100, payload)
                    .unwrap()
            }
        })
        .collect();
    let spec = CohortSpec {
        name: "synthetic monthly residuals".into(),
        frozen_at_ns: 2,
        members: ids.clone(),
        resolution_rule: "synthetic original monthly observations".into(),
        eligible_sources: vec!["synthetic".into()],
        policy: AssessmentPolicy {
            cusum_monitor: Some(CusumMonitorPolicy {
                reference_value: 0.5,
                threshold: 2.0,
                support: "Non-overlapping synthetic monthly observations".into(),
                reference_assumption: "Independent standard-normal standardized forecast residuals"
                    .into(),
                tuning_basis: "Synthetic arithmetic example; no ARL claim".into(),
                order: ids.clone(),
            }),
            ..AssessmentPolicy::default()
        },
    };
    (ledger, spec, ids)
}

fn monitor(assessment: &Assessment) -> &CusumMonitorAssessment {
    assessment.cusum_monitor.as_ref().unwrap()
}

#[test]
fn gaussian_residuals_use_each_frozen_mean_and_scale_and_retain_sources() {
    let references = [(10.0, 2.0), (100.0, 4.0), (-10.0, 0.5), (0.0, 8.0)];
    let values = [13.0, 106.0, -10.5, -20.0];
    let (mut ledger, spec, ids) = setup(references.iter().map(|(m, s)| gaussian(*m, *s)).collect());
    let cohort = ledger.freeze_cohort(spec).unwrap();
    for (id, value) in ids.iter().zip(values) {
        ledger
            .append_outcome(*id, 101, 102, published(value))
            .unwrap();
    }
    let report = ledger
        .assess(cohort, 103, OutcomeSelector::FirstPublished)
        .unwrap();
    let evidence = monitor(report);
    assert_eq!(
        (
            evidence.snapshot.n,
            evidence.snapshot.upper,
            evidence.snapshot.lower
        ),
        (4, 0.0, 2.5)
    );
    assert_eq!(
        (evidence.snapshot.max_upper, evidence.snapshot.max_lower),
        (2.0, 2.5)
    );
    assert_eq!(
        (
            evidence.snapshot.first_upper_crossing,
            evidence.snapshot.first_lower_crossing
        ),
        (Some(2), Some(4))
    );
    let z = evidence
        .observations
        .iter()
        .map(|row| row.standardized_residual)
        .collect::<Vec<_>>();
    assert_eq!(z, [1.5, 1.5, -1.0, -2.5]);
    for (i, observation) in evidence.observations.iter().enumerate() {
        assert_eq!((observation.mean, observation.std_dev), references[i]);
        assert_eq!(observation.member.forecast_id, ids[i]);
        assert!(matches!(&observation.member.outcome,
            MemberOutcome::Resolved { citation: source, .. } if source == &citation("v1")));
        assert_eq!(observation.snapshot.n, i as u64 + 1);
    }
    assert!(evidence.blocked_by.is_none());
    assert_eq!((evidence.n_deferred, evidence.n_deferred_resolved), (0, 0));
}

#[test]
fn missing_member_blocks_declared_order_and_later_publication_unblocks_it() {
    let (mut ledger, mut spec, ids) = setup(vec![gaussian(0.0, 1.0); 3]);
    spec.policy.cusum_monitor.as_mut().unwrap().order = vec![ids[2], ids[0], ids[1]];
    let cohort = ledger.freeze_cohort(spec).unwrap();
    ledger
        .append_outcome(ids[2], 101, 102, published(1.5))
        .unwrap();
    ledger
        .append_outcome(ids[1], 101, 102, published(1.5))
        .unwrap();
    ledger
        .append_outcome(
            ids[0],
            101,
            102,
            OutcomeKind::Missing {
                reason: MissingReason::NotPublished,
            },
        )
        .unwrap();
    let initial = ledger
        .assess(cohort, 101, OutcomeSelector::LatestAsOf)
        .unwrap()
        .clone();
    assert_eq!(monitor(&initial).snapshot.n, 0);
    assert_eq!(
        monitor(&initial).blocked_by.as_ref().unwrap().forecast_id,
        ids[2]
    );
    let before = ledger
        .assess(cohort, 103, OutcomeSelector::LatestAsOf)
        .unwrap()
        .clone();
    let evidence = monitor(&before);
    assert_eq!((evidence.snapshot.n, evidence.snapshot.upper), (1, 1.0));
    assert_eq!((evidence.n_deferred, evidence.n_deferred_resolved), (1, 1));
    let blocker = evidence.blocked_by.as_ref().unwrap();
    assert_eq!(blocker.forecast_id, ids[0]);
    assert!(blocker.selected_event.is_some());
    assert_eq!(
        blocker.outcome,
        MemberOutcome::Unresolved {
            reason: MissingReason::NotPublished
        }
    );
    ledger
        .append_outcome(ids[0], 104, 105, published(1.5))
        .unwrap();
    let after = ledger
        .assess(cohort, 106, OutcomeSelector::FirstPublished)
        .unwrap();
    assert_eq!(monitor(after).snapshot.first_upper_crossing, Some(2));
    assert_eq!(monitor(after).snapshot.upper, 3.0);
    assert!(monitor(after).blocked_by.is_none());
    assert_eq!(
        monitor(after)
            .observations
            .iter()
            .map(|row| row.member.forecast_id)
            .collect::<Vec<_>>(),
        vec![ids[2], ids[0], ids[1]]
    );
    assert_eq!(
        ledger
            .assess(cohort, 103, OutcomeSelector::LatestAsOf)
            .unwrap(),
        &before
    );
}

#[test]
fn original_evidence_and_alarm_survive_restatements_and_aggregate_recovery() {
    let (mut ledger, spec, ids) = setup(vec![gaussian(0.0, 1.0); 6]);
    let cohort = ledger.freeze_cohort(spec).unwrap();
    for (i, id) in ids.iter().enumerate() {
        ledger
            .append_outcome(*id, 101, 102, published(if i < 2 { 1.5 } else { 0.0 }))
            .unwrap();
    }
    let original = ledger
        .assess(cohort, 103, OutcomeSelector::FirstPublished)
        .unwrap()
        .clone();
    assert_eq!(monitor(&original).snapshot.upper, 0.0);
    assert_eq!(monitor(&original).snapshot.first_crossing(), Some(2));
    for id in &ids[..2] {
        ledger
            .append_outcome(
                *id,
                104,
                105,
                OutcomeKind::Restated {
                    value: 0.0,
                    citation: citation("v2"),
                    supersedes: 1,
                },
            )
            .unwrap();
    }
    let latest = ledger
        .assess(cohort, 106, OutcomeSelector::LatestAsOf)
        .unwrap()
        .clone();
    assert_eq!(latest.summary.coverage.unwrap().inside, 6);
    assert_eq!(monitor(&latest), monitor(&original));
    assert_eq!(
        monitor(&latest).observations[0].member.selected_event,
        Some((1, ledger.outcomes(ids[0])[0].digest))
    );
    assert_eq!(
        ledger
            .assess(cohort, 103, OutcomeSelector::FirstPublished)
            .unwrap(),
        &original
    );
}

#[test]
fn monitor_suppresses_only_a_positive_calibration_label() {
    let flags = |mode: u8, enabled: bool| {
        let (mut ledger, mut spec, ids) = setup(vec![gaussian(0.0, 1.0); 40]);
        spec.policy.correlation_range = (0.0, 0.0);
        if !enabled {
            spec.policy.cusum_monitor = None;
        }
        let cohort = ledger.freeze_cohort(spec).unwrap();
        for (i, id) in ids.iter().enumerate() {
            let value = match mode {
                0 if i < 4 => 2.0,
                0 if i >= 36 => -2.0,
                0 => 0.0,
                1 => 2.0,
                _ if i % 2 == 0 => 2.0,
                _ => -2.0,
            };
            ledger
                .append_outcome(*id, 101, 102, published(value))
                .unwrap();
        }
        ledger
            .assess(cohort, 103, OutcomeSelector::FirstPublished)
            .unwrap()
            .summary
            .flag
    };
    assert_eq!(flags(0, false), CalibrationFlag::Calibrated);
    assert_eq!(flags(0, true), CalibrationFlag::Descriptive);
    assert_eq!(flags(1, true), CalibrationFlag::Biased);
    assert_eq!(flags(2, true), CalibrationFlag::Overconfident);
}

#[test]
fn invalid_policies_and_nongaussian_members_refuse_without_freezing() {
    let (ledger, spec, ids) = setup(vec![gaussian(0.0, 1.0); 2]);
    let valid = spec.policy.cusum_monitor.as_ref().unwrap();
    let mut invalid = Vec::new();
    for value in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        invalid.push(CusumMonitorPolicy {
            reference_value: value,
            ..valid.clone()
        });
        invalid.push(CusumMonitorPolicy {
            threshold: value,
            ..valid.clone()
        });
    }
    invalid.extend([
        CusumMonitorPolicy {
            support: " ".into(),
            ..valid.clone()
        },
        CusumMonitorPolicy {
            reference_assumption: "".into(),
            ..valid.clone()
        },
        CusumMonitorPolicy {
            tuning_basis: "no\nhidden fields".into(),
            ..valid.clone()
        },
        CusumMonitorPolicy {
            order: vec![],
            ..valid.clone()
        },
        CusumMonitorPolicy {
            order: vec![ids[0], ids[0]],
            ..valid.clone()
        },
        CusumMonitorPolicy {
            order: vec![ids[0]],
            ..valid.clone()
        },
    ]);
    for policy in invalid {
        let mut copy = ledger.clone();
        let mut bad = spec.clone();
        bad.policy.cusum_monitor = Some(policy);
        assert!(copy.freeze_cohort(bad).is_err());
        assert!(copy.cohorts.is_empty());
        assert_eq!(copy.latest_record_ns, Some(1));
        assert!(copy.freeze_cohort(spec.clone()).is_ok());
    }
    for unsupported in [
        ForecastPayload::Bernoulli { probability: 0.5 },
        ForecastPayload::Interval {
            lo: -1.0,
            hi: 1.0,
            nominal: Some(0.8),
        },
        ForecastPayload::Quantiles {
            levels: vec![0.1, 0.9],
            values: vec![-1.0, 1.0],
        },
        ForecastPayload::Empirical {
            samples: vec![-1.0, 1.0],
        },
    ] {
        let (mut ledger, spec, _) = setup(vec![unsupported]);
        assert!(
            ledger
                .freeze_cohort(spec)
                .unwrap_err()
                .what
                .contains("Gaussian")
        );
        assert!(ledger.cohorts.is_empty());
    }
    let mut copy = ledger;
    let mut bad = spec;
    bad.policy.event_log_floor = Some(0.01);
    assert!(
        copy.freeze_cohort(bad)
            .unwrap_err()
            .what
            .contains("separate cohorts")
    );
}

#[test]
fn cusum_composes_with_coverage_bounds_and_declared_metadata() {
    for metadata in [false, true] {
        for bounds in [false, true] {
            for coverage in [false, true] {
                let (mut ledger, mut spec, ids) =
                    setup_with_metadata(vec![gaussian(0.0, 1.0); 2], metadata);
                if bounds {
                    spec.policy.gaussian_crps_bounds = true;
                    spec.policy.outcome_bounds = Some((-4.0, 4.0));
                }
                if coverage {
                    spec.policy.coverage_monitor = Some(CoverageMonitorPolicy {
                        alarm_alpha: 0.05,
                        support: "Synthetic independent months".into(),
                        conditional_null: "Assumed conditional hit probability 0.8".into(),
                        order: ids.clone(),
                    });
                }
                let cohort = ledger.freeze_cohort(spec).unwrap();
                ledger
                    .append_outcome(ids[0], 101, 102, published(1.5))
                    .unwrap();
                let report = ledger
                    .assess(cohort, 103, OutcomeSelector::FirstPublished)
                    .unwrap()
                    .clone();
                assert_eq!(monitor(&report).snapshot.n, 1);
                assert_eq!(report.coverage_monitor.is_some(), coverage);
                assert_eq!(report.summary.gaussian_crps_bounds, bounds);
                assert_eq!(&report.canonical_bytes()[..10], b"FMNEASM1\x05\x00");
                let segmented = ledger.segmented_assessment(report.digest).unwrap();
                assert_eq!(segmented.groups.len(), usize::from(metadata) * 2);
            }
        }
    }
}

#[test]
fn policy_and_observation_changes_bind_the_assessment_digest() {
    let (ledger, spec, ids) = setup(vec![gaussian(0.0, 1.0); 3]);
    let mut specifications = vec![spec.clone(); 7];
    specifications[1]
        .policy
        .cusum_monitor
        .as_mut()
        .unwrap()
        .reference_value = 0.25;
    specifications[2]
        .policy
        .cusum_monitor
        .as_mut()
        .unwrap()
        .threshold = 3.0;
    specifications[3]
        .policy
        .cusum_monitor
        .as_mut()
        .unwrap()
        .support
        .push_str(" changed");
    specifications[4]
        .policy
        .cusum_monitor
        .as_mut()
        .unwrap()
        .reference_assumption
        .push_str(" changed");
    specifications[5]
        .policy
        .cusum_monitor
        .as_mut()
        .unwrap()
        .tuning_basis
        .push_str(" changed");
    specifications[6]
        .policy
        .cusum_monitor
        .as_mut()
        .unwrap()
        .order
        .reverse();
    let digests: std::collections::BTreeSet<_> = specifications
        .into_iter()
        .map(|spec| ledger.clone().freeze_cohort(spec).unwrap())
        .collect();
    assert_eq!(digests.len(), 7);
    let mut ledger = ledger;
    let cohort = ledger.freeze_cohort(spec).unwrap();
    ledger
        .append_outcome(ids[0], 101, 102, published(2.5))
        .unwrap();
    ledger
        .append_outcome(ids[2], 101, 102, published(-2.5))
        .unwrap();
    let original = ledger
        .assess(cohort, 103, OutcomeSelector::FirstPublished)
        .unwrap()
        .clone();
    let mut edits = vec![original.clone(); 11];
    edits[0].cusum_monitor.as_mut().unwrap().observations[0].standardized_residual += 0.25;
    edits[1].cusum_monitor.as_mut().unwrap().observations[0].mean += 0.25;
    edits[2].cusum_monitor.as_mut().unwrap().observations[0].std_dev += 0.25;
    edits[3].cusum_monitor.as_mut().unwrap().observations[0]
        .member
        .selected_event = None;
    edits[4].cusum_monitor.as_mut().unwrap().observations[0]
        .snapshot
        .first_upper_crossing = None;
    edits[5]
        .cusum_monitor
        .as_mut()
        .unwrap()
        .snapshot
        .first_upper_crossing = None;
    edits[6].cusum_monitor.as_mut().unwrap().snapshot.max_lower += 1.0;
    edits[7].cusum_monitor.as_mut().unwrap().blocked_by = None;
    edits[8].cusum_monitor.as_mut().unwrap().n_deferred = 2;
    edits[9].cusum_monitor.as_mut().unwrap().n_deferred_resolved = 0;
    edits[10]
        .cusum_monitor
        .as_mut()
        .unwrap()
        .policy
        .tuning_basis
        .push_str(" changed");
    for edit in edits {
        assert_ne!(blake3_hash(&edit.canonical_bytes()), original.digest);
    }
}

#[test]
fn numerical_refusal_does_not_seal_or_cache_assessment() {
    let (mut ledger, mut spec, ids) = setup(vec![gaussian(0.0, 1.0)]);
    spec.policy.alpha = 0.9;
    spec.policy.cusum_monitor.as_mut().unwrap().reference_value = f64::MAX;
    let cohort = ledger.freeze_cohort(spec).unwrap();
    ledger
        .append_outcome(ids[0], 101, 102, published(6e307))
        .unwrap();
    assert_eq!(
        ledger
            .assess(cohort, 200, OutcomeSelector::FirstPublished)
            .unwrap_err()
            .code,
        "FMINE-REF-207"
    );
    assert!(ledger.assessments.is_empty());
    assert!(ledger.assessed_through.is_empty());
    assert_eq!(ledger.sealed_through_ns, None);
    assert_eq!(ledger.latest_record_ns, Some(102));
    ledger
        .register_payload("after failed assessment", 103, 300, gaussian(0.0, 1.0))
        .unwrap();
}
