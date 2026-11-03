//! Complete subscription declarations, ownership changes and separately conserved call cash.
//! Determinism: Exact (rendering preserves supplied and evaluated values without arithmetic).

use std::{collections::BTreeMap, fmt::Write as _};

use super::*;
use crate::render::{currency, js, md, mq, ratio, rq};

fn ordered<'a>(
    funding: &'a EntityCapitalCallFunding,
    periods: &BTreeMap<&str, usize>,
) -> Vec<&'a CapitalCall> {
    let mut calls: Vec<_> = funding.calls.iter().collect();
    calls.sort_by(|a, b| {
        (
            periods.get(a.period_id.as_str()),
            &a.period_id,
            &a.entity_id,
            a.sequence,
            &a.id,
        )
            .cmp(&(
                periods.get(b.period_id.as_str()),
                &b.period_id,
                &b.entity_id,
                b.sequence,
                &b.id,
            ))
    });
    calls
}

pub(crate) fn funding_json(
    funding: &EntityCapitalCallFunding,
    periods: &BTreeMap<&str, usize>,
) -> String {
    let calls = ordered(funding, periods).iter().map(|call| {
        let mut states: Vec<_> = call.states.iter().collect();
        states.sort_by(|a, b| a.state_id.cmp(&b.state_id));
        let states = states.iter().map(|row| format!(
            "{{\"state_id\":{},\"pre_money_value\":{},\"pre_money_value_currency\":{},\"required_funding\":{},\"required_funding_currency\":{},\"parent_contribution\":{},\"parent_contribution_currency\":{},\"external_contribution\":{},\"external_contribution_currency\":{}}}",
            js(&row.state_id), mq(row.pre_money_value), currency(row.pre_money_value.ccy), mq(row.required_funding), currency(row.required_funding.ccy), mq(row.parent_contribution), currency(row.parent_contribution.ccy), mq(row.external_contribution), currency(row.external_contribution.ccy)
        )).collect::<Vec<_>>().join(",");
        format!("{{\"id\":{},\"entity_id\":{},\"period_id\":{},\"sequence\":{},\"candidate_id\":{},\"basis\":{},\"states\":[{states}]}}", js(&call.id), js(&call.entity_id), js(&call.period_id), call.sequence, call.candidate_id.as_deref().map_or_else(|| "null".into(), js), js(&call.basis))
    }).collect::<Vec<_>>().join(",");
    format!(
        "{{\"method\":{},\"cash_flow_convention\":{},\"calls\":[{calls}]}}",
        js(&funding.method),
        js(&funding.cash_flow_convention)
    )
}

pub(crate) fn cash_flows_json(flows: &CapitalCallCashFlows) -> String {
    format!(
        "{{\"parent_in\":{},\"parent_out\":{},\"external_in\":{}}}",
        rq(flows.parent_in.major),
        rq(flows.parent_out.major),
        rq(flows.external_in.major)
    )
}

pub(crate) fn result_json(row: &CapitalCallResult) -> String {
    format!(
        "{{\"call_id\":{},\"entity_id\":{},\"parent_entity_id\":{},\"period_id\":{},\"checkpoint_date\":{},\"sequence\":{},\"candidate_id\":{},\"basis\":{},\"active\":{},\"pre_money_value\":{},\"required_funding\":{},\"declared_parent_contribution\":{},\"declared_external_contribution\":{},\"parent_contribution\":{},\"external_contribution\":{},\"funding_surplus\":{},\"parent_share_before\":{},\"parent_share_after\":{},\"external_share_after\":{},\"ownership_residual\":{}}}",
        js(&row.call_id),
        js(&row.entity_id),
        js(&row.parent_entity_id),
        js(&row.period_id),
        js(&row.checkpoint_date),
        row.sequence,
        row.candidate_id
            .as_deref()
            .map_or_else(|| "null".into(), js),
        js(&row.basis),
        row.active,
        rq(row.pre_money_value.major),
        rq(row.required_funding.major),
        rq(row.declared_parent_contribution.major),
        rq(row.declared_external_contribution.major),
        rq(row.parent_contribution.major),
        rq(row.external_contribution.major),
        rq(row.funding_surplus.major),
        rq(row.parent_share_before),
        rq(row.parent_share_after),
        rq(row.external_share_after),
        rq(row.ownership_residual)
    )
}

