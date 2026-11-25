//! DESIGN (bd-g5nh): a meeting view of the exact static capital comparison.
//! Determinism: Exact

use std::cmp::Ordering::{Equal, Greater, Less};
use std::fmt::Write as _;

use super::{evidence, md, ratio};
use crate::robustness::kpi::{Aggregation, Direction, EvaluatedKpiProgram};
use crate::{
    CASH_PAYOFF_CONVENTION, EquityBasis, EvaluatedProgram, Money, ProgramChoice, ProgramSummary,
    Rat, RefusalCode, Result, arithmetic, bad, exact_compare,
};

mod recourse;

struct TailStep<'a> {
    limit: Rat,
    mean: Rat,
    choices: Vec<&'a ProgramChoice>,
}

/// Find the first representable cap that admits a strictly better bundle.
/// Structural rejections never enter this calculation.
fn next_tail_step(s: &ProgramSummary, current_mean: Option<Rat>) -> Result<Option<TailStep<'_>>> {
    let mut first: Option<TailStep<'_>> = None;
    for choice in &s.choices {
        if !choice.structural_rejections.is_empty() || !choice.risk_rejected {
            continue;
        }
        let (Some(stats), Some(tail)) = (&choice.statistics, choice.tail_loss) else {
            return Err(bad(
                RefusalCode::NumericalCertificateFailed207,
                "structurally feasible bundle lacks its valuation or tail result",
            ));
        };
        let mean = stats.mean.major;
        if let Some(current) = current_mean
            && arithmetic(mean.checked_cmp(current))? != Greater
        {
            continue;
        }
        let scaled = arithmetic(tail.major.checked_mul(Rat::from_integer(Money::SCALE)))?;
        // A scenario-weighted tail loss can fall between monetary minor units.
        // Round toward +infinity to the first accepted worksheet limit.
        let cap_minor = scaled.num() / scaled.den() + i128::from(scaled.num() % scaled.den() > 0);
        let limit = arithmetic(Rat::new(cap_minor, Money::SCALE))?;
        if let Some(step) = &mut first {
            match arithmetic(limit.checked_cmp(step.limit))? {
                Greater => continue,
                Equal => {
                    match arithmetic(mean.checked_cmp(step.mean))? {
                        Greater => {
                            step.mean = mean;
                            step.choices.clear();
                            step.choices.push(choice);
                        }
                        Equal => step.choices.push(choice),
                        Less => {}
                    }
                    continue;
                }
                Less => {}
            }
        }
        first = Some(TailStep {
            limit,
            mean,
            choices: vec![choice],
        });
    }
    Ok(first)
}

fn name(choice: &ProgramChoice) -> String {
    if choice.selected.is_empty() {
        "Hold cash".into()
    } else {
        md(&choice.selected.join(" + "))
    }
}

fn names(s: &ProgramSummary, ids: &[String]) -> String {
    if ids.is_empty() {
        return "none".into();
    }
    ids.iter()
        .filter_map(|id| s.choices.iter().find(|row| row.bundle_id == *id))
        .map(name)
        .collect::<Vec<_>>()
        .join(", ")
}

fn usd(value: Rat) -> String {
    // Render an exact four-place decimal where possible, otherwise keep the ratio.
    let Ok(minor) = value.checked_mul(Rat::from_integer(Money::SCALE)) else {
        return format!("{} USD", ratio(value));
    };
    if minor.den() != 1 {
        return format!("{} USD", ratio(value));
    }
    let n = minor.num().unsigned_abs();
    let scale = Money::SCALE as u128;
    let units = (n / scale).to_string();
    let mut text = String::from(if value.signum() < 0 { "-US$" } else { "US$" });
    for (i, digit) in units.bytes().enumerate() {
        if i != 0 && (units.len() - i).is_multiple_of(3) {
            text.push(',');
        }
        text.push(char::from(digit));
    }
    if n % scale != 0 {
        let fraction = format!("{:04}", n % scale);
        text.push('.');
        text.push_str(fraction.trim_end_matches('0'));
    }
    text
}

fn major(money: Money) -> Result<Rat> {
    arithmetic(Rat::new(money.minor, Money::SCALE))
}

