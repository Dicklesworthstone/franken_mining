//! Realized-outcome event probabilities, inclusive limits and source-bound complete populations.
//! Determinism: Exact

use super::*;
use fmine_core::anchor::SourceAnchor;

const MODEL: &str = include_str!("../../../../../tests/decisions/wasserstein_information.toml");

fn r(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}

fn parsed(text: &str) -> Result<DecisionInputs> {
    model::parse(
        text,
        SourceAnchor::new(
            "synthetic-downside.toml",
            sha256(text.as_bytes()).0,
            "declared risk assumptions",
        ),
    )
}

fn fixture(floor: &str, maximum: &str) -> DecisionInputs {
    parsed(&format!(
        "{MODEL}\n[[downside_limit]]\nfloor = \"{floor}\"\nmax_breach_probability = \"{maximum}\"\n"
    ))
    .unwrap()
}

fn downside(card: &EvaluatedCard) -> &DownsideSummary {
    card.summary().downside.as_ref().unwrap()
}

fn choice<'a>(card: &'a EvaluatedCard, id: &str) -> &'a DownsideChoice {
    downside(card)
        .choices
        .iter()
        .find(|c| c.choice.id == id)
        .unwrap()
}

fn retag_limit(input: &mut DecisionInputs, change: impl FnOnce(&mut DownsideLimit)) {
    let old = input.downside_limit.as_ref().unwrap();
    let mut value = old.value.clone();
    change(&mut value);
    let tag = value.canonical_tag();
    input.downside_limit = Some(Claim::new(
        &old.metric_key,
        value,
        &tag,
        old.representation.clone(),
        old.support,
        old.additivity,
        old.evidence,
        old.epistemic,
        old.state,
        old.confidentiality.clone(),
        old.uu_terms.clone(),
        old.parents.clone(),
        old.anchors.clone(),
    ));
}

#[test]
fn realized_signal_breaches_drive_nominal_and_stressed_probability_screens() {
    let input = fixture("0", "3/20");
    let card = evaluate(&input).unwrap();
    let result = downside(&card);
    let probe = choice(&card, "probe");
    // Only low/good loses money: P(good|low)=1/4, rather than indicator(E[net|low]<0)=1.
    assert_eq!(probe.conditional_breach_probabilities, [ZERO, r(1, 4)]);
    assert_eq!(probe.nominal_breach_probability, r(1, 8));
    assert_eq!(probe.worst_case_breach_probability, Some(r(3, 16)));
    assert!(probe.nominal_eligible);
    assert_eq!(probe.robust_eligible, Some(false));
    let certificate = probe.breach_certificate.as_ref().unwrap();
    assert_eq!(certificate.destination_weights, [r(1, 4), r(3, 4)]);
    assert_eq!(certificate.transport_cost, r(1, 4));
    assert_eq!(certificate.dual_lambda, r(1, 4));
    assert_eq!(certificate.dual_value, r(3, 16));
    assert_eq!(certificate.duality_gap, ZERO);
    assert_eq!(choice(&card, "build").nominal_breach_probability, r(1, 2));
    assert_eq!(
        choice(&card, "build").worst_case_breach_probability,
        Some(r(3, 4))
    );
    assert_eq!(
        choice(&card, "safe").worst_case_breach_probability,
        Some(ZERO)
    );
    assert_eq!(
        choice(&card, "hold").worst_case_breach_probability,
        Some(ZERO)
    );
    assert_eq!(result.choices.len(), 4);
    assert_eq!(result.nominal.winners[0].id, "probe");
    assert_eq!(result.nominal.best_expected_value, Some(r(67_500_000, 1)));
    let robust = result.robust.as_ref().unwrap();
    assert_eq!(robust.winners[0].id, "safe");
    assert_eq!(robust.expected_value_cost_of_limit, Some(r(37_500_000, 1)));
    let limit = input.downside_limit.as_ref().unwrap();
    assert!(card.claim().parents.contains(&limit.claim_id));
    assert!(
        limit
            .anchors
            .iter()
            .all(|a| card.claim().anchors.contains(a))
    );
    assert_eq!(limit.evidence, EvidenceClass::Assumed);
    assert_eq!(limit.confidentiality, Confidentiality::internal());
    let json = card.to_json();
    assert!(json.contains("\"certificate_objective_unit\":\"probability\""));
    assert!(json.contains("\"downside_limit\":"));
    let markdown = card.to_markdown();
    assert!(markdown.contains("Downside probability limit"));
    assert!(markdown.contains(
        "| information_option | probe | 67500000/1 | 1/8 | eligible | 3/16 | rejected |"
    ));
    assert!(markdown.contains("probability per metric unit"));
}

