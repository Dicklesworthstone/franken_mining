//! Independent finite transport oracles and complete-population capital integration controls.
//! Determinism: Exact

use super::*;
use crate::tests::{
    claim, equity_claim, inputs, policy_claim, r, set_baseline_flow, set_candidate_flow,
    set_share_change, usd, with_equity, with_schedule,
};

fn design_claim(design: RobustnessDesign) -> Claim<RobustnessDesign> {
    let tag = design.canonical_tag();
    claim(
        "capital.robustness",
        design,
        &tag,
        AdditivityClass::NonAdditive,
    )
}

fn with_design(mut program: ProgramInputs, radius: Rat) -> ProgramInputs {
    let mut distances = Vec::new();
    for (from, _) in &program.weights.value.states {
        for (to, _) in &program.weights.value.states {
            distances.push(StateDistance {
                from_state: from.clone(),
                to_state: to.clone(),
                distance: if from == to { ZERO } else { ONE },
            });
        }
    }
    program.robustness = Some(design_claim(RobustnessDesign {
        model_key: program.policy.value.model_key.clone(),
        method: METHOD.into(),
        radius,
        metric_basis: "Invented unit distance between distinct complete states".into(),
        radius_basis: "Declared sensitivity budget; no confidence level inferred".into(),
        distances,
    }));
    program
}

fn example(radius: Rat) -> ProgramInputs {
    with_design(
        inputs(
            &[("risky", 1, &[20, -12]), ("safe", 1, &[8, 8])],
            &[r(3, 4), r(1, 4)],
            1,
            r(1, 2),
            None,
        ),
        radius,
    )
}

fn row<'a>(summary: &'a RobustnessSummary, id: &str) -> &'a RobustBundle {
    summary
        .bundles
        .iter()
        .find(|row| row.bundle_id == id)
        .unwrap()
}

/// Enumerate both directed masses independently. For this two-state rational fixture the feasible
/// polygon vertices lie on the quarter grid; this oracle never calls transport or capital helpers.
fn exhaustive_worst(values: [Rat; 2], radius: Rat) -> Rat {
    let mut best = None;
    for forward in 0..=3 {
        for backward in 0..=1 {
            if exact_compare(r(forward + backward, 4), radius).is_gt() {
                continue;
            }
            let left = r(3 - forward + backward, 4);
            let right = r(1 + forward - backward, 4);
            let value = values[0]
                .checked_mul(left)
                .unwrap()
                .checked_add(values[1].checked_mul(right).unwrap())
                .unwrap();
            if best.is_none_or(|old| exact_compare(value, old).is_gt()) {
                best = Some(value);
            }
        }
    }
    best.unwrap()
}

/// Independently check the primal and dual equalities and inequalities, including state indices.
fn witness(c: &TransportCertificate, values: [Rat; 2]) {
    let nominal = [r(3, 4), r(1, 4)];
    let mut source = [ZERO; 2];
    let mut destination = [ZERO; 2];
    let mut cost = ZERO;
    let mut objective = ZERO;
    for arc in &c.arcs {
        assert!(arc.source < 2 && arc.target < 2 && arc.mass.signum() > 0);
        source[arc.source] = source[arc.source].checked_add(arc.mass).unwrap();
        destination[arc.target] = destination[arc.target].checked_add(arc.mass).unwrap();
        if arc.source != arc.target {
            cost = cost.checked_add(arc.mass).unwrap();
        }
        objective = objective
            .checked_add(arc.mass.checked_mul(values[arc.target]).unwrap())
            .unwrap();
    }
    assert_eq!(source, nominal);
    assert_eq!(destination.as_slice(), c.destination_weights.as_slice());
    assert_eq!(cost, c.transport_cost);
    assert!(!exact_compare(cost, c.radius).is_gt());
    assert_eq!(objective, c.worst_case_value);
    assert!(c.dual_lambda.signum() >= 0);
    let mut dual = c.dual_lambda.checked_mul(c.radius).unwrap();
    for (i, mass) in nominal.iter().enumerate() {
        dual = dual
            .checked_add(mass.checked_mul(c.dual_potentials[i]).unwrap())
            .unwrap();
        for (j, value) in values.iter().enumerate() {
            let distance = if i == j { ZERO } else { ONE };
            let bound = value
                .checked_sub(c.dual_lambda.checked_mul(distance).unwrap())
                .unwrap();
            assert!(!exact_compare(c.dual_potentials[i], bound).is_lt());
        }
    }
    assert_eq!(dual, c.dual_value);
    assert_eq!(dual, objective);
    assert_eq!(c.duality_gap, ZERO);
}

