//! SYNTHETIC: exact independent score oracles and frozen physical-target/source-selection controls.

use fmine_calib::ledger::ForecastMetadata;
use fmine_calib::ledger::binding::{
    PublicMetric, PublicOwnership, PublicPeriod, PublicResolutionPolicy,
};
use fmine_calib::ledger::comparison::{PairStatus, PairedScore};
use fmine_calib::{
    AssessmentPolicy, CohortSpec, ForecastLedger, ForecastPayload, OutcomeCitation, OutcomeKind,
    OutcomeSelector,
};
use fmine_core::{Digest32, ObjectId};
use fmine_numerics::exact_feas::Rat;

fn r(num: i128, den: i128) -> Rat {
    Rat::new(num, den).unwrap()
}

fn metadata(site: &str) -> ForecastMetadata {
    ForecastMetadata {
        family: "synthetic-raw-plant-model".into(),
        author_role: "synthetic-model-role".into(),
        domain: "mill/blend".into(),
        horizon: "synthetic complete processing hour".into(),
        site: site.into(),
        quantity: "model_recovered_metal:Au".into(),
        unit: "kg".into(),
    }
}

fn payload(shift: f64) -> ForecastPayload {
    ForecastPayload::WeightedEmpirical {
        samples: vec![shift, 4.0 + shift],
        weights: vec![r(1, 4), r(3, 4)],
    }
}

fn register(
    ledger: &mut ForecastLedger,
    name: &str,
    metadata: ForecastMetadata,
    due: i64,
    payload: ForecastPayload,
) -> ObjectId {
    ledger
        .register_payload_with_metadata(name, 1, due, payload, metadata)
        .unwrap()
}

fn spec(name: &str, members: Vec<ObjectId>) -> CohortSpec {
    CohortSpec {
        name: name.into(),
        frozen_at_ns: 2,
        members,
        resolution_rule: "Supplied elemental mass for the complete frozen processing support"
            .into(),
        eligible_sources: vec!["synthetic:mill-report".into()],
        policy: AssessmentPolicy {
            alpha: 0.5,
            ..AssessmentPolicy::default()
        },
    }
}

fn publish(ledger: &mut ForecastLedger, id: ObjectId, locator: &str) {
    ledger
        .append_outcome(
            id,
            11,
            12,
            OutcomeKind::Published {
                value: 3.0,
                citation: OutcomeCitation {
                    source: "synthetic:mill-report".into(),
                    vintage: "invented-complete-hour".into(),
                    locator: locator.into(),
                    sha256: [4; 32],
                },
            },
        )
        .unwrap();
}

fn assess(ledger: &mut ForecastLedger, cohort: Digest32) -> Digest32 {
    ledger
        .assess(cohort, 20, OutcomeSelector::FirstPublished)
        .unwrap()
        .digest
}

