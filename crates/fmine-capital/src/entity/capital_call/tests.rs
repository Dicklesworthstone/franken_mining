//! Independent subscription answers through complete static and adaptive comparisons.
//! Determinism: Exact (hand-derived fractions, retained failed paths and source binding).

use super::*;
use crate::entity::tests::{
    TWO_PERIODS, baseline, cash, choice, distribution, dividend, edit, flow, recourse_example,
    state, with_entities,
};
use crate::recourse;
use crate::tests::{inputs, r, usd};

fn call(
    id: &str,
    sequence: u16,
    period: &str,
    candidate: Option<&str>,
    amounts: &[(i128, i128, i128, i128)],
) -> CapitalCall {
    CapitalCall {
        id: id.into(),
        entity_id: "jv".into(),
        period_id: period.into(),
        sequence,
        candidate_id: candidate.map(str::to_owned),
        basis: "Synthetic same-price subscription; explicit positive pre-money equity valuation"
            .into(),
        states: amounts
            .iter()
            .enumerate()
            .map(|(i, (value, required, parent, outside))| CapitalCallState {
                state_id: format!("s{i}"),
                pre_money_value: usd(*value),
                required_funding: usd(*required),
                parent_contribution: usd(*parent),
                external_contribution: usd(*outside),
            })
            .collect(),
    }
}

fn attach(program: &mut ProgramInputs, calls: Vec<CapitalCall>) {
    edit(program, |funding| {
        funding.capital_calls = Some(EntityCapitalCallFunding {
            method: METHOD.into(),
            cash_flow_convention: CASH_FLOW_CONVENTION.into(),
            calls,
        })
    });
}

fn example() -> ProgramInputs {
    let mut program = with_entities(
        inputs(&[("fund", 1, &[10])], &[ONE], 1, r(1, 5), None),
        &[("parent", 200, 0), ("jv", 0, 0)],
        TWO_PERIODS,
    );
    edit(&mut program, |funding| {
        distribution(funding, "jv", "parent", r(3, 5), 1000);
        baseline(funding, "second", "jv", "s0", 200);
    });
    attach(
        &mut program,
        vec![call(
            "raise",
            1,
            "first",
            Some("fund"),
            &[(100, 100, 100, 0)],
        )],
    );
    program
}

