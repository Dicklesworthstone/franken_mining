//! Independent dual-denominator answers through the complete static and policy evaluators.
//! Determinism: Exact (hand calculations; entity NAV is separately supplied, never inferred from cash).

use super::*;
use crate::entity::tests::{
    THREE_PERIODS, baseline, cash, choice, distribution, edit, flow, state, with_entities,
};
use crate::recourse::{
    self, CandidateStage, DecisionStage, EvaluatedRecourseProgram, PolicyChoice, RecourseBasis,
    RecourseInputs, StateObservation,
};
use crate::tests::{
    candidate_claim, claim, equity_claim, inputs, policy_claim, r, schedule_claim,
    set_candidate_flow, set_share_change, usd, with_equity, with_schedule,
};

fn valuation_claim(value: EntityValuation) -> Claim<EntityValuation> {
    let tag = value.canonical_tag();
    claim(
        &format!("capital.entity-valuation.{}", value.equity.entity),
        value,
        &tag,
        AdditivityClass::NonAdditive,
    )
}

fn with_valuation(
    mut program: ProgramInputs,
    entity: &str,
    baseline_nav: &[i128],
    initial_shares: Rat,
    rows: &[(&str, &[i128], &[Rat])],
    cap: Option<Rat>,
) -> ProgramInputs {
    let mut equity = program.equity.as_ref().unwrap().value.clone();
    equity.entity = entity.into();
    equity.valuation_basis = format!(
        "Synthetic separately supplied {entity} equity NAV, including all declared transaction economics"
    );
    equity.initial_share_units = initial_shares;
    equity.shares_per_unit = ONE;
    equity.max_tail_loss_per_share = cap;
    equity.baseline_nav = program
        .weights
        .value
        .states
        .iter()
        .zip(baseline_nav)
        .map(|((state, _), nav)| (state.clone(), usd(*nav)))
        .collect();
    equity.share_changes.clear();
    let mut nav_changes = Vec::new();
    for candidate in &program.candidates {
        let (_, nav, shares) = rows
            .iter()
            .find(|(name, _, _)| *name == candidate.value.candidate_id)
            .unwrap();
        assert_eq!(nav.len(), program.weights.value.states.len());
        assert_eq!(shares.len(), nav.len());
        for (index, (state, _)) in program.weights.value.states.iter().enumerate() {
            equity.share_changes.push(CandidateShareChange {
                candidate_id: candidate.value.candidate_id.clone(),
                state_id: state.clone(),
                units: shares[index],
            });
            nav_changes.push(CandidateNavChange {
                candidate_id: candidate.value.candidate_id.clone(),
                state_id: state.clone(),
                value: usd(nav[index]),
            });
        }
    }
    program
        .entity_valuations
        .push(valuation_claim(EntityValuation {
            equity,
            nav_changes,
        }));
    program
}

fn edit_valuation(
    program: &mut ProgramInputs,
    index: usize,
    change: impl FnOnce(&mut EntityValuation),
) {
    let mut value = program.entity_valuations[index].value.clone();
    change(&mut value);
    program.entity_valuations[index] = valuation_claim(value);
}

fn assessment<'a>(
    rows: &'a [EntityValuationAssessment],
    entity: &str,
) -> &'a EntityValuationAssessment {
    rows.iter().find(|row| row.entity == entity).unwrap()
}

fn equity_state<'a>(assessment: &'a EntityValuationAssessment, state: &str) -> &'a EquityState {
    assessment
        .equity
        .states
        .iter()
        .find(|row| row.state_id == state)
        .unwrap()
}

fn static_example() -> ProgramInputs {
    let mut program = with_equity(
        inputs(
            &[("aggressive", 0, &[100, 100]), ("fair", 0, &[60, 60])],
            &[r(1, 2); 2],
            0,
            r(1, 5),
            None,
        ),
        &[1000, 1000],
        r(100, 1),
    );
    for candidate in &mut program.candidates {
        let mut value = candidate.value.clone();
        value.exclusion_group = Some("deal".into());
        *candidate = candidate_claim(value);
    }
    with_valuation(
        program,
        "carve-out",
        &[600, 600],
        r(60, 1),
        &[
            ("aggressive", &[120, 120], &[r(30, 1); 2]),
            ("fair", &[126, 126], &[r(6, 1); 2]),
        ],
        Some(ZERO),
    )
}