#[test]
fn exact_oracle_and_primal_dual_certificates_cover_radius_zero_and_saturation() {
    for radius in [ZERO, r(1, 4), r(1, 2), ONE] {
        let evaluated = evaluate(&example(radius)).unwrap();
        let summary = evaluated.summary().robustness.as_ref().unwrap();
        assert_eq!(summary.state_ids, ["s0", "s1"]);
        assert_eq!(summary.nominal_weights, [r(3, 4), r(1, 4)]);
        for (id, losses, regrets) in [
            ("bundle_00", [ZERO, ZERO], [r(20, 1), r(8, 1)]),
            ("bundle_01", [r(-20, 1), r(12, 1)], [ZERO, r(20, 1)]),
            ("bundle_02", [r(-8, 1), r(-8, 1)], [r(12, 1), ZERO]),
        ] {
            let bundle = row(summary, id);
            assert_eq!(
                bundle.worst_case_mean,
                ZERO.checked_sub(exhaustive_worst(losses, radius)).unwrap()
            );
            assert_eq!(
                bundle.worst_case_expected_regret,
                exhaustive_worst(regrets, radius)
            );
            witness(&bundle.loss_certificate, losses);
            witness(&bundle.regret_certificate, regrets);
            if radius == ZERO {
                assert_eq!(bundle.worst_case_mean, bundle.nominal_mean);
                assert_eq!(
                    bundle.worst_case_expected_regret,
                    bundle.nominal_mean_regret
                );
            }
        }
        if radius == ZERO {
            assert_eq!(
                summary.maximin_mean_winners,
                evaluated.summary().feasible_winners
            );
        }
    }
}

#[test]
fn expected_value_and_regret_choose_different_stress_winners_without_changing_nominal_choice() {
    let program = example(r(1, 4));
    let evaluated = evaluate(&program).unwrap();
    let summary = evaluated.summary().robustness.as_ref().unwrap();
    assert_eq!(evaluated.summary().feasible_winners, ["bundle_01"]);
    assert_eq!(summary.nominal_expected_value_winners, ["bundle_01"]);
    assert_eq!(summary.maximin_mean_winners, ["bundle_02"]);
    assert_eq!(summary.minimax_expected_regret_winners, ["bundle_01"]);
    assert_eq!(row(summary, "bundle_01").nominal_mean, r(12, 1));
    assert_eq!(row(summary, "bundle_01").worst_case_mean, r(4, 1));
    assert_eq!(
        row(summary, "bundle_01").worst_case_expected_regret,
        r(10, 1)
    );
    assert_eq!(row(summary, "bundle_02").worst_case_mean, r(8, 1));
    assert_eq!(
        row(summary, "bundle_02").worst_case_expected_regret,
        r(12, 1)
    );
    assert!(!summary.nominal_winner_survives_maximin);
    assert!(summary.nominal_winner_survives_minimax_regret);
    assert_eq!(summary.structural_rejections, ["bundle_03"]);
    assert_eq!(evaluated.summary().choices.len(), 4);
    let mut plain = program;
    plain.robustness = None;
    let baseline = evaluate(&plain).unwrap();
    assert_eq!(baseline.summary().choices, evaluated.summary().choices);
    assert!(!baseline.to_json().contains("\"robustness\""));
    assert!(
        !baseline
            .to_markdown()
            .contains("Capital scenario-weight robustness")
    );
    assert!(
        evaluated
            .to_json()
            .contains("\"stressed_tail_loss_feasibility_certified\":false")
    );
    assert!(evaluated.to_markdown().contains("Source dual potential"));
}

#[test]
fn exact_metric_scaling_retains_all_ties() {
    let mut program = example(r(1, 4));
    let mut design = program.robustness.take().unwrap().value;
    for distance in &mut design.distances {
        distance.distance = distance.distance.checked_mul(r(2, 1)).unwrap();
    }
    program.robustness = Some(design_claim(design));
    let evaluated = evaluate(&program).unwrap();
    let summary = evaluated.summary().robustness.as_ref().unwrap();
    assert_eq!(summary.maximin_mean_winners, ["bundle_01", "bundle_02"]);
    assert_eq!(row(summary, "bundle_01").worst_case_mean, r(8, 1));
    assert_eq!(row(summary, "bundle_02").worst_case_mean, r(8, 1));
}