fn stats(choice: &ProgramChoice) -> String {
    let mean = choice
        .statistics
        .as_ref()
        .map_or_else(|| "—".into(), |s| usd(s.mean.major));
    let tail = choice
        .tail_loss
        .map_or_else(|| "—".into(), |v| usd(v.major));
    let slack = choice
        .funding
        .as_ref()
        .map(|r| r.minimum_slack)
        .or_else(|| choice.funding_schedule.as_ref().map(|r| r.minimum_slack))
        .or_else(|| choice.entity_funding.as_ref().map(|r| r.minimum_slack))
        .map_or_else(|| "—".into(), |v| usd(v.major));
    format!(
        "{mean} | {tail} | {} | {} | {slack}",
        usd(choice.spend.major),
        usd(choice.budget_slack.major)
    )
}

// DESIGN (bd-229.4). For a given entity, other caps still screen the same jointly
// structural population. Exact per-share loss is itself an admissible cap.
fn next_equity_step<'a>(
    summary: &'a ProgramSummary,
    primary: &EquityBasis,
    entity: &EquityBasis,
    cap: Rat,
    current: Option<Rat>,
) -> Option<TailStep<'a>> {
    let choices = summary
        .choices
        .iter()
        .filter(|row| row.structural_rejections.is_empty())
        .filter_map(|row| {
            let mean = row.per_share_statistics.as_ref()?.mean.major;
            let primary_loss = row.per_share_tail_loss?.major;
            if current.is_some_and(|now| !exact_compare(mean, now).is_gt())
                || (entity.entity != primary.entity
                    && primary
                        .max_tail_loss_per_share
                        .is_some_and(|limit| exact_compare(primary_loss, limit).is_gt()))
                || row
                    .entity_valuations
                    .iter()
                    .any(|other| other.entity != entity.entity && other.risk_rejected)
            {
                return None;
            }
            let loss = if entity.entity == primary.entity {
                primary_loss
            } else {
                row.entity_valuations
                    .iter()
                    .find(|other| other.entity == entity.entity)?
                    .per_share_tail_loss?
                    .major
            };
            exact_compare(loss, cap)
                .is_gt()
                .then_some((loss, mean, row))
        })
        .collect::<Vec<_>>();
    let &(limit, mean, _) = choices
        .iter()
        .min_by(|a, b| exact_compare(a.0, b.0).then_with(|| exact_compare(b.1, a.1)))?;
    Some(TailStep {
        limit,
        mean,
        choices: choices
            .into_iter()
            .filter_map(|(loss, value, row)| (loss == limit && value == mean).then_some(row))
            .collect(),
    })
}

fn equity_brief(program: &EvaluatedProgram, primary: &EquityBasis) -> Result<String> {
    let summary = program.summary();
    let mean_for = |winners: &[String]| {
        summary.choices.iter().find_map(|row| {
            if winners.contains(&row.bundle_id) {
                row.per_share_statistics
                    .as_ref()
                    .map(|stats| stats.mean.major)
            } else {
                None
            }
        })
    };
    let current = mean_for(&summary.feasible_winners);
    let value = |qty: Option<Rat>| {
        qty.map_or_else(
            || "unavailable".into(),
            |v| format!("{} USD/share", ratio(v)),
        )
    };
    let mut out = format!(
        "# Static equity capital brief\n\n{}\n\n**Primary objective** Expected ΔNAV per actual share of {} ({}). Budget {}.\n\n| Comparison | Co-winners | Expected ΔNAV per share |\n|---|---|---:|\n| Before nominal tail caps | {} | {} |\n| All nominal tail caps | {} | {} |\n\n**Expected value given up** {}.\n",
        md(&summary.question),
        md(&primary.entity),
        md(&primary.share_class),
        usd(major(summary.policy.budget)?),
        names(summary, &summary.unconstrained_winners),
        value(mean_for(&summary.unconstrained_winners)),
        names(summary, &summary.feasible_winners),
        value(current),
        value(
            summary
                .expected_per_share_cost_of_risk_limit
                .map(|v| v.major)
        ),
    );
    let _ = writeln!(
        out,
        "\n## Nominal tail limits by entity\n\nTail loss averages signed losses over the worst {} of declared scenario mass. Each step varies one cap subject to the other caps and structural constraints.",
        ratio(summary.policy.tail_mass),
    );
    for equity in
        std::iter::once(primary).chain(summary.entity_valuations.iter().map(|v| &v.equity))
    {
        let entity = md(&equity.entity);
        let Some(cap) = equity.max_tail_loss_per_share else {
            let _ = writeln!(out, "\n- **{entity}** No declared cap.");
            continue;
        };
        let _ = writeln!(
            out,
            "\n- **{entity}** Inclusive cap **{}**.",
            value(Some(cap))
        );
        if let Some(step) = next_equity_step(summary, primary, equity, cap, current) {
            let choices = step
                .choices
                .iter()
                .map(|row| name(row))
                .collect::<Vec<_>>()
                .join(", ");
            let label = if current.is_some() {
                "First higher-value cap"
            } else {
                "First feasible cap"
            };
            let _ = writeln!(
                out,
                "  **{label}** {}. Co-winners: {choices}. Expected primary ΔNAV {}.",
                value(Some(step.limit)),
                value(Some(step.mean)),
            );
        } else if current.is_none() {
            out.push_str("  No feasible bundle from loosening this cap alone.\n");
        } else {
            out.push_str("  No higher primary value from loosening this cap alone.\n");
        }
    }
    if summary.robustness.is_some() || summary.selection_stability.is_some() {
        out.push_str("\nThe full comparison includes the declared scenario-weight stress or resampling results.\n");
    }
    let _ = writeln!(
        out,
        "\n**Primary valuation basis** {}\n\n**Reference-class basis** {}\n\nEvidence class {}. Scenario weights are declared assumptions.\n\nComparison digest `{}`.",
        md(&primary.valuation_basis),
        md(&summary.policy.reference_class_basis),
        evidence(program.claim().evidence),
        program.digest(),
    );
    Ok(out)
}