fn information_claim(value: RecourseBasis) -> Claim<RecourseBasis> {
    let tag = value.canonical_tag();
    claim(
        "capital.recourse",
        value,
        &tag,
        AdditivityClass::NonAdditive,
    )
}

fn recourse_example() -> RecourseInputs {
    let mut program = with_entities(
        with_equity(
            inputs(
                &[
                    ("study", 10, &[-10, -10]),
                    ("aggressive", 20, &[160, -80]),
                    ("fair", 20, &[100, -40]),
                ],
                &[r(1, 2); 2],
                30,
                r(1, 5),
                None,
            ),
            &[1000, 1000],
            r(100, 1),
        ),
        &[("parent", 30, 20), ("carve-out", 20, 20)],
        THREE_PERIODS,
    );
    let mut primary = program.equity.as_ref().unwrap().value.clone();
    primary.entity = "parent".into();
    primary.max_tail_loss_per_share = Some(r(1, 10));
    program.equity = Some(equity_claim(primary));
    for candidate in &mut program.candidates {
        if candidate.value.candidate_id != "study" {
            let mut value = candidate.value.clone();
            value.requires = vec!["study".into()];
            value.exclusion_group = Some("deal".into());
            *candidate = candidate_claim(value);
        }
    }
    edit(&mut program, |funding| {
        distribution(funding, "carve-out", "parent", r(3, 5), 50);
        baseline(funding, "second", "carve-out", "s0", 50);
        for state in ["s0", "s1"] {
            flow(funding, "first", "parent", "study", state, -10);
            for candidate in ["aggressive", "fair"] {
                flow(funding, "third", "parent", candidate, state, -20);
            }
        }
    });
    program = with_valuation(
        program,
        "carve-out",
        &[600, 600],
        r(60, 1),
        &[
            ("study", &[-6, -6], &[ZERO; 2]),
            ("aggressive", &[120, -60], &[r(30, 1); 2]),
            ("fair", &[126, -54], &[r(6, 1); 2]),
        ],
        Some(r(1, 10)),
    );
    let information = RecourseBasis {
        model_key: program.policy.value.model_key.clone(),
        currency: Currency::USD,
        valuation_basis: program.policy.value.valuation_basis.clone(),
        information_candidate: "study".into(),
        observation_date: "2030-05-01".into(),
        information_basis:
            "Synthetic paid Good/Bad observation; complete independent entity valuations".into(),
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
        observations: vec![
            StateObservation {
                state_id: "s0".into(),
                observation_id: "good".into(),
            },
            StateObservation {
                state_id: "s1".into(),
                observation_id: "bad".into(),
            },
        ],
    };
    RecourseInputs {
        program,
        information: information_claim(information),
    }
}

fn adaptive<'a>(
    report: &'a EvaluatedRecourseProgram,
    good: &[&str],
    bad: &[&str],
) -> &'a PolicyChoice {
    report
        .summary()
        .policies
        .iter()
        .find(|policy| {
            policy.initial_selected == ["study"]
                && [("good", good), ("bad", bad)]
                    .iter()
                    .all(|(name, selected)| {
                        policy.branches.iter().any(|branch| {
                            branch.observation_id.as_deref() == Some(*name)
                                && branch
                                    .selected
                                    .iter()
                                    .map(String::as_str)
                                    .eq(selected.iter().copied())
                        })
                    })
        })
        .unwrap()
}

