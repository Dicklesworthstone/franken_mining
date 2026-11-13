//! Independent known answers for stress of complete, nonanticipating capital policies.
//! Determinism: Exact

use super::*;
use crate::robustness::{self as stress, RobustnessDesign, RobustnessScope, StateDistance};
use fmine_numerics::risk::transport::TransportCertificate;

fn design_claim(design: RobustnessDesign) -> Claim<RobustnessDesign> {
    let tag = design.canonical_tag();
    claim(
        "capital.robustness",
        design,
        &tag,
        AdditivityClass::NonAdditive,
    )
}

fn with_stress(mut inputs: RecourseInputs, radius: Rat) -> RecourseInputs {
    let mut distances = Vec::new();
    for (from, _) in &inputs.program.weights.value.states {
        for (to, _) in &inputs.program.weights.value.states {
            distances.push(StateDistance {
                from_state: from.clone(),
                to_state: to.clone(),
                distance: if from == to { ZERO } else { ONE },
            });
        }
    }
    inputs.program.robustness = Some(design_claim(RobustnessDesign {
        model_key: inputs.program.policy.value.model_key.clone(),
        method: stress::METHOD.into(),
        radius,
        metric_basis: "Synthetic unit distance between complete states".into(),
        radius_basis: "Declared sensitivity; not a confidence radius".into(),
        distances,
    }));
    inputs
}

fn result<'a>(report: &'a EvaluatedRecourseProgram, id: &str) -> &'a stress::RobustBundle {
    report
        .summary()
        .robustness
        .as_ref()
        .unwrap()
        .bundles
        .iter()
        .find(|row| row.bundle_id == id)
        .unwrap()
}

/// The two-state feasible polygon has vertices on this eighth-mass grid at the tested radii.
fn enumerate_worst(values: [Rat; 2], radius: Rat) -> Rat {
    let mut best = None;
    for forward in 0..=4 {
        for backward in 0..=4 {
            if exact_compare(r(forward + backward, 8), radius).is_gt() {
                continue;
            }
            let value = values[0]
                .checked_mul(r(4 - forward + backward, 8))
                .unwrap()
                .checked_add(values[1].checked_mul(r(4 + forward - backward, 8)).unwrap())
                .unwrap();
            if best.is_none_or(|old| exact_compare(value, old).is_gt()) {
                best = Some(value);
            }
        }
    }
    best.unwrap()
}

/// Check the full primal and dual witness independently against the declared unit-distance geometry.
fn witness(cert: &TransportCertificate, values: [Rat; 2]) {
    let mut sources = [ZERO; 2];
    let mut targets = [ZERO; 2];
    let mut cost = ZERO;
    for arc in &cert.arcs {
        assert!(arc.source < 2 && arc.target < 2 && arc.mass.signum() > 0);
        sources[arc.source] = sources[arc.source].checked_add(arc.mass).unwrap();
        targets[arc.target] = targets[arc.target].checked_add(arc.mass).unwrap();
        if arc.source != arc.target {
            cost = cost.checked_add(arc.mass).unwrap();
        }
    }
    assert_eq!(sources, [r(1, 2); 2]);
    assert_eq!(targets.as_slice(), cert.destination_weights.as_slice());
    assert_eq!(cost, cert.transport_cost);
    assert!(!exact_compare(cost, cert.radius).is_gt());
    let primal = targets[0]
        .checked_mul(values[0])
        .unwrap()
        .checked_add(targets[1].checked_mul(values[1]).unwrap())
        .unwrap();
    let mut dual = cert.radius.checked_mul(cert.dual_lambda).unwrap();
    assert!(cert.dual_lambda.signum() >= 0);
    for (i, potential) in cert.dual_potentials.iter().enumerate() {
        dual = dual
            .checked_add(potential.checked_mul(r(1, 2)).unwrap())
            .unwrap();
        for (j, value) in values.iter().enumerate() {
            let bound = potential
                .checked_add(if i == j { ZERO } else { cert.dual_lambda })
                .unwrap();
            assert!(!exact_compare(bound, *value).is_lt());
        }
    }
    assert_eq!(cert.worst_case_value, primal);
    assert_eq!(cert.dual_value, dual);
    assert_eq!(primal, dual);
    assert_eq!(cert.duality_gap, ZERO);
}

