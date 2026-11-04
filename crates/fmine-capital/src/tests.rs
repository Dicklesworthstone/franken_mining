//! Known-answer and admission controls for the declared static capital program.
//! Determinism: Exact
#![cfg(test)]

use super::*;
use fmine_evidence::KuKind;

pub(crate) fn r(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}
pub(crate) fn usd(n: i128) -> Money {
    Money::from_minor(Currency::USD, n.checked_mul(Money::SCALE).unwrap())
}
pub(crate) fn claim<T>(metric: &str, value: T, tag: &str, add: AdditivityClass) -> Claim<T> {
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        add,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Estimated,
        Confidentiality::internal(),
        vec![UuTerm {
            scenario_id: "model-error".into(),
            description: "Unrepresented joint shocks".into(),
            downside_exposure_usd_m: 25.0,
            trigger_indicator: "Source model no longer matches the operating regime".into(),
        }],
        vec![],
        vec![SourceAnchor::new(
            "synthetic/capital-test.toml",
            [42; 32],
            metric,
        )],
    )
}
pub(crate) fn policy_claim(value: ProgramPolicy) -> Claim<ProgramPolicy> {
    let tag = value.canonical_tag();
    claim("capital.policy", value, &tag, AdditivityClass::NonAdditive)
}
pub(crate) fn candidate_claim(value: CapitalCandidate) -> Claim<CapitalCandidate> {
    let tag = value.canonical_tag();
    claim(
        &format!("capital.{}", value.candidate_id),
        value,
        &tag,
        AdditivityClass::ExtensiveAdditive,
    )
}
fn funding_claim(value: ProgramFunding) -> Claim<ProgramFunding> {
    let tag = value.canonical_tag();
    claim("capital.funding", value, &tag, AdditivityClass::NonAdditive)
}
pub(crate) fn schedule_claim(value: FundingSchedule) -> Claim<FundingSchedule> {
    let tag = value.canonical_tag();
    claim(
        "capital.cash-schedule",
        value,
        &tag,
        AdditivityClass::NonAdditive,
    )
}
pub(crate) fn with_schedule(mut inputs: ProgramInputs) -> ProgramInputs {
    let states = &inputs.weights.value.states;
    let periods = vec![
        CashPeriod {
            id: "start".into(),
            cash_period: "2030-01-01/2030-06-30".into(),
            minimum_cash: usd(20),
        },
        CashPeriod {
            id: "finish".into(),
            cash_period: "2030-07-01/2030-12-31".into(),
            minimum_cash: usd(20),
        },
    ];
    let mut baseline_flows = Vec::new();
    let mut candidate_flows = Vec::new();
    for period in &periods {
        for (state, _) in states {
            baseline_flows.push(StateCashFlow {
                period_id: period.id.clone(),
                state_id: state.clone(),
                value: usd(0),
            });
            for candidate in &inputs.candidates {
                candidate_flows.push(CandidateCashFlow {
                    period_id: period.id.clone(),
                    candidate_id: candidate.value.candidate_id.clone(),
                    state_id: state.clone(),
                    value: usd(0),
                });
            }
        }
    }
    inputs.funding_schedule = Some(schedule_claim(FundingSchedule {
        model_key: inputs.policy.value.model_key.clone(),
        currency: Currency::USD,
        cash_basis: "Synthetic nominal USD cash; full project ownership, signed cash flows".into(),
        opening_date: "2030-01-01".into(),
        opening_cash: states
            .iter()
            .map(|(state, _)| (state.clone(), usd(100)))
            .collect(),
        periods,
        baseline_flows,
        candidate_flows,
    }));
    inputs
}
pub(crate) fn set_candidate_flow(
    schedule: &mut FundingSchedule,
    period: &str,
    candidate: &str,
    state: &str,
    value: i128,
) {
    schedule
        .candidate_flows
        .iter_mut()
        .find(|flow| {
            flow.period_id == period && flow.candidate_id == candidate && flow.state_id == state
        })
        .unwrap()
        .value = usd(value);
}
pub(crate) fn set_baseline_flow(
    schedule: &mut FundingSchedule,
    period: &str,
    state: &str,
    value: i128,
) {
    schedule
        .baseline_flows
        .iter_mut()
        .find(|flow| flow.period_id == period && flow.state_id == state)
        .unwrap()
        .value = usd(value);
}
pub(crate) fn equity_claim(value: EquityBasis) -> Claim<EquityBasis> {
    let tag = value.canonical_tag();
    claim("capital.equity", value, &tag, AdditivityClass::NonAdditive)
}
pub(crate) fn with_equity(
    mut inputs: ProgramInputs,
    baseline_nav: &[i128],
    initial_shares: Rat,
) -> ProgramInputs {
    let mut policy = inputs.policy.value.clone();
    policy.payoff_convention = EQUITY_PAYOFF_CONVENTION.into();
    inputs.policy = policy_claim(policy);
    let mut share_changes = Vec::new();
    for candidate in &inputs.candidates {
        for (state, _) in &inputs.weights.value.states {
            share_changes.push(CandidateShareChange {
                candidate_id: candidate.value.candidate_id.clone(),
                state_id: state.clone(),
                units: ZERO,
            });
        }
    }
    inputs.equity = Some(equity_claim(EquityBasis {
        model_key: inputs.policy.value.model_key.clone(),
        currency: Currency::USD,
        valuation_basis: inputs.policy.value.valuation_basis.clone(),
        entity: "synthetic-parent".into(),
        share_class: "ordinary".into(),
        shares_per_unit: ONE,
        initial_share_units: initial_shares,
        baseline_nav: inputs
            .weights
            .value
            .states
            .iter()
            .zip(baseline_nav)
            .map(|((state, _), value)| (state.clone(), usd(*value)))
            .collect(),
        share_changes,
        max_tail_loss_per_share: None,
    }));
    inputs
}
pub(crate) fn set_share_change(equity: &mut EquityBasis, candidate: &str, state: &str, units: Rat) {
    equity
        .share_changes
        .iter_mut()
        .find(|change| change.candidate_id == candidate && change.state_id == state)
        .unwrap()
        .units = units;
}
fn equity_state<'a>(choice: &'a ProgramChoice, state: &str) -> &'a EquityState {
    choice
        .equity
        .as_ref()
        .unwrap()
        .states
        .iter()
        .find(|row| row.state_id == state)
        .unwrap()
}
fn with_funding(
    mut i: ProgramInputs,
    available: &[i128],
    required: &[(&str, &[i128])],
    minimum: i128,
) -> ProgramInputs {
    let states = &i.weights.value.states;
    let funding = ProgramFunding {
        model_key: i.policy.value.model_key.clone(),
        currency: Currency::USD,
        cash_period: "2030-01-01/2030-12-31".into(),
        cash_basis: "Nominal USD, full project ownership; available cash before this program"
            .into(),
        minimum_cash: usd(minimum),
        available_cash: states
            .iter()
            .zip(available)
            .map(|((id, _), value)| (id.clone(), usd(*value)))
            .collect(),
        requirements: required
            .iter()
            .flat_map(|(candidate, values)| {
                states
                    .iter()
                    .zip(*values)
                    .map(move |((id, _), value)| CashRequirement {
                        candidate_id: (*candidate).into(),
                        state_id: id.clone(),
                        value: usd(*value),
                    })
            })
            .collect(),
    };
    i.funding = Some(funding_claim(funding));
    i
}
pub(crate) fn inputs(
    rows: &[(&str, i128, &[i128])],
    weights: &[Rat],
    budget: i128,
    tail: Rat,
    cap: Option<i128>,
) -> ProgramInputs {
    let policy = ProgramPolicy {
        model_key: "synthetic".into(),
        currency: Currency::USD,
        valuation_basis: "synthetic fixed ownership and valuation date; net USD NAV".into(),
        budget: usd(budget),
        tail_mass: tail,
        max_tail_loss: cap.map(usd),
        payoff_convention: PAYOFF_CONVENTION.into(),
        reference_class_basis: "Unadjusted illustrative inputs; no empirical reference class"
            .into(),
    };
    let weights = WeightSet {
        model_key: policy.model_key.clone(),
        question: "Which declared subset meets the budget and risk limit?".into(),
        states: weights
            .iter()
            .enumerate()
            .map(|(i, w)| (format!("s{i}"), *w))
            .collect(),
    };
    let tag = weights.canonical_tag();
    let candidates = rows
        .iter()
        .map(|(id, cost, values)| {
            candidate_claim(CapitalCandidate {
                model_key: policy.model_key.clone(),
                candidate_id: (*id).into(),
                label: (*id).into(),
                currency: Currency::USD,
                valuation_basis: policy.valuation_basis.clone(),
                cost: usd(*cost),
                payoffs: values
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (format!("s{i}"), usd(*v)))
                    .collect(),
                requires: vec![],
                exclusion_group: None,
                reference_class_basis: "Unadjusted declared synthetic values".into(),
            })
        })
        .collect();
    ProgramInputs {
        policy: policy_claim(policy),
        weights: claim(
            "capital.weights",
            weights,
            &tag,
            AdditivityClass::NonAdditive,
        ),
        candidates,
        funding: None,
        funding_schedule: None,
        entity_funding: None,
        equity: None,
        entity_valuations: vec![],
        selection_stability: None,
    }
}
fn row<'a>(report: &'a EvaluatedProgram, members: &[&str]) -> &'a ProgramChoice {
    report
        .summary()
        .choices
        .iter()
        .find(|r| {
            r.selected
                .iter()
                .map(String::as_str)
                .eq(members.iter().copied())
        })
        .expect("declared subset retained")
}
fn selected_winners(report: &EvaluatedProgram) -> Vec<Vec<String>> {
    report
        .summary()
        .choices
        .iter()
        .filter(|r| report.summary().feasible_winners.contains(&r.bundle_id))
        .map(|r| r.selected.clone())
        .collect()
}
fn checkpoint<'a>(
    choice: &'a ProgramChoice,
    period: Option<&str>,
    state: &str,
) -> &'a CashScheduleCheckpoint {
    choice
        .funding_schedule
        .as_ref()
        .unwrap()
        .checkpoints
        .iter()
        .find(|row| row.period_id.as_deref() == period && row.state_id == state)
        .unwrap()
}

