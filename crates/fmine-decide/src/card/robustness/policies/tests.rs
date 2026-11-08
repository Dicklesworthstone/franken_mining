//! Independent state/signal expectations, adverse laws and fixed-policy boundaries.

use super::*;
use fmine_core::anchor::SourceAnchor;

fn r(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}

fn fixture(radius: Rat, accuracy: Rat, test: i128, delay: i128, id: &str) -> DecisionInputs {
    let mut text = String::from(
        "schema = \"fmine.decision-model.v1\"\nmodel_key = \"fixed-policy\"\nquestion = \"Act now or buy an invented test?\"\ncurrency = \"USD\"\nvaluation_basis = \"Invented already-discounted incremental USD\"\n",
    );
    for state in ["high", "low"] {
        text.push_str(&format!(
            "\n[[state]]\nid = \"{state}\"\nweight = \"1/2\"\n"
        ));
    }
    for (action, high, low, role) in [
        ("build", 200, -100, "action"),
        ("safe", 30, 30, "action"),
        ("hold", 0, 0, "do_nothing"),
    ] {
        text.push_str(&format!(
            "\n[[action]]\nid = \"{action}\"\nlabel = \"{action}\"\nrole = \"{role}\"\n"
        ));
        for (state, payoff) in [("high", high), ("low", low)] {
            text.push_str(&format!(
                "\n[[payoff]]\naction = \"{action}\"\nstate = \"{state}\"\nvalue = \"{payoff}\"\n"
            ));
        }
    }
    text.push_str("\n[[uu]]\nid = \"UU-test\"\ndescription = \"An omitted regime can change value\"\ntrigger = \"Regime changes\"\ndownside_usd_m = \"1.0\"\n");
    text.push_str(&format!("\n[[information_option]]\nid = \"{id}\"\nlabel = \"Invented test\"\ntest_cost = \"{test}\"\ndelay_cost = \"{delay}\"\ndelay_days = 17\n"));
    for signal in ["bad", "good", "impossible"] {
        text.push_str(&format!(
            "\n[[signal]]\noption = \"{id}\"\nid = \"{signal}\"\n"
        ));
        for state in ["high", "low"] {
            let probability = match (state, signal) {
                (_, "impossible") => ZERO,
                ("high", "good") | ("low", "bad") => accuracy,
                _ => ONE.checked_sub(accuracy).unwrap(),
            };
            text.push_str(&format!("\n[[likelihood]]\noption = \"{id}\"\nstate = \"{state}\"\nsignal = \"{signal}\"\nprobability = \"{}/{}\"\n", probability.num(), probability.den()));
        }
    }
    text.push_str(&format!("\n[[robustness]]\nmethod = \"wasserstein_1\"\nradius = \"{}/{}\"\nmetric_basis = \"Unit distance between invented regimes\"\nradius_basis = \"Declared sensitivity; no calibration\"\n", radius.num(), radius.den()));
    for from in ["high", "low"] {
        for to in ["high", "low"] {
            text.push_str(&format!(
                "\n[[transport_distance]]\nfrom = \"{from}\"\nto = \"{to}\"\nvalue = \"{}/1\"\n",
                u8::from(from != to)
            ));
        }
    }
    model::parse(
        &text,
        SourceAnchor::new(
            "synthetic-policy.toml",
            sha256(text.as_bytes()).0,
            "synthetic assumptions",
        ),
    )
    .unwrap()
}

fn comparison(card: &EvaluatedCard) -> &FixedPolicyComparison {
    card.summary()
        .robustness
        .as_ref()
        .unwrap()
        .fixed_policy_comparison
        .as_ref()
        .unwrap()
}

#[test]
fn informative_test_loses_to_safe_action_under_declared_state_weight_stress() {
    let input = fixture(r(1, 4), r(3, 4), 6, 4, "probe");
    let card = evaluate(&input).unwrap();
    let result = comparison(&card);
    let policy = &result.policies[0];
    assert_eq!(policy.state_net_payoffs, [r(295, 2), r(-25, 2)]);
    assert_eq!(policy.nominal_mean, r(135, 2));
    assert_eq!(policy.worst_case_mean, r(55, 2));
    assert_eq!(
        policy.loss_certificate.destination_weights,
        [r(1, 4), r(3, 4)]
    );
    assert_eq!(policy.loss_certificate.transport_cost, r(1, 4));
    assert_eq!(policy.loss_certificate.dual_lambda, r(160, 1));
    assert_eq!(policy.loss_certificate.duality_gap, ZERO);
    assert_eq!(policy.loss_certificate.dual_value, r(-55, 2));
    assert_eq!(
        result.nominal_expected_value_winners[0].kind,
        ChoiceKind::InformationOption
    );
    assert_eq!(result.nominal_expected_value_winners[0].id, "probe");
    assert_eq!(result.maximin_mean_winners.len(), 1);
    assert_eq!(result.maximin_mean_winners[0].kind, ChoiceKind::ActNow);
    assert_eq!(result.maximin_mean_winners[0].id, "safe");
    assert!(!result.nominal_winner_survives_maximin);
    assert_eq!(
        policy.signal_actions,
        [
            ("bad".into(), Some("safe".into())),
            ("good".into(), Some("build".into())),
            ("impossible".into(), None),
        ]
    );
    assert!(card.to_json().contains("\"fixed_policy_comparison\":{"));
    assert!(
        card.to_json()
            .contains("\"state_net_payoffs\":[\"295/2\",\"-25/2\"]")
    );
    let markdown = card.to_markdown();
    assert!(markdown.contains("Fixed information-policy robustness"));
    assert!(markdown.contains("| information_option | probe |"));
    assert!(markdown.contains("| impossible | unreachable |"));
    assert!(markdown.contains("negative conditional net payoff"));
    assert!(markdown.contains("not realized signal outcomes"));
    for option in &input.information_options {
        assert!(card.claim().parents.contains(&option.claim_id));
        assert!(
            option
                .anchors
                .iter()
                .all(|a| card.claim().anchors.contains(a))
        );
    }
    assert_eq!(card.claim().evidence, EvidenceClass::Assumed);
}

