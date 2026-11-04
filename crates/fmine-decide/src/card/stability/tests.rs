//! Determinism: Exact

use super::*;
use fmine_core::anchor::SourceAnchor;
use fmine_evidence::KuKind;

fn r(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}

fn alternative(id: &str, values: [i128; 2], mask: u16) -> StabilityAlternative {
    StabilityAlternative {
        alternative_id: id.into(),
        structurally_feasible: true,
        values: values.into_iter().map(Rat::from_integer).collect(),
        tail_limits: vec![],
        selections: vec![mask],
    }
}

fn problem() -> SelectionStabilityProblem {
    SelectionStabilityProblem {
        design: SelectionStabilityDesign {
            model_key: "invented_stability".into(),
            method: METHOD.into(),
            seed: 229,
            replicates: 200,
            draws_per_replicate: 1,
            sampling_basis: "Synthetic finite joint law; declared experiment size, no calibration"
                .into(),
        },
        states: vec![("bad".into(), r(1, 2)), ("good".into(), r(1, 2))],
        objective_unit: "USD".into(),
        tail_mass: r(1, 2),
        primary_tail_limit: None,
        candidate_ids: vec!["project".into()],
        scopes: vec![SelectionScope {
            scope: "static".into(),
            state_id: None,
        }],
        alternatives: vec![alternative("a", [-1, 3], 1), alternative("b", [1, 0], 0)],
    }
}

fn claim(value: SelectionStabilityProblem) -> Claim<SelectionStabilityProblem> {
    let tag = value.canonical_tag();
    Claim::new(
        "decision.stability/1",
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Estimated,
        Confidentiality::internal(),
        vec![UuTerm {
            scenario_id: "omitted_regime".into(),
            description: "Invented omitted regime".into(),
            downside_exposure_usd_m: 1.0,
            trigger_indicator: "New regime evidence".into(),
        }],
        vec![],
        vec![SourceAnchor::new(
            "invented-stability.toml",
            [11; 32],
            "complete model",
        )],
    )
}

fn frequency(count: u32) -> Rat {
    r(i128::from(count), 200)
}

fn evaluate(inputs: &Claim<SelectionStabilityProblem>) -> Result<SelectionStabilitySummary> {
    super::evaluate(inputs).map(|claim| claim.value)
}

#[test]
fn dominant_negative_alternative_keeps_rejected_population_and_real_runner_up() {
    let mut p = problem();
    p.alternatives = vec![alternative("a", [-2, -2], 1), alternative("b", [-3, -3], 0)];
    p.alternatives.push(StabilityAlternative {
        alternative_id: "rejected".into(),
        structurally_feasible: false,
        values: vec![],
        tail_limits: vec![],
        selections: vec![1],
    });
    let out = evaluate(&claim(p)).unwrap();
    assert_eq!(out.nominal_winners, ["a"]);
    assert_eq!(out.nominal_runner_up, ["b"]);
    assert_eq!(out.alternatives.len(), 3);
    assert_eq!(out.alternatives[0].selection_frequency, ONE);
    assert!(!out.alternatives[0].fragile);
    assert_eq!(out.alternatives[2].risk_feasible_replicates, 0);
    assert_eq!(out.alternatives[2].selection_frequency, ZERO);
    assert_eq!(out.candidate_frequencies[0].frequency_lower, ONE);
    assert_eq!(out.candidate_frequencies[0].frequency_upper, ONE);
    assert_eq!(out.definite_flip_frequency, Some(ZERO));
    assert_eq!(out.possible_flip_frequency, Some(ZERO));
    assert!(
        out.replicates
            .iter()
            .all(|rep| rep.feasible_winners == ["a"] && rep.risk_feasible_alternatives == 2)
    );
}

