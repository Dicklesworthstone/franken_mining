//! Independent finite-atom oracles and source-bound constrained-policy examples.
//! Determinism: Exact

use super::*;
use fmine_core::anchor::SourceAnchor;

const MODEL: &str = include_str!("../../../../../../tests/decisions/constrained_testwork.toml");

fn r(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}

fn parsed(text: &str) -> Result<DecisionInputs> {
    model::parse(
        text,
        SourceAnchor::new(
            "invented-policy.toml",
            sha256(text.as_bytes()).0,
            "declared model",
        ),
    )
}

fn result(card: &EvaluatedCard) -> &PolicyOptimizationSummary {
    card.summary()
        .downside
        .as_ref()
        .unwrap()
        .policy_optimization
        .as_ref()
        .unwrap()
}

fn run(text: &str) -> EvaluatedCard {
    evaluate(&parsed(text).unwrap()).unwrap()
}

fn retag<T>(old: &Claim<T>, value: T, tag: String) -> Claim<T> {
    Claim::new(
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
    )
}

fn change_limit(input: &mut DecisionInputs, f: impl FnOnce(&mut DownsideLimit)) {
    let old = input.downside_limit.as_ref().unwrap();
    let mut value = old.value.clone();
    f(&mut value);
    let tag = value.canonical_tag();
    input.downside_limit = Some(retag(old, value, tag));
}

#[test]
fn constrained_mapping_recovers_value_rejected_by_the_fixed_ev_policy() {
    let input = parsed(MODEL).unwrap();
    let card = evaluate(&input).unwrap();
    let optimization = result(&card);
    let option = &optimization.options[0];
    let optimum = option.optimum.as_ref().unwrap();
    assert_eq!(optimization.policies_evaluated, 8);
    assert_eq!(optimization.work_units, 76);
    assert_eq!(option.reachable_signals, 3);
    assert_eq!(option.feasible_policies, 2);
    assert_eq!(option.optimal_policy_count, 1);
    assert_eq!(option.fixed_policy_mean, r(34, 1));
    assert_eq!(option.fixed_policy_screened_breach_probability, r(1, 10));
    assert!(!option.fixed_policy_eligible);
    assert_eq!(optimum.nominal_mean, r(57, 2));
    assert_eq!(optimum.nominal_breach_probability, r(1, 100));
    assert_eq!(optimum.screened_breach_probability, r(1, 100));
    assert_eq!(optimum.conditional_breach_probabilities, [ZERO, r(1, 50)]);
    assert_eq!(
        optimum.signal_actions,
        [
            ("low".into(), Some("hold".into())),
            ("medium".into(), Some("hold".into())),
            ("strong".into(), Some("act".into())),
        ]
    );
    assert_eq!(option.expected_value_cost_of_limit, Some(r(11, 2)));
    assert_eq!(
        option.improvement_over_best_eligible_act_now,
        Some(r(57, 2))
    );
    assert_eq!(
        optimization.best_eligible_act_now.best_expected_value,
        Some(ZERO)
    );
    assert_eq!(optimization.best_eligible_act_now.winners[0].id, "hold");
    assert_eq!(optimization.overall.winners[0].id, "testwork");
    assert_eq!(
        optimization.overall.expected_value_cost_of_limit,
        Some(r(11, 2))
    );
    let risk = &optimum.risk;
    assert_eq!(risk.mean.major, r(57, 2));
    assert_eq!(risk.cvar95_loss.major, r(11, 1));
    assert_eq!(risk.loss_probability, r(7, 10));
    assert_eq!(
        risk.p10_p50_p90.map(|m| m.major),
        [r(-1, 1), r(-1, 1), r(99, 1)]
    );
    assert_eq!(
        risk.distribution
            .iter()
            .map(|b| (b.net_payoff.major, b.probability))
            .collect::<Vec<_>>(),
        [
            (r(-51, 1), r(1, 100)),
            (r(-1, 1), r(69, 100)),
            (r(99, 1), r(3, 10))
        ]
    );
    // The existing fixed EV policy and its rejected downside result remain available unchanged.
    assert_eq!(
        card.summary().information.as_ref().unwrap().options[0]
            .net_value
            .major,
        r(34, 1)
    );
    assert_eq!(
        card.summary().downside.as_ref().unwrap().nominal.winners[0].id,
        "hold"
    );
    assert!(card.to_json().contains("\"optimal_policy_count\":1"));
    assert!(
        card.to_markdown()
            .contains("Optimized one-test downside policies")
    );
    let limit = input.downside_limit.as_ref().unwrap();
    assert!(card.claim().parents.contains(&limit.claim_id));
    assert!(
        limit
            .anchors
            .iter()
            .all(|a| card.claim().anchors.contains(a))
    );
    assert!(
        limit
            .anchors
            .iter()
            .any(|a| a.locator.contains("policy_optimization declaration"))
    );
    let mut reversed = input.clone();
    reversed.actions.reverse();
    reversed.weights.value.states.reverse();
    reversed.information_options[0].value.signals.reverse();
    reversed.information_options[0].value.likelihoods.reverse();
    assert_eq!(evaluate(&reversed).unwrap().to_json(), card.to_json());
    assert_eq!(input, parsed(MODEL).unwrap());
}

