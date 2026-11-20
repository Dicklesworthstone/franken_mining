//! Synthetic independent arithmetic and source-selection controls for paired saved assessments.

#![cfg(test)]

use fmine_calib::ledger::binding::{
    PublicMetric, PublicOwnership, PublicPeriod, PublicResolutionPolicy, PublicVintagePolicy,
};
use fmine_calib::ledger::comparison::{PairStatus, PairedScore};
use fmine_calib::{
    AssessmentPolicy, CohortSpec, ForecastLedger, ForecastPayload, OutcomeCitation, OutcomeKind,
    OutcomeSelector,
};
use fmine_core::{Digest32, ObjectId};
use fmine_numerics::exact_feas::Rat;

fn r(value: i128) -> Rat {
    Rat::from_integer(value)
}
fn deadline() -> i64 {
    PublicPeriod::Year(2030).end_ns().unwrap()
}

fn forecast(ledger: &mut ForecastLedger, name: &str, lo: f64, hi: f64) -> ObjectId {
    ledger
        .register_payload(
            name,
            1,
            deadline(),
            ForecastPayload::Interval {
                lo,
                hi,
                nominal: Some(0.5),
            },
        )
        .unwrap()
}

fn cohort(
    ledger: &mut ForecastLedger,
    name: &str,
    members: &[(ObjectId, &str)],
    metric: PublicMetric,
) -> Digest32 {
    let policy = PublicResolutionPolicy {
        issuer: "synthetic".into(),
        metric,
        period: PublicPeriod::Year(2030),
        ownership: PublicOwnership::Full,
        members: members
            .iter()
            .map(|(id, subject)| (*id, (*subject).into()))
            .collect(),
    };
    ledger
        .freeze_cohort(CohortSpec {
            name: name.into(),
            frozen_at_ns: 2,
            members: members.iter().map(|(id, _)| *id).collect(),
            resolution_rule: policy
                .to_resolution_rule_with_vintages(PublicVintagePolicy::RegisteredComparatives)
                .unwrap(),
            eligible_sources: vec!["synthetic".into()],
            policy: AssessmentPolicy {
                alpha: 0.5,
                ..AssessmentPolicy::default()
            },
        })
        .unwrap()
}

fn citation(cell: &str) -> OutcomeCitation {
    OutcomeCitation {
        source: "synthetic".into(),
        vintage: "2030Q4".into(),
        locator: cell.into(),
        sha256: [7; 32],
    }
}

fn publish(ledger: &mut ForecastLedger, id: ObjectId, value: f64, cell: &str) {
    ledger
        .append_outcome(
            id,
            deadline() + 1,
            deadline() + 2,
            OutcomeKind::Published {
                value,
                citation: citation(cell),
            },
        )
        .unwrap();
}

fn assess(ledger: &mut ForecastLedger, cohort: Digest32) -> Digest32 {
    ledger
        .assess(cohort, deadline() + 10, OutcomeSelector::FirstPublished)
        .unwrap()
        .digest
}

struct Pair {
    ledger: ForecastLedger,
    left: Digest32,
    right: Digest32,
    ids: [[ObjectId; 2]; 2],
}

fn pair() -> Pair {
    let mut ledger = ForecastLedger::new();
    let ids = [
        [
            forecast(&mut ledger, "left-a", 10.0, 20.0),
            forecast(&mut ledger, "left-b", 2.0, 4.0),
        ],
        [
            forecast(&mut ledger, "right-a", 0.0, 20.0),
            forecast(&mut ledger, "right-b", 0.0, 4.0),
        ],
    ];
    let left = cohort(
        &mut ledger,
        "left",
        &[(ids[0][0], "A"), (ids[0][1], "B")],
        PublicMetric::ProductionKoz,
    );
    let right = cohort(
        &mut ledger,
        "right",
        &[(ids[1][0], "A"), (ids[1][1], "B")],
        PublicMetric::ProductionKoz,
    );
    Pair {
        ledger,
        left,
        right,
        ids,
    }
}