#[test]
fn gb30_uses_displayed_inputs_exactly_and_joint_tail_risk_changes_the_selection() {
    let a = [
        -60_000_000,
        40_000_000,
        220_000_000,
        480_000_000,
        220_000_000,
    ];
    let e = [
        -150_000_000,
        60_000_000,
        420_000_000,
        960_000_000,
        -330_000_000,
    ];
    let d = [-70_600_000, 0, 70_600_000, 258_800_000, 47_100_000];
    let mut i = inputs(
        &[
            ("A", 400_000_000, &a),
            ("E", 400_000_000, &e),
            ("D", 400_000_000, &d),
        ],
        &[r(1, 5); 5],
        800_000_000,
        r(1, 5),
        Some(150_000_000),
    );
    let report = evaluate(&i).unwrap();
    assert_eq!(report.summary().choices.len(), 8);
    assert_eq!(
        selected_winners(&report),
        vec![vec!["A".to_string(), "D".to_string()]]
    );
    let ae = row(&report, &["A", "E"]);
    assert_eq!(
        ae.statistics.as_ref().unwrap().mean.major,
        r(372_000_000, 1)
    );
    assert_eq!(ae.tail_loss.unwrap().major, r(210_000_000, 1));
    assert!(ae.risk_rejected);
    assert_eq!(
        row(&report, &["A", "D"])
            .statistics
            .as_ref()
            .unwrap()
            .mean
            .major,
        r(241_180_000, 1)
    );
    assert_eq!(
        report
            .summary()
            .expected_value_cost_of_risk_limit
            .unwrap()
            .major,
        r(130_820_000, 1)
    );
    let es = [
        -90_000_000,
        100_000_000,
        410_000_000,
        820_000_000,
        -260_000_000,
    ];
    let mut streamed = i.candidates[1].value.clone();
    streamed.candidate_id = "Es".into();
    streamed.label = "Streamed E".into();
    streamed.payoffs = es
        .iter()
        .enumerate()
        .map(|(n, v)| (format!("s{n}"), usd(*v)))
        .collect();
    streamed.exclusion_group = Some("E-structure".into());
    let mut original = i.candidates[1].value.clone();
    original.exclusion_group = streamed.exclusion_group.clone();
    i.candidates[1] = candidate_claim(original);
    i.candidates.push(candidate_claim(streamed));
    let report = evaluate(&i).unwrap();
    assert_eq!(
        selected_winners(&report),
        vec![vec!["A".to_string(), "Es".to_string()]]
    );
    assert_eq!(
        row(&report, &["A", "Es"])
            .statistics
            .as_ref()
            .unwrap()
            .mean
            .major,
        r(376_000_000, 1)
    );
    assert_eq!(
        row(&report, &["A", "Es"]).tail_loss.unwrap().major,
        r(150_000_000, 1)
    );
    assert_eq!(row(&report, &["A", "Es"]).budget_slack.major, ZERO);
    assert!(
        row(&report, &["E", "Es"])
            .structural_rejections
            .iter()
            .any(|v| matches!(v, StructuralRejection::MutualExclusion { .. }))
    );
}

#[test]
fn fractional_tail_and_same_state_diversification_are_not_sums_of_marginal_risks() {
    let i = inputs(
        &[("A", 1, &[-100, -20, 50])],
        &[r(1, 10), r(3, 10), r(3, 5)],
        1,
        r(1, 4),
        None,
    );
    let report = evaluate(&i).unwrap();
    assert_eq!(row(&report, &["A"]).tail_loss.unwrap().major, r(52, 1));
    assert_eq!(
        row(&report, &["A"])
            .statistics
            .as_ref()
            .unwrap()
            .cvar95_loss
            .major,
        r(100, 1)
    );
    let i = inputs(
        &[("A", 1, &[-100, 100]), ("B", 1, &[100, -100])],
        &[r(1, 2); 2],
        2,
        r(1, 5),
        Some(0),
    );
    let report = evaluate(&i).unwrap();
    assert!(row(&report, &["A"]).risk_rejected && row(&report, &["B"]).risk_rejected);
    assert_eq!(row(&report, &["A", "B"]).tail_loss.unwrap().major, ZERO);
    assert_eq!(
        selected_winners(&report),
        vec![vec![], vec!["A".to_string(), "B".to_string()]]
    );
}

#[test]
fn prerequisites_empty_choice_and_infeasible_risk_are_explicit() {
    let mut i = inputs(
        &[("base", 3, &[-1]), ("expansion", 7, &[20])],
        &[ONE],
        10,
        ONE,
        None,
    );
    let mut c = i.candidates[1].value.clone();
    c.requires = vec!["base".into()];
    i.candidates[1] = candidate_claim(c);
    let report = evaluate(&i).unwrap();
    assert!(row(&report, &["expansion"]).statistics.is_none());
    assert_eq!(row(&report, &["base", "expansion"]).spend.major, r(10, 1));
    assert_eq!(
        selected_winners(&report),
        vec![vec!["base".to_string(), "expansion".to_string()]]
    );
    let mut p = i.policy.value.clone();
    p.budget = usd(0);
    i.policy = policy_claim(p);
    let report = evaluate(&i).unwrap();
    assert_eq!(selected_winners(&report), vec![Vec::<String>::new()]);
    let zero = ExactMoney {
        currency: Currency::USD,
        major: ZERO,
    };
    assert_eq!(
        row(&report, &[]).statistics.as_ref().unwrap(),
        &AlternativeStats {
            action_id: "bundle_00".into(),
            label: "Do nothing".into(),
            role: ActionRole::DoNothing,
            mean: zero,
            p10_p50_p90: [zero; 3],
            cvar95_loss: zero,
            mean_regret: zero,
            max_regret: zero,
            states: vec![fmine_decide::card::StateResult {
                state_id: "s0".into(),
                weight: ONE,
                payoff: zero,
                best_in_hindsight: zero,
                regret: zero,
            }],
        }
    );
    let mut p = i.policy.value.clone();
    p.max_tail_loss = Some(usd(-1));
    i.policy = policy_claim(p);
    let report = evaluate(&i).unwrap();
    assert!(report.summary().feasible_winners.is_empty());
    assert!(report.summary().expected_value_cost_of_risk_limit.is_none());
    assert!(
        report
            .to_json()
            .contains("\"status\":\"no_risk_feasible_program\"")
    );
    assert!(report.to_markdown().contains("no risk-feasible program"));
}

#[test]
fn all_candidates_constraints_and_metadata_bind_the_immutable_receipt() {
    let i = inputs(
        &[("A", 1, &[10, 20]), ("loser", 2, &[-100, -200])],
        &[r(1, 2); 2],
        1,
        r(1, 5),
        None,
    );
    let report = evaluate(&i).unwrap();
    assert_eq!(report.claim().parents.len(), 4);
    assert!(report.claim().parents.contains(&i.candidates[1].claim_id));
    assert_eq!(report.claim().evidence, EvidenceClass::Assumed);
    assert_eq!(report.claim().confidentiality, Confidentiality::internal());
    let mut changed = i.clone();
    let mut c = changed.candidates[1].value.clone();
    c.reference_class_basis.push_str("; revised basis");
    changed.candidates[1] = candidate_claim(c);
    let next = evaluate(&changed).unwrap();
    assert_eq!(selected_winners(&report), selected_winners(&next));
    assert_ne!(report.digest(), next.digest());
    let mut changed = i.clone();
    for c in &mut changed.candidates {
        c.value.payoffs.reverse();
    }
    changed.candidates.reverse();
    changed.weights.value.states.reverse();
    assert_eq!(evaluate(&changed).unwrap(), report);
    let mut changed = i.clone();
    // These fields are not all hashed into the generic Claim id, but must bind the capital report.
    for c in &mut changed.candidates {
        c.uu_terms[0].downside_exposure_usd_m = f64::from_bits(25.0_f64.to_bits() + 1);
    }
    changed.policy.uu_terms[0].downside_exposure_usd_m = f64::from_bits(25.0_f64.to_bits() + 1);
    changed.weights.uu_terms[0].downside_exposure_usd_m = f64::from_bits(25.0_f64.to_bits() + 1);
    assert_eq!(i.candidates[0].claim_id, changed.candidates[0].claim_id);
    assert_ne!(evaluate(&changed).unwrap().digest(), report.digest());
    assert!(report.to_json().contains("canonical_body_hex"));
    assert!(report.to_json().contains("\"selection_frequency\":null"));
}

#[test]
fn rejected_candidates_still_require_valid_values_evidence_and_acyclic_constraints() {
    let i = inputs(&[("A", 1, &[10]), ("B", 2, &[20])], &[ONE], 1, ONE, None);
    let mut changed = i.clone();
    changed.candidates[1].value.cost = usd(3);
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-507");
    let mut changed = i.clone();
    let mut c = changed.candidates[1].value.clone();
    c.payoffs.clear();
    changed.candidates[1] = candidate_claim(c);
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-201");
    let mut changed = i.clone();
    changed.candidates[1].stale = true;
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-502");
    let mut changed = i.clone();
    changed.candidates[1].uu_terms.clear();
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-204");
    let mut changed = i.clone();
    for (n, required) in ["B", "A"].iter().enumerate() {
        let mut c = changed.candidates[n].value.clone();
        c.requires = vec![(*required).into()];
        changed.candidates[n] = candidate_claim(c);
    }
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-105");
    let mut changed = i.clone();
    let mut c = changed.candidates[1].value.clone();
    c.cost.ccy = Currency::CAD;
    changed.candidates[1] = candidate_claim(c);
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-105");
    let mut changed = i.clone();
    for c in &mut changed.candidates {
        let mut value = c.value.clone();
        value.payoffs[0].1.minor = i128::MAX;
        *c = candidate_claim(value);
    }
    let mut p = changed.policy.value.clone();
    p.budget = usd(3);
    changed.policy = policy_claim(p);
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-207");
}

const MODEL: &str = r#"schema = "fmine.capital-program.v1"
model_key = "example"
question = "Fund a declared candidate?"
currency = "USD"
valuation_basis = "synthetic net USD NAV on a fixed ownership basis"
budget = "1.0001"
tail_mass = "1/5"
payoff_convention = "additive_net_delta_nav"
reference_class_basis = "Unadjusted synthetic assumption"
[[state]]
id = "s"
weight = "1/1"
[[candidate]]
id = "A"
label = "Candidate A"
cost = "1.0001"
reference_class_basis = "Unadjusted synthetic assumption"
[[payoff]]
candidate = "A"
state = "s"
value = "2.0001"
[[uu]]
id = "model-risk"
description = "Missing joint shocks"
trigger = "Observation outside the model"
downside_usd_m = "1.25"
"#;
fn parse(text: &str) -> Result<ProgramInputs> {
    model::parse(
        text,
        SourceAnchor::new(
            "synthetic/model.toml",
            sha256(text.as_bytes()).0,
            "complete declared model",
        ),
    )
}