#[test]
fn costs_apply_once_even_to_hold_and_the_floor_is_strict() {
    let delayed = run(&MODEL.replace("delay_cost = \"0\"", "delay_cost = \"2\""));
    let policy = result(&delayed).options[0].optimum.as_ref().unwrap();
    assert_eq!(policy.nominal_mean, r(53, 2));
    assert_eq!(policy.risk.cvar95_loss.major, r(13, 1));
    assert!(
        policy
            .risk
            .distribution
            .iter()
            .any(|bin| bin.net_payoff.major == r(-3, 1) && bin.probability == r(69, 100))
    );
    let at_floor = run(&MODEL.replace("floor = \"-10\"", "floor = \"-1\""));
    assert_eq!(
        result(&at_floor).options[0]
            .optimum
            .as_ref()
            .unwrap()
            .nominal_mean,
        r(57, 2)
    );
    let above_floor = run(&MODEL.replace("floor = \"-10\"", "floor = \"-0.9999\""));
    assert!(result(&above_floor).options[0].optimum.is_none());
    assert_eq!(result(&above_floor).overall.winners[0].id, "hold");
    let equality = run(&MODEL.replace(
        "max_breach_probability = \"1/20\"",
        "max_breach_probability = \"1/100\"",
    ));
    assert_eq!(
        result(&equality).options[0]
            .optimum
            .as_ref()
            .unwrap()
            .nominal_breach_probability,
        r(1, 100)
    );
}

fn geometry() -> String {
    let mut text = String::from(
        "\n[[robustness]]\nmethod = \"wasserstein_1\"\nradius = \"1/4\"\nmetric_basis = \"invented unit-distance regimes\"\nradius_basis = \"invented quarter-mass stress, uncalibrated\"\n",
    );
    for from in ["high", "low"] {
        for to in ["high", "low"] {
            let value = usize::from(from != to);
            text.push_str(&format!("\n[[transport_distance]]\nfrom = \"{from}\"\nto = \"{to}\"\nvalue = \"{value}/1\"\n"));
        }
    }
    text
}