#[test]
fn a_parent_gain_cannot_hide_carve_out_per_share_dilution() {
    let report = crate::evaluate(&static_example()).unwrap();
    let aggressive = choice(&report, &["aggressive"]);
    let fair = choice(&report, &["fair"]);
    assert_eq!(report.summary().choices.len(), 4);
    assert_eq!(
        report.summary().unconstrained_winners,
        vec![aggressive.bundle_id.clone()]
    );
    assert_eq!(
        report.summary().feasible_winners,
        vec![fair.bundle_id.clone()]
    );
    assert_eq!(
        report
            .summary()
            .expected_per_share_cost_of_risk_limit
            .unwrap()
            .major,
        r(2, 5)
    );
    assert_eq!(
        aggressive.per_share_statistics.as_ref().unwrap().mean.major,
        ONE
    );
    assert_eq!(
        fair.per_share_statistics.as_ref().unwrap().mean.major,
        r(3, 5)
    );
    let harmed = assessment(&aggressive.entity_valuations, "carve-out");
    let improved = assessment(&fair.entity_valuations, "carve-out");
    let good = equity_state(harmed, "s0");
    assert_eq!(good.nav_change.major, r(120, 1));
    assert_eq!(good.ending_nav.major, r(720, 1));
    assert_eq!(good.ending_shares, r(90, 1));
    assert_eq!(good.delta_nav_per_share.unwrap().major, r(-2, 1));
    assert_eq!(
        harmed.per_share_statistics.as_ref().unwrap().mean.major,
        r(-2, 1)
    );
    assert_eq!(harmed.per_share_tail_loss.unwrap().major, r(2, 1));
    assert_eq!(
        harmed
            .per_share_statistics
            .as_ref()
            .unwrap()
            .mean_regret
            .major,
        r(3, 1)
    );
    assert!(harmed.risk_rejected && aggressive.risk_rejected);
    assert_eq!(
        improved.per_share_statistics.as_ref().unwrap().mean.major,
        ONE
    );
    assert!(!improved.risk_rejected && !fair.risk_rejected);
    let excluded = choice(&report, &["aggressive", "fair"]);
    assert!(!excluded.structural_rejections.is_empty());
    assert_eq!(excluded.entity_valuations[0].equity.states.len(), 2);
    assert!(excluded.entity_valuations[0].per_share_statistics.is_none());
    assert!(excluded.entity_valuations[0].per_share_tail_loss.is_none());
}

#[test]
fn all_entity_limits_remain_binding_and_primary_objective_is_never_blended() {
    let mut program = static_example();
    let mut primary = program.equity.as_ref().unwrap().value.clone();
    primary.max_tail_loss_per_share = Some(r(-3, 4));
    program.equity = Some(equity_claim(primary));
    let report = crate::evaluate(&program).unwrap();
    assert!(report.summary().feasible_winners.is_empty());
    assert!(
        report
            .summary()
            .expected_per_share_cost_of_risk_limit
            .is_none()
    );
    assert!(choice(&report, &["aggressive"]).risk_rejected);
    let fair = choice(&report, &["fair"]);
    assert!(fair.risk_rejected);
    assert!(!fair.entity_valuations[0].risk_rejected);
    assert!(choice(&report, &[]).risk_rejected);

    let program = with_valuation(
        static_example(),
        "minority-company",
        &[100, 100],
        r(10, 1),
        &[
            ("aggressive", &[0, 0], &[ZERO; 2]),
            ("fair", &[-20, -20], &[ZERO; 2]),
        ],
        Some(ZERO),
    );
    let report = crate::evaluate(&program).unwrap();
    assert_eq!(
        report.summary().feasible_winners,
        vec![choice(&report, &[]).bundle_id.clone()]
    );
    assert_eq!(
        report
            .summary()
            .expected_per_share_cost_of_risk_limit
            .unwrap()
            .major,
        ONE
    );
    let aggressive = choice(&report, &["aggressive"]);
    assert!(assessment(&aggressive.entity_valuations, "carve-out").risk_rejected);
    assert!(!assessment(&aggressive.entity_valuations, "minority-company").risk_rejected);
    assert!(aggressive.risk_rejected);
}

