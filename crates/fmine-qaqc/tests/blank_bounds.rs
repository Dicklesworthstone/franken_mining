//! Known answers for censor-aware blank predicates (`BD-167`, `G-CENS`); synthetic inputs only.

#![allow(clippy::unwrap_used, clippy::float_cmp)]

use fmine_drill::records::{AssayValue, BlankStage};
use fmine_qaqc::insertion::{
    BlankFailureMode, BlankObservation, BlankStatus, PrecedingSampleGrade, SiteBlankRule,
    evaluate_blanks,
};

fn observation(value: AssayValue, preceding: Option<PrecedingSampleGrade>) -> BlankObservation {
    BlankObservation {
        result_id: "synthetic-result".into(),
        sample_id: "synthetic-sample".into(),
        stage: BlankStage::Coarse,
        lab: "synthetic-lab".into(),
        lab_batch: "synthetic-batch".into(),
        value,
        preceding_sample_grade: preceding,
    }
}

fn measured(value: f64, preceding: Option<PrecedingSampleGrade>) -> BlankObservation {
    observation(
        AssayValue::Measured {
            mass_fraction: value,
        },
        preceding,
    )
}

fn rule(mode: BlankFailureMode) -> SiteBlankRule {
    SiteBlankRule::new(
        "synthetic-rule",
        "2",
        "synthetic source; no site qualification",
        mode,
    )
}

fn statuses(rows: &[BlankObservation], mode: BlankFailureMode) -> Vec<BlankStatus> {
    evaluate_blanks(rows, &rule(mode))
        .unwrap()
        .verdicts
        .iter()
        .map(|v| v.status)
        .collect()
}

#[test]
fn censored_blanks_are_classified_against_the_policy_without_substitution() {
    let values = [
        AssayValue::BelowDetection { limit: 0.125 },
        AssayValue::BelowDetection { limit: 0.25 },
        AssayValue::BelowDetection { limit: 0.5 },
        AssayValue::Measured {
            mass_fraction: 0.25,
        },
        AssayValue::Measured { mass_fraction: 0.5 },
        AssayValue::AboveUpper { limit: 0.125 },
        AssayValue::AboveUpper { limit: 0.25 },
        AssayValue::AboveUpper { limit: 0.75 },
    ];
    let rows: Vec<_> = values
        .into_iter()
        .enumerate()
        .map(|(i, value)| {
            let mut row = observation(value, None);
            row.result_id = format!("result-{i}");
            row.sample_id = format!("sample-{i}");
            row
        })
        .collect();
    let policy = rule(BlankFailureMode::AboveMultipleOfDl {
        detection_limit: 0.125,
        dl_multiple: 2.0,
    });
    let result = evaluate_blanks(&rows, &policy).unwrap();
    use BlankStatus::{
        FailContamination as Fail, Indeterminate as Unknown, PassBelowDetection as PassDl,
        PassMeasured as Pass,
    };
    assert_eq!(
        result.verdicts.iter().map(|v| v.status).collect::<Vec<_>>(),
        [PassDl, PassDl, Unknown, Pass, Fail, Unknown, Fail, Fail]
    );
    assert_eq!(result.total_count, 8);
    assert_eq!(result.below_detection_count, 3);
    assert_eq!(
        (
            result.pass_count,
            result.failure_count,
            result.indeterminate_count
        ),
        (3, 3, 2)
    );
    assert_eq!(result.failure_rate_pct, Some(37.5));
    assert_eq!(result.possible_failure_rate_pct, Some(62.5));
    assert_eq!(result.rule_mode, policy.mode);
    assert_eq!(result.rule_source_doc, policy.source_doc);
    for (row, verdict) in rows.iter().zip(&result.verdicts) {
        assert_eq!(verdict.result_id, row.result_id);
        assert_eq!(verdict.value, row.value);
        assert_eq!(verdict.preceding_sample_grade, row.preceding_sample_grade);
        assert_eq!(
            (verdict.threshold_lower, verdict.threshold_upper),
            (0.25, 0.25)
        );
        assert!(!verdict.threshold_lower_exclusive && !verdict.threshold_upper_exclusive);
        if !matches!(row.value, AssayValue::Measured { .. }) {
            assert!(verdict.measured_mass_fraction.is_none());
        }
    }
    let mut reversed = rows;
    reversed.reverse();
    let reverse_result = evaluate_blanks(&reversed, &policy).unwrap();
    assert_eq!(reverse_result.failure_rate_pct, result.failure_rate_pct);
    assert_eq!(
        reverse_result.possible_failure_rate_pct,
        result.possible_failure_rate_pct
    );
    assert_eq!(
        reverse_result
            .verdicts
            .into_iter()
            .rev()
            .collect::<Vec<_>>(),
        result.verdicts
    );
}