#[test]
fn parser_is_exact_source_bound_and_cannot_upgrade_assumptions() {
    let report = evaluate(&parse(MODEL).unwrap()).unwrap();
    let chosen = row(&report, &["A"]);
    assert_eq!(chosen.spend.major, r(10001, 10000));
    assert_eq!(
        chosen.statistics.as_ref().unwrap().mean.major,
        r(20001, 10000)
    );
    assert_eq!(chosen.budget_slack.major, ZERO);
    let comment = format!("{MODEL}\n# revised source commentary\n");
    let next = evaluate(&parse(&comment).unwrap()).unwrap();
    assert_eq!(next.summary(), report.summary());
    assert_ne!(next.digest(), report.digest());
    assert_eq!(
        model::parse(MODEL, SourceAnchor::new("model", [1; 32], "all"))
            .unwrap_err()
            .code,
        "FMINE-REF-505"
    );
    for text in [
        MODEL.replace("cost = \"1.0001\"", "cost = \"1.00001\""),
        MODEL.replace("currency = \"USD\"", "currency = \"CAD\""),
        MODEL.replace("tail_mass = \"1/5\"", "tail_mass = \"0/1\""),
        MODEL.replace("question =", "calibrated = \"true\"\nquestion ="),
        MODEL.replace("value = \"2.0001\"", "value = \"NaN\""),
        MODEL.replace("Unadjusted synthetic assumption", ""),
    ] {
        assert!(
            parse(&text).is_err(),
            "malformed source was admitted: {text}"
        );
    }
    assert!(report.to_json().contains("\"evidence\":\"assumed\""));
    assert!(report.to_markdown().contains("SHA-256"));
}

#[test]
fn oversized_identifiers_text_and_anchors_refuse_before_enumeration() {
    let i = inputs(&[("A", 1, &[10])], &[ONE], 1, ONE, None);
    let mut changed = i.clone();
    let mut c = changed.candidates[0].value.clone();
    c.candidate_id = "x".repeat(MAX_IDENTIFIER_BYTES + 1);
    changed.candidates[0] = candidate_claim(c);
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-105");
    let mut changed = i.clone();
    let mut c = changed.candidates[0].value.clone();
    c.label = "x".repeat(MAX_TEXT_BYTES + 1);
    changed.candidates[0] = candidate_claim(c);
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-105");
    let oversized_id = MODEL.replace(
        "id = \"s\"",
        &format!("id = \"{}\"", "x".repeat(MAX_IDENTIFIER_BYTES + 1)),
    );
    assert_eq!(parse(&oversized_id).unwrap_err().code, "FMINE-REF-506");
    let oversized_text = MODEL.replace(
        "Fund a declared candidate?",
        &"x".repeat(MAX_TEXT_BYTES + 1),
    );
    assert_eq!(parse(&oversized_text).unwrap_err().code, "FMINE-REF-506");
    for source in [
        SourceAnchor::new(
            "x".repeat(MAX_TEXT_BYTES + 1),
            sha256(MODEL.as_bytes()).0,
            "all",
        ),
        SourceAnchor::new(
            "model",
            sha256(MODEL.as_bytes()).0,
            "x".repeat(MAX_TEXT_BYTES + 1),
        ),
    ] {
        assert_eq!(
            model::parse(MODEL, source).unwrap_err().code,
            "FMINE-REF-506"
        );
    }
}

#[test]
fn scenario_overrun_changes_the_fundable_choice_without_charging_nav_twice() {
    let i = inputs(
        &[("A", 100, &[50, 100]), ("B", 100, &[30, 35])],
        &[r(1, 2), r(1, 2)],
        100,
        r(1, 5),
        None,
    );
    assert_eq!(selected_winners(&evaluate(&i).unwrap()), vec![vec!["A"]]);
    let funded = with_funding(
        i,
        &[150, 150],
        &[("A", &[100, 150]), ("B", &[100, 100])],
        50,
    );
    let report = evaluate(&funded).unwrap();
    assert_eq!(selected_winners(&report), vec![vec!["B"]]);
    assert!(row(&report, &["A"]).statistics.is_none());
    let b = row(&report, &["B"]);
    assert_eq!(b.statistics.as_ref().unwrap().mean.major, r(65, 2));
    assert_eq!(b.funding.as_ref().unwrap().minimum_slack.major, ZERO);
    let both = row(&report, &["A", "B"]);
    assert!(
        both.structural_rejections
            .iter()
            .any(|r| matches!(r, StructuralRejection::BudgetExceeded))
    );
    assert!(both.structural_rejections.iter().any(|r| matches!(r, StructuralRejection::LiquidityShortfall { state_id, shortfall } if state_id == "s1" && shortfall.major == Rat::from_integer(150))));
    let adverse = &both.funding.as_ref().unwrap().states[1];
    assert_eq!(adverse.cash_required.major, Rat::from_integer(250));
    assert_eq!(adverse.remaining_cash.major, Rat::from_integer(-100));
    assert_eq!(adverse.slack.major, Rat::from_integer(-150));
    assert_eq!(report.claim().parents.len(), 5);
    assert!(
        report
            .claim()
            .parents
            .contains(&funded.funding.as_ref().unwrap().claim_id)
    );
    assert!(
        report
            .to_markdown()
            .contains("Funding over the declared cash window")
    );
    assert!(report.to_markdown().contains("cash-floor shortfall in s1"));
}

#[test]
fn cash_demands_follow_shared_states_and_never_average_away_a_shortfall() {
    let i = inputs(
        &[("A", 1, &[5, 5]), ("B", 1, &[7, 7])],
        &[r(1, 100), r(99, 100)],
        2,
        ONE,
        None,
    );
    let offset = with_funding(
        i.clone(),
        &[150, 150],
        &[("A", &[100, 0]), ("B", &[0, 100])],
        50,
    );
    let offset_report = evaluate(&offset).unwrap();
    assert_eq!(selected_winners(&offset_report), vec![vec!["A", "B"]]);
    assert_eq!(
        row(&offset_report, &["A", "B"])
            .funding
            .as_ref()
            .unwrap()
            .minimum_slack
            .major,
        ZERO
    );
    let correlated = with_funding(i, &[150, 150], &[("A", &[100, 0]), ("B", &[100, 0])], 50);
    let report = evaluate(&correlated).unwrap();
    assert_eq!(selected_winners(&report), vec![vec!["B"]]);
    let both = row(&report, &["A", "B"]);
    assert!(both.structural_rejections.iter().any(|r| matches!(r, StructuralRejection::LiquidityShortfall { state_id, shortfall } if state_id == "s0" && shortfall.major == Rat::from_integer(100))));
    let mut reordered = correlated;
    reordered.candidates.reverse();
    reordered.weights.value.states.reverse();
    let funding = &mut reordered.funding.as_mut().unwrap().value;
    funding.available_cash.reverse();
    funding.requirements.reverse();
    assert_eq!(evaluate(&reordered).unwrap(), report);
}

#[test]
fn hold_cannot_be_forced_feasible_when_preexisting_cash_is_below_the_floor() {
    let i = with_funding(
        inputs(&[("A", 1, &[100, 100])], &[r(1, 2), r(1, 2)], 1, ONE, None),
        &[40, 100],
        &[("A", &[0, 10])],
        50,
    );
    let report = evaluate(&i).unwrap();
    assert!(report.summary().unconstrained_winners.is_empty());
    assert!(report.summary().feasible_winners.is_empty());
    assert!(report.summary().expected_value_cost_of_risk_limit.is_none());
    for choice in &report.summary().choices {
        assert!(choice.statistics.is_none());
        assert!(choice.tail_loss.is_none());
        assert!(!choice.risk_rejected);
        assert_eq!(
            choice.funding.as_ref().unwrap().minimum_slack.major,
            Rat::from_integer(-10)
        );
        assert!(choice.structural_rejections.iter().any(|r| matches!(r, StructuralRejection::LiquidityShortfall { state_id, shortfall } if state_id == "s0" && shortfall.major == Rat::from_integer(10))));
    }
    assert_eq!(
        row(&report, &[]).funding.as_ref().unwrap().states[0]
            .remaining_cash
            .major,
        Rat::from_integer(40)
    );
    assert!(
        report
            .to_json()
            .contains("\"status\":\"no_structurally_feasible_program\"")
    );
    assert!(
        report
            .to_markdown()
            .contains("none: no structurally feasible program")
    );
}

#[test]
fn complete_funding_values_and_unhashed_metadata_bind_even_without_a_selection_change() {
    let i = inputs(
        &[("A", 1, &[10]), ("loser", 10, &[-10])],
        &[ONE],
        1,
        ONE,
        None,
    );
    let legacy = evaluate(&i).unwrap();
    let funded = with_funding(i, &[100], &[("A", &[1]), ("loser", &[2])], 10);
    let report = evaluate(&funded).unwrap();
    assert_eq!(selected_winners(&report), selected_winners(&legacy));
    for before in &legacy.summary().choices {
        let after = report
            .summary()
            .choices
            .iter()
            .find(|r| r.selected == before.selected)
            .unwrap();
        assert_eq!(after.statistics, before.statistics);
        assert_eq!(after.tail_loss, before.tail_loss);
    }
    let mut changed = funded.clone();
    let mut f = changed.funding.as_ref().unwrap().value.clone();
    f.requirements
        .iter_mut()
        .find(|r| r.candidate_id == "loser")
        .unwrap()
        .value = usd(3);
    changed.funding = Some(funding_claim(f));
    let next = evaluate(&changed).unwrap();
    assert_eq!(selected_winners(&next), selected_winners(&report));
    assert_ne!(next.digest(), report.digest());

    let mut changed = funded.clone();
    changed.funding.as_mut().unwrap().supersedes = Some(changed.policy.claim_id);
    assert_eq!(
        changed.funding.as_ref().unwrap().claim_id,
        funded.funding.as_ref().unwrap().claim_id
    );
    let next = evaluate(&changed).unwrap();
    assert_eq!(next.summary(), report.summary());
    assert_ne!(next.digest(), report.digest());
    let mut changed = funded;
    changed.funding.as_mut().unwrap().value.minimum_cash = usd(11);
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-507");
}