#[test]
fn complete_policies_match_independent_transport_vertices_and_paid_information_known_answers() {
    let baseline = evaluate(&study_example()).unwrap();
    for radius in [ZERO, r(1, 8), r(1, 4), r(1, 2), ONE] {
        let report = evaluate(&with_stress(study_example(), radius)).unwrap();
        assert_eq!(report.summary().policies, baseline.summary().policies);
        assert_eq!(
            report.summary().feasible_winners,
            baseline.summary().feasible_winners
        );
        assert_eq!(
            report.summary().net_adaptive_value,
            baseline.summary().net_adaptive_value
        );
        let summary = report.summary().robustness.as_ref().unwrap();
        assert_eq!(summary.scope, RobustnessScope::AdaptivePolicies);
        assert_eq!(summary.state_ids, ["s0", "s1"]);
        assert_eq!(summary.nominal_weights, [r(1, 2); 2]);
        assert_eq!(summary.bundles.len(), 5);
        assert_eq!(
            summary.structural_rejections,
            [common(&report, &["build"]).policy_id.clone()]
        );
        let hold = common(&report, &[]);
        let bought = adaptive(&report, &[("good", &["build"]), ("bad", &[])]);
        let both = adaptive(&report, &[("good", &["build"]), ("bad", &["build"])]);
        let neither = adaptive(&report, &[("good", &[]), ("bad", &[])]);
        let wrong = adaptive(&report, &[("good", &[]), ("bad", &["build"])]);
        // Fixed utilities include the sunk USD 10 study on both states of every paid policy.
        for (policy, payoffs) in [
            (hold, [0, 0]),
            (bought, [90, -10]),
            (both, [90, -70]),
            (neither, [-10, -10]),
            (wrong, [-10, -70]),
        ] {
            let row = result(&report, &policy.policy_id);
            let losses = [r(-payoffs[0], 1), r(-payoffs[1], 1)];
            // Only feasible policies define hindsight: good can earn 90, bad can hold for 0.
            let regrets = [r(90 - payoffs[0], 1), r(-payoffs[1], 1)];
            assert_eq!(row.nominal_mean, r(payoffs[0] + payoffs[1], 2));
            assert_eq!(
                row.worst_case_mean,
                ZERO.checked_sub(enumerate_worst(losses, radius)).unwrap()
            );
            assert_eq!(
                row.worst_case_expected_regret,
                enumerate_worst(regrets, radius)
            );
            witness(&row.loss_certificate, losses);
            witness(&row.regret_certificate, regrets);
        }
        if radius == ZERO {
            assert_eq!(
                summary.maximin_mean_winners,
                report.summary().feasible_winners
            );
        }
        if radius == r(1, 4) {
            let row = result(&report, &bought.policy_id);
            assert_eq!(row.nominal_mean, r(40, 1));
            assert_eq!(row.worst_case_mean, r(15, 1));
            assert_eq!(row.nominal_mean_regret, r(5, 1));
            assert_eq!(row.worst_case_expected_regret, r(15, 2));
            assert_eq!(row.loss_certificate.destination_weights, [r(1, 4), r(3, 4)]);
            assert_eq!(
                summary.maximin_mean_winners,
                std::slice::from_ref(&bought.policy_id)
            );
            assert_eq!(
                summary.minimax_expected_regret_winners,
                std::slice::from_ref(&bought.policy_id)
            );
            assert_eq!(bought.branches[0].probability, r(1, 2));
            assert_eq!(bought.branches[1].probability, r(1, 2));
        }
        if radius == r(1, 2) {
            assert_eq!(
                summary.maximin_mean_winners,
                std::slice::from_ref(&hold.policy_id)
            );
            assert_eq!(
                summary.minimax_expected_regret_winners,
                std::slice::from_ref(&bought.policy_id)
            );
            assert!(!summary.nominal_winner_survives_maximin);
            assert!(summary.nominal_winner_survives_minimax_regret);
        }
        assert_eq!(
            report
                .static_program()
                .summary()
                .robustness
                .as_ref()
                .unwrap()
                .scope,
            RobustnessScope::StaticBundles
        );
        assert!(
            report
                .to_json()
                .contains("\"scope\":\"nominally_feasible_adaptive_policies\"")
        );
        assert!(summary.to_json().contains("\"policy_id\""));
        assert!(!summary.to_json().contains("\"bundle_id\""));
        assert!(
            report
                .to_markdown()
                .contains("## Capital policy scenario-weight robustness")
        );
    }
}

