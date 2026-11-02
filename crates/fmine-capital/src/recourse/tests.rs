//! Independent synthetic answers for paid information, unconditional risk and realized funding.
//! Determinism: Exact (expected values below are hand calculations, not another policy solver).

use super::*;
use crate::tests::{
    candidate_claim, claim, equity_claim, inputs, policy_claim, r, schedule_claim,
    set_baseline_flow, set_candidate_flow, set_share_change, usd, with_equity, with_schedule,
};

fn information_claim(value: RecourseBasis) -> Claim<RecourseBasis> {
    let tag = value.canonical_tag();
    claim(
        "capital.recourse",
        value,
        &tag,
        AdditivityClass::NonAdditive,
    )
}

fn staged(program: ProgramInputs, observations: &[&str]) -> RecourseInputs {
    let mut program = with_schedule(program);
    for candidate in &mut program.candidates {
        if candidate.value.candidate_id != "study" {
            let mut value = candidate.value.clone();
            value.requires = vec!["study".into()];
            *candidate = candidate_claim(value);
        }
    }
    let study = program
        .candidates
        .iter()
        .find(|c| c.value.candidate_id == "study")
        .unwrap();
    let mut schedule = program.funding_schedule.as_ref().unwrap().value.clone();
    for flow in &mut schedule.candidate_flows {
        if flow.candidate_id == "study" && flow.period_id == "start" {
            flow.value = Money::from_minor(Currency::USD, -study.value.cost.minor);
        }
    }
    program.funding_schedule = Some(schedule_claim(schedule));
    assert_eq!(program.weights.value.states.len(), observations.len());
    let basis = RecourseBasis {
        model_key: program.policy.value.model_key.clone(),
        currency: Currency::USD,
        valuation_basis: program.policy.value.valuation_basis.clone(),
        information_candidate: "study".into(),
        observation_date: "2030-07-01".into(),
        information_basis: "Synthetic observation partition; revealed only after paying for study"
            .into(),
        candidate_stages: program
            .candidates
            .iter()
            .map(|candidate| CandidateStage {
                candidate_id: candidate.value.candidate_id.clone(),
                stage: if candidate.value.candidate_id == "study" {
                    DecisionStage::Initial
                } else {
                    DecisionStage::Recourse
                },
            })
            .collect(),
        observations: program
            .weights
            .value
            .states
            .iter()
            .zip(observations)
            .map(|((state, _), observation)| StateObservation {
                state_id: state.clone(),
                observation_id: (*observation).into(),
            })
            .collect(),
    };
    RecourseInputs {
        program,
        information: information_claim(basis),
    }
}

fn edit_candidate(inputs: &mut RecourseInputs, id: &str, edit: impl FnOnce(&mut CapitalCandidate)) {
    let candidate = inputs
        .program
        .candidates
        .iter_mut()
        .find(|c| c.value.candidate_id == id)
        .unwrap();
    let mut value = candidate.value.clone();
    edit(&mut value);
    *candidate = candidate_claim(value);
}

fn edit_cash(inputs: &mut RecourseInputs, edit: impl FnOnce(&mut FundingSchedule)) {
    let mut value = inputs
        .program
        .funding_schedule
        .as_ref()
        .unwrap()
        .value
        .clone();
    edit(&mut value);
    inputs.program.funding_schedule = Some(schedule_claim(value));
}

fn edit_information(inputs: &mut RecourseInputs, edit: impl FnOnce(&mut RecourseBasis)) {
    let mut value = inputs.information.value.clone();
    edit(&mut value);
    inputs.information = information_claim(value);
}

fn ids_match(actual: &[String], expected: &[&str]) -> bool {
    actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
}

fn adaptive<'a>(
    report: &'a EvaluatedRecourseProgram,
    branches: &[(&str, &[&str])],
) -> &'a PolicyChoice {
    report
        .summary()
        .policies
        .iter()
        .find(|policy| {
            ids_match(&policy.initial_selected, &["study"])
                && policy.branches.len() == branches.len()
                && branches.iter().all(|(id, selected)| {
                    policy.branches.iter().any(|branch| {
                        branch.observation_id.as_deref() == Some(*id)
                            && ids_match(&branch.selected, selected)
                    })
                })
        })
        .expect("complete adaptive policy retained")
}