#[test]
fn nominal_risk_rejections_never_enter_stressed_hindsight_regret() {
    let mut program = example(r(1, 4));
    let mut policy = program.policy.value.clone();
    policy.tail_mass = r(1, 4);
    policy.max_tail_loss = Some(usd(0));
    program.policy = policy_claim(policy);
    let evaluated = evaluate(&program).unwrap();
    let summary = evaluated.summary().robustness.as_ref().unwrap();
    assert_eq!(summary.nominal_risk_rejections, ["bundle_01"]);
    assert_eq!(summary.bundles.len(), 2);
    assert_eq!(row(summary, "bundle_02").nominal_mean_regret, ZERO);
    assert_eq!(row(summary, "bundle_02").worst_case_expected_regret, ZERO);
    assert_eq!(summary.minimax_expected_regret_winners, ["bundle_02"]);
    let safe = evaluated
        .summary()
        .choices
        .iter()
        .find(|row| row.bundle_id == "bundle_02")
        .unwrap();
    assert_eq!(safe.statistics.as_ref().unwrap().mean_regret.major, r(9, 1));
}

#[test]
fn exact_nav_per_actual_share_uses_whole_bundle_denominators() {
    let mut program = with_equity(example(r(1, 4)), &[1000, 1000], r(100, 1));
    let mut equity = program.equity.take().unwrap().value;
    set_share_change(&mut equity, "risky", "s0", r(-10, 1));
    set_share_change(&mut equity, "risky", "s1", r(20, 1));
    program.equity = Some(equity_claim(equity));
    let evaluated = evaluate(&program).unwrap();
    let summary = evaluated.summary().robustness.as_ref().unwrap();
    assert_eq!(summary.objective_unit, "USD/share");
    // Risky: 1020/90 - 10 = 4/3; 988/120 - 10 = -53/30.
    assert_eq!(row(summary, "bundle_01").nominal_mean, r(67, 120));
    assert_eq!(row(summary, "bundle_01").worst_case_mean, r(-13, 60));
    assert_eq!(row(summary, "bundle_02").nominal_mean, r(2, 25));
    assert_eq!(summary.nominal_expected_value_winners, ["bundle_01"]);
    assert_eq!(summary.maximin_mean_winners, ["bundle_02"]);
}

#[test]
fn input_permutations_preserve_named_state_alignment_and_all_bytes() {
    let program = example(r(1, 4));
    let original = evaluate(&program).unwrap();
    let mut permuted = program;
    permuted.weights.value.states.reverse();
    permuted.candidates.reverse();
    for candidate in &mut permuted.candidates {
        candidate.value.payoffs.reverse();
    }
    permuted
        .robustness
        .as_mut()
        .unwrap()
        .value
        .distances
        .reverse();
    let reordered = evaluate(&permuted).unwrap();
    assert_eq!(original.summary(), reordered.summary());
    assert_eq!(original.digest(), reordered.digest());
    assert_eq!(original.to_json(), reordered.to_json());
    assert_eq!(original.to_markdown(), reordered.to_markdown());
}

#[test]
fn no_nominally_feasible_population_keeps_empty_results_and_full_geometry() {
    let program = with_design(
        inputs(
            &[("loss", 1, &[-2, -3])],
            &[r(3, 4), r(1, 4)],
            1,
            r(1, 2),
            Some(-1),
        ),
        r(1, 4),
    );
    let evaluated = evaluate(&program).unwrap();
    let summary = evaluated.summary().robustness.as_ref().unwrap();
    assert!(summary.bundles.is_empty());
    assert!(summary.maximin_mean_winners.is_empty());
    assert!(summary.minimax_expected_regret_winners.is_empty());
    assert_eq!(summary.nominal_risk_rejections, ["bundle_00", "bundle_01"]);
    assert_eq!(summary.design.distances.len(), 4);
    assert_eq!(evaluated.summary().choices.len(), 2);
    assert!(evaluated.to_json().contains("no_nominally_feasible_bundle"));
}

#[test]
fn a_cash_required_negative_survivor_keeps_signed_values_and_zero_regret() {
    let mut program = with_schedule(with_design(
        inputs(
            &[("bridge", 0, &[-2, -3])],
            &[r(3, 4), r(1, 4)],
            0,
            r(1, 2),
            None,
        ),
        r(1, 4),
    ));
    let mut schedule = program.funding_schedule.take().unwrap().value;
    for state in ["s0", "s1"] {
        set_baseline_flow(&mut schedule, "start", state, -90);
        set_candidate_flow(&mut schedule, "start", "bridge", state, 50);
    }
    program.funding_schedule = Some(crate::tests::schedule_claim(schedule));
    let evaluated = evaluate(&program).unwrap();
    let summary = evaluated.summary().robustness.as_ref().unwrap();
    assert_eq!(summary.bundles.len(), 1);
    assert_eq!(summary.structural_rejections, ["bundle_00"]);
    assert_eq!(summary.maximin_mean_winners, ["bundle_01"]);
    assert_eq!(row(summary, "bundle_01").worst_case_mean, r(-5, 2));
    assert_eq!(row(summary, "bundle_01").worst_case_expected_regret, ZERO);
}

