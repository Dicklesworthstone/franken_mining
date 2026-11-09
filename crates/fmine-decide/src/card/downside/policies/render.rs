//! Complete bounded-policy optima, original-policy comparison and realized risk.
//! Determinism: Exact

use super::*;
use crate::card::render::{js, md, policy_risk_json_with_rule, ratio, rq};
use std::fmt::Write as _;

const RISK_RULE: &str = "Follow the canonical optimal feasible mapping displayed in signal_actions. Its co-optimal count concerns complete mappings under the requested probability screen, not posterior EV ties. reachable_tie_signals retains the original nominal posterior EV-tie diagnostic. Reported quantiles and CVaR describe this representative's nominal realized payoff law; CVaR is not the optimized objective or a constraint.";

fn optional(value: Option<Rat>) -> String {
    value.map_or_else(|| "null".into(), rq)
}

impl PolicyOptimization {
    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"method\":{},\"screen\":{},\"max_policies\":{},\"max_work\":{}}}",
            js(METHOD),
            js(self.screen.as_str()),
            self.max_policies,
            self.max_work
        )
    }
}

impl OptimizedPolicy {
    fn to_json(&self) -> String {
        let mapping = self
            .signal_actions
            .iter()
            .map(|(signal, action)| {
                format!(
                    "{{\"signal_id\":{},\"action_id\":{}}}",
                    js(signal),
                    action.as_deref().map_or_else(|| "null".into(), js)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let conditional = self
            .conditional_breach_probabilities
            .iter()
            .map(|p| rq(*p))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"signal_actions\":[{mapping}],\"nominal_mean\":{},\"nominal_breach_probability\":{},\"screened_breach_probability\":{},\"conditional_breach_probabilities\":[{conditional}],\"breach_certificate\":{},\"risk\":{}}}",
            rq(self.nominal_mean),
            rq(self.nominal_breach_probability),
            rq(self.screened_breach_probability),
            self.breach_certificate
                .as_ref()
                .map_or_else(|| "null".into(), robustness::certificate_json),
            policy_risk_json_with_rule(&self.risk, RISK_RULE)
        )
    }
}

impl PolicyOptionSearch {
    fn to_json(&self) -> String {
        format!(
            "{{\"option_id\":{},\"status\":{},\"reachable_signals\":{},\"policies_evaluated\":{},\"feasible_policies\":{},\"optimal_policy_count\":{},\"optimum\":{},\"fixed_policy_mean\":{},\"fixed_policy_screened_breach_probability\":{},\"fixed_policy_eligible\":{},\"expected_value_cost_of_limit\":{},\"improvement_over_best_eligible_act_now\":{}}}",
            js(&self.option_id),
            js(if self.optimum.is_some() {
                "optimal_feasible_policy"
            } else {
                "no_feasible_policy"
            }),
            self.reachable_signals,
            self.policies_evaluated,
            self.feasible_policies,
            self.optimal_policy_count,
            self.optimum
                .as_ref()
                .map_or_else(|| "null".into(), OptimizedPolicy::to_json),
            rq(self.fixed_policy_mean),
            rq(self.fixed_policy_screened_breach_probability),
            self.fixed_policy_eligible,
            optional(self.expected_value_cost_of_limit),
            optional(self.improvement_over_best_eligible_act_now)
        )
    }
}

impl PolicyOptimizationSummary {
    #[must_use]
    pub fn to_json(&self) -> String {
        let options = self
            .options
            .iter()
            .map(PolicyOptionSearch::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":\"fmine.policy-optimization.v1\",\"configuration\":{},\"objective\":\"nominal_expected_value\",\"policies_evaluated\":{},\"work_units\":{},\"work_unit\":{},\"options\":[{options}],\"best_eligible_act_now\":{},\"overall\":{},\"tie_rule\":{},\"scope\":{}}}",
            self.configuration.to_json(),
            self.policies_evaluated,
            self.work_units,
            js(WORK_UNIT),
            self.best_eligible_act_now.to_json(),
            self.overall.to_json(),
            js(TIE_RULE),
            js(SCOPE)
        )
    }

    pub(in crate::card::downside) fn to_markdown(&self) -> String {
        let mut out = format!(
            "\n## Optimized one-test downside policies\n\n{SCOPE}\n\n**Requested probability screen:** {}. **Objective:** nominal expected net payoff. **Complete mappings evaluated:** {} / {}. **Conservative work units reserved:** {} / {}.\n\n{TIE_RULE}\n",
            self.configuration.screen.as_str(),
            self.policies_evaluated,
            self.configuration.max_policies,
            self.work_units,
            self.configuration.max_work
        );
        self.best_eligible_act_now
            .append_markdown(&mut out, "Best eligible act-now comparison");
        self.overall
            .append_markdown(&mut out, "Act-now and optimized-policy comparison");
        for option in &self.options {
            let _ = writeln!(
                out,
                "\n### Information option {}\n\nReachable signals: {}. Complete deterministic mappings: {}. Feasible mappings: {}. Exact co-optimal mappings: {}.\n\nOriginal fixed EV-policy mean: {}. Its requested-screen breach probability: {} ({}).",
                md(&option.option_id),
                option.reachable_signals,
                option.policies_evaluated,
                option.feasible_policies,
                option.optimal_policy_count,
                ratio(option.fixed_policy_mean),
                ratio(option.fixed_policy_screened_breach_probability),
                if option.fixed_policy_eligible {
                    "eligible"
                } else {
                    "rejected"
                }
            );
            let Some(optimum) = &option.optimum else {
                out.push_str("\n**No feasible deterministic policy for this option.** No mapping or value improvement is fabricated.\n");
                continue;
            };
            let text = |value: Option<Rat>| value.map_or_else(|| "unavailable".into(), ratio);
            let _ = writeln!(
                out,
                "\nOptimal nominal mean: **{}**. Nominal breach probability: **{}**. Requested-screen breach probability: **{}**. Expected-value cost of the limit: **{}**. Improvement over the best eligible act-now alternative: **{}**.\n\n| Signal | Selected action |\n|---|---|",
                ratio(optimum.nominal_mean),
                ratio(optimum.nominal_breach_probability),
                ratio(optimum.screened_breach_probability),
                text(option.expected_value_cost_of_limit),
                text(option.improvement_over_best_eligible_act_now)
            );
            for (signal, action) in &optimum.signal_actions {
                let _ = writeln!(
                    out,
                    "| {} | {} |",
                    md(signal),
                    action
                        .as_deref()
                        .map_or_else(|| "unreachable; no action".into(), md)
                );
            }
            let risk = &optimum.risk;
            let _ = writeln!(
                out,
                "\nRepresentative nominal risk: P10/P50/P90 = {} / {} / {}; signed-loss CVaR95 = {}; probability of negative net payoff = {}. These characterize this canonical representative; CVaR is not constrained or optimized.\n\n| Gross payoff | Net payoff | Joint probability |\n|---:|---:|---:|",
                ratio(risk.p10_p50_p90[0].major),
                ratio(risk.p10_p50_p90[1].major),
                ratio(risk.p10_p50_p90[2].major),
                ratio(risk.cvar95_loss.major),
                ratio(risk.loss_probability)
            );
            for bin in &risk.distribution {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} |",
                    ratio(bin.gross_payoff.major),
                    ratio(bin.net_payoff.major),
                    ratio(bin.probability)
                );
            }
        }
        out
    }
}