#[test]
fn parent_top_up_changes_current_and_future_dividends_and_retains_inactive_calls() {
    let report = crate::evaluate(&example()).unwrap();
    let active = state(
        choice(&report, &["fund"]).entity_funding.as_ref().unwrap(),
        "s0",
    );
    assert_eq!(active.capital_calls[0].parent_share_after, r(4, 5));
    assert_eq!(
        dividend(active, "jv", "first").parent_receipt.major,
        r(80, 1)
    );
    assert_eq!(
        dividend(active, "jv", "second").parent_receipt.major,
        r(160, 1)
    );
    assert_eq!(
        cash(active, "parent", Some("second"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(340, 1)
    );
    for row in &active.conservation {
        assert_eq!(row.residual.major, ZERO);
        let flows = row.capital_call.unwrap();
        assert_eq!(flows.parent_in, flows.parent_out);
        assert_eq!(flows.external_in.major, ZERO);
    }
    let inactive =
        &state(choice(&report, &[]).entity_funding.as_ref().unwrap(), "s0").capital_calls[0];
    assert!(!inactive.active);
    assert_eq!(inactive.declared_parent_contribution.major, r(100, 1));
    assert_eq!(inactive.parent_contribution.major, ZERO);
    assert_eq!(inactive.funding_surplus.major, ZERO);
    assert_eq!(inactive.parent_share_before, inactive.parent_share_after);
    assert!(report.to_markdown().contains("Ownership after each call"));
}

#[test]
fn outside_funding_dilutes_exactly_and_underfunding_is_not_hidden_by_existing_cash() {
    let mut program = example();
    attach(
        &mut program,
        vec![call(
            "raise",
            1,
            "first",
            Some("fund"),
            &[(100, 100, 0, 100)],
        )],
    );
    let report = crate::evaluate(&program).unwrap();
    let ledger = state(
        choice(&report, &["fund"]).entity_funding.as_ref().unwrap(),
        "s0",
    );
    assert_eq!(ledger.capital_calls[0].parent_share_after, r(3, 10));
    assert_eq!(
        ledger.conservation[0]
            .capital_call
            .unwrap()
            .external_in
            .major,
        r(100, 1)
    );
    assert_eq!(
        cash(ledger, "parent", Some("second"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(290, 1)
    );
    edit(&mut program, |funding| {
        funding
            .opening_cash
            .iter_mut()
            .find(|row| row.entity_id == "jv")
            .unwrap()
            .value = usd(500);
        funding.capital_calls.as_mut().unwrap().calls[0].states[0].external_contribution = usd(20);
    });
    let report = crate::evaluate(&program).unwrap();
    let rejected = choice(&report, &["fund"]);
    assert!(rejected.statistics.is_none());
    assert!(rejected.structural_rejections.iter().any(|reason| matches!(reason, StructuralRejection::EntityCapitalCallShortfall { shortfall, .. } if shortfall.major == r(80, 1))));
    let ledger = state(rejected.entity_funding.as_ref().unwrap(), "s0");
    assert_eq!(ledger.capital_calls[0].parent_share_after, r(1, 2));
    assert!(
        ledger
            .checkpoints
            .iter()
            .all(|row| row.slack.major.signum() >= 0)
    );
    assert!(
        ledger
            .conservation
            .iter()
            .all(|row| row.residual.major == ZERO)
    );
}

#[test]
fn unaffordable_parent_funding_is_never_clamped_and_later_dividends_do_not_erase_failure() {
    let mut program = example();
    edit(&mut program, |funding| {
        funding
            .opening_cash
            .iter_mut()
            .find(|row| row.entity_id == "parent")
            .unwrap()
            .value = usd(20);
        for floor in funding
            .floors
            .iter_mut()
            .filter(|row| row.entity_id == "parent")
        {
            floor.minimum_cash = usd(20);
        }
    });
    attach(
        &mut program,
        vec![call(
            "raise",
            1,
            "first",
            Some("fund"),
            &[(100, 150, 50, 100)],
        )],
    );
    let report = crate::evaluate(&program).unwrap();
    let rejected = choice(&report, &["fund"]);
    let ledger = state(rejected.entity_funding.as_ref().unwrap(), "s0");
    assert_eq!(ledger.capital_calls[0].parent_contribution.major, r(50, 1));
    assert_eq!(ledger.capital_calls[0].parent_share_after, r(11, 25));
    assert_eq!(
        cash(
            ledger,
            "parent",
            Some("first"),
            EntityCashPhase::PreDistribution
        )
        .closing_cash
        .major,
        r(-30, 1)
    );
    assert_eq!(
        cash(ledger, "parent", Some("first"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(36, 1)
    );
    assert!(rejected.structural_rejections.iter().any(|reason| matches!(reason, StructuralRejection::EntityCashShortfall { phase: EntityCashPhase::PreDistribution, shortfall, .. } if shortfall.major == r(50, 1))));
}

#[test]
fn explicit_sequence_controls_repricing_and_canonical_input_order_does_not() {
    let mut program = example();
    attach(
        &mut program,
        vec![
            call("z-first", 1, "first", None, &[(100, 100, 0, 100)]),
            call("a-second", 2, "first", None, &[(100, 100, 100, 0)]),
        ],
    );
    let report = crate::evaluate(&program).unwrap();
    let ledger = state(choice(&report, &[]).entity_funding.as_ref().unwrap(), "s0");
    assert_eq!(
        ledger
            .capital_calls
            .iter()
            .map(|row| row.call_id.as_str())
            .collect::<Vec<_>>(),
        ["z-first", "a-second"]
    );
    assert_eq!(ledger.capital_calls[1].parent_share_after, r(13, 20));
    edit(&mut program, |funding| {
        funding.capital_calls.as_mut().unwrap().calls.reverse()
    });
    assert_eq!(
        crate::evaluate(&program).unwrap().to_json(),
        report.to_json()
    );
    edit(&mut program, |funding| {
        for call in &mut funding.capital_calls.as_mut().unwrap().calls {
            call.sequence = 3 - call.sequence;
        }
    });
    let reordered = crate::evaluate(&program).unwrap();
    assert_eq!(
        state(
            choice(&reordered, &[]).entity_funding.as_ref().unwrap(),
            "s0"
        )
        .capital_calls[1]
            .parent_share_after,
        r(2, 5)
    );
}

#[test]
fn declared_calls_bind_sources_and_refuse_missing_states_ambiguous_order_and_unsupported_terms() {
    let program = example();
    for invalid_case in 0..7 {
        let mut changed = program.clone();
        edit(&mut changed, |funding| {
            let calls = funding.capital_calls.as_mut().unwrap();
            match invalid_case {
                0 => calls.calls[0].states.clear(),
                1 => calls.calls[0].sequence = 0,
                2 => calls.calls[0].sequence = 65,
                3 => calls.calls[0].states[0].pre_money_value = usd(0),
                4 => calls.calls[0].states[0].external_contribution = usd(-1),
                5 => calls.method = "invented_penalty".into(),
                _ => {
                    let mut duplicate = calls.calls[0].clone();
                    duplicate.id = "duplicate-order".into();
                    calls.calls.push(duplicate);
                }
            }
        });
        assert!(crate::evaluate(&changed).is_err(), "case {invalid_case}");
    }
    let mut stale = program;
    stale
        .entity_funding
        .as_mut()
        .unwrap()
        .value
        .capital_calls
        .as_mut()
        .unwrap()
        .calls[0]
        .states[0]
        .required_funding = usd(101);
    assert_eq!(crate::evaluate(&stale).unwrap_err().code, "FMINE-REF-507");
}

#[test]
fn recourse_keeps_initial_calls_after_abandonment_and_routes_only_realized_call_failures() {
    let mut program = recourse_example(&[r(1, 2); 2], &["good", "bad"], false);
    edit(&mut program.program, |funding| {
        for row in funding
            .opening_cash
            .iter_mut()
            .filter(|row| row.entity_id == "parent")
        {
            row.value = usd(125);
        }
        for state in ["s0", "s1"] {
            baseline(funding, "second", "jv", state, 0);
            flow(funding, "third", "parent", "project", state, 0);
            flow(funding, "second", "jv", "study", state, -10);
            flow(funding, "second", "jv", "project", state, -100);
            flow(
                funding,
                "third",
                "jv",
                "project",
                state,
                if state == "s0" { 210 } else { 0 },
            );
        }
    });
    attach(
        &mut program.program,
        vec![
            call(
                "initial",
                1,
                "second",
                Some("study"),
                &[(100, 10, 10, 0); 2],
            ),
            call(
                "build",
                2,
                "second",
                Some("project"),
                &[(110, 100, 60, 40), (110, 110, 100, 0)],
            ),
        ],
    );
    let report = recourse::evaluate(&program).unwrap();
    let policy = report
        .summary()
        .policies
        .iter()
        .find(|policy| {
            policy.information_acquired
                && policy
                    .branches
                    .iter()
                    .all(|branch| match branch.observation_id.as_deref() {
                        Some("good") => branch.selected == ["project"],
                        Some("bad") => branch.selected.is_empty(),
                        _ => false,
                    })
        })
        .unwrap();
    assert!(policy.structural_rejections.is_empty());
    assert_eq!(policy.statistics.as_ref().unwrap().mean.major, r(45, 1));
    let ledger = policy.entity_funding.as_ref().unwrap();
    assert_eq!(
        cash(
            state(ledger, "s0"),
            "parent",
            Some("third"),
            EntityCashPhase::Closing
        )
        .closing_cash
        .major,
        r(180, 1)
    );
    assert_eq!(
        cash(
            state(ledger, "s1"),
            "parent",
            Some("third"),
            EntityCashPhase::Closing
        )
        .closing_cash
        .major,
        r(110, 1)
    );
    assert_eq!(
        state(ledger, "s1").capital_calls[0].parent_share_after,
        r(7, 11)
    );
    assert!(state(ledger, "s1").capital_calls[0].active);
    assert!(!state(ledger, "s1").capital_calls[1].active);
    assert!(choice(report.static_program(), &["project", "study"]).structural_rejections.iter().any(|reason| matches!(reason, StructuralRejection::EntityCapitalCallShortfall { state_id, shortfall, .. } if state_id == "s1" && shortfall.major == r(10, 1))));
    edit(&mut program.program, |funding| {
        let early = &mut funding.capital_calls.as_mut().unwrap().calls[1];
        early.period_id = "first".into();
        for row in &mut early.states {
            row.parent_contribution = usd(0);
            row.external_contribution = usd(0);
            row.required_funding = usd(0);
        }
    });
    assert_eq!(
        recourse::evaluate(&program).unwrap_err().code,
        "FMINE-REF-105"
    );
}