#[test]
fn exact_limit_equality_passes_and_stressed_eligibility_keeps_nominal_value_objective() {
    let card = evaluate(&fixture("0", "3/16")).unwrap();
    let probe = choice(&card, "probe");
    assert_eq!(probe.robust_eligible, Some(true));
    let ranking = downside(&card).robust.as_ref().unwrap();
    assert_eq!(ranking.winners[0].id, "probe");
    assert_eq!(ranking.best_expected_value, Some(r(67_500_000, 1)));
    assert_eq!(ranking.expected_value_cost_of_limit, Some(ZERO));
    // The separate maximin-value view still picks safe. A probability screen does not change objectives.
    assert_eq!(
        card.summary()
            .robustness
            .as_ref()
            .unwrap()
            .fixed_policy_comparison
            .as_ref()
            .unwrap()
            .maximin_mean_winners[0]
            .id,
        "safe"
    );
}

#[test]
fn strict_monetary_floor_and_exact_once_costs_distinguish_neighboring_events() {
    let at = evaluate(&fixture("20000000", "1/1")).unwrap();
    let above = evaluate(&fixture("20000000.0001", "1/1")).unwrap();
    assert_eq!(choice(&at, "probe").nominal_breach_probability, r(1, 8));
    assert_eq!(
        choice(&above, "probe").conditional_breach_probabilities,
        [r(1, 4), ONE]
    );
    assert_eq!(choice(&above, "probe").nominal_breach_probability, r(5, 8));
    assert_eq!(
        choice(&above, "probe").worst_case_breach_probability,
        Some(r(13, 16))
    );
    assert_eq!(choice(&at, "hold").nominal_breach_probability, ONE);
    let negative = evaluate(&fixture("-110000000", "0/1")).unwrap();
    assert!(
        downside(&negative)
            .choices
            .iter()
            .all(|c| c.nominal_breach_probability == ZERO)
    );
    assert!(
        downside(&negative)
            .choices
            .iter()
            .all(|c| c.robust_eligible == Some(true))
    );
}

#[test]
fn empty_eligible_sets_are_complete_results_without_a_fallback_or_value_cost() {
    let card = evaluate(&fixture("30000000.0001", "0/1")).unwrap();
    let result = downside(&card);
    assert_eq!(result.choices.len(), 4);
    assert!(
        result
            .choices
            .iter()
            .all(|c| !c.nominal_eligible && c.robust_eligible == Some(false))
    );
    for ranking in [&result.nominal, result.robust.as_ref().unwrap()] {
        assert_eq!(ranking.status, DownsideStatus::NoEligibleChoice);
        assert!(ranking.eligible_order.is_empty());
        assert!(ranking.winners.is_empty());
        assert!(ranking.best_expected_value.is_none());
        assert!(ranking.expected_value_cost_of_limit.is_none());
    }
    assert!(card.to_json().contains("\"status\":\"no_eligible_choice\""));
    assert!(card.to_markdown().contains("**No eligible choice.**"));
    assert!(card.to_markdown().contains("no fallback is selected"));
}

#[test]
fn absent_robustness_uses_nominal_probabilities_and_omitted_limit_preserves_existing_card() {
    let input = fixture("0", "3/20");
    let card = evaluate(&input).unwrap();
    let mut nominal = input.clone();
    nominal.robustness = None;
    let nominal = evaluate(&nominal).unwrap();
    assert!(downside(&nominal).robust.is_none());
    assert!(
        downside(&nominal)
            .choices
            .iter()
            .all(|c| c.worst_case_breach_probability.is_none()
                && c.robust_eligible.is_none()
                && c.breach_certificate.is_none())
    );
    assert_eq!(downside(&nominal).nominal, downside(&card).nominal);
    assert_eq!(nominal.summary().information, card.summary().information);
    let mut without = input;
    without.downside_limit = None;
    let old = evaluate(&without).unwrap();
    assert!(old.summary().downside.is_none());
    assert!(!old.to_json().contains("\"downside\":"));
    assert!(!old.to_json().contains("\"downside_limit\":"));
    assert!(!old.to_markdown().contains("Downside probability limit"));
    assert_eq!(old.summary().information, card.summary().information);
    assert_eq!(old.summary().alternatives, card.summary().alternatives);
    assert_eq!(old.summary().robustness, card.summary().robustness);
}