#[test]
fn paired_losses_and_pending_completion_have_independent_known_answers() {
    let mut p = pair();
    publish(&mut p.ledger, p.ids[0][0], 15.0, "Sheet!A1");
    publish(&mut p.ledger, p.ids[1][0], 15.0, "Sheet!A1");
    let (left, right) = (
        assess(&mut p.ledger, p.left),
        assess(&mut p.ledger, p.right),
    );
    let report = p.ledger.compare_assessments(left, right).unwrap();
    assert_eq!(
        (report.n_paired, report.n_unresolved, report.n_unmatched),
        (1, 1, 0)
    );
    assert_eq!(report.members[0].difference, Some(r(-10)));
    // IS([2,4],y)-IS([0,4],y), alpha=1/2: 6 below zero, -2 above four.
    assert_eq!(report.members[1].completion, Some((r(-2), r(6))));
    assert_eq!(report.observed_mean_difference, Some(r(-10)));
    assert_eq!(report.whole_population, Some((r(-6), r(-2))));
    assert_eq!(report, p.ledger.compare_assessments(left, right).unwrap());
    let reverse = p.ledger.compare_assessments(right, left).unwrap();
    assert_eq!(reverse.observed_mean_difference, Some(r(10)));
    assert_eq!(reverse.whole_population, Some((r(2), r(6))));
    assert_ne!(reverse.digest, report.digest);
    for id in p.ids.into_iter().flatten() {
        assert!(p.ledger.outcomes(id).len() <= 1);
    }
}

#[test]
fn all_resolved_pairs_equal_the_mean_of_independent_interval_losses() {
    for y in [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 20.0] {
        let mut p = pair();
        for model in p.ids {
            publish(&mut p.ledger, model[0], 15.0, "Sheet!A1");
            publish(&mut p.ledger, model[1], y, "Sheet!B1");
        }
        let (left, right) = (
            assess(&mut p.ledger, p.left),
            assess(&mut p.ledger, p.right),
        );
        let report = p.ledger.compare_assessments(left, right).unwrap();
        let expected = if y < 2.0 {
            -2.0 + 4.0 * (2.0 - y)
        } else {
            -2.0
        };
        let expected = Rat::new((-10.0 + expected) as i128, 2).unwrap();
        assert_eq!(report.observed_mean_difference, Some(expected));
        assert_eq!(report.whole_population, Some((expected, expected)));
    }
}

#[test]
fn matching_values_do_not_launder_different_source_publications() {
    let mut p = pair();
    publish(&mut p.ledger, p.ids[0][0], 15.0, "Sheet!A1");
    publish(&mut p.ledger, p.ids[1][0], 15.0, "Sheet!A2");
    let (left, right) = (
        assess(&mut p.ledger, p.left),
        assess(&mut p.ledger, p.right),
    );
    let report = p.ledger.compare_assessments(left, right).unwrap();
    assert_eq!(
        report.members[0].status,
        PairStatus::OutcomeSelectionMismatch
    );
    assert_eq!(report.n_unmatched, 1);
    assert_eq!(report.observed_mean_difference, None);
    assert_eq!(report.whole_population, None);
    assert!(report.members[0].left.is_some() && report.members[0].right.is_some());
}

#[test]
fn unmatched_forecasts_and_one_sided_outcomes_remain_visible() {
    let mut ledger = ForecastLedger::new();
    let ids = [
        forecast(&mut ledger, "a", 1.0, 2.0),
        forecast(&mut ledger, "b", 1.0, 2.0),
        forecast(&mut ledger, "c", 1.0, 2.0),
        forecast(&mut ledger, "d", 1.0, 2.0),
    ];
    let left = cohort(
        &mut ledger,
        "left",
        &[(ids[0], "A"), (ids[1], "B")],
        PublicMetric::ProductionKoz,
    );
    let right = cohort(
        &mut ledger,
        "right",
        &[(ids[2], "A"), (ids[3], "C")],
        PublicMetric::ProductionKoz,
    );
    publish(&mut ledger, ids[0], 1.0, "Sheet!A1");
    let (left, right) = (assess(&mut ledger, left), assess(&mut ledger, right));
    let report = ledger.compare_assessments(left, right).unwrap();
    assert_eq!(report.members.len(), 3);
    assert_eq!(report.n_unmatched, 3);
    assert_eq!(
        report.members.iter().map(|r| r.status).collect::<Vec<_>>(),
        [
            PairStatus::OutcomeSelectionMismatch,
            PairStatus::RightForecastMissing,
            PairStatus::LeftForecastMissing
        ]
    );
    assert_eq!(report.whole_population, None);
}

