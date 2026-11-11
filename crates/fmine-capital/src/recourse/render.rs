//! Source-complete two-stage policy reports. Rendering performs no selection arithmetic.
//! Determinism: Exact (inherited evidence metadata retains its stored floating-point bits).

use std::fmt::Write as _;

use super::{
    DecisionStage, EvaluatedRecourseProgram, PolicyChoice, RecourseBasis, RecourseSummary,
};
use crate::StructuralRejection;
use crate::render::{
    claim_json, claim_markdown, currency, equity_assessment_json, evidence,
    funding_schedule_assessment_json, ids, js, md, mq, per_share_shown, per_share_statistics_json,
    ratio, rejection_json, rq, shown, statistics_json,
};

pub(super) fn basis_json(basis: &RecourseBasis) -> String {
    let mut stages: Vec<_> = basis.candidate_stages.iter().collect();
    stages.sort_by(|a, b| a.candidate_id.cmp(&b.candidate_id));
    let stages = stages
        .iter()
        .map(|row| {
            format!(
                "{{\"candidate_id\":{},\"stage\":{}}}",
                js(&row.candidate_id),
                js(match row.stage {
                    DecisionStage::Initial => "initial",
                    DecisionStage::Recourse => "recourse",
                })
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut observations: Vec<_> = basis.observations.iter().collect();
    observations
        .sort_by(|a, b| (&a.state_id, &a.observation_id).cmp(&(&b.state_id, &b.observation_id)));
    let observations = observations
        .iter()
        .map(|row| {
            format!(
                "{{\"state_id\":{},\"observation_id\":{}}}",
                js(&row.state_id),
                js(&row.observation_id)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"model_key\":{},\"currency\":{},\"valuation_basis\":{},\"information_candidate\":{},\"observation_date\":{},\"information_basis\":{},\"candidate_stages\":[{stages}],\"observations\":[{observations}]}}",
        js(&basis.model_key),
        currency(basis.currency),
        js(&basis.valuation_basis),
        js(&basis.information_candidate),
        js(&basis.observation_date),
        js(&basis.information_basis)
    )
}

fn policy_json(policy: &PolicyChoice) -> String {
    let branches = policy
        .branches
        .iter()
        .map(|branch| {
            format!(
                "{{\"observation_id\":{},\"probability\":{},\"selected\":{},\"spend\":{}}}",
                branch
                    .observation_id
                    .as_deref()
                    .map_or_else(|| "null".into(), js),
                rq(branch.probability),
                ids(&branch.selected),
                rq(branch.spend.major)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let states = policy.states.iter().map(|state| format!(
        "{{\"state_id\":{},\"observation_id\":{},\"bundle_id\":{},\"selected\":{},\"spend\":{},\"budget_slack\":{},\"net_nav\":{}}}",
        js(&state.state_id),
        state.observation_id.as_deref().map_or_else(|| "null".into(), js),
        js(&state.bundle_id), ids(&state.selected), rq(state.spend.major),
        rq(state.budget_slack.major), rq(state.net_nav.major)
    )).collect::<Vec<_>>().join(",");
    let rejections = policy
        .structural_rejections
        .iter()
        .map(|rejection| {
            format!(
                "{{\"state_id\":{},\"reason\":{}}}",
                js(&rejection.state_id),
                rejection_json(&rejection.reason)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let entity = policy
        .entity_funding
        .as_ref()
        .map_or_else(String::new, |funding| {
            format!(
                ",\"entity_funding\":{}",
                crate::entity::render::assessment_json(funding)
            )
        });
    let entity_valuations = if policy.entity_valuations.is_empty() {
        String::new()
    } else {
        format!(
            ",\"entity_valuations\":[{}]",
            policy
                .entity_valuations
                .iter()
                .map(crate::entity::valuation::render::assessment_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    format!(
        "{{\"policy_id\":{},\"initial_selected\":{},\"initial_spend\":{},\"information_acquired\":{},\"branches\":[{branches}],\"states\":[{states}],\"structural_rejections\":[{rejections}],\"funding_schedule\":{}{entity}{entity_valuations},\"equity\":{},\"statistics\":{},\"tail_loss\":{},\"per_share_statistics\":{},\"per_share_tail_loss\":{},\"risk_rejected\":{}}}",
        js(&policy.policy_id),
        ids(&policy.initial_selected),
        rq(policy.initial_spend.major),
        policy.information_acquired,
        policy
            .funding_schedule
            .as_ref()
            .map_or_else(|| "null".into(), funding_schedule_assessment_json),
        policy
            .equity
            .as_ref()
            .map_or_else(|| "null".into(), equity_assessment_json),
        policy
            .statistics
            .as_ref()
            .map_or_else(|| "null".into(), statistics_json),
        policy
            .tail_loss
            .map_or_else(|| "null".into(), |v| rq(v.major)),
        policy
            .per_share_statistics
            .as_ref()
            .map_or_else(|| "null".into(), per_share_statistics_json),
        policy
            .per_share_tail_loss
            .map_or_else(|| "null".into(), |v| rq(v.major)),
        policy.risk_rejected
    )
}

pub(super) fn summary_json(summary: &RecourseSummary) -> String {
    let policies = summary
        .policies
        .iter()
        .map(policy_json)
        .collect::<Vec<_>>()
        .join(",");
    let (adaptive_available, unconstrained_available) = if summary.per_share_objective {
        (
            summary.net_adaptive_value_per_share.is_some(),
            summary.unconstrained_net_adaptive_value_per_share.is_some(),
        )
    } else {
        (
            summary.net_adaptive_value.is_some(),
            summary.unconstrained_net_adaptive_value.is_some(),
        )
    };
    let adaptive_status = js(if summary.feasible_winners.is_empty() {
        "no_risk_feasible_policy"
    } else if adaptive_available {
        "available"
    } else {
        "no_feasible_static_comparator"
    });
    let unconstrained_adaptive_status = js(if summary.unconstrained_winners.is_empty() {
        "no_structurally_feasible_policy"
    } else if unconstrained_available {
        "available"
    } else {
        "no_structurally_feasible_static_comparator"
    });
    let stability = summary.selection_stability.as_ref().map_or_else(
        || "null".into(),
        crate::stability::SelectionStabilitySummary::to_json,
    );
    let stability_status = if summary.selection_stability.is_some() {
        "conditional_weighted_bootstrap"
    } else {
        "unavailable_declared_two_stage_model"
    };
    let robustness = summary
        .robustness
        .as_ref()
        .map_or_else(String::new, |comparison| {
            format!(",\"robustness\":{}", comparison.to_json())
        });
    format!(
        "{{\"model_key\":{},\"question\":{},\"currency\":{},\"status\":{},\"information\":{},\"per_share_objective\":{},\"objective\":{},\"objective_unit\":{},\"statistics_unit\":\"USD\",\"per_share_statistics_unit\":\"USD/share\",\"tail_loss_unit\":\"USD\",\"per_share_tail_loss_unit\":\"USD/share\",\"spending_unit\":\"USD\",\"cash_unit\":\"USD\",\"policies\":[{policies}],\"unconstrained_winners\":{},\"feasible_winners\":{},\"expected_value_cost_of_risk_limit\":{},\"expected_per_share_cost_of_risk_limit\":{},\"net_adaptive_value\":{},\"net_adaptive_value_per_share\":{},\"net_adaptive_value_status\":{adaptive_status},\"unconstrained_net_adaptive_value\":{},\"unconstrained_net_adaptive_value_per_share\":{},\"unconstrained_net_adaptive_value_status\":{unconstrained_adaptive_status},\"selection_frequency\":{stability},\"selection_frequency_status\":\"{stability_status}\",\"regret_reference\":\"all_structurally_feasible_policies_before_risk_limit\"{robustness}}}",
        js(&summary.model_key),
        js(&summary.question),
        currency(summary.currency),
        js(if summary.unconstrained_winners.is_empty() {
            "no_structurally_feasible_policy"
        } else if summary.feasible_winners.is_empty() {
            "no_risk_feasible_policy"
        } else {
            "ok"
        }),
        basis_json(&summary.information),
        summary.per_share_objective,
        js(if summary.per_share_objective {
            "expected_change_in_equity_nav_per_share"
        } else {
            "expected_net_delta_nav"
        }),
        js(if summary.per_share_objective {
            "USD/share"
        } else {
            "USD"
        }),
        ids(&summary.unconstrained_winners),
        ids(&summary.feasible_winners),
        summary
            .expected_value_cost_of_risk_limit
            .map_or_else(|| "null".into(), |v| rq(v.major)),
        summary
            .expected_per_share_cost_of_risk_limit
            .map_or_else(|| "null".into(), |v| rq(v.major)),
        summary
            .net_adaptive_value
            .map_or_else(|| "null".into(), |v| rq(v.major)),
        summary
            .net_adaptive_value_per_share
            .map_or_else(|| "null".into(), |v| rq(v.major)),
        summary
            .unconstrained_net_adaptive_value
            .map_or_else(|| "null".into(), |v| rq(v.major)),
        summary
            .unconstrained_net_adaptive_value_per_share
            .map_or_else(|| "null".into(), |v| rq(v.major))
    )
}

fn listed(values: &[String]) -> String {
    if values.is_empty() {
        "none".into()
    } else {
        md(&values.join(", "))
    }
}

fn status(policy: &PolicyChoice) -> &'static str {
    if !policy.structural_rejections.is_empty() {
        "structurally rejected"
    } else if policy.risk_rejected {
        "global risk limit exceeded"
    } else {
        "feasible"
    }
}

fn rejection_text(reason: &StructuralRejection) -> String {
    match reason {
        StructuralRejection::BudgetExceeded => "budget exceeded".into(),
        StructuralRejection::MissingPrerequisite {
            candidate,
            prerequisite,
        } => {
            format!("{candidate} requires {prerequisite}")
        }
        StructuralRejection::MutualExclusion { group, candidates } => {
            format!("group {group}: {} are exclusive", candidates.join(", "))
        }
        StructuralRejection::LiquidityShortfall {
            state_id,
            shortfall,
        } => {
            format!(
                "cash-floor shortfall in {state_id}: {} USD",
                ratio(shortfall.major)
            )
        }
        StructuralRejection::CashScheduleShortfall {
            period_id,
            checkpoint_date,
            state_id,
            shortfall,
        } => {
            format!(
                "cash-floor shortfall in {state_id} at {} ({checkpoint_date}): {} USD",
                period_id.as_deref().unwrap_or("opening"),
                ratio(shortfall.major)
            )
        }
        StructuralRejection::NonpositiveShareCount { state_id, shares } => {
            format!(
                "nonpositive ending shares in {state_id}: {} shares",
                ratio(*shares)
            )
        }
        StructuralRejection::EntityNonpositiveShareCount {
            entity,
            share_class,
            state_id,
            shares,
        } => format!(
            "nonpositive ending shares for {entity}/{share_class} in {state_id}: {} shares",
            ratio(*shares)
        ),
        StructuralRejection::EntityCashShortfall {
            entity_id,
            state_id,
            period_id,
            checkpoint_date,
            phase,
            shortfall,
        } => format!(
            "entity cash-floor shortfall for {entity_id} in {state_id} at {} ({checkpoint_date}, {}): {} USD",
            period_id.as_deref().unwrap_or("opening"),
            crate::entity::render::phase_label(phase),
            ratio(shortfall.major)
        ),
        StructuralRejection::EntityCapitalCallShortfall {
            entity_id,
            call_id,
            state_id,
            period_id,
            checkpoint_date,
            shortfall,
        } => format!(
            "capital-call shortfall for {entity_id}/{call_id} in {state_id} at {period_id} ({checkpoint_date}): {} USD",
            ratio(shortfall.major)
        ),
    }
}

impl EvaluatedRecourseProgram {
    #[must_use]
    pub fn to_json(&self) -> String {
        let summary = summary_json(self.summary());
        format!(
            "{{\"schema\":\"fmine.capital-recourse.v1\",\"digest\":{},\"summary\":{summary},\"claim\":{},\"inputs\":{{\"information\":{}}},\"static_comparison\":{},\"no_claim\":{},\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}}",
            js(self.digest()),
            claim_json(self.claim(), &summary),
            claim_json(
                &self.inputs().information,
                &basis_json(&self.inputs().information.value)
            ),
            self.static_program().to_json(),
            js(self.no_claim())
        )
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let s = self.summary();
        let selection_note = if s.selection_stability.is_some() {
            "Conditional selection stability is reported below."
        } else {
            "Selection frequency is unavailable."
        };
        let baseline = self.static_program().summary();
        let basis = &self.inputs().information.value;
        let (unit, limit, risk_cost, before, after) = if let Some(equity) = &baseline.equity {
            (
                "USD/share",
                equity
                    .max_tail_loss_per_share
                    .map_or_else(|| "unbounded".into(), ratio),
                per_share_shown(s.expected_per_share_cost_of_risk_limit),
                per_share_shown(s.unconstrained_net_adaptive_value_per_share),
                per_share_shown(s.net_adaptive_value_per_share),
            )
        } else {
            (
                "USD",
                baseline.policy.max_tail_loss.map_or_else(
                    || "unbounded".into(),
                    |v| mq(v).trim_matches('"').to_string(),
                ),
                shown(s.expected_value_cost_of_risk_limit),
                shown(s.unconstrained_net_adaptive_value),
                shown(s.net_adaptive_value),
            )
        };
        let mut out = format!(
            "# Two-stage capital policy comparison\n\n{}\n\n**Declared model:** `{}`. **Evidence:** {}. **Confidentiality:** {}. **Digest:** `{}`.\n\n{}\n\n**Valuation basis:** {}. **Objective unit:** {unit}. **Budget on every realized path:** {} USD. **Upper loss tail mass:** {}. **Maximum global tail loss:** {limit} {unit}.\n\n## Adaptive and precommitted comparison\n\n| Result | Before the risk limit | After the risk limit |\n|---|---|---|\n| Two-stage policy co-winners | {} | {} |\n| Precommitted subset co-winners | {} | {} |\n| Net adaptive value ({unit}) | {before} | {after} |\n\nExpected-value cost of the policy risk limit: {risk_cost} {unit}. Net adaptive value is the difference between the best complete policy and the best precommitted subset under the corresponding constraints. Information-gate costs and all initial commitments are already included in policy outcomes. The reported difference must not be added to policy NAV again. If either comparison population has no applicable feasible winner, its difference is unavailable.\n\nRisk is evaluated once over each complete unconditional state distribution. Observation probabilities do not turn branch-specific risk limits into a global limit. Regret compares structurally feasible policies before applying the risk limit. {selection_note} All numeric results are exact rational quantities in the stated units.\n",
            md(&s.question),
            md(&s.model_key),
            evidence(self.claim().evidence),
            md(&self.claim().confidentiality.to_string()),
            self.digest(),
            self.no_claim(),
            md(&basis.valuation_basis),
            mq(baseline.policy.budget).trim_matches('"'),
            ratio(baseline.policy.tail_mass),
            listed(&s.unconstrained_winners),
            listed(&s.feasible_winners),
            listed(&baseline.unconstrained_winners),
            listed(&baseline.feasible_winners)
        );
        if !baseline.entity_valuations.is_empty() {
            out.push_str("\nAdditional valuation entities retain separate NAV/share ledgers and signed tail-loss limits. The primary equity entity remains the sole objective. The constrained winners and reported risk cost apply every declared entity limit jointly to the complete policy distribution; an entity's state-specific share failure applies only when that path is realized. Entity NAV/share values are never blended into a common score.\n");
        }
        claim_markdown(&mut out, self.claim());
        let _ = writeln!(
            out,
            "\n## Declared information and stages\n\n**Information candidate:** {}. **Observation date:** {}. **Information basis:** {}.\n\nInitial candidates are chosen before observing the signal, and their full effects remain in every continuation. If the information gate is selected, one continuation is chosen for each declared observation class. States in the same class receive the same continuation. Without the gate, one common continuation applies to every state. Recourse-candidate cash flows before the observation boundary must be explicit zeros.\n",
            md(&basis.information_candidate),
            md(&basis.observation_date),
            md(&basis.information_basis)
        );
        claim_markdown(&mut out, &self.inputs().information);
        out.push_str("\n| Candidate | Decision stage |\n|---|---|\n");
        let mut stages: Vec<_> = basis.candidate_stages.iter().collect();
        stages.sort_by(|a, b| a.candidate_id.cmp(&b.candidate_id));
        for stage in stages {
            let _ = writeln!(
                out,
                "| {} | {} |",
                md(&stage.candidate_id),
                match stage.stage {
                    DecisionStage::Initial => "Initial",
                    DecisionStage::Recourse => "Recourse",
                }
            );
        }
        out.push_str("\n| Shared state | Declared observation class |\n|---|---|\n");
        let mut observations: Vec<_> = basis.observations.iter().collect();
        observations.sort_by(|a, b| a.state_id.cmp(&b.state_id));
        for observation in observations {
            let _ = writeln!(
                out,
                "| {} | {} |",
                md(&observation.state_id),
                md(&observation.observation_id)
            );
        }
        out.push_str("\n## Complete policy outcomes in USD\n\nTotal-USD NAV statistics remain distinct from per-share statistics. Initial spending, continuation spending, path budgets and cash ledgers are always USD.\n\n| Policy | Mean net NAV | P10 | P50 | P90 | Tail loss | CVaR95 loss | Mean regret | Max regret | Status |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---|\n");
        for policy in &s.policies {
            let stats = policy.statistics.as_ref();
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                md(&policy.policy_id),
                shown(stats.map(|v| v.mean)),
                shown(stats.map(|v| v.p10_p50_p90[0])),
                shown(stats.map(|v| v.p10_p50_p90[1])),
                shown(stats.map(|v| v.p10_p50_p90[2])),
                shown(policy.tail_loss),
                shown(stats.map(|v| v.cvar95_loss)),
                shown(stats.map(|v| v.mean_regret)),
                shown(stats.map(|v| v.max_regret)),
                status(policy)
            );
        }
        if s.per_share_objective {
            out.push_str("\n## Complete policy outcomes in USD/share\n\nThe per-share objective and risk limit use these complete policy distributions, after each state's selected candidates are assembled with its equity NAV and actual share count.\n\n| Policy | Mean delta NAV/share | P10 | P50 | P90 | Tail loss | CVaR95 loss | Mean regret | Max regret | Status |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---|\n");
            for policy in &s.policies {
                let stats = policy.per_share_statistics.as_ref();
                let _ = writeln!(
                    out,
                    "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                    md(&policy.policy_id),
                    per_share_shown(stats.map(|v| v.mean)),
                    per_share_shown(stats.map(|v| v.p10_p50_p90[0])),
                    per_share_shown(stats.map(|v| v.p10_p50_p90[1])),
                    per_share_shown(stats.map(|v| v.p10_p50_p90[2])),
                    per_share_shown(policy.per_share_tail_loss),
                    per_share_shown(stats.map(|v| v.cvar95_loss)),
                    per_share_shown(stats.map(|v| v.mean_regret)),
                    per_share_shown(stats.map(|v| v.max_regret)),
                    status(policy)
                );
            }
        }
        for policy in &s.policies {
            policy_markdown(&mut out, policy);
        }
        out.push_str("\n## Declared unknown unknowns for the policy comparison\n\n");
        for term in &self.claim().uu_terms {
            let _ = writeln!(
                out,
                "- {}: {}. Downside exposure: {} USD millions. Trigger: {}.",
                md(&term.scenario_id),
                md(&term.description),
                term.downside_exposure_usd_m,
                md(&term.trigger_indicator)
            );
        }
        if let Some(stability) = &s.selection_stability {
            out.push_str(&stability.to_markdown());
        }
        if let Some(robustness) = &s.robustness {
            out.push_str(&robustness.to_markdown());
        }
        out.push_str("\nAll four authority flags are false: reserve disclosure, execution authorization, geotechnical sign-off and regulatory compliance.\n\n## Complete precommitted comparison and program inputs\n\nThe following retained static report includes every declared candidate, state, cash-flow input, equity input and source Claim. It is the independent precommitted comparison population.\n\n---\n\n");
        out.push_str(&self.static_program().to_markdown());
        out
    }
}

fn policy_markdown(out: &mut String, policy: &PolicyChoice) {
    let _ = writeln!(
        out,
        "\n## Policy `{}`\n\n**Initial choices:** {}. **Initial spend:** {} USD. **Information acquired:** {}. **Status:** {}.\n\n### Continuation by information set\n\nContinuation spending excludes the initial spending shown above.\n\n| Available observation | Exact probability | Recourse choices | Continuation spend (USD) |\n|---|---:|---|---:|",
        md(&policy.policy_id),
        listed(&policy.initial_selected),
        ratio(policy.initial_spend.major),
        if policy.information_acquired {
            "yes"
        } else {
            "no"
        },
        status(policy)
    );
    for branch in &policy.branches {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            branch
                .observation_id
                .as_deref()
                .map_or_else(|| "Not acquired: common continuation".into(), md),
            ratio(branch.probability),
            listed(&branch.selected),
            ratio(branch.spend.major)
        );
    }
    out.push_str("\n### Every realized state and retained commitment\n\n| State | Available observation | Retained static subset | All selected candidates | Spend (USD) | Budget slack (USD) | Net NAV (USD) |\n|---|---|---|---|---:|---:|---:|\n");
    for state in &policy.states {
        let _ = writeln!(
            out,
            "| {} | {} | `{}` | {} | {} | {} | {} |",
            md(&state.state_id),
            state
                .observation_id
                .as_deref()
                .map_or_else(|| "Not acquired".into(), md),
            md(&state.bundle_id),
            listed(&state.selected),
            ratio(state.spend.major),
            ratio(state.budget_slack.major),
            ratio(state.net_nav.major)
        );
    }
    if !policy.structural_rejections.is_empty() {
        out.push_str("\n### Structural rejections\n\n");
        for rejection in &policy.structural_rejections {
            let _ = writeln!(
                out,
                "- State {}: {}.",
                md(&rejection.state_id),
                md(&rejection_text(&rejection.reason))
            );
        }
    }
    if let Some(funding) = &policy.funding_schedule {
        let _ = writeln!(
            out,
            "\n### Cash at every declared checkpoint\n\nMinimum cash-floor slack across all states and checkpoints: {} USD. All cash columns are USD. Positive flows are receipts; negative flows are expenditures.\n\n| State | Period | Checkpoint date | Opening cash | Baseline net | Program net | Closing cash | Floor | Slack | Checkpoint result |\n|---|---|---|---:|---:|---:|---:|---:|---:|---|",
            ratio(funding.minimum_slack.major)
        );
        let mut checkpoints: Vec<_> = funding.checkpoints.iter().collect();
        checkpoints.sort_by(|a, b| {
            (&a.state_id, &a.checkpoint_date, &a.period_id).cmp(&(
                &b.state_id,
                &b.checkpoint_date,
                &b.period_id,
            ))
        });
        for c in checkpoints {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                md(&c.state_id),
                c.period_id.as_deref().map_or_else(|| "Opening".into(), md),
                md(&c.checkpoint_date),
                ratio(c.opening_cash.major),
                ratio(c.baseline_cash_flow.major),
                ratio(c.candidate_cash_flow.major),
                ratio(c.closing_cash.major),
                ratio(c.minimum_cash.major),
                ratio(c.slack.major),
                if c.slack.major.num() < 0 {
                    "floor breach"
                } else {
                    "floor met"
                }
            );
        }
    }
    if let Some(funding) = &policy.entity_funding {
        crate::entity::render::assessment_markdown(
            out,
            &format!("Policy {}", policy.policy_id),
            funding,
        );
    }
    if let Some(equity) = &policy.equity {
        out.push_str("\n### Equity and actual shares in every state\n\nShare-count columns are actual shares. NAV/share columns are USD/share. Undefined per-share ratios remain unavailable.\n\n| State | Baseline NAV (USD) | NAV change (USD) | Ending NAV (USD) | Baseline shares | Share change | Ending shares | Baseline NAV/share | Ending NAV/share | Delta NAV/share |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
        for state in &equity.states {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                md(&state.state_id),
                ratio(state.baseline_nav.major),
                ratio(state.nav_change.major),
                ratio(state.ending_nav.major),
                ratio(state.baseline_shares),
                ratio(state.share_change),
                ratio(state.ending_shares),
                ratio(state.baseline_nav_per_share.major),
                per_share_shown(state.ending_nav_per_share),
                per_share_shown(state.delta_nav_per_share)
            );
        }
    }
    for assessment in &policy.entity_valuations {
        crate::entity::valuation::render::assessment_markdown(
            out,
            &format!("Policy {}", policy.policy_id),
            assessment,
        );
    }
}