#[test]
fn same_state_ratios_fractional_tail_and_share_unit_normalization_are_exact() {
    let mut program = with_valuation(
        with_equity(
            inputs(&[("issue", 0, &[50, 50])], &[r(1, 2); 2], 0, r(3, 4), None),
            &[1000, 1000],
            r(100, 1),
        ),
        "carve-out",
        &[600, 300],
        r(60, 1),
        &[("issue", &[120, 120], &[r(30, 1), ZERO])],
        Some(r(2, 3)),
    );
    let report = crate::evaluate(&program).unwrap();
    let issued = choice(&report, &["issue"]);
    let secondary = &issued.entity_valuations[0];
    assert_eq!(
        equity_state(secondary, "s0")
            .delta_nav_per_share
            .unwrap()
            .major,
        r(-2, 1)
    );
    assert_eq!(
        equity_state(secondary, "s1")
            .delta_nav_per_share
            .unwrap()
            .major,
        r(2, 1)
    );
    assert_eq!(
        secondary.per_share_statistics.as_ref().unwrap().mean.major,
        ZERO
    );
    assert_eq!(secondary.per_share_tail_loss.unwrap().major, r(2, 3));
    assert!(!secondary.risk_rejected); // The lowest three quarters include fractional boundary mass.
    assert_eq!(
        report.summary().feasible_winners,
        vec![issued.bundle_id.clone()]
    );
    edit_valuation(&mut program, 0, |value| {
        value.equity.shares_per_unit = r(10, 1);
        value.equity.initial_share_units = r(6, 1);
        for row in &mut value.equity.share_changes {
            row.units = row.units.checked_div(r(10, 1)).unwrap();
        }
    });
    let rescaled = crate::evaluate(&program).unwrap();
    assert_eq!(rescaled.summary().choices, report.summary().choices);
    assert_ne!(rescaled.digest(), report.digest());
    edit_valuation(&mut program, 0, |value| {
        value.equity.max_tail_loss_per_share = Some(r(19_999, 30_000))
    });
    let rejected = crate::evaluate(&program).unwrap();
    assert!(choice(&rejected, &["issue"]).risk_rejected);
    assert_eq!(
        rejected.summary().feasible_winners,
        vec![choice(&rejected, &[]).bundle_id.clone()]
    );
}

#[test]
fn hindsight_for_every_entity_uses_the_same_joint_structural_population() {
    let mut program = static_example();
    let mut primary = program.equity.as_ref().unwrap().value.clone();
    for state in ["s0", "s1"] {
        set_share_change(&mut primary, "aggressive", state, r(-100, 1));
    }
    program.equity = Some(equity_claim(primary));
    edit_valuation(&mut program, 0, |value| {
        for row in &mut value.nav_changes {
            if row.candidate_id == "aggressive" {
                row.value = usd(1200);
            }
        }
    });
    let report = crate::evaluate(&program).unwrap();
    let excluded = choice(&report, &["aggressive"]);
    assert!(
        excluded.entity_valuations[0].equity.states[0]
            .delta_nav_per_share
            .unwrap()
            .major
            .signum()
            > 0
    );
    assert!(excluded.per_share_statistics.is_none());
    assert!(excluded.entity_valuations[0].per_share_statistics.is_none());
    let fair = choice(&report, &["fair"]);
    assert_eq!(
        fair.per_share_statistics
            .as_ref()
            .unwrap()
            .mean_regret
            .major,
        ZERO
    );
    assert_eq!(
        fair.entity_valuations[0]
            .per_share_statistics
            .as_ref()
            .unwrap()
            .mean_regret
            .major,
        ZERO
    );
    let hold = choice(&report, &[]);
    assert_eq!(
        hold.per_share_statistics
            .as_ref()
            .unwrap()
            .mean_regret
            .major,
        r(3, 5)
    );
    assert_eq!(
        hold.entity_valuations[0]
            .per_share_statistics
            .as_ref()
            .unwrap()
            .mean_regret
            .major,
        ONE
    );

    let mut program = static_example();
    edit_valuation(&mut program, 0, |value| {
        for state in ["s0", "s1"] {
            set_share_change(&mut value.equity, "aggressive", state, r(-60, 1));
        }
    });
    let report = crate::evaluate(&program).unwrap();
    let aggressive = choice(&report, &["aggressive"]);
    assert!(
        aggressive
            .structural_rejections
            .iter()
            .all(|reason| matches!(reason,
                StructuralRejection::EntityNonpositiveShareCount { entity, shares, .. }
                    if entity == "carve-out" && *shares == ZERO
            ))
    );
    assert!(
        aggressive.entity_valuations[0]
            .equity
            .states
            .iter()
            .all(|row| row.delta_nav_per_share.is_none())
    );
    assert_eq!(
        choice(&report, &["fair"])
            .per_share_statistics
            .as_ref()
            .unwrap()
            .mean_regret
            .major,
        ZERO
    );
}

