//! Complete declared entity funding and realized cash ledgers, without render-time arithmetic.
//! Determinism: Exact (every native amount, gate, transfer and ownership fraction remains visible).

use std::{collections::BTreeMap, fmt::Write as _};

use super::*;
use crate::render::{claim_markdown, currency, js, md, mq, ratio, rq};

pub(crate) fn phase_name(phase: &EntityCashPhase) -> &'static str {
    match phase {
        EntityCashPhase::Opening => "opening",
        EntityCashPhase::PreDistribution => "pre_distribution",
        EntityCashPhase::Closing => "closing",
    }
}

pub(crate) fn phase_label(phase: &EntityCashPhase) -> &'static str {
    match phase {
        EntityCashPhase::Opening => "Opening",
        EntityCashPhase::PreDistribution => "Before distributions",
        EntityCashPhase::Closing => "Closing",
    }
}

pub(crate) fn funding_json(funding: &EntityFunding) -> String {
    let mut periods: Vec<_> = funding.periods.iter().collect();
    periods.sort_by(|a, b| (&a.cash_period, &a.id).cmp(&(&b.cash_period, &b.id)));
    let order: BTreeMap<_, _> = periods
        .iter()
        .enumerate()
        .map(|(index, p)| (p.id.as_str(), index))
        .collect();
    let position = |id: &str| order.get(id).copied().unwrap_or(usize::MAX);
    let periods = periods
        .iter()
        .map(|p| {
            format!(
                "{{\"id\":{},\"cash_period\":{}}}",
                js(&p.id),
                js(&p.cash_period)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut entities: Vec<_> = funding.entities.iter().collect();
    entities.sort_by(|a, b| a.id.cmp(&b.id));
    let entities = entities
        .iter()
        .map(|e| format!("{{\"id\":{},\"label\":{}}}", js(&e.id), js(&e.label)))
        .collect::<Vec<_>>()
        .join(",");
    let mut floors: Vec<_> = funding.floors.iter().collect();
    floors.sort_by(|a, b| {
        (position(&a.period_id), &a.period_id, &a.entity_id).cmp(&(
            position(&b.period_id),
            &b.period_id,
            &b.entity_id,
        ))
    });
    let floors = floors.iter().map(|f| format!("{{\"entity_id\":{},\"period_id\":{},\"minimum_cash\":{},\"minimum_cash_currency\":{}}}", js(&f.entity_id), js(&f.period_id), mq(f.minimum_cash), currency(f.minimum_cash.ccy))).collect::<Vec<_>>().join(",");
    let mut opening: Vec<_> = funding.opening_cash.iter().collect();
    opening.sort_by(|a, b| (&a.entity_id, &a.state_id).cmp(&(&b.entity_id, &b.state_id)));
    let opening = opening
        .iter()
        .map(|c| {
            format!(
                "{{\"entity_id\":{},\"state_id\":{},\"value\":{},\"currency\":{}}}",
                js(&c.entity_id),
                js(&c.state_id),
                mq(c.value),
                currency(c.value.ccy)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut baseline: Vec<_> = funding.baseline_flows.iter().collect();
    baseline.sort_by(|a, b| {
        (
            position(&a.period_id),
            &a.period_id,
            &a.entity_id,
            &a.state_id,
        )
            .cmp(&(
                position(&b.period_id),
                &b.period_id,
                &b.entity_id,
                &b.state_id,
            ))
    });
    let baseline = baseline.iter().map(|f| format!("{{\"entity_id\":{},\"period_id\":{},\"state_id\":{},\"value\":{},\"currency\":{}}}", js(&f.entity_id), js(&f.period_id), js(&f.state_id), mq(f.value), currency(f.value.ccy))).collect::<Vec<_>>().join(",");
    let mut candidates: Vec<_> = funding.candidate_flows.iter().collect();
    candidates.sort_by(|a, b| {
        (
            position(&a.period_id),
            &a.period_id,
            &a.entity_id,
            &a.candidate_id,
            &a.state_id,
        )
            .cmp(&(
                position(&b.period_id),
                &b.period_id,
                &b.entity_id,
                &b.candidate_id,
                &b.state_id,
            ))
    });
    let candidates = candidates.iter().map(|f| format!("{{\"entity_id\":{},\"period_id\":{},\"candidate_id\":{},\"state_id\":{},\"value\":{},\"currency\":{}}}", js(&f.entity_id), js(&f.period_id), js(&f.candidate_id), js(&f.state_id), mq(f.value), currency(f.value.ccy))).collect::<Vec<_>>().join(",");
    let mut transfers: Vec<_> = funding.transfers.iter().collect();
    transfers.sort_by(|a, b| {
        (position(&a.period_id), &a.period_id, &a.id).cmp(&(
            position(&b.period_id),
            &b.period_id,
            &b.id,
        ))
    });
    let transfers = transfers.iter().map(|t| {
        let mut amounts: Vec<_> = t.amounts.iter().collect();
        amounts.sort_by(|a, b| a.0.cmp(&b.0));
        let amounts = amounts.iter().map(|(state, value)| format!("{{\"state_id\":{},\"value\":{},\"currency\":{}}}", js(state), mq(*value), currency(value.ccy))).collect::<Vec<_>>().join(",");
        format!("{{\"id\":{},\"from_entity\":{},\"to_entity\":{},\"period_id\":{},\"candidate_id\":{},\"amounts\":[{amounts}],\"basis\":{}}}", js(&t.id), js(&t.from_entity), js(&t.to_entity), js(&t.period_id), t.candidate_id.as_deref().map_or_else(|| "null".into(), js), js(&t.basis))
    }).collect::<Vec<_>>().join(",");
    let mut distributions: Vec<_> = funding.distributions.iter().collect();
    distributions.sort_by(|a, b| {
        (&a.entity_id, &a.parent_entity_id).cmp(&(&b.entity_id, &b.parent_entity_id))
    });
    let distributions = distributions
        .iter()
        .map(|d| {
            format!(
                "{{\"entity_id\":{},\"parent_entity_id\":{},\"parent_share\":{},\"basis\":{}}}",
                js(&d.entity_id),
                js(&d.parent_entity_id),
                rq(d.parent_share),
                js(&d.basis)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut gates: Vec<_> = funding.gates.iter().collect();
    gates.sort_by(|a, b| {
        (
            position(&a.period_id),
            &a.period_id,
            &a.entity_id,
            &a.state_id,
        )
            .cmp(&(
                position(&b.period_id),
                &b.period_id,
                &b.entity_id,
                &b.state_id,
            ))
    });
    let gates = gates.iter().map(|g| format!("{{\"entity_id\":{},\"period_id\":{},\"state_id\":{},\"allowed\":{},\"maximum_distribution\":{},\"maximum_distribution_currency\":{}}}", js(&g.entity_id), js(&g.period_id), js(&g.state_id), g.allowed, mq(g.maximum_distribution), currency(g.maximum_distribution.ccy))).collect::<Vec<_>>().join(",");
    let debt = funding.debt.as_ref().map_or_else(String::new, |debt| {
        format!(",\"debt\":{}", debt::render::funding_json(debt, &order))
    });
    format!(
        "{{\"model_key\":{},\"currency\":{},\"cash_basis\":{},\"opening_date\":{},\"objective_entity\":{},\"entities\":[{entities}],\"periods\":[{periods}],\"floors\":[{floors}],\"opening_cash\":[{opening}],\"baseline_flows\":[{baseline}],\"candidate_flows\":[{candidates}],\"transfers\":[{transfers}],\"distributions\":[{distributions}],\"gates\":[{gates}]{debt}}}",
        js(&funding.model_key),
        currency(funding.currency),
        js(&funding.cash_basis),
        js(&funding.opening_date),
        js(&funding.objective_entity)
    )
}

pub(crate) fn assessment_json(assessment: &EntityFundingAssessment) -> String {
    let states = assessment
        .states
        .iter()
        .map(state_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"states\":[{states}],\"minimum_slack\":{}}}",
        rq(assessment.minimum_slack.major)
    )
}

fn state_json(state: &EntityFundingState) -> String {
    let checkpoints = state.checkpoints.iter().map(|c| {
        let debt = c.debt.as_ref().map_or_else(String::new, |flows| format!(",\"debt\":{}", debt::render::cash_flows_json(flows)));
        format!(
        "{{\"entity_id\":{},\"period_id\":{},\"checkpoint_date\":{},\"phase\":{},\"opening_cash\":{},\"baseline_cash_flow\":{},\"candidate_cash_flow\":{},\"transfer_in\":{},\"transfer_out\":{},\"distribution_in\":{},\"distribution_out\":{},\"closing_cash\":{},\"minimum_cash\":{},\"slack\":{}{debt}}}",
        js(&c.entity_id), c.period_id.as_deref().map_or_else(|| "null".into(), js), js(&c.checkpoint_date), js(phase_name(&c.phase)), rq(c.opening_cash.major), rq(c.baseline_cash_flow.major), rq(c.candidate_cash_flow.major), rq(c.transfer_in.major), rq(c.transfer_out.major), rq(c.distribution_in.major), rq(c.distribution_out.major), rq(c.closing_cash.major), rq(c.minimum_cash.major), rq(c.slack.major)
    )}).collect::<Vec<_>>().join(",");
    let transfers = state.transfers.iter().map(|t| format!(
        "{{\"transfer_id\":{},\"from_entity\":{},\"to_entity\":{},\"period_id\":{},\"checkpoint_date\":{},\"candidate_id\":{},\"basis\":{},\"active\":{},\"declared_amount\":{},\"actual_amount\":{}}}",
        js(&t.transfer_id), js(&t.from_entity), js(&t.to_entity), js(&t.period_id), js(&t.checkpoint_date), t.candidate_id.as_deref().map_or_else(|| "null".into(), js), js(&t.basis), t.active, rq(t.declared_amount.major), rq(t.actual_amount.major)
    )).collect::<Vec<_>>().join(",");
    let distributions = state.distributions.iter().map(|d| {
        let debt_gate = d.debt_gate.as_ref().map_or_else(String::new, |gate| format!(",\"debt_gate\":{}", debt::render::gate_json(gate)));
        format!(
        "{{\"entity_id\":{},\"parent_entity_id\":{},\"period_id\":{},\"checkpoint_date\":{},\"basis\":{},\"parent_share\":{},\"external_share\":{},\"allowed\":{},\"maximum_distribution\":{},\"cash_before_distribution\":{},\"minimum_cash\":{},\"available_surplus\":{},\"gross_distribution\":{},\"parent_receipt\":{},\"external_distribution\":{},\"cash_after_distribution\":{}{debt_gate}}}",
        js(&d.entity_id), js(&d.parent_entity_id), js(&d.period_id), js(&d.checkpoint_date), js(&d.basis), rq(d.parent_share), rq(d.external_share), d.allowed, rq(d.maximum_distribution.major), rq(d.cash_before_distribution.major), rq(d.minimum_cash.major), rq(d.available_surplus.major), rq(d.gross_distribution.major), rq(d.parent_receipt.major), rq(d.external_distribution.major), rq(d.cash_after_distribution.major)
    )}).collect::<Vec<_>>().join(",");
    let conservation = state.conservation.iter().map(|c| {
        let debt = c.debt.as_ref().map_or_else(String::new, |flows| format!(",\"debt\":{}", debt::render::cash_flows_json(flows)));
        format!(
        "{{\"period_id\":{},\"checkpoint_date\":{},\"opening_cash\":{},\"baseline_cash_flow\":{},\"candidate_cash_flow\":{},\"internal_transfers\":{},\"internal_distributions\":{},\"external_distributions\":{},\"closing_cash\":{},\"residual\":{}{debt}}}",
        js(&c.period_id), js(&c.checkpoint_date), rq(c.opening_cash.major), rq(c.baseline_cash_flow.major), rq(c.candidate_cash_flow.major), rq(c.internal_transfers.major), rq(c.internal_distributions.major), rq(c.external_distributions.major), rq(c.closing_cash.major), rq(c.residual.major)
    )}).collect::<Vec<_>>().join(",");
    let debt = if state.debt.is_empty() {
        String::new()
    } else {
        format!(
            ",\"debt\":[{}]",
            state
                .debt
                .iter()
                .map(debt::render::service_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    format!(
        "{{\"state_id\":{},\"checkpoints\":[{checkpoints}],\"transfers\":[{transfers}],\"distributions\":[{distributions}],\"conservation\":[{conservation}],\"minimum_slack\":{}{debt}}}",
        js(&state.state_id),
        rq(state.minimum_slack.major)
    )
}

pub(crate) fn inputs_markdown(out: &mut String, claim: &Claim<EntityFunding>) {
    let f = &claim.value;
    let _ = writeln!(
        out,
        "\n## Entity cash, capital calls and distributions\n\n**Objective entity:** {}. **Opening date:** {}. **Cash basis:** {}.\n\nEach entity retains its own cash and floor. Native baseline and selected-candidate flows and explicit fixed transfers are applied before distributions. Floors are checked at opening, before distributions and at closing; an earlier breach remains a rejection. Distributions follow the declared acyclic ownership graph from children to parents, obey each state's permission gate and cap, and retain the donor's floor. The ownership remainder leaves the modeled graph and is never credited to the parent.\n\nThese are exact declared checkpoint calculations, not a guarantee of every intra-period cash balance, inferred partner funding, or inferred legal permission. Monetary values are exact rational USD. Cash transfers and distributions do not add to or subtract from the already-net NAV inputs again.\n",
        md(&f.objective_entity),
        md(&f.opening_date),
        md(&f.cash_basis)
    );
    claim_markdown(out, claim);
    let mut entities: Vec<_> = f.entities.iter().collect();
    entities.sort_by(|a, b| a.id.cmp(&b.id));
    out.push_str("\n### Declared entities\n\n| Entity | Label |\n|---|---|\n");
    for entity in entities {
        let _ = writeln!(out, "| {} | {} |", md(&entity.id), md(&entity.label));
    }
    let mut periods: Vec<_> = f.periods.iter().collect();
    periods.sort_by(|a, b| (&a.cash_period, &a.id).cmp(&(&b.cash_period, &b.id)));
    let order: BTreeMap<_, _> = periods
        .iter()
        .enumerate()
        .map(|(index, p)| (p.id.as_str(), index))
        .collect();
    let position = |id: &str| order.get(id).copied().unwrap_or(usize::MAX);
    out.push_str(
        "\n### Declared entity cash periods\n\n| Period | Inclusive cash period |\n|---|---|\n",
    );
    for period in &periods {
        let _ = writeln!(out, "| {} | {} |", md(&period.id), md(&period.cash_period));
    }
    let mut floors: Vec<_> = f.floors.iter().collect();
    floors.sort_by(|a, b| {
        (position(&a.period_id), &a.entity_id).cmp(&(position(&b.period_id), &b.entity_id))
    });
    out.push_str("\n### Declared entity floors\n\nThe first period's floor also applies at opening.\n\n| Period | Entity | Minimum cash (USD) |\n|---|---|---:|\n");
    for floor in floors {
        let _ = writeln!(
            out,
            "| {} | {} | {} |",
            md(&floor.period_id),
            md(&floor.entity_id),
            mq(floor.minimum_cash).trim_matches('"')
        );
    }
    let mut opening: Vec<_> = f.opening_cash.iter().collect();
    opening.sort_by(|a, b| (&a.entity_id, &a.state_id).cmp(&(&b.entity_id, &b.state_id)));
    out.push_str(
        "\n### Declared entity opening cash\n\n| Entity | State | Cash (USD) |\n|---|---|---:|\n",
    );
    for cash in opening {
        let _ = writeln!(
            out,
            "| {} | {} | {} |",
            md(&cash.entity_id),
            md(&cash.state_id),
            mq(cash.value).trim_matches('"')
        );
    }
    let mut baseline: Vec<_> = f.baseline_flows.iter().collect();
    baseline.sort_by(|a, b| {
        (position(&a.period_id), &a.entity_id, &a.state_id).cmp(&(
            position(&b.period_id),
            &b.entity_id,
            &b.state_id,
        ))
    });
    out.push_str("\n### Declared native baseline cash flows\n\nPositive values are external receipts; negative values are external expenditures. Internal transfers are declared separately.\n\n| Period | Entity | State | Net cash flow (USD) |\n|---|---|---|---:|\n");
    for flow in baseline {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            md(&flow.period_id),
            md(&flow.entity_id),
            md(&flow.state_id),
            mq(flow.value).trim_matches('"')
        );
    }
    let mut candidates: Vec<_> = f.candidate_flows.iter().collect();
    candidates.sort_by(|a, b| {
        (
            position(&a.period_id),
            &a.entity_id,
            &a.candidate_id,
            &a.state_id,
        )
            .cmp(&(
                position(&b.period_id),
                &b.entity_id,
                &b.candidate_id,
                &b.state_id,
            ))
    });
    out.push_str("\n### Declared native candidate cash flows\n\nAll candidates and explicit zeros remain visible, including candidates absent from every winning choice.\n\n| Period | Entity | Candidate | State | Net cash flow (USD) |\n|---|---|---|---|---:|\n");
    for flow in candidates {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            md(&flow.period_id),
            md(&flow.entity_id),
            md(&flow.candidate_id),
            md(&flow.state_id),
            mq(flow.value).trim_matches('"')
        );
    }
    let mut transfers: Vec<_> = f.transfers.iter().collect();
    transfers.sort_by(|a, b| (position(&a.period_id), &a.id).cmp(&(position(&b.period_id), &b.id)));
    out.push_str("\n### Declared fixed transfers and capital calls\n\nEvery transfer has a modeled payer and recipient. Candidate-linked transfers occur only when that candidate is selected; other transfers are unconditional.\n\n| Transfer | Period | From | To | Activation | State | Amount (USD) | Basis |\n|---|---|---|---|---|---|---:|---|\n");
    for transfer in transfers {
        let mut amounts: Vec<_> = transfer.amounts.iter().collect();
        amounts.sort_by(|a, b| a.0.cmp(&b.0));
        for (state, amount) in amounts {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                md(&transfer.id),
                md(&transfer.period_id),
                md(&transfer.from_entity),
                md(&transfer.to_entity),
                transfer
                    .candidate_id
                    .as_deref()
                    .map_or_else(|| "Unconditional".into(), md),
                md(state),
                mq(*amount).trim_matches('"'),
                md(&transfer.basis)
            );
        }
    }
    let mut distributions: Vec<_> = f.distributions.iter().collect();
    distributions.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
    out.push_str("\n### Declared ownership distributions\n\nThe parent's declared share is applied to the gross distribution. The complementary share is paid outside the modeled graph; it is explicit in every realized distribution below.\n\n| Distributing entity | Parent entity | Parent share | Basis |\n|---|---|---:|---|\n");
    for distribution in distributions {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            md(&distribution.entity_id),
            md(&distribution.parent_entity_id),
            ratio(distribution.parent_share),
            md(&distribution.basis)
        );
    }
    let mut gates: Vec<_> = f.gates.iter().collect();
    gates.sort_by(|a, b| {
        (position(&a.period_id), &a.entity_id, &a.state_id).cmp(&(
            position(&b.period_id),
            &b.entity_id,
            &b.state_id,
        ))
    });
    out.push_str("\n### Declared distribution gates and caps\n\nClosed gates retain cash in the distributing entity, even if another entity needs it.\n\n| Period | Entity | State | Gate allowed | Maximum distribution (USD) |\n|---|---|---|---|---:|\n");
    for gate in gates {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            md(&gate.period_id),
            md(&gate.entity_id),
            md(&gate.state_id),
            gate.allowed,
            mq(gate.maximum_distribution).trim_matches('"')
        );
    }
    if let Some(debt) = &f.debt {
        debt::render::inputs_markdown(out, debt, &order);
    }
}

pub(crate) fn assessment_markdown(
    out: &mut String,
    label: &str,
    assessment: &EntityFundingAssessment,
) {
    let _ = writeln!(
        out,
        "\n### Entity cash ledger: {}\n\nMinimum cash-floor slack across every entity, state and checkpoint: {} USD. Every rejected path and inactive transfer remains visible.\n",
        md(label),
        ratio(assessment.minimum_slack.major)
    );
    for state in &assessment.states {
        let _ = writeln!(
            out,
            "\n#### Shared state {}\n\nMinimum entity cash-floor slack: {} USD. All monetary columns below are USD; ownership shares are exact fractions.\n\n**Checkpoint balances and floors**\n\n| Entity | Period | Date | Phase | Opening cash | Closing cash | Floor | Slack | Result |\n|---|---|---|---|---:|---:|---:|---:|---|",
            md(&state.state_id),
            ratio(state.minimum_slack.major)
        );
        for c in &state.checkpoints {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                md(&c.entity_id),
                c.period_id.as_deref().map_or_else(|| "Opening".into(), md),
                md(&c.checkpoint_date),
                phase_label(&c.phase),
                ratio(c.opening_cash.major),
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
        out.push_str("\n**Movement detail by checkpoint**\n\n| Entity | Period | Phase | Baseline net | Candidate net | Transfer in | Transfer out | Distribution in | Distribution out |\n|---|---|---|---:|---:|---:|---:|---:|---:|\n");
        for c in &state.checkpoints {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                md(&c.entity_id),
                c.period_id.as_deref().map_or_else(|| "Opening".into(), md),
                phase_label(&c.phase),
                ratio(c.baseline_cash_flow.major),
                ratio(c.candidate_cash_flow.major),
                ratio(c.transfer_in.major),
                ratio(c.transfer_out.major),
                ratio(c.distribution_in.major),
                ratio(c.distribution_out.major)
            );
        }
        debt::render::checkpoint_markdown(out, state);
        debt::render::assessment_markdown(out, state);
        out.push_str("\n**Fixed transfers and capital calls**\n\nActual amounts debit the payer and credit the recipient equally.\n\n| Transfer | Period | Date | From | To | Candidate trigger | Active | Declared amount | Actual amount | Basis |\n|---|---|---|---|---|---|---|---:|---:|---|\n");
        for t in &state.transfers {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                md(&t.transfer_id),
                md(&t.period_id),
                md(&t.checkpoint_date),
                md(&t.from_entity),
                md(&t.to_entity),
                t.candidate_id
                    .as_deref()
                    .map_or_else(|| "Unconditional".into(), md),
                t.active,
                ratio(t.declared_amount.major),
                ratio(t.actual_amount.major),
                md(&t.basis)
            );
        }
        out.push_str("\n**Distribution gates and ownership allocation**\n\nRows retain the actual child-first execution order.\n\n| Entity | Parent | Period | Date | Allowed | Cap | Parent share | External share | Basis |\n|---|---|---|---|---|---:|---:|---:|---|\n");
        for d in &state.distributions {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                md(&d.entity_id),
                md(&d.parent_entity_id),
                md(&d.period_id),
                md(&d.checkpoint_date),
                d.allowed,
                ratio(d.maximum_distribution.major),
                ratio(d.parent_share),
                ratio(d.external_share),
                md(&d.basis)
            );
        }
        debt::render::gates_markdown(out, state);
        out.push_str("\n**Realized distributions**\n\nThe gross debit equals the parent's receipt plus the external ownership distribution.\n\n| Entity | Period | Cash before | Floor | Available surplus | Gross distribution | Parent receipt | External distribution | Cash after |\n|---|---|---:|---:|---:|---:|---:|---:|---:|\n");
        for d in &state.distributions {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                md(&d.entity_id),
                md(&d.period_id),
                ratio(d.cash_before_distribution.major),
                ratio(d.minimum_cash.major),
                ratio(d.available_surplus.major),
                ratio(d.gross_distribution.major),
                ratio(d.parent_receipt.major),
                ratio(d.external_distribution.major),
                ratio(d.cash_after_distribution.major)
            );
        }
        if state.debt.is_empty() {
            out.push_str("\n**Cash conservation across the modeled graph**\n\nInternal transfers and parent distribution receipts move cash between entities. External ownership distributions leave the graph. The exact residual is closing cash plus external distributions minus opening cash and native flows.\n\n| Period | Date | Opening cash | Baseline net | Candidate net | Internal transfers | Internal distributions | External distributions | Closing cash | Residual |\n|---|---|---:|---:|---:|---:|---:|---:|---:|---:|\n");
        } else {
            out.push_str("\n**Cash conservation across the modeled graph**\n\nInternal transfers and parent distribution receipts move cash between entities. External ownership distributions and debt service leave the graph; draws enter it. The exact residual is closing cash plus external distributions, interest, scheduled principal and sweeps, minus opening cash, native flows and draws. All debt movement totals are retained below.\n\n| Period | Date | Opening cash | Baseline net | Candidate net | Internal transfers | Internal distributions | External distributions | Closing cash | Residual |\n|---|---|---:|---:|---:|---:|---:|---:|---:|---:|\n");
        }
        for c in &state.conservation {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                md(&c.period_id),
                md(&c.checkpoint_date),
                ratio(c.opening_cash.major),
                ratio(c.baseline_cash_flow.major),
                ratio(c.candidate_cash_flow.major),
                ratio(c.internal_transfers.major),
                ratio(c.internal_distributions.major),
                ratio(c.external_distributions.major),
                ratio(c.closing_cash.major),
                ratio(c.residual.major)
            );
        }
        debt::render::conservation_markdown(out, state);
    }
}