#[test]
fn incomplete_foreign_invalid_period_currency_and_overflowing_funding_refuse() {
    let i = with_funding(
        inputs(&[("A", 1, &[10]), ("B", 1, &[20])], &[ONE], 2, ONE, None),
        &[100],
        &[("A", &[10]), ("B", &[20])],
        5,
    );
    for case in 0..12 {
        let mut changed = i.clone();
        let mut f = changed.funding.as_ref().unwrap().value.clone();
        match case {
            0 => {
                f.requirements.pop();
            }
            1 => f.requirements[1] = f.requirements[0].clone(),
            2 => f.requirements[0].candidate_id = "foreign".into(),
            3 => f.available_cash[0].0 = "foreign".into(),
            4 => f.minimum_cash = usd(-1),
            5 => f.available_cash[0].1 = usd(-1),
            6 => f.requirements[0].value = usd(-1),
            7 => f.requirements[0].value.ccy = Currency::CAD,
            8 => f.cash_period = "2030-02-29/2030-12-31".into(),
            9 => f.cash_period = "2030-12-31/2030-01-01".into(),
            10 => f.cash_basis.clear(),
            11 => f.available_cash[0].1.ccy = Currency::CAD,
            _ => unreachable!(),
        }
        changed.funding = Some(funding_claim(f));
        assert!(
            evaluate(&changed).is_err(),
            "funding case {case} was admitted"
        );
    }
    let mut changed = i.clone();
    changed.funding.as_mut().unwrap().stale = true;
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-502");
    let mut changed = i.clone();
    changed.funding.as_mut().unwrap().uu_terms.clear();
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-204");
    let mut changed = i;
    let mut f = changed.funding.as_ref().unwrap().value.clone();
    f.minimum_cash = usd(0);
    f.available_cash[0].1.minor = i128::MAX;
    for required in &mut f.requirements {
        required.value.minor = i128::MAX;
    }
    changed.funding = Some(funding_claim(f));
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-207");
}

const FUNDING: &str = r#"[[funding]]
cash_period = "2030-01-01/2030-12-31"
cash_basis = "Nominal USD cash over the stated window, full ownership"
minimum_cash = "1"
[[available_cash]]
state = "s"
value = "3.0001"
[[cash_requirement]]
candidate = "A"
state = "s"
value = "2.0001"
"#;

#[test]
fn funding_parser_keeps_exact_cash_scope_and_refuses_partial_declarations() {
    let text = format!("{MODEL}{FUNDING}");
    let report = evaluate(&parse(&text).unwrap()).unwrap();
    let chosen = row(&report, &["A"]);
    let cash = &chosen.funding.as_ref().unwrap().states[0];
    assert_eq!(cash.cash_required.major, r(20001, 10000));
    assert_eq!(cash.remaining_cash.major, ONE);
    assert_eq!(cash.slack.major, ZERO);
    assert_eq!(
        chosen.statistics.as_ref().unwrap().mean.major,
        r(20001, 10000)
    );
    assert!(
        report
            .to_json()
            .contains("\"minimum_cash_currency\":\"USD\"")
    );
    for bad in [
        text.replace("3.0001", "3.00001"),
        text.replace(
            "[[available_cash]]\nstate = \"s\"\nvalue = \"3.0001\"\n",
            "",
        ),
        text.replace(
            "[[cash_requirement]]\ncandidate = \"A\"\nstate = \"s\"\nvalue = \"2.0001\"\n",
            "",
        ),
        format!("{text}[[available_cash]]\nstate = \"s\"\nvalue = \"0\"\n"),
        format!("{text}{FUNDING}"),
        format!("{MODEL}[[available_cash]]\nstate = \"s\"\nvalue = \"1\"\n"),
        text.replace(
            "minimum_cash = \"1\"",
            "minimum_cash = \"1\"\nexecution_authorized = \"true\"",
        ),
    ] {
        assert!(
            parse(&bad).is_err(),
            "partial or altered funding declaration was admitted"
        );
    }
    let changed_basis = text.replace("full ownership", "attributable ownership");
    let next = evaluate(&parse(&changed_basis).unwrap()).unwrap();
    assert_eq!(selected_winners(&next), selected_winners(&report));
    assert_ne!(next.digest(), report.digest());
}

#[test]
fn cash_timing_changes_the_winner_without_charging_net_nav_twice() {
    let i = inputs(&[("A", 1, &[80]), ("B", 1, &[50])], &[ONE], 1, ONE, None);
    let aggregate = with_funding(i.clone(), &[200], &[("A", &[110]), ("B", &[90])], 20);
    assert_eq!(
        selected_winners(&evaluate(&aggregate).unwrap()),
        vec![vec!["A"]]
    );
    let mut timed = with_schedule(i);
    let mut schedule = timed.funding_schedule.as_ref().unwrap().value.clone();
    set_baseline_flow(&mut schedule, "finish", "s0", 100);
    set_candidate_flow(&mut schedule, "start", "A", "s0", -100);
    set_candidate_flow(&mut schedule, "finish", "A", "s0", -10);
    set_candidate_flow(&mut schedule, "start", "B", "s0", -70);
    set_candidate_flow(&mut schedule, "finish", "B", "s0", -20);
    timed.funding_schedule = Some(schedule_claim(schedule));
    let report = evaluate(&timed).unwrap();
    assert_eq!(selected_winners(&report), vec![vec!["B"]]);
    assert_eq!(report.summary().choices.len(), 4);
    let a = row(&report, &["A"]);
    assert_eq!(checkpoint(a, Some("start"), "s0").closing_cash.major, ZERO);
    assert_eq!(
        checkpoint(a, Some("finish"), "s0").closing_cash.major,
        r(90, 1)
    );
    assert_eq!(a.structural_rejections.len(), 1);
    assert!(
        matches!(&a.structural_rejections[0], StructuralRejection::CashScheduleShortfall {
        period_id, checkpoint_date, state_id, shortfall
    } if period_id.as_deref() == Some("start") && checkpoint_date == "2030-06-30"
        && state_id == "s0" && shortfall.major == r(20, 1))
    );
    let b = row(&report, &["B"]);
    assert_eq!(
        checkpoint(b, Some("finish"), "s0").opening_cash.major,
        r(30, 1)
    );
    assert_eq!(
        checkpoint(b, Some("finish"), "s0").closing_cash.major,
        r(110, 1)
    );
    assert_eq!(b.statistics.as_ref().unwrap().mean.major, r(50, 1));
    let both = row(&report, &["A", "B"]);
    assert!(
        both.structural_rejections
            .contains(&StructuralRejection::BudgetExceeded)
    );
    assert_eq!(both.funding_schedule.as_ref().unwrap().checkpoints.len(), 3);
    assert_eq!(
        checkpoint(both, Some("start"), "s0").closing_cash.major,
        r(-70, 1)
    );
    assert_eq!(
        checkpoint(both, Some("finish"), "s0").closing_cash.major,
        ZERO
    );
    assert_eq!(both.structural_rejections.len(), 3);
}

#[test]
fn a_single_financing_candidate_can_survive_when_hold_is_infeasible() {
    let mut i = with_schedule(inputs(
        &[("finance", 0, &[-5, -15])],
        &[r(3, 4), r(1, 4)],
        0,
        r(1, 2),
        None,
    ));
    let mut schedule = i.funding_schedule.as_ref().unwrap().value.clone();
    set_baseline_flow(&mut schedule, "start", "s0", -90);
    set_baseline_flow(&mut schedule, "start", "s1", -100);
    set_candidate_flow(&mut schedule, "start", "finance", "s0", 40);
    set_candidate_flow(&mut schedule, "start", "finance", "s1", 50);
    i.funding_schedule = Some(schedule_claim(schedule));
    let report = evaluate(&i).unwrap();
    assert_eq!(selected_winners(&report), vec![vec!["finance"]]);
    assert!(row(&report, &[]).statistics.is_none());
    let chosen = row(&report, &["finance"]);
    let stats = chosen.statistics.as_ref().unwrap();
    assert_eq!(stats.role, ActionRole::Action);
    assert_eq!(stats.mean.major, r(-15, 2));
    assert_eq!(
        stats.p10_p50_p90.map(|m| m.major),
        [r(-15, 1), r(-5, 1), r(-5, 1)]
    );
    assert_eq!(stats.cvar95_loss.major, r(15, 1));
    assert_eq!(chosen.tail_loss.unwrap().major, r(10, 1));
    assert_eq!(stats.mean_regret.major, ZERO);
    assert_eq!(stats.max_regret.major, ZERO);
    for state in &stats.states {
        assert_eq!(state.best_in_hindsight, state.payoff);
        assert_eq!(state.regret.major, ZERO);
        assert_eq!(
            checkpoint(chosen, Some("start"), &state.state_id)
                .closing_cash
                .major,
            r(50, 1)
        );
    }
    let mut policy = i.policy.value.clone();
    policy.max_tail_loss = Some(usd(9));
    i.policy = policy_claim(policy);
    let report = evaluate(&i).unwrap();
    assert_eq!(report.summary().unconstrained_winners.len(), 1);
    assert!(report.summary().feasible_winners.is_empty());
    assert!(report.summary().expected_value_cost_of_risk_limit.is_none());
    assert!(row(&report, &["finance"]).risk_rejected);
}