#[test]
fn wasserstein_screen_changes_feasibility_and_preserves_nominal_objective() {
    let text = format!("{MODEL}{}", geometry());
    let nominal = run(&text.replace(
        "max_breach_probability = \"1/20\"",
        "max_breach_probability = \"1/100\"",
    ));
    assert_eq!(
        result(&nominal).options[0]
            .optimum
            .as_ref()
            .unwrap()
            .nominal_mean,
        r(57, 2)
    );
    let robust = run(&text
        .replace("screen = \"nominal\"", "screen = \"wasserstein\"")
        .replace(
            "max_breach_probability = \"1/20\"",
            "max_breach_probability = \"1/100\"",
        ));
    let option = &result(&robust).options[0];
    assert_eq!(
        result(&robust).configuration.screen,
        PolicyScreen::Wasserstein
    );
    assert_eq!(result(&robust).work_units, 108);
    assert_eq!(option.optimum.as_ref().unwrap().nominal_mean, r(-1, 1));
    assert_eq!(
        option.improvement_over_best_eligible_act_now,
        Some(r(-1, 1))
    );
    assert_eq!(result(&robust).overall.winners[0].id, "hold");
    let equality = run(&text
        .replace("screen = \"nominal\"", "screen = \"wasserstein\"")
        .replace(
            "max_breach_probability = \"1/20\"",
            "max_breach_probability = \"3/200\"",
        ));
    let optimum = result(&equality).options[0].optimum.as_ref().unwrap();
    assert_eq!(optimum.nominal_mean, r(57, 2));
    assert_eq!(optimum.screened_breach_probability, r(3, 200));
    let certificate = optimum.breach_certificate.as_ref().unwrap();
    assert_eq!(certificate.duality_gap, ZERO);
    assert_eq!(certificate.destination_weights, [r(1, 4), r(3, 4)]);
    assert!(equality.to_json().contains("\"screen\":\"wasserstein\""));
}

#[test]
fn all_infeasible_has_no_mapping_winner_or_fabricated_comparison() {
    let card = run(&MODEL.replace("floor = \"-10\"", "floor = \"101\""));
    let search = result(&card);
    let option = &search.options[0];
    assert_eq!(option.policies_evaluated, 8);
    assert_eq!(option.feasible_policies, 0);
    assert_eq!(option.optimal_policy_count, 0);
    assert!(option.optimum.is_none());
    assert!(option.expected_value_cost_of_limit.is_none());
    assert!(option.improvement_over_best_eligible_act_now.is_none());
    for ranking in [&search.best_eligible_act_now, &search.overall] {
        assert_eq!(ranking.status, DownsideStatus::NoEligibleChoice);
        assert!(ranking.winners.is_empty());
        assert!(ranking.best_expected_value.is_none());
        assert!(ranking.expected_value_cost_of_limit.is_none());
    }
    assert!(card.to_json().contains("\"optimum\":null"));
}

/// Original model construction for the independent twenty-equiprobable-atom oracle below.
fn small_model(
    payoffs: [[i128; 2]; 3],
    x_counts: [usize; 2],
    cost: i128,
    floor: i128,
    cap: usize,
) -> String {
    let mut text = String::from(
        "schema = \"fmine.decision-model.v1\"\nmodel_key = \"invented-atom-oracle\"\nquestion = \"Compare invented finite atoms\"\ncurrency = \"USD\"\nvaluation_basis = \"invented NPV\"\n",
    );
    for state in ["high", "low"] {
        text.push_str(&format!(
            "\n[[state]]\nid = \"{state}\"\nweight = \"1/2\"\n"
        ));
    }
    for (id, values) in ["a", "b", "hold"].into_iter().zip(payoffs) {
        let role = if id == "hold" { "do_nothing" } else { "action" };
        text.push_str(&format!(
            "\n[[action]]\nid = \"{id}\"\nlabel = \"{id}\"\nrole = \"{role}\"\n"
        ));
        for (state, value) in ["high", "low"].into_iter().zip(values) {
            text.push_str(&format!(
                "\n[[payoff]]\naction = \"{id}\"\nstate = \"{state}\"\nvalue = \"{value}\"\n"
            ));
        }
    }
    text.push_str(&format!("\n[[information_option]]\nid = \"probe\"\nlabel = \"Invented probe\"\ntest_cost = \"{cost}\"\ndelay_cost = \"0\"\ndelay_days = 0\n"));
    for signal in ["x", "y"] {
        text.push_str(&format!(
            "\n[[signal]]\noption = \"probe\"\nid = \"{signal}\"\n"
        ));
        for (state, count) in ["high", "low"].into_iter().zip(x_counts) {
            let count = if signal == "x" { count } else { 10 - count };
            text.push_str(&format!("\n[[likelihood]]\noption = \"probe\"\nstate = \"{state}\"\nsignal = \"{signal}\"\nprobability = \"{count}/10\"\n"));
        }
    }
    text.push_str(&format!("\n[[downside_limit]]\nfloor = \"{floor}\"\nmax_breach_probability = \"{cap}/20\"\n\n[[policy_optimization]]\nmethod = \"exact_deterministic\"\nscreen = \"nominal\"\nmax_policies = 4096\nmax_work = 1000000\n\n[[uu]]\nid = \"unknown\"\ndescription = \"invented omitted regime\"\ntrigger = \"new evidence\"\ndownside_usd_m = \"1\"\n"));
    text
}

