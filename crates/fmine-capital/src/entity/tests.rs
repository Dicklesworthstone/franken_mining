//! Independent known answers for ring-fenced cash and declared ownership distributions.
//! Determinism: Exact (hand calculations; tests invoke the complete static and policy evaluators).

use super::*;
use crate::recourse::{
    self, CandidateStage, DecisionStage, RecourseBasis, RecourseInputs, StateObservation,
};
use crate::tests::{
    candidate_claim, claim, equity_claim, inputs, r, usd, with_equity, with_schedule,
};

pub(super) const TWO_PERIODS: &[(&str, &str)] = &[
    ("first", "2030-01-01/2030-06-30"),
    ("second", "2030-07-01/2030-12-31"),
];
pub(super) const THREE_PERIODS: &[(&str, &str)] = &[
    ("first", "2030-01-01/2030-04-30"),
    ("second", "2030-05-01/2030-08-31"),
    ("third", "2030-09-01/2030-12-31"),
];

pub(super) fn funding_claim(value: EntityFunding) -> Claim<EntityFunding> {
    let tag = value.canonical_tag();
    claim(
        "capital.entity-funding",
        value,
        &tag,
        AdditivityClass::NonAdditive,
    )
}

pub(super) fn with_entities(
    mut program: ProgramInputs,
    entities: &[(&str, i128, i128)],
    periods: &[(&str, &str)],
) -> ProgramInputs {
    let mut funding = EntityFunding {
        model_key: program.policy.value.model_key.clone(), currency: Currency::USD,
        cash_basis: "Synthetic nominal USD by legal entity; fixed calls and declared distribution permissions".into(),
        opening_date: "2030-01-01".into(), objective_entity: "parent".into(),
        entities: entities.iter().map(|(id, _, _)| Entity { id: (*id).into(), label: (*id).into() }).collect(),
        periods: periods.iter().map(|(id, dates)| EntityPeriod { id: (*id).into(), cash_period: (*dates).into() }).collect(),
        floors: vec![], opening_cash: vec![], baseline_flows: vec![], candidate_flows: vec![],
        transfers: vec![], distributions: vec![], gates: vec![], debt: None,
    };
    for (entity, opening, floor) in entities {
        for (state, _) in &program.weights.value.states {
            funding.opening_cash.push(EntityOpeningCash {
                entity_id: (*entity).into(),
                state_id: state.clone(),
                value: usd(*opening),
            });
        }
        for (period, _) in periods {
            funding.floors.push(EntityFloor {
                entity_id: (*entity).into(),
                period_id: (*period).into(),
                minimum_cash: usd(*floor),
            });
            for (state, _) in &program.weights.value.states {
                funding.baseline_flows.push(EntityStateCashFlow {
                    entity_id: (*entity).into(),
                    period_id: (*period).into(),
                    state_id: state.clone(),
                    value: usd(0),
                });
                for candidate in &program.candidates {
                    funding.candidate_flows.push(EntityCandidateCashFlow {
                        entity_id: (*entity).into(),
                        period_id: (*period).into(),
                        candidate_id: candidate.value.candidate_id.clone(),
                        state_id: state.clone(),
                        value: usd(0),
                    });
                }
            }
        }
    }
    program.entity_funding = Some(funding_claim(funding));
    program
}

pub(super) fn edit(program: &mut ProgramInputs, change: impl FnOnce(&mut EntityFunding)) {
    let mut value = program.entity_funding.as_ref().unwrap().value.clone();
    change(&mut value);
    program.entity_funding = Some(funding_claim(value));
}

pub(super) fn flow(
    funding: &mut EntityFunding,
    period: &str,
    entity: &str,
    candidate: &str,
    state: &str,
    value: i128,
) {
    funding
        .candidate_flows
        .iter_mut()
        .find(|row| {
            row.period_id == period
                && row.entity_id == entity
                && row.candidate_id == candidate
                && row.state_id == state
        })
        .unwrap()
        .value = usd(value);
}

pub(super) fn baseline(
    funding: &mut EntityFunding,
    period: &str,
    entity: &str,
    state: &str,
    value: i128,
) {
    funding
        .baseline_flows
        .iter_mut()
        .find(|row| row.period_id == period && row.entity_id == entity && row.state_id == state)
        .unwrap()
        .value = usd(value);
}