#[test]
fn fragile_nominal_winner_and_runner_up_match_each_resampled_known_answer() {
    // DESIGN hand answers: nominal means a=1, b=1/2. One Bad draw chooses b; one Good chooses a.
    let p = problem();
    let out = evaluate(&claim(p.clone())).unwrap();
    assert_eq!(out, evaluate(&claim(p)).unwrap());
    assert_eq!(out.nominal_winners, ["a"]);
    assert_eq!(out.nominal_runner_up, ["b"]);
    let mut good = 0;
    for (index, rep) in out.replicates.iter().enumerate() {
        assert_eq!(rep.index, index);
        assert_eq!(rep.state_counts.iter().sum::<u32>(), 1);
        let winner = if rep.state_counts[0] == 1 {
            "b"
        } else {
            good += 1;
            "a"
        };
        assert_eq!(rep.feasible_winners, [winner]);
        assert_eq!(rep.risk_feasible_alternatives, 2);
    }
    assert!(good > 0 && good < 160);
    assert_eq!(out.alternatives[0].selection_frequency, frequency(good));
    assert!(out.alternatives[0].fragile);
    assert!(!out.alternatives[1].fragile);
    assert_eq!(out.alternatives[1].co_winner_replicates, 200 - good);
    assert_eq!(
        out.candidate_frequencies[0].frequency_lower,
        frequency(good)
    );
    assert_eq!(
        out.candidate_frequencies[0].frequency_upper,
        frequency(good)
    );
    assert_eq!(out.definite_flip_frequency, Some(frequency(200 - good)));
    assert_eq!(out.possible_flip_frequency, out.definite_flip_frequency);
}

#[test]
fn every_exact_tie_retains_candidate_bounds_and_unsampled_policy_continuations() {
    let mut p = problem();
    p.candidate_ids = vec!["a".into(), "b".into(), "study".into()];
    p.scopes = vec![
        SelectionScope {
            scope: "initial".into(),
            state_id: None,
        },
        SelectionScope {
            scope: "realized_state".into(),
            state_id: Some("bad".into()),
        },
        SelectionScope {
            scope: "realized_state".into(),
            state_id: Some("good".into()),
        },
    ];
    p.alternatives = vec![
        alternative("a", [5, 5], 0),
        alternative("b", [5, 5], 0),
        alternative("c", [5, 5], 0),
    ];
    for (i, row) in p.alternatives.iter_mut().enumerate() {
        row.selections = vec![4, 5, (i + 5) as u16];
    }
    let out = evaluate(&claim(p)).unwrap();
    assert_eq!(out.nominal_winners, ["a", "b", "c"]);
    assert!(out.nominal_runner_up.is_empty());
    assert!(
        out.replicates
            .iter()
            .all(|rep| rep.feasible_winners == ["a", "b", "c"])
    );
    assert!(
        out.alternatives
            .iter()
            .all(|row| row.selection_frequency == ONE && !row.fragile)
    );
    assert_eq!(out.candidate_frequencies[2].frequency_lower, ONE); // Study committed initially.
    assert_eq!(out.candidate_frequencies[3].frequency_lower, ONE); // Bad continuation even with zero Bad count.
    assert_eq!(out.candidate_frequencies[6].frequency_lower, ZERO);
    assert_eq!(out.candidate_frequencies[6].frequency_upper, ONE);
    assert_eq!(out.candidate_frequencies[7].frequency_lower, ZERO);
    assert_eq!(out.candidate_frequencies[7].frequency_upper, ONE);
    assert_eq!(out.candidate_frequencies[8].frequency_lower, ONE);
    assert_eq!(out.definite_flip_frequency, Some(ZERO));
    assert_eq!(out.possible_flip_frequency, Some(ZERO));
}

#[test]
fn primary_risk_is_recomputed_and_no_feasible_draws_stay_in_the_denominator() {
    let mut p = problem();
    p.alternatives = vec![alternative("a", [-4, 6], 1)];
    p.primary_tail_limit = Some(Rat::from_integer(3));
    let out = evaluate(&claim(p)).unwrap();
    assert!(out.nominal_winners.is_empty()); // Nominal worst-half loss4 exceeds3.
    assert_eq!(out.definite_flip_frequency, None);
    assert_eq!(out.possible_flip_frequency, None);
    let good = out
        .replicates
        .iter()
        .map(|rep| rep.state_counts[1])
        .sum::<u32>();
    assert!(good > 0 && good < 200);
    for rep in &out.replicates {
        assert_eq!(
            rep.feasible_winners,
            if rep.state_counts[1] == 1 {
                vec!["a"]
            } else {
                vec![]
            }
        );
    }
    assert_eq!(out.no_feasible_replicates, 200 - good);
    assert_eq!(out.alternatives[0].risk_feasible_replicates, good);
    assert_eq!(out.alternatives[0].selection_frequency, frequency(good));
    assert_eq!(
        out.candidate_frequencies[0].frequency_lower,
        frequency(good)
    );
    assert_eq!(
        out.candidate_frequencies[0].frequency_upper,
        frequency(good)
    );
}