#[test]
fn exact_tie_count_and_canonical_representative_ignore_unreachable_signals() {
    let text = small_model([[10, 10], [10, 10], [0, 0]], [5, 5], 0, 0, 0);
    let card = run(&text);
    let option = &result(&card).options[0];
    assert_eq!(option.policies_evaluated, 9);
    assert_eq!(option.optimal_policy_count, 4);
    assert!(
        option
            .optimum
            .as_ref()
            .unwrap()
            .signal_actions
            .iter()
            .all(|(_, a)| a.as_deref() == Some("a"))
    );
    let unreachable = run(&small_model(
        [[10, 10], [10, 10], [0, 0]],
        [10, 10],
        0,
        0,
        0,
    ));
    let option = &result(&unreachable).options[0];
    assert_eq!(option.reachable_signals, 1);
    assert_eq!(option.policies_evaluated, 3);
    assert_eq!(option.optimal_policy_count, 2);
    let optimum = option.optimum.as_ref().unwrap();
    assert_eq!(
        optimum.signal_actions,
        [("x".into(), Some("a".into())), ("y".into(), None)]
    );
    assert_eq!(optimum.risk.cvar95_loss.major, r(-10, 1));
}

#[test]
fn independent_integer_atom_oracle_matches_complete_search_and_representative_risk() {
    for case in 0..24usize {
        let payoffs = [
            [(case % 9) as i128 - 2, -((case % 7) as i128)],
            [(case % 5) as i128, (case % 11) as i128 - 3],
            [0, 0],
        ];
        let x_counts = [case % 11, (case * 7 + 3) % 11];
        let cost = (case % 3) as i128;
        let floor = [-2, 0, 5][case % 3];
        let cap = [0, 2, 10, 20][case % 4];
        let card = run(&small_model(payoffs, x_counts, cost, floor, cap));
        let actual = &result(&card).options[0];
        let reachable: Vec<_> = (0..2)
            .filter(|&signal| {
                x_counts
                    .iter()
                    .any(|&n| if signal == 0 { n > 0 } else { n < 10 })
            })
            .collect();
        let mut best: Option<(i128, Vec<usize>, Vec<i128>)> = None;
        let (mut feasible, mut ties) = (0u64, 0u64);
        // This oracle expands the original integer likelihoods into twenty equiprobable atoms.
        // It does not call policy, risk, transport or ranking helpers from the implementation.
        for encoded in 0..3usize.pow(reachable.len() as u32) {
            let mut quotient = encoded;
            let mapping: Vec<_> = reachable
                .iter()
                .map(|_| {
                    let a = quotient % 3;
                    quotient /= 3;
                    a
                })
                .collect();
            let mut atoms = Vec::new();
            for (state, &cut) in x_counts.iter().enumerate() {
                for draw in 0..10 {
                    let signal = usize::from(draw >= cut);
                    let action = mapping[reachable.iter().position(|&s| s == signal).unwrap()];
                    atoms.push(payoffs[action][state] - cost);
                }
            }
            if atoms.iter().filter(|&&v| v < floor).count() > cap {
                continue;
            }
            feasible += 1;
            let total: i128 = atoms.iter().sum();
            match best.as_ref().map(|(v, _, _)| total.cmp(v)) {
                None | Some(Ordering::Greater) => {
                    best = Some((total, mapping, atoms));
                    ties = 1;
                }
                Some(Ordering::Equal) => {
                    ties += 1;
                    if mapping < best.as_ref().unwrap().1 {
                        best = Some((total, mapping, atoms));
                    }
                }
                Some(Ordering::Less) => {}
            }
        }
        assert_eq!(actual.feasible_policies, feasible, "case {case}");
        assert_eq!(actual.optimal_policy_count, ties, "case {case}");
        match (actual.optimum.as_ref(), best) {
            (None, None) => {}
            (Some(actual), Some((total, mapping, mut atoms))) => {
                assert_eq!(actual.nominal_mean, r(total, 20), "case {case}");
                assert_eq!(
                    actual.nominal_breach_probability,
                    r(atoms.iter().filter(|&&v| v < floor).count() as i128, 20)
                );
                for (&signal, &action) in reachable.iter().zip(&mapping) {
                    assert_eq!(
                        actual.signal_actions[signal].1.as_deref(),
                        Some(["a", "b", "hold"][action])
                    );
                }
                atoms.sort();
                assert_eq!(actual.risk.cvar95_loss.major, r(-atoms[0], 1));
                assert_eq!(
                    actual.risk.p10_p50_p90.map(|v| v.major),
                    [r(atoms[1], 1), r(atoms[9], 1), r(atoms[17], 1)]
                );
            }
            _ => panic!("case {case}: feasible-population disagreement"),
        }
    }
}

