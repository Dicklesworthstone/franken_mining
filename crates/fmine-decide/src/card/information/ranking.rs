//! Exact tail-risk comparison for act-now alternatives and one-test policies (BD-225/227).
//! Determinism: Exact
//!
//! This module deliberately does not collapse expected value and tail risk into one score. It ranks
//! signed-loss CVaR95 separately and retains the exact expected-value/CVaR95 Pareto frontier.

use std::cmp::Ordering;

use super::{ChoiceKind, InformationStats};
use crate::card::{DecisionSummary, ExactMoney, Result, ZERO, arithmetic, refusal};
use fmine_evidence::RefusalCode;
use fmine_numerics::exact_feas::Rat;

pub const RULE: &str = "Tail-risk order minimizes signed-loss CVaR95. The expected-value/CVaR95 frontier removes a choice only when another admitted choice has weakly higher expected value and weakly lower CVaR95 loss, with at least one strict improvement. Loss probability is retained as a separate descriptor and never silently scalarized with expected value or CVaR95. All ties are retained; kind/id order is presentation only.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskChoice {
    pub kind: ChoiceKind,
    pub id: String,
    pub expected_value: ExactMoney,
    pub cvar95_loss: ExactMoney,
    pub loss_probability: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskComparison {
    pub tail_risk_order: Vec<RiskChoice>,
    pub tail_risk_winners: Vec<RiskChoice>,
    pub expected_value_cvar95_frontier: Vec<RiskChoice>,
}

fn failed(detail: &str) -> fmine_evidence::Refusal {
    refusal(RefusalCode::NumericalCertificateFailed207, detail)
}

fn act_now_loss_probability(summary: &DecisionSummary, action_id: &str) -> Result<Rat> {
    let action = summary
        .alternatives
        .iter()
        .find(|a| a.action_id == action_id)
        .ok_or_else(|| failed("risk comparison action is absent"))?;
    let mut probability = ZERO;
    for state in &action.states {
        if state.payoff.currency != summary.currency {
            return Err(failed(
                "risk comparison action currency differs from the card",
            ));
        }
        if state.payoff.major.signum() < 0 {
            probability = arithmetic(probability.checked_add(state.weight))?;
        }
    }
    Ok(probability)
}

fn tail_cmp(a: &RiskChoice, b: &RiskChoice) -> Result<Ordering> {
    let risk = arithmetic(a.cvar95_loss.major.checked_cmp(b.cvar95_loss.major))?;
    if risk != Ordering::Equal {
        return Ok(risk);
    }
    let value = arithmetic(a.expected_value.major.checked_cmp(b.expected_value.major))?.reverse();
    Ok(value.then_with(|| (a.kind, &a.id).cmp(&(b.kind, &b.id))))
}

fn value_cmp(a: &RiskChoice, b: &RiskChoice) -> Result<Ordering> {
    let value = arithmetic(a.expected_value.major.checked_cmp(b.expected_value.major))?.reverse();
    if value != Ordering::Equal {
        return Ok(value);
    }
    let risk = arithmetic(a.cvar95_loss.major.checked_cmp(b.cvar95_loss.major))?;
    Ok(risk.then_with(|| (a.kind, &a.id).cmp(&(b.kind, &b.id))))
}

fn insertion_sort(
    rows: &mut [RiskChoice],
    cmp: impl Fn(&RiskChoice, &RiskChoice) -> Result<Ordering>,
) -> Result<()> {
    for i in 1..rows.len() {
        let mut j = i;
        while j > 0 && cmp(&rows[j], &rows[j - 1])? == Ordering::Less {
            rows.swap(j, j - 1);
            j -= 1;
        }
    }
    Ok(())
}

fn frontier(mut choices: Vec<RiskChoice>) -> Result<Vec<RiskChoice>> {
    let mut keep = Vec::new();
    for (i, candidate) in choices.iter().enumerate() {
        let mut dominated = false;
        for (j, other) in choices.iter().enumerate() {
            if i == j {
                continue;
            }
            let ev = arithmetic(
                other
                    .expected_value
                    .major
                    .checked_cmp(candidate.expected_value.major),
            )?;
            let risk = arithmetic(
                other
                    .cvar95_loss
                    .major
                    .checked_cmp(candidate.cvar95_loss.major),
            )?;
            if ev != Ordering::Less
                && risk != Ordering::Greater
                && (ev == Ordering::Greater || risk == Ordering::Less)
            {
                dominated = true;
                break;
            }
        }
        if !dominated {
            keep.push(candidate.clone());
        }
    }
    insertion_sort(&mut keep, value_cmp)?;
    choices.clear();
    Ok(keep)
}