#[test]
fn probability_radius_zero_and_saturation_match_nominal_and_worst_conditional_events() {
    for (radius, expected) in [(ZERO, r(1, 8)), (ONE, r(1, 4))] {
        let text = MODEL.replace(
            "radius = \"1/4\"",
            &format!("radius = \"{}/{}\"", radius.num(), radius.den()),
        );
        let input = parsed(&format!(
            "{text}\n[[downside_limit]]\nfloor = \"0\"\nmax_breach_probability = \"1/1\"\n"
        ))
        .unwrap();
        let card = evaluate(&input).unwrap();
        assert_eq!(
            choice(&card, "probe").worst_case_breach_probability,
            Some(expected)
        );
        assert!(
            downside(&card).choices.iter().all(|c| c
                .breach_certificate
                .as_ref()
                .unwrap()
                .duality_gap
                == ZERO)
        );
    }
}

#[test]
fn malformed_limits_and_edited_evidence_refuse_without_partial_screening() {
    for row in [
        "floor = \"0\"",
        "max_breach_probability = \"1/2\"",
        "floor = \"0\"\nmax_breach_probability = \"1.0\"",
        "floor = \"0\"\nmax_breach_probability = \"-1/2\"",
        "floor = \"0\"\nmax_breach_probability = \"2/1\"",
        "floor = \"0\"\nmax_breach_probability = \"1/0\"",
        "floor = \"0.00001\"\nmax_breach_probability = \"1/2\"",
        "floor = \"0\"\nmax_breach_probability = \"1/2\"\nobjective = \"robust_mean\"",
    ] {
        assert!(parsed(&format!("{MODEL}\n[[downside_limit]]\n{row}\n")).is_err());
    }
    let text =
        format!("{MODEL}\n[[downside_limit]]\nfloor = \"0\"\nmax_breach_probability = \"1/2\"\n");
    assert!(
        parsed(&format!(
            "{text}\n[[downside_limit]]\nfloor = \"0\"\nmax_breach_probability = \"1/2\"\n"
        ))
        .is_err()
    );
    let mut edited = fixture("0", "3/20");
    edited.downside_limit.as_mut().unwrap().value.floor.minor += 1;
    assert_eq!(evaluate(&edited).unwrap_err().code, "FMINE-REF-507");
    let mut stale = fixture("0", "3/20");
    stale.downside_limit.as_mut().unwrap().stale = true;
    assert_eq!(evaluate(&stale).unwrap_err().code, "FMINE-REF-502");
    let mut wrong = fixture("0", "3/20");
    retag_limit(&mut wrong, |l| l.floor.ccy = Currency::CAD);
    assert_eq!(evaluate(&wrong).unwrap_err().code, "FMINE-REF-105");
    retag_limit(&mut wrong, |l| {
        l.floor.ccy = Currency::USD;
        l.max_breach_probability = r(2, 1);
    });
    assert_eq!(evaluate(&wrong).unwrap_err().code, "FMINE-REF-109");
}

#[test]
fn each_downside_probability_solve_counts_before_hashing_or_transport() {
    let mut input = fixture("0", "3/20");
    input.weights.value.states = (0..64).map(|i| (format!("state{i}"), r(1, 64))).collect();
    input.robustness.as_mut().unwrap().value.distances = input
        .weights
        .value
        .states
        .iter()
        .flat_map(|(from, _)| {
            input
                .weights
                .value
                .states
                .iter()
                .map(move |(to, _)| robustness::StateDistance {
                    from_state: from.clone(),
                    to_state: to.clone(),
                    distance: if from == to { ZERO } else { ONE },
                })
        })
        .collect();
    input.information_options[0].value.likelihoods = input
        .weights
        .value
        .states
        .iter()
        .flat_map(|(state, _)| {
            ["bad", "good"].map(|signal| Likelihood {
                state_id: state.clone(),
                signal_id: signal.into(),
                probability: r(1, 2),
            })
        })
        .collect();
    input.actions.resize(128, input.actions[0].clone());
    input
        .information_options
        .resize(64, input.information_options[0].clone());
    assert!(robustness::check_work(&input).is_ok());
    input.actions.push(input.actions[0].clone());
    let refusal = evaluate(&input).unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-201");
    assert!(refusal.what.contains("plus one per choice"));
}
