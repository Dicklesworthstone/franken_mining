//! Independent known answers for exact threshold economics and ratio aggregation.
//! Determinism: Exact.

use super::*;
use crate::tests::{claim, inputs, r, with_equity};

fn design(program: &EvaluatedProgram, limit: Rat) -> Design {
    let states = &program.inputs().weights.value.states;
    Design {
        model_key: program.inputs().policy.value.model_key.clone(),
        constraints: vec![Constraint {
            id: "aisc".into(), metric: Metric::GoldAisc,
            basis: "Synthetic declared reconciled AISC numerator on attributable sold gold ounces".into(),
            period: "2030-01-01/2030-12-31".into(), aggregation: Aggregation::RatioOfExpectedTotals,
            direction: Direction::Upper, limit,
            baseline: states.iter().map(|(state, _)| Baseline { state_id: state.clone(), numerator: r(100, 1), denominator: r(100, 1) }).collect(),
            increments: program.inputs().candidates.iter().flat_map(|candidate| {
                states.iter().map(move |(state, _)| Increment {
                    candidate_id: candidate.value.candidate_id.clone(), state_id: state.clone(),
                    numerator: r(if candidate.value.candidate_id == "growth" { 300 } else { 100 }, 1), denominator: r(100, 1),
                })
            }).collect(),
        }],
    }
}

fn source(value: Design) -> Claim<Design> {
    let tag = value.canonical_tag();
    claim("capital.kpis", value, &tag, AdditivityClass::NonAdditive)
}

fn example() -> EvaluatedProgram {
    crate::evaluate(&inputs(&[("efficient", 1, &[80]), ("growth", 1, &[100])], &[ONE], 1, r(1, 5), None)).unwrap()
}

fn row<'a>(result: &'a EvaluatedKpiProgram, bundle: &str) -> &'a Choice {
    result.summary().choices.iter().find(|r| r.bundle_id == bundle).unwrap()
}

#[test]
fn kpi_cap_changes_the_choice_and_prices_every_exact_threshold() {
    let program = example();
    let original = program.to_json();
    let report = evaluate(&program, &source(design(&program, r(3, 2)))).unwrap();
    let s = report.summary();
    assert_eq!(s.baseline_winners, ["bundle_02"]);
    assert_eq!(s.constrained_winners, ["bundle_01"]);
    assert_eq!(s.baseline_objective, Some(r(100, 1)));
    assert_eq!(s.constrained_objective, Some(r(80, 1)));
    assert_eq!(s.opportunity_cost, Some(r(20, 1)));
    assert_eq!(row(&report, "bundle_02").metrics[0].threshold_value, Some(r(2, 1)));
    assert_eq!(row(&report, "bundle_03").metrics[0].threshold_value, Some(r(5, 3)));
    assert!(!row(&report, "bundle_03").base_eligible);
    let f = &s.frontiers[0];
    assert_eq!(f.points.iter().map(|p| p.limit).collect::<Vec<_>>(), [ONE, r(3, 2), r(2, 1)]);
    assert_eq!(f.points[2].winners, ["bundle_02"]);
    assert_eq!(f.points[2].opportunity_cost, Some(ZERO));
    assert_eq!(program.to_json(), original);
    assert!(report.to_json().contains("\"execution_authorized\":false"));
    assert!(report.to_markdown().contains("finite economic differences"));
}

#[test]
fn ratio_of_expected_totals_is_not_the_average_of_state_ratios() {
    let program = crate::evaluate(&inputs(&[("growth", 1, &[100, 100])], &[r(1, 2), r(1, 2)], 1, r(1, 5), None)).unwrap();
    let mut d = design(&program, r(2, 1));
    d.constraints[0].baseline[0].denominator = r(10, 1);
    d.constraints[0].baseline[1].denominator = r(100, 1);
    for delta in &mut d.constraints[0].increments {
        delta.numerator = ZERO;
        delta.denominator = ZERO;
    }
    let expected = evaluate(&program, &source(d.clone())).unwrap();
    assert_eq!(row(&expected, "bundle_01").metrics[0].threshold_value, Some(r(20, 11)));
    assert_eq!(expected.summary().constrained_winners, ["bundle_01"]);
    d.constraints[0].aggregation = Aggregation::EveryState;
    let every = evaluate(&program, &source(d)).unwrap();
    assert_eq!(row(&every, "bundle_01").metrics[0].threshold_value, Some(r(10, 1)));
    assert!(every.summary().constrained_winners.is_empty());
    assert_eq!(every.summary().opportunity_cost, None);
}

