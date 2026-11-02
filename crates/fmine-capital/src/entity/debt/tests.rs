//! Independent debt, coverage and sweep answers through the complete capital evaluators.
//! Determinism: Exact (hand-calculated cash and principal balances; shared-state policy evaluation).

use super::*;
use crate::entity::tests::{
    TWO_PERIODS, baseline, cash, choice, distribution, dividend, edit, flow, no_op,
    recourse_example, state, with_entities,
};
use crate::recourse::{self, RecourseInputs};
use crate::tests::{candidate_claim, equity_claim, inputs, r, usd, with_equity};

fn with_debt(mut program: ProgramInputs, facilities: &[(&str, &str, i128)]) -> ProgramInputs {
    let candidates: Vec<_> = program
        .candidates
        .iter()
        .map(|candidate| candidate.value.candidate_id.clone())
        .collect();
    edit(&mut program, |funding| {
        let mut debt = EntityDebtFunding {
            cash_flow_convention: CASH_FLOW_CONVENTION.into(),
            facilities: vec![],
            opening_principal: vec![],
            baseline_flows: vec![],
            candidate_flows: vec![],
        };
        for (id, entity, principal) in facilities {
            debt.facilities.push(DebtFacility {
                id: (*id).into(),
                entity_id: (*entity).into(),
                basis: "Synthetic senior external loan; full-period rate on opening plus draw"
                    .into(),
                cfads_basis: "Synthetic operating covenant metric; excludes financing receipts"
                    .into(),
                minimum_dscr: r(3, 2),
                sweep_fraction: ZERO,
                allow_distributions_without_service: true,
            });
            for opening in funding
                .opening_cash
                .iter()
                .filter(|row| row.entity_id == *entity)
            {
                debt.opening_principal.push(DebtOpeningPrincipal {
                    facility_id: (*id).into(),
                    state_id: opening.state_id.clone(),
                    value: usd(*principal),
                });
                for period in &funding.periods {
                    debt.baseline_flows.push(DebtBaselineFlow {
                        facility_id: (*id).into(),
                        period_id: period.id.clone(),
                        state_id: opening.state_id.clone(),
                        interest_rate: ZERO,
                        draw: usd(0),
                        principal_payment_cap: usd(0),
                        cfads: usd(0),
                    });
                    for candidate in &candidates {
                        debt.candidate_flows.push(DebtCandidateFlow {
                            facility_id: (*id).into(),
                            period_id: period.id.clone(),
                            candidate_id: candidate.clone(),
                            state_id: opening.state_id.clone(),
                            draw: usd(0),
                            principal_payment_cap: usd(0),
                            cfads: usd(0),
                        });
                    }
                }
            }
        }
        funding.debt = Some(debt);
    });
    program
}

fn edit_debt(program: &mut ProgramInputs, change: impl FnOnce(&mut EntityDebtFunding)) {
    edit(program, |funding| change(funding.debt.as_mut().unwrap()));
}

fn baseline_debt<'a>(
    debt: &'a mut EntityDebtFunding,
    facility: &str,
    period: &str,
    state: &str,
) -> &'a mut DebtBaselineFlow {
    debt.baseline_flows
        .iter_mut()
        .find(|row| row.facility_id == facility && row.period_id == period && row.state_id == state)
        .unwrap()
}

fn candidate_debt<'a>(
    debt: &'a mut EntityDebtFunding,
    facility: &str,
    period: &str,
    candidate: &str,
    state: &str,
) -> &'a mut DebtCandidateFlow {
    debt.candidate_flows
        .iter_mut()
        .find(|row| {
            row.facility_id == facility
                && row.period_id == period
                && row.candidate_id == candidate
                && row.state_id == state
        })
        .unwrap()
}

fn service<'a>(
    ledger: &'a EntityFundingState,
    facility: &str,
    period: &str,
) -> &'a DebtServiceResult {
    ledger
        .debt
        .iter()
        .find(|row| row.facility_id == facility && row.period_id == period)
        .unwrap()
}

fn sum(values: &[Rat]) -> Rat {
    values
        .iter()
        .fold(ZERO, |total, value| total.checked_add(*value).unwrap())
}

fn conserved(ledger: &EntityFundingState) {
    for row in &ledger.debt {
        assert_eq!(row.principal_residual.major, ZERO);
        assert_eq!(
            sum(&[row.opening_principal.major, row.draw.major]),
            sum(&[
                row.closing_principal.major,
                row.scheduled_principal.major,
                row.sweep.major
            ]),
        );
    }
    for row in &ledger.conservation {
        let debt = row.debt.as_ref().unwrap();
        assert_eq!(row.residual.major, ZERO);
        assert_eq!(
            sum(&[
                row.opening_cash.major,
                row.baseline_cash_flow.major,
                row.candidate_cash_flow.major,
                debt.draw.major
            ]),
            sum(&[
                row.closing_cash.major,
                row.external_distributions.major,
                debt.interest.major,
                debt.scheduled_principal.major,
                debt.sweep.major
            ]),
        );
    }
}

fn roll_forward_example() -> ProgramInputs {
    let mut program = with_debt(
        with_entities(no_op(), &[("parent", 20, 20), ("jv", 20, 20)], TWO_PERIODS),
        &[("loan", "jv", 100)],
    );
    edit(&mut program, |funding| {
        distribution(funding, "jv", "parent", r(3, 5), 1000);
        for period in ["first", "second"] {
            baseline(funding, period, "jv", "s0", 50);
        }
    });
    edit_debt(&mut program, |debt| {
        debt.facilities[0].sweep_fraction = r(1, 2);
        for row in &mut debt.baseline_flows {
            row.interest_rate = r(1, 10);
            row.principal_payment_cap = usd(20);
            row.cfads = usd(50);
        }
    });
    program
}