#[test]
fn censored_predecessors_keep_and_or_rules_and_open_endpoints_distinct() {
    let upper = Some(PrecedingSampleGrade::AboveUpper { limit: 0.5 });
    let and = BlankFailureMode::CarryoverAnd {
        abs_threshold: 0.125,
        carryover_fraction: 0.5,
    };
    let rows = [
        measured(0.25, upper),
        measured(0.375, upper),
        measured(0.75, upper),
    ];
    assert_eq!(
        statuses(&rows, and),
        [
            BlankStatus::PassMeasured,
            BlankStatus::Indeterminate,
            BlankStatus::FailContamination
        ]
    );
    let result = evaluate_blanks(&rows, &rule(and)).unwrap();
    assert_eq!(result.verdicts[0].threshold_lower, 0.25);
    assert_eq!(result.verdicts[0].threshold_upper, 0.5);
    assert!(result.verdicts[0].threshold_lower_exclusive);
    assert!(!result.verdicts[0].threshold_upper_exclusive);

    let below = Some(PrecedingSampleGrade::BelowDetection { limit: 0.5 });
    let or = BlankFailureMode::CarryoverOr {
        abs_threshold: 0.75,
        carryover_fraction: 0.5,
    };
    // The carryover threshold is strictly below 0.25, so equality with its upper bound FAILS.
    assert_eq!(
        statuses(
            &[
                measured(0.0, below),
                measured(0.125, below),
                measured(0.25, below)
            ],
            or
        ),
        [
            BlankStatus::PassMeasured,
            BlankStatus::Indeterminate,
            BlankStatus::FailContamination
        ]
    );

    // Clipping the uncertain threshold to the absolute threshold makes it exact at the contact.
    assert_eq!(
        statuses(
            &[measured(0.25, below)],
            BlankFailureMode::CarryoverAnd {
                abs_threshold: 0.25,
                carryover_fraction: 0.5,
            }
        ),
        [BlankStatus::PassMeasured]
    );
    assert_eq!(
        statuses(
            &[measured(0.25, upper)],
            BlankFailureMode::CarryoverOr {
                abs_threshold: 0.25,
                carryover_fraction: 0.5,
            }
        ),
        [BlankStatus::PassMeasured]
    );
}

#[test]
fn missing_predecessors_are_unknown_and_never_silently_remove_a_rule_condition() {
    let rows = [
        measured(0.0, None),
        measured(0.125, None),
        measured(0.375, None),
        measured(0.75, None),
    ];
    assert_eq!(
        statuses(
            &rows,
            BlankFailureMode::CarryoverAnd {
                abs_threshold: 0.25,
                carryover_fraction: 0.5,
            }
        ),
        [
            BlankStatus::PassMeasured,
            BlankStatus::PassMeasured,
            BlankStatus::Indeterminate,
            BlankStatus::FailContamination
        ]
    );
    assert_eq!(
        statuses(
            &rows,
            BlankFailureMode::CarryoverOr {
                abs_threshold: 0.25,
                carryover_fraction: 0.5,
            }
        ),
        [
            BlankStatus::PassMeasured,
            BlankStatus::Indeterminate,
            BlankStatus::FailContamination,
            BlankStatus::FailContamination
        ]
    );
}

#[test]
fn physical_upper_bound_can_prove_censored_pass_and_empty_batches_have_no_rate() {
    let policy = rule(BlankFailureMode::AboveMultipleOfDl {
        detection_limit: 0.5,
        dl_multiple: 2.0,
    });
    let result = evaluate_blanks(
        &[observation(AssayValue::AboveUpper { limit: 0.75 }, None)],
        &policy,
    )
    .unwrap();
    assert_eq!(result.verdicts[0].status, BlankStatus::PassCensored);
    assert_eq!(result.pass_count, 1);
    assert_eq!(result.failure_rate_pct, Some(0.0));
    assert_eq!(result.possible_failure_rate_pct, Some(0.0));
    let empty = evaluate_blanks(&[], &policy).unwrap();
    assert_eq!(empty.total_count, 0);
    assert_eq!(empty.failure_rate_pct, None);
    assert_eq!(empty.possible_failure_rate_pct, None);
}

#[test]
fn invalid_ranges_and_lost_positive_thresholds_refuse() {
    let policy = rule(BlankFailureMode::AboveMultipleOfDl {
        detection_limit: 0.125,
        dl_multiple: 2.0,
    });
    for value in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert!(evaluate_blanks(&[measured(value, None)], &policy).is_err());
        assert!(
            evaluate_blanks(
                &[measured(0.0, Some(PrecedingSampleGrade::Measured(value)))],
                &policy
            )
            .is_err()
        );
    }
    for limit in [0.0, -0.1, 1.1, f64::NAN, f64::INFINITY] {
        assert!(
            evaluate_blanks(
                &[observation(AssayValue::BelowDetection { limit }, None)],
                &policy
            )
            .is_err()
        );
        assert!(
            evaluate_blanks(
                &[observation(AssayValue::AboveUpper { limit }, None)],
                &policy
            )
            .is_err()
        );
        assert!(
            evaluate_blanks(
                &[measured(
                    0.0,
                    Some(PrecedingSampleGrade::BelowDetection { limit })
                )],
                &policy
            )
            .is_err()
        );
    }
    // There is no mass fraction strictly above one.
    assert!(
        evaluate_blanks(
            &[observation(AssayValue::AboveUpper { limit: 1.0 }, None)],
            &policy
        )
        .is_err()
    );
    assert!(
        evaluate_blanks(
            &[measured(
                0.0,
                Some(PrecedingSampleGrade::AboveUpper { limit: 1.0 })
            )],
            &policy
        )
        .is_err()
    );
    let tiny = f64::from_bits(1);
    let lost_dl = rule(BlankFailureMode::AboveMultipleOfDl {
        detection_limit: tiny,
        dl_multiple: 0.5,
    });
    assert!(evaluate_blanks(&[measured(0.0, None)], &lost_dl).is_err());
    let lost_carryover = rule(BlankFailureMode::CarryoverAnd {
        abs_threshold: 0.0,
        carryover_fraction: 0.5,
    });
    for preceding in [
        PrecedingSampleGrade::Measured(tiny),
        PrecedingSampleGrade::BelowDetection { limit: tiny },
    ] {
        assert!(evaluate_blanks(&[measured(0.0, Some(preceding))], &lost_carryover).is_err());
    }
}
