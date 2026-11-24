//! DESIGN (bd-u91v): meeting view of the evaluated two-stage capital policy.
//! Determinism: Exact

use super::{Result, evidence, major, md, names, ratio, usd};
use crate::Rat;
use crate::recourse::{EvaluatedRecourseProgram, PolicyChoice};
use std::fmt::Write as _;

fn listed(ids: &[String], separator: &str, empty: &str) -> String {
    if ids.is_empty() {
        empty.into()
    } else {
        md(&ids.join(separator))
    }
}

fn value(amount: Option<Rat>, per_share: bool) -> String {
    amount.map_or_else(
        || "unavailable".into(),
        |amount| {
            if per_share {
                format!("{} USD/share", ratio(amount))
            } else {
                usd(amount)
            }
        },
    )
}

fn policy_outcome(policy: &PolicyChoice, per_share: bool) -> (String, String, String) {
    let (mean, loss) = if per_share {
        (
            policy.per_share_statistics.as_ref().map(|s| s.mean.major),
            policy.per_share_tail_loss.map(|loss| loss.major),
        )
    } else {
        (
            policy.statistics.as_ref().map(|s| s.mean.major),
            policy.tail_loss.map(|loss| loss.major),
        )
    };
    let cash = policy
        .funding_schedule
        .as_ref()
        .map(|f| f.minimum_slack.major)
        .or_else(|| {
            policy
                .entity_funding
                .as_ref()
                .map(|f| f.minimum_slack.major)
        });
    (
        value(mean, per_share),
        value(loss, per_share),
        value(cash, false),
    )
}

fn holds_cash(policy: &PolicyChoice) -> bool {
    policy.initial_selected.is_empty() && policy.branches.iter().all(|b| b.selected.is_empty())
}