fn common<'a>(report: &'a EvaluatedRecourseProgram, selected: &[&str]) -> &'a PolicyChoice {
    report
        .summary()
        .policies
        .iter()
        .find(|policy| {
            policy.initial_selected.is_empty()
                && policy.branches.len() == 1
                && policy.branches[0].observation_id.is_none()
                && ids_match(&policy.branches[0].selected, selected)
        })
        .expect("unobserved uniform policy retained")
}

fn state<'a>(policy: &'a PolicyChoice, id: &str) -> &'a PolicyState {
    policy
        .states
        .iter()
        .find(|state| state.state_id == id)
        .unwrap()
}

fn cash<'a>(policy: &'a PolicyChoice, period: &str, state: &str) -> &'a CashScheduleCheckpoint {
    policy
        .funding_schedule
        .as_ref()
        .unwrap()
        .checkpoints
        .iter()
        .find(|row| row.period_id.as_deref() == Some(period) && row.state_id == state)
        .unwrap()
}

fn study_example() -> RecourseInputs {
    let mut i = staged(
        inputs(
            &[("study", 10, &[-10, -10]), ("build", 40, &[100, -60])],
            &[r(1, 2); 2],
            50,
            r(1, 5),
            None,
        ),
        &["good", "bad"],
    );
    edit_cash(&mut i, |schedule| {
        for state in ["s0", "s1"] {
            set_candidate_flow(schedule, "finish", "build", state, -40);
        }
    });
    i
}

#[test]
fn paid_information_retains_sunk_cost_and_cannot_split_an_observation_class() {
    let i = study_example();
    let report = evaluate(&i).unwrap();
    assert_eq!(report.summary().policies.len(), 6);
    assert_eq!(report.static_program().summary().choices.len(), 4);
    assert_eq!(
        report.static_program(),
        &crate::evaluate(&i.program).unwrap()
    );
    let winner = adaptive(&report, &[("good", &["build"]), ("bad", &[])]);
    assert_eq!(
        report.summary().feasible_winners,
        vec![winner.policy_id.clone()]
    );
    assert_eq!(winner.statistics.as_ref().unwrap().mean.major, r(40, 1));
    assert_eq!(winner.tail_loss.unwrap().major, r(10, 1));
    assert_eq!(winner.initial_spend.major, r(10, 1));
    assert_eq!(state(winner, "s0").net_nav.major, r(90, 1));
    assert_eq!(state(winner, "s1").net_nav.major, r(-10, 1));
    assert_eq!(state(winner, "s0").spend.major, r(50, 1));
    assert_eq!(state(winner, "s1").spend.major, r(10, 1));
    assert_eq!(cash(winner, "start", "s1").closing_cash.major, r(90, 1));
    assert_eq!(cash(winner, "finish", "s0").closing_cash.major, r(50, 1));
    assert_eq!(cash(winner, "finish", "s1").closing_cash.major, r(90, 1));
    assert_eq!(report.summary().net_adaptive_value.unwrap().major, r(30, 1));
    let ungated = common(&report, &["build"]);
    assert_eq!(ungated.branches[0].probability, ONE);
    assert!(ungated.statistics.is_none());
    assert!(
        ungated
            .structural_rejections
            .iter()
            .any(|r| matches!(r.reason, StructuralRejection::MissingPrerequisite { .. }))
    );

    let mut indistinguishable = i.clone();
    edit_information(&mut indistinguishable, |basis| {
        for row in &mut basis.observations {
            row.observation_id = "same".into();
        }
    });
    let grouped = evaluate(&indistinguishable).unwrap();
    assert_eq!(grouped.summary().policies.len(), 4);
    let both = adaptive(&grouped, &[("same", &["build"])]);
    assert_eq!(
        grouped.summary().feasible_winners,
        vec![both.policy_id.clone()]
    );
    assert_eq!(both.statistics.as_ref().unwrap().mean.major, r(10, 1));
    assert_eq!(grouped.summary().net_adaptive_value.unwrap().major, ZERO);

    // A project without a prerequisite is available without buying information, but only uniformly.
    let mut unrestricted = i;
    edit_candidate(&mut unrestricted, "build", |candidate| {
        candidate.requires.clear()
    });
    let unrestricted = evaluate(&unrestricted).unwrap();
    let unobserved = common(&unrestricted, &["build"]);
    assert_eq!(unobserved.statistics.as_ref().unwrap().mean.major, r(20, 1));
    assert_eq!(unrestricted.summary().policies.len(), 6);
    assert_eq!(
        unrestricted.summary().net_adaptive_value.unwrap().major,
        r(20, 1)
    );
    for policy in unrestricted
        .summary()
        .policies
        .iter()
        .filter(|policy| !policy.information_acquired)
    {
        assert_eq!(policy.branches.len(), 1);
        assert_eq!(policy.branches[0].observation_id, None);
        assert_eq!(policy.branches[0].probability, ONE);
        assert!(
            policy
                .states
                .iter()
                .all(|s| s.selected == policy.branches[0].selected)
        );
    }
}