#[test]
fn financing_alternatives_keep_same_state_regret_and_risk_selection_without_hold() {
    let mut i = inputs(
        &[("A", 0, &[10, 100]), ("B", 0, &[30, 30])],
        &[r(1, 2); 2],
        0,
        r(1, 2),
        Some(-20),
    );
    for candidate in &mut i.candidates {
        let mut value = candidate.value.clone();
        value.exclusion_group = Some("financing".into());
        *candidate = candidate_claim(value);
    }
    i = with_schedule(i);
    let mut schedule = i.funding_schedule.as_ref().unwrap().value.clone();
    for state in ["s0", "s1"] {
        set_baseline_flow(&mut schedule, "start", state, -100);
        for candidate in ["A", "B"] {
            set_candidate_flow(&mut schedule, "start", candidate, state, 50);
        }
    }
    i.funding_schedule = Some(schedule_claim(schedule));
    let report = evaluate(&i).unwrap();
    assert!(row(&report, &[]).statistics.is_none());
    assert!(row(&report, &["A", "B"]).statistics.is_none());
    let a = row(&report, &["A"]);
    let b = row(&report, &["B"]);
    assert_eq!(
        report.summary().unconstrained_winners,
        vec![a.bundle_id.clone()]
    );
    assert_eq!(selected_winners(&report), vec![vec!["B"]]);
    assert_eq!(a.statistics.as_ref().unwrap().mean.major, r(55, 1));
    assert_eq!(a.statistics.as_ref().unwrap().mean_regret.major, r(10, 1));
    assert_eq!(a.statistics.as_ref().unwrap().max_regret.major, r(20, 1));
    assert_eq!(b.statistics.as_ref().unwrap().mean_regret.major, r(35, 1));
    assert_eq!(b.statistics.as_ref().unwrap().max_regret.major, r(70, 1));
    assert_eq!(a.tail_loss.unwrap().major, r(-10, 1));
    assert_eq!(b.tail_loss.unwrap().major, r(-30, 1));
    assert!(a.risk_rejected);
    assert!(!b.risk_rejected);
    assert_eq!(
        report
            .summary()
            .expected_value_cost_of_risk_limit
            .unwrap()
            .major,
        r(25, 1)
    );
}

#[test]
fn receipts_carry_forward_but_never_erase_opening_or_earlier_breaches() {
    let mut i = with_schedule(inputs(&[("A", 1, &[10])], &[ONE], 1, ONE, None));
    let mut schedule = i.funding_schedule.as_ref().unwrap().value.clone();
    set_candidate_flow(&mut schedule, "start", "A", "s0", 60);
    set_candidate_flow(&mut schedule, "finish", "A", "s0", -130);
    i.funding_schedule = Some(schedule_claim(schedule.clone()));
    let report = evaluate(&i).unwrap();
    assert_eq!(selected_winners(&report), vec![vec!["A"]]);
    let a = row(&report, &["A"]);
    assert_eq!(
        checkpoint(a, Some("finish"), "s0").opening_cash.major,
        r(160, 1)
    );
    assert_eq!(
        checkpoint(a, Some("finish"), "s0").closing_cash.major,
        r(30, 1)
    );

    set_candidate_flow(&mut schedule, "start", "A", "s0", -90);
    set_candidate_flow(&mut schedule, "finish", "A", "s0", 130);
    i.funding_schedule = Some(schedule_claim(schedule.clone()));
    let report = evaluate(&i).unwrap();
    let a = row(&report, &["A"]);
    assert_eq!(a.structural_rejections.len(), 1);
    assert_eq!(
        checkpoint(a, Some("finish"), "s0").closing_cash.major,
        r(140, 1)
    );
    assert_eq!(
        a.funding_schedule.as_ref().unwrap().minimum_slack.major,
        r(-10, 1)
    );

    schedule.opening_cash[0].1 = usd(19);
    set_candidate_flow(&mut schedule, "start", "A", "s0", 1000);
    i.funding_schedule = Some(schedule_claim(schedule));
    let report = evaluate(&i).unwrap();
    assert!(report.summary().feasible_winners.is_empty());
    for choice in &report.summary().choices {
        assert!(choice.statistics.is_none());
        assert!(choice.structural_rejections.iter().any(|rejection| matches!(rejection,
            StructuralRejection::CashScheduleShortfall { period_id: None, checkpoint_date, shortfall, .. }
            if checkpoint_date == "2030-01-01" && shortfall.major == ONE)));
    }
}

#[test]
fn cash_floors_keep_minor_units_and_require_every_shared_state_not_average_cash() {
    let mut i = with_schedule(inputs(
        &[("A", 1, &[10, 10]), ("B", 1, &[8, 8])],
        &[r(1, 100), r(99, 100)],
        2,
        ONE,
        None,
    ));
    let mut schedule = i.funding_schedule.as_ref().unwrap().value.clone();
    set_candidate_flow(&mut schedule, "start", "A", "s0", -80);
    set_candidate_flow(&mut schedule, "start", "B", "s1", -80);
    i.funding_schedule = Some(schedule_claim(schedule.clone()));
    let report = evaluate(&i).unwrap();
    assert_eq!(selected_winners(&report), vec![vec!["A", "B"]]);
    assert_eq!(
        row(&report, &["A", "B"])
            .funding_schedule
            .as_ref()
            .unwrap()
            .minimum_slack
            .major,
        ZERO
    );
    set_candidate_flow(&mut schedule, "start", "B", "s1", 0);
    set_candidate_flow(&mut schedule, "start", "B", "s0", -80);
    i.funding_schedule = Some(schedule_claim(schedule.clone()));
    let report = evaluate(&i).unwrap();
    assert_eq!(selected_winners(&report), vec![vec!["A"]]);
    let both = row(&report, &["A", "B"]);
    assert_eq!(
        both.funding_schedule.as_ref().unwrap().minimum_slack.major,
        r(-80, 1)
    );
    assert_eq!(both.structural_rejections.len(), 2);
    assert!(
        both.structural_rejections
            .iter()
            .all(|rejection| matches!(rejection,
        StructuralRejection::CashScheduleShortfall { state_id, shortfall, .. }
        if state_id == "s0" && shortfall.major == r(80, 1)))
    );

    let mut permuted = i.clone();
    permuted.candidates.reverse();
    permuted.weights.value.states.reverse();
    for candidate in &mut permuted.candidates {
        candidate.value.payoffs.reverse();
    }
    let value = &mut permuted.funding_schedule.as_mut().unwrap().value;
    value.periods.reverse();
    value.opening_cash.reverse();
    value.baseline_flows.reverse();
    value.candidate_flows.reverse();
    assert_eq!(evaluate(&permuted).unwrap(), report);

    schedule.periods[1].minimum_cash.minor += 1;
    i.funding_schedule = Some(schedule_claim(schedule));
    let report = evaluate(&i).unwrap();
    let a = row(&report, &["A"]);
    assert!(
        a.structural_rejections
            .iter()
            .any(|rejection| matches!(rejection,
        StructuralRejection::CashScheduleShortfall { period_id, shortfall, .. }
        if period_id.as_deref() == Some("finish") && shortfall.major == r(1, Money::SCALE)))
    );
    assert_eq!(selected_winners(&report), vec![Vec::<String>::new()]);
}

#[test]
fn cash_schedule_admits_only_complete_bound_state_candidate_period_matrices() {
    let i = with_schedule(inputs(
        &[("A", 1, &[10, 10]), ("loser", 10, &[-10, -10])],
        &[r(1, 2); 2],
        1,
        ONE,
        None,
    ));
    for case in 0..21 {
        let mut changed = i.clone();
        let mut schedule = changed.funding_schedule.as_ref().unwrap().value.clone();
        let expected = match case {
            0 => {
                schedule.opening_cash.pop();
                "FMINE-REF-201"
            }
            1 => {
                schedule.baseline_flows.pop();
                "FMINE-REF-201"
            }
            2 => {
                schedule
                    .candidate_flows
                    .retain(|f| f.candidate_id != "loser");
                "FMINE-REF-201"
            }
            3 => {
                schedule.opening_cash[1] = schedule.opening_cash[0].clone();
                "FMINE-REF-105"
            }
            4 => {
                schedule.baseline_flows[1] = schedule.baseline_flows[0].clone();
                "FMINE-REF-105"
            }
            5 => {
                schedule.candidate_flows[1] = schedule.candidate_flows[0].clone();
                "FMINE-REF-105"
            }
            6 => {
                schedule.opening_cash[0].0 = "foreign".into();
                "FMINE-REF-105"
            }
            7 => {
                schedule.baseline_flows[0].period_id = "foreign".into();
                "FMINE-REF-105"
            }
            8 => {
                schedule.baseline_flows[0].state_id = "foreign".into();
                "FMINE-REF-105"
            }
            9 => {
                schedule.candidate_flows[0].period_id = "foreign".into();
                "FMINE-REF-105"
            }
            10 => {
                schedule.candidate_flows[0].candidate_id = "foreign".into();
                "FMINE-REF-105"
            }
            11 => {
                schedule.candidate_flows[0].state_id = "foreign".into();
                "FMINE-REF-105"
            }
            12 => {
                schedule.opening_cash[0].1 = usd(-1);
                "FMINE-REF-105"
            }
            13 => {
                schedule.periods[0].minimum_cash = usd(-1);
                "FMINE-REF-105"
            }
            14 => {
                schedule.opening_cash[0].1.ccy = Currency::CAD;
                "FMINE-REF-105"
            }
            15 => {
                schedule.baseline_flows[0].value.ccy = Currency::CAD;
                "FMINE-REF-105"
            }
            16 => {
                schedule.candidate_flows[0].value.ccy = Currency::CAD;
                "FMINE-REF-105"
            }
            17 => {
                schedule.cash_basis.clear();
                "FMINE-REF-105"
            }
            18 => {
                schedule.model_key = "foreign".into();
                "FMINE-REF-105"
            }
            19 => {
                schedule.periods[1].id = schedule.periods[0].id.clone();
                "FMINE-REF-105"
            }
            20 => {
                schedule.periods = vec![schedule.periods[0].clone(); 17];
                "FMINE-REF-201"
            }
            _ => unreachable!(),
        };
        changed.funding_schedule = Some(schedule_claim(schedule));
        assert_eq!(
            evaluate(&changed).unwrap_err().code,
            expected,
            "schedule case {case}"
        );
    }
    let mut changed = i.clone();
    changed.funding_schedule.as_mut().unwrap().stale = true;
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-502");
    let mut changed = i.clone();
    changed.funding_schedule.as_mut().unwrap().uu_terms.clear();
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-204");
    let mut changed = i.clone();
    changed
        .funding_schedule
        .as_mut()
        .unwrap()
        .value
        .opening_cash[0]
        .1 = usd(99);
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-507");
    let changed = with_funding(i, &[100, 100], &[("A", &[0, 0]), ("loser", &[0, 0])], 20);
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-105");
}