pub(super) fn distribution(
    funding: &mut EntityFunding,
    child: &str,
    parent: &str,
    share: Rat,
    cap: i128,
) {
    funding.distributions.push(EntityDistribution {
        entity_id: child.into(),
        parent_entity_id: parent.into(),
        parent_share: share,
        basis: "Synthetic fixed share; remainder paid to shareholders outside this graph".into(),
    });
    for period in &funding.periods {
        for opening in funding
            .opening_cash
            .iter()
            .filter(|row| row.entity_id == child)
        {
            funding.gates.push(EntityDistributionGate {
                entity_id: child.into(),
                period_id: period.id.clone(),
                state_id: opening.state_id.clone(),
                allowed: true,
                maximum_distribution: usd(cap),
            });
        }
    }
}

pub(super) fn transfer(
    funding: &mut EntityFunding,
    id: &str,
    from: &str,
    to: &str,
    candidate: Option<&str>,
    amounts: &[i128],
) {
    funding.transfers.push(EntityTransfer {
        id: id.into(),
        from_entity: from.into(),
        to_entity: to.into(),
        period_id: "first".into(),
        candidate_id: candidate.map(str::to_owned),
        amounts: amounts
            .iter()
            .enumerate()
            .map(|(i, amount)| (format!("s{i}"), usd(*amount)))
            .collect(),
        basis: "Explicit fixed capital call; no automatic top-up, netting or dilution".into(),
    });
}

pub(super) fn choice<'a>(report: &'a EvaluatedProgram, selected: &[&str]) -> &'a ProgramChoice {
    report
        .summary()
        .choices
        .iter()
        .find(|row| {
            row.selected
                .iter()
                .map(String::as_str)
                .eq(selected.iter().copied())
        })
        .unwrap()
}

pub(super) fn state<'a>(
    assessment: &'a EntityFundingAssessment,
    id: &str,
) -> &'a EntityFundingState {
    assessment
        .states
        .iter()
        .find(|row| row.state_id == id)
        .unwrap()
}

pub(super) fn cash<'a>(
    state: &'a EntityFundingState,
    entity: &str,
    period: Option<&str>,
    phase: EntityCashPhase,
) -> &'a EntityCashCheckpoint {
    state
        .checkpoints
        .iter()
        .find(|row| {
            row.entity_id == entity && row.period_id.as_deref() == period && row.phase == phase
        })
        .unwrap()
}

pub(super) fn dividend<'a>(
    state: &'a EntityFundingState,
    entity: &str,
    period: &str,
) -> &'a EntityDistributionResult {
    state
        .distributions
        .iter()
        .find(|row| row.entity_id == entity && row.period_id == period)
        .unwrap()
}

pub(super) fn no_op() -> ProgramInputs {
    inputs(&[("unused", 1, &[-1])], &[ONE], 0, r(1, 5), None)
}