#[test]
fn risk_is_global_not_nodewise_and_premium_uses_all_static_alternatives() {
    let mut i = staged(
        inputs(
            &[("study", 1, &[-1; 4]), ("build", 1, &[-100, 40, -100, 20])],
            &[r(1, 100), r(9, 100), r(9, 100), r(81, 100)],
            2,
            r(1, 5),
            Some(10),
        ),
        &["rare", "rare", "common", "common"],
    );
    edit_candidate(&mut i, "study", |candidate| {
        candidate.cost = Money::from_minor(Currency::USD, 1000);
        for (_, payoff) in &mut candidate.payoffs {
            *payoff = Money::from_minor(Currency::USD, -1000);
        }
    });
    let mut policy = i.program.policy.value.clone();
    policy.budget = Money::from_minor(Currency::USD, 11_000);
    i.program.policy = policy_claim(policy);
    edit_cash(&mut i, |schedule| {
        for flow in &mut schedule.candidate_flows {
            if flow.candidate_id == "study" && flow.period_id == "start" {
                flow.value = Money::from_minor(Currency::USD, -1000);
            } else if flow.candidate_id == "build" && flow.period_id == "finish" {
                flow.value = usd(-1);
            }
        }
    });
    let report = evaluate(&i).unwrap();
    let winner = adaptive(&report, &[("rare", &["build"]), ("common", &[])]);
    let greedy = adaptive(&report, &[("rare", &["build"]), ("common", &["build"])]);
    let common_only = adaptive(&report, &[("rare", &[]), ("common", &["build"])]);
    assert_eq!(
        report.summary().feasible_winners,
        vec![winner.policy_id.clone()]
    );
    assert_eq!(
        report.summary().unconstrained_winners,
        vec![greedy.policy_id.clone()]
    );
    assert_eq!(winner.statistics.as_ref().unwrap().mean.major, r(5, 2));
    // Worst fifth: 1% loses 100.1 and the next 19% loses 0.1, hence 5.1 globally.
    assert_eq!(winner.tail_loss.unwrap().major, r(51, 10));
    assert_eq!(
        winner.statistics.as_ref().unwrap().cvar95_loss.major,
        r(201, 10)
    );
    assert!(!winner.risk_rejected);
    assert_eq!(greedy.statistics.as_ref().unwrap().mean.major, r(97, 10));
    assert_eq!(greedy.tail_loss.unwrap().major, r(401, 10));
    assert!(greedy.risk_rejected);
    assert_eq!(
        common_only.statistics.as_ref().unwrap().mean.major,
        r(71, 10)
    );
    assert_eq!(common_only.tail_loss.unwrap().major, r(441, 10));
    assert!(common_only.risk_rejected);
    assert_eq!(
        report
            .summary()
            .expected_value_cost_of_risk_limit
            .unwrap()
            .major,
        r(36, 5)
    );
    assert_eq!(report.summary().net_adaptive_value.unwrap().major, r(5, 2));
    assert_eq!(
        report
            .summary()
            .unconstrained_net_adaptive_value
            .unwrap()
            .major,
        ZERO
    );
    for branch in &winner.branches {
        assert_eq!(
            branch.probability,
            if branch.observation_id.as_deref() == Some("rare") {
                r(1, 10)
            } else {
                r(9, 10)
            }
        );
    }
    assert_eq!(state(winner, "s0").spend.major, r(11, 10));
    assert_eq!(state(winner, "s2").spend.major, r(1, 10));
    let static_hold = report
        .static_program()
        .summary()
        .choices
        .iter()
        .find(|c| c.selected.is_empty())
        .unwrap();
    assert_eq!(
        report.static_program().summary().feasible_winners,
        vec![static_hold.bundle_id.clone()]
    );
}