#[test]
fn financed_recourse_keeps_both_denominators_and_secondary_cap_changes_the_deal() {
    let report = recourse::evaluate(&recourse_example()).unwrap();
    let fair = adaptive(&report, &["fair"], &[]);
    let aggressive = adaptive(&report, &["aggressive"], &[]);
    assert_eq!(report.summary().policies.len(), 20);
    assert_eq!(report.static_program().summary().choices.len(), 8);
    assert_eq!(
        report.summary().unconstrained_winners,
        vec![aggressive.policy_id.clone()]
    );
    assert_eq!(
        report.summary().feasible_winners,
        vec![fair.policy_id.clone()]
    );
    assert_eq!(
        report
            .summary()
            .expected_per_share_cost_of_risk_limit
            .unwrap()
            .major,
        r(3, 10)
    );
    assert_eq!(
        report.summary().net_adaptive_value_per_share.unwrap().major,
        r(2, 5)
    );
    assert_eq!(
        report
            .summary()
            .unconstrained_net_adaptive_value_per_share
            .unwrap()
            .major,
        r(7, 10)
    );
    assert_eq!(
        fair.per_share_statistics.as_ref().unwrap().mean.major,
        r(2, 5)
    );
    assert_eq!(fair.per_share_tail_loss.unwrap().major, r(1, 10));
    assert_eq!(
        aggressive.per_share_statistics.as_ref().unwrap().mean.major,
        r(7, 10)
    );
    assert_eq!(aggressive.per_share_tail_loss.unwrap().major, r(1, 10));
    assert!(aggressive.risk_rejected && !fair.risk_rejected);
    let harmed = assessment(&aggressive.entity_valuations, "carve-out");
    assert_eq!(
        harmed.per_share_statistics.as_ref().unwrap().mean.major,
        r(-13, 12)
    );
    assert_eq!(harmed.per_share_tail_loss.unwrap().major, r(31, 15));
    let secondary = assessment(&fair.entity_valuations, "carve-out");
    assert_eq!(
        secondary.per_share_statistics.as_ref().unwrap().mean.major,
        r(89, 220)
    );
    assert_eq!(secondary.per_share_tail_loss.unwrap().major, r(1, 10));
    assert!(!secondary.risk_rejected);
    let good = equity_state(secondary, "s0");
    let bad = equity_state(secondary, "s1");
    assert_eq!(
        (good.ending_nav.major, good.ending_shares),
        (r(720, 1), r(66, 1))
    );
    assert_eq!(good.delta_nav_per_share.unwrap().major, r(10, 11));
    assert_eq!(
        (bad.ending_nav.major, bad.ending_shares),
        (r(594, 1), r(60, 1))
    );
    assert_eq!(bad.delta_nav_per_share.unwrap().major, r(-1, 10));
    for (state_id, expected) in [("s0", 30), ("s1", 20)] {
        assert_eq!(
            cash(
                state(fair.entity_funding.as_ref().unwrap(), state_id),
                "parent",
                Some("third"),
                EntityCashPhase::Closing
            )
            .closing_cash
            .major,
            r(expected, 1)
        );
    }
    assert_eq!(
        report.static_program().summary().feasible_winners,
        vec![choice(report.static_program(), &[]).bundle_id.clone()]
    );
}

#[test]
fn an_unrealized_secondary_share_failure_does_not_poison_an_adaptive_path() {
    let mut program = recourse_example();
    edit(&mut program.program, |funding| {
        baseline(funding, "third", "parent", "s1", 20)
    });
    edit_valuation(&mut program.program, 0, |value| {
        set_share_change(&mut value.equity, "fair", "s1", r(-60, 1));
    });
    let report = recourse::evaluate(&program).unwrap();
    let good_only = adaptive(&report, &["fair"], &[]);
    assert!(good_only.structural_rejections.is_empty());
    assert_eq!(
        report.summary().feasible_winners,
        vec![good_only.policy_id.clone()]
    );
    assert_eq!(
        good_only.entity_valuations[0]
            .per_share_statistics
            .as_ref()
            .unwrap()
            .mean
            .major,
        r(89, 220)
    );
    let invalid = adaptive(&report, &["fair"], &["fair"]);
    assert!(invalid.per_share_statistics.is_none());
    assert!(invalid.entity_valuations[0].per_share_statistics.is_none());
    assert!(invalid.structural_rejections.iter().any(|rejection| rejection.state_id == "s1" && matches!(
        &rejection.reason, StructuralRejection::EntityNonpositiveShareCount { entity, state_id, shares, .. }
            if entity == "carve-out" && state_id == "s1" && *shares == ZERO
    )));
    let bad = equity_state(&invalid.entity_valuations[0], "s1");
    assert_eq!(bad.ending_shares, ZERO);
    assert!(bad.ending_nav_per_share.is_none());
    assert!(bad.delta_nav_per_share.is_none());
    let source = choice(report.static_program(), &["fair", "study"]);
    assert!(source.entity_valuations[0].per_share_statistics.is_none());
    assert!(
        source
            .structural_rejections
            .iter()
            .any(|reason| matches!(reason,
                StructuralRejection::EntityNonpositiveShareCount { entity, state_id, .. }
                    if entity == "carve-out" && state_id == "s1"
            ))
    );
}