impl EvaluatedRecourseProgram {
    /// Render the evaluated decisions at their actual observation boundary.
    /// The complete policy population and state ledgers remain in the full reports.
    ///
    /// # Errors
    /// Refuses a budget or total-USD risk limit that cannot be represented exactly.
    pub fn to_brief(&self) -> Result<String> {
        let summary = self.summary();
        let baseline = self.static_program().summary();
        let basis = &summary.information;
        let per_share = summary.per_share_objective;
        let (limit, adaptive, risk_cost, objective) = if let Some(equity) = &baseline.equity {
            (
                equity.max_tail_loss_per_share,
                summary.net_adaptive_value_per_share.map(|v| v.major),
                summary
                    .expected_per_share_cost_of_risk_limit
                    .map(|v| v.major),
                format!(
                    "Mean ΔNAV per share of {} ({})",
                    md(&equity.entity),
                    md(&equity.share_class)
                ),
            )
        } else {
            (
                baseline.policy.max_tail_loss.map(major).transpose()?,
                summary.net_adaptive_value.map(|v| v.major),
                summary.expected_value_cost_of_risk_limit.map(|v| v.major),
                "Mean net ΔNAV".into(),
            )
        };
        let mut out = format!(
            "# Two-stage capital brief\n\n{}\n\n**Current policy co-winners** {}.\n\n**Best precommitted choice** {}. **Net adaptive value** {}.\n\n{objective}. Budget on every path {}. Tail loss is the average signed loss over the worst {} of the complete declared distribution. Primary limit {}.\n",
            md(&summary.question),
            listed(&summary.feasible_winners, ", ", "none"),
            names(baseline, &baseline.feasible_winners),
            value(adaptive, per_share),
            usd(major(baseline.policy.budget)?),
            ratio(baseline.policy.tail_mass),
            limit.map_or_else(|| "unbounded".into(), |v| value(Some(v), per_share)),
        );
        let secondary_limits = baseline
            .entity_valuations
            .iter()
            .any(|v| v.equity.max_tail_loss_per_share.is_some());
        if summary.feasible_winners.is_empty() {
            out.push_str(if summary.unconstrained_winners.is_empty() {
                "\nEvery policy fails at least one declared structural constraint.\n"
            } else {
                "\nEvery structurally feasible policy fails at least one declared tail-loss limit.\n"
            });
        }
        if limit.is_some() || secondary_limits {
            let _ = writeln!(
                out,
                "\n**Before the tail-loss limits** {}. **Expected value given up** {}.",
                listed(&summary.unconstrained_winners, ", ", "none"),
                value(risk_cost, per_share)
            );
        }
        let shortlist: Vec<_> = summary
            .policies
            .iter()
            .filter(|p| {
                holds_cash(p)
                    || summary.feasible_winners.contains(&p.policy_id)
                    || summary.unconstrained_winners.contains(&p.policy_id)
            })
            .collect();
        out.push_str("\n| Policy | Mean | Primary tail loss | Initial planned cost | Minimum cash slack | Status |\n|---|---:|---:|---:|---:|---|\n");
        for policy in &shortlist {
            let current = summary.feasible_winners.contains(&policy.policy_id);
            let status = if !policy.structural_rejections.is_empty() {
                "Fails structural constraints"
            } else if policy.risk_rejected {
                "Fails a tail-loss limit"
            } else if current {
                "Current co-winner"
            } else {
                "Feasible"
            };
            let (mean, risk, cash) = policy_outcome(policy, per_share);
            let label = if holds_cash(policy) {
                format!("{} (hold cash)", md(&policy.policy_id))
            } else {
                md(&policy.policy_id)
            };
            let _ = writeln!(
                out,
                "| {label} | {mean} | {risk} | {} | {cash} | {status} |",
                usd(policy.initial_spend.major)
            );
        }
        if secondary_limits {
            out.push_str("\n| Policy | Additional equity risk limit | Tail loss | Limit | Status |\n|---|---|---:|---:|---|\n");
            for policy in &shortlist {
                for row in &policy.entity_valuations {
                    let Some(cap) = row.max_tail_loss_per_share else {
                        continue;
                    };
                    let status = match row.per_share_tail_loss {
                        None => "Unavailable",
                        Some(_) if row.risk_rejected => "Fails",
                        Some(_) => "Passes",
                    };
                    let _ = writeln!(
                        out,
                        "| {} | {} ({}) | {} | {} | {status} |",
                        md(&policy.policy_id),
                        md(&row.entity),
                        md(&row.share_class),
                        value(row.per_share_tail_loss.map(|v| v.major), true),
                        value(Some(cap.major), true)
                    );
                }
            }
        }
        for policy in summary
            .policies
            .iter()
            .filter(|p| summary.feasible_winners.contains(&p.policy_id))
        {
            let _ = writeln!(
                out,
                "\n## Actions for {}\n\n**Initial commitment** {}. **Initial planned cost** {}.",
                md(&policy.policy_id),
                listed(&policy.initial_selected, " + ", "None"),
                usd(policy.initial_spend.major)
            );
            if !policy.initial_selected.is_empty() {
                out.push_str("\nInitial commitments carry into every follow-on branch.\n");
            }
            if policy.information_acquired {
                let _ = writeln!(
                    out,
                    "\n**Review on {}**. {}",
                    md(&basis.observation_date),
                    md(&basis.information_basis)
                );
            } else {
                out.push_str("\nThis policy selects one follow-on action for all scenarios without buying the information gate.\n");
            }
            out.push_str("\n| Observation class | Assumed weight | Follow-on commitment | Additional planned cost |\n|---|---:|---|---:|\n");
            for branch in &policy.branches {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} |",
                    branch
                        .observation_id
                        .as_deref()
                        .map_or_else(|| "All scenarios".into(), md),
                    ratio(branch.probability),
                    listed(&branch.selected, " + ", "No additional commitment"),
                    usd(branch.spend.major)
                );
            }
        }
        if let Some(permits) = &self.inputs().program.permits {
            out.push_str("\n**Permit conditions at the declared review**\n\n| Application | State | Review result | Declared clock deadline |\n|---|---|---|---|\n");
            for ((application, state), review) in &baseline.permit_reviews {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} |",
                    md(application),
                    md(state),
                    review.state.label().replace('_', " "),
                    review.clock_deadline.as_deref().unwrap_or("unresolved")
                );
            }
            for (candidate, application) in &permits.value.requirements {
                let _ = writeln!(
                    out,
                    "\n{} requires documented permission for {}.",
                    md(candidate),
                    md(application)
                );
            }
        }
        if let Some(stress) = &summary.robustness {
            let _ = writeln!(
                out,
                "\n**Declared scenario-weight stress**\n\nAmong nominally eligible policies, worst-mean preference {} and lowest worst expected regret {}.",
                listed(&stress.maximin_mean_winners, ", ", "none"),
                listed(&stress.minimax_expected_regret_winners, ", ", "none")
            );
            if stress.declared_tail_limits > 0 {
                let _ = writeln!(
                    out,
                    "Worst-mean preference among policies meeting all tail limits under every stressed law {}.",
                    listed(&stress.robust_tail_maximin_mean_winners, ", ", "none")
                );
            }
        }
        if let Some(stability) = &summary.selection_stability {
            let fragile: Vec<_> = stability
                .alternatives
                .iter()
                .filter(|row| row.fragile)
                .map(|row| row.alternative_id.clone())
                .collect();
            let _ = writeln!(
                out,
                "\n**Conditional resampling**\n\n{} of {} replicates had no feasible policy. Nominal co-winners below 4/5 selection frequency {}.",
                stability.no_feasible_replicates,
                stability.design.replicates,
                listed(&fragile, ", ", "none")
            );
        }
        let _ = writeln!(
            out,
            "\n**Valuation basis** {}\n\n**Reference-class basis** {}\n\nEvidence class {}. Full Markdown and JSON retain all assessed policies and the original input evidence.\n\nComparison digest `{}`.",
            md(&basis.valuation_basis),
            md(&baseline.policy.reference_class_basis),
            evidence(self.claim().evidence),
            self.digest()
        );
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{EvaluatedModel, evaluate};
    use fmine_core::anchor::SourceAnchor;
    use fmine_core::hash::sha256;

    const CASE: &str = include_str!("../../../../../tests/capital/staged_permit.toml");

    fn report(text: &str) -> Box<super::EvaluatedRecourseProgram> {
        let source =
            SourceAnchor::new("staged_permit.toml", sha256(text.as_bytes()).0, "worksheet");
        match evaluate(text, source).expect("source-bound staged program") {
            EvaluatedModel::Recourse(program) => program,
            other => panic!("expected staged program, got {}", other.mode()),
        }
    }

    #[test]
    fn brief_distinguishes_absent_recommendations_and_never_promotes_a_rejected_policy() {
        for (input, why) in [
            (
                CASE.replacen(
                    "minimum_cash = \"15000000\"",
                    "minimum_cash = \"27000000\"",
                    1,
                ),
                "Every policy fails at least one declared structural constraint",
            ),
            (
                CASE.replace(
                    "tail_mass = \"1/5\"",
                    "tail_mass = \"1/5\"\nmax_tail_loss = \"-1\"",
                ),
                "Every structurally feasible policy fails at least one declared tail-loss limit",
            ),
        ] {
            let program = report(&input);
            let brief = program.to_brief().expect("brief");
            assert!(program.summary().feasible_winners.is_empty());
            assert!(brief.contains(why));
            assert!(brief.contains(
                "**Best precommitted choice** none. **Net adaptive value** unavailable."
            ));
            assert!(!brief.contains("## Actions for "));
        }
    }

    #[test]
    fn brief_retains_every_tied_policy_and_keeps_unobserved_actions_uniform() {
        let input = CASE.replace("value = \"13000000\"", "value = \"10000000\"");
        let program = report(&input);
        let brief = program.to_brief().expect("brief");
        let winners = &program.summary().feasible_winners;
        assert_eq!(winners.len(), 2);
        for id in winners {
            assert!(brief.contains(&format!("## Actions for {id}")));
        }
        assert!(brief.contains("| All scenarios | 1/1 | No additional commitment | US$0 |"));
        assert_eq!(brief.matches("**Review on 2030-04-02**").count(), 1);
    }

    #[test]
    fn brief_shows_the_separate_equity_limit_that_changes_the_decision() {
        let input = include_str!("../../../../../tests/capital/entity_valuation.toml");
        let brief = report(input).to_brief().expect("brief with secondary risk");
        assert!(brief.contains("**Net adaptive value** 2/5 USD/share."));
        assert!(brief.contains("**Expected value given up** 3/10 USD/share."));
        for text in [
            "| carve-out (Invented ordinary shares) | 31/15 USD/share | 1/10 USD/share | Fails |",
            "| carve-out (Invented ordinary shares) | 1/10 USD/share | 1/10 USD/share | Passes |",
            "| good | 1/2 | fair | US$20 |",
        ] {
            assert!(brief.contains(text), "missing {text}\n{brief}");
        }
        assert!(!brief.contains("| good | 1/2 | aggressive |"));
    }
}