#[test]
fn equity_objective_and_dated_funding_follow_the_same_realized_policy() {
    let mut i = staged(
        with_equity(
            inputs(
                &[
                    ("study", 10, &[-10, -10]),
                    ("issue", 0, &[160, 160]),
                    ("project", 100, &[100, -100]),
                ],
                &[r(1, 2); 2],
                110,
                r(1, 5),
                None,
            ),
            &[1000, 1000],
            r(100, 1),
        ),
        &["good", "bad"],
    );
    let mut equity = i.program.equity.as_ref().unwrap().value.clone();
    for state in ["s0", "s1"] {
        set_share_change(&mut equity, "issue", state, r(20, 1));
    }
    equity.max_tail_loss_per_share = Some(r(1, 10));
    i.program.equity = Some(equity_claim(equity));
    edit_cash(&mut i, |schedule| {
        for (_, opening) in &mut schedule.opening_cash {
            *opening = usd(30);
        }
        for state in ["s0", "s1"] {
            set_candidate_flow(schedule, "finish", "issue", state, 160);
            set_candidate_flow(schedule, "finish", "project", state, -100);
        }
    });
    let report = evaluate(&i).unwrap();
    assert_eq!(report.summary().policies.len(), 20);
    let winner = adaptive(&report, &[("good", &["issue", "project"]), ("bad", &[])]);
    assert_eq!(
        report.summary().feasible_winners,
        vec![winner.policy_id.clone()]
    );
    assert_eq!(winner.statistics.as_ref().unwrap().mean.major, r(120, 1));
    assert_eq!(
        winner.per_share_statistics.as_ref().unwrap().mean.major,
        r(19, 120)
    );
    assert_eq!(winner.per_share_tail_loss.unwrap().major, r(1, 10));
    assert!(!winner.risk_rejected); // Equality with the declared limit is admitted.
    assert_eq!(
        report.summary().net_adaptive_value_per_share.unwrap().major,
        r(19, 120)
    );
    assert!(report.summary().net_adaptive_value.is_none());
    assert!(report.summary().expected_value_cost_of_risk_limit.is_none());
    let equity = &winner.equity.as_ref().unwrap().states;
    let good = equity.iter().find(|s| s.state_id == "s0").unwrap();
    let bad = equity.iter().find(|s| s.state_id == "s1").unwrap();
    assert_eq!(
        (good.ending_nav.major, good.ending_shares),
        (r(1250, 1), r(120, 1))
    );
    assert_eq!(good.delta_nav_per_share.unwrap().major, r(5, 12));
    assert_eq!(
        (bad.ending_nav.major, bad.ending_shares),
        (r(990, 1), r(100, 1))
    );
    assert_eq!(bad.delta_nav_per_share.unwrap().major, r(-1, 10));
    assert_eq!(cash(winner, "finish", "s0").closing_cash.major, r(80, 1));
    assert_eq!(cash(winner, "finish", "s1").closing_cash.major, r(20, 1));
    let total_nav_greedy = adaptive(
        &report,
        &[("good", &["issue", "project"]), ("bad", &["issue"])],
    );
    assert_eq!(
        total_nav_greedy.statistics.as_ref().unwrap().mean.major,
        r(200, 1)
    );
    assert_eq!(
        total_nav_greedy
            .per_share_statistics
            .as_ref()
            .unwrap()
            .mean
            .major,
        ZERO
    );
    assert!(total_nav_greedy.risk_rejected);
    let unfunded = adaptive(&report, &[("good", &["project"]), ("bad", &[])]);
    assert!(unfunded.statistics.is_none());
    assert!(unfunded.structural_rejections.iter().any(|rejection| matches!(
        &rejection.reason, StructuralRejection::CashScheduleShortfall { state_id, .. } if state_id == "s0"
    )));

    // Later issuance cannot erase a pre-observation shortfall of one ten-thousandth of a dollar.
    edit_cash(&mut i, |schedule| {
        for (_, opening) in &mut schedule.opening_cash {
            *opening = Money::from_minor(Currency::USD, 299_999);
        }
    });
    let short = evaluate(&i).unwrap();
    let held = common(&short, &[]);
    assert_eq!(
        short.summary().feasible_winners,
        vec![held.policy_id.clone()]
    );
    let recovered = adaptive(&short, &[("good", &["issue", "project"]), ("bad", &[])]);
    assert!(recovered.statistics.is_none());
    assert_eq!(
        cash(recovered, "finish", "s0").closing_cash.major,
        r(799_999, 10_000)
    );
    assert!(recovered.structural_rejections.iter().any(|rejection| matches!(
        &rejection.reason, StructuralRejection::CashScheduleShortfall { period_id, shortfall, .. }
            if period_id.as_deref() == Some("start") && shortfall.major == r(1, 10_000)
    )));
}