#[test]
fn secondary_tail_laws_bind_jointly_without_changing_the_primary_objective() {
    let mut p = problem();
    p.alternatives[1].values = vec![ZERO, ZERO];
    p.primary_tail_limit = Some(ONE); // Equality at nominal loss1 is admitted.
    p.alternatives[0].tail_limits.push(StabilityTailLimit {
        name: "carve_out".into(),
        values: vec![Rat::from_integer(-3), ONE],
        limit: Rat::from_integer(2),
    });
    let first = evaluate(&claim(p.clone())).unwrap();
    assert_eq!(first.nominal_winners, ["b"]);
    for rep in &first.replicates {
        assert_eq!(
            rep.feasible_winners,
            [if rep.state_counts[1] == 1 { "a" } else { "b" }]
        );
    }
    p.alternatives[0].tail_limits.push(StabilityTailLimit {
        name: "second_entity".into(),
        values: vec![Rat::from_integer(2), Rat::from_integer(-4)],
        limit: Rat::from_integer(3),
    });
    let second = evaluate(&claim(p)).unwrap();
    assert_eq!(second.alternatives[0].risk_feasible_replicates, 0);
    assert!(
        second
            .replicates
            .iter()
            .all(|rep| rep.feasible_winners == ["b"])
    );
    assert_eq!(
        first
            .replicates
            .iter()
            .map(|rep| &rep.state_counts)
            .collect::<Vec<_>>(),
        second
            .replicates
            .iter()
            .map(|rep| &rep.state_counts)
            .collect::<Vec<_>>()
    );
}

#[test]
fn source_bound_claim_rejects_mutated_values_sources_stale_and_unbudgeted_inputs() {
    let original = claim(problem());
    let mut changed = original.clone();
    changed.value.design.seed += 1;
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-507");
    changed = original.clone();
    changed.anchors[0].artifact_sha256[0] ^= 1;
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-507");
    changed = original.clone();
    changed.stale = true;
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-502");
    changed = original.clone();
    changed.uu_terms.clear();
    assert!(evaluate(&changed).is_err());
    changed = original.clone();
    changed.value.alternatives[0].values[0] = Rat::from_integer(-2);
    let rebound = claim(changed.value.clone());
    assert_ne!(original.claim_id, rebound.claim_id);
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-507");
    assert!(evaluate(&rebound).is_ok());
}

#[test]
fn admission_work_and_arithmetic_refuse_without_partial_frequencies() {
    let mut cases = Vec::new();
    let mut p = problem();
    p.states.reverse();
    cases.push(p);
    let mut p = problem();
    p.states[0].1 = ZERO;
    cases.push(p);
    let mut p = problem();
    p.alternatives[1].alternative_id = "a".into();
    cases.push(p);
    let mut p = problem();
    p.alternatives[0].values.pop();
    cases.push(p);
    let mut p = problem();
    p.alternatives[0].structurally_feasible = false;
    cases.push(p);
    let mut p = problem();
    p.alternatives[0].selections[0] = 2;
    cases.push(p);
    let mut p = problem();
    p.scopes[0].state_id = Some("bad".into());
    cases.push(p);
    let mut p = problem();
    p.objective_unit = "blended_entities".into();
    cases.push(p);
    let mut p = problem();
    p.tail_mass = ZERO;
    cases.push(p);
    let mut p = problem();
    p.design.method = "invented".into();
    cases.push(p);
    for p in cases {
        assert!(evaluate(&claim(p)).is_err());
    }
    let mut design = problem().design;
    design.replicates = 2_000;
    assert!(design.check_work(256, 62, 1).is_ok());
    assert_eq!(
        design.check_work(256, 63, 1).unwrap_err().code,
        "FMINE-REF-201"
    );
    assert!(design.check_work(2, 2, 5).is_err());
    assert!(design.check_work(2, usize::MAX, usize::MAX).is_err());
    let mut p = problem();
    p.alternatives[0].values = vec![r(i128::MAX, 2), r(i128::MAX - 2, 3)];
    assert_eq!(evaluate(&claim(p)).unwrap_err().code, "FMINE-REF-207");
}

