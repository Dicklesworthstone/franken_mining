//! Known-answer and admission controls for the declared static capital program.
//! Determinism: Exact
#![cfg(test)]

use super::*;
use fmine_evidence::KuKind;

fn r(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}
fn usd(n: i128) -> Money {
    Money::from_minor(Currency::USD, n.checked_mul(Money::SCALE).unwrap())
}
fn claim<T>(metric: &str, value: T, tag: &str, add: AdditivityClass) -> Claim<T> {
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
fn policy_claim(value: ProgramPolicy) -> Claim<ProgramPolicy> {
    let tag = value.canonical_tag();
    claim("capital.policy", value, &tag, AdditivityClass::NonAdditive)
}
fn candidate_claim(value: CapitalCandidate) -> Claim<CapitalCandidate> {
    let tag = value.canonical_tag();
    claim(
        &format!("capital.{}", value.candidate_id),
        value,
        &tag,
        AdditivityClass::ExtensiveAdditive,
    )
}
fn inputs(
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
    assert_eq!(
        row(&report, &[]).statistics.as_ref().unwrap().mean.major,
        ZERO
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