#[test]
fn nonpositive_shares_reject_only_policies_that_realize_the_invalid_state() {
    let mut i = study_example();
    i.program = with_equity(i.program, &[1000, 1000], r(100, 1));
    let mut equity = i.program.equity.as_ref().unwrap().value.clone();
    set_share_change(&mut equity, "build", "s1", r(-100, 1));
    i.program.equity = Some(equity_claim(equity));
    let report = evaluate(&i).unwrap();
    let winner = adaptive(&report, &[("good", &["build"]), ("bad", &[])]);
    assert_eq!(
        report.summary().feasible_winners,
        vec![winner.policy_id.clone()]
    );
    assert!(winner.structural_rejections.is_empty());
    assert_eq!(
        winner.per_share_statistics.as_ref().unwrap().mean.major,
        r(2, 5)
    );
    let invalid = adaptive(&report, &[("good", &["build"]), ("bad", &["build"])]);
    assert!(invalid.statistics.is_none());
    assert!(invalid.per_share_statistics.is_none());
    assert!(
        invalid
            .structural_rejections
            .iter()
            .any(|rejection| matches!(
                &rejection.reason, StructuralRejection::NonpositiveShareCount { state_id, shares }
                    if state_id == "s1" && *shares == ZERO
            ))
    );
    assert!(
        invalid
            .structural_rejections
            .iter()
            .all(|rejection| rejection.state_id == "s1")
    );
    let invalid_equity = invalid
        .equity
        .as_ref()
        .unwrap()
        .states
        .iter()
        .find(|state| state.state_id == "s1")
        .unwrap();
    assert_eq!(invalid_equity.ending_shares, ZERO);
    assert_eq!(invalid_equity.ending_nav_per_share, None);
    assert_eq!(invalid_equity.delta_nav_per_share, None);
    // The winning good-state path comes from a subset rejected only in the unchosen bad state.
    let source = report
        .static_program()
        .summary()
        .choices
        .iter()
        .find(|choice| choice.bundle_id == state(winner, "s0").bundle_id)
        .unwrap();
    assert!(source.statistics.is_none());
    assert!(source.structural_rejections.iter().any(|reason| matches!(
        reason, StructuralRejection::NonpositiveShareCount { state_id, .. } if state_id == "s1"
    )));
}