#[test]
fn positive_affine_payoff_and_risk_transform_preserves_selection_and_exact_ties() {
    let mut p = problem();
    p.design.draws_per_replicate = 3;
    p.primary_tail_limit = Some(Rat::from_integer(2));
    p.alternatives.push(alternative("c", [-1, 3], 1));
    let original = evaluate(&claim(p.clone())).unwrap();
    for row in &mut p.alternatives {
        for value in &mut row.values {
            *value = value
                .checked_mul(Rat::from_integer(3))
                .unwrap()
                .checked_add(Rat::from_integer(7))
                .unwrap();
        }
    }
    p.primary_tail_limit = Some(Rat::from_integer(-1)); // loss transforms to3*loss-7.
    p.objective_unit = "USD/share".into();
    let transformed = evaluate(&claim(p)).unwrap();
    assert_eq!(original.nominal_winners, transformed.nominal_winners);
    assert_eq!(original.nominal_runner_up, transformed.nominal_runner_up);
    assert_eq!(original.replicates, transformed.replicates);
    assert_eq!(original.alternatives, transformed.alternatives);
    assert_eq!(
        original.candidate_frequencies,
        transformed.candidate_frequencies
    );
    assert_eq!(
        original.definite_flip_frequency,
        transformed.definite_flip_frequency
    );
    assert_eq!(
        original.possible_flip_frequency,
        transformed.possible_flip_frequency
    );
}

#[test]
fn retained_tie_identifiers_and_named_laws_are_bounded_before_sampling() {
    let mut p = problem();
    p.design.replicates = 2_000;
    p.alternatives = (0..512)
        .map(|i| alternative(&format!("{i:03}{}", "a".repeat(125)), [1, 1], 1))
        .collect();
    // The arithmetic work is small, but the unbounded tied winner copies would exceed 128 MB.
    assert!(p.design.check_work(2, 512, 1).is_ok());
    assert_eq!(evaluate(&claim(p)).unwrap_err().code, "FMINE-REF-201");
    let mut p = problem();
    p.alternatives[0].alternative_id = "a".repeat(129);
    assert!(evaluate(&claim(p)).is_err());
    let states = problem().states;
    assert_eq!(
        ordered_values(&states, [("good", ONE), ("bad", ZERO)].into_iter()).unwrap(),
        [ZERO, ONE]
    );
    for values in [
        vec![("bad", ZERO)],
        vec![("bad", ZERO), ("bad", ONE)],
        vec![("bad", ZERO), ("other", ONE)],
        vec![("bad", ZERO), ("good", ONE), ("extra", ONE)],
    ] {
        assert!(ordered_values(&states, values.into_iter()).is_err());
    }
    assert!(ordered_values(&[], std::iter::empty()).is_err());
    assert!(
        ordered_values(
            &[("a".into(), r(1, 2)), ("a".into(), r(1, 2))],
            [("a", ONE), ("b", Rat::from_integer(2))].into_iter()
        )
        .is_err()
    );
}

#[test]
fn result_claim_keeps_source_parent_uncertainty_and_evidence_without_upgrading() {
    let input = claim(problem());
    let result = super::evaluate(&input).unwrap();
    result.check_id().unwrap();
    assert_eq!(result.parents, [input.claim_id]);
    assert_eq!(result.anchors, input.anchors);
    assert_eq!(result.evidence, EvidenceClass::Assumed);
    assert_eq!(result.uu_terms, input.uu_terms);
    assert_eq!(result.state, input.state);
    assert_eq!(result.confidentiality, input.confidentiality);
    let mut changed = input.clone();
    changed.uu_terms[0].downside_exposure_usd_m = 2.0;
    assert_eq!(changed.claim_id, input.claim_id);
    let revised = super::evaluate(&changed).unwrap();
    assert_eq!(revised.value, result.value);
    assert_ne!(revised.claim_id, result.claim_id);
}