#[test]
fn incompatible_targets_cuts_selectors_and_unknown_reports_refuse() {
    let mut ledger = ForecastLedger::new();
    let a = forecast(&mut ledger, "gold", 1.0, 2.0);
    let b = forecast(&mut ledger, "copper", 1.0, 2.0);
    let left = cohort(
        &mut ledger,
        "gold",
        &[(a, "A")],
        PublicMetric::ProductionKoz,
    );
    let right = cohort(
        &mut ledger,
        "copper",
        &[(b, "A")],
        PublicMetric::ProductionKt,
    );
    let (la, ra) = (assess(&mut ledger, left), assess(&mut ledger, right));
    assert!(ledger.compare_assessments(la, ra).is_err());
    assert!(ledger.compare_assessments(la, Digest32::default()).is_err());
    let late = ledger
        .assess(left, deadline() + 11, OutcomeSelector::FirstPublished)
        .unwrap()
        .digest;
    assert!(ledger.compare_assessments(la, late).is_err());
    let latest = ledger
        .assess(left, deadline() + 10, OutcomeSelector::LatestAsOf)
        .unwrap()
        .digest;
    assert!(ledger.compare_assessments(la, latest).is_err());
}

#[test]
fn saved_comparison_is_immutable_after_later_outcomes() {
    let mut p = pair();
    let (left, right) = (
        assess(&mut p.ledger, p.left),
        assess(&mut p.ledger, p.right),
    );
    let old = p.ledger.compare_assessments(left, right).unwrap();
    assert_eq!(old.n_paired, 0);
    assert!(old.whole_population.is_some());
    for model in p.ids {
        for (i, id) in model.into_iter().enumerate() {
            p.ledger
                .append_outcome(
                    id,
                    deadline() + 11,
                    deadline() + 12,
                    OutcomeKind::Published {
                        value: 15.0,
                        citation: citation(&format!("Sheet!{i}")),
                    },
                )
                .unwrap();
        }
    }
    assert_eq!(old, p.ledger.compare_assessments(left, right).unwrap());
    let later: Vec<_> = [p.left, p.right]
        .into_iter()
        .map(|cohort| {
            p.ledger
                .assess(cohort, deadline() + 20, OutcomeSelector::FirstPublished)
                .unwrap()
                .digest
        })
        .collect();
    assert_eq!(
        p.ledger
            .compare_assessments(later[0], later[1])
            .unwrap()
            .n_paired,
        2
    );
}

#[test]
fn exact_capacity_failure_does_not_drop_or_round_a_paired_member() {
    let mut ledger = ForecastLedger::new();
    let a = forecast(
        &mut ledger,
        "tiny",
        f64::MIN_POSITIVE,
        f64::MIN_POSITIVE * 2.0,
    );
    let b = forecast(&mut ledger, "wide", 0.0, 1.0);
    let left = cohort(
        &mut ledger,
        "tiny",
        &[(a, "A")],
        PublicMetric::ProductionKoz,
    );
    let right = cohort(
        &mut ledger,
        "wide",
        &[(b, "A")],
        PublicMetric::ProductionKoz,
    );
    publish(&mut ledger, a, 0.0, "Sheet!A1");
    publish(&mut ledger, b, 0.0, "Sheet!A1");
    let (left, right) = (assess(&mut ledger, left), assess(&mut ledger, right));
    let report = ledger.compare_assessments(left, right).unwrap();
    assert_eq!(report.n_paired, 1);
    assert_eq!(report.n_arithmetic_indeterminate, 1);
    assert_eq!(report.observed_mean_difference, None);
    assert_eq!(report.whole_population, None);
}