#[test]
fn frontiers_hold_other_kpis_fixed_and_empty_sets_never_invent_zero_cost() {
    let program = example();
    let mut d = design(&program, r(3, 2));
    let mut roce = d.constraints[0].clone();
    roce.id = "roce".into();
    roce.metric = Metric::Roce;
    roce.direction = Direction::Lower;
    roce.limit = r(1, 5);
    roce.baseline[0].numerator = r(10, 1);
    for delta in &mut roce.increments {
        delta.numerator = r(if delta.candidate_id == "growth" { 50 } else { 10 }, 1);
    }
    d.constraints.push(roce);
    let report = evaluate(&program, &source(d)).unwrap();
    assert!(report.summary().constrained_winners.is_empty());
    assert_eq!(report.summary().constrained_objective, None);
    assert_eq!(report.summary().opportunity_cost, None);
    let cap = &report.summary().frontiers[0];
    assert_eq!(cap.points.last().unwrap().winners, ["bundle_02"]);
    let floor = &report.summary().frontiers[1];
    assert_eq!(floor.points.last().unwrap().winners, ["bundle_01"]);
    assert_eq!(floor.points.last().unwrap().opportunity_cost, Some(r(20, 1)));
    assert!(floor.points.windows(2).all(|w| exact_compare(w[0].limit, w[1].limit).is_gt()));
}

#[test]
fn ties_keep_every_winner_and_the_minimum_economic_sacrifice() {
    let program = crate::evaluate(&inputs(&[("efficient", 1, &[100]), ("growth", 1, &[100])], &[ONE], 1, r(1, 5), None)).unwrap();
    let tight = evaluate(&program, &source(design(&program, r(3, 2)))).unwrap();
    assert_eq!(tight.summary().baseline_winners, ["bundle_01", "bundle_02"]);
    assert_eq!(tight.summary().constrained_winners, ["bundle_01"]);
    assert_eq!(tight.summary().opportunity_cost, Some(ZERO));
    let relaxed = evaluate(&program, &source(design(&program, r(2, 1)))).unwrap();
    assert_eq!(relaxed.summary().constrained_winners, ["bundle_01", "bundle_02"]);
}

#[test]
fn nominal_risk_rejections_and_cash_failures_cannot_be_resurrected_by_kpis() {
    let program = crate::evaluate(&inputs(&[("efficient", 1, &[80, 80]), ("growth", 1, &[-100, 400])], &[r(1, 2), r(1, 2)], 1, r(1, 2), Some(50))).unwrap();
    let report = evaluate(&program, &source(design(&program, r(5, 1)))).unwrap();
    assert!(!row(&report, "bundle_02").base_eligible);
    assert!(report.summary().frontiers[0].points.iter().all(|p| p.winners == ["bundle_01"]));
    let mut i = crate::tests::with_schedule(program.inputs().clone());
    let mut schedule = i.funding_schedule.take().unwrap().value;
    crate::tests::set_candidate_flow(&mut schedule, "start", "efficient", "s0", -200);
    i.funding_schedule = Some(crate::tests::schedule_claim(schedule));
    let cash = crate::evaluate(&i).unwrap();
    let report = evaluate(&cash, &source(design(&cash, r(5, 1)))).unwrap();
    assert_eq!(report.summary().constrained_winners, ["bundle_00"]);
}

#[test]
fn equity_objective_is_priced_in_actual_usd_per_share() {
    let i = with_equity(example().inputs().clone(), &[1000], r(100, 1));
    let program = crate::evaluate(&i).unwrap();
    let report = evaluate(&program, &source(design(&program, r(3, 2)))).unwrap();
    assert_eq!(report.summary().objective_unit, "USD/share");
    assert_eq!(report.summary().baseline_objective, Some(ONE));
    assert_eq!(report.summary().constrained_objective, Some(r(4, 5)));
    assert_eq!(report.summary().opportunity_cost, Some(r(1, 5)));
}

#[test]
fn complete_state_admission_and_positive_denominators_never_drop_a_portfolio() {
    let program = example();
    let good = design(&program, r(3, 2));
    let mut d = good.clone();
    d.constraints[0].increments.pop();
    assert_eq!(evaluate(&program, &source(d)).unwrap_err().code, "FMINE-REF-201");
    let mut d = good.clone();
    d.constraints[0].increments[0].state_id = "missing".into();
    assert_eq!(evaluate(&program, &source(d)).unwrap_err().code, "FMINE-REF-105");
    let mut d = good.clone();
    d.constraints[0].increments[0].denominator = r(-100, 1);
    assert_eq!(evaluate(&program, &source(d)).unwrap_err().code, "FMINE-REF-107");
    let mut d = good.clone();
    d.model_key = "other-model".into();
    assert_eq!(evaluate(&program, &source(d)).unwrap_err().code, "FMINE-REF-105");
    let mut d = source(good);
    d.value.constraints[0].limit = r(2, 1);
    assert_eq!(evaluate(&program, &d).unwrap_err().code, "FMINE-REF-507");
}