pub(super) fn compare(
    base: &DecisionSummary,
    options: &[InformationStats],
) -> Result<RiskComparison> {
    let mut choices = Vec::with_capacity(base.alternatives.len() + options.len());
    for action in &base.alternatives {
        if action.mean.currency != base.currency || action.cvar95_loss.currency != base.currency {
            return Err(failed("risk comparison action has a currency mismatch"));
        }
        choices.push(RiskChoice {
            kind: ChoiceKind::ActNow,
            id: action.action_id.clone(),
            expected_value: action.mean,
            cvar95_loss: action.cvar95_loss,
            loss_probability: act_now_loss_probability(base, &action.action_id)?,
        });
    }
    for option in options {
        if option.net_value.currency != base.currency
            || option.policy_risk.cvar95_loss.currency != base.currency
        {
            return Err(failed("information-policy risk has a currency mismatch"));
        }
        choices.push(RiskChoice {
            kind: ChoiceKind::InformationOption,
            id: option.option_id.clone(),
            expected_value: option.net_value,
            cvar95_loss: option.policy_risk.cvar95_loss,
            loss_probability: option.policy_risk.loss_probability,
        });
    }
    if choices.is_empty() {
        return Err(failed("risk comparison has no choices"));
    }
    let mut tail_risk_order = choices.clone();
    insertion_sort(&mut tail_risk_order, tail_cmp)?;
    let best = tail_risk_order[0].cvar95_loss;
    let tail_risk_winners = tail_risk_order
        .iter()
        .take_while(|row| row.cvar95_loss == best)
        .cloned()
        .collect();
    let expected_value_cvar95_frontier = frontier(choices)?;
    Ok(RiskComparison {
        tail_risk_order,
        tail_risk_winners,
        expected_value_cvar95_frontier,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_units::Currency;

    fn r(n: i128, d: i128) -> Rat {
        Rat::new(n, d).unwrap()
    }
    fn money(n: i128) -> ExactMoney {
        ExactMoney {
            currency: Currency::USD,
            major: Rat::from_integer(n),
        }
    }
    fn choice(kind: ChoiceKind, id: &str, ev: i128, cvar: i128) -> RiskChoice {
        RiskChoice {
            kind,
            id: id.into(),
            expected_value: money(ev),
            cvar95_loss: money(cvar),
            loss_probability: ZERO,
        }
    }

    #[test]
    fn frontier_keeps_expected_value_tail_tradeoffs_and_drops_dominated_choices() {
        let rows = vec![
            choice(ChoiceKind::ActNow, "safe", 0, 0),
            choice(ChoiceKind::ActNow, "build", 100, 200),
            choice(ChoiceKind::InformationOption, "test", 131, 210),
            choice(ChoiceKind::InformationOption, "dominated", 90, 220),
        ];
        let frontier = frontier(rows).unwrap();
        assert_eq!(
            frontier
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["test", "build", "safe"]
        );
    }

    #[test]
    fn tail_ranking_retains_exact_ties_and_uses_value_only_for_presentation() {
        let mut rows = vec![
            choice(ChoiceKind::InformationOption, "z", 5, 10),
            choice(ChoiceKind::ActNow, "a", 7, 10),
            choice(ChoiceKind::ActNow, "safe", 0, 0),
        ];
        insertion_sort(&mut rows, tail_cmp).unwrap();
        assert_eq!(
            rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            ["safe", "a", "z"]
        );
        assert_eq!(rows[1].cvar95_loss, rows[2].cvar95_loss);
    }

    #[test]
    fn equal_expected_value_with_strictly_lower_tail_loss_dominates() {
        let rows = vec![
            choice(ChoiceKind::ActNow, "better", 10, 4),
            choice(ChoiceKind::InformationOption, "worse", 10, 5),
        ];
        let frontier = frontier(rows).unwrap();
        assert_eq!(frontier.len(), 1);
        assert_eq!(frontier[0].id, "better");
    }

    #[test]
    fn exact_loss_probability_distinguishes_zero_from_negative_payoff() {
        let mut summary = DecisionSummary {
            model_key: "m".into(),
            question: "q".into(),
            currency: Currency::USD,
            valuation_basis: "v".into(),
            alternatives: vec![],
            expected_value_order: vec![],
            expected_value_winners: vec![],
            expected_value_primary: "a".into(),
            runner_up: Some("b".into()),
            minimax_regret_order: vec![],
            minimax_regret_winners: vec![],
            minimax_regret_primary: "a".into(),
            information: None,
            robustness: None,
        };
        summary.alternatives.push(crate::card::AlternativeStats {
            action_id: "a".into(),
            label: "a".into(),
            role: crate::card::ActionRole::Action,
            mean: money(0),
            p10_p50_p90: [money(0); 3],
            cvar95_loss: money(0),
            mean_regret: money(0),
            max_regret: money(0),
            states: vec![
                crate::card::StateResult {
                    state_id: "loss".into(),
                    weight: r(1, 4),
                    payoff: money(-1),
                    best_in_hindsight: money(0),
                    regret: money(1),
                },
                crate::card::StateResult {
                    state_id: "zero".into(),
                    weight: r(3, 4),
                    payoff: money(0),
                    best_in_hindsight: money(0),
                    regret: money(0),
                },
            ],
        });
        assert_eq!(act_now_loss_probability(&summary, "a").unwrap(), r(1, 4));
    }
}
