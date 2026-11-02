//! Complete declared debt terms and evaluated service, coverage and sweep rows.
//! Determinism: Exact (rendering preserves supplied and evaluated values without arithmetic).

use std::{collections::BTreeMap, fmt::Write as _};

use super::*;
use crate::entity::EntityFundingState;
use crate::render::{currency, js, md, mq, ratio, rq};

pub(crate) fn coverage_name(status: &DebtCoverageStatus) -> &'static str {
    match status {
        DebtCoverageStatus::Covered => "covered",
        DebtCoverageStatus::BelowMinimum => "below_minimum",
        DebtCoverageStatus::NoServiceDue => "no_service_due",
    }
}

fn coverage_label(status: &DebtCoverageStatus) -> &'static str {
    match status {
        DebtCoverageStatus::Covered => "Coverage threshold met",
        DebtCoverageStatus::BelowMinimum => "Below coverage threshold",
        DebtCoverageStatus::NoServiceDue => "No scheduled service due",
    }
}

pub(crate) fn funding_json(debt: &EntityDebtFunding, periods: &BTreeMap<&str, usize>) -> String {
    let position = |id: &str| periods.get(id).copied().unwrap_or(usize::MAX);
    let mut facilities: Vec<_> = debt.facilities.iter().collect();
    facilities.sort_by(|a, b| a.id.cmp(&b.id));
    let facilities = facilities.iter().map(|f| format!(
        "{{\"id\":{},\"entity_id\":{},\"basis\":{},\"cfads_basis\":{},\"minimum_dscr\":{},\"sweep_fraction\":{},\"allow_distributions_without_service\":{}}}",
        js(&f.id), js(&f.entity_id), js(&f.basis), js(&f.cfads_basis), rq(f.minimum_dscr), rq(f.sweep_fraction), f.allow_distributions_without_service
    )).collect::<Vec<_>>().join(",");
    let mut opening: Vec<_> = debt.opening_principal.iter().collect();
    opening.sort_by(|a, b| (&a.facility_id, &a.state_id).cmp(&(&b.facility_id, &b.state_id)));
    let opening = opening
        .iter()
        .map(|p| {
            format!(
                "{{\"facility_id\":{},\"state_id\":{},\"value\":{},\"currency\":{}}}",
                js(&p.facility_id),
                js(&p.state_id),
                mq(p.value),
                currency(p.value.ccy)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut baseline: Vec<_> = debt.baseline_flows.iter().collect();
    baseline.sort_by(|a, b| {
        (
            position(&a.period_id),
            &a.period_id,
            &a.facility_id,
            &a.state_id,
        )
            .cmp(&(
                position(&b.period_id),
                &b.period_id,
                &b.facility_id,
                &b.state_id,
            ))
    });
    let baseline = baseline.iter().map(|f| format!(
        "{{\"facility_id\":{},\"period_id\":{},\"state_id\":{},\"interest_rate\":{},\"draw\":{},\"draw_currency\":{},\"principal_payment_cap\":{},\"principal_payment_cap_currency\":{},\"cfads\":{},\"cfads_currency\":{}}}",
        js(&f.facility_id), js(&f.period_id), js(&f.state_id), rq(f.interest_rate), mq(f.draw), currency(f.draw.ccy), mq(f.principal_payment_cap), currency(f.principal_payment_cap.ccy), mq(f.cfads), currency(f.cfads.ccy)
    )).collect::<Vec<_>>().join(",");
    let mut candidates: Vec<_> = debt.candidate_flows.iter().collect();
    candidates.sort_by(|a, b| {
        (
            position(&a.period_id),
            &a.period_id,
            &a.facility_id,
            &a.candidate_id,
            &a.state_id,
        )
            .cmp(&(
                position(&b.period_id),
                &b.period_id,
                &b.facility_id,
                &b.candidate_id,
                &b.state_id,
            ))
    });
    let candidates = candidates.iter().map(|f| format!(
        "{{\"facility_id\":{},\"period_id\":{},\"candidate_id\":{},\"state_id\":{},\"draw\":{},\"draw_currency\":{},\"principal_payment_cap\":{},\"principal_payment_cap_currency\":{},\"cfads\":{},\"cfads_currency\":{}}}",
        js(&f.facility_id), js(&f.period_id), js(&f.candidate_id), js(&f.state_id), mq(f.draw), currency(f.draw.ccy), mq(f.principal_payment_cap), currency(f.principal_payment_cap.ccy), mq(f.cfads), currency(f.cfads.ccy)
    )).collect::<Vec<_>>().join(",");
    format!(
        "{{\"cash_flow_convention\":{},\"facilities\":[{facilities}],\"opening_principal\":[{opening}],\"baseline_flows\":[{baseline}],\"candidate_flows\":[{candidates}]}}",
        js(&debt.cash_flow_convention)
    )
}

pub(crate) fn cash_flows_json(flows: &DebtCashFlows) -> String {
    format!(
        "{{\"draw\":{},\"interest\":{},\"scheduled_principal\":{},\"sweep\":{}}}",
        rq(flows.draw.major),
        rq(flows.interest.major),
        rq(flows.scheduled_principal.major),
        rq(flows.sweep.major)
    )
}

pub(crate) fn gate_json(gate: &DebtDistributionGate) -> String {
    format!(
        "{{\"facility_id\":{},\"dscr\":{},\"minimum_dscr\":{},\"coverage_status\":{},\"distribution_allowed\":{},\"effective_allowed\":{}}}",
        js(&gate.facility_id),
        gate.dscr.map_or_else(|| "null".into(), rq),
        rq(gate.minimum_dscr),
        js(coverage_name(&gate.coverage_status)),
        gate.distribution_allowed,
        gate.effective_allowed
    )
}

pub(crate) fn service_json(row: &DebtServiceResult) -> String {
    format!(
        "{{\"facility_id\":{},\"entity_id\":{},\"period_id\":{},\"checkpoint_date\":{},\"opening_principal\":{},\"baseline_draw\":{},\"candidate_draw\":{},\"draw\":{},\"principal_before_service\":{},\"interest_rate\":{},\"interest\":{},\"baseline_principal_payment_cap\":{},\"candidate_principal_payment_cap\":{},\"principal_payment_cap\":{},\"scheduled_principal\":{},\"scheduled_service\":{},\"baseline_cfads\":{},\"candidate_cfads\":{},\"cfads\":{},\"dscr\":{},\"minimum_dscr\":{},\"coverage_status\":{},\"distribution_allowed\":{},\"allow_distributions_without_service\":{},\"cash_before_service\":{},\"cash_after_service\":{},\"cash_before_sweep\":{},\"minimum_cash\":{},\"sweep_fraction\":{},\"available_sweep_cash\":{},\"sweep\":{},\"cash_after_sweep\":{},\"closing_principal\":{},\"principal_residual\":{}}}",
        js(&row.facility_id),
        js(&row.entity_id),
        js(&row.period_id),
        js(&row.checkpoint_date),
        rq(row.opening_principal.major),
        rq(row.baseline_draw.major),
        rq(row.candidate_draw.major),
        rq(row.draw.major),
        rq(row.principal_before_service.major),
        rq(row.interest_rate),
        rq(row.interest.major),
        rq(row.baseline_principal_payment_cap.major),
        rq(row.candidate_principal_payment_cap.major),
        rq(row.principal_payment_cap.major),
        rq(row.scheduled_principal.major),
        rq(row.scheduled_service.major),
        rq(row.baseline_cfads.major),
        rq(row.candidate_cfads.major),
        rq(row.cfads.major),
        row.dscr.map_or_else(|| "null".into(), rq),
        rq(row.minimum_dscr),
        js(coverage_name(&row.coverage_status)),
        row.distribution_allowed,
        row.allow_distributions_without_service,
        rq(row.cash_before_service.major),
        rq(row.cash_after_service.major),
        rq(row.cash_before_sweep.major),
        rq(row.minimum_cash.major),
        rq(row.sweep_fraction),
        rq(row.available_sweep_cash.major),
        rq(row.sweep.major),
        rq(row.cash_after_sweep.major),
        rq(row.closing_principal.major),
        rq(row.principal_residual.major)
    )
}

pub(crate) fn inputs_markdown(
    out: &mut String,
    debt: &EntityDebtFunding,
    periods: &BTreeMap<&str, usize>,
) {
    let _ = writeln!(
        out,
        "\n### Declared debt facilities\n\n**Cash-flow convention:** {}. Native cash excludes the modeled debt draws, interest, principal service and sweeps. Cash available for debt service (CFADS) is a separately declared covenant metric and is never added to cash. Rates are exact full-period rates applied to opening principal plus that period's draws; no annualization is inferred.\n\nMandatory interest and principal service precede every pre-distribution floor check. Child-first processing then sweeps each debtor's current cash before its own dividend, including receipts from children; roots are swept too. Later receipts cannot erase the earlier mandatory-service floor breach. Debt cash movements do not adjust already-net NAV again.\n\n| Facility | Entity | Minimum DSCR | Sweep fraction | Distribution allowed without service | Terms basis | CFADS basis |\n|---|---|---:|---:|---|---|---|",
        md(&debt.cash_flow_convention)
    );
    let mut facilities: Vec<_> = debt.facilities.iter().collect();
    facilities.sort_by(|a, b| a.id.cmp(&b.id));
    for facility in facilities {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} |",
            md(&facility.id),
            md(&facility.entity_id),
            ratio(facility.minimum_dscr),
            ratio(facility.sweep_fraction),
            facility.allow_distributions_without_service,
            md(&facility.basis),
            md(&facility.cfads_basis)
        );
    }
    out.push_str("\n### Declared debt opening principal\n\n| Facility | State | Principal (USD) |\n|---|---|---:|\n");
    let mut opening: Vec<_> = debt.opening_principal.iter().collect();
    opening.sort_by(|a, b| (&a.facility_id, &a.state_id).cmp(&(&b.facility_id, &b.state_id)));
    for principal in opening {
        let _ = writeln!(
            out,
            "| {} | {} | {} |",
            md(&principal.facility_id),
            md(&principal.state_id),
            mq(principal.value).trim_matches('"')
        );
    }
    let position = |id: &str| periods.get(id).copied().unwrap_or(usize::MAX);
    let mut baseline: Vec<_> = debt.baseline_flows.iter().collect();
    baseline.sort_by(|a, b| {
        (
            position(&a.period_id),
            &a.period_id,
            &a.facility_id,
            &a.state_id,
        )
            .cmp(&(
                position(&b.period_id),
                &b.period_id,
                &b.facility_id,
                &b.state_id,
            ))
    });
    out.push_str("\n### Declared baseline debt service and CFADS\n\nDraws, principal caps and CFADS are USD; the rate is a dimensionless exact fraction. Actual scheduled principal is capped by the principal owed. Every shared-state period is explicit, including zeros.\n\n| Period | Facility | State | Full-period rate | Draw | Principal payment cap | CFADS |\n|---|---|---|---:|---:|---:|---:|\n");
    for flow in baseline {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} |",
            md(&flow.period_id),
            md(&flow.facility_id),
            md(&flow.state_id),
            ratio(flow.interest_rate),
            mq(flow.draw).trim_matches('"'),
            mq(flow.principal_payment_cap).trim_matches('"'),
            mq(flow.cfads).trim_matches('"')
        );
    }
    let mut candidates: Vec<_> = debt.candidate_flows.iter().collect();
    candidates.sort_by(|a, b| {
        (
            position(&a.period_id),
            &a.period_id,
            &a.facility_id,
            &a.candidate_id,
            &a.state_id,
        )
            .cmp(&(
                position(&b.period_id),
                &b.period_id,
                &b.facility_id,
                &b.candidate_id,
                &b.state_id,
            ))
    });
    out.push_str("\n### Declared candidate debt increments\n\nEach selected candidate adds its declared draw, principal payment cap and CFADS in the same shared state. All values are USD. Unselected and losing candidates' complete inputs remain visible.\n\n| Period | Facility | Candidate | State | Draw | Principal payment cap | CFADS |\n|---|---|---|---|---:|---:|---:|\n");
    for flow in candidates {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} |",
            md(&flow.period_id),
            md(&flow.facility_id),
            md(&flow.candidate_id),
            md(&flow.state_id),
            mq(flow.draw).trim_matches('"'),
            mq(flow.principal_payment_cap).trim_matches('"'),
            mq(flow.cfads).trim_matches('"')
        );
    }
}

