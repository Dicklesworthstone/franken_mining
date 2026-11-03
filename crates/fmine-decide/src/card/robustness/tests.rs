//! Source admission, exact decision meaning and unchanged nominal comparisons.
//! Determinism: Exact

use super::*;
use fmine_core::anchor::SourceAnchor;

fn r(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}

fn configuration(value: RobustnessDesign, inputs: &DecisionInputs) -> Claim<RobustnessDesign> {
    let source = &inputs.weights;
    let tag = value.canonical_tag();
    Claim::new(
        "decision.wasserstein/1",
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        source.epistemic,
        source.state,
        source.confidentiality.clone(),
        source.uu_terms.clone(),
        vec![],
        source.anchors.clone(),
    )
}

fn fixture(radius: Rat) -> DecisionInputs {
    let text = include_str!("../../../../../tests/decisions/expansion.toml");
    let mut input = super::super::model::parse(
        text,
        SourceAnchor::new(
            "synthetic.toml",
            sha256(text.as_bytes()).0,
            "synthetic decision",
        ),
    )
    .unwrap();
    let mut distances = Vec::new();
    for (from, _) in &input.weights.value.states {
        for (to, _) in &input.weights.value.states {
            distances.push(StateDistance {
                from_state: from.clone(),
                to_state: to.clone(),
                distance: if from == to { ZERO } else { ONE },
            });
        }
    }
    input.robustness = Some(configuration(
        RobustnessDesign {
            model_key: input.weights.value.model_key.clone(),
            method: METHOD.into(),
            radius,
            metric_basis: "Synthetic unit distance between the two declared regimes".into(),
            radius_basis: "Declared sensitivity budget, no empirical calibration".into(),
            distances,
        },
        &input,
    ));
    input
}

fn replace_design(input: &mut DecisionInputs, change: impl FnOnce(&mut RobustnessDesign)) {
    let mut design = input.robustness.as_ref().unwrap().value.clone();
    change(&mut design);
    input.robustness = Some(configuration(design, input));
}

#[test]
fn robust_mean_and_expected_regret_are_distinct_questions_with_exact_witnesses() {
    let input = fixture(r(1, 4));
    let card = evaluate(&input).unwrap();
    let robust = card.summary().robustness.as_ref().unwrap();
    assert_eq!(card.summary().expected_value_winners, ["build"]);
    assert_eq!(robust.maximin_mean_winners, ["smaller"]);
    assert_eq!(robust.minimax_expected_regret_winners, ["build"]);
    let build = robust
        .alternatives
        .iter()
        .find(|a| a.action_id == "build")
        .unwrap();
    let smaller = robust
        .alternatives
        .iter()
        .find(|a| a.action_id == "smaller")
        .unwrap();
    assert_eq!(build.nominal_mean, r(100_000_000, 1));
    assert_eq!(build.worst_case_mean, r(-25_000_000, 1));
    assert_eq!(build.worst_case_expected_regret, r(165_750_000, 1));
    assert_eq!(smaller.worst_case_mean, r(55_000_000, 1));
    assert_eq!(smaller.worst_case_expected_regret, r(208_250_000, 1));
    assert_eq!(
        build.loss_certificate.destination_weights,
        [r(7, 20), r(13, 20)]
    );
    assert!(!robust.nominal_winner_survives_maximin);
    assert!(robust.nominal_winner_survives_minimax_regret);
    for a in &robust.alternatives {
        for c in [&a.loss_certificate, &a.regret_certificate] {
            assert_eq!(c.duality_gap, ZERO);
            assert_eq!(c.dual_value, c.worst_case_value);
            assert!(!compare(c.transport_cost, robust.design.radius).is_gt());
            assert!(c.arcs.len() <= robust.state_ids.len() + 1);
        }
    }
    assert!(card.to_markdown().contains("Wasserstein robustness"));
    assert!(card.to_json().contains("\"scope\":\"act_now_actions\""));
}

#[test]
fn zero_radius_is_nominal_and_saturation_matches_same_state_minimax() {
    let zero = evaluate(&fixture(ZERO)).unwrap();
    let z = zero.summary().robustness.as_ref().unwrap();
    assert_eq!(
        z.maximin_mean_winners,
        zero.summary().expected_value_winners
    );
    assert_eq!(
        z.minimax_expected_regret_winners,
        zero.summary().expected_value_winners
    );
    for a in &z.alternatives {
        assert_eq!(a.nominal_mean, a.worst_case_mean);
        assert_eq!(a.nominal_mean_regret, a.worst_case_expected_regret);
        assert_eq!(a.loss_certificate.destination_weights, z.nominal_weights);
    }
    let saturated = evaluate(&fixture(r(100, 1))).unwrap();
    let s = saturated.summary().robustness.as_ref().unwrap();
    assert_eq!(
        s.minimax_expected_regret_winners,
        saturated.summary().minimax_regret_winners
    );
    for a in &s.alternatives {
        let nominal = saturated
            .summary()
            .alternatives
            .iter()
            .find(|v| v.action_id == a.action_id)
            .unwrap();
        assert_eq!(a.worst_case_expected_regret, nominal.max_regret.major);
        assert_eq!(a.loss_certificate.dual_lambda, ZERO);
        assert_eq!(a.regret_certificate.dual_lambda, ZERO);
    }
    assert_eq!(
        zero.summary().alternatives,
        saturated.summary().alternatives
    );
}