#[test]
fn cash_schedule_calendar_requires_contiguous_valid_inclusive_dates() {
    assert_eq!(next_date("2028-02-28").as_deref(), Some("2028-02-29"));
    assert_eq!(next_date("2028-02-29").as_deref(), Some("2028-03-01"));
    assert_eq!(next_date("2000-02-28").as_deref(), Some("2000-02-29"));
    assert_eq!(next_date("2100-02-28").as_deref(), Some("2100-03-01"));
    assert_eq!(next_date("2030-12-31").as_deref(), Some("2031-01-01"));
    assert_eq!(next_date("9999-12-31"), None);
    assert_eq!(next_date("2030-02-29"), None);
    let i = with_schedule(inputs(&[("A", 1, &[10])], &[ONE], 1, ONE, None));
    for period in [
        "2030-01-01/2030-06-29",  // gap before the next period
        "2030-01-01/2030-07-01",  // overlapping inclusive endpoint
        "2030-06-30/2030-01-01",  // reversed
        "2030-02-29/2030-06-30",  // nonexistent date
        "2030-01-02/2030-06-30",  // missing opening day
        "2030-01-01/2030-06-30/", // extra date syntax
    ] {
        let mut changed = i.clone();
        let mut schedule = changed.funding_schedule.as_ref().unwrap().value.clone();
        schedule.periods[0].cash_period = period.into();
        changed.funding_schedule = Some(schedule_claim(schedule));
        assert_eq!(
            evaluate(&changed).unwrap_err().code,
            "FMINE-REF-107",
            "{period}"
        );
    }
    let mut changed = i.clone();
    let mut schedule = changed.funding_schedule.as_ref().unwrap().value.clone();
    schedule.opening_date = "2030-02-29".into();
    changed.funding_schedule = Some(schedule_claim(schedule));
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-107");
    let mut leap = i;
    let mut schedule = leap.funding_schedule.as_ref().unwrap().value.clone();
    schedule.opening_date = "2028-02-28".into();
    schedule.periods[0].cash_period = "2028-02-28/2028-02-29".into();
    schedule.periods[1].cash_period = "2028-03-01/2028-12-31".into();
    leap.funding_schedule = Some(schedule_claim(schedule));
    assert!(evaluate(&leap).is_ok());
}

#[test]
fn every_schedule_value_and_claim_metadata_binds_even_when_economics_do_not_change() {
    let i = inputs(
        &[("A", 1, &[10]), ("loser", 10, &[-10])],
        &[ONE],
        1,
        ONE,
        None,
    );
    let legacy = evaluate(&i).unwrap();
    let scheduled = with_schedule(i);
    let report = evaluate(&scheduled).unwrap();
    assert_eq!(legacy.no_claim(), NO_CLAIM);
    assert_eq!(report.no_claim(), CASH_SCHEDULE_NO_CLAIM);
    assert!(legacy.to_json().contains(NO_CLAIM));
    assert!(legacy.to_markdown().contains(NO_CLAIM));
    assert!(report.to_json().contains(CASH_SCHEDULE_NO_CLAIM));
    assert!(report.to_markdown().contains(CASH_SCHEDULE_NO_CLAIM));
    assert_eq!(selected_winners(&report), selected_winners(&legacy));
    for before in &legacy.summary().choices {
        let after = report
            .summary()
            .choices
            .iter()
            .find(|r| r.selected == before.selected)
            .unwrap();
        assert_eq!(after.statistics, before.statistics);
        assert_eq!(after.tail_loss, before.tail_loss);
    }
    let claim = scheduled.funding_schedule.as_ref().unwrap();
    assert!(report.claim().parents.contains(&claim.claim_id));
    assert!(
        claim
            .anchors
            .iter()
            .all(|a| report.claim().anchors.contains(a))
    );
    let mut changed = scheduled.clone();
    let mut schedule = claim.value.clone();
    set_candidate_flow(&mut schedule, "finish", "loser", "s0", 1);
    changed.funding_schedule = Some(schedule_claim(schedule));
    let next = evaluate(&changed).unwrap();
    assert_eq!(selected_winners(&next), selected_winners(&report));
    assert_ne!(next.digest(), report.digest());
    let mut changed = scheduled;
    changed.funding_schedule.as_mut().unwrap().supersedes = Some(changed.policy.claim_id);
    let next = evaluate(&changed).unwrap();
    assert_eq!(next.summary(), report.summary());
    assert_ne!(next.digest(), report.digest());
}

#[test]
fn cash_schedule_overflow_refuses_even_a_budget_rejected_subset() {
    let i = with_schedule(inputs(
        &[("A", 1, &[10]), ("B", 1, &[20])],
        &[ONE],
        1,
        ONE,
        None,
    ));
    for case in 0..3 {
        let mut changed = i.clone();
        let mut schedule = changed.funding_schedule.as_ref().unwrap().value.clone();
        match case {
            0 => schedule.baseline_flows[0].value.minor = i128::MAX,
            1 => {
                schedule.opening_cash[0].1 = usd(0);
                for period in &mut schedule.periods {
                    period.minimum_cash = usd(0);
                }
                for flow in &mut schedule.candidate_flows {
                    if flow.period_id == "start" {
                        flow.value.minor = i128::MAX;
                    }
                }
            }
            2 => {
                schedule.opening_cash[0].1 = usd(0);
                schedule.baseline_flows[0].value.minor = i128::MIN + 1;
            }
            _ => unreachable!(),
        }
        changed.funding_schedule = Some(schedule_claim(schedule));
        assert_eq!(
            evaluate(&changed).unwrap_err().code,
            "FMINE-REF-207",
            "overflow case {case}"
        );
    }
}

#[test]
fn combined_cash_checkpoint_bound_is_admitted_exactly_without_truncation() {
    let ids = ["A", "B", "C", "D", "E", "F", "G", "H"];
    for state_count in [128, 129] {
        let values = vec![1; state_count];
        let rows: Vec<_> = ids.iter().map(|id| (*id, 1, values.as_slice())).collect();
        let mut i = with_schedule(inputs(
            &rows,
            &vec![r(1, state_count as i128); state_count],
            8,
            ONE,
            None,
        ));
        let mut schedule = i.funding_schedule.as_ref().unwrap().value.clone();
        schedule.periods.pop();
        schedule.baseline_flows.retain(|f| f.period_id == "start");
        schedule.candidate_flows.retain(|f| f.period_id == "start");
        i.funding_schedule = Some(schedule_claim(schedule));
        assert_eq!(MAX_CASH_CHECKPOINTS, 256 * 128 * 2);
        if state_count == 128 {
            assert!(validate(&i).is_ok());
        } else {
            // Admission must reject work volume before checking even a tampered schedule Claim.
            i.funding_schedule.as_mut().unwrap().value.cash_basis =
                "changed without rehashing".into();
            assert_eq!(evaluate(&i).unwrap_err().code, "FMINE-REF-201");
        }
    }
}

#[test]
fn buyback_accretion_can_win_over_a_project_despite_negative_total_nav_change() {
    let mut i = with_equity(
        inputs(
            &[("buyback", 100, &[-100]), ("project", 100, &[100])],
            &[ONE],
            100,
            ONE,
            None,
        ),
        &[1000],
        r(100, 1),
    );
    let mut equity = i.equity.as_ref().unwrap().value.clone();
    // Synthetic hand answer: retire C/P = 100/5 = 20 shares, and remove the $100 cash from NAV.
    set_share_change(&mut equity, "buyback", "s0", r(-20, 1));
    i.equity = Some(equity_claim(equity));
    let report = evaluate(&i).unwrap();
    assert_eq!(selected_winners(&report), vec![vec!["buyback"]]);
    let buyback = row(&report, &["buyback"]);
    let state = equity_state(buyback, "s0");
    assert_eq!(state.baseline_nav.major, r(1000, 1));
    assert_eq!(state.nav_change.major, r(-100, 1));
    assert_eq!(state.ending_nav.major, r(900, 1));
    assert_eq!(state.baseline_shares, r(100, 1));
    assert_eq!(state.share_change, r(-20, 1));
    assert_eq!(state.ending_shares, r(80, 1));
    assert_eq!(state.baseline_nav_per_share.major, r(10, 1));
    assert_eq!(state.ending_nav_per_share.unwrap().major, r(45, 4));
    assert_eq!(state.delta_nav_per_share.unwrap().major, r(5, 4));
    assert_eq!(buyback.statistics.as_ref().unwrap().mean.major, r(-100, 1));
    assert_eq!(buyback.tail_loss.unwrap().major, r(100, 1));
    assert_eq!(
        buyback.per_share_statistics.as_ref().unwrap().mean.major,
        r(5, 4)
    );
    assert_eq!(buyback.per_share_tail_loss.unwrap().major, r(-5, 4));
    let project = row(&report, &["project"]);
    assert_eq!(project.statistics.as_ref().unwrap().mean.major, r(100, 1));
    assert_eq!(
        project.per_share_statistics.as_ref().unwrap().mean.major,
        ONE
    );
    assert_eq!(
        project
            .per_share_statistics
            .as_ref()
            .unwrap()
            .mean_regret
            .major,
        r(1, 4)
    );
    let both = row(&report, &["buyback", "project"]);
    assert!(
        both.structural_rejections
            .contains(&StructuralRejection::BudgetExceeded)
    );
    assert_eq!(
        equity_state(both, "s0").delta_nav_per_share.unwrap().major,
        r(5, 2)
    );
    assert!(both.statistics.is_none() && both.per_share_statistics.is_none());
    assert!(report.summary().expected_value_cost_of_risk_limit.is_none());
    assert_eq!(
        report
            .summary()
            .expected_per_share_cost_of_risk_limit
            .unwrap()
            .major,
        ZERO
    );
    assert_eq!(report.no_claim(), EQUITY_NO_CLAIM);
}

#[test]
fn issuance_values_the_dilution_of_existing_equity_and_keeps_negative_nav_valid() {
    let mut i = with_equity(
        inputs(&[("issue", 0, &[200])], &[ONE], 0, ONE, None),
        &[1000],
        r(100, 1),
    );
    let mut equity = i.equity.as_ref().unwrap().value.clone();
    set_share_change(&mut equity, "issue", "s0", r(100, 1));
    i.equity = Some(equity_claim(equity.clone()));
    let report = evaluate(&i).unwrap();
    let issue = row(&report, &["issue"]);
    assert_eq!(selected_winners(&report), vec![Vec::<String>::new()]);
    assert_eq!(issue.statistics.as_ref().unwrap().mean.major, r(200, 1));
    assert_eq!(
        issue.per_share_statistics.as_ref().unwrap().mean.major,
        r(-4, 1)
    );
    assert_eq!(
        equity_state(issue, "s0")
            .ending_nav_per_share
            .unwrap()
            .major,
        r(6, 1)
    );

    let mut candidate = i.candidates[0].value.clone();
    candidate.payoffs[0].1 = usd(1200);
    i.candidates[0] = candidate_claim(candidate);
    let report = evaluate(&i).unwrap();
    assert_eq!(selected_winners(&report), vec![vec!["issue"]]);
    assert_eq!(
        row(&report, &["issue"])
            .per_share_statistics
            .as_ref()
            .unwrap()
            .mean
            .major,
        ONE
    );

    equity.baseline_nav[0].1 = usd(-1000);
    i.equity = Some(equity_claim(equity));
    let report = evaluate(&i).unwrap();
    assert_eq!(
        equity_state(row(&report, &[]), "s0")
            .baseline_nav_per_share
            .major,
        r(-10, 1)
    );
    assert_eq!(
        row(&report, &["issue"])
            .per_share_statistics
            .as_ref()
            .unwrap()
            .mean
            .major,
        r(11, 1)
    );
}