#[test]
fn supplied_weighted_models_have_exact_scores_and_complete_unresolved_bounds() {
    let mut ledger = ForecastLedger::new();
    let left: Vec<_> = ["A", "B"]
        .into_iter()
        .map(|site| register(&mut ledger, "raw", metadata(site), 10, payload(0.0)))
        .collect();
    let right: Vec<_> = ["A", "B"]
        .into_iter()
        .map(|site| {
            let mut metadata = metadata(site);
            metadata.family = "synthetic-fitted-plant-model".into();
            metadata.author_role = "synthetic-corrected-model-role".into();
            register(&mut ledger, "fitted", metadata, 10, payload(1.0))
        })
        .collect();
    let lc = ledger.freeze_cohort(spec("raw", left.clone())).unwrap();
    let rc = ledger.freeze_cohort(spec("fitted", right.clone())).unwrap();
    for id in [left[0], right[0]] {
        publish(&mut ledger, id, "Synthetic!A1");
    }
    let (l, rr) = (assess(&mut ledger, lc), assess(&mut ledger, rc));
    let compared = ledger
        .compare_supplied_assessments(l, rr, PairedScore::Crps)
        .unwrap();
    assert_eq!(
        (
            compared.n_paired,
            compared.n_unresolved,
            compared.n_unmatched
        ),
        (1, 1, 0)
    );
    // At y=3: raw E|X-y|=3/2 and half E|X-X'|=3/4, hence CRPS=3/4.
    // Fitted E|X-y|=2 with the same spread, hence CRPS=5/4: raw minus fitted=-1/2.
    assert_eq!(compared.members[0].pair.difference, Some(r(-1, 2)));
    // A unit translation changes finite CRPS by -1 at the lower tail and +1 at the upper
    // tail; the CDF difference is nonnegative everywhere, so these are exact extrema.
    assert_eq!(compared.members[1].pair.completion, Some((r(-1, 1), r(1, 1))));
    assert_eq!(compared.observed_mean_difference, Some(r(-1, 2)));
    assert_eq!(compared.whole_population, Some((r(-3, 4), r(1, 4))));
    assert_eq!(compared.members[0].target.site, "A");
    assert_eq!(compared.members[0].outcome_due_ns, 10);
    assert_eq!(
        compared.members[0].pair.left.as_ref().unwrap().forecast_id,
        left[0]
    );
    assert_eq!(
        compared.members[0].pair.right.as_ref().unwrap().forecast_id,
        right[0]
    );
    assert_ne!(
        compared.members[0].pair.left.as_ref().unwrap().selected_event,
        compared.members[0].pair.right.as_ref().unwrap().selected_event
    );
    assert_eq!(
        compared,
        ledger
            .compare_supplied_assessments(l, rr, PairedScore::Crps)
            .unwrap()
    );
    let reverse = ledger
        .compare_supplied_assessments(rr, l, PairedScore::Crps)
        .unwrap();
    assert_eq!(reverse.observed_mean_difference, Some(r(1, 2)));
    assert_eq!(reverse.whole_population, Some((r(-1, 4), r(3, 4))));
    let interval = ledger
        .compare_supplied_assessments(l, rr, PairedScore::Interval)
        .unwrap();
    assert_eq!(interval.observed_mean_difference, Some(r(0, 1)));
    assert_eq!(interval.members[1].pair.completion, Some((r(-4, 1), r(4, 1))));
    assert_eq!(interval.whole_population, Some((r(-2, 1), r(2, 1))));
    assert_ne!(interval.digest, compared.digest);
    assert!(ledger.compare_assessments(l, rr).is_err());
    assert!(ledger.compare_assessments_full_crps(l, rr).is_err());

    // Later supplied evidence cannot rewrite the saved selected outcomes or comparison.
    for id in [left[1], right[1]] {
        ledger
            .append_outcome(
                id,
                21,
                22,
                OutcomeKind::Published {
                    value: 3.0,
                    citation: OutcomeCitation {
                        source: "synthetic:mill-report".into(),
                        vintage: "invented-complete-hour".into(),
                        locator: "Synthetic!B1".into(),
                        sha256: [4; 32],
                    },
                },
            )
            .unwrap();
    }
    assert_eq!(
        compared,
        ledger
            .compare_supplied_assessments(l, rr, PairedScore::Crps)
            .unwrap()
    );
    let newer: Vec<_> = [lc, rc]
        .into_iter()
        .map(|cohort| {
            ledger
                .assess(cohort, 30, OutcomeSelector::FirstPublished)
                .unwrap()
                .digest
        })
        .collect();
    let completed = ledger
        .compare_supplied_assessments(newer[0], newer[1], PairedScore::Crps)
        .unwrap();
    assert_eq!(completed.n_paired, 2);
    assert_eq!(completed.whole_population, Some((r(-1, 2), r(-1, 2))));
    assert_ne!(completed.digest, compared.digest);
}

#[test]
fn equal_values_and_citations_never_substitute_for_frozen_physical_identity() {
    for dimension in ["domain", "site", "horizon"] {
        let mut ledger = ForecastLedger::new();
        let l = register(&mut ledger, "raw", metadata("A"), 10, payload(0.0));
        let mut changed = metadata("A");
        match dimension {
            "domain" => changed.domain = "different circuit/blend".into(),
            "site" => changed.site = "different boundary".into(),
            _ => changed.horizon = "different physical hour".into(),
        }
        let rr = register(&mut ledger, "fitted", changed, 10, payload(1.0));
        let lc = ledger.freeze_cohort(spec("raw", vec![l])).unwrap();
        let rc = ledger.freeze_cohort(spec("fitted", vec![rr])).unwrap();
        publish(&mut ledger, l, "Synthetic!A1");
        publish(&mut ledger, rr, "Synthetic!A1");
        let (left, right) = (assess(&mut ledger, lc), assess(&mut ledger, rc));
        let report = ledger
            .compare_supplied_assessments(left, right, PairedScore::Crps)
            .unwrap();
        assert_eq!(
            (report.members.len(), report.n_paired, report.n_unmatched),
            (2, 0, 2),
            "{dimension}"
        );
        assert_eq!(report.observed_mean_difference, None);
        assert_eq!(report.whole_population, None);
        assert!(
            report
                .members
                .iter()
                .any(|m| m.pair.status == PairStatus::LeftForecastMissing)
        );
        assert!(
            report
                .members
                .iter()
                .any(|m| m.pair.status == PairStatus::RightForecastMissing)
        );
    }
}

