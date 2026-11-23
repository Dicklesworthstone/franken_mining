//! Conditional quarterly gold cost rates from matching cumulative rates and sold quantities.
//! All operands must come from one filing; competing vintages never disappear through selection.

use super::*;
use fmine_metrics::public_derivation::quarter_rate::{self, PrintedPeriod};

fn family(cell: &KeyedCell) -> Option<&'static str> {
    match (cell.key.line.label.as_str(), cell.key.unit.as_str()) {
        ("html:aisc_usd_per_oz_sold", "usd_per_oz") => Some("aisc_per_oz"),
        ("html:cos_usd_per_oz_sold", "usd_per_oz") => Some("cos_per_oz"),
        _ => None,
    }
}

fn printed(source: &HtmlClaim, rate: bool) -> Result<Printed, Refusal> {
    let cell = &source.cell;
    let invalid = || {
        Refusal::new(
            RefusalCode::SourceCellUnusable208,
            format!(
                "quarterly cost-rate arithmetic requires an unflagged exact source at {}",
                cell.locator
            ),
        )
    };
    cell.claim.check_id()?;
    let PublicValue::Number {
        stored,
        displayed: Ok(displayed),
        under_status: None,
    } = &cell.claim.value
    else {
        return Err(invalid());
    };
    let number = source.exact_number.as_ref().ok_or_else(invalid)?;
    let additivity = if rate {
        AdditivityClass::IntensiveWeighted
    } else {
        AdditivityClass::ExtensiveAdditive
    };
    if stored != &number.raw
        || displayed != &number.raw
        || number.percent
        || cell.claim.metric_key != cell.key.line.label
        || cell.claim.value_tag() != canonical_value_tag(&cell.key, &cell.claim.value)
        || cell.key.scope != Scope::Site
        || cell.printed_basis != cell.key.ownership_basis
        || cell.key.ownership_basis == OwnershipBasis::Unprinted
        || cell.claim.additivity != additivity
        || cell.claim.support != SpatialSupport::PeriodAggregate
        || cell.claim.representation != Representation::Deterministic
        || cell.claim.stale
        || cell.claim.calibration_failed
        || !matches!(
            cell.claim.state,
            KnowledgeState::Known | KnowledgeState::Estimated
        )
        || !cell.claim.confidentiality.is_public()
    {
        return Err(invalid());
    }
    Ok(Printed {
        value: decimal(number.value)?,
        step: decimal(number.step)?,
    })
}