#[test]
fn pathwise_constraints_can_leave_one_adaptive_policy_and_no_static_comparator() {
    let mut i = staged(
        inputs(
            &[
                ("study", 1, &[-1, -1]),
                ("A", 10, &[5, -5]),
                ("B", 10, &[-5, 5]),
            ],
            &[r(1, 2); 2],
            11,
            r(1, 5),
            Some(0),
        ),
        &["A", "B"],
    );
    for id in ["A", "B"] {
        edit_candidate(&mut i, id, |candidate| {
            candidate.exclusion_group = Some("alternative".into())
        });
    }
    edit_cash(&mut i, |schedule| {
        for (_, opening) in &mut schedule.opening_cash {
            *opening = usd(11);
        }
        for period in &mut schedule.periods {
            period.minimum_cash = usd(10);
        }
        for state in ["s0", "s1"] {
            set_baseline_flow(schedule, "finish", state, -2);
        }
        set_candidate_flow(schedule, "finish", "A", "s0", 10);
        set_candidate_flow(schedule, "finish", "A", "s1", -10);
        set_candidate_flow(schedule, "finish", "B", "s0", -10);
        set_candidate_flow(schedule, "finish", "B", "s1", 10);
    });
    let report = evaluate(&i).unwrap();
    assert_eq!(report.summary().policies.len(), 20);
    let winner = adaptive(&report, &[("A", &["A"]), ("B", &["B"])]);
    assert_eq!(
        report
            .summary()
            .policies
            .iter()
            .filter(|p| p.structural_rejections.is_empty())
            .count(),
        1
    );
    assert_eq!(
        report.summary().feasible_winners,
        vec![winner.policy_id.clone()]
    );
    let stats = winner.statistics.as_ref().unwrap();
    assert_eq!(stats.mean.major, r(4, 1));
    assert_eq!(winner.tail_loss.unwrap().major, r(-4, 1));
    assert_eq!(stats.mean_regret.major, ZERO);
    assert_eq!(stats.max_regret.major, ZERO);
    for state_id in ["s0", "s1"] {
        assert_eq!(state(winner, state_id).spend.major, r(11, 1));
        assert_eq!(state(winner, state_id).budget_slack.major, ZERO);
        assert_eq!(
            cash(winner, "finish", state_id).closing_cash.major,
            r(18, 1)
        );
    }
    assert!(
        report
            .static_program()
            .summary()
            .unconstrained_winners
            .is_empty()
    );
    assert!(
        report
            .static_program()
            .summary()
            .feasible_winners
            .is_empty()
    );
    assert!(report.summary().net_adaptive_value.is_none());
    assert!(report.summary().unconstrained_net_adaptive_value.is_none());
    assert!(
        report
            .to_json()
            .contains("\"net_adaptive_value_status\":\"no_feasible_static_comparator\"")
    );
    assert!(report.to_json().contains(
        "\"unconstrained_net_adaptive_value_status\":\"no_structurally_feasible_static_comparator\""
    ));
    let conflicting = adaptive(&report, &[("A", &["A", "B"]), ("B", &[])]);
    assert!(
        conflicting
            .structural_rejections
            .iter()
            .any(|r| matches!(r.reason, StructuralRejection::BudgetExceeded))
    );
    assert!(
        conflicting
            .structural_rejections
            .iter()
            .any(|r| matches!(r.reason, StructuralRejection::MutualExclusion { .. }))
    );
    assert!(conflicting.statistics.is_none());
    assert_eq!(conflicting.states.len(), 2);
    assert_eq!(
        conflicting
            .funding_schedule
            .as_ref()
            .unwrap()
            .checkpoints
            .len(),
        6
    );
}

#[test]
fn information_requires_complete_stage_and_observation_partitions() {
    let i = study_example();
    type Mutation = fn(&mut RecourseBasis);
    let cases: &[(RefusalCode, Mutation)] = &[
        (RefusalCode::NoClaimInDomain201, |b| {
            b.candidate_stages.pop();
        }),
        (RefusalCode::NoClaimInDomain201, |b| {
            b.observations.pop();
        }),
        (RefusalCode::BasisMismatch105, |b| {
            b.candidate_stages[1].candidate_id = b.candidate_stages[0].candidate_id.clone()
        }),
        (RefusalCode::BasisMismatch105, |b| {
            b.candidate_stages[1].candidate_id = "foreign".into()
        }),
        (RefusalCode::BasisMismatch105, |b| {
            b.observations[1].state_id = "s0".into()
        }),
        (RefusalCode::BasisMismatch105, |b| {
            b.observations[1].state_id = "foreign".into()
        }),
        (RefusalCode::BasisMismatch105, |b| {
            b.observations[1].observation_id.clear()
        }),
        (RefusalCode::BasisMismatch105, |b| {
            b.information_candidate = "build".into()
        }),
        (RefusalCode::BasisMismatch105, |b| {
            b.information_candidate = "foreign".into()
        }),
        (RefusalCode::BasisMismatch105, |b| {
            b.model_key = "foreign".into()
        }),
        (RefusalCode::BasisMismatch105, |b| {
            b.valuation_basis = "different ownership".into()
        }),
        (RefusalCode::BasisMismatch105, |b| {
            b.information_basis.clear()
        }),
    ];
    for (code, change) in cases {
        let mut invalid = i.clone();
        edit_information(&mut invalid, change);
        assert_eq!(evaluate(&invalid).unwrap_err().code, code.as_str());
    }
    let mut stale = i.clone();
    stale.information.stale = true;
    assert_eq!(
        evaluate(&stale).unwrap_err().code,
        RefusalCode::StaleAnchor502.as_str()
    );
    let mut unbudgeted = i.clone();
    unbudgeted.information.uu_terms.clear();
    assert_eq!(
        evaluate(&unbudgeted).unwrap_err().code,
        RefusalCode::UnbudgetedUnknowns204.as_str()
    );
    let mut tampered = i;
    tampered
        .information
        .value
        .information_basis
        .push_str(" altered without a new identity");
    assert_eq!(
        evaluate(&tampered).unwrap_err().code,
        RefusalCode::FrozenRecordMutation507.as_str()
    );
}

