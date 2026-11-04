//! Independent coverage-mixture and durable-ledger regressions (BD-051).
//! Determinism: DeterministicPerIsa

use super::*;
use crate::{
    Assessment, AssessmentPolicy, CohortSpec, ForecastLedger, MissingReason, OutcomeCitation,
    OutcomeKind, OutcomeSelector,
};
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash};

fn interval() -> ForecastPayload {
    ForecastPayload::Interval {
        lo: 0.0,
        hi: 1.0,
        nominal: Some(0.8),
    }
}

fn citation(vintage: &str) -> OutcomeCitation {
    OutcomeCitation {
        source: "synthetic".into(),
        vintage: vintage.into(),
        locator: "synthetic:monitor".into(),
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
    let mut ledger = ForecastLedger::new();
    let ids: Vec<_> = payloads
        .into_iter()
        .enumerate()
        .map(|(i, payload)| {
            ledger
                .register_payload(format!("monitor:{i}"), 1, 100, payload)
                .unwrap()
        })
        .collect();
    let spec = CohortSpec {
        name: "monthly support".into(),
        frozen_at_ns: 2,
        members: ids.clone(),
        resolution_rule: "synthetic original monthly totals".into(),
        eligible_sources: vec!["synthetic".into()],
        policy: AssessmentPolicy {
            coverage_monitor: Some(CoverageMonitorPolicy {
                alarm_alpha: 0.05,
                support: "Non-overlapping synthetic monthly reconciled domain totals".into(),
                conditional_null:
                    "Assume P(hit_t | every prior monthly outcome) = 0.8 in frozen month order"
                        .into(),
                order: ids.clone(),
            }),
            ..AssessmentPolicy::default()
        },
    };
    (ledger, spec, ids)
}

fn monitor(assessment: &Assessment) -> &CoverageMonitorAssessment {
    assessment.coverage_monitor.as_ref().unwrap()
}

#[test]
fn plan_gb08_counts_match_independent_beta_integral() {
    for (n, k) in [(12, 7), (24, 14), (60, 36)] {
        let (mut ledger, spec, ids) = setup(vec![interval(); n]);
        let cohort = ledger.freeze_cohort(spec).unwrap();
        for (i, id) in ids.iter().enumerate() {
            ledger
                .append_outcome(*id, 101, 102, published(if i < k { 0.5 } else { 2.0 }))
                .unwrap();
        }
        let report = ledger
            .assess(cohort, 103, OutcomeSelector::FirstPublished)
            .unwrap();
        let evidence = monitor(report);
        let binomial = (0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64);
        // FACT: B(k+1,n-k+1) = 1 / ((n+1) binom(n,k)), independent of sequential log updates.
        let oracle = 1.0
            / ((n + 1) as f64 * binomial * 0.8_f64.powi(k as i32) * 0.2_f64.powi((n - k) as i32));
        assert!((evidence.snapshot.e_value().unwrap() / oracle - 1.0).abs() < 1e-12);
        assert_eq!(
            (evidence.snapshot.n, evidence.snapshot.hits),
            (n as u64, k as u64)
        );
        assert_eq!(evidence.snapshot.alarmed(), n == 60);
        assert_eq!(
            evidence
                .observations
                .iter()
                .map(|o| o.member.forecast_id)
                .collect::<Vec<_>>(),
            ids
        );
        assert!(evidence.blocked_by.is_none());
        assert_eq!((evidence.n_deferred, evidence.n_deferred_resolved), (0, 0));
    }
}

#[test]
fn originals_and_first_crossing_survive_restatements_and_lower_current_evidence() {
    let (mut ledger, spec, ids) = setup(vec![interval(); 12]);
    let cohort = ledger.freeze_cohort(spec).unwrap();
    for (i, id) in ids.iter().enumerate() {
        ledger
            .append_outcome(*id, 101, 102, published(if i < 4 { 2.0 } else { 0.5 }))
            .unwrap();
    }
    let before = ledger
        .assess(cohort, 103, OutcomeSelector::FirstPublished)
        .unwrap()
        .clone();
    assert_eq!(monitor(&before).snapshot.first_crossing, Some(3));
    assert!(monitor(&before).snapshot.e_value().unwrap() < 20.0);
    assert!(monitor(&before).snapshot.max_e_value().unwrap() > 100.0);
    for id in &ids[..4] {
        ledger
            .append_outcome(
                *id,
                104,
                105,
                OutcomeKind::Restated {
                    value: 0.5,
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
    assert_eq!(latest.summary.coverage.unwrap().inside, 12);
    assert_eq!(monitor(&latest), monitor(&before));
    assert_eq!(
        monitor(&latest).observations[0].member.selected_event,
        Some((1, ledger.outcomes(ids[0])[0].digest))
    );
    assert_eq!(
        ledger
            .assess(cohort, 103, OutcomeSelector::FirstPublished)
            .unwrap(),
        &before
    );
}

#[test]
fn unresolved_member_blocks_later_originals_in_frozen_order_and_retains_missing_event() {
    let (mut ledger, mut spec, ids) = setup(vec![interval(); 3]);
    spec.policy.coverage_monitor.as_mut().unwrap().order = vec![ids[2], ids[0], ids[1]];
    let cohort = ledger.freeze_cohort(spec).unwrap();
    ledger
        .append_outcome(ids[2], 101, 102, published(0.5))
        .unwrap();
    ledger
        .append_outcome(ids[1], 101, 102, published(2.0))
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
    let report = ledger
        .assess(cohort, 103, OutcomeSelector::LatestAsOf)
        .unwrap();
    assert_eq!(report.summary.n_resolved, 2);
    let evidence = monitor(report);
    assert_eq!((evidence.snapshot.n, evidence.snapshot.hits), (1, 1));
    assert_eq!(evidence.observations[0].member.forecast_id, ids[2]);
    assert_eq!((evidence.n_deferred, evidence.n_deferred_resolved), (1, 1));
    let blocker = evidence.blocked_by.as_ref().unwrap();
    assert_eq!(blocker.forecast_id, ids[0]);
    assert!(matches!(
        blocker.outcome,
        MemberOutcome::Unresolved {
            reason: MissingReason::NotPublished
        }
    ));
    assert!(blocker.selected_event.is_some());
    ledger
        .append_outcome(ids[0], 104, 105, published(0.5))
        .unwrap();
    let after = monitor(
        ledger
            .assess(cohort, 106, OutcomeSelector::FirstPublished)
            .unwrap(),
    );
    assert_eq!((after.snapshot.n, after.snapshot.hits), (3, 2));
    assert!(after.blocked_by.is_none());
    assert_eq!(
        after
            .observations
            .iter()
            .map(|o| o.member.forecast_id)
            .collect::<Vec<_>>(),
        vec![ids[2], ids[0], ids[1]]
    );
}

#[test]
fn initial_prefix_and_bitemporal_cut_do_not_consume_unrecorded_outcomes() {
    let (mut ledger, spec, ids) = setup(vec![interval(); 2]);
    let cohort = ledger.freeze_cohort(spec).unwrap();
    ledger
        .append_outcome(ids[0], 101, 102, published(2.0))
        .unwrap();
    ledger
        .append_outcome(ids[1], 101, 102, published(2.0))
        .unwrap();
    let evidence = monitor(
        ledger
            .assess(cohort, 101, OutcomeSelector::FirstPublished)
            .unwrap(),
    );
    assert_eq!(evidence.snapshot.n, 0);
    assert_eq!(evidence.snapshot.e_value(), Some(1.0));
    assert_eq!(evidence.snapshot.max_e_value(), Some(1.0));
    assert_eq!(evidence.blocked_by.as_ref().unwrap().forecast_id, ids[0]);
    assert_eq!((evidence.n_deferred, evidence.n_deferred_resolved), (1, 0));
}

#[test]
fn malformed_monitor_policies_refuse_without_mutating_authority() {
    let (ledger, spec, ids) = setup(vec![interval(); 2]);
    let valid = spec.policy.coverage_monitor.as_ref().unwrap().clone();
    let mut invalid = Vec::new();
    for alarm_alpha in [0.0, 1.0, -0.1, f64::NAN, f64::INFINITY] {
        invalid.push(CoverageMonitorPolicy {
            alarm_alpha,
            ..valid.clone()
        });
    }
    invalid.push(CoverageMonitorPolicy {
        support: "  \n".into(),
        ..valid.clone()
    });
    invalid.push(CoverageMonitorPolicy {
        conditional_null: " ".into(),
        ..valid.clone()
    });
    for order in [
        vec![],
        vec![ids[0]],
        vec![ids[0], ids[0]],
        vec![
            ids[0],
            ObjectId::from_canonical_bytes(fmine_core::id::PrefixKind::Forecast, b"foreign"),
        ],
    ] {
        invalid.push(CoverageMonitorPolicy {
            order,
            ..valid.clone()
        });
    }
    for policy in invalid {
        let mut trial = ledger.clone();
        let mut bad = spec.clone();
        bad.frozen_at_ns = 50;
        bad.policy.coverage_monitor = Some(policy);
        assert_eq!(trial.freeze_cohort(bad).unwrap_err().code, "FMINE-REF-109");
        assert_eq!(trial.latest_record_ns, Some(1));
        assert!(trial.cohorts.is_empty());
        assert!(trial.cohort_names.is_empty());
        assert!(trial.assessments.is_empty());
        assert_eq!(trial.sealed_through_ns, None);
        trial
            .freeze_cohort(spec.clone())
            .expect("failed freeze must not consume name or advance time");
    }
}

#[test]
fn missing_probability_discrete_payloads_and_nearby_nominals_are_not_exact_nulls() {
    let bad_payloads = [
        ForecastPayload::Interval {
            lo: 0.0,
            hi: 1.0,
            nominal: None,
        },
        ForecastPayload::Interval {
            lo: 0.0,
            hi: 1.0,
            nominal: Some(0.8 + 1e-13),
        },
        ForecastPayload::Bernoulli { probability: 0.8 },
        ForecastPayload::Empirical {
            samples: vec![0.0, 1.0, 2.0],
        },
        ForecastPayload::Quantiles {
            levels: vec![0.1 + 1e-13, 0.9],
            values: vec![0.0, 1.0],
        },
    ];
    for payload in bad_payloads {
        let (mut ledger, spec, _) = setup(vec![payload]);
        assert_eq!(
            ledger.freeze_cohort(spec.clone()).unwrap_err().code,
            "FMINE-REF-109"
        );
        assert_eq!(ledger.latest_record_ns, Some(1));
        assert!(ledger.cohorts.is_empty());
        let mut descriptive = spec;
        descriptive.policy.coverage_monitor = None;
        ledger
            .freeze_cohort(descriptive)
            .expect("ordinary descriptive scoring remains supported");
    }
}

#[test]
fn gaussian_and_exact_central_quantiles_are_admitted_without_tolerance_aliasing() {
    let quantiles = ForecastPayload::Quantiles {
        levels: vec![0.1 - 1e-13, 0.1, 0.9],
        values: vec![-10.0, 0.0, 1.0],
    };
    let (mut ledger, spec, ids) = setup(vec![
        ForecastPayload::Gaussian {
            mean: 0.0,
            std_dev: 1.0,
        },
        quantiles,
    ]);
    let cohort = ledger.freeze_cohort(spec).unwrap();
    ledger
        .append_outcome(ids[0], 101, 102, published(0.0))
        .unwrap();
    ledger
        .append_outcome(ids[1], 101, 102, published(-1.0))
        .unwrap();
    let evidence = monitor(
        ledger
            .assess(cohort, 103, OutcomeSelector::FirstPublished)
            .unwrap(),
    );
    assert_eq!(evidence.observations[1].interval, (0.0, 1.0));
    assert!(!evidence.observations[1].hit);
    assert_eq!(evidence.snapshot.hits, 1);
}

#[test]
fn correlation_sensitivity_never_changes_conditional_evidence() {
    let (ledger, spec, ids) = setup(vec![interval(); 12]);
    let mut results = Vec::new();
    for rho in [0.0, 0.9] {
        let mut trial = ledger.clone();
        let mut policy = spec.clone();
        policy.policy.correlation_range = (rho, rho);
        let cohort = trial.freeze_cohort(policy).unwrap();
        for (i, id) in ids.iter().enumerate() {
            trial
                .append_outcome(*id, 101, 102, published(if i < 7 { 0.5 } else { 2.0 }))
                .unwrap();
        }
        results.push(
            trial
                .assess(cohort, 103, OutcomeSelector::FirstPublished)
                .unwrap()
                .clone(),
        );
    }
    assert_ne!(
        results[0].summary.effective_n,
        results[1].summary.effective_n
    );
    assert_eq!(monitor(&results[0]), monitor(&results[1]));
}

#[test]
fn failed_assessment_does_not_cache_or_seal_any_prefix() {
    let (mut ledger, spec, ids) = setup(vec![interval()]);
    let cohort = ledger.freeze_cohort(spec).unwrap();
    ledger
        .append_outcome(ids[0], 101, 102, published(f64::MAX))
        .unwrap();
    assert!(
        ledger
            .assess(cohort, 200, OutcomeSelector::FirstPublished)
            .is_err()
    );
    assert!(ledger.assessments.is_empty());
    assert!(ledger.assessed_through.is_empty());
    assert_eq!(ledger.sealed_through_ns, None);
    assert_eq!(ledger.latest_record_ns, Some(102));
    ledger
        .register_payload("after failed assessment", 103, 300, interval())
        .unwrap();
}

#[test]
fn configuration_and_every_monitor_evidence_component_are_digest_bound() {
    let (ledger, spec, ids) = setup(vec![interval(); 3]);
    let mut policies = vec![spec.clone(); 4];
    policies[1]
        .policy
        .coverage_monitor
        .as_mut()
        .unwrap()
        .support
        .push_str(" other");
    policies[2]
        .policy
        .coverage_monitor
        .as_mut()
        .unwrap()
        .conditional_null
        .push_str(" other");
    policies[3]
        .policy
        .coverage_monitor
        .as_mut()
        .unwrap()
        .order
        .swap(0, 1);
    let digests: Vec<_> = policies
        .into_iter()
        .map(|policy| ledger.clone().freeze_cohort(policy).unwrap())
        .collect();
    assert_eq!(
        digests
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );
    let mut ledger = ledger;
    let cohort = ledger.freeze_cohort(spec).unwrap();
    ledger
        .append_outcome(ids[0], 101, 102, published(0.5))
        .unwrap();
    ledger
        .append_outcome(ids[2], 101, 102, published(2.0))
        .unwrap();
    let original = ledger
        .assess(cohort, 103, OutcomeSelector::FirstPublished)
        .unwrap()
        .clone();
    let mut edits = vec![original.clone(); 9];
    edits[0].coverage_monitor.as_mut().unwrap().observations[0].hit = false;
    edits[1].coverage_monitor.as_mut().unwrap().observations[0]
        .interval
        .1 = 2.0;
    edits[2].coverage_monitor.as_mut().unwrap().observations[0]
        .member
        .selected_event = None;
    edits[3]
        .coverage_monitor
        .as_mut()
        .unwrap()
        .blocked_by
        .as_mut()
        .unwrap()
        .outcome = MemberOutcome::Unresolved {
        reason: MissingReason::Divested,
    };
    edits[4].coverage_monitor.as_mut().unwrap().n_deferred = 2;
    edits[5]
        .coverage_monitor
        .as_mut()
        .unwrap()
        .n_deferred_resolved = 0;
    edits[6]
        .coverage_monitor
        .as_mut()
        .unwrap()
        .snapshot
        .max_log_e_value += 1.0;
    edits[7]
        .coverage_monitor
        .as_mut()
        .unwrap()
        .snapshot
        .first_crossing = Some(1);
    edits[8]
        .coverage_monitor
        .as_mut()
        .unwrap()
        .policy
        .alarm_alpha = 0.1;
    for edited in edits {
        assert_ne!(blake3_hash(&edited.canonical_bytes()), original.digest);
    }
}

#[test]
fn unmonitored_assessment_matches_the_complete_legacy_v1_frame() {
    let mut ledger = ForecastLedger::new();
    let id = ledger
        .register_payload(
            "golden",
            1,
            100,
            ForecastPayload::Bernoulli { probability: 0.25 },
        )
        .unwrap();
    let cohort = ledger
        .freeze_cohort(CohortSpec {
            name: "golden".into(),
            frozen_at_ns: 2,
            members: vec![id],
            resolution_rule: "binary event".into(),
            eligible_sources: vec!["synthetic".into()],
            policy: AssessmentPolicy {
                correlation_range: (0.0, 0.0),
                ..AssessmentPolicy::default()
            },
        })
        .unwrap();
    let citation = OutcomeCitation {
        source: "synthetic".into(),
        vintage: "v1".into(),
        locator: "synthetic:event".into(),
        sha256: [7; 32],
    };
    ledger
        .append_outcome(
            id,
            101,
            102,
            OutcomeKind::Published {
                value: 1.0,
                citation,
            },
        )
        .unwrap();
    let event = ledger.outcomes(id)[0].digest;
    let assessment = ledger
        .assess(cohort, 103, OutcomeSelector::FirstPublished)
        .unwrap();
    assert!(assessment.coverage_monitor.is_none());
    assert_eq!(
        cohort.to_hex(),
        "9aabc96a1dd23f0ee2e95c0f02924b21cab9eeb5e9fa647d01cad06ce98107d2"
    );
    // DESIGN: spell out the pre-monitor ASM1/v1 frame, including all absence markers and exact bits.
    let mut legacy = Encoder::new_frame(*b"ASM1", 1);
    legacy.put_fixed(&cohort.0);
    legacy.put_i64(103);
    legacy.put_u8(0);
    legacy.put_u64(1);
    legacy.put_fixed(&id.bytes);
    legacy.put_u8(1);
    legacy.put_u64(1);
    legacy.put_fixed(&event.0);
    legacy.put_u8(0);
    legacy.put_u64(1.0_f64.to_bits());
    legacy.put_u64(1);
    legacy.put_fixed(&event.0);
    legacy.put_str("synthetic");
    legacy.put_str("v1");
    legacy.put_str("synthetic:event");
    legacy.put_fixed(&[7; 32]);
    for n in [1, 1, 0, 0] {
        legacy.put_u64(n);
    }
    legacy.put_u8(0); // no interval coverage
    legacy.put_u8(0); // no CRPS
    legacy.put_u8(0); // no interval score
    legacy.put_u8(1);
    legacy.put_u64(1);
    legacy.put_u8(1);
    legacy.put_u64(0.5625_f64.to_bits());
    legacy.put_u64(0.5625_f64.to_bits());
    legacy.put_u8(1);
    legacy.put_u64(0.5625_f64.to_bits());
    for x in [1.0_f64, 1.0, 0.0, 0.0] {
        legacy.put_u64(x.to_bits());
    }
    legacy.put_u8(0);
    legacy.put_u8(1);
    assert_eq!(assessment.canonical_bytes(), legacy.finish());
    assert_eq!(
        assessment.digest.to_hex(),
        "7d8c61e468bab0d43ed0cbe695da8dc63a0c11a94a1fed2d5628d9229ede0af9"
    );
    assert_ne!(assessment.digest, Digest32::default());
}

#[test]
fn an_early_alarm_precludes_positive_calibration_after_aggregate_recovery() {
    let outcomes: Vec<_> = (0..50)
        .map(|i| {
            if (4..44).contains(&i) {
                0.5
            } else if i % 2 == 0 {
                2.0
            } else {
                -1.0
            }
        })
        .collect();
    let (ledger, mut spec, ids) = setup(vec![interval(); outcomes.len()]);
    spec.policy.correlation_range = (0.0, 0.0);
    for enabled in [false, true] {
        let mut trial = ledger.clone();
        let mut policy = spec.clone();
        if !enabled {
            policy.policy.coverage_monitor = None;
        }
        let cohort = trial.freeze_cohort(policy).unwrap();
        for (id, value) in ids.iter().zip(&outcomes) {
            trial
                .append_outcome(*id, 101, 102, published(*value))
                .unwrap();
        }
        let report = trial
            .assess(cohort, 103, OutcomeSelector::FirstPublished)
            .unwrap();
        assert_eq!(
            report.summary.coverage.unwrap().observed_fraction,
            Some(0.8)
        );
        if enabled {
            assert_eq!(monitor(report).snapshot.first_crossing, Some(3));
            assert!(monitor(report).snapshot.e_value().unwrap() < 20.0);
            assert_eq!(report.summary.flag, crate::CalibrationFlag::Descriptive);
        } else {
            assert_eq!(report.summary.flag, crate::CalibrationFlag::Calibrated);
        }
    }
}

#[test]
fn coverage_alarm_preserves_existing_overconfidence_and_bias_findings() {
    for expected in [
        crate::CalibrationFlag::Overconfident,
        crate::CalibrationFlag::Biased,
    ] {
        let (mut ledger, mut spec, ids) = setup(vec![
            ForecastPayload::Gaussian {
                mean: 0.0,
                std_dev: 1.0
            };
            40
        ]);
        spec.policy.correlation_range = (0.0, 0.0);
        let cohort = ledger.freeze_cohort(spec).unwrap();
        for (i, id) in ids.iter().enumerate() {
            let value = if expected == crate::CalibrationFlag::Biased || i % 2 == 0 {
                3.0
            } else {
                -3.0
            };
            ledger
                .append_outcome(*id, 101, 102, published(value))
                .unwrap();
        }
        let report = ledger
            .assess(cohort, 103, OutcomeSelector::FirstPublished)
            .unwrap();
        assert!(monitor(report).snapshot.alarmed());
        assert_eq!(report.summary.flag, expected);
    }
}