#[test]
fn geo_requires_declared_fixed_denominators_and_fractional_threshold_equality_passes() {
    let program = example();
    let mut d = design(&program, r(400, 1));
    let c = &mut d.constraints[0];
    c.metric = Metric::Geo;
    c.direction = Direction::Lower;
    c.baseline[0].denominator = ONE;
    for delta in &mut c.increments { delta.denominator = ZERO; }
    let report = evaluate(&program, &source(d.clone())).unwrap();
    assert_eq!(report.summary().constrained_winners, ["bundle_02"]);
    assert_eq!(row(&report, "bundle_02").metrics[0].threshold_value, Some(r(400, 1)));
    d.constraints[0].increments[0].denominator = ONE;
    assert_eq!(evaluate(&program, &source(d)).unwrap_err().code, "FMINE-REF-107");
}

#[test]
fn metadata_and_sources_bind_the_result_without_promoting_evidence() {
    let program = example();
    let design = source(design(&program, r(3, 2)));
    let report = evaluate(&program, &design).unwrap();
    assert_eq!(report.claim().evidence, EvidenceClass::Assumed);
    assert_eq!(report.claim().confidentiality, Confidentiality::internal());
    assert!(report.claim().parents.contains(&program.claim().claim_id));
    assert!(report.claim().parents.contains(&design.claim_id));
    assert!(design.anchors.iter().all(|a| report.claim().anchors.contains(a)));
    let mut edited = design.clone();
    edited.uu_terms[0].downside_exposure_usd_m += 1.0;
    // The original and KPI inputs cannot silently disagree about one UU scenario.
    assert_eq!(evaluate(&program, &edited).unwrap_err().code, "FMINE-REF-204");
    let mut edited = design;
    edited.calibration_failed = true;
    let changed = evaluate(&program, &edited).unwrap();
    assert!(changed.claim().calibration_failed);
    assert_ne!(changed.digest(), report.digest());
    let mut failed_base = program.inputs().clone();
    failed_base.candidates[0].calibration_failed = true;
    let failed_base = crate::evaluate(&failed_base).unwrap();
    let inherited = evaluate(&failed_base, report.design()).unwrap();
    assert!(inherited.claim().calibration_failed);
}

#[test]
fn numerator_and_denominator_rescaling_preserves_every_threshold_choice() {
    let program = example();
    let d = design(&program, r(3, 2));
    let original = evaluate(&program, &source(d.clone())).unwrap();
    let mut scaled = d;
    for row in &mut scaled.constraints[0].baseline {
        row.numerator = row.numerator.checked_mul(r(7, 3)).unwrap();
        row.denominator = row.denominator.checked_mul(r(7, 3)).unwrap();
    }
    for row in &mut scaled.constraints[0].increments {
        row.numerator = row.numerator.checked_mul(r(7, 3)).unwrap();
        row.denominator = row.denominator.checked_mul(r(7, 3)).unwrap();
    }
    let scaled = evaluate(&program, &source(scaled)).unwrap();
    assert_eq!(scaled.summary().frontiers, original.summary().frontiers);
    assert_eq!(scaled.summary().constrained_winners, original.summary().constrained_winners);
}

#[test]
fn unrepresentable_component_sums_refuse_instead_of_saturating_or_dropping_bundles() {
    let program = example();
    let mut d = design(&program, r(3, 2));
    d.constraints[0].baseline[0].numerator = Rat::from_integer(i128::MAX);
    assert_eq!(evaluate(&program, &source(d)).unwrap_err().code, "FMINE-REF-207");
}

#[test]
fn component_row_order_never_changes_the_canonical_report() {
    let program = crate::evaluate(&inputs(&[("efficient", 1, &[80, 80]), ("growth", 1, &[100, 100])], &[r(1, 2), r(1, 2)], 1, r(1, 5), None)).unwrap();
    let original = source(design(&program, r(3, 2)));
    let mut reordered = original.clone();
    reordered.value.constraints[0].baseline.reverse();
    reordered.value.constraints[0].increments.reverse();
    let a = evaluate(&program, &original).unwrap();
    let b = evaluate(&program, &reordered).unwrap();
    assert_eq!(a.digest(), b.digest());
    assert_eq!(a.to_json(), b.to_json());
    assert_eq!(a.to_markdown(), b.to_markdown());
}
