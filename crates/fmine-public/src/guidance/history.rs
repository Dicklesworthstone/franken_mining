//! Original-vintage rolling historical windows for the joint guidance test (BD-342).

use fmine_metrics::guidance::{HistoricalContinuation, HistoryVerdict, quarters};
use fmine_numerics::exact_feas::Certificate;

use super::*;

#[derive(Debug, Clone)]
pub struct HistoryExclusion {
    pub id: String,
    pub code: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct HistorySourceWindow {
    pub id: String,
    pub source_claims: Vec<KeyedCell>,
    pub quarters: Vec<HistoricalContinuation>,
}

#[derive(Debug, Clone)]
pub struct HistoricalEnvelope {
    pub comparison: quarters::QuarterlyHistoryReport,
    pub sources: Vec<HistorySourceWindow>,
    pub exclusions: Vec<HistoryExclusion>,
}

fn quarter(serial: u32) -> Result<String> {
    let year = serial / 4;
    if !(1..=9999).contains(&year) {
        return Err(refuse(
            RefusalCode::SupportMismatch102,
            "historical quarter outside the calendar",
        ));
    }
    Ok(format!("{year}Q{}", serial % 4 + 1))
}

fn window(
    inputs: &CompletionInputs,
    cells: &[KeyedCell],
    start: u32,
    count: u32,
) -> Result<HistorySourceWindow> {
    let subject = &inputs.production_guidance.value.subject;
    let prod = if inputs.production_guidance.value.ownership == Ownership::Full {
        "summary:au_prod_100_koz"
    } else {
        "summary:au_prod_attr_koz"
    };
    let mut source_claims = Vec::new();
    let mut history = Vec::new();
    for serial in start..start + count {
        let release = quarter(serial)?;
        let p = select_cell(cells, subject, &release, &release, prod)?;
        let s = select_cell(
            cells,
            subject,
            &release,
            &release,
            "summary:au_sold_attr_koz",
        )?;
        let r = select_cell(
            cells,
            subject,
            &release,
            &release,
            "summary:aisc_usd_per_oz",
        )?;
        history.push(HistoricalContinuation {
            id: release,
            production: actual_claim(p, QuantityKind::ProductionKoz)?,
            sold: actual_claim(s, QuantityKind::SoldKoz)?,
            aisc: actual_claim(r, QuantityKind::AiscUsdPerSoldOz)?,
        });
        source_claims.extend([p.clone(), s.clone(), r.clone()]);
    }
    Ok(HistorySourceWindow {
        id: format!("{}..{}", quarter(start)?, quarter(start + count - 1)?),
        source_claims,
        quarters: history,
    })
}

/// Forms all contiguous prior-year windows of the required duration from verified source Claims.
/// A later comparative cannot fill a missing original release. Every omitted window names its reason.
pub fn compare(inputs: &CompletionInputs, cells: &[KeyedCell]) -> Result<HistoricalEnvelope> {
    // Validate the current assumptions even when the inventory contains no historical quarter.
    let empty = quarters::compare_quarters(inputs, &[])?;
    let subject = &inputs.production_guidance.value.subject;
    let year = inputs.production_guidance.value.period.year;
    let count = u32::from(13 - inputs.remaining_sold.value.period.first_month) / 3;
    let periods: BTreeSet<_> = cells
        .iter()
        .filter_map(|c| {
            if c.key.subject != *subject
                || c.key.valid_period.year >= year
                || !matches!(c.key.scope, Scope::Site)
            {
                return None;
            }
            if let crate::key::PeriodKind::Quarter(q) = c.key.valid_period.kind {
                (1..=4)
                    .contains(&q)
                    .then_some(u32::from(c.key.valid_period.year) * 4 + u32::from(q - 1))
            } else {
                None
            }
        })
        .collect();
    let mut out = HistoricalEnvelope {
        comparison: empty,
        sources: Vec::new(),
        exclusions: Vec::new(),
    };
    let (Some(first), Some(last)) = (periods.first().copied(), periods.last().copied()) else {
        out.exclusions.push(HistoryExclusion { id: "historical-quarter-inventory".into(), code: "FMINE-REF-201".into(), reason: "No prior-year site quarters are available for an original-vintage historical window".into() });
        return Ok(out);
    };
    if count == 0 || last + 1 < first + count {
        out.exclusions.push(HistoryExclusion {
            id: "historical-quarter-inventory".into(),
            code: "FMINE-REF-201".into(),
            reason:
                "Available prior-year site quarters do not span the required remaining duration"
                    .into(),
        });
        return Ok(out);
    }
    let mut histories = Vec::new();
    for start in first..=last + 1 - count {
        let id = format!("{}..{}", quarter(start)?, quarter(start + count - 1)?);
        let candidate = window(inputs, cells, start, count).and_then(|source| {
            let history = quarters::QuarterlyHistory {
                id: id.clone(),
                quarters: source.quarters.clone(),
            };
            quarters::aggregate_quarters(inputs, &history)?;
            Ok((source, history))
        });
        match candidate {
            Ok((source, history)) => {
                out.sources.push(source);
                histories.push(history);
            }
            Err(e) => out.exclusions.push(HistoryExclusion {
                id,
                code: e.code.into(),
                reason: e.what,
            }),
        }
    }
    out.comparison = quarters::compare_quarters(inputs, &histories)?;
    if out.comparison.verdict != HistoryVerdict::WithinEnvelope
        && out.exclusions.iter().any(|e| e.code == "FMINE-REF-207")
    {
        out.comparison.verdict = HistoryVerdict::Indeterminate;
    }
    Ok(out)
}

pub(super) fn tier(report: &quarters::QuarterlyHistoryReport) -> &'static str {
    match report.tier {
        kernel::EnvelopeTier::T1 => "T1",
        kernel::EnvelopeTier::T3 => "T3",
    }
}