#[test]
fn stress_cannot_split_shared_observations_or_repair_nominal_risk_and_cash_failures() {
    let mut grouped = with_stress(study_example(), r(1, 4));
    edit_information(&mut grouped, |basis| {
        for row in &mut basis.observations {
            row.observation_id = "same".into();
        }
    });
    let report = evaluate(&grouped).unwrap();
    assert_eq!(report.summary().policies.len(), 4);
    assert!(
        report
            .summary()
            .policies
            .iter()
            .all(|policy| policy.branches.len() == 1)
    );
    assert_eq!(
        report
            .summary()
            .robustness
            .as_ref()
            .unwrap()
            .maximin_mean_winners,
        [common(&report, &[]).policy_id.clone()]
    );

    let mut limited = with_stress(study_example(), r(1, 4));
    let mut policy = limited.program.policy.value.clone();
    policy.max_tail_loss = Some(usd(0));
    limited.program.policy = policy_claim(policy);
    let report = evaluate(&limited).unwrap();
    let summary = report.summary().robustness.as_ref().unwrap();
    assert_eq!(summary.nominal_risk_rejections.len(), 4);
    assert_eq!(summary.bundles.len(), 1);
    assert_eq!(summary.bundles[0].worst_case_expected_regret, ZERO);
    assert_eq!(summary.bundles[0].nominal_mean_regret, ZERO);
    assert_eq!(summary.bundles[0].bundle_id, common(&report, &[]).policy_id);

    let mut no_cash = with_stress(study_example(), ONE);
    edit_cash(&mut no_cash, |schedule| {
        for (_, cash) in &mut schedule.opening_cash {
            *cash = usd(0);
        }
    });
    let report = evaluate(&no_cash).unwrap();
    let summary = report.summary().robustness.as_ref().unwrap();
    assert!(summary.bundles.is_empty());
    assert_eq!(summary.structural_rejections.len(), 6);
    assert!(summary.maximin_mean_winners.is_empty());
    assert!(summary.minimax_expected_regret_winners.is_empty());
    assert!(summary.to_json().contains("no_nominally_feasible_policy"));
}

#[test]
fn per_share_stress_uses_realized_whole_policy_denominators() {
    let mut inputs = with_stress(study_example(), r(1, 4));
    inputs.program = with_equity(inputs.program, &[1000, 1000], r(100, 1));
    let mut equity = inputs.program.equity.take().unwrap().value;
    set_share_change(&mut equity, "build", "s0", r(-10, 1));
    set_share_change(&mut equity, "build", "s1", r(20, 1));
    inputs.program.equity = Some(equity_claim(equity));
    let report = evaluate(&inputs).unwrap();
    let bought = adaptive(&report, &[("good", &["build"]), ("bad", &[])]);
    let summary = report.summary().robustness.as_ref().unwrap();
    assert_eq!(summary.objective_unit, "USD/share");
    // Good: 1090/90 - 10 = 19/9; bad abandonment: 990/100 - 10 = -1/10.
    let row = result(&report, &bought.policy_id);
    assert_eq!(row.nominal_mean, r(181, 180));
    assert_eq!(row.worst_case_mean, r(163, 360));
    assert_eq!(row.worst_case_expected_regret, r(3, 40));
    witness(&row.loss_certificate, [r(-19, 9), r(1, 10)]);
    witness(&row.regret_certificate, [ZERO, r(1, 10)]);
}

#[test]
fn stressed_tail_limit_uses_the_complete_paid_policy_and_retains_nominal_value() {
    let mut inputs = with_stress(study_example(), r(1, 4));
    let mut policy = inputs.program.policy.value.clone();
    policy.tail_mass = r(3, 4);
    policy.max_tail_loss = Some(usd(0));
    inputs.program.policy = policy_claim(policy);
    let report = evaluate(&inputs).unwrap();
    let bought = adaptive(&report, &[("good", &["build"]), ("bad", &[])]);
    let held = common(&report, &[]);
    let stress = report.summary().robustness.as_ref().unwrap();
    assert_eq!(
        report.summary().feasible_winners,
        std::slice::from_ref(&bought.policy_id)
    );
    assert_eq!(report.summary().net_adaptive_value.unwrap().major, r(40, 1));
    let tail = &result(&report, &bought.policy_id).tail_limits[0];
    // Paid policy values are 90 and -10, including the sunk study in both states.
    // Its worst 3/4 nominal mass is 1/2 at loss10 and 1/4 at loss-90 => -70/3.
    assert_eq!(tail.nominal_tail_loss, r(-70, 3));
    assert_eq!(tail.worst_case_tail_loss, r(10, 1));
    assert!(!tail.passes);
    let held_ids = std::slice::from_ref(&held.policy_id);
    assert_eq!(stress.robust_tail_feasible, held_ids);
    assert_eq!(stress.robust_tail_maximin_mean_winners, held_ids);
    assert_eq!(stress.robust_tail_minimax_expected_regret_winners, held_ids);
    assert_eq!(tail.certificate.duality_gap, ZERO);
    assert!(
        report
            .to_json()
            .contains("\"stressed_tail_loss_feasibility_certified\":true")
    );
    assert!(report.to_markdown().contains("Worst-tail submass"));
}

