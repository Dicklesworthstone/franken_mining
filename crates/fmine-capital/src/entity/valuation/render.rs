//! Complete independent entity valuation inputs and same-state per-share outcomes.
//! Determinism: Exact (rendering preserves each entity's own denominator and risk result).

use std::fmt::Write as _;

use super::{EntityValuation, EntityValuationAssessment};
use crate::render::{
    claim_markdown, currency, equity_assessment_json, equity_json, js, md, mq, per_share_shown,
    per_share_statistics_json, ratio, rq,
};
use crate::{Claim, ProgramChoice};

pub(crate) fn input_json(valuation: &EntityValuation) -> String {
    let mut changes: Vec<_> = valuation.nav_changes.iter().collect();
    changes.sort_by(|a, b| (&a.candidate_id, &a.state_id).cmp(&(&b.candidate_id, &b.state_id)));
    let changes = changes
        .iter()
        .map(|change| {
            format!(
                "{{\"candidate_id\":{},\"state_id\":{},\"value\":{},\"currency\":{}}}",
                js(&change.candidate_id),
                js(&change.state_id),
                mq(change.value),
                currency(change.value.ccy)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"equity\":{},\"nav_changes\":[{changes}]}}",
        equity_json(&valuation.equity)
    )
}

pub(crate) fn assessment_json(assessment: &EntityValuationAssessment) -> String {
    format!(
        "{{\"entity\":{},\"share_class\":{},\"equity\":{},\"per_share_statistics\":{},\"per_share_tail_loss\":{},\"max_tail_loss_per_share\":{},\"risk_rejected\":{}}}",
        js(&assessment.entity),
        js(&assessment.share_class),
        equity_assessment_json(&assessment.equity),
        assessment
            .per_share_statistics
            .as_ref()
            .map_or_else(|| "null".into(), per_share_statistics_json),
        assessment
            .per_share_tail_loss
            .map_or_else(|| "null".into(), |value| rq(value.major)),
        assessment
            .max_tail_loss_per_share
            .map_or_else(|| "null".into(), |value| rq(value.major)),
        assessment.risk_rejected
    )
}

fn risk_status(assessment: &EntityValuationAssessment) -> &'static str {
    if assessment.per_share_statistics.is_none() {
        "unavailable: structurally rejected"
    } else if assessment.risk_rejected {
        "entity tail-loss cap exceeded"
    } else if assessment.max_tail_loss_per_share.is_some() {
        "entity tail-loss cap met"
    } else {
        "no entity tail-loss cap declared"
    }
}

pub(crate) fn inputs_markdown(out: &mut String, claim: &Claim<EntityValuation>) {
    let valuation = &claim.value;
    let equity = &valuation.equity;
    let _ = writeln!(
        out,
        "\n### Valuation declaration for {} / {}\n\n**Valuation basis:** {}. **Shares per declared unit:** {}. **Initial declared share units:** {}. **Maximum upper-tail loss:** {} USD/share.\n\nEach selected candidate contributes this entity's declared net equity-NAV change and share-unit change in the same shared state. Its NAV/share is calculated before averaging. Cash transfers, dividends, debt service and other entities' NAV are not added again. The same program tail mass applies to this entity's separate loss distribution.\n",
        md(&equity.entity),
        md(&equity.share_class),
        md(&equity.valuation_basis),
        ratio(equity.shares_per_unit),
        ratio(equity.initial_share_units),
        equity
            .max_tail_loss_per_share
            .map_or_else(|| "unbounded".into(), ratio)
    );
    claim_markdown(out, claim);
    out.push_str("\n| State | Baseline equity NAV (USD) |\n|---|---:|\n");
    let mut baseline: Vec<_> = equity.baseline_nav.iter().collect();
    baseline.sort_by(|a, b| a.0.cmp(&b.0));
    for (state, value) in baseline {
        let _ = writeln!(out, "| {} | {} |", md(state), mq(*value).trim_matches('"'));
    }
    out.push_str("\nEvery candidate/state NAV change is explicit, including zeroes and candidates absent from every winner.\n\n| Candidate | State | Net equity NAV change (USD) |\n|---|---|---:|\n");
    let mut changes: Vec<_> = valuation.nav_changes.iter().collect();
    changes.sort_by(|a, b| (&a.candidate_id, &a.state_id).cmp(&(&b.candidate_id, &b.state_id)));
    for change in changes {
        let _ = writeln!(
            out,
            "| {} | {} | {} |",
            md(&change.candidate_id),
            md(&change.state_id),
            mq(change.value).trim_matches('"')
        );
    }
    out.push_str("\nThe following changes use declared share units; outcome ledgers normalize them to actual shares.\n\n| Candidate | State | Share-unit change |\n|---|---|---:|\n");
    let mut shares: Vec<_> = equity.share_changes.iter().collect();
    shares.sort_by(|a, b| (&a.candidate_id, &a.state_id).cmp(&(&b.candidate_id, &b.state_id)));
    for change in shares {
        let _ = writeln!(
            out,
            "| {} | {} | {} |",
            md(&change.candidate_id),
            md(&change.state_id),
            ratio(change.units)
        );
    }
}

pub(crate) fn assessment_markdown(
    out: &mut String,
    label: &str,
    assessment: &EntityValuationAssessment,
) {
    let stats = assessment.per_share_statistics.as_ref();
    let _ = writeln!(
        out,
        "\n### {}: {} / {} valuation\n\nAll summary statistics and the risk cap are exact USD per actual share of this entity. Its risk status is separate from the complete choice's structural and other-entity constraints.\n\n| Mean delta NAV/share | P10 | P50 | P90 | Tail loss | CVaR95 loss | Mean regret | Max regret | Risk cap | Entity risk status |\n|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|\n| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
        md(label),
        md(&assessment.entity),
        md(&assessment.share_class),
        per_share_shown(stats.map(|value| value.mean)),
        per_share_shown(stats.map(|value| value.p10_p50_p90[0])),
        per_share_shown(stats.map(|value| value.p10_p50_p90[1])),
        per_share_shown(stats.map(|value| value.p10_p50_p90[2])),
        per_share_shown(assessment.per_share_tail_loss),
        per_share_shown(stats.map(|value| value.cvar95_loss)),
        per_share_shown(stats.map(|value| value.mean_regret)),
        per_share_shown(stats.map(|value| value.max_regret)),
        assessment
            .max_tail_loss_per_share
            .map_or_else(|| "unbounded".into(), |value| ratio(value.major)),
        risk_status(assessment)
    );
    out.push_str("\nShare-count columns are actual shares. Nonpositive ending shares retain undefined ratios and a named structural rejection.\n\n| State | Baseline NAV (USD) | NAV change (USD) | Ending NAV (USD) | Baseline shares | Share change | Ending shares | Baseline NAV/share | Ending NAV/share | Delta NAV/share |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for state in &assessment.equity.states {
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

pub(crate) fn static_markdown(
    out: &mut String,
    claims: &[Claim<EntityValuation>],
    choices: &[ProgramChoice],
) {
    if claims.is_empty() {
        return;
    }
    out.push_str("\n## Additional entity valuations and binding risk limits\n\nThe primary equity entity remains the sole selection objective. Each additional entity has its own independently declared NAV, share denominator, valuation basis and optional tail-loss cap. Every declared cap must pass for a choice to be risk-feasible. Entity NAV and per-share values are never summed or blended. Regret uses all structurally feasible choices before any entity's risk cap.\n");
    for claim in claims {
        inputs_markdown(out, claim);
    }
    for choice in choices {
        for assessment in &choice.entity_valuations {
            assessment_markdown(out, &format!("Bundle {}", choice.bundle_id), assessment);
        }
    }
}