#[test]
fn initial_secondary_nav_and_share_obligations_survive_abandonment() {
    let mut input = recourse_example();
    edit_valuation(&mut input.program, 0, |value| {
        for state in ["s0", "s1"] {
            set_share_change(&mut value.equity, "study", state, r(6, 1));
        }
        value.equity.max_tail_loss_per_share = Some(ONE);
    });
    let report = recourse::evaluate(&input).unwrap();
    let fair = adaptive(&report, &["fair"], &[]);
    assert_eq!(
        report.summary().feasible_winners,
        vec![fair.policy_id.clone()]
    );
    let child = &fair.entity_valuations[0];
    let abandoned = equity_state(child, "s1");
    assert_eq!(abandoned.nav_change.major, r(-6, 1));
    assert_eq!(abandoned.share_change, r(6, 1));
    assert_eq!(abandoned.ending_shares, r(66, 1));
    assert_eq!(abandoned.delta_nav_per_share.unwrap().major, r(-1, 1));
    assert_eq!(equity_state(child, "s0").ending_shares, r(72, 1));
    assert_eq!(
        equity_state(child, "s0").delta_nav_per_share.unwrap().major,
        ZERO
    );
    assert_eq!(
        child.per_share_statistics.as_ref().unwrap().mean.major,
        r(-1, 2)
    );
    assert_eq!(child.per_share_tail_loss.unwrap().major, ONE);
    assert!(!fair.risk_rejected);
}

#[test]
fn complete_secondary_matrices_and_distinct_valued_entities_are_required() {
    let input = static_example();
    type Change = fn(&mut EntityValuation);
    let cases: &[(&str, Change)] = &[
        ("missing baseline", |value| {
            value.equity.baseline_nav.pop();
        }),
        ("duplicate baseline", |value| {
            value.equity.baseline_nav[1] = value.equity.baseline_nav[0].clone()
        }),
        ("unknown baseline state", |value| {
            value.equity.baseline_nav[0].0 = "foreign".into()
        }),
        ("missing candidate NAV", |value| {
            value.nav_changes.pop();
        }),
        ("duplicate candidate NAV", |value| {
            value.nav_changes[1] = value.nav_changes[0].clone()
        }),
        ("unknown NAV state", |value| {
            value.nav_changes[0].state_id = "foreign".into()
        }),
        ("unknown NAV candidate", |value| {
            value.nav_changes[0].candidate_id = "foreign".into()
        }),
        ("missing share change", |value| {
            value.equity.share_changes.pop();
        }),
        ("duplicate share change", |value| {
            value.equity.share_changes[1] = value.equity.share_changes[0].clone()
        }),
        ("unknown share state", |value| {
            value.equity.share_changes[0].state_id = "foreign".into()
        }),
        ("unknown share candidate", |value| {
            value.equity.share_changes[0].candidate_id = "foreign".into()
        }),
        ("model mismatch", |value| {
            value.equity.model_key = "foreign".into()
        }),
        ("missing valuation basis", |value| {
            value.equity.valuation_basis.clear()
        }),
        ("missing share class", |value| {
            value.equity.share_class.clear()
        }),
        ("missing entity", |value| value.equity.entity.clear()),
        ("nonpositive units", |value| {
            value.equity.initial_share_units = ZERO
        }),
        ("nonpositive scale", |value| {
            value.equity.shares_per_unit = ZERO
        }),
        ("currency mismatch", |value| {
            value.equity.currency = Currency::CAD
        }),
        ("baseline currency", |value| {
            value.equity.baseline_nav[0].1 = Money::from_minor(Currency::CAD, 0)
        }),
        ("payoff currency", |value| {
            value.nav_changes[0].value = Money::from_minor(Currency::CAD, 0)
        }),
        ("primary entity repeated", |value| {
            value.equity.entity = "synthetic-parent".into()
        }),
    ];
    for (name, change) in cases {
        let mut invalid = input.clone();
        edit_valuation(&mut invalid, 0, *change);
        assert!(crate::evaluate(&invalid).is_err(), "accepted {name}");
    }
    let mut no_primary = input.clone();
    no_primary.equity = None;
    assert!(crate::evaluate(&no_primary).is_err());
    let mut duplicate = input.clone();
    duplicate
        .entity_valuations
        .push(duplicate.entity_valuations[0].clone());
    assert!(crate::evaluate(&duplicate).is_err());
    let mut excessive = input.clone();
    for index in 1..4 {
        let mut value = input.entity_valuations[0].value.clone();
        value.equity.entity = format!("extra-{index}");
        excessive.entity_valuations.push(valuation_claim(value));
    }
    assert!(crate::evaluate(&excessive).is_err());
    let mut altered = input;
    altered.entity_valuations[0].value.nav_changes[0].value = usd(999);
    assert_eq!(crate::evaluate(&altered).unwrap_err().code, "FMINE-REF-507");
}