pub(super) fn verdict(report: &quarters::QuarterlyHistoryReport) -> &'static str {
    match (report.tier, report.verdict) {
        (kernel::EnvelopeTier::T3, _) => "necessary_arithmetic_condition_only",
        (_, HistoryVerdict::WithinEnvelope) => "within_supplied_history_under_sales_scenario",
        (_, HistoryVerdict::ExceedsEnvelope) => "outside_supplied_history_under_sales_scenario",
        (_, HistoryVerdict::Indeterminate) => "indeterminate",
    }
}

fn rational(value: Rat) -> String {
    js(&format!("{}/{}", value.num(), value.den()))
}

fn interval(value: Interval) -> String {
    format!(
        "{{\"lo\":{},\"hi\":{},\"closed\":true}}",
        rational(value.lo),
        rational(value.hi)
    )
}

fn period(p: Period) -> String {
    format!(
        "{{\"year\":{},\"first_month\":{},\"last_month\":{}}}",
        p.year, p.first_month, p.last_month
    )
}

fn witness(c: &Claim<quarters::QuarterlyWitness>) -> String {
    let p = &c.value.completion;
    let qs = c.value.quarters.iter().map(|q| format!("{{\"source_period\":{},\"replay_period\":{},\"production_koz\":{},\"sold_koz\":{},\"aisc_usd_per_oz\":{},\"spend_usd_m\":{}}}", period(q.source_period), period(q.replay_period), rational(q.production_koz), rational(q.sold_koz), rational(q.aisc_usd_per_oz), rational(q.spend_usd_m))).collect::<Vec<_>>().join(",");
    format!(
        "{{{},\"independently_checked\":true,\"completion\":{{\"ytd_production_koz\":{},\"remaining_production_koz\":{},\"ytd_sold_koz\":{},\"remaining_sold_koz\":{},\"ytd_aisc_usd_per_oz\":{},\"remaining_aisc_usd_per_oz\":{},\"annual_production_koz\":{},\"annual_aisc_usd_per_oz\":{},\"remaining_spend_usd_m\":{}}},\"quarters\":[{}]}}",
        claim_fields(c),
        rational(p.ytd_production_koz),
        rational(p.remaining_production_koz),
        rational(p.ytd_sold_koz),
        rational(p.remaining_sold_koz),
        rational(p.ytd_aisc_usd_per_oz),
        rational(p.remaining_aisc_usd_per_oz),
        rational(p.annual_production_koz),
        rational(p.annual_aisc_usd_per_oz),
        rational(p.remaining_spend_usd_m),
        qs
    )
}