pub(super) fn finding(
    draft: &str,
    finding: &LintFinding,
    snapshot: &HtmlClaimsReport,
) -> Result<Option<(String, String)>, Refusal> {
    if finding.claim_id.is_some() || finding.source.is_some() || !finding.candidates.is_empty() {
        return Ok(None);
    }
    let Some((prototype, target, family)) = snapshot.claims.iter().find_map(|source| {
        Some((
            source,
            equality_target(draft, finding.span, &source.cell)?,
            family(&source.cell)?,
        ))
    }) else {
        return Ok(None);
    };
    let periods = match target.period.kind {
        PeriodKind::Quarter(2) => [PeriodKind::Quarter(1), PeriodKind::H1],
        PeriodKind::Quarter(3) => [PeriodKind::H1, PeriodKind::NineMonths],
        PeriodKind::Quarter(4) => [PeriodKind::NineMonths, PeriodKind::FullYear],
        _ => return Ok(None),
    };
    let is_rate = |source: &HtmlClaim| source.cell.key.line.label == prototype.cell.key.line.label;
    let mut sources: Vec<_> = snapshot
        .claims
        .iter()
        .filter(|source| {
            let cell = &source.cell;
            cell.key.issuer == prototype.cell.key.issuer
                && cell.key.subject == prototype.cell.key.subject
                && cell.key.valid_period.year == target.period.year
                && ((is_rate(source)
                    && (periods.contains(&cell.key.valid_period.kind)
                        || cell.key.valid_period == target.period))
                    || (cell.key.line.label == "html:gold_sales_koz"
                        && cell.key.unit == "koz"
                        && periods.contains(&cell.key.valid_period.kind)))
        })
        .collect();
    sources.sort_by_key(|source| {
        (
            source.cell.key.valid_period,
            !is_rate(source),
            source.cell.claim.claim_id,
        )
    });
    let mut status = "not_available";
    let mut reason =
        "both cumulative periods require their own rate and sold-gold quantity".to_string();
    let mut derived = None;
    let mut draft_interval = None;
    let mut refusal = None;
    let evaluate = (|| -> Result<(), Refusal> {
        let draft = Printed {
            value: decimal(target.value)?,
            step: decimal(target.step)?,
        }
        .interval()
        .map_err(|why| Refusal::new(RefusalCode::NumericalCertificateFailed207, why))?;
        draft_interval = Some(draft);
        if sources
            .iter()
            .any(|source| source.cell.key.valid_period == target.period)
        {
            status = "direct_assertion_available";
            reason =
                "inspect or cite the directly reported rate; arithmetic does not replace it".into();
            return Ok(());
        }
        let mut slots = Vec::new();
        for period in periods {
            for rate in [true, false] {
                slots.push(
                    sources
                        .iter()
                        .copied()
                        .filter(|source| {
                            source.cell.key.valid_period.kind == period && is_rate(source) == rate
                        })
                        .collect::<Vec<_>>(),
                );
            }
        }
        if slots.iter().any(|slot| slot.len() > 1) {
            status = "ambiguous";
            reason = "competing admitted rate or sales assertions remain visible; no vintage is selected by amount".into();
            return Ok(());
        }
        if slots.iter().any(Vec::is_empty) {
            return Ok(());
        }
        let [pr, pq, cr, cq] = [slots[0][0], slots[1][0], slots[2][0], slots[3][0]];
        let key = &pr.cell.key;
        for source in [pq, cr, cq] {
            if source.cell.key.ownership_basis != key.ownership_basis
                || source.cell.key.reporting_vintage != key.reporting_vintage
                || source.cell.key.line.occurrence != key.line.occurrence
            {
                return Err(Refusal::new(
                    RefusalCode::BasisMismatch105,
                    "quarterly cost-rate operands must share one filing, subject perimeter and printed ownership basis",
                ));
            }
        }
        let output = quarter_rate::derive(
            family,
            calendar(target.period),
            [
                &pr.cell.claim,
                &pq.cell.claim,
                &cr.cell.claim,
                &cq.cell.claim,
            ],
            PrintedPeriod {
                rate: printed(pr, true)?,
                sales: printed(pq, false)?,
            },
            PrintedPeriod {
                rate: printed(cr, true)?,
                sales: printed(cq, false)?,
            },
        )?;
        status = compare(draft, output.value)?;
        reason = match status {
            "derived" => "under the stated sales-weighted identity, the exact source enclosure fits the draft precision",
            "contradicted" => "under the stated sales-weighted identity, the exact source enclosure is disjoint from the draft",
            _ => "source rounding uncertainty exceeds the draft precision under the stated sales-weighted identity",
        }.into();
        derived = Some(output);
        Ok(())
    })();
    if let Err(error) = evaluate {
        status = "refused";
        reason = error.what.clone();
        refusal = Some(error);
    }
    let evidence = sources
        .iter()
        .map(|source| {
            Ok(format!(
                "{{\"role\":{},\"claim\":{},\"publication\":{}}}",
                js(if is_rate(source) {
                    "cumulative_rate"
                } else {
                    "cumulative_sales"
                }),
                canonical_claim_json(&source.cell.claim, "html_source", source.cell.to_json())?,
                source_date(&source.cell, snapshot)?,
            ))
        })
        .collect::<Result<Vec<_>, Refusal>>()?
        .join(",");
    let ownership = match prototype.cell.key.ownership_basis {
        OwnershipBasis::Consolidated100 => "100%".into(),
        OwnershipBasis::Attributable { num, den, .. } => format!("attributable:{num}/{den}"),
        OwnershipBasis::Unprinted => "unprinted".into(),
    };
    let value_json = |interval| {
        format!(
            "{{\"issuer\":{},\"subject\":{},\"metric\":{},\"unit\":\"usd_per_oz\",\"ownership\":{},\"period\":{},\"interval\":{}}}",
            js(&prototype.cell.key.issuer),
            js(&prototype.cell.key.subject),
            js(family),
            js(&ownership),
            js(&target.period.code()),
            interval_json(interval),
        )
    };
    let derived_json = derived
        .as_ref()
        .map(|claim| {
            canonical_claim_json(
                claim,
                "html_sales_weighted_quarter_rate",
                value_json(claim.value),
            )
        })
        .transpose()?
        .unwrap_or_else(|| "null".into());
    let refusal_json = refusal.as_ref().map_or_else(
        || "null".into(),
        |r| {
            format!(
                "{{\"code\":{},\"what\":{},\"fix\":{}}}",
                js(r.code),
                js(&r.what),
                js(r.fix),
            )
        },
    );
    let json = format!(
        "{{\"span\":{{\"start_byte\":{},\"end_byte\":{}}},\"status\":{},\"reason\":{},\"rule\":{},\"formula\":\"(current_rate*current_sales-prior_rate*prior_sales)/(current_sales-prior_sales)\",\"assumption\":{},\"target_period\":{},\"unit\":\"usd_per_oz\",\"draft_interval\":{},\"source_evidence\":[{evidence}],\"derived_claim\":{derived_json},\"refusal\":{refusal_json}}}",
        finding.span.0,
        finding.span.1,
        js(status),
        js(&reason),
        js(quarter_rate::RULE),
        js(quarter_rate::ASSUMPTION),
        js(&target.period.code()),
        draft_interval.map_or_else(|| "null".into(), interval_json),
    );
    let mut markdown = format!(
        "- Bytes {}–{}: **{status}**, sales-weighted quarterly {family}, {}. {}\n  - Assumption: {}\n",
        finding.span.0,
        finding.span.1,
        target.period.code(),
        crate::guidance::text(&reason),
        quarter_rate::ASSUMPTION,
    );
    if let Some(claim) = derived {
        let _ = writeln!(
            markdown,
            "  - Derived Claim `{}`: [{}/{}, {}/{}] USD/oz.",
            claim.claim_id,
            claim.value.lo.num(),
            claim.value.lo.den(),
            claim.value.hi.num(),
            claim.value.hi.den()
        );
    }
    for source in sources {
        let _ = writeln!(
            markdown,
            "  - Source Claim `{}`: {}; SHA-256 `{}`.",
            source.cell.claim.claim_id,
            crate::guidance::text(&source.cell.locator),
            source.cell.key.reporting_vintage.file_sha256
        );
    }
    Ok(Some((json, markdown)))
}