impl EvaluatedKpiProgram {
    /// DESIGN (bd-229.3). Render choices and the first decision-relevant row
    /// of each already evaluated one-limit frontier.
    pub fn to_brief(&self) -> String {
        let result = self.summary();
        let original = self.static_program().summary();
        let exact = |qty: Rat| {
            if qty.den() == 1 {
                qty.num().to_string()
            } else {
                ratio(qty)
            }
        };
        let value = |qty: Option<Rat>| {
            qty.map_or_else(
                || "unavailable".into(),
                |qty| format!("{} {}", exact(qty), result.objective_unit),
            )
        };
        let mut out = format!(
            "# KPI capital brief\n\n{}\n\n| Comparison | Co-winners | Expected value |\n|---|---|---:|\n| Original eligibility | {} | {} |\n| All declared KPI limits | {} | {} |\n\n**Expected value given up** {}.\n",
            md(&result.model_key),
            names(original, &result.baseline_winners),
            value(result.baseline_objective),
            names(original, &result.constrained_winners),
            value(result.constrained_objective),
            value(result.opportunity_cost),
        );
        out.push_str("\n## Decision thresholds\n\nEach step varies one limit subject to every other declared constraint.\n");
        // Evaluation builds both in the same canonical constraint order.
        for (constraint, frontier) in self
            .design()
            .value
            .constraints
            .iter()
            .zip(&result.frontiers)
        {
            let direction = match constraint.direction {
                Direction::Upper => "cap",
                Direction::Lower => "floor",
            };
            let aggregation = match constraint.aggregation {
                Aggregation::RatioOfExpectedTotals => "ratio of expected totals",
                Aggregation::EveryState => "every original state",
            };
            let unit = constraint.metric.unit();
            let _ = writeln!(
                out,
                "\n- **{}** ({}) uses an inclusive {direction} of **{} {unit}** for {} over {}. Basis: {}.",
                md(&constraint.id),
                constraint.metric.as_str(),
                exact(constraint.limit),
                aggregation,
                md(&constraint.period),
                md(&constraint.basis),
            );
            let current = result.constrained_objective;
            let next = frontier
                .points
                .iter()
                .skip_while(|point| !point.at_declared_limit)
                .skip(1)
                .find(|point| {
                    point.objective_mean.is_some_and(|next| {
                        current.is_none_or(|now| exact_compare(next, now).is_gt())
                    })
                });
            let step = match (current, next) {
                (None, Some(_)) => "First feasible limit",
                (Some(_), Some(_)) => "First higher-value limit",
                (None, None) => "No feasible bundle from loosening this limit alone",
                (Some(_), None) => "No higher value from loosening this limit alone",
            };
            if let Some(point) = next {
                let _ = writeln!(
                    out,
                    "  **{step}** {} {unit}. Co-winners: {}. Expected value {}.",
                    exact(point.limit),
                    names(original, &point.winners),
                    value(point.objective_mean),
                );
            } else {
                let _ = writeln!(out, "  {step}.");
            }
        }
        let _ = writeln!(out, "\nComparison digest `{}`.", self.digest());
        out
    }
}