#[test]
fn complete_search_budgets_are_inclusive_global_and_fail_before_any_optimum() {
    let exact = MODEL
        .replace("max_policies = 4096", "max_policies = 8")
        .replace("max_work = 1000000", "max_work = 76");
    assert_eq!(result(&run(&exact)).policies_evaluated, 8);
    for text in [
        exact.replace("max_policies = 8", "max_policies = 7"),
        exact.replace("max_work = 76", "max_work = 75"),
    ] {
        let err = evaluate(&parsed(&text).unwrap()).unwrap_err();
        assert_eq!(err.code, "FMINE-REF-203");
        assert!(err.to_string().contains("no policy optimum was produced"));
    }
    let mut two = parsed(&exact).unwrap();
    let old = &two.information_options[0];
    let mut value = old.value.clone();
    value.option_id = "another_test".into();
    let tag = value.canonical_tag();
    two.information_options.push(retag(old, value, tag));
    assert_eq!(evaluate(&two).unwrap_err().code, "FMINE-REF-203");
    change_limit(&mut two, |limit| {
        let budget = limit.policy_optimization.as_mut().unwrap();
        budget.max_policies = 16;
        budget.max_work = 152;
    });
    let card = evaluate(&two).unwrap();
    assert_eq!(result(&card).policies_evaluated, 16);
    assert_eq!(result(&card).overall.winners.len(), 2);
    // Typed API callers receive the same domain check even with a correctly retagged claim.
    change_limit(&mut two, |limit| {
        limit.policy_optimization.as_mut().unwrap().max_policies = 0
    });
    assert_eq!(evaluate(&two).unwrap_err().code, "FMINE-REF-203");

    // A valid conditional matrix can imply millions of mappings; refuse the product without
    // constructing that population. Only the ordinary 25-signal input and statistics are present.
    let mut large = parsed(MODEL).unwrap();
    let old = &large.information_options[0];
    let mut value = old.value.clone();
    value.signals = (0..25).map(|i| format!("signal_{i:02}")).collect();
    value.likelihoods = ["high", "low"]
        .into_iter()
        .flat_map(|state| {
            value
                .signals
                .iter()
                .map(move |signal| information::Likelihood {
                    state_id: state.into(),
                    signal_id: signal.clone(),
                    probability: r(1, 25),
                })
        })
        .collect();
    let tag = value.canonical_tag();
    large.information_options[0] = retag(old, value, tag);
    change_limit(&mut large, |limit| {
        let budget = limit.policy_optimization.as_mut().unwrap();
        budget.max_policies = MAX_POLICIES;
        budget.max_work = MAX_WORK;
    });
    assert_eq!(evaluate(&large).unwrap_err().code, "FMINE-REF-203");
}