#[test]
fn policy_stress_matches_direct_state_signal_enumeration_and_all_feasible_two_state_laws() {
    // For the unit two-state metric the entire feasible law is |p(high)-1/2| <= radius.
    // Independently enumerate joint state/signal outcomes; no posterior value enters the oracle.
    for accuracy in [r(1, 2), r(2, 3), r(3, 4), ONE] {
        for radius in [ZERO, r(1, 10), r(1, 4), r(1, 2), ONE] {
            let input = fixture(radius, accuracy, 6, 4, "probe");
            let card = evaluate(&input).unwrap();
            let policy = &comparison(&card).policies[0];
            let chosen = |signal: &str, high: bool| -> Rat {
                let action = policy
                    .signal_actions
                    .iter()
                    .find(|(s, _)| s == signal)
                    .unwrap()
                    .1
                    .as_deref()
                    .unwrap();
                Rat::from_integer(match (action, high) {
                    ("build", true) => 200,
                    ("build", false) => -100,
                    ("safe", _) => 30,
                    ("hold", _) => 0,
                    _ => panic!("unknown action"),
                })
            };
            let oracle = |high_mass: Rat| {
                let mut value = r(-10, 1);
                for (high, mass) in [
                    (true, high_mass),
                    (false, ONE.checked_sub(high_mass).unwrap()),
                ] {
                    for (signal, chance) in [
                        (
                            "good",
                            if high {
                                accuracy
                            } else {
                                ONE.checked_sub(accuracy).unwrap()
                            },
                        ),
                        (
                            "bad",
                            if high {
                                ONE.checked_sub(accuracy).unwrap()
                            } else {
                                accuracy
                            },
                        ),
                    ] {
                        let joint = mass.checked_mul(chance).unwrap();
                        value = value
                            .checked_add(joint.checked_mul(chosen(signal, high)).unwrap())
                            .unwrap();
                    }
                }
                value
            };
            assert_eq!(oracle(r(1, 2)), policy.nominal_mean);
            let bound = if compare(radius, r(1, 2)).is_gt() {
                r(1, 2)
            } else {
                radius
            };
            let low = oracle(r(1, 2).checked_sub(bound).unwrap());
            let high = oracle(r(1, 2).checked_add(bound).unwrap());
            assert_eq!(
                policy.worst_case_mean,
                if compare(low, high).is_lt() {
                    low
                } else {
                    high
                }
            );
            for numerator in 0..=20 {
                let mass = r(numerator, 20);
                let distance = mass.checked_sub(r(1, 2)).unwrap();
                let distance = if distance.signum() < 0 {
                    distance.checked_neg().unwrap()
                } else {
                    distance
                };
                if !compare(distance, radius).is_gt() {
                    assert!(!compare(oracle(mass), policy.worst_case_mean).is_lt());
                }
            }
            let certificate = &policy.loss_certificate;
            assert_eq!(
                oracle(certificate.destination_weights[0]),
                policy.worst_case_mean
            );
            assert_eq!(certificate.duality_gap, ZERO);
            assert_eq!(certificate.dual_value, certificate.worst_case_value);
            if radius == ZERO {
                assert_eq!(policy.nominal_mean, policy.worst_case_mean);
            }
            if radius == ONE {
                assert_eq!(certificate.dual_lambda, ZERO);
            }
        }
    }
}

#[test]
fn posterior_tie_resolution_stays_fixed_even_when_reoptimization_would_help() {
    // Bad-signal conditional build value is exactly 30, tied with safe. The nominal id rule picks build.
    let card = evaluate(&fixture(r(1, 4), r(17, 30), 6, 4, "probe")).unwrap();
    let result = comparison(&card);
    let policy = &result.policies[0];
    assert_eq!(policy.posterior_tie_signals, ["bad"]);
    assert_eq!(
        policy.signal_actions[0],
        ("bad".into(), Some("build".into()))
    );
    assert_eq!(
        policy.signal_actions[1],
        ("good".into(), Some("build".into()))
    );
    assert_eq!(policy.state_net_payoffs, [r(190, 1), r(-110, 1)]);
    assert_eq!(policy.nominal_mean, r(40, 1));
    assert_eq!(policy.worst_case_mean, r(-35, 1));
    // Always taking safe after the test would yield 20. A value of 20 here would silently change π.
    assert_ne!(policy.worst_case_mean, r(20, 1));
    assert!(card.to_markdown().contains("Posterior-tied signals"));
}