#[test]
fn supplied_pairing_keeps_equal_value_different_source_selections_unmatched() {
    let mut ledger = ForecastLedger::new();
    let l = register(&mut ledger, "raw", metadata("A"), 10, payload(0.0));
    let rr = register(&mut ledger, "fitted", metadata("A"), 10, payload(1.0));
    let lc = ledger.freeze_cohort(spec("raw", vec![l])).unwrap();
    let rc = ledger.freeze_cohort(spec("fitted", vec![rr])).unwrap();
    publish(&mut ledger, l, "Synthetic!A1");
    publish(&mut ledger, rr, "Different support!A1");
    let (left, right) = (assess(&mut ledger, lc), assess(&mut ledger, rc));
    let report = ledger
        .compare_supplied_assessments(left, right, PairedScore::Crps)
        .unwrap();
    assert_eq!(
        report.members[0].pair.status,
        PairStatus::OutcomeSelectionMismatch
    );
    assert_eq!(report.n_unmatched, 1);
    assert_eq!(report.observed_mean_difference, None);
    assert_eq!(report.whole_population, None);
    assert_ne!(
        report.members[0].pair.left.as_ref().unwrap().outcome,
        report.members[0].pair.right.as_ref().unwrap().outcome
    );
}

#[test]
fn incompatible_frozen_rules_sources_units_deadlines_cuts_and_selectors_refuse() {
    for change in [
        "rule", "sources", "unit", "quantity", "deadline", "alpha", "cut", "selector",
    ] {
        let mut ledger = ForecastLedger::new();
        let l = register(&mut ledger, "raw", metadata("A"), 10, payload(0.0));
        let mut changed = metadata("A");
        if change == "unit" {
            changed.unit = "g".into();
        }
        if change == "quantity" {
            changed.quantity = "model_contained_metal:Au".into();
        }
        let due = if change == "deadline" { 11 } else { 10 };
        let rr = register(&mut ledger, "fitted", changed, due, payload(1.0));
        let lc = ledger.freeze_cohort(spec("raw", vec![l])).unwrap();
        let mut right_spec = spec("fitted", vec![rr]);
        match change {
            "rule" => right_spec.resolution_rule = "Different resolution definition".into(),
            "sources" => right_spec.eligible_sources = vec!["other:supplied-source".into()],
            "alpha" => right_spec.policy.alpha = 0.2,
            _ => {}
        }
        let rc = ledger.freeze_cohort(right_spec).unwrap();
        let left = assess(&mut ledger, lc);
        let right = ledger
            .assess(
                rc,
                if change == "cut" { 21 } else { 20 },
                if change == "selector" {
                    OutcomeSelector::LatestAsOf
                } else {
                    OutcomeSelector::FirstPublished
                },
            )
            .unwrap()
            .digest;
        assert!(
            ledger
                .compare_supplied_assessments(left, right, PairedScore::Crps)
                .is_err(),
            "{change}"
        );
    }
}