#[test]
fn admission_requires_paid_preobservation_information_and_zero_early_recourse_flows() {
    let i = study_example();
    let mut missing = i.clone();
    missing.program.funding_schedule = None;
    assert_eq!(
        evaluate(&missing).unwrap_err().code,
        RefusalCode::NoClaimInDomain201.as_str()
    );
    for date in ["2030-01-01", "2030-06-30", "2030-07-02", "2030-02-30"] {
        let mut invalid = i.clone();
        edit_information(&mut invalid, |b| b.observation_date = date.into());
        assert_eq!(
            evaluate(&invalid).unwrap_err().code,
            RefusalCode::UndeclaredUnitBasisOrTz107.as_str()
        );
    }
    let mut early = i.clone();
    edit_cash(&mut early, |s| {
        set_candidate_flow(s, "start", "build", "s1", 1)
    });
    assert_eq!(
        evaluate(&early).unwrap_err().code,
        RefusalCode::BasisMismatch105.as_str()
    );
    let mut backward = i.clone();
    edit_candidate(&mut backward, "build", |c| c.requires.clear());
    edit_candidate(&mut backward, "study", |c| {
        c.requires = vec!["build".into()]
    });
    assert_eq!(
        evaluate(&backward).unwrap_err().code,
        RefusalCode::BasisMismatch105.as_str()
    );
    let mut free = i.clone();
    edit_candidate(&mut free, "study", |c| c.cost = usd(0));
    assert_eq!(
        evaluate(&free).unwrap_err().code,
        RefusalCode::BasisMismatch105.as_str()
    );
    let mut profitable = i.clone();
    edit_candidate(&mut profitable, "study", |c| c.payoffs[0].1 = usd(0));
    assert_eq!(
        evaluate(&profitable).unwrap_err().code,
        RefusalCode::BasisMismatch105.as_str()
    );
    let mut deferred = i;
    edit_cash(&mut deferred, |s| {
        set_candidate_flow(s, "start", "study", "s1", 0);
        set_candidate_flow(s, "finish", "study", "s1", -10);
    });
    assert_eq!(
        evaluate(&deferred).unwrap_err().code,
        RefusalCode::BasisMismatch105.as_str()
    );
}

fn reanchored<T: Clone>(original: &Claim<T>, marker: u8) -> Claim<T> {
    Claim::new(
        original.metric_key.clone(),
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
            "synthetic/recourse-test.toml",
            [marker; 32],
            "declared assumptions",
        )],
    )
}