#[test]
fn exact_ties_keep_action_and_option_with_the_same_identifier_distinct() {
    let card = evaluate(&fixture(ZERO, r(1, 2), 0, 0, "build")).unwrap();
    let result = comparison(&card);
    assert_eq!(result.nominal_expected_value_winners.len(), 2);
    assert_eq!(result.maximin_mean_winners.len(), 2);
    for winners in [
        &result.nominal_expected_value_winners,
        &result.maximin_mean_winners,
    ] {
        assert_eq!(
            (winners[0].kind, winners[0].id.as_str()),
            (ChoiceKind::ActNow, "build")
        );
        assert_eq!(
            (winners[1].kind, winners[1].id.as_str()),
            (ChoiceKind::InformationOption, "build")
        );
    }
    assert!(result.nominal_winner_survives_maximin);
}

#[test]
fn input_order_cost_provenance_and_absent_robustness_keep_their_existing_meaning() {
    let input = fixture(r(1, 4), r(3, 4), 6, 4, "probe");
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
    for option in &mut permuted.information_options {
        option.value.signals.reverse();
        option.value.likelihoods.reverse();
    }
    assert_eq!(evaluate(&permuted).unwrap(), card);
    let alternate_cost_sources = evaluate(&fixture(r(1, 4), r(3, 4), 3, 7, "probe")).unwrap();
    assert_eq!(comparison(&alternate_cost_sources), comparison(&card));
    assert_ne!(alternate_cost_sources.digest(), card.digest());
    let mut nominal = input.clone();
    nominal.robustness = None;
    let original = evaluate(&nominal).unwrap();
    assert_eq!(original.summary().information, card.summary().information);
    assert_eq!(original.summary().alternatives, card.summary().alternatives);
    assert!(!original.to_json().contains("fixed_policy_comparison"));
    assert!(
        !original
            .to_markdown()
            .contains("Fixed information-policy robustness")
    );
    let mut act_now = input.clone();
    act_now.information_options.clear();
    let only_actions = evaluate(&act_now).unwrap();
    assert!(
        only_actions
            .summary()
            .robustness
            .as_ref()
            .unwrap()
            .fixed_policy_comparison
            .is_none()
    );
    assert!(!only_actions.to_json().contains("fixed_policy_comparison"));
    let mut changed = input;
    changed.information_options[0].value.delay_cost.minor += 1;
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-507");
}

#[test]
fn each_information_policy_counts_toward_the_transport_work_limit_before_admission() {
    let mut input = fixture(r(1, 4), r(3, 4), 6, 4, "probe");
    input.weights.value.states = (0..64).map(|i| (format!("state{i}"), r(1, 64))).collect();
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
    input.robustness.as_mut().unwrap().value.distances = distances;
    input.actions.resize(224, input.actions[0].clone());
    input.information_options[0].value.likelihoods = input
        .weights
        .value
        .states
        .iter()
        .flat_map(|(state, _)| {
            ["bad", "good", "impossible"].map(|signal| Likelihood {
                state_id: state.clone(),
                signal_id: signal.into(),
                probability: r(1, 3),
            })
        })
        .collect();
    input
        .information_options
        .resize(64, input.information_options[0].clone());
    assert!(super::super::check_work(&input).is_ok());
    input.actions.push(input.actions[0].clone());
    let refusal = evaluate(&input).unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-201");
    assert!(refusal.what.contains("one per information policy"));
}

#[test]
fn changed_selected_actions_or_original_likelihoods_refuse_the_conditional_audit() {
    let input = fixture(r(1, 4), r(3, 4), 6, 4, "probe");
    let card = evaluate(&input).unwrap();
    let summary = card.summary();
    let original = &card.inputs().information_options[0].value;
    let policy = &summary.information.as_ref().unwrap().options[0];
    let mut changed = policy.clone();
    changed.signals[0].selected_action = Some("build".into());
    assert_eq!(
        conditional_payoffs(original, &changed, summary)
            .unwrap_err()
            .code,
        "FMINE-REF-207"
    );
    let mut changed = original.clone();
    changed.likelihoods[0].probability = ONE;
    assert_eq!(
        conditional_payoffs(&changed, policy, summary)
            .unwrap_err()
            .code,
        "FMINE-REF-207"
    );
    let mut changed = policy.clone();
    changed.net_value.major = ZERO;
    assert_eq!(
        conditional_payoffs(original, &changed, summary)
            .unwrap_err()
            .code,
        "FMINE-REF-207"
    );
}