fn finite_forecast(ledger: &mut ForecastLedger, name: &str, payload: ForecastPayload) -> ObjectId {
    ledger
        .register_payload(name, 1, deadline(), payload)
        .unwrap()
}

fn empirical(values: &[f64]) -> ForecastPayload {
    ForecastPayload::Empirical {
        samples: values.to_vec(),
    }
}

fn weighted(values: &[f64], weights: &[(i128, i128)]) -> ForecastPayload {
    ForecastPayload::WeightedEmpirical {
        samples: values.to_vec(),
        weights: weights
            .iter()
            .map(|&(n, d)| Rat::new(n, d).unwrap())
            .collect(),
    }
}

fn finite_pair() -> Pair {
    let mut ledger = ForecastLedger::new();
    let ids = [
        [
            finite_forecast(
                &mut ledger,
                "weighted-a",
                weighted(&[0.0, 2.0], &[(1, 4), (3, 4)]),
            ),
            finite_forecast(&mut ledger, "point-b", empirical(&[1.0])),
        ],
        [
            finite_forecast(&mut ledger, "uniform-a", empirical(&[0.0, 2.0])),
            finite_forecast(&mut ledger, "point-c", empirical(&[0.0])),
        ],
    ];
    let left = cohort(
        &mut ledger,
        "left",
        &[(ids[0][0], "A"), (ids[0][1], "B")],
        PublicMetric::ProductionKoz,
    );
    let right = cohort(
        &mut ledger,
        "right",
        &[(ids[1][0], "A"), (ids[1][1], "B")],
        PublicMetric::ProductionKoz,
    );
    Pair {
        ledger,
        left,
        right,
        ids,
    }
}

#[test]
fn paired_crps_uses_all_probability_mass_and_exact_missing_outcome_extrema() {
    let mut p = finite_pair();
    for model in p.ids {
        publish(&mut p.ledger, model[0], 1.0, "Sheet!A1");
    }
    let (left, right) = (
        assess(&mut p.ledger, p.left),
        assess(&mut p.ledger, p.right),
    );
    let interval = p.ledger.compare_assessments(left, right).unwrap();
    let report = p.ledger.compare_assessments_crps(left, right).unwrap();
    assert_eq!(report.score, PairedScore::Crps);
    assert_eq!(
        (report.n_paired, report.n_unresolved, report.n_unmatched),
        (1, 1, 0)
    );
    // Identical [0,2] central intervals conceal a real distribution difference:
    // CRPS(1/4 at 0, 3/4 at 2; y=1)=5/8, versus 1/2 for the uniform distribution.
    assert_eq!(interval.members[0].difference, Some(r(0)));
    assert_eq!(report.members[0].difference, Some(Rat::new(1, 8).unwrap()));
    assert_eq!(report.members[1].completion, Some((r(-1), r(1))));
    assert_eq!(
        report.observed_mean_difference,
        Some(Rat::new(1, 8).unwrap())
    );
    assert_eq!(
        report.whole_population,
        Some((Rat::new(-7, 16).unwrap(), Rat::new(9, 16).unwrap()))
    );
    assert_ne!(report.digest, interval.digest);
    let reverse = p.ledger.compare_assessments_crps(right, left).unwrap();
    assert_eq!(
        reverse.observed_mean_difference,
        Some(Rat::new(-1, 8).unwrap())
    );
    assert_eq!(
        reverse.whole_population,
        Some((Rat::new(-9, 16).unwrap(), Rat::new(7, 16).unwrap()))
    );
    assert_eq!(interval, p.ledger.compare_assessments(left, right).unwrap());
    assert_eq!(
        report,
        p.ledger.compare_assessments_crps(left, right).unwrap()
    );
}

