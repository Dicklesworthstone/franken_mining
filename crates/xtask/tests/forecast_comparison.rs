//! Synthetic independent arithmetic and source-selection controls for paired saved assessments.

#![cfg(test)]

use fmine_calib::ledger::binding::{
    PublicMetric, PublicOwnership, PublicPeriod, PublicResolutionPolicy, PublicVintagePolicy,
};
use fmine_calib::ledger::comparison::PairStatus;
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