#[test]
fn per_share_values_remain_rational_and_invariant_to_declared_share_units() {
    let mut i = with_equity(
        inputs(&[("A", 1, &[1, -1])], &[r(1, 3), r(2, 3)], 1, r(1, 2), None),
        &[7, 11],
        r(3, 1),
    );
    let mut equity = i.equity.as_ref().unwrap().value.clone();
    set_share_change(&mut equity, "A", "s0", r(-1, 1));
    set_share_change(&mut equity, "A", "s1", ONE);
    equity.max_tail_loss_per_share = Some(r(7, 6));
    i.equity = Some(equity_claim(equity.clone()));
    let report = evaluate(&i).unwrap();
    let a = row(&report, &["A"]);
    assert_eq!(
        equity_state(a, "s0").delta_nav_per_share.unwrap().major,
        r(5, 3)
    );
    assert_eq!(
        equity_state(a, "s1").delta_nav_per_share.unwrap().major,
        r(-7, 6)
    );
    assert_eq!(
        a.per_share_statistics.as_ref().unwrap().mean.major,
        r(-2, 9)
    );
    assert_eq!(a.per_share_tail_loss.unwrap().major, r(7, 6));
    assert!(!a.risk_rejected);
    equity.shares_per_unit = r(1_000_000, 1);
    equity.initial_share_units = r(3, 1_000_000);
    set_share_change(&mut equity, "A", "s0", r(-1, 1_000_000));
    set_share_change(&mut equity, "A", "s1", r(1, 1_000_000));
    let mut millions = i.clone();
    millions.equity = Some(equity_claim(equity.clone()));
    let normalized = evaluate(&millions).unwrap();
    assert_eq!(report.summary().choices, normalized.summary().choices);
    assert_eq!(
        report.summary().feasible_winners,
        normalized.summary().feasible_winners
    );
    assert_ne!(report.digest(), normalized.digest());
    equity.max_tail_loss_per_share = Some(r(7, 6).checked_sub(r(1, 1_000_000)).unwrap());
    millions.equity = Some(equity_claim(equity));
    assert!(row(&evaluate(&millions).unwrap(), &["A"]).risk_rejected);
}

#[test]
fn per_share_tail_limit_uses_joint_fractional_mass_and_reports_its_own_risk_cost() {
    let mut i = with_equity(
        inputs(
            &[("risky", 1, &[-4, 8]), ("safe", 1, &[3, 3])],
            &[r(1, 4), r(3, 4)],
            1,
            r(1, 2),
            None,
        ),
        &[100, 100],
        r(10, 1),
    );
    let mut equity = i.equity.as_ref().unwrap().value.clone();
    equity.max_tail_loss_per_share = Some(r(-1, 4));
    i.equity = Some(equity_claim(equity));
    let report = evaluate(&i).unwrap();
    let risky = row(&report, &["risky"]);
    let safe = row(&report, &["safe"]);
    assert_eq!(
        report.summary().unconstrained_winners,
        vec![risky.bundle_id.clone()]
    );
    assert_eq!(selected_winners(&report), vec![vec!["safe"]]);
    let stats = risky.per_share_statistics.as_ref().unwrap();
    assert_eq!(stats.mean.major, r(1, 2));
    assert_eq!(
        stats.p10_p50_p90.map(|m| m.major),
        [r(-2, 5), r(4, 5), r(4, 5)]
    );
    assert_eq!(stats.cvar95_loss.major, r(2, 5));
    assert_eq!(stats.mean_regret.major, r(7, 40));
    assert_eq!(
        safe.per_share_statistics
            .as_ref()
            .unwrap()
            .mean_regret
            .major,
        r(3, 8)
    );
    assert_eq!(risky.per_share_tail_loss.unwrap().major, r(-1, 5));
    assert_eq!(safe.per_share_tail_loss.unwrap().major, r(-3, 10));
    assert_eq!(risky.tail_loss.unwrap().major, r(-2, 1));
    assert_eq!(safe.tail_loss.unwrap().major, r(-3, 1));
    assert!(risky.risk_rejected && row(&report, &[]).risk_rejected);
    assert!(!safe.risk_rejected);
    assert!(report.summary().expected_value_cost_of_risk_limit.is_none());
    assert_eq!(
        report
            .summary()
            .expected_per_share_cost_of_risk_limit
            .unwrap()
            .major,
        r(1, 5)
    );
}

#[test]
fn nonpositive_ending_shares_reject_each_bad_state_without_dropping_the_bundle() {
    let mut i = with_equity(
        inputs(
            &[("good", 1, &[10, 10]), ("retire", 1, &[10000, 10000])],
            &[r(1, 2); 2],
            1,
            ONE,
            None,
        ),
        &[1000, 1000],
        r(100, 1),
    );
    let mut equity = i.equity.as_ref().unwrap().value.clone();
    set_share_change(&mut equity, "retire", "s0", r(-100, 1));
    set_share_change(&mut equity, "retire", "s1", r(-110, 1));
    i.equity = Some(equity_claim(equity));
    let report = evaluate(&i).unwrap();
    assert_eq!(selected_winners(&report), vec![vec!["good"]]);
    assert_eq!(report.summary().choices.len(), 4);
    for members in [&["retire"][..], &["good", "retire"][..]] {
        let choice = row(&report, members);
        assert!(choice.statistics.is_none() && choice.per_share_statistics.is_none());
        assert!(choice.tail_loss.is_none() && choice.per_share_tail_loss.is_none());
        for (state, shares) in [("s0", ZERO), ("s1", r(-10, 1))] {
            let cell = equity_state(choice, state);
            assert_eq!(cell.ending_shares, shares);
            assert!(cell.ending_nav_per_share.is_none() && cell.delta_nav_per_share.is_none());
            assert!(choice.structural_rejections.contains(
                &StructuralRejection::NonpositiveShareCount {
                    state_id: state.into(),
                    shares,
                }
            ));
        }
    }
    assert_eq!(
        row(&report, &["good"])
            .per_share_statistics
            .as_ref()
            .unwrap()
            .max_regret
            .major,
        ZERO
    );
}

#[test]
fn scheduled_issuance_can_be_the_only_feasible_choice_despite_per_share_dilution() {
    let mut i = with_equity(
        with_schedule(inputs(&[("issue", 0, &[10])], &[ONE], 0, ONE, None)),
        &[100],
        r(10, 1),
    );
    let mut schedule = i.funding_schedule.as_ref().unwrap().value.clone();
    set_baseline_flow(&mut schedule, "start", "s0", -120);
    set_candidate_flow(&mut schedule, "start", "issue", "s0", 50);
    i.funding_schedule = Some(schedule_claim(schedule));
    let mut equity = i.equity.as_ref().unwrap().value.clone();
    set_share_change(&mut equity, "issue", "s0", r(10, 1));
    i.equity = Some(equity_claim(equity.clone()));
    let report = evaluate(&i).unwrap();
    assert_eq!(selected_winners(&report), vec![vec!["issue"]]);
    assert!(row(&report, &[]).per_share_statistics.is_none());
    let issue = row(&report, &["issue"]);
    assert_eq!(issue.statistics.as_ref().unwrap().mean.major, r(10, 1));
    let stats = issue.per_share_statistics.as_ref().unwrap();
    assert_eq!(stats.mean.major, r(-9, 2));
    assert_eq!(stats.states[0].best_in_hindsight.major, r(-9, 2));
    assert_eq!(stats.mean_regret.major, ZERO);
    assert_eq!(stats.max_regret.major, ZERO);
    assert_eq!(
        checkpoint(issue, Some("finish"), "s0").closing_cash.major,
        r(30, 1)
    );
    assert_eq!(report.no_claim(), EQUITY_CASH_SCHEDULE_NO_CLAIM);
    equity.max_tail_loss_per_share = Some(r(4, 1));
    i.equity = Some(equity_claim(equity));
    let report = evaluate(&i).unwrap();
    assert_eq!(report.summary().unconstrained_winners.len(), 1);
    assert!(report.summary().feasible_winners.is_empty());
    assert!(
        report
            .summary()
            .expected_per_share_cost_of_risk_limit
            .is_none()
    );
}

#[test]
fn equity_shared_state_results_are_canonical_and_all_evidence_binds() {
    let mut i = with_equity(
        with_schedule(inputs(
            &[("A", 1, &[10, 20]), ("loser", 10, &[-10, -20])],
            &[r(1, 3), r(2, 3)],
            1,
            ONE,
            None,
        )),
        &[100, 200],
        r(10, 1),
    );
    let mut equity = i.equity.as_ref().unwrap().value.clone();
    set_share_change(&mut equity, "A", "s0", r(-1, 1));
    set_share_change(&mut equity, "A", "s1", ONE);
    i.equity = Some(equity_claim(equity));
    let report = evaluate(&i).unwrap();
    let input_claim = i.equity.as_ref().unwrap();
    assert!(report.claim().parents.contains(&input_claim.claim_id));
    assert!(
        input_claim
            .anchors
            .iter()
            .all(|a| report.claim().anchors.contains(a))
    );
    let mut reordered = i.clone();
    reordered.candidates.reverse();
    reordered.weights.value.states.reverse();
    for candidate in &mut reordered.candidates {
        candidate.value.payoffs.reverse();
    }
    let equity = &mut reordered.equity.as_mut().unwrap().value;
    equity.baseline_nav.reverse();
    equity.share_changes.reverse();
    let schedule = &mut reordered.funding_schedule.as_mut().unwrap().value;
    schedule.opening_cash.reverse();
    schedule.periods.reverse();
    schedule.baseline_flows.reverse();
    schedule.candidate_flows.reverse();
    assert_eq!(evaluate(&reordered).unwrap(), report);
    let mut changed = i.clone();
    let mut equity = changed.equity.as_ref().unwrap().value.clone();
    set_share_change(&mut equity, "loser", "s0", r(2, 1));
    changed.equity = Some(equity_claim(equity));
    let next = evaluate(&changed).unwrap();
    assert_eq!(selected_winners(&next), selected_winners(&report));
    assert_ne!(next.digest(), report.digest());
    let mut changed = i;
    changed.equity.as_mut().unwrap().supersedes = Some(changed.policy.claim_id);
    let next = evaluate(&changed).unwrap();
    assert_eq!(next.summary(), report.summary());
    assert_ne!(next.digest(), report.digest());
}