fn single_crps(
    left: ForecastPayload,
    right: ForecastPayload,
    outcome: Option<f64>,
) -> fmine_calib::ledger::comparison::PairedAssessment {
    let mut ledger = ForecastLedger::new();
    let a = finite_forecast(&mut ledger, "left", left);
    let b = finite_forecast(&mut ledger, "right", right);
    let l = cohort(
        &mut ledger,
        "left",
        &[(a, "A")],
        PublicMetric::ProductionKoz,
    );
    let r = cohort(
        &mut ledger,
        "right",
        &[(b, "A")],
        PublicMetric::ProductionKoz,
    );
    if let Some(value) = outcome {
        publish(&mut ledger, a, value, "Sheet!A1");
        publish(&mut ledger, b, value, "Sheet!A1");
    }
    let (l, r) = (assess(&mut ledger, l), assess(&mut ledger, r));
    ledger.compare_assessments_crps(l, r).unwrap()
}

#[test]
fn crps_completion_sweeps_interior_knots_and_has_constant_tails() {
    let left = empirical(&[1.0]);
    let right = empirical(&[0.0, 2.0]);
    let pending = single_crps(left.clone(), right.clone(), None);
    let half = Rat::new(1, 2).unwrap();
    assert_eq!(
        pending.whole_population,
        Some((Rat::new(-1, 2).unwrap(), half))
    );
    for (y, expected) in [
        (-100.0, half),
        (0.0, half),
        (1.0, Rat::new(-1, 2).unwrap()),
        (2.0, half),
        (100.0, half),
    ] {
        let resolved = single_crps(left.clone(), right.clone(), Some(y));
        assert_eq!(resolved.observed_mean_difference, Some(expected));
        assert_eq!(resolved.whole_population, Some((expected, expected)));
    }
}

#[test]
fn crps_exact_sweep_matches_independent_pair_distance_definition() {
    // Independent oracle: E|X-y| - E|X-X'|/2, with a double sum over small supports.
    fn reference(points: &[(i128, Rat)], y: i128) -> Rat {
        let first = points.iter().fold(r(0), |sum, (x, w)| {
            sum.checked_add(w.checked_mul(r((x - y).abs())).unwrap())
                .unwrap()
        });
        let mut double = r(0);
        for (x, w) in points {
            for (z, v) in points {
                double = double
                    .checked_add(
                        w.checked_mul(*v)
                            .unwrap()
                            .checked_mul(r((x - z).abs()))
                            .unwrap(),
                    )
                    .unwrap();
            }
        }
        first
            .checked_sub(double.checked_div(r(2)).unwrap())
            .unwrap()
    }
    let l = [
        (0, Rat::new(1, 6).unwrap()),
        (2, Rat::new(1, 3).unwrap()),
        (6, Rat::new(1, 2).unwrap()),
    ];
    let rr = [
        (1, Rat::new(1, 3).unwrap()),
        (3, Rat::new(1, 3).unwrap()),
        (4, Rat::new(1, 3).unwrap()),
    ];
    let left = weighted(&[0.0, 2.0, 6.0], &[(1, 6), (1, 3), (1, 2)]);
    let right = empirical(&[1.0, 3.0, 4.0]);
    for y in -2..=8 {
        let report = single_crps(left.clone(), right.clone(), Some(y as f64));
        let expected = reference(&l, y).checked_sub(reference(&rr, y)).unwrap();
        assert_eq!(report.observed_mean_difference, Some(expected), "y={y}");
    }
    let knots: Vec<_> = [0, 1, 2, 3, 4, 6]
        .into_iter()
        .map(|y| reference(&l, y).checked_sub(reference(&rr, y)).unwrap())
        .collect();
    let min = *knots
        .iter()
        .min_by(|a, b| a.checked_cmp(**b).unwrap())
        .unwrap();
    let max = *knots
        .iter()
        .max_by(|a, b| a.checked_cmp(**b).unwrap())
        .unwrap();
    assert_eq!(
        single_crps(left, right, None).whole_population,
        Some((min, max))
    );
}