#[test]
fn secondary_sources_losing_inputs_and_metadata_bind_while_row_order_is_canonical() {
    let mut input = static_example();
    let mut policy = input.policy.value.clone();
    policy.budget = usd(0);
    input.policy = policy_claim(policy);
    for candidate in &mut input.candidates {
        if candidate.value.candidate_id == "aggressive" {
            let mut value = candidate.value.clone();
            value.cost = usd(999);
            *candidate = candidate_claim(value);
        }
    }
    let report = crate::evaluate(&input).unwrap();
    let source = &input.entity_valuations[0];
    assert!(report.claim().parents.contains(&source.claim_id));
    assert!(
        source
            .anchors
            .iter()
            .all(|anchor| report.claim().anchors.contains(anchor))
    );
    let mut reversed = input.clone();
    reversed.entity_valuations[0]
        .value
        .equity
        .baseline_nav
        .reverse();
    reversed.entity_valuations[0]
        .value
        .equity
        .share_changes
        .reverse();
    reversed.entity_valuations[0].value.nav_changes.reverse();
    assert_eq!(crate::evaluate(&reversed).unwrap(), report);
    let mut losing = input.clone();
    edit_valuation(&mut losing, 0, |value| {
        value
            .nav_changes
            .iter_mut()
            .find(|row| row.candidate_id == "aggressive")
            .unwrap()
            .value = usd(121)
    });
    let changed = crate::evaluate(&losing).unwrap();
    assert_eq!(
        changed.summary().feasible_winners,
        report.summary().feasible_winners
    );
    assert_ne!(changed.digest(), report.digest());
    let mut metadata = input.clone();
    metadata.entity_valuations[0].supersedes = Some(input.policy.claim_id);
    let changed = crate::evaluate(&metadata).unwrap();
    assert_eq!(changed.summary(), report.summary());
    assert_ne!(changed.digest(), report.digest());
    let mut revised = input.clone();
    revised.entity_valuations[0] = Claim::new(
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
            "synthetic/revised-carve-out.toml",
            [43; 32],
            "revised independent NAV basis",
        )],
    );
    let changed = crate::evaluate(&revised).unwrap();
    assert_eq!(changed.summary(), report.summary());
    assert_ne!(changed.digest(), report.digest());
    let mut legacy = input;
    legacy.entity_valuations.clear();
    assert!(
        !crate::evaluate(&legacy)
            .unwrap()
            .to_json()
            .contains("\"entity_valuations\":")
    );
}