#[test]
fn complete_information_and_losing_inputs_bind_canonical_reports() {
    let i = study_example();
    let report = evaluate(&i).unwrap();
    assert!(report.claim().parents.contains(&i.information.claim_id));
    assert!(
        i.information
            .anchors
            .iter()
            .all(|anchor| report.claim().anchors.contains(anchor))
    );
    let mut reversed = i.clone();
    reversed.program.candidates.reverse();
    reversed.program.weights.value.states.reverse();
    for candidate in &mut reversed.program.candidates {
        candidate.value.payoffs.reverse();
    }
    reversed.information.value.candidate_stages.reverse();
    reversed.information.value.observations.reverse();
    let schedule = &mut reversed.program.funding_schedule.as_mut().unwrap().value;
    schedule.periods.reverse();
    schedule.opening_cash.reverse();
    schedule.baseline_flows.reverse();
    schedule.candidate_flows.reverse();
    let reordered = evaluate(&reversed).unwrap();
    assert_eq!(reordered, report);
    assert_eq!(reordered.to_json(), report.to_json());
    assert_eq!(reordered.to_markdown(), report.to_markdown());

    let mut source = i.clone();
    source.information = reanchored(&source.information, 43);
    let sourced = evaluate(&source).unwrap();
    assert_eq!(sourced.summary(), report.summary());
    assert_ne!(sourced.claim().claim_id, report.claim().claim_id);
    assert_ne!(sourced.digest(), report.digest());
    let mut metadata = i.clone();
    metadata.information.supersedes = Some(i.program.policy.claim_id);
    let amended = evaluate(&metadata).unwrap();
    assert_eq!(amended.summary(), report.summary());
    assert_ne!(amended.digest(), report.digest());
    let mut assumptions = i.clone();
    edit_information(&mut assumptions, |b| {
        b.information_basis
            .push_str("; revised source interpretation")
    });
    let amended = evaluate(&assumptions).unwrap();
    assert_eq!(amended.summary().policies, report.summary().policies);
    assert_ne!(amended.digest(), report.digest());

    // This build payoff is never realized by the winning policy, but remains part of the comparison.
    let mut losing = i;
    edit_candidate(&mut losing, "build", |c| c.payoffs[1].1 = usd(-61));
    let changed = evaluate(&losing).unwrap();
    assert_eq!(
        changed.summary().feasible_winners,
        report.summary().feasible_winners
    );
    assert_ne!(changed.digest(), report.digest());
    assert_ne!(changed.claim().claim_id, report.claim().claim_id);
    let json = report.to_json();
    assert!(json.contains("\"schema\":\"fmine.capital-recourse.v1\""));
    assert!(json.contains(&format!(
        "\"static_comparison\":{}",
        report.static_program().to_json()
    )));
    assert!(
        report
            .to_markdown()
            .contains("## Complete precommitted comparison and program inputs")
    );
}

fn broad_example(recourse: usize, weights: &[Rat], observations: &[&str]) -> RecourseInputs {
    let names: Vec<_> = (0..recourse).map(|i| format!("project_{i}")).collect();
    let zeros = vec![0; weights.len()];
    let study = vec![-1; weights.len()];
    let mut rows: Vec<(&str, i128, &[i128])> = vec![("study", 1, &study)];
    rows.extend(
        names
            .iter()
            .map(|name| (name.as_str(), 0, zeros.as_slice())),
    );
    staged(inputs(&rows, weights, 1, r(1, 5), None), observations)
}

#[test]
fn admission_bounds_include_static_work_and_do_not_duplicate_unobserved_policies() {
    let boundary = evaluate(&broad_example(7, &[ONE], &["same"])).unwrap();
    assert_eq!(boundary.summary().policies.len(), MAX_POLICIES);
    assert_eq!(boundary.static_program().summary().choices.len(), 256);
    assert_eq!(boundary.summary().net_adaptive_value.unwrap().major, ZERO);
    // Four recourse candidates and two signals require 16 common + 256 adaptive policies.
    assert_eq!(
        evaluate(&broad_example(4, &[r(1, 2); 2], &["a", "b"]))
            .unwrap_err()
            .code,
        RefusalCode::NoClaimInDomain201.as_str()
    );
    assert_eq!(
        evaluate(&broad_example(1, &[r(1, 5); 5], &["a", "b", "c", "d", "e"]))
            .unwrap_err()
            .code,
        RefusalCode::NoClaimInDomain201.as_str()
    );
    // Static 256*64*3 fits alone; the additional 256 policies take total work to 98,304.
    assert_eq!(
        evaluate(&broad_example(7, &[r(1, 64); 64], &["same"; 64]))
            .unwrap_err()
            .code,
        RefusalCode::NoClaimInDomain201.as_str()
    );
}

#[test]
fn arithmetic_overflow_refuses_the_whole_policy_comparison() {
    let mut i = broad_example(2, &[r(1, 2); 2], &["a", "b"]);
    for id in ["project_0", "project_1"] {
        edit_candidate(&mut i, id, |candidate| {
            for (_, payoff) in &mut candidate.payoffs {
                *payoff = Money::from_minor(Currency::USD, i128::MAX);
            }
        });
    }
    assert_eq!(
        evaluate(&i).unwrap_err().code,
        RefusalCode::NumericalCertificateFailed207.as_str()
    );
}