#[test]
fn service_sweep_and_fractional_interest_carry_conserve_cash_without_changing_nav() {
    let report = crate::evaluate(&roll_forward_example()).unwrap();
    let hold = choice(&report, &[]);
    assert!(hold.structural_rejections.is_empty());
    assert_eq!(hold.statistics.as_ref().unwrap().mean.major, ZERO);
    let ledger = state(hold.entity_funding.as_ref().unwrap(), "s0");
    let first = service(ledger, "loan", "first");
    assert_eq!(first.opening_principal.major, r(100, 1));
    assert_eq!(first.interest.major, r(10, 1));
    assert_eq!(first.scheduled_principal.major, r(20, 1));
    assert_eq!(first.scheduled_service.major, r(30, 1));
    assert_eq!(first.dscr, Some(r(5, 3)));
    assert_eq!(first.coverage_status, DebtCoverageStatus::Covered);
    assert_eq!(first.cash_after_service.major, r(40, 1));
    assert_eq!(first.sweep.major, r(10, 1));
    assert_eq!(first.closing_principal.major, r(70, 1));
    let second = service(ledger, "loan", "second");
    assert_eq!(second.opening_principal.major, r(70, 1));
    assert_eq!(second.interest.major, r(7, 1));
    assert_eq!(second.scheduled_service.major, r(27, 1));
    assert_eq!(second.dscr, Some(r(50, 27)));
    assert_eq!(second.sweep.major, r(23, 2));
    assert_eq!(second.closing_principal.major, r(77, 2));
    assert_eq!(
        dividend(ledger, "jv", "first").gross_distribution.major,
        r(10, 1)
    );
    assert_eq!(
        dividend(ledger, "jv", "first").parent_receipt.major,
        r(6, 1)
    );
    assert_eq!(
        dividend(ledger, "jv", "first").external_distribution.major,
        r(4, 1)
    );
    assert_eq!(
        dividend(ledger, "jv", "second").parent_receipt.major,
        r(69, 10)
    );
    assert_eq!(
        dividend(ledger, "jv", "second").external_distribution.major,
        r(23, 5)
    );
    assert_eq!(
        cash(ledger, "parent", Some("second"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(329, 10)
    );
    assert_eq!(
        cash(ledger, "jv", Some("second"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(20, 1)
    );
    let pre = cash(
        ledger,
        "jv",
        Some("first"),
        EntityCashPhase::PreDistribution,
    )
    .debt
    .as_ref()
    .unwrap();
    let close = cash(ledger, "jv", Some("first"), EntityCashPhase::Closing)
        .debt
        .as_ref()
        .unwrap();
    assert_eq!(pre.interest.major, r(10, 1));
    assert_eq!(pre.scheduled_principal.major, r(20, 1));
    assert_eq!(pre.sweep.major, ZERO);
    assert_eq!(close.sweep.major, r(10, 1));
    assert!(
        cash(ledger, "jv", None, EntityCashPhase::Opening)
            .debt
            .is_none()
    );
    assert!(
        cash(ledger, "parent", Some("first"), EntityCashPhase::Closing)
            .debt
            .is_none()
    );
    assert_eq!(ledger.conservation[0].closing_cash.major, r(46, 1));
    assert_eq!(ledger.conservation[1].closing_cash.major, r(529, 10));
    for row in &report.summary().choices {
        for ledger in &row.entity_funding.as_ref().unwrap().states {
            conserved(ledger);
        }
    }
}

#[test]
fn coverage_locks_carry_cash_threshold_equality_passes_and_permission_still_applies() {
    let mut program = with_debt(
        with_entities(
            inputs(&[("unused", 1, &[-1, -1])], &[r(1, 2); 2], 0, r(1, 5), None),
            &[("parent", 20, 20), ("jv", 20, 20)],
            TWO_PERIODS,
        ),
        &[("loan", "jv", 100)],
    );
    edit(&mut program, |funding| {
        distribution(funding, "jv", "parent", r(3, 5), 1000);
        for (state, first_cash) in [("s0", 36), ("s1", 45)] {
            baseline(funding, "first", "jv", state, first_cash);
            baseline(funding, "second", "jv", state, 50);
        }
        funding
            .gates
            .iter_mut()
            .find(|row| row.period_id == "second" && row.state_id == "s1")
            .unwrap()
            .allowed = false;
    });
    edit_debt(&mut program, |debt| {
        for row in &mut debt.baseline_flows {
            row.interest_rate = r(1, 10);
            row.principal_payment_cap = usd(20);
            row.cfads = usd(if row.period_id == "second" {
                50
            } else if row.state_id == "s0" {
                36
            } else {
                45
            });
        }
    });
    let report = crate::evaluate(&program).unwrap();
    let hold = choice(&report, &[]);
    assert!(hold.structural_rejections.is_empty());
    let locked = state(hold.entity_funding.as_ref().unwrap(), "s0");
    assert_eq!(service(locked, "loan", "first").dscr, Some(r(6, 5)));
    assert_eq!(
        service(locked, "loan", "first").coverage_status,
        DebtCoverageStatus::BelowMinimum
    );
    assert_eq!(
        dividend(locked, "jv", "first").gross_distribution.major,
        ZERO
    );
    assert_eq!(
        cash(locked, "jv", Some("first"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(26, 1)
    );
    assert_eq!(service(locked, "loan", "second").dscr, Some(r(25, 14)));
    assert_eq!(
        dividend(locked, "jv", "second").gross_distribution.major,
        r(28, 1)
    );
    assert_eq!(
        cash(locked, "parent", Some("second"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(184, 5)
    );
    let equality = state(hold.entity_funding.as_ref().unwrap(), "s1");
    assert_eq!(service(equality, "loan", "first").dscr, Some(r(3, 2)));
    assert_eq!(
        dividend(equality, "jv", "first").gross_distribution.major,
        r(15, 1)
    );
    let gate = dividend(equality, "jv", "second")
        .debt_gate
        .as_ref()
        .unwrap();
    assert!(gate.distribution_allowed);
    assert!(!gate.effective_allowed);
    assert_eq!(
        dividend(equality, "jv", "second").gross_distribution.major,
        ZERO
    );
    conserved(locked);
    conserved(equality);
}

#[test]
fn child_receipts_feed_intermediate_and_root_sweeps_but_not_prior_service_checkpoints() {
    let mut program = with_debt(
        with_entities(
            no_op(),
            &[("parent", 20, 20), ("hold", 20, 20), ("mine", 20, 20)],
            TWO_PERIODS,
        ),
        &[("a_root", "parent", 20), ("z_intermediate", "hold", 50)],
    );
    edit(&mut program, |funding| {
        baseline(funding, "first", "mine", "s0", 80);
        distribution(funding, "mine", "hold", ONE, 1000);
        distribution(funding, "hold", "parent", r(3, 4), 1000);
    });
    edit_debt(&mut program, |debt| {
        for facility in &mut debt.facilities {
            facility.sweep_fraction = r(1, 2);
        }
    });
    let report = crate::evaluate(&program).unwrap();
    let ledger = state(choice(&report, &[]).entity_funding.as_ref().unwrap(), "s0");
    let middle = service(ledger, "z_intermediate", "first");
    assert_eq!(middle.cash_before_sweep.major, r(100, 1));
    assert_eq!(middle.sweep.major, r(40, 1));
    assert_eq!(middle.closing_principal.major, r(10, 1));
    assert_eq!(
        dividend(ledger, "hold", "first").gross_distribution.major,
        r(40, 1)
    );
    assert_eq!(
        dividend(ledger, "hold", "first").parent_receipt.major,
        r(30, 1)
    );
    assert_eq!(
        dividend(ledger, "hold", "first")
            .external_distribution
            .major,
        r(10, 1)
    );
    let root = service(ledger, "a_root", "first");
    assert_eq!(root.cash_before_sweep.major, r(50, 1));
    assert_eq!(root.sweep.major, r(15, 1));
    assert_eq!(root.closing_principal.major, r(5, 1));
    assert_eq!(
        cash(ledger, "parent", Some("first"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(35, 1)
    );
    assert_eq!(ledger.conservation[0].closing_cash.major, r(75, 1));
    assert_eq!(
        ledger.conservation[0].debt.as_ref().unwrap().sweep.major,
        r(55, 1)
    );
    assert_eq!(
        ledger
            .debt
            .iter()
            .take(2)
            .map(|row| row.facility_id.as_str())
            .collect::<Vec<_>>(),
        ["a_root", "z_intermediate"]
    );
    conserved(ledger);

    edit_debt(&mut program, |debt| {
        let first = baseline_debt(debt, "z_intermediate", "first", "s0");
        first.principal_payment_cap = usd(5);
        first.cfads = usd(10);
    });
    let report = crate::evaluate(&program).unwrap();
    let hold = choice(&report, &[]);
    assert!(hold.statistics.is_none());
    assert!(hold.structural_rejections.iter().any(|reason| matches!(reason,
        StructuralRejection::EntityCashShortfall { entity_id, period_id, phase: EntityCashPhase::PreDistribution, shortfall, .. }
            if entity_id == "hold" && period_id.as_deref() == Some("first") && shortfall.major == r(5, 1)
    )));
    let ledger = state(hold.entity_funding.as_ref().unwrap(), "s0");
    assert_eq!(
        cash(
            ledger,
            "hold",
            Some("first"),
            EntityCashPhase::PreDistribution
        )
        .closing_cash
        .major,
        r(15, 1)
    );
    assert_eq!(
        service(ledger, "z_intermediate", "first")
            .cash_before_sweep
            .major,
        r(95, 1)
    );
    assert_eq!(
        service(ledger, "z_intermediate", "first").sweep.major,
        r(75, 2)
    );
    assert_eq!(
        cash(ledger, "hold", Some("first"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(20, 1)
    );
    conserved(ledger);
}

#[test]
fn derived_coverage_lock_changes_the_winner_even_with_abundant_jv_cash() {
    let mut program = with_debt(
        with_entities(
            inputs(
                &[("big", 30, &[50]), ("small", 5, &[10])],
                &[ONE],
                30,
                r(1, 5),
                None,
            ),
            &[("parent", 25, 20), ("jv", 100, 20)],
            TWO_PERIODS,
        ),
        &[("loan", "jv", 100)],
    );
    edit(&mut program, |funding| {
        baseline(funding, "first", "jv", "s0", 40);
        distribution(funding, "jv", "parent", r(3, 5), 1000);
        flow(funding, "second", "parent", "big", "s0", -30);
        flow(funding, "second", "parent", "small", "s0", -5);
    });
    edit_debt(&mut program, |debt| {
        debt.facilities[0].allow_distributions_without_service = false;
        let first = baseline_debt(debt, "loan", "first", "s0");
        first.principal_payment_cap = usd(40);
        first.cfads = usd(40);
    });
    let report = crate::evaluate(&program).unwrap();
    let small = choice(&report, &["small"]);
    assert_eq!(
        report.summary().feasible_winners,
        vec![small.bundle_id.clone()]
    );
    assert_eq!(small.statistics.as_ref().unwrap().mean.major, r(10, 1));
    let big = choice(&report, &["big"]);
    assert!(big.statistics.is_none());
    assert!(
        big.structural_rejections
            .iter()
            .any(|reason| matches!(reason,
                StructuralRejection::EntityCashShortfall { entity_id, shortfall, .. }
                    if entity_id == "parent" && shortfall.major == r(25, 1)
            ))
    );
    let ledger = state(small.entity_funding.as_ref().unwrap(), "s0");
    assert_eq!(service(ledger, "loan", "first").dscr, Some(ONE));
    assert_eq!(
        cash(ledger, "jv", Some("first"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(100, 1)
    );
    assert_eq!(
        dividend(ledger, "jv", "first").gross_distribution.major,
        ZERO
    );

    edit_debt(&mut program, |debt| debt.facilities[0].minimum_dscr = ONE);
    let relaxed = crate::evaluate(&program).unwrap();
    let big = choice(&relaxed, &["big"]);
    assert_eq!(
        relaxed.summary().feasible_winners,
        vec![big.bundle_id.clone()]
    );
    assert_eq!(big.statistics.as_ref().unwrap().mean.major, r(50, 1));
    assert_eq!(
        dividend(
            state(big.entity_funding.as_ref().unwrap(), "s0"),
            "jv",
            "first"
        )
        .parent_receipt
        .major,
        r(48, 1)
    );
}

#[test]
fn candidate_financing_and_covenant_metrics_remain_separate_from_native_cash_and_nav() {
    let mut program = with_debt(
        with_entities(
            inputs(
                &[("borrow", 0, &[0]), ("project", 50, &[100])],
                &[ONE],
                50,
                r(1, 5),
                None,
            ),
            &[("parent", 20, 20)],
            TWO_PERIODS,
        ),
        &[("loan", "parent", 0)],
    );
    edit(&mut program, |funding| {
        baseline(funding, "first", "parent", "s0", 5);
        flow(funding, "first", "parent", "project", "s0", -50);
        baseline(funding, "second", "parent", "s0", 60);
    });
    edit_debt(&mut program, |debt| {
        baseline_debt(debt, "loan", "first", "s0").interest_rate = r(1, 10);
        candidate_debt(debt, "loan", "first", "borrow", "s0").draw = usd(50);
        candidate_debt(debt, "loan", "second", "borrow", "s0").principal_payment_cap = usd(50);
        baseline_debt(debt, "loan", "second", "s0").cfads = usd(60);
        candidate_debt(debt, "loan", "second", "project", "s0").cfads = usd(20);
    });
    let report = crate::evaluate(&program).unwrap();
    let both = choice(&report, &["borrow", "project"]);
    assert_eq!(
        report.summary().feasible_winners,
        vec![both.bundle_id.clone()]
    );
    assert_eq!(both.statistics.as_ref().unwrap().mean.major, r(100, 1));
    assert!(choice(&report, &["project"]).statistics.is_none());
    assert_eq!(
        choice(&report, &["borrow"])
            .statistics
            .as_ref()
            .unwrap()
            .mean
            .major,
        ZERO
    );
    let ledger = state(both.entity_funding.as_ref().unwrap(), "s0");
    let first = service(ledger, "loan", "first");
    assert_eq!(first.candidate_draw.major, r(50, 1));
    assert_eq!(first.principal_before_service.major, r(50, 1));
    assert_eq!(first.interest.major, r(5, 1));
    assert_eq!(first.scheduled_service.major, r(5, 1));
    assert_eq!(first.dscr, Some(ZERO));
    assert_eq!(first.coverage_status, DebtCoverageStatus::BelowMinimum);
    assert_eq!(first.cfads.major, ZERO);
    assert_eq!(first.closing_principal.major, r(50, 1));
    assert_eq!(
        cash(ledger, "parent", Some("first"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(20, 1)
    );
    let second = service(ledger, "loan", "second");
    assert_eq!(second.baseline_cfads.major, r(60, 1));
    assert_eq!(second.candidate_cfads.major, r(20, 1));
    assert_eq!(second.cfads.major, r(80, 1));
    assert_eq!(second.dscr, Some(r(8, 5)));
    assert_eq!(second.candidate_principal_payment_cap.major, r(50, 1));
    assert_eq!(second.scheduled_principal.major, r(50, 1));
    assert_eq!(second.closing_principal.major, ZERO);
    // The separate +20 covenant metric never becomes another cash receipt or another NAV gain.
    assert_eq!(
        cash(ledger, "parent", Some("second"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(30, 1)
    );
    let borrow_only = state(
        choice(&report, &["borrow"])
            .entity_funding
            .as_ref()
            .unwrap(),
        "s0",
    );
    assert_eq!(service(borrow_only, "loan", "second").dscr, Some(r(6, 5)));
    assert_eq!(
        service(borrow_only, "loan", "second").coverage_status,
        DebtCoverageStatus::BelowMinimum
    );
    for row in &report.summary().choices {
        for ledger in &row.entity_funding.as_ref().unwrap().states {
            conserved(ledger);
        }
    }
}

#[test]
fn mandatory_service_is_never_clipped_to_cash_and_later_recovery_keeps_the_breach() {
    let mut program = roll_forward_example();
    edit(&mut program, |funding| {
        baseline(funding, "first", "jv", "s0", 5);
        baseline(funding, "second", "jv", "s0", 100);
    });
    edit_debt(&mut program, |debt| {
        debt.facilities[0].sweep_fraction = ZERO;
        baseline_debt(debt, "loan", "first", "s0").cfads = usd(5);
        baseline_debt(debt, "loan", "second", "s0").cfads = usd(100);
    });
    let report = crate::evaluate(&program).unwrap();
    let hold = choice(&report, &[]);
    assert!(hold.statistics.is_none());
    let ledger = state(hold.entity_funding.as_ref().unwrap(), "s0");
    let first = service(ledger, "loan", "first");
    assert_eq!(first.scheduled_service.major, r(30, 1));
    assert_eq!(first.cash_after_service.major, r(-5, 1));
    assert_eq!(first.closing_principal.major, r(80, 1));
    assert_eq!(first.dscr, Some(r(1, 6)));
    assert_eq!(service(ledger, "loan", "second").interest.major, r(8, 1));
    assert_eq!(
        cash(ledger, "jv", Some("second"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(20, 1)
    );
    assert_eq!(
        dividend(ledger, "jv", "second").gross_distribution.major,
        r(47, 1)
    );
    assert!(hold.structural_rejections.iter().any(|reason| matches!(reason,
        StructuralRejection::EntityCashShortfall { entity_id, period_id, phase: EntityCashPhase::PreDistribution, shortfall, .. }
            if entity_id == "jv" && period_id.as_deref() == Some("first") && shortfall.major == r(25, 1)
    )));
    conserved(ledger);
}

#[test]
fn no_service_permission_is_explicit_and_both_repayment_paths_stop_at_outstanding_debt() {
    let mut program = with_debt(
        with_entities(no_op(), &[("parent", 20, 20), ("jv", 20, 20)], TWO_PERIODS),
        &[("loan", "jv", 5)],
    );
    edit(&mut program, |funding| {
        baseline(funding, "first", "jv", "s0", 20);
        distribution(funding, "jv", "parent", r(1, 3), 1000);
    });
    edit_debt(&mut program, |debt| {
        debt.facilities[0].sweep_fraction = ONE;
        baseline_debt(debt, "loan", "first", "s0").cfads = usd(20);
    });
    let report = crate::evaluate(&program).unwrap();
    let ledger = state(choice(&report, &[]).entity_funding.as_ref().unwrap(), "s0");
    let first = service(ledger, "loan", "first");
    assert_eq!(first.dscr, None);
    assert_eq!(first.coverage_status, DebtCoverageStatus::NoServiceDue);
    assert!(first.distribution_allowed);
    assert_eq!(first.available_sweep_cash.major, r(20, 1));
    assert_eq!(first.sweep.major, r(5, 1));
    assert_eq!(first.closing_principal.major, ZERO);
    assert_eq!(
        dividend(ledger, "jv", "first").gross_distribution.major,
        r(15, 1)
    );
    assert_eq!(
        cash(ledger, "parent", Some("first"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(25, 1)
    );
    assert_eq!(service(ledger, "loan", "second").sweep.major, ZERO);
    conserved(ledger);

    edit_debt(&mut program, |debt| {
        debt.facilities[0].allow_distributions_without_service = false
    });
    let report = crate::evaluate(&program).unwrap();
    let ledger = state(choice(&report, &[]).entity_funding.as_ref().unwrap(), "s0");
    assert!(!service(ledger, "loan", "first").distribution_allowed);
    assert_eq!(service(ledger, "loan", "first").sweep.major, r(5, 1));
    assert_eq!(
        dividend(ledger, "jv", "first").gross_distribution.major,
        ZERO
    );
    assert_eq!(
        cash(ledger, "jv", Some("second"), EntityCashPhase::Closing)
            .closing_cash
            .major,
        r(35, 1)
    );

    edit_debt(&mut program, |debt| {
        baseline_debt(debt, "loan", "first", "s0").principal_payment_cap = usd(10);
    });
    let report = crate::evaluate(&program).unwrap();
    let ledger = state(choice(&report, &[]).entity_funding.as_ref().unwrap(), "s0");
    let first = service(ledger, "loan", "first");
    assert_eq!(first.principal_payment_cap.major, r(10, 1));
    assert_eq!(first.scheduled_principal.major, r(5, 1));
    assert_eq!(first.sweep.major, ZERO);
    assert_eq!(first.closing_principal.major, ZERO);
    assert_eq!(first.dscr, Some(r(4, 1)));
    assert_eq!(
        dividend(ledger, "jv", "first").gross_distribution.major,
        r(15, 1)
    );
    conserved(ledger);
}

fn debt_recourse_example() -> RecourseInputs {
    let mut input = recourse_example(&[r(1, 2); 2], &["good", "bad"], false);
    for candidate in &mut input.program.candidates {
        if candidate.value.candidate_id == "project" {
            let mut value = candidate.value.clone();
            value.cost = usd(20);
            *candidate = candidate_claim(value);
        }
    }
    input.program = with_debt(input.program, &[("loan", "jv", 100)]);
    edit(&mut input.program, |funding| {
        for (state, receipt, later_receipt) in [("s0", 130, 4), ("s1", 30, 9)] {
            baseline(funding, "second", "jv", state, receipt);
            baseline(funding, "third", "jv", state, later_receipt);
            flow(funding, "third", "parent", "project", state, -20);
        }
    });
    edit_debt(&mut input.program, |debt| {
        debt.facilities[0].sweep_fraction = r(1, 2);
        for (state, cfads, later_cfads) in [("s0", 130, 3), ("s1", 30, 8)] {
            let second = baseline_debt(debt, "loan", "second", state);
            second.interest_rate = r(1, 10);
            second.principal_payment_cap = usd(20);
            second.cfads = usd(cfads);
            let third = baseline_debt(debt, "loan", "third", state);
            third.interest_rate = r(1, 10);
            third.cfads = usd(later_cfads);
            let commitment = candidate_debt(debt, "loan", "third", "study", state);
            commitment.principal_payment_cap = usd(1);
            commitment.cfads = usd(1);
        }
    });
    input
}

#[test]
fn adaptive_debt_uses_realized_states_and_keeps_initial_obligations_after_abandonment() {
    let input = debt_recourse_example();
    let report = recourse::evaluate(&input).unwrap();
    assert_eq!(report.summary().policies.len(), 6);
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
    assert_eq!(
        report.summary().feasible_winners,
        vec![winner.policy_id.clone()]
    );
    assert_eq!(winner.statistics.as_ref().unwrap().mean.major, r(45, 1));
    assert_eq!(winner.tail_loss.unwrap().major, r(5, 1));
    assert_eq!(report.summary().net_adaptive_value.unwrap().major, r(45, 1));
    assert!(winner.structural_rejections.is_empty());
    let funding = winner.entity_funding.as_ref().unwrap();
    for (state_id, parent_cash, second_debt, interest, closing_debt) in
        [("s0", 30, 30, 3, 29), ("s1", 20, 80, 8, 79)]
    {
        let ledger = state(funding, state_id);
        assert_eq!(
            cash(ledger, "parent", Some("third"), EntityCashPhase::Closing)
                .closing_cash
                .major,
            r(parent_cash, 1)
        );
        assert_eq!(
            service(ledger, "loan", "second").closing_principal.major,
            r(second_debt, 1)
        );
        let continuing = service(ledger, "loan", "third");
        assert_eq!(continuing.interest.major, r(interest, 1));
        assert_eq!(continuing.candidate_principal_payment_cap.major, ONE);
        assert_eq!(continuing.candidate_cfads.major, ONE);
        assert_eq!(continuing.scheduled_principal.major, ONE);
        assert_eq!(continuing.closing_principal.major, r(closing_debt, 1));
        conserved(ledger);
    }
    let fixed = choice(report.static_program(), &["project", "study"]);
    assert!(fixed.statistics.is_none());
    assert!(
        fixed
            .structural_rejections
            .iter()
            .any(|reason| matches!(reason,
                StructuralRejection::EntityCashShortfall { state_id, entity_id, shortfall, .. }
                    if state_id == "s1" && entity_id == "parent" && shortfall.major == r(20, 1)
            ))
    );

    let mut equity_input = input;
    equity_input.program = with_equity(equity_input.program, &[1000, 1000], r(100, 1));
    let mut equity = equity_input.program.equity.as_ref().unwrap().value.clone();
    equity.entity = "parent".into();
    equity_input.program.equity = Some(equity_claim(equity));
    let equity_report = recourse::evaluate(&equity_input).unwrap();
    assert_eq!(
        equity_report.summary().feasible_winners,
        report.summary().feasible_winners
    );
    assert_eq!(
        equity_report
            .summary()
            .net_adaptive_value_per_share
            .unwrap()
            .major,
        r(9, 20)
    );
    let selected = equity_report
        .summary()
        .policies
        .iter()
        .find(|policy| policy.policy_id == winner.policy_id)
        .unwrap();
    assert_eq!(selected.statistics.as_ref().unwrap().mean.major, r(45, 1));
    assert_eq!(
        selected.per_share_statistics.as_ref().unwrap().mean.major,
        r(9, 20)
    );
    assert_eq!(selected.per_share_tail_loss.unwrap().major, r(1, 20));
}

#[test]
fn recourse_cannot_act_on_early_debt_or_buy_information_with_a_covenant_metric() {
    let input = debt_recourse_example();
    type Change = fn(&mut DebtCandidateFlow);
    let changes: &[(&str, Change)] = &[
        ("draw", |row| row.draw = usd(1)),
        ("principal cap", |row| row.principal_payment_cap = usd(1)),
        ("positive CFADS", |row| row.cfads = usd(1)),
        ("negative CFADS", |row| row.cfads = usd(-1)),
        ("offsetting draw and principal", |row| {
            row.draw = usd(1);
            row.principal_payment_cap = usd(1);
        }),
    ];
    for (name, change) in changes {
        let mut invalid = input.clone();
        edit_debt(&mut invalid.program, |debt| {
            change(candidate_debt(debt, "loan", "first", "project", "s0"))
        });
        assert_eq!(
            recourse::evaluate(&invalid).unwrap_err().code,
            "FMINE-REF-105",
            "accepted early {name}"
        );
    }
    let mut fake_payment = input;
    edit(&mut fake_payment.program, |funding| {
        for state in ["s0", "s1"] {
            flow(funding, "first", "parent", "study", state, 0);
        }
    });
    edit_debt(&mut fake_payment.program, |debt| {
        for state in ["s0", "s1"] {
            candidate_debt(debt, "loan", "first", "study", state).cfads = usd(-5);
        }
    });
    assert_eq!(
        recourse::evaluate(&fake_payment).unwrap_err().code,
        "FMINE-REF-105"
    );
}

fn declaration_example() -> ProgramInputs {
    let mut program = with_debt(
        with_entities(
            inputs(&[("unused", 1, &[-1, -1])], &[r(1, 2); 2], 0, r(1, 5), None),
            &[("parent", 20, 20), ("jv", 20, 20)],
            TWO_PERIODS,
        ),
        &[("loan", "jv", 100), ("parent_loan", "parent", 0)],
    );
    edit(&mut program, |funding| {
        distribution(funding, "jv", "parent", r(3, 5), 1000)
    });
    program
}

#[test]
fn debt_admission_requires_complete_known_currency_consistent_and_explicit_terms() {
    let input = declaration_example();
    assert!(crate::evaluate(&input).is_ok());
    type Change = fn(&mut EntityDebtFunding);
    let changes: &[(&str, Change)] = &[
        ("cash convention", |debt| {
            debt.cash_flow_convention = "native_already_includes_debt".into()
        }),
        ("missing facility", |debt| {
            debt.facilities.pop();
        }),
        ("missing opening", |debt| {
            debt.opening_principal.pop();
        }),
        ("missing baseline", |debt| {
            debt.baseline_flows.pop();
        }),
        ("missing losing candidate", |debt| {
            debt.candidate_flows.pop();
        }),
        ("duplicate facility", |debt| {
            debt.facilities[1].id = debt.facilities[0].id.clone()
        }),
        ("duplicate debtor entity", |debt| {
            debt.facilities[1].entity_id = debt.facilities[0].entity_id.clone()
        }),
        ("duplicate opening", |debt| {
            debt.opening_principal[1] = debt.opening_principal[0].clone()
        }),
        ("duplicate baseline", |debt| {
            debt.baseline_flows[1] = debt.baseline_flows[0].clone()
        }),
        ("duplicate candidate", |debt| {
            debt.candidate_flows[1] = debt.candidate_flows[0].clone()
        }),
        ("foreign entity", |debt| {
            debt.facilities[0].entity_id = "foreign".into()
        }),
        ("foreign facility", |debt| {
            debt.baseline_flows[0].facility_id = "foreign".into()
        }),
        ("foreign period", |debt| {
            debt.baseline_flows[0].period_id = "foreign".into()
        }),
        ("foreign state", |debt| {
            debt.opening_principal[0].state_id = "foreign".into()
        }),
        ("foreign candidate", |debt| {
            debt.candidate_flows[0].candidate_id = "foreign".into()
        }),
        ("empty debt basis", |debt| debt.facilities[0].basis.clear()),
        ("empty CFADS basis", |debt| {
            debt.facilities[0].cfads_basis.clear()
        }),
        ("long identifier", |debt| {
            debt.facilities[0].id = "x".repeat(129)
        }),
        ("negative principal", |debt| {
            debt.opening_principal[0].value = usd(-1)
        }),
        ("negative rate", |debt| {
            debt.baseline_flows[0].interest_rate = r(-1, 10)
        }),
        ("negative draw", |debt| {
            debt.baseline_flows[0].draw = usd(-1)
        }),
        ("negative candidate draw", |debt| {
            debt.candidate_flows[0].draw = usd(-1)
        }),
        ("negative principal cap", |debt| {
            debt.baseline_flows[0].principal_payment_cap = usd(-1)
        }),
        ("negative candidate cap", |debt| {
            debt.candidate_flows[0].principal_payment_cap = usd(-1)
        }),
        ("nonpositive DSCR", |debt| {
            debt.facilities[0].minimum_dscr = ZERO
        }),
        ("negative sweep", |debt| {
            debt.facilities[0].sweep_fraction = r(-1, 2)
        }),
        ("excess sweep", |debt| {
            debt.facilities[0].sweep_fraction = r(3, 2)
        }),
        ("opening currency", |debt| {
            debt.opening_principal[0].value = Money::from_minor(Currency::CAD, 0)
        }),
        ("baseline currency", |debt| {
            debt.baseline_flows[0].cfads = Money::from_minor(Currency::CAD, 0)
        }),
        ("candidate currency", |debt| {
            debt.candidate_flows[0].draw = Money::from_minor(Currency::CAD, 0)
        }),
    ];
    for (name, change) in changes {
        let mut invalid = input.clone();
        edit_debt(&mut invalid, change);
        assert!(crate::evaluate(&invalid).is_err(), "accepted {name}");
    }
    let mut signed_cfads = input.clone();
    edit_debt(&mut signed_cfads, |debt| {
        debt.baseline_flows[0].cfads = usd(-10);
        debt.candidate_flows[0].cfads = usd(-20);
    });
    assert!(crate::evaluate(&signed_cfads).is_ok());
    let mut unbound = input;
    unbound
        .entity_funding
        .as_mut()
        .unwrap()
        .value
        .debt
        .as_mut()
        .unwrap()
        .facilities[0]
        .minimum_dscr = r(2, 1);
    assert_eq!(crate::evaluate(&unbound).unwrap_err().code, "FMINE-REF-507");
}

#[test]
fn all_debt_inputs_and_source_metadata_bind_identity_while_row_order_does_not() {
    let input = declaration_example();
    let report = crate::evaluate(&input).unwrap();
    let source = input.entity_funding.as_ref().unwrap();
    assert!(report.claim().parents.contains(&source.claim_id));
    assert!(
        source
            .anchors
            .iter()
            .all(|anchor| report.claim().anchors.contains(anchor))
    );
    let mut reversed = input.clone();
    let debt = reversed
        .entity_funding
        .as_mut()
        .unwrap()
        .value
        .debt
        .as_mut()
        .unwrap();
    debt.facilities.reverse();
    debt.opening_principal.reverse();
    debt.baseline_flows.reverse();
    debt.candidate_flows.reverse();
    assert_eq!(crate::evaluate(&reversed).unwrap(), report);

    let mut losing = input.clone();
    edit_debt(&mut losing, |debt| {
        candidate_debt(debt, "loan", "second", "unused", "s0").cfads = usd(1);
    });
    let changed = crate::evaluate(&losing).unwrap();
    assert_eq!(
        changed.summary().feasible_winners,
        report.summary().feasible_winners
    );
    assert_ne!(changed.digest(), report.digest());
    assert_ne!(changed.claim().claim_id, report.claim().claim_id);

    let mut metadata = input.clone();
    metadata.entity_funding.as_mut().unwrap().supersedes = Some(input.policy.claim_id);
    let changed = crate::evaluate(&metadata).unwrap();
    assert_eq!(changed.summary(), report.summary());
    assert_ne!(changed.digest(), report.digest());

    let mut revised = input.clone();
    revised.entity_funding = Some(Claim::new(
        source.metric_key.clone(),
        source.value.clone(),
        source.value_tag(),
        source.representation.clone(),
        source.support,
        source.additivity,
        source.evidence,
        source.epistemic,
        source.state,
        source.confidentiality.clone(),
        source.uu_terms.clone(),
        source.parents.clone(),
        vec![SourceAnchor::new(
            "synthetic/revised-debt-source.toml",
            [43; 32],
            "revised debt and CFADS source",
        )],
    ));
    let changed = crate::evaluate(&revised).unwrap();
    assert_eq!(changed.summary(), report.summary());
    assert_ne!(changed.digest(), report.digest());
    let no_debt = with_entities(no_op(), &[("parent", 20, 20)], TWO_PERIODS);
    let legacy = crate::evaluate(&no_debt).unwrap();
    assert!(!legacy.to_json().contains("\"debt\":"));
    assert!(!legacy.to_json().contains("\"debt_gate\":"));
    assert!(
        report
            .to_json()
            .contains("\"native_excludes_modeled_debt\"")
    );
    assert!(report.to_json().contains("\"no_service_due\""));
}

#[test]
fn debt_overflow_and_combined_output_bounds_refuse_the_whole_comparison() {
    let mut overflow = roll_forward_example();
    edit_debt(&mut overflow, |debt| {
        baseline_debt(debt, "loan", "first", "s0").interest_rate = r(i128::MAX, 1);
    });
    assert_eq!(
        crate::evaluate(&overflow).unwrap_err().code,
        "FMINE-REF-207"
    );

    let names: Vec<_> = (0..8).map(|index| format!("c{index}")).collect();
    let payoffs = vec![0; 17];
    let rows: Vec<_> = names
        .iter()
        .map(|name| (name.as_str(), 0, payoffs.as_slice()))
        .collect();
    let mut large = with_entities(
        inputs(&rows, &[r(1, 17); 17], 0, r(1, 5), None),
        &[("parent", 20, 20), ("jv", 20, 20)],
        TWO_PERIODS,
    );
    edit(&mut large, |funding| {
        distribution(funding, "jv", "parent", ONE, 1000)
    });
    assert_eq!(
        crate::entity::work_per_state(&large.entity_funding.as_ref().unwrap().value).unwrap(),
        14
    );
    large = with_debt(large, &[("loan", "jv", 0)]);
    assert_eq!(
        crate::entity::work_per_state(&large.entity_funding.as_ref().unwrap().value).unwrap(),
        16
    );
    // Base: 256*17*14=60,928 rows; with debt: 256*17*16=69,632, above 65,536.
    assert_eq!(crate::evaluate(&large).unwrap_err().code, "FMINE-REF-201");

    let mut observations = vec!["bad"; 110];
    observations[0] = "good";
    let mut staged = recourse_example(&[r(1, 110); 110], &observations, true);
    assert_eq!(
        crate::entity::work_per_state(&staged.program.entity_funding.as_ref().unwrap().value)
            .unwrap(),
        20
    );
    staged.program = with_debt(staged.program, &[("loan", "jv", 0)]);
    assert_eq!(
        crate::entity::work_per_state(&staged.program.entity_funding.as_ref().unwrap().value)
            .unwrap(),
        23
    );
    // Static alone needs 8*110*23=20,240 rows; static+20 policies needs 70,840.
    // Without the three debt rows per state, the combined 61,600 rows would fit.
    assert_eq!(
        recourse::evaluate(&staged).unwrap_err().code,
        "FMINE-REF-201"
    );
}