pub(crate) fn checkpoint_markdown(out: &mut String, state: &EntityFundingState) {
    if state.debt.is_empty() {
        return;
    }
    out.push_str("\n**Debt movement detail by checkpoint**\n\nDraws are cash receipts; interest, scheduled principal and sweeps are cash expenditures. All amounts are USD. Opening checkpoints have no period movements.\n\n| Entity | Period | Phase | Draw | Interest | Scheduled principal | Sweep |\n|---|---|---|---:|---:|---:|---:|\n");
    for checkpoint in &state.checkpoints {
        if let Some(debt) = &checkpoint.debt {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} |",
                md(&checkpoint.entity_id),
                checkpoint
                    .period_id
                    .as_deref()
                    .map_or_else(|| "Opening".into(), md),
                crate::entity::render::phase_label(&checkpoint.phase),
                ratio(debt.draw.major),
                ratio(debt.interest.major),
                ratio(debt.scheduled_principal.major),
                ratio(debt.sweep.major)
            );
        }
    }
}

pub(crate) fn assessment_markdown(out: &mut String, state: &EntityFundingState) {
    if state.debt.is_empty() {
        return;
    }
    out.push_str("\n**Debt service and principal balances**\n\nRows are ordered by period and facility. Cash and principal values are exact USD. Principal conservation requires closing principal plus scheduled principal and sweep to equal opening principal plus draw; every residual is retained. Mandatory service is debited in full even when the resulting cash violates the floor.\n\n| Facility | Entity | Period | Date | Opening principal | Draw | Scheduled principal | Sweep | Closing principal | Principal residual |\n|---|---|---|---|---:|---:|---:|---:|---:|---:|\n");
    for row in &state.debt {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            md(&row.facility_id),
            md(&row.entity_id),
            md(&row.period_id),
            md(&row.checkpoint_date),
            ratio(row.opening_principal.major),
            ratio(row.draw.major),
            ratio(row.scheduled_principal.major),
            ratio(row.sweep.major),
            ratio(row.closing_principal.major),
            ratio(row.principal_residual.major)
        );
    }
    out.push_str("\n**Debt calculation inputs by selected program**\n\n| Facility | Period | Baseline draw | Candidate draw | Baseline principal cap | Candidate principal cap | Baseline CFADS | Candidate CFADS |\n|---|---|---:|---:|---:|---:|---:|---:|\n");
    for row in &state.debt {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            md(&row.facility_id),
            md(&row.period_id),
            ratio(row.baseline_draw.major),
            ratio(row.candidate_draw.major),
            ratio(row.baseline_principal_payment_cap.major),
            ratio(row.candidate_principal_payment_cap.major),
            ratio(row.baseline_cfads.major),
            ratio(row.candidate_cfads.major)
        );
    }
    out.push_str("\n**Interest and scheduled principal**\n\nInterest uses principal before service, including this period's draw. Scheduled service is interest plus the applied principal; sweeps are excluded from its coverage denominator.\n\n| Facility | Period | Principal before service | Full-period rate | Interest | Principal cap | Applied principal | Scheduled service |\n|---|---|---:|---:|---:|---:|---:|---:|\n");
    for row in &state.debt {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            md(&row.facility_id),
            md(&row.period_id),
            ratio(row.principal_before_service.major),
            ratio(row.interest_rate),
            ratio(row.interest.major),
            ratio(row.principal_payment_cap.major),
            ratio(row.scheduled_principal.major),
            ratio(row.scheduled_service.major)
        );
    }
    out.push_str("\n**Debt coverage**\n\nDSCR is declared CFADS divided by scheduled service. Threshold equality passes. When no service is due, the ratio is unavailable and the explicit no-service permission determines debt eligibility. A coverage lock retains dividends and does not itself declare insolvency.\n\n| Facility | Period | CFADS | Scheduled service | DSCR | Minimum DSCR | Coverage status | Allowed without service | Debt allows distribution |\n|---|---|---:|---:|---:|---:|---|---|---|\n");
    for row in &state.debt {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            md(&row.facility_id),
            md(&row.period_id),
            ratio(row.cfads.major),
            ratio(row.scheduled_service.major),
            row.dscr
                .map_or_else(|| "Unavailable: no service due".into(), ratio),
            ratio(row.minimum_dscr),
            coverage_label(&row.coverage_status),
            row.allow_distributions_without_service,
            row.distribution_allowed
        );
    }
    out.push_str("\n**Cash sweeps before distributions**\n\nCash before service is measured before the current draw; cash after service includes the draw less interest and scheduled principal. Each sweep uses current cash above the entity floor, including child dividends already received, and cannot exceed remaining principal. Sweeps are applied to roots as well as distributing entities. Current child receipts can fund a sweep, but cannot repair the mandatory-service checkpoint taken earlier.\n\n| Facility | Entity | Period | Cash before service | Cash after service | Cash before sweep | Floor | Available sweep cash | Sweep fraction | Sweep | Cash after sweep |\n|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for row in &state.debt {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            md(&row.facility_id),
            md(&row.entity_id),
            md(&row.period_id),
            ratio(row.cash_before_service.major),
            ratio(row.cash_after_service.major),
            ratio(row.cash_before_sweep.major),
            ratio(row.minimum_cash.major),
            ratio(row.available_sweep_cash.major),
            ratio(row.sweep_fraction),
            ratio(row.sweep.major),
            ratio(row.cash_after_sweep.major)
        );
    }
}