fn certificate(c: &quarters::CheckedCertificate) -> String {
    let proof = match &c.certificate {
        Certificate::Farkas { multipliers } => format!(
            "\"kind\":\"farkas\",\"multipliers\":[{}]",
            multipliers
                .iter()
                .map(|(id, n)| format!(
                    "{{\"identity\":{},\"multiplier\":{}}}",
                    js(id),
                    rational(*n)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Certificate::Residual { identity, residual } => format!(
            "\"kind\":\"residual\",\"identity\":{},\"residual\":{}",
            js(identity),
            rational(*residual)
        ),
    };
    format!(
        "{{\"component\":{},\"digest\":{},\"independently_checked\":true,{proof}}}",
        js(&c.component),
        js(&c.digest)
    )
}

pub(super) fn to_json(e: &HistoricalEnvelope) -> String {
    let checks = e.comparison.checks.iter().map(|c| {
        let t = &c.totals.value;
        format!("{{\"id\":{},\"same_season\":{},\"totals\":{{{},\"marginal_projections_only\":true,\"first_period\":{},\"last_period\":{},\"production_koz\":{},\"sold_koz\":{},\"aisc_usd_per_oz\":{},\"aisc_spend_usd_m\":{}}},\"production_possible\":{},\"cost_possible\":{},\"witness\":{},\"certificates\":[{}],\"indeterminate\":[{}]}}", js(&c.id), c.same_season, claim_fields(&c.totals), period(t.first_period), period(t.last_period), interval(t.production_koz), interval(t.sold_koz), interval(t.aisc_usd_per_oz), interval(t.aisc_spend_usd_m), c.production_possible.map_or("null".into(), |v| v.to_string()), c.cost_possible.map_or("null".into(), |v| v.to_string()), c.witness.as_ref().map_or("null".into(), witness), c.certificates.iter().map(certificate).collect::<Vec<_>>().join(","), c.indeterminate.iter().map(|s| js(s)).collect::<Vec<_>>().join(","))
    }).collect::<Vec<_>>().join(",");
    let sources = e
        .sources
        .iter()
        .map(|s| {
            format!(
                "{{\"id\":{},\"source_claims\":[{}],\"quarters\":[{}]}}",
                js(&s.id),
                s.source_claims
                    .iter()
                    .map(KeyedCell::to_json)
                    .collect::<Vec<_>>()
                    .join(","),
                s.quarters
                    .iter()
                    .map(|q| format!(
                        "{{\"id\":{},\"production\":{},\"sold\":{},\"aisc\":{}}}",
                        js(&q.id),
                        quantity_json(&q.production),
                        quantity_json(&q.sold),
                        quantity_json(&q.aisc)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let exclusions = e
        .exclusions
        .iter()
        .map(|x| {
            format!(
                "{{\"id\":{},\"code\":{},\"reason\":{}}}",
                js(&x.id),
                js(&x.code),
                js(&x.reason)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"population\":\"All complete, usable contiguous original-vintage quarter windows preceding the guidance year, with the same duration as the remaining period; cross-year windows included.\",\"condition\":\"Replay each window jointly under the separately declared remaining-sales interval. Any exclusion applies only to these supplied windows under that assumption, never to mine capacity.\",\"seasonality\":\"same_season is calendar context only; it does not select or predict a continuation\",\"tier\":{},\"verdict\":{},\"checks\":[{checks}],\"sources\":[{sources}],\"exclusions\":[{exclusions}]}}",
        js(tier(&e.comparison)),
        js(verdict(&e.comparison))
    )
}