impl EvaluatedProgram {
    /// Render the static meeting view and the first exact improvement if a
    /// declared nominal tail-loss limit moves.
    ///
    /// # Errors
    /// Refuses an unrepresentable exact comparison.
    pub fn to_brief(&self) -> Result<String> {
        let s = self.summary();
        if let Some(equity) = &s.equity {
            return equity_brief(self, equity);
        }
        let limit = s.policy.max_tail_loss.map(major).transpose()?;
        let current_mean = s
            .choices
            .iter()
            .find(|row| s.feasible_winners.contains(&row.bundle_id))
            .and_then(|row| row.statistics.as_ref().map(|stats| stats.mean.major));
        let step = if limit.is_some() {
            next_tail_step(s, current_mean)?
        } else {
            None
        };
        let heading = if s.policy.payoff_convention == CASH_PAYOFF_CONVENTION {
            "Mean discounted incremental cash"
        } else {
            "Mean net ΔNAV"
        };
        let before = limit.map_or_else(String::new, |_| {
            format!(
                " **Before the tail-loss limit:** {}.",
                names(s, &s.unconstrained_winners)
            )
        });
        let mut out = format!(
            "# Capital program brief\n\n{}\n\n**Current co-winners:** {}.{before}\n\nBudget {}. Tail loss is the average signed loss over the worst {} of declared scenario mass. Current limit {}.\n",
            md(&s.question),
            names(s, &s.feasible_winners),
            usd(major(s.policy.budget)?),
            ratio(s.policy.tail_mass),
            limit.map_or_else(|| "unbounded".into(), usd)
        );
        if let Some(limit) = limit {
            if let Some(cost) = s.expected_value_cost_of_risk_limit {
                let _ = writeln!(
                    out,
                    "\n**Expected value given up under the current limit:** {}.",
                    usd(cost.major)
                );
            }
            if let Some(step) = &step {
                let names = step
                    .choices
                    .iter()
                    .map(|row| name(row))
                    .collect::<Vec<_>>()
                    .join(", ");
                let increase = usd(arithmetic(step.limit.checked_sub(limit))?);
                if let Some(mean) = current_mean {
                    let gain = usd(arithmetic(step.mean.checked_sub(mean))?);
                    let _ = writeln!(
                        out,
                        "\n**First higher-value limit:** {} ({increase} more). {names} would add {gain} in modeled mean value.",
                        usd(step.limit)
                    );
                } else {
                    let _ = writeln!(
                        out,
                        "\n**First feasible limit:** {} ({increase} more). {names} would enter at a modeled mean of {}.",
                        usd(step.limit),
                        usd(step.mean)
                    );
                }
            } else if current_mean.is_some() {
                out.push_str("\nRelaxing this limit yields no strictly higher modeled mean within the other declared constraints.\n");
            } else {
                out.push_str(
                    "\nEvery package, including holding cash, fails another declared constraint.\n",
                );
            }
        }

        let _ = writeln!(
            out,
            "\n| Package | {heading} | Tail loss | Spend | Budget slack | Minimum cash slack | Status |\n|---|---:|---:|---:|---:|---:|---|"
        );
        for row in &s.choices {
            let current = s.feasible_winners.contains(&row.bundle_id);
            let before = s.unconstrained_winners.contains(&row.bundle_id);
            let enters = step.as_ref().is_some_and(|v| {
                v.choices
                    .iter()
                    .any(|choice| choice.bundle_id == row.bundle_id)
            });
            if !(current || before || enters || row.selected.is_empty()) {
                continue;
            }
            let status = if !row.structural_rejections.is_empty() {
                "Fails declared constraints"
            } else if current {
                "Current co-winner"
            } else if enters && current_mean.is_some() {
                "First increase in modeled mean"
            } else if enters {
                "Feasible at the first higher limit"
            } else if row.risk_rejected {
                "Exceeds the current limit"
            } else {
                "Feasible"
            };
            let _ = writeln!(out, "| {} | {} | {status} |", name(row), stats(row));
        }
        if s.funding.is_none() && s.funding_schedule.is_none() && s.entity_funding.is_none() {
            out.push_str("\nCash slack is unavailable for this worksheet.\n");
        }
        if let Some(stress) = &s.robustness {
            let _ = writeln!(
                out,
                "\n**Under the declared scenario-weight stress**\n\nAmong packages that pass the nominal limit, worst-mean preference {} and lowest worst expected regret {}.",
                names(s, &stress.maximin_mean_winners),
                names(s, &stress.minimax_expected_regret_winners)
            );
            if stress.declared_tail_limits > 0 {
                let _ = writeln!(
                    out,
                    "Worst-mean preference among packages that satisfy the tail limit under every stressed law {}.",
                    names(s, &stress.robust_tail_maximin_mean_winners)
                );
            }
        }
        if let Some(stability) = &s.selection_stability {
            let fragile = stability
                .alternatives
                .iter()
                .filter(|row| row.fragile)
                .map(|row| row.alternative_id.clone())
                .collect::<Vec<_>>();
            let _ = writeln!(
                out,
                "\n**Conditional resampling**\n\n{} of {} replicates had no feasible package. Nominal co-winners below 4/5 selection frequency {}.",
                stability.no_feasible_replicates,
                stability.design.replicates,
                names(s, &fragile)
            );
        }
        let _ = writeln!(
            out,
            "\n**Valuation basis** {}\n\n**Reference-class basis** {}\n\nEvidence class {}. Scenario weights are declared assumptions.\n\nComparison digest `{}`.",
            md(&s.policy.valuation_basis),
            md(&s.policy.reference_class_basis),
            evidence(self.claim().evidence),
            self.digest()
        );
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::{anchor::SourceAnchor, hash::sha256};

    const GB30: &str = include_str!("../../../../tests/capital/gb30.toml");

    fn program(text: &str) -> EvaluatedProgram {
        let anchor = SourceAnchor::new(
            "synthetic-program.toml",
            sha256(text.as_bytes()).0,
            "worksheet",
        );
        let crate::model::EvaluatedModel::Static(result) =
            crate::model::evaluate(text, anchor).expect("static synthetic program")
        else {
            panic!("expected static program");
        };
        *result
    }

    fn equity_fixture(with_funding: bool, parent: &str, child: &str) -> EvaluatedProgram {
        let source = include_str!("../../../../tests/capital/entity_valuation.toml");
        let end = if with_funding {
            "[[recourse]]"
        } else {
            "[[entity_funding]]"
        };
        let (terms, _) = source.split_once(end).expect("end of static terms");
        let (_, uu) = source.split_once("[[uu]]").expect("named uncertainty");
        let cap = "max_tail_loss_per_share = \"1/10\"";
        let (start, rest) = terms.split_once(cap).expect("primary cap");
        let rest = rest.replacen(cap, &format!("max_tail_loss_per_share = \"{child}\""), 1);
        program(&format!(
            "{start}max_tail_loss_per_share = \"{parent}\"{rest}[[uu]]{uu}"
        ))
    }

    #[test]
    fn equity_brief_finds_each_exact_cap_subject_to_the_other_entity() {
        let tight = equity_fixture(false, "1/10", "1/10");
        let brief = tight.to_brief().expect("jointly constrained equity");
        assert_eq!(
            brief
                .matches("No higher primary value from loosening this cap alone.")
                .count(),
            2
        );
        assert!(brief.contains("**Expected value given up** 3/10 USD/share."));

        let child = equity_fixture(false, "1/1", "1/10");
        let brief = child.to_brief().expect("secondary cap binds");
        assert!(brief.contains("**First higher-value cap** 20/11 USD/share. Co-winners: fair + study. Expected primary ΔNAV 1/5 USD/share."));
        let below = equity_fixture(false, "1/1", "909/500");
        assert_eq!(below.summary().feasible_winners, ["bundle_00"]);
        let at = equity_fixture(false, "1/1", "20/11");
        assert_eq!(
            names(at.summary(), &at.summary().feasible_winners),
            "fair + study"
        );

        let parent = equity_fixture(false, "1/10", "4/1");
        let brief = parent.to_brief().expect("primary cap binds");
        assert!(brief.contains("**First higher-value cap** 1/2 USD/share. Co-winners: fair + study. Expected primary ΔNAV 1/5 USD/share."));
        assert_eq!(brief.matches("**First higher-value cap**").count(), 1);
    }

    #[test]
    fn equity_brief_handles_empty_choices_and_structural_exclusions() {
        let empty = equity_fixture(false, "1/1", "-1/1");
        let brief = empty.to_brief().expect("empty equity comparison");
        assert!(brief.contains("| All nominal tail caps | none | unavailable |"));
        assert!(brief.contains("**Expected value given up** unavailable."));
        assert!(brief.contains("**First feasible cap** 0/1 USD/share. Co-winners: Hold cash."));
        let funded = equity_fixture(true, "1/1", "1/10");
        let brief = funded
            .to_brief()
            .expect("cash constrains the terminal packages");
        assert!(brief.contains("| Before nominal tail caps | Hold cash | 0/1 USD/share |"));
        assert_eq!(
            brief
                .matches("No higher primary value from loosening this cap alone.")
                .count(),
            2
        );
    }

    #[test]
    fn tail_breakpoint_agrees_with_the_full_solver_at_and_just_below_the_limit() {
        let below = program(&GB30.replace(
            "max_tail_loss = \"150000000\"",
            "max_tail_loss = \"209999999.9999\"",
        ));
        let step = next_tail_step(below.summary(), Some(Rat::from_integer(241_180_000)))
            .expect("valid threshold arithmetic")
            .expect("one improvement");
        assert_eq!(step.limit, Rat::from_integer(210_000_000));
        assert_eq!(step.choices[0].selected, ["A", "E"]);
        let boundary = program(&GB30.replace(
            "max_tail_loss = \"150000000\"",
            "max_tail_loss = \"210000000\"",
        ));
        assert_eq!(
            boundary.summary().feasible_winners,
            [step.choices[0].bundle_id.clone()]
        );
        assert!(
            boundary
                .to_brief()
                .expect("brief")
                .contains("Relaxing this limit yields no strictly higher")
        );
    }

    #[test]
    fn threshold_respects_budget_and_can_name_the_first_feasible_cash_choice() {
        let constrained =
            program(&GB30.replace("budget = \"800000000\"", "budget = \"400000000\""));
        let brief = constrained.to_brief().expect("brief");
        assert!(brief.contains("**Current co-winners:** A."));
        assert!(brief.contains("**First higher-value limit:** US$330,000,000"));
        assert!(!brief.contains("A + E"));
        let all_fail =
            program(&GB30.replace("max_tail_loss = \"150000000\"", "max_tail_loss = \"-1\""));
        let brief = all_fail.to_brief().expect("brief");
        assert!(brief.contains("**Current co-winners:** none."));
        assert!(brief.contains("**First feasible limit:** US$0 (US$1 more). Hold cash"));
    }

    #[test]
    fn first_enterable_money_cap_ranks_every_bundle_admitted_at_that_cap() {
        const TERMS: &str = r#"
schema = "fmine.capital-program.v1"
model_key = "fractional-tail-loss"
question = "Which new bundle enters at the first permitted USD limit?"
currency = "USD"
valuation_basis = "Explicit synthetic net delta-NAV"
budget = "1"
tail_mass = "1/2"
max_tail_loss = "0"
payoff_convention = "additive_net_delta_nav"
reference_class_basis = "Declared synthetic"
[[state]]
id = "loss"
weight = "1/10"
[[state]]
id = "flat"
weight = "2/5"
[[state]]
id = "up"
weight = "1/2"
[[candidate]]
id = "a"
label = "A"
cost = "1"
reference_class_basis = "Declared synthetic"
[[candidate]]
id = "b"
label = "B"
cost = "1"
reference_class_basis = "Declared synthetic"
[[payoff]]
candidate = "a"
state = "loss"
value = "-0.0002"
[[payoff]]
candidate = "a"
state = "flat"
value = "0"
[[payoff]]
candidate = "a"
state = "up"
value = "10"
[[payoff]]
candidate = "b"
state = "loss"
value = "-0.0001"
[[payoff]]
candidate = "b"
state = "flat"
value = "-0.0001"
[[payoff]]
candidate = "b"
state = "up"
value = "20"
[[uu]]
id = "uncertainty"
description = "Illustrative omitted regime"
trigger = "Original output basis changes"
downside_usd_m = "0.01"
"#;
        let original = program(TERMS);
        let step = next_tail_step(original.summary(), Some(Rat::from_integer(0)))
            .expect("valid threshold arithmetic")
            .expect("improvement");
        // A's 0.00004 loss and B's 0.0001 loss both enter at the first
        // representable step. B has the higher mean and wins at that cap.
        assert_eq!(step.limit, Rat::new(1, 10_000).expect("one minor unit"));
        assert_eq!(step.choices[0].selected, ["b"]);
        let repriced =
            program(&TERMS.replace("max_tail_loss = \"0\"", "max_tail_loss = \"0.0001\""));
        assert_eq!(
            repriced.summary().feasible_winners,
            [step.choices[0].bundle_id.clone()]
        );
    }
}