#[test]
fn crps_permutation_splitting_replication_and_signed_zero_preserve_the_distribution() {
    let reference = empirical(&[0.0, 2.0]);
    let representations = [
        weighted(&[0.0, 2.0], &[(1, 4), (3, 4)]),
        weighted(&[2.0, -0.0], &[(3, 4), (1, 4)]),
        weighted(&[-0.0, 2.0, 0.0, 2.0], &[(1, 8), (1, 2), (1, 8), (1, 4)]),
        empirical(&[0.0, 2.0, 2.0, 2.0]),
        empirical(&[2.0, 2.0, -0.0, 2.0, 2.0, 2.0, 0.0, 2.0]),
    ];
    for payload in representations {
        let pending = single_crps(payload.clone(), reference.clone(), None);
        assert_eq!(
            pending.whole_population,
            Some((Rat::new(-3, 8).unwrap(), Rat::new(5, 8).unwrap()))
        );
        assert_eq!(
            single_crps(payload, reference.clone(), Some(1.0)).observed_mean_difference,
            Some(Rat::new(1, 8).unwrap())
        );
    }
}

#[test]
fn crps_refuses_unsupported_distributions_without_silently_selecting_a_subset() {
    for unsupported in [
        ForecastPayload::Gaussian {
            mean: 1.0,
            std_dev: 1.0,
        },
        ForecastPayload::Interval {
            lo: 0.0,
            hi: 2.0,
            nominal: None,
        },
        ForecastPayload::Quantiles {
            levels: vec![0.25, 0.75],
            values: vec![0.0, 2.0],
        },
    ] {
        let mut ledger = ForecastLedger::new();
        let a = finite_forecast(&mut ledger, "finite", empirical(&[0.0, 2.0]));
        let b = finite_forecast(&mut ledger, "unsupported", unsupported);
        let l = cohort(
            &mut ledger,
            "left",
            &[(a, "A")],
            PublicMetric::ProductionKoz,
        );
        // The unsupported member is unmatched, so the implementation cannot filter it away.
        let rr = cohort(
            &mut ledger,
            "right",
            &[(b, "B")],
            PublicMetric::ProductionKoz,
        );
        let (l, rr) = (assess(&mut ledger, l), assess(&mut ledger, rr));
        assert!(
            ledger
                .compare_assessments_crps(l, rr)
                .unwrap_err()
                .what
                .contains("supports only")
        );
        assert_eq!(ledger.compare_assessments(l, rr).unwrap().n_unmatched, 2);
    }
}

#[test]
fn crps_keeps_source_mismatches_missing_members_and_exact_capacity_failures_visible() {
    let mut p = finite_pair();
    publish(&mut p.ledger, p.ids[0][0], 1.0, "Sheet!A1");
    publish(&mut p.ledger, p.ids[1][0], 1.0, "Sheet!A2");
    let (l, rr) = (
        assess(&mut p.ledger, p.left),
        assess(&mut p.ledger, p.right),
    );
    let report = p.ledger.compare_assessments_crps(l, rr).unwrap();
    assert_eq!(report.n_unmatched, 1);
    assert_eq!(report.n_unresolved, 1);
    assert_eq!(
        report.members[0].status,
        PairStatus::OutcomeSelectionMismatch
    );
    assert!(report.members[0].left.is_some() && report.members[0].right.is_some());
    assert_eq!(report.whole_population, None);
    assert_eq!(report.observed_mean_difference, None);
    assert_eq!(report.members[1].completion, Some((r(-1), r(1))));

    let overflow = single_crps(
        empirical(&[f64::MIN_POSITIVE, 1.0]),
        empirical(&[0.0, 1.0]),
        Some(0.0),
    );
    assert_eq!(overflow.n_paired, 1);
    assert_eq!(overflow.n_arithmetic_indeterminate, 1);
    assert_eq!(overflow.observed_mean_difference, None);
    assert_eq!(overflow.whole_population, None);
    assert_eq!(overflow.members.len(), 1);
}