#[test]
fn secondary_overflow_refuses_even_when_the_candidate_cannot_win() {
    let mut input = static_example();
    edit_valuation(&mut input, 0, |value| {
        value.equity.initial_share_units = r(i128::MAX, 1);
        value.equity.shares_per_unit = r(2, 1);
    });
    assert_eq!(crate::evaluate(&input).unwrap_err().code, "FMINE-REF-207");
    let mut input = static_example();
    edit_valuation(&mut input, 0, |value| {
        value.equity.baseline_nav[0].1 = Money::from_minor(Currency::USD, i128::MAX);
    });
    assert_eq!(crate::evaluate(&input).unwrap_err().code, "FMINE-REF-207");
}

#[test]
fn valuation_row_bound_covers_static_and_complete_policy_populations() {
    let names: Vec<_> = (0..8).map(|index| format!("c{index}")).collect();
    let payoffs = vec![0; 128];
    let shares = vec![ZERO; 128];
    let rows: Vec<_> = names
        .iter()
        .map(|name| (name.as_str(), 0, payoffs.as_slice()))
        .collect();
    let valuations: Vec<_> = names
        .iter()
        .map(|name| (name.as_str(), payoffs.as_slice(), shares.as_slice()))
        .collect();
    let program = with_valuation(
        with_equity(
            inputs(&rows, &vec![r(1, 128); 128], 0, r(1, 5), None),
            &vec![1000; 128],
            r(100, 1),
        ),
        "carve-out",
        &vec![600; 128],
        r(60, 1),
        &valuations,
        None,
    );
    assert!(crate::validate(&program).is_ok()); // 256 * 128 * (primary + one secondary) = 65,536.
    let excessive = with_valuation(
        program,
        "other",
        &vec![600; 128],
        r(60, 1),
        &valuations,
        None,
    );
    assert_eq!(
        crate::evaluate(&excessive).unwrap_err().code,
        "FMINE-REF-201"
    );

    let count = 147;
    let zeroes = vec![0; count];
    let study_payoffs = vec![-1; count];
    let share_changes = vec![ZERO; count];
    let mut program = with_equity(
        with_schedule(inputs(
            &[
                ("study", 1, &study_payoffs),
                ("initial-a", 0, &zeroes),
                ("initial-b", 0, &zeroes),
                ("fair", 0, &zeroes),
                ("aggressive", 0, &zeroes),
            ],
            &vec![r(1, count as i128); count],
            1,
            r(1, 5),
            None,
        )),
        &vec![1000; count],
        r(100, 1),
    );
    let mut schedule = program.funding_schedule.as_ref().unwrap().value.clone();
    for (state, _) in &program.weights.value.states {
        set_candidate_flow(&mut schedule, "start", "study", state, -1);
    }
    program.funding_schedule = Some(schedule_claim(schedule));
    let values = ["study", "initial-a", "initial-b", "fair", "aggressive"]
        .map(|name| (name, zeroes.as_slice(), share_changes.as_slice()));
    for entity in ["one", "two", "three"] {
        program = with_valuation(program, entity, &vec![600; count], r(60, 1), &values, None);
    }
    let information = RecourseBasis {
        model_key: program.policy.value.model_key.clone(),
        currency: Currency::USD,
        valuation_basis: program.policy.value.valuation_basis.clone(),
        information_candidate: "study".into(),
        observation_date: "2030-07-01".into(),
        information_basis: "Synthetic two-class paid observation".into(),
        candidate_stages: program
            .candidates
            .iter()
            .map(|candidate| CandidateStage {
                candidate_id: candidate.value.candidate_id.clone(),
                stage: if ["fair", "aggressive"].contains(&candidate.value.candidate_id.as_str()) {
                    DecisionStage::Recourse
                } else {
                    DecisionStage::Initial
                },
            })
            .collect(),
        observations: program
            .weights
            .value
            .states
            .iter()
            .enumerate()
            .map(|(index, (state, _))| StateObservation {
                state_id: state.clone(),
                observation_id: if index == 0 { "good" } else { "bad" }.into(),
            })
            .collect(),
    };
    assert!(crate::validate(&program).is_ok()); // Static: 32 * 147 * 4 = 18,816.
    let input = RecourseInputs {
        program,
        information: information_claim(information),
    };
    // 32 static subsets + 80 policies: cash rows = 49,392; valuation rows = 65,856.
    assert_eq!(
        recourse::evaluate(&input).unwrap_err().code,
        "FMINE-REF-201"
    );
}