#[test]
fn configuration_source_evidence_and_all_input_identity_bind_the_result() {
    let input = fixture(r(1, 4));
    let card = evaluate(&input).unwrap();
    let source = input.robustness.as_ref().unwrap();
    assert!(card.claim().parents.contains(&source.claim_id));
    assert_eq!(card.claim().evidence, EvidenceClass::Assumed);
    assert_eq!(card.claim().confidentiality, source.confidentiality);
    assert_eq!(card.claim().uu_terms, source.uu_terms);
    let mut changed = input.clone();
    replace_design(&mut changed, |d| {
        d.radius_basis
            .push_str("; independent sensitivity declaration")
    });
    let rebound = evaluate(&changed).unwrap();
    assert_ne!(card.claim().claim_id, rebound.claim().claim_id);
    assert_ne!(card.digest(), rebound.digest());
    assert_eq!(
        card.summary().robustness.as_ref().unwrap().alternatives,
        rebound.summary().robustness.as_ref().unwrap().alternatives
    );
    let original = changed.robustness.as_ref().unwrap();
    changed.robustness = Some(Claim::new(
        &original.metric_key,
        original.value.clone(),
        original.value_tag(),
        original.representation.clone(),
        original.support,
        original.additivity,
        original.evidence,
        original.epistemic,
        original.state,
        original.confidentiality.clone(),
        original.uu_terms.clone(),
        original.parents.clone(),
        vec![SourceAnchor::new(
            "changed-source.toml",
            [17; 32],
            "declared geometry",
        )],
    ));
    let new_source = evaluate(&changed).unwrap();
    assert_ne!(new_source.digest(), rebound.digest());
    assert_eq!(new_source.summary(), rebound.summary());
}

#[test]
fn edited_stale_or_unknown_free_configuration_never_reaches_the_solver() {
    let original = fixture(r(1, 4));
    let mut edited = original.clone();
    edited.robustness.as_mut().unwrap().value.radius = r(1, 3);
    assert_eq!(evaluate(&edited).unwrap_err().code, "FMINE-REF-507");
    let mut stale = original.clone();
    stale.robustness.as_mut().unwrap().stale = true;
    assert_eq!(evaluate(&stale).unwrap_err().code, "FMINE-REF-502");
    let mut unknown_free = original.clone();
    unknown_free.robustness.as_mut().unwrap().uu_terms.clear();
    assert_eq!(evaluate(&unknown_free).unwrap_err().code, "FMINE-REF-204");
    let mut wrong_model = original;
    replace_design(&mut wrong_model, |d| d.model_key = "another-model".into());
    assert_eq!(evaluate(&wrong_model).unwrap_err().code, "FMINE-REF-105");
}

#[test]
fn validly_retagged_bad_metrics_and_oversized_support_refuse() {
    for change in [0, 1, 2, 3] {
        let mut input = fixture(r(1, 4));
        replace_design(&mut input, |d| match change {
            0 => d.distances[1] = d.distances[0].clone(),
            1 => d.distances[1].distance = ZERO,
            2 => d.distances[1].distance = r(2, 1),
            _ => d.distances[0].from_state = "missing".into(),
        });
        assert!(evaluate(&input).is_err());
    }
    let mut oversized = fixture(r(1, 4));
    oversized.weights.value.states = (0..65)
        .map(|i| (format!("state{i:02}"), r(1, 65)))
        .collect();
    assert_eq!(evaluate(&oversized).unwrap_err().code, "FMINE-REF-201");
    let mut missing = fixture(r(1, 4));
    replace_design(&mut missing, |d| {
        d.distances.pop();
    });
    assert_eq!(evaluate(&missing).unwrap_err().code, "FMINE-REF-201");
}

#[test]
fn reordering_sources_preserves_the_computed_card_and_nominal_comparison() {
    let input = fixture(r(1, 4));
    let card = evaluate(&input).unwrap();
    let mut permuted = input.clone();
    permuted.actions.reverse();
    permuted.weights.value.states.reverse();
    permuted
        .robustness
        .as_mut()
        .unwrap()
        .value
        .distances
        .reverse();
    for action in &mut permuted.actions {
        action.value.values.reverse();
    }
    assert_eq!(evaluate(&permuted).unwrap(), card);
    let mut nominal = input;
    nominal.robustness = None;
    let without = evaluate(&nominal).unwrap();
    assert_eq!(without.summary().alternatives, card.summary().alternatives);
    assert_eq!(
        without.summary().expected_value_winners,
        card.summary().expected_value_winners
    );
    assert!(!without.to_json().contains("\"robustness\":"));
    assert!(!without.to_markdown().contains("Wasserstein robustness"));
}