#[test]
fn duplicate_targets_and_unknown_or_public_targets_cannot_enter_the_supplied_lane() {
    let mut ledger = ForecastLedger::new();
    let l = register(&mut ledger, "raw", metadata("A"), 10, payload(0.0));
    let duplicate = register(&mut ledger, "second raw", metadata("A"), 10, payload(1.0));
    let lc = ledger
        .freeze_cohort(spec("duplicate", vec![l, duplicate]))
        .unwrap();
    let rc = ledger.freeze_cohort(spec("single", vec![l])).unwrap();
    let (left, right) = (assess(&mut ledger, lc), assess(&mut ledger, rc));
    assert!(
        ledger
            .compare_supplied_assessments(left, right, PairedScore::Crps)
            .is_err()
    );

    let mut ledger = ForecastLedger::new();
    let legacy = ledger.register_payload("legacy", 1, 10, payload(0.0)).unwrap();
    let cohort = ledger.freeze_cohort(spec("unknown", vec![legacy])).unwrap();
    let saved = assess(&mut ledger, cohort);
    assert!(
        ledger
            .compare_supplied_assessments(saved, saved, PairedScore::Crps)
            .is_err()
    );

    let mut ledger = ForecastLedger::new();
    let period = PublicPeriod::Year(2030);
    let mut declared = metadata("A");
    declared.quantity = "production_koz".into();
    declared.unit = "koz".into();
    let public = register(
        &mut ledger,
        "public",
        declared,
        period.end_ns().unwrap(),
        payload(0.0),
    );
    let policy = PublicResolutionPolicy {
        issuer: "synthetic".into(),
        metric: PublicMetric::ProductionKoz,
        period,
        ownership: PublicOwnership::Full,
        members: [(public, "A".into())].into_iter().collect(),
    };
    let mut public_spec = spec("public", vec![public]);
    public_spec.resolution_rule = policy.to_resolution_rule().unwrap();
    public_spec.eligible_sources = vec!["synthetic".into()];
    let cohort = ledger.freeze_cohort(public_spec).unwrap();
    let saved = assess(&mut ledger, cohort);
    assert!(
        ledger
            .compare_supplied_assessments(saved, saved, PairedScore::Crps)
            .is_err()
    );
    assert_eq!(
        ledger
            .compare_assessments_crps(saved, saved)
            .unwrap()
            .whole_population,
        Some((r(0, 1), r(0, 1)))
    );
}

#[test]
fn an_unsupported_unmatched_distribution_cannot_be_silently_dropped() {
    let mut ledger = ForecastLedger::new();
    let l = register(&mut ledger, "raw", metadata("A"), 10, payload(0.0));
    let rr = register(&mut ledger, "fitted", metadata("A"), 10, payload(1.0));
    let missing = register(
        &mut ledger,
        "incomplete",
        metadata("B"),
        10,
        ForecastPayload::Interval {
            lo: 0.0,
            hi: 4.0,
            nominal: Some(0.5),
        },
    );
    let lc = ledger.freeze_cohort(spec("raw", vec![l, missing])).unwrap();
    let rc = ledger.freeze_cohort(spec("fitted", vec![rr])).unwrap();
    let (left, right) = (assess(&mut ledger, lc), assess(&mut ledger, rc));
    let interval = ledger
        .compare_supplied_assessments(left, right, PairedScore::Interval)
        .unwrap();
    assert_eq!(interval.n_unresolved, 1);
    assert_eq!(interval.n_unmatched, 1);
    assert_eq!(interval.whole_population, None);
    assert!(
        ledger
            .compare_supplied_assessments(left, right, PairedScore::Crps)
            .is_err()
    );
}

fn event_metadata(site: &str) -> ForecastMetadata {
    let mut metadata = metadata(site);
    metadata.family = "synthetic-event".into();
    metadata.quantity = "event:complete_hour_meets_production_target".into();
    metadata.unit = "1".into();
    metadata
}

fn event_spec(name: &str, members: Vec<ObjectId>) -> CohortSpec {
    let mut spec = spec(name, members);
    spec.resolution_rule =
        "Supplied binary event: complete frozen processing hour meets production target".into();
    spec.policy.outcome_bounds = Some((0.0, 1.0));
    spec
}

fn publish_event(ledger: &mut ForecastLedger, id: ObjectId, value: f64, locator: &str) {
    ledger
        .append_outcome(
            id,
            11,
            12,
            OutcomeKind::Published {
                value,
                citation: OutcomeCitation {
                    source: "synthetic:mill-report".into(),
                    vintage: "invented-complete-hour".into(),
                    locator: locator.into(),
                    sha256: [4; 32],
                },
            },
        )
        .unwrap();
}