pub(crate) fn gates_markdown(out: &mut String, state: &EntityFundingState) {
    if state
        .distributions
        .iter()
        .all(|row| row.debt_gate.is_none())
    {
        return;
    }
    out.push_str("\n**Debt coverage and effective distribution gates**\n\nThe effective permission requires both the original declared gate and debt eligibility. The declared cap and ownership share still apply.\n\n| Entity | Facility | Period | Declared allowed | DSCR | Minimum DSCR | Coverage status | Debt allowed | Effective allowed |\n|---|---|---|---|---:|---:|---|---|---|\n");
    for distribution in &state.distributions {
        if let Some(gate) = &distribution.debt_gate {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                md(&distribution.entity_id),
                md(&gate.facility_id),
                md(&distribution.period_id),
                distribution.allowed,
                gate.dscr
                    .map_or_else(|| "Unavailable: no service due".into(), ratio),
                ratio(gate.minimum_dscr),
                coverage_label(&gate.coverage_status),
                gate.distribution_allowed,
                gate.effective_allowed
            );
        }
    }
}

pub(crate) fn conservation_markdown(out: &mut String, state: &EntityFundingState) {
    if state.conservation.iter().all(|row| row.debt.is_none()) {
        return;
    }
    out.push_str("\n**External debt movements in cash conservation**\n\nThese graph totals are already included in each period's cash-conservation residual. Draws enter the modeled graph; interest, scheduled principal and sweeps leave it.\n\n| Period | Date | Draw | Interest | Scheduled principal | Sweep |\n|---|---|---:|---:|---:|---:|\n");
    for conservation in &state.conservation {
        if let Some(debt) = &conservation.debt {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} |",
                md(&conservation.period_id),
                md(&conservation.checkpoint_date),
                ratio(debt.draw.major),
                ratio(debt.interest.major),
                ratio(debt.scheduled_principal.major),
                ratio(debt.sweep.major)
            );
        }
    }
}