#[test]
fn equity_requires_complete_matrices_explicit_units_and_matching_claims() {
    let i = with_equity(
        inputs(
            &[("A", 1, &[10, 10]), ("loser", 10, &[-10, -10])],
            &[r(1, 2); 2],
            1,
            ONE,
            None,
        ),
        &[100, 100],
        r(10, 1),
    );
    for case in 0..17 {
        let mut changed = i.clone();
        let mut equity = changed.equity.as_ref().unwrap().value.clone();
        let expected = match case {
            0 => {
                equity.baseline_nav.pop();
                "FMINE-REF-201"
            }
            1 => {
                equity.share_changes.retain(|c| c.candidate_id != "loser");
                "FMINE-REF-201"
            }
            2 => {
                equity.baseline_nav[1] = equity.baseline_nav[0].clone();
                "FMINE-REF-105"
            }
            3 => {
                equity.share_changes[1] = equity.share_changes[0].clone();
                "FMINE-REF-105"
            }
            4 => {
                equity.baseline_nav[0].0 = "foreign".into();
                "FMINE-REF-105"
            }
            5 => {
                equity.share_changes[0].state_id = "foreign".into();
                "FMINE-REF-105"
            }
            6 => {
                equity.share_changes[0].candidate_id = "foreign".into();
                "FMINE-REF-105"
            }
            7 => {
                equity.baseline_nav[0].1.ccy = Currency::CAD;
                "FMINE-REF-105"
            }
            8 => {
                equity.currency = Currency::CAD;
                "FMINE-REF-105"
            }
            9 => {
                equity.model_key = "foreign".into();
                "FMINE-REF-105"
            }
            10 => {
                equity.valuation_basis = "foreign".into();
                "FMINE-REF-105"
            }
            11 => {
                equity.entity.clear();
                "FMINE-REF-105"
            }
            12 => {
                equity.share_class.clear();
                "FMINE-REF-105"
            }
            13 => {
                equity.initial_share_units = ZERO;
                "FMINE-REF-109"
            }
            14 => {
                equity.initial_share_units = r(-1, 1);
                "FMINE-REF-109"
            }
            15 => {
                equity.shares_per_unit = ZERO;
                "FMINE-REF-109"
            }
            16 => {
                equity.shares_per_unit = r(-1, 1);
                "FMINE-REF-109"
            }
            _ => unreachable!(),
        };
        changed.equity = Some(equity_claim(equity));
        assert_eq!(
            evaluate(&changed).unwrap_err().code,
            expected,
            "equity case {case}"
        );
    }
    let mut changed = i.clone();
    changed.equity.as_mut().unwrap().stale = true;
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-502");
    let mut changed = i.clone();
    changed.equity.as_mut().unwrap().uu_terms.clear();
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-204");
    let mut changed = i.clone();
    changed.equity.as_mut().unwrap().value.initial_share_units = r(11, 1);
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-507");
    for legacy_cap in [false, true] {
        let mut changed = i.clone();
        let mut policy = changed.policy.value.clone();
        if legacy_cap {
            policy.max_tail_loss = Some(usd(0));
        } else {
            policy.payoff_convention = PAYOFF_CONVENTION.into();
        }
        changed.policy = policy_claim(policy);
        assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-105");
    }
    let mut changed = i;
    changed.equity = None;
    assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-105");
}

#[test]
fn equity_overflow_refuses_even_when_other_constraints_reject_the_bundle() {
    let i = with_equity(
        inputs(&[("A", 1, &[0]), ("B", 1, &[0])], &[ONE], 1, ONE, None),
        &[0],
        ONE,
    );
    for case in 0..4 {
        let mut changed = i.clone();
        let mut equity = changed.equity.as_ref().unwrap().value.clone();
        match case {
            0 => {
                equity.initial_share_units = r(i128::MAX, 1);
                equity.shares_per_unit = r(2, 1);
            }
            1 => {
                for flow in &mut equity.share_changes {
                    flow.units = r(i128::MAX - 1, 1);
                }
            }
            2 => {
                equity.baseline_nav[0].1.minor = i128::MAX;
                let mut candidate = changed.candidates[0].value.clone();
                candidate.cost = usd(2);
                candidate.payoffs[0].1 = usd(1);
                changed.candidates[0] = candidate_claim(candidate);
            }
            3 => {
                equity.baseline_nav[0].1 = usd(1);
                equity.initial_share_units = r(1, i128::MAX);
                let mut candidate = changed.candidates[0].value.clone();
                candidate.cost = usd(2);
                candidate.payoffs[0].1 = usd(1);
                changed.candidates[0] = candidate_claim(candidate);
            }
            _ => unreachable!(),
        }
        changed.equity = Some(equity_claim(equity));
        assert_eq!(
            evaluate(&changed).unwrap_err().code,
            "FMINE-REF-207",
            "equity overflow case {case}"
        );
    }
}

#[test]
fn large_equity_counts_compare_exactly_without_cross_product_overflow() {
    let mut i = with_equity(
        inputs(
            &[("buyback", 240_000_009, &[-240_000_008, -240_000_008])],
            &[r(1, 2); 2],
            300_000_000,
            r(1, 5),
            None,
        ),
        &[30_000_000_000; 2],
        r(3_000_000_007, 1),
    );
    let mut candidate = i.candidates[0].value.clone();
    for (_, payoff) in &mut candidate.payoffs {
        *payoff = Money::from_minor(Currency::USD, -2_400_000_080_200);
    }
    i.candidates[0] = candidate_claim(candidate.clone());
    let mut equity = i.equity.as_ref().unwrap().value.clone();
    for (_, nav) in &mut equity.baseline_nav {
        *nav = Money::from_minor(Currency::USD, 300_000_000_000_100);
    }
    for change in &mut equity.share_changes {
        change.units = r(-30_000_001, 1);
    }
    i.equity = Some(equity_claim(equity.clone()));
    let delta = r(6_000_000_142_009_998_129, 297_000_001_293_000_001_400);
    // All actual results fit; the unnecessary numerator-by-denominator comparison does not.
    assert!(delta.checked_cmp(delta).is_err());
    let report = evaluate(&i).unwrap();
    assert_eq!(selected_winners(&report), vec![vec!["buyback"]]);
    let buyback = row(&report, &["buyback"]);
    let stats = buyback.per_share_statistics.as_ref().unwrap();
    assert_eq!(stats.mean.major, delta);
    assert_eq!(stats.p10_p50_p90.map(|m| m.major), [delta; 3]);
    assert_eq!(stats.cvar95_loss.major, delta.checked_neg().unwrap());
    assert_eq!(
        buyback.per_share_tail_loss.unwrap().major,
        delta.checked_neg().unwrap()
    );
    assert_eq!(stats.mean_regret.major, ZERO);
    assert_eq!(stats.max_regret.major, ZERO);
    for state in &buyback.equity.as_ref().unwrap().states {
        assert_eq!(state.ending_shares, r(2_970_000_006, 1));
        assert_eq!(state.ending_nav.major, r(2_975_999_999_199, 100));
        assert_eq!(
            state.baseline_nav_per_share.major,
            r(3_000_000_000_001, 300_000_000_700)
        );
        assert_eq!(
            state.ending_nav_per_share.unwrap().major,
            r(991_999_999_733, 99_000_000_200)
        );
        assert_eq!(state.delta_nav_per_share.unwrap().major, delta);
    }
    let hold = row(&report, &[]).per_share_statistics.as_ref().unwrap();
    assert_eq!(hold.mean_regret.major, delta);
    assert_eq!(hold.max_regret.major, delta);

    // A distinct share retirement creates a distinct large denominator. It must still rank exactly.
    candidate.candidate_id = "buyback_b".into();
    candidate.label = "Second buyback".into();
    candidate.cost = usd(240_000_017);
    for (_, payoff) in &mut candidate.payoffs {
        *payoff = Money::from_minor(Currency::USD, -2_400_000_160_300);
    }
    i.candidates.push(candidate_claim(candidate));
    for state in ["s0", "s1"] {
        equity.share_changes.push(CandidateShareChange {
            candidate_id: "buyback_b".into(),
            state_id: state.into(),
            units: r(-30_000_002, 1),
        });
    }
    let delta_b = r(2_571_428_717_575_712_683, 127_285_714_797_000_000_500);
    let improvement = r(28_429_999_733, 42_004_285_869_857_143_000);
    assert!(delta.checked_cmp(delta_b).is_err());
    assert_eq!(delta_b.checked_sub(delta).unwrap(), improvement);
    equity.max_tail_loss_per_share = Some(delta_b.checked_neg().unwrap());
    i.equity = Some(equity_claim(equity));
    let report = evaluate(&i).unwrap();
    assert_eq!(selected_winners(&report), vec![vec!["buyback_b"]]);
    let first = row(&report, &["buyback"]);
    let second = row(&report, &["buyback_b"]);
    assert_eq!(
        second.per_share_statistics.as_ref().unwrap().mean.major,
        delta_b
    );
    assert_eq!(
        first
            .per_share_statistics
            .as_ref()
            .unwrap()
            .mean_regret
            .major,
        improvement
    );
    assert_eq!(
        first
            .per_share_statistics
            .as_ref()
            .unwrap()
            .max_regret
            .major,
        improvement
    );
    assert!(first.risk_rejected && row(&report, &[]).risk_rejected);
    assert!(!second.risk_rejected);
    assert!(
        row(&report, &["buyback", "buyback_b"])
            .structural_rejections
            .contains(&StructuralRejection::BudgetExceeded)
    );
}