#[test]
fn combined_static_and_policy_work_is_bounded_before_claim_admission() {
    let zeros = vec![0; 64];
    let study = vec![-10; 64];
    let build = vec![100; 64];
    let weights = vec![r(1, 64); 64];
    let observations = (0..64)
        .map(|i| if i < 32 { "a" } else { "b" })
        .collect::<Vec<_>>();
    let program = inputs(
        &[
            ("study", 10, &study),
            ("build", 40, &build),
            ("a", 0, &zeros),
            ("b", 0, &zeros),
            ("c", 0, &zeros),
            ("d", 0, &zeros),
            ("e", 0, &zeros),
        ],
        &weights,
        50,
        r(1, 5),
        None,
    );
    let mut inputs = with_stress(staged(program, &observations), ZERO);
    edit_information(&mut inputs, |basis| {
        for row in &mut basis.candidate_stages {
            if row.candidate_id != "build" {
                row.stage = DecisionStage::Initial;
            }
        }
    });
    // 128 static + 192 policies: cash fits (61,440 rows), transport exceeds 2,097,152 work units.
    assert!(stress::check_work(&inputs.program).is_ok());
    let refusal = evaluate(&inputs).unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-201");
    assert!(refusal.what.contains("objective/state-pair"));
}

#[test]
fn adaptive_stress_retains_source_bindings_canonical_order_and_bootstrap_results() {
    let mut inputs = with_stress(study_example(), r(1, 4));
    let design = stability::SelectionStabilityDesign {
        model_key: inputs.program.policy.value.model_key.clone(),
        method: stability::METHOD.into(),
        seed: 17,
        replicates: 200,
        draws_per_replicate: 4,
        sampling_basis: "Conditional synthetic resamples, not independent observations".into(),
    };
    let tag = design.canonical_tag();
    inputs.program.selection_stability = Some(claim(
        "capital.stability",
        design,
        &tag,
        AdditivityClass::NonAdditive,
    ));
    let report = evaluate(&inputs).unwrap();
    let mut plain = inputs.clone();
    plain.program.robustness = None;
    let plain = evaluate(&plain).unwrap();
    assert_eq!(report.summary().policies, plain.summary().policies);
    assert_eq!(
        report.summary().selection_stability,
        plain.summary().selection_stability
    );
    assert!(
        report
            .claim()
            .parents
            .contains(&inputs.program.robustness.as_ref().unwrap().claim_id)
    );
    assert!(
        report
            .claim()
            .parents
            .contains(&inputs.information.claim_id)
    );
    let mut reordered = inputs.clone();
    reordered.program.weights.value.states.reverse();
    reordered.program.candidates.reverse();
    reordered.information.value.candidate_stages.reverse();
    reordered.information.value.observations.reverse();
    reordered
        .program
        .robustness
        .as_mut()
        .unwrap()
        .value
        .distances
        .reverse();
    assert_eq!(report.to_json(), evaluate(&reordered).unwrap().to_json());
    assert_eq!(
        report.to_markdown(),
        evaluate(&reordered).unwrap().to_markdown()
    );
    let mut superseded = inputs.clone();
    superseded.program.robustness.as_mut().unwrap().supersedes =
        Some(inputs.program.policy.claim_id);
    let updated = evaluate(&superseded).unwrap();
    assert_eq!(updated.summary(), report.summary());
    assert_ne!(updated.digest(), report.digest());
    let mut mutated = inputs;
    mutated.program.robustness.as_mut().unwrap().value.radius = ONE;
    assert_eq!(evaluate(&mutated).unwrap_err().code, "FMINE-REF-507");
}