#[test]
fn supplied_brier_comparison_keeps_binary_extremes_missing_events_and_saved_cuts() {
    let mut ledger = ForecastLedger::new();
    let left: Vec<_> = [("A", 1.0), ("B", 0.25), ("C", 0.0)]
        .into_iter()
        .map(|(site, probability)| {
            register(
                &mut ledger,
                "left",
                event_metadata(site),
                10,
                ForecastPayload::Bernoulli { probability },
            )
        })
        .collect();
    let right: Vec<_> = [("A", 0.0), ("B", 0.75), ("C", 0.0)]
        .into_iter()
        .map(|(site, probability)| {
            register(
                &mut ledger,
                "right",
                event_metadata(site),
                10,
                ForecastPayload::Bernoulli { probability },
            )
        })
        .collect();
    let mut left_spec = event_spec("left", left.clone());
    let mut right_spec = event_spec("right", right.clone());
    // Brier compares original probabilities; these different log-loss floors cannot clip them.
    left_spec.policy.event_log_floor = Some(0.01);
    right_spec.policy.event_log_floor = Some(0.1);
    let lc = ledger.freeze_cohort(left_spec).unwrap();
    let rc = ledger.freeze_cohort(right_spec).unwrap();
    for id in [left[0], right[0]] {
        publish_event(&mut ledger, id, 1.0, "Synthetic!A1");
    }
    for id in [left[1], right[1]] {
        ledger
            .append_outcome(
                id,
                11,
                12,
                OutcomeKind::Missing {
                    reason: fmine_calib::ledger::MissingReason::NotPublished,
                },
            )
            .unwrap();
    }
    let (l, rr) = (assess(&mut ledger, lc), assess(&mut ledger, rc));
    let compared = ledger
        .compare_supplied_assessments(l, rr, PairedScore::Brier)
        .unwrap();
    assert_eq!(
        (
            compared.n_paired,
            compared.n_unresolved,
            compared.n_unmatched,
            compared.n_arithmetic_indeterminate,
        ),
        (1, 2, 0, 0)
    );
    // Independent direct-loss oracle: A gives 0-1=-1. B gives -1/2 at y=0 and
    // +1/2 at y=1. Equal deterministic forecasts for C give zero at either outcome.
    assert_eq!(compared.members[0].pair.difference, Some(r(-1, 1)));
    assert_eq!(
        compared.members[1].pair.completion,
        Some((r(-1, 2), r(1, 2)))
    );
    assert_eq!(
        compared.members[2].pair.completion,
        Some((r(0, 1), r(0, 1)))
    );
    assert_eq!(compared.observed_mean_difference, Some(r(-1, 1)));
    assert_eq!(compared.whole_population, Some((r(-1, 2), r(-1, 6))));
    assert!(compared.members.iter().all(|member| {
        member.pair.left_interval.is_none() && member.pair.right_interval.is_none()
    }));
    assert!(
        compared.members[1]
            .pair
            .left
            .as_ref()
            .unwrap()
            .selected_event
            .is_some()
    );
    assert!(
        compared.members[2]
            .pair
            .left
            .as_ref()
            .unwrap()
            .selected_event
            .is_none()
    );
    assert!(compared.method().contains("no log-score floor"));
    let reverse = ledger
        .compare_supplied_assessments(rr, l, PairedScore::Brier)
        .unwrap();
    assert_eq!(reverse.whole_population, Some((r(1, 6), r(1, 2))));
    assert!(
        ledger
            .compare_supplied_assessments(l, rr, PairedScore::Crps)
            .is_err()
    );

    for (index, site) in [(1, "B"), (2, "C")] {
        for id in [left[index], right[index]] {
            ledger
                .append_outcome(
                    id,
                    21,
                    22,
                    OutcomeKind::Published {
                        value: 0.0,
                        citation: OutcomeCitation {
                            source: "synthetic:mill-report".into(),
                            vintage: "invented-complete-hour".into(),
                            locator: format!("Synthetic!{site}1"),
                            sha256: [4; 32],
                        },
                    },
                )
                .unwrap();
        }
    }
    assert_eq!(
        ledger
            .compare_supplied_assessments(l, rr, PairedScore::Brier)
            .unwrap(),
        compared
    );
    let newer: Vec<_> = [lc, rc]
        .into_iter()
        .map(|cohort| {
            ledger
                .assess(cohort, 30, OutcomeSelector::FirstPublished)
                .unwrap()
                .digest
        })
        .collect();
    let completed = ledger
        .compare_supplied_assessments(newer[0], newer[1], PairedScore::Brier)
        .unwrap();
    assert_eq!(completed.n_paired, 3);
    assert_eq!(completed.whole_population, Some((r(-1, 2), r(-1, 2))));
    assert_ne!(completed.digest, compared.digest);
}