#[test]
fn opt_in_is_source_bound_strict_and_leaves_old_fixed_policy_behavior_intact() {
    let mut original = parsed(MODEL).unwrap();
    let enabled = evaluate(&original).unwrap();
    change_limit(&mut original, |limit| limit.policy_optimization = None);
    let disabled = evaluate(&original).unwrap();
    assert!(
        disabled
            .summary()
            .downside
            .as_ref()
            .unwrap()
            .policy_optimization
            .is_none()
    );
    assert_eq!(
        disabled.summary().information,
        enabled.summary().information
    );
    assert_eq!(
        disabled.summary().downside.as_ref().unwrap().choices,
        enabled.summary().downside.as_ref().unwrap().choices
    );
    assert_ne!(disabled.claim().claim_id, enabled.claim().claim_id);
    let limit = &original.downside_limit.as_ref().unwrap().value;
    let mut old_frame = Encoder::new_frame(*b"DCDL", 1);
    old_frame.put_str(&limit.model_key);
    old_frame.put_str(&limit.valuation_basis);
    old_frame.put_str(currency_name(limit.floor.ccy));
    old_frame.put_fixed(&limit.floor.minor.to_le_bytes());
    put_rat(&mut old_frame, limit.max_breach_probability);
    assert_eq!(
        limit.canonical_tag(),
        format!("decision-downside-limit/v1:{}", sha256(&old_frame.finish()))
    );
    for text in [
        MODEL.replace("method = \"exact_deterministic\"", "method = \"greedy\""),
        MODEL.replace("screen = \"nominal\"", "screen = \"cvar\""),
        MODEL.replace("max_policies = 4096", "max_policies = 0"),
        MODEL.replace("max_work = 1000000", "max_work = 4000001"),
        MODEL.replace("max_work = 1000000", "max_work = \"1000000\""),
        MODEL.replace("max_work = 1000000", "max_work = 1000000\nunknown = \"x\""),
        MODEL.replace(
            "[[downside_limit]]\nfloor = \"-10\"\nmax_breach_probability = \"1/20\"\n",
            "",
        ),
    ] {
        assert!(parsed(&text).is_err(), "{text}");
    }
    let robust_without_geometry =
        parsed(&MODEL.replace("screen = \"nominal\"", "screen = \"wasserstein\"")).unwrap();
    assert_eq!(
        evaluate(&robust_without_geometry).unwrap_err().code,
        "FMINE-REF-201"
    );
    let duplicate = format!(
        "{MODEL}\n[[policy_optimization]]\nmethod = \"exact_deterministic\"\nscreen = \"nominal\"\nmax_policies = 8\nmax_work = 76\n"
    );
    assert!(parsed(&duplicate).is_err());
    let mut no_information = parsed(MODEL).unwrap();
    no_information.information_options.clear();
    assert_eq!(evaluate(&no_information).unwrap_err().code, "FMINE-REF-201");
    let changed_budget = run(&MODEL.replace("max_policies = 4096", "max_policies = 4095"));
    assert_ne!(enabled.claim().claim_id, changed_budget.claim().claim_id);
}