#[test]
fn trapped_jv_cash_cannot_fund_the_parents_higher_value_project() {
    let mut i = with_entities(
        with_equity(
            inputs(
                &[("big", 40, &[30]), ("small", 10, &[10])],
                &[ONE],
                40,
                r(1, 5),
                None,
            ),
            &[1000],
            r(100, 1),
        ),
        &[("parent", 30, 20), ("jv", 100, 20)],
        TWO_PERIODS,
    );
    let mut equity = i.equity.as_ref().unwrap().value.clone();
    equity.entity = "parent".into();
    i.equity = Some(equity_claim(equity));
    edit(&mut i, |funding| {
        flow(funding, "first", "parent", "big", "s0", -40);
        flow(funding, "first", "parent", "small", "s0", -10);
    });
    let report = crate::evaluate(&i).unwrap();
    assert_eq!(report.summary().choices.len(), 4);
    let big = choice(&report, &["big"]);
    let small = choice(&report, &["small"]);
    assert_eq!(
        report.summary().feasible_winners,
        vec![small.bundle_id.clone()]
    );
    assert_eq!(small.statistics.as_ref().unwrap().mean.major, r(10, 1));
    assert_eq!(
        small.per_share_statistics.as_ref().unwrap().mean.major,
        r(1, 10)
    );
    assert!(big.statistics.is_none());
    let ledger = state(big.entity_funding.as_ref().unwrap(), "s0");
    assert_eq!(
        cash(ledger, "parent", Some("first"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(-10, 1)
    );
    assert_eq!(
        cash(ledger, "jv", Some("first"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(100, 1)
    );
    // Pooled cash would be 90 against combined floors of 40, concealing the parent's 30 shortfall.
    assert!(
        big.structural_rejections
            .iter()
            .any(|reason| matches!(reason,
                StructuralRejection::EntityCashShortfall { entity_id, state_id, shortfall, .. }
                    if entity_id == "parent" && state_id == "s0" && shortfall.major == r(30, 1)
            ))
    );
}

#[test]
fn fixed_parent_and_partner_calls_are_conserved_and_never_implicitly_topped_up() {
    let mut i = with_entities(
        inputs(
            &[("build", 60, &[50, 50])],
            &[r(1, 2); 2],
            60,
            r(1, 5),
            None,
        ),
        &[("parent", 100, 20), ("partner", 40, 0), ("jv", 20, 20)],
        TWO_PERIODS,
    );
    edit(&mut i, |funding| {
        transfer(
            funding,
            "parent_call",
            "parent",
            "jv",
            Some("build"),
            &[60, 60],
        );
        transfer(
            funding,
            "partner_call",
            "partner",
            "jv",
            Some("build"),
            &[40, 40],
        );
        for state in ["s0", "s1"] {
            flow(funding, "second", "jv", "build", state, -100);
        }
    });
    let report = crate::evaluate(&i).unwrap();
    let build = choice(&report, &["build"]);
    assert_eq!(
        report.summary().feasible_winners,
        vec![build.bundle_id.clone()]
    );
    assert_eq!(build.statistics.as_ref().unwrap().mean.major, r(50, 1));
    for ledger in &build.entity_funding.as_ref().unwrap().states {
        assert_eq!(
            cash(ledger, "parent", Some("second"), EntityCashPhase::Closing)
                .closing_cash
                .major,
            r(40, 1)
        );
        assert_eq!(
            cash(ledger, "partner", Some("second"), EntityCashPhase::Closing)
                .closing_cash
                .major,
            ZERO
        );
        assert_eq!(
            cash(ledger, "jv", Some("second"), EntityCashPhase::Closing)
                .closing_cash
                .major,
            r(20, 1)
        );
        assert_eq!(ledger.conservation[0].opening_cash.major, r(160, 1));
        assert_eq!(ledger.conservation[0].closing_cash.major, r(160, 1));
        assert_eq!(ledger.conservation[0].internal_transfers.major, r(100, 1));
        assert_eq!(ledger.conservation[1].candidate_cash_flow.major, r(-100, 1));
        assert_eq!(ledger.conservation[1].closing_cash.major, r(60, 1));
        assert!(
            ledger
                .conservation
                .iter()
                .all(|row| row.residual.major == ZERO)
        );
    }
    let held = choice(&report, &[]).entity_funding.as_ref().unwrap();
    assert!(
        held.states
            .iter()
            .flat_map(|s| &s.transfers)
            .all(|row| !row.active && row.actual_amount.major == ZERO)
    );
    edit(&mut i, |funding| {
        funding
            .transfers
            .iter_mut()
            .find(|row| row.id == "partner_call")
            .unwrap()
            .amounts[1]
            .1 = usd(0)
    });
    let missing = crate::evaluate(&i).unwrap();
    let build = choice(&missing, &["build"]);
    assert!(build.statistics.is_none());
    let bad = state(build.entity_funding.as_ref().unwrap(), "s1");
    assert_eq!(
        cash(bad, "jv", Some("second"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(-20, 1)
    );
    assert_eq!(
        cash(bad, "parent", Some("second"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(40, 1)
    );
    assert!(
        build
            .structural_rejections
            .iter()
            .any(|reason| matches!(reason,
                StructuralRejection::EntityCashShortfall { entity_id, state_id, shortfall, .. }
                    if entity_id == "jv" && state_id == "s1" && shortfall.major == r(40, 1)
            ))
    );
}

#[test]
fn capped_distributions_pay_outside_shareholders_and_carry_the_retained_cash() {
    let mut i = with_entities(no_op(), &[("parent", 20, 20), ("jv", 100, 20)], TWO_PERIODS);
    edit(&mut i, |funding| {
        distribution(funding, "jv", "parent", r(3, 5), 50)
    });
    let report = crate::evaluate(&i).unwrap();
    let held = choice(&report, &[]);
    let ledger = state(held.entity_funding.as_ref().unwrap(), "s0");
    let first = dividend(ledger, "jv", "first");
    assert_eq!(first.available_surplus.major, r(80, 1));
    assert_eq!(first.gross_distribution.major, r(50, 1));
    assert_eq!(first.parent_receipt.major, r(30, 1));
    assert_eq!(first.external_distribution.major, r(20, 1));
    assert_eq!(first.cash_after_distribution.major, r(50, 1));
    let second = dividend(ledger, "jv", "second");
    assert_eq!(second.gross_distribution.major, r(30, 1));
    assert_eq!(second.parent_receipt.major, r(18, 1));
    assert_eq!(second.external_distribution.major, r(12, 1));
    assert_eq!(
        cash(ledger, "parent", Some("second"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(68, 1)
    );
    assert_eq!(ledger.conservation[0].closing_cash.major, r(100, 1));
    assert_eq!(
        ledger.conservation[0].external_distributions.major,
        r(20, 1)
    );
    assert_eq!(ledger.conservation[1].closing_cash.major, r(88, 1));
    assert_eq!(
        ledger.conservation[1].external_distributions.major,
        r(12, 1)
    );
    assert!(
        ledger
            .conservation
            .iter()
            .all(|row| row.residual.major == ZERO)
    );
    assert_eq!(held.statistics.as_ref().unwrap().mean.major, ZERO); // Cash movement is not another NAV payoff.
}

#[test]
fn fractional_ownership_does_not_round_cash_back_to_money_minor_units() {
    let mut i = with_entities(no_op(), &[("parent", 20, 20), ("jv", 21, 20)], TWO_PERIODS);
    edit(&mut i, |funding| {
        distribution(funding, "jv", "parent", r(1, 3), 1)
    });
    let report = crate::evaluate(&i).unwrap();
    let ledger = state(choice(&report, &[]).entity_funding.as_ref().unwrap(), "s0");
    let paid = dividend(ledger, "jv", "first");
    assert_eq!(paid.parent_receipt.major, r(1, 3));
    assert_eq!(paid.external_distribution.major, r(2, 3));
    assert_eq!(
        cash(ledger, "parent", Some("first"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(61, 3)
    );
    assert_eq!(ledger.conservation[0].closing_cash.major, r(121, 3));
    assert_eq!(ledger.conservation[0].residual.major, ZERO);
}

#[test]
fn child_first_waterfall_applies_both_ownership_tiers_before_parent_spending() {
    let mut i = with_entities(
        inputs(
            &[("big", 40, &[40]), ("small", 38, &[30])],
            &[ONE],
            40,
            r(1, 5),
            None,
        ),
        &[("parent", 20, 20), ("holding", 10, 10), ("mine", 100, 20)],
        TWO_PERIODS,
    );
    edit(&mut i, |funding| {
        distribution(funding, "holding", "parent", r(4, 5), 1000);
        distribution(funding, "mine", "holding", r(3, 5), 1000);
        flow(funding, "second", "parent", "big", "s0", -40);
        flow(funding, "second", "parent", "small", "s0", -38);
    });
    let report = crate::evaluate(&i).unwrap();
    let small = choice(&report, &["small"]);
    assert_eq!(
        report.summary().feasible_winners,
        vec![small.bundle_id.clone()]
    );
    let ledger = state(small.entity_funding.as_ref().unwrap(), "s0");
    assert_eq!(
        dividend(ledger, "mine", "first").gross_distribution.major,
        r(80, 1)
    );
    assert_eq!(
        dividend(ledger, "holding", "first")
            .gross_distribution
            .major,
        r(48, 1)
    );
    assert_eq!(
        dividend(ledger, "holding", "first").parent_receipt.major,
        r(192, 5)
    );
    assert_eq!(
        cash(ledger, "parent", Some("first"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(292, 5)
    );
    assert_eq!(
        cash(ledger, "parent", Some("second"), EntityCashPhase::Closing)
            .slack
            .major,
        r(2, 5)
    );
    assert_eq!(ledger.conservation[0].closing_cash.major, r(442, 5));
    assert_eq!(
        ledger.conservation[0].external_distributions.major,
        r(208, 5)
    );
    assert_eq!(ledger.conservation[0].residual.major, ZERO);
    let big = choice(&report, &["big"]);
    assert!(big.statistics.is_none());
    assert!(
        big.structural_rejections
            .iter()
            .any(|reason| matches!(reason,
                StructuralRejection::EntityCashShortfall { entity_id, shortfall, .. }
                    if entity_id == "parent" && shortfall.major == r(8, 5)
            ))
    );
}

#[test]
fn locked_cash_releases_later_without_erasing_an_earlier_entity_shortfall() {
    let mut i = with_entities(
        inputs(
            &[("early", 30, &[50]), ("wait", 30, &[30])],
            &[ONE],
            30,
            r(1, 5),
            None,
        ),
        &[("parent", 20, 20), ("jv", 20, 20)],
        THREE_PERIODS,
    );
    edit(&mut i, |funding| {
        distribution(funding, "jv", "parent", r(3, 5), 1000);
        funding
            .gates
            .iter_mut()
            .find(|gate| gate.period_id == "first")
            .unwrap()
            .allowed = false;
        baseline(funding, "first", "jv", "s0", 100);
        flow(funding, "first", "parent", "early", "s0", -30);
        flow(funding, "third", "parent", "wait", "s0", -30);
    });
    let report = crate::evaluate(&i).unwrap();
    let wait = choice(&report, &["wait"]);
    assert_eq!(
        report.summary().feasible_winners,
        vec![wait.bundle_id.clone()]
    );
    let ledger = state(wait.entity_funding.as_ref().unwrap(), "s0");
    assert_eq!(
        dividend(ledger, "jv", "first").gross_distribution.major,
        ZERO
    );
    assert_eq!(
        cash(ledger, "jv", Some("first"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(120, 1)
    );
    assert_eq!(
        dividend(ledger, "jv", "second").gross_distribution.major,
        r(100, 1)
    );
    assert_eq!(
        cash(ledger, "parent", Some("third"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(50, 1)
    );
    let early = choice(&report, &["early"]);
    assert!(early.statistics.is_none());
    let ledger = state(early.entity_funding.as_ref().unwrap(), "s0");
    assert_eq!(
        cash(
            ledger,
            "parent",
            Some("first"),
            EntityCashPhase::PreDistribution
        )
        .slack
        .major,
        r(-30, 1)
    );
    assert_eq!(
        cash(ledger, "parent", Some("second"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(50, 1)
    );
    assert!(early.structural_rejections.iter().any(|reason| matches!(reason,
        StructuralRejection::EntityCashShortfall { period_id, entity_id, phase: EntityCashPhase::PreDistribution, shortfall, .. }
            if period_id.as_deref() == Some("first") && entity_id == "parent" && shortfall.major == r(30, 1)
    )));
}

pub(super) fn recourse_example(
    weights: &[Rat],
    observations: &[&str],
    extra_candidate: bool,
) -> RecourseInputs {
    let study = vec![-5; weights.len()];
    let project: Vec<_> = observations
        .iter()
        .map(|observation| if *observation == "good" { 100 } else { -20 })
        .collect();
    let unused = vec![0; weights.len()];
    let mut rows: Vec<(&str, i128, &[i128])> =
        vec![("study", 5, &study), ("project", 50, &project)];
    if extra_candidate {
        rows.push(("unused", 0, &unused));
    }
    let mut program = with_entities(
        inputs(&rows, weights, 55, r(1, 5), None),
        &[("parent", 25, 20), ("jv", 20, 20)],
        THREE_PERIODS,
    );
    for candidate in &mut program.candidates {
        if candidate.value.candidate_id != "study" {
            let mut value = candidate.value.clone();
            value.requires = vec!["study".into()];
            *candidate = candidate_claim(value);
        }
    }
    edit(&mut program, |funding| {
        distribution(funding, "jv", "parent", r(3, 5), 1000);
        for (index, observation) in observations.iter().enumerate() {
            let state = format!("s{index}");
            flow(funding, "first", "parent", "study", &state, -5);
            flow(funding, "third", "parent", "project", &state, -50);
            baseline(
                funding,
                "second",
                "jv",
                &state,
                if *observation == "good" { 100 } else { 0 },
            );
        }
    });
    let basis = RecourseBasis {
        model_key: program.policy.value.model_key.clone(), currency: Currency::USD,
        valuation_basis: program.policy.value.valuation_basis.clone(), information_candidate: "study".into(),
        observation_date: "2030-05-01".into(),
        information_basis: "Synthetic study observes Good/Bad before second-period receipts and third-period project spend".into(),
        candidate_stages: program.candidates.iter().map(|candidate| CandidateStage {
            candidate_id: candidate.value.candidate_id.clone(),
            stage: if candidate.value.candidate_id == "study" { DecisionStage::Initial } else { DecisionStage::Recourse },
        }).collect(),
        observations: observations.iter().enumerate().map(|(index, observation)| StateObservation {
            state_id: format!("s{index}"), observation_id: (*observation).into(),
        }).collect(),
    };
    let tag = basis.canonical_tag();
    RecourseInputs {
        program,
        information: claim(
            "capital.recourse",
            basis,
            &tag,
            AdditivityClass::NonAdditive,
        ),
    }
}

#[test]
fn recourse_uses_only_each_realized_states_entity_cash_and_rejections() {
    let i = recourse_example(&[r(1, 2); 2], &["good", "bad"], false);
    let report = recourse::evaluate(&i).unwrap();
    let winner = report
        .summary()
        .policies
        .iter()
        .find(|policy| {
            policy.information_acquired
                && policy
                    .branches
                    .iter()
                    .all(|branch| match branch.observation_id.as_deref() {
                        Some("good") => branch.selected.iter().map(String::as_str).eq(["project"]),
                        Some("bad") => branch.selected.is_empty(),
                        _ => false,
                    })
        })
        .unwrap();
    assert_eq!(report.summary().policies.len(), 6);
    assert_eq!(
        report.summary().feasible_winners,
        vec![winner.policy_id.clone()]
    );
    assert_eq!(winner.statistics.as_ref().unwrap().mean.major, r(45, 1));
    assert_eq!(winner.tail_loss.unwrap().major, r(5, 1));
    assert_eq!(report.summary().net_adaptive_value.unwrap().major, r(45, 1));
    assert!(winner.funding_schedule.is_none());
    assert!(winner.structural_rejections.is_empty());
    let funding = winner.entity_funding.as_ref().unwrap();
    assert_eq!(
        cash(
            state(funding, "s0"),
            "parent",
            Some("third"),
            EntityCashPhase::Closing
        )
        .closing_cash
        .major,
        r(30, 1)
    );
    assert_eq!(
        cash(
            state(funding, "s1"),
            "parent",
            Some("third"),
            EntityCashPhase::Closing
        )
        .closing_cash
        .major,
        r(20, 1)
    );
    assert_eq!(
        dividend(state(funding, "s0"), "jv", "second")
            .external_distribution
            .major,
        r(40, 1)
    );
    let fixed = choice(report.static_program(), &["project", "study"]);
    assert!(fixed.statistics.is_none());
    assert!(
        fixed
            .structural_rejections
            .iter()
            .any(|reason| matches!(reason,
                StructuralRejection::EntityCashShortfall { state_id, shortfall, .. }
                    if state_id == "s1" && shortfall.major == r(50, 1)
            ))
    );
    assert!(report.to_json().contains("\"funding_schedule\":null"));
    assert!(report.to_json().contains("\"entity_cash_shortfall\""));
}

fn declaration_example() -> ProgramInputs {
    let mut i = with_entities(
        inputs(
            &[("good", 1, &[10, 20]), ("loser", 999, &[-10, -20])],
            &[r(1, 2); 2],
            1,
            r(1, 5),
            None,
        ),
        &[("parent", 100, 20), ("jv", 20, 20)],
        TWO_PERIODS,
    );
    edit(&mut i, |funding| {
        transfer(funding, "call", "parent", "jv", Some("good"), &[1, 1]);
        distribution(funding, "jv", "parent", r(3, 5), 10);
        for state in ["s0", "s1"] {
            flow(funding, "first", "jv", "good", state, -1);
            baseline(funding, "second", "jv", state, 10);
        }
    });
    i
}

#[test]
fn entity_declarations_require_complete_matching_matrices_and_acyclic_ownership() {
    let i = declaration_example();
    type Change = fn(&mut EntityFunding);
    let cases: &[(&str, Change)] = &[
        ("missing opening", |f| {
            f.opening_cash.pop();
        }),
        ("missing floor", |f| {
            f.floors.pop();
        }),
        ("missing baseline", |f| {
            f.baseline_flows.pop();
        }),
        ("missing losing flow", |f| {
            f.candidate_flows.pop();
        }),
        ("missing gate", |f| {
            f.gates.pop();
        }),
        ("duplicate opening", |f| {
            f.opening_cash[1] = f.opening_cash[0].clone()
        }),
        ("duplicate floor", |f| f.floors[1] = f.floors[0].clone()),
        ("duplicate native flow", |f| {
            f.candidate_flows[1] = f.candidate_flows[0].clone()
        }),
        ("duplicate gate", |f| f.gates[1] = f.gates[0].clone()),
        ("foreign entity", |f| {
            f.candidate_flows[0].entity_id = "foreign".into()
        }),
        ("foreign state", |f| {
            f.baseline_flows[0].state_id = "foreign".into()
        }),
        ("foreign candidate", |f| {
            f.transfers[0].candidate_id = Some("foreign".into())
        }),
        ("foreign destination", |f| {
            f.transfers[0].to_entity = "foreign".into()
        }),
        ("missing transfer amount", |f| {
            f.transfers[0].amounts.pop();
        }),
        ("duplicate transfer state", |f| {
            f.transfers[0].amounts[1].0 = "s0".into()
        }),
        ("negative transfer", |f| {
            f.transfers[0].amounts[0].1 = usd(-1)
        }),
        ("foreign currency", |f| {
            f.opening_cash[0].value = Money::from_minor(Currency::CAD, 100)
        }),
        ("negative floor", |f| f.floors[0].minimum_cash = usd(-1)),
        ("negative distribution cap", |f| {
            f.gates[0].maximum_distribution = usd(-1)
        }),
        ("excess ownership", |f| {
            f.distributions[0].parent_share = r(6, 5)
        }),
        ("self ownership", |f| {
            f.distributions[0].parent_entity_id = "jv".into()
        }),
        ("ownership cycle", |f| {
            distribution(f, "parent", "jv", ONE, 100)
        }),
        ("period gap", |f| {
            f.periods[1].cash_period = "2030-07-02/2030-12-31".into()
        }),
        ("unknown objective", |f| {
            f.objective_entity = "foreign".into()
        }),
    ];
    for (name, change) in cases {
        let mut invalid = i.clone();
        edit(&mut invalid, change);
        assert!(crate::evaluate(&invalid).is_err(), "accepted {name}");
    }
    let conflict = with_schedule(i.clone());
    assert!(crate::evaluate(&conflict).is_err());
    let mut wrong_equity = with_equity(i.clone(), &[1000, 1000], r(100, 1));
    assert!(crate::evaluate(&wrong_equity).is_err());
    let mut equity = wrong_equity.equity.as_ref().unwrap().value.clone();
    equity.entity = "parent".into();
    wrong_equity.equity = Some(equity_claim(equity));
    assert!(crate::evaluate(&wrong_equity).is_ok());
    let mut stale = i.clone();
    stale.entity_funding.as_mut().unwrap().stale = true;
    assert_eq!(crate::evaluate(&stale).unwrap_err().code, "FMINE-REF-502");
    let mut changed = i;
    changed
        .entity_funding
        .as_mut()
        .unwrap()
        .value
        .cash_basis
        .push_str(" without rebinding");
    assert_eq!(crate::evaluate(&changed).unwrap_err().code, "FMINE-REF-507");
}

#[test]
fn recourse_refuses_early_entity_actions_and_internal_transfers_as_fake_study_costs() {
    let i = recourse_example(&[r(1, 2); 2], &["good", "bad"], false);
    let mut early = i.clone();
    edit(&mut early.program, |funding| {
        flow(funding, "first", "parent", "project", "s0", 1)
    });
    assert!(recourse::evaluate(&early).is_err());
    let mut early_transfer = i.clone();
    edit(&mut early_transfer.program, |funding| {
        transfer(funding, "early", "parent", "jv", Some("project"), &[1, 1])
    });
    assert!(recourse::evaluate(&early_transfer).is_err());
    let mut internal_only = i;
    edit(&mut internal_only.program, |funding| {
        for state in ["s0", "s1"] {
            flow(funding, "first", "parent", "study", state, 0);
        }
        transfer(
            funding,
            "study_call",
            "parent",
            "jv",
            Some("study"),
            &[5, 5],
        );
    });
    assert!(recourse::evaluate(&internal_only).is_err());
}

#[test]
fn entity_inputs_and_losing_flows_bind_identity_independently_of_row_order() {
    let i = declaration_example();
    let report = crate::evaluate(&i).unwrap();
    let input = i.entity_funding.as_ref().unwrap();
    assert!(report.claim().parents.contains(&input.claim_id));
    assert!(
        input
            .anchors
            .iter()
            .all(|anchor| report.claim().anchors.contains(anchor))
    );
    let mut reversed = i.clone();
    reversed.candidates.reverse();
    reversed.weights.value.states.reverse();
    for candidate in &mut reversed.candidates {
        candidate.value.payoffs.reverse();
    }
    let funding = &mut reversed.entity_funding.as_mut().unwrap().value;
    funding.entities.reverse();
    funding.periods.reverse();
    funding.floors.reverse();
    funding.opening_cash.reverse();
    funding.baseline_flows.reverse();
    funding.candidate_flows.reverse();
    funding.transfers.reverse();
    for transfer in &mut funding.transfers {
        transfer.amounts.reverse();
    }
    funding.distributions.reverse();
    funding.gates.reverse();
    assert_eq!(crate::evaluate(&reversed).unwrap(), report);
    let mut losing = i.clone();
    edit(&mut losing, |funding| {
        flow(funding, "first", "jv", "loser", "s0", -1)
    });
    let changed = crate::evaluate(&losing).unwrap();
    assert_eq!(
        changed.summary().feasible_winners,
        report.summary().feasible_winners
    );
    assert_ne!(changed.digest(), report.digest());
    assert_ne!(changed.claim().claim_id, report.claim().claim_id);
    let mut metadata = i.clone();
    metadata.entity_funding.as_mut().unwrap().supersedes = Some(i.policy.claim_id);
    let changed = crate::evaluate(&metadata).unwrap();
    assert_eq!(changed.summary(), report.summary());
    assert_ne!(changed.digest(), report.digest());
    let mut source = i.clone();
    source.entity_funding = Some(Claim::new(
        input.metric_key.clone(),
        input.value.clone(),
        input.value_tag(),
        input.representation.clone(),
        input.support,
        input.additivity,
        input.evidence,
        input.epistemic,
        input.state,
        input.confidentiality.clone(),
        input.uu_terms.clone(),
        input.parents.clone(),
        vec![SourceAnchor::new(
            "synthetic/revised-entity-source.toml",
            [43; 32],
            "revised ownership source",
        )],
    ));
    let changed = crate::evaluate(&source).unwrap();
    assert_eq!(changed.summary(), report.summary());
    assert_ne!(changed.digest(), report.digest());
    assert_ne!(changed.claim().claim_id, report.claim().claim_id);
}

#[test]
fn exact_entity_cash_overflow_refuses_the_entire_comparison() {
    let mut i = with_entities(no_op(), &[("parent", 1, 0), ("jv", 1, 0)], TWO_PERIODS);
    edit(&mut i, |funding| {
        distribution(funding, "jv", "parent", r(i128::MAX - 1, i128::MAX), 1)
    });
    // Parent closing 2 - 1/i128::MAX has an irreducible numerator exceeding signed i128.
    assert_eq!(crate::evaluate(&i).unwrap_err().code, "FMINE-REF-207");
}

#[test]
fn entity_work_bounds_count_every_phase_transfer_distribution_and_policy() {
    let names: Vec<_> = (0..8).map(|index| format!("c{index}")).collect();
    let values = vec![0; 16];
    let rows: Vec<_> = names
        .iter()
        .map(|name| (name.as_str(), 0, values.as_slice()))
        .collect();
    let i = with_entities(
        inputs(&rows, &[r(1, 16); 16], 0, r(1, 5), None),
        &[
            ("parent", 20, 20),
            ("a", 20, 20),
            ("b", 20, 20),
            ("c", 20, 20),
        ],
        TWO_PERIODS,
    );
    // Each state has 4*(2*2+1)+2 = 22 rows: 256 subsets * 16 states * 22 = 90,112.
    assert_eq!(crate::evaluate(&i).unwrap_err().code, "FMINE-REF-201");
    let mut observations = vec!["bad"; 128];
    observations[0] = "good";
    let i = recourse_example(&[r(1, 128); 128], &observations, true);
    // Static work 8*128*20 fits; static plus 20 policies needs 28*128*20 = 71,680 rows.
    assert_eq!(recourse::evaluate(&i).unwrap_err().code, "FMINE-REF-201");
    let too_many = with_entities(
        no_op(),
        &[
            ("parent", 20, 20),
            ("a", 20, 20),
            ("b", 20, 20),
            ("c", 20, 20),
            ("d", 20, 20),
        ],
        TWO_PERIODS,
    );
    assert_eq!(
        crate::evaluate(&too_many).unwrap_err().code,
        "FMINE-REF-201"
    );
}