#[test]
fn brier_pairing_refuses_incompatible_targets_and_keeps_source_mismatches() {
    for change in [
        "none", "citation", "outcome", "event", "unit", "due", "cut", "selector", "domain",
    ] {
        let mut ledger = ForecastLedger::new();
        let left = register(
            &mut ledger,
            "left",
            event_metadata("A"),
            10,
            ForecastPayload::Bernoulli { probability: 0.0 },
        );
        let mut changed = event_metadata("A");
        match change {
            "event" => changed.quantity = "event:different_production_target".into(),
            "unit" => changed.unit = "percent".into(),
            "domain" => changed.domain = "different mill support".into(),
            _ => {}
        }
        let right = register(
            &mut ledger,
            "right",
            changed,
            if change == "due" { 11 } else { 10 },
            ForecastPayload::Bernoulli { probability: 1.0 },
        );
        let lc = ledger
            .freeze_cohort(event_spec("left", vec![left]))
            .unwrap();
        let rc = ledger
            .freeze_cohort(event_spec("right", vec![right]))
            .unwrap();
        publish_event(&mut ledger, left, 0.0, "Synthetic!A1");
        publish_event(
            &mut ledger,
            right,
            if change == "outcome" { 1.0 } else { 0.0 },
            if change == "citation" {
                "Different support!A1"
            } else {
                "Synthetic!A1"
            },
        );
        let l = assess(&mut ledger, lc);
        let rr = ledger
            .assess(
                rc,
                if change == "cut" { 21 } else { 20 },
                if change == "selector" {
                    OutcomeSelector::LatestAsOf
                } else {
                    OutcomeSelector::FirstPublished
                },
            )
            .unwrap()
            .digest;
        let report = ledger.compare_supplied_assessments(l, rr, PairedScore::Brier);
        if matches!(change, "event" | "unit" | "due" | "cut" | "selector") {
            assert!(report.is_err(), "{change}");
        } else {
            let report = report.unwrap();
            if change == "none" {
                assert_eq!(report.n_paired, 1);
                assert_eq!(report.whole_population, Some((r(-1, 1), r(-1, 1))));
            } else {
                assert_eq!(report.n_paired, 0, "{change}");
                assert_eq!(report.n_unmatched, if change == "domain" { 2 } else { 1 });
                assert_eq!(report.observed_mean_difference, None);
                assert_eq!(report.whole_population, None);
            }
        }
    }
}

#[test]
fn brier_requires_binary_forecasts_and_keeps_unrepresentable_members() {
    let mut ledger = ForecastLedger::new();
    let left = register(
        &mut ledger,
        "left",
        event_metadata("A"),
        10,
        ForecastPayload::Bernoulli {
            probability: f64::from_bits(1),
        },
    );
    let right = register(
        &mut ledger,
        "right",
        event_metadata("A"),
        10,
        ForecastPayload::Bernoulli { probability: 0.5 },
    );
    let lc = ledger
        .freeze_cohort(event_spec("left", vec![left]))
        .unwrap();
    let rc = ledger
        .freeze_cohort(event_spec("right", vec![right]))
        .unwrap();
    let (l, rr) = (assess(&mut ledger, lc), assess(&mut ledger, rc));
    let report = ledger
        .compare_supplied_assessments(l, rr, PairedScore::Brier)
        .unwrap();
    assert_eq!(report.members.len(), 1);
    assert_eq!(report.n_unresolved, 1);
    assert_eq!(report.n_arithmetic_indeterminate, 1);
    assert_eq!(report.whole_population, None);
    let identical = ledger
        .compare_supplied_assessments(l, l, PairedScore::Brier)
        .unwrap();
    assert_eq!(identical.n_arithmetic_indeterminate, 0);
    assert_eq!(identical.whole_population, Some((r(0, 1), r(0, 1))));

    // Even an unmatched distribution with atoms at 0 and 1 cannot silently redefine an event.
    let mut ledger = ForecastLedger::new();
    let left = register(
        &mut ledger,
        "left",
        event_metadata("A"),
        10,
        ForecastPayload::Bernoulli { probability: 0.5 },
    );
    let right = register(
        &mut ledger,
        "right",
        event_metadata("B"),
        10,
        ForecastPayload::Empirical {
            samples: vec![0.0, 1.0],
        },
    );
    let lc = ledger
        .freeze_cohort(event_spec("left", vec![left]))
        .unwrap();
    let rc = ledger
        .freeze_cohort(event_spec("right", vec![right]))
        .unwrap();
    let (l, rr) = (assess(&mut ledger, lc), assess(&mut ledger, rc));
    assert!(
        ledger
            .compare_supplied_assessments(l, rr, PairedScore::Brier)
            .is_err()
    );
}