#[test]
fn invalid_geometry_and_stale_claims_refuse_before_emitting_a_comparison() {
    for mode in 0..7 {
        let mut program = example(r(1, 4));
        let mut design = program.robustness.take().unwrap().value;
        match mode {
            0 => design.radius = r(-1, 1),
            1 => design.method = "estimated".into(),
            2 => design.model_key = "foreign".into(),
            3 => design.distances[0].distance = ONE,
            4 => design.distances[1].distance = r(2, 1),
            5 => design.distances[1].to_state = "absent".into(),
            _ => design.distances[1] = design.distances[0].clone(),
        }
        program.robustness = Some(design_claim(design));
        assert!(evaluate(&program).is_err(), "mode {mode}");
    }
    let mut stale = example(r(1, 4));
    stale.robustness.as_mut().unwrap().value.radius = r(1, 2);
    assert_eq!(evaluate(&stale).unwrap_err().code, "FMINE-REF-507");
    let mut oversized = example(r(1, 4));
    oversized.robustness.as_mut().unwrap().value.distances.pop();
    assert_eq!(evaluate(&oversized).unwrap_err().code, "FMINE-REF-201");
}

#[test]
fn complete_population_work_bound_is_checked_before_transport_or_claim_hashing() {
    let values = vec![0; 64];
    let weights = vec![r(1, 64); 64];
    let mut program = with_design(
        inputs(&[("one", 0, &values)], &weights, 0, r(1, 2), None),
        ZERO,
    );
    program.candidates = vec![program.candidates[0].clone(); 8];
    assert!(check_work(&program).is_ok()); // 2 * 256 * 64^2 = 2,097,152.
    program.candidates.push(program.candidates[0].clone());
    assert_eq!(check_work(&program).unwrap_err().code, "FMINE-REF-201");
    program.candidates.pop();
    program
        .weights
        .value
        .states
        .push(("excess".into(), r(1, 65)));
    assert_eq!(check_work(&program).unwrap_err().code, "FMINE-REF-201");
}

#[test]
fn stress_and_bootstrap_coexist_and_both_source_claims_remain_bound() {
    let mut program = example(r(1, 4));
    let design = stability::SelectionStabilityDesign {
        model_key: program.policy.value.model_key.clone(),
        method: stability::METHOD.into(),
        seed: 7,
        replicates: 200,
        draws_per_replicate: 4,
        sampling_basis: "Conditional synthetic resampling; not empirical observations".into(),
    };
    let tag = design.canonical_tag();
    program.selection_stability = Some(claim(
        "capital.stability",
        design,
        &tag,
        AdditivityClass::NonAdditive,
    ));
    let report = evaluate(&program).unwrap();
    let mut without = program.clone();
    without.robustness = None;
    let baseline = evaluate(&without).unwrap();
    assert_eq!(
        report.summary().selection_stability,
        baseline.summary().selection_stability
    );
    assert_eq!(
        report.summary().feasible_winners,
        baseline.summary().feasible_winners
    );
    assert!(
        report
            .claim()
            .parents
            .contains(&program.robustness.as_ref().unwrap().claim_id)
    );
    assert!(
        report
            .claim()
            .parents
            .contains(&program.selection_stability.as_ref().unwrap().claim_id)
    );
    let mut changed = program.clone();
    changed.robustness.as_mut().unwrap().supersedes = Some(changed.policy.claim_id);
    let next = evaluate(&changed).unwrap();
    assert_eq!(next.summary(), report.summary());
    assert_ne!(next.digest(), report.digest());
    let mut design = program.robustness.take().unwrap().value;
    design
        .radius_basis
        .push_str("; an additional declared limitation");
    program.robustness = Some(design_claim(design));
    let altered = evaluate(&program).unwrap();
    assert_eq!(
        altered.summary().robustness.as_ref().unwrap().bundles,
        report.summary().robustness.as_ref().unwrap().bundles
    );
    assert_ne!(altered.digest(), report.digest());
}