pub(crate) fn inputs_markdown(
    out: &mut String,
    funding: &EntityCapitalCallFunding,
    periods: &BTreeMap<&str, usize>,
) {
    let _ = writeln!(
        out,
        "\n### Declared JV subscriptions\n\n**Method:** {}. **Cash convention:** {}. Each call declares the positive 100% equity value immediately before that subscription. If the parent's previous fraction is s, pre-money value V and parent/outside contributions P/X, the new fraction is (sV + P)/(V + P + X). The outside owner receives its complement. Explicit sequence orders calls within each entity/period; identifiers do not select economic order. All call cash settles before mandatory debt service and pre-distribution floors. The resulting fractions apply to that period's dividend and carry forward.\n\nNative cash excludes these contributions. Parent funding debits the parent and credits the JV equally; outside funding enters only the JV. Contributions are never reduced to affordable cash. An active raise below its declared requirement remains a structural failure, with its planned cash and partial issuance visible; later cash cannot repair that failure. Inactive calls preserve declared terms but transfer zero, change no ownership and report zero funding surplus. These are declared subscription assumptions, not inferred JV agreement penalties or a derivation of any entity's separately declared NAV.\n\n| Call | Entity | Period | Sequence | Candidate trigger | Basis |\n|---|---|---|---:|---|---|",
        md(&funding.method),
        md(&funding.cash_flow_convention)
    );
    let calls = ordered(funding, periods);
    for call in &calls {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} |",
            md(&call.id),
            md(&call.entity_id),
            md(&call.period_id),
            call.sequence,
            call.candidate_id
                .as_deref()
                .map_or_else(|| "Unconditional".into(), md),
            md(&call.basis)
        );
    }
    out.push_str("\n**Declared subscription amounts (USD)**\n\n| Call | State | Pre-money equity value | Required funding | Parent contribution | Outside contribution |\n|---|---|---:|---:|---:|---:|\n");
    for call in calls {
        let mut states: Vec<_> = call.states.iter().collect();
        states.sort_by(|a, b| a.state_id.cmp(&b.state_id));
        for row in states {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} |",
                md(&call.id),
                md(&row.state_id),
                mq(row.pre_money_value).trim_matches('"'),
                mq(row.required_funding).trim_matches('"'),
                mq(row.parent_contribution).trim_matches('"'),
                mq(row.external_contribution).trim_matches('"')
            );
        }
    }
}

pub(crate) fn assessment_markdown(out: &mut String, state: &EntityFundingState) {
    if state.capital_calls.is_empty() {
        return;
    }
    out.push_str("\n**JV subscription activity**\n\n| Call | Entity | Parent | Period | Date | Sequence | Candidate trigger | Active | Basis |\n|---|---|---|---|---|---:|---|---|---|\n");
    for row in &state.capital_calls {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            md(&row.call_id),
            md(&row.entity_id),
            md(&row.parent_entity_id),
            md(&row.period_id),
            md(&row.checkpoint_date),
            row.sequence,
            row.candidate_id
                .as_deref()
                .map_or_else(|| "Unconditional".into(), md),
            row.active,
            md(&row.basis)
        );
    }
    out.push_str("\n**Subscription funding (USD)**\n\nNegative active funding surplus is a retained structural rejection. Planned contributions and partial ownership remain visible on rejected paths.\n\n| Call | Pre-money value | Required funding | Declared parent | Declared outside | Parent paid | Outside paid | Funding surplus |\n|---|---:|---:|---:|---:|---:|---:|---:|\n");
    for row in &state.capital_calls {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            md(&row.call_id),
            ratio(row.pre_money_value.major),
            ratio(row.required_funding.major),
            ratio(row.declared_parent_contribution.major),
            ratio(row.declared_external_contribution.major),
            ratio(row.parent_contribution.major),
            ratio(row.external_contribution.major),
            ratio(row.funding_surplus.major)
        );
    }
    out.push_str("\n**Ownership after each call**\n\n| Call | Parent before | Parent after | Outside after | Ownership residual |\n|---|---:|---:|---:|---:|\n");
    for row in &state.capital_calls {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            md(&row.call_id),
            ratio(row.parent_share_before),
            ratio(row.parent_share_after),
            ratio(row.external_share_after),
            ratio(row.ownership_residual)
        );
    }
    out.push_str("\n**Subscription cash by checkpoint (USD)**\n\n| Entity | Period | Phase | Parent funding in | Parent funding out | Outside funding in |\n|---|---|---|---:|---:|---:|\n");
    for row in &state.checkpoints {
        if let Some(flows) = row.capital_call {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} |",
                md(&row.entity_id),
                row.period_id
                    .as_deref()
                    .map_or_else(|| "Opening".into(), md),
                crate::entity::render::phase_label(&row.phase),
                ratio(flows.parent_in.major),
                ratio(flows.parent_out.major),
                ratio(flows.external_in.major)
            );
        }
    }
}

pub(crate) fn conservation_markdown(out: &mut String, state: &EntityFundingState) {
    if state.capital_calls.is_empty() {
        return;
    }
    out.push_str("\n**Subscription cash conservation components (USD)**\n\nParent funding in and out cancel. Outside funding is a separate inflow in the complete cash residual above.\n\n| Period | Parent funding in | Parent funding out | Outside funding in |\n|---|---:|---:|---:|\n");
    for row in &state.conservation {
        if let Some(flows) = row.capital_call {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                md(&row.period_id),
                ratio(flows.parent_in.major),
                ratio(flows.parent_out.major),
                ratio(flows.external_in.major)
            );
        }
    }
}
