//! Joint replay of consecutive historical quarters (BD-342).
//! Determinism: Exact
//!
//! DESIGN: each source quarter keeps its original support. Its sales/numerator wedge is exact:
//! `rate_lo * sold <= 1000 * spend <= rate_hi * sold`. The existing exact Sum/Ratio solver combines
//! those wedges, the explicit remaining-sales scenario and both annual targets. Aggregate rate/sales
//! intervals are display projections only; they never replace the quarter constraints in a verdict.

use std::collections::BTreeMap;

use fmine_core::hash::sha256;
use fmine_numerics::exact_feas::{self as xf, Certificate, Convention, Outcome};

use super::*;

/// One proposed replay of consecutive original historical quarters, possibly crossing a calendar year.
#[derive(Debug, Clone, PartialEq)]
pub struct QuarterlyHistory {
    pub id: String,
    pub quarters: Vec<HistoricalContinuation>,
}

/// Marginal projections of one window. Source support is explicit even for a cross-year window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowTotals {
    pub first_period: Period,
    pub last_period: Period,
    pub production_koz: Interval,
    pub sold_koz: Interval,
    pub aisc_usd_per_oz: Interval,
    pub aisc_spend_usd_m: Interval,
}

/// One point inside a source quarter's rounding boxes, with its hypothetical replay support kept separate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarterWitness {
    pub source_period: Period,
    pub replay_period: Period,
    pub production_koz: Rat,
    pub sold_koz: Rat,
    pub aisc_usd_per_oz: Rat,
    pub spend_usd_m: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarterlyWitness {
    pub completion: CompletionWitness,
    pub quarters: Vec<QuarterWitness>,
}

/// A checked exclusion for one component of the explicitly supplied finite window model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCertificate {
    pub component: String,
    pub certificate: Certificate,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuarterlyCheck {
    pub id: String,
    pub same_season: bool,
    pub totals: Claim<WindowTotals>,
    pub production_possible: Option<bool>,
    pub cost_possible: Option<bool>,
    pub witness: Option<Claim<QuarterlyWitness>>,
    pub certificates: Vec<CheckedCertificate>,
    pub indeterminate: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuarterlyHistoryReport {
    pub tier: EnvelopeTier,
    pub verdict: HistoryVerdict,
    pub checks: Vec<QuarterlyCheck>,
}

fn validate<'a>(
    i: &CompletionInputs,
    h: &'a QuarterlyHistory,
) -> Result<Vec<&'a HistoricalContinuation>> {
    let remaining = validate_inputs(i)?;
    if h.id.trim().is_empty() || h.quarters.is_empty() {
        return refuse(
            RefusalCode::NoClaimInDomain201,
            "historical window has no id or quarters",
        );
    }
    if remaining.first_month % 3 != 1
        || h.quarters.len() * 3 != usize::from(13 - remaining.first_month)
    {
        return refuse(
            RefusalCode::SupportMismatch102,
            "historical window must have exactly the remaining number of calendar quarters",
        );
    }
    let mut quarters: Vec<_> = h.quarters.iter().collect();
    quarters.sort_by_key(|q| {
        (
            q.production.value.period.year,
            q.production.value.period.first_month,
        )
    });
    let mut next_month = None;
    for q in &quarters {
        for (claim, role, kind) in [
            (
                &q.production,
                "historical quarter production",
                QuantityKind::ProductionKoz,
            ),
            (&q.sold, "historical quarter sold", QuantityKind::SoldKoz),
            (
                &q.aisc,
                "historical quarter AISC",
                QuantityKind::AiscUsdPerSoldOz,
            ),
        ] {
            validate_claim(claim, role, kind)?;
            same_basis(&i.production_guidance.value, &claim.value, role)?;
        }
        let p = q.production.value.period;
        let serial = u32::from(p.year) * 12 + u32::from(p.first_month);
        if p != q.sold.value.period
            || p != q.aisc.value.period
            || p.first_month % 3 != 1
            || p.last_month != p.first_month + 2
            || next_month.is_some_and(|next| serial != next)
        {
            return refuse(
                RefusalCode::SupportMismatch102,
                format!(
                    "{}: quarters must be consecutive, with production, sales and AISC on the same support",
                    h.id
                ),
            );
        }
        if p.year >= remaining.year {
            return refuse(
                RefusalCode::AssayLookahead108,
                "every historical quarter must precede the guidance year",
            );
        }
        if q.aisc.value.definition != i.aisc_guidance.value.definition {
            return refuse(
                RefusalCode::BasisMismatch105,
                "historical quarter and guidance AISC definitions differ",
            );
        }
        next_month = Some(serial + 3);
    }
    Ok(quarters)
}

fn sum_ranges(ranges: impl IntoIterator<Item = Interval>) -> Exact<Interval> {
    let mut total = point(ZERO);
    for r in ranges {
        total.lo = total.lo.checked_add(r.lo)?;
        total.hi = total.hi.checked_add(r.hi)?;
    }
    Ok(total)
}

fn spend(s: Interval, r: Interval) -> Exact<Interval> {
    Ok(closed(
        s.lo.checked_mul(r.lo)?.checked_div(THOUSAND)?,
        s.hi.checked_mul(r.hi)?.checked_div(THOUSAND)?,
    ))
}

fn source_parents<'a>(
    i: &'a CompletionInputs,
    quarters: &[&'a HistoricalContinuation],
) -> Vec<&'a Claim<Quantity>> {
    let mut parents = vec![
        &i.production_guidance,
        &i.aisc_guidance,
        &i.ytd_production,
        &i.ytd_sold,
        &i.ytd_aisc,
        &i.remaining_sold,
    ];
    for q in quarters {
        parents.extend([&q.production, &q.sold, &q.aisc]);
    }
    parents
}

fn totals(quarters: &[&HistoricalContinuation]) -> Result<Claim<WindowTotals>> {
    let first = quarters
        .first()
        .ok_or_else(|| Refusal::new(RefusalCode::NoClaimInDomain201, "no historical quarters"))?;
    let last = quarters
        .last()
        .ok_or_else(|| Refusal::new(RefusalCode::NoClaimInDomain201, "no historical quarters"))?;
    let mut rates = Vec::new();
    // A positive sold-weighted ratio reaches an extremum at sales endpoints. Rates are monotone,
    // so all lower (respectively upper) rate endpoints suffice. At most three quarters: <=16 points.
    for mask in 0..(1_usize << quarters.len()) {
        for upper in [false, true] {
            let (mut sold, mut numerator) = (ZERO, ZERO);
            for (j, q) in quarters.iter().enumerate() {
                let s = if mask & (1 << j) == 0 {
                    q.sold.value.interval.lo
                } else {
                    q.sold.value.interval.hi
                };
                let r = if upper {
                    q.aisc.value.interval.hi
                } else {
                    q.aisc.value.interval.lo
                };
                sold = exact(sold.checked_add(s))?;
                numerator = exact(numerator.checked_add(exact(s.checked_mul(r))?))?;
            }
            rates.push(exact(numerator.checked_div(sold))?);
        }
    }
    let spend_ranges = quarters
        .iter()
        .map(|q| spend(q.sold.value.interval, q.aisc.value.interval))
        .collect::<Exact<Vec<_>>>()
        .map_err(|e| Refusal::new(RefusalCode::NumericalCertificateFailed207, e.to_string()))?;
    let value = WindowTotals {
        first_period: first.production.value.period,
        last_period: last.production.value.period,
        production_koz: exact(sum_ranges(
            quarters.iter().map(|q| q.production.value.interval),
        ))?,
        sold_koz: exact(sum_ranges(quarters.iter().map(|q| q.sold.value.interval)))?,
        aisc_usd_per_oz: exact(hull(rates))?,
        aisc_spend_usd_m: exact(sum_ranges(spend_ranges))?,
    };
    let mut enc = Encoder::new_frame(*b"GQWT", 1);
    for p in [value.first_period, value.last_period] {
        enc.put_u16(p.year);
        enc.put_u8(p.first_month);
        enc.put_u8(p.last_month);
    }
    for r in [
        value.production_koz,
        value.sold_koz,
        value.aisc_usd_per_oz,
        value.aisc_spend_usd_m,
    ] {
        put_rat(&mut enc, r.lo);
        put_rat(&mut enc, r.hi);
    }
    let parents: Vec<_> = quarters
        .iter()
        .flat_map(|q| [&q.production, &q.sold, &q.aisc])
        .collect();
    let tag = blake3_hash(&enc.finish()).to_hex();
    Ok(derived_value(
        "guidance.historical_window_totals/1",
        &parents,
        value,
        &tag,
        AdditivityClass::NonAdditive,
    ))
}

/// Validates the source window and returns its exact marginal projections; these are not a joint feasibility test.
pub fn aggregate_quarters(
    i: &CompletionInputs,
    h: &QuarterlyHistory,
) -> Result<Claim<WindowTotals>> {
    totals(&validate(i, h)?)
}

struct Model {
    cells: Vec<xf::Cell>,
    identities: Vec<xf::Identity>,
    rules: xf::Rules,
    binding: String,
}

impl Model {
    fn variable(&mut self, name: &str, range: Interval) -> Result<()> {
        self.cells.push(xf::Cell {
            locator: name.into(),
            sha256: self.binding.clone(),
            token: format!(
                "derived closed interval [{}/{},{}/{}]",
                range.lo.num(),
                range.lo.den(),
                range.hi.num(),
                range.hi.den()
            ),
            printed: exact(range.lo.checked_add(exact(
                exact(range.hi.checked_sub(range.lo))?.checked_div(Rat::from_integer(2)),
            )?))?,
            step: exact(range.hi.checked_sub(range.lo))?,
            scope: xf::Scope::Visible,
        });
        Ok(())
    }
    fn identity(&mut self, id: &str, kind: xf::IdentityKind) {
        self.identities.push(xf::Identity {
            id: id.into(),
            family: "guidance-quarter-replay".into(),
            closure: xf::Closure::Definitional,
            kind,
        });
    }
    fn sum(&mut self, id: &str, total: &str, lines: Vec<String>) {
        let signs = vec![1; lines.len()];
        self.identity(
            id,
            xf::IdentityKind::Sum {
                total: total.into(),
                lines,
                signs,
            },
        );
    }
    fn ratio(&mut self, id: &str, rate: &str, num: &str, den: &str) {
        self.identity(
            id,
            xf::IdentityKind::Ratio {
                rate: rate.into(),
                num: num.into(),
                den: den.into(),
                factor: THOUSAND,
            },
        );
    }
}

fn model(
    i: &CompletionInputs,
    quarters: &[&HistoricalContinuation],
    total: &WindowTotals,
) -> Result<Model> {
    let mut bindings: Vec<_> = source_parents(i, quarters)
        .into_iter()
        .map(|c| format!("{}:{}", c.claim_id, c.value.canonical_tag()))
        .collect();
    bindings.sort();
    // Internal solver cells describe claim-bound variables, not invented workbook cells. This SHA-256
    // binds the exact source Claim ids/value tags used to construct their bounds and identities.
    let mut m = Model {
        cells: Vec::new(),
        identities: Vec::new(),
        rules: xf::Rules::new(Convention::Closed)
            .with("guidance-quarter-replay", xf::Construction::RoundedOnce),
        binding: sha256(bindings.join("\n").as_bytes()).to_hex(),
    };
    let mut productions = vec!["ytd.production".into()];
    let mut sales = Vec::new();
    let mut numerators = vec!["ytd.spend".into()];
    for (prefix, p, s, r) in std::iter::once((
        "ytd".to_string(),
        &i.ytd_production,
        &i.ytd_sold,
        &i.ytd_aisc,
    ))
    .chain(
        quarters
            .iter()
            .enumerate()
            .map(|(n, q)| (format!("q{n}"), &q.production, &q.sold, &q.aisc)),
    ) {
        for (suffix, range) in [
            ("production", p.value.interval),
            ("sold", s.value.interval),
            ("rate", r.value.interval),
            ("spend", exact(spend(s.value.interval, r.value.interval))?),
        ] {
            m.variable(&format!("{prefix}.{suffix}"), range)?;
        }
        m.ratio(
            &format!("cost.{prefix}"),
            &format!("{prefix}.rate"),
            &format!("{prefix}.spend"),
            &format!("{prefix}.sold"),
        );
        if prefix != "ytd" {
            productions.push(format!("{prefix}.production"));
            sales.push(format!("{prefix}.sold"));
            numerators.push(format!("{prefix}.spend"));
        }
    }
    m.variable("annual.production", i.production_guidance.value.interval)?;
    m.sum("production.annual", "annual.production", productions);
    m.variable("remaining.sold", i.remaining_sold.value.interval)?;
    m.sum("sales.remaining", "remaining.sold", sales);
    m.variable(
        "annual.sold",
        exact(sum_ranges([
            i.ytd_sold.value.interval,
            i.remaining_sold.value.interval,
        ]))?,
    )?;
    m.sum(
        "sales.annual",
        "annual.sold",
        vec!["ytd.sold".into(), "remaining.sold".into()],
    );
    m.variable(
        "annual.spend",
        exact(sum_ranges([
            exact(spend(i.ytd_sold.value.interval, i.ytd_aisc.value.interval))?,
            total.aisc_spend_usd_m,
        ]))?,
    )?;
    m.sum("spend.annual", "annual.spend", numerators);
    m.variable("annual.rate", i.aisc_guidance.value.interval)?;
    m.ratio("cost.annual", "annual.rate", "annual.spend", "annual.sold");
    Ok(m)
}

fn certificate_error(e: impl std::fmt::Display) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        format!("historical quarter certificate: {e}"),
    )
}

fn witness_from(
    i: &CompletionInputs,
    quarters: &[&HistoricalContinuation],
    values: &BTreeMap<String, Rat>,
) -> Result<QuarterlyWitness> {
    let get = |name: &str| {
        values
            .get(name)
            .copied()
            .ok_or_else(|| certificate_error(format!("witness has no {name}")))
    };
    let mut points = Vec::new();
    let (mut production, mut spend) = (ZERO, ZERO);
    for ((n, q), first_month) in quarters
        .iter()
        .enumerate()
        .zip((i.remaining_sold.value.period.first_month..=12).step_by(3))
    {
        let p = QuarterWitness {
            source_period: q.production.value.period,
            replay_period: Period {
                year: i.remaining_sold.value.period.year,
                first_month,
                last_month: first_month + 2,
            },
            production_koz: get(&format!("q{n}.production"))?,
            sold_koz: get(&format!("q{n}.sold"))?,
            aisc_usd_per_oz: get(&format!("q{n}.rate"))?,
            spend_usd_m: get(&format!("q{n}.spend"))?,
        };
        production = exact(production.checked_add(p.production_koz))?;
        spend = exact(spend.checked_add(p.spend_usd_m))?;
        points.push(p);
    }
    let sold = get("remaining.sold")?;
    Ok(QuarterlyWitness {
        quarters: points,
        completion: CompletionWitness {
            ytd_production_koz: get("ytd.production")?,
            remaining_production_koz: production,
            ytd_sold_koz: get("ytd.sold")?,
            remaining_sold_koz: sold,
            ytd_aisc_usd_per_oz: get("ytd.rate")?,
            remaining_aisc_usd_per_oz: exact(
                exact(spend.checked_mul(THOUSAND))?.checked_div(sold),
            )?,
            annual_production_koz: get("annual.production")?,
            annual_aisc_usd_per_oz: get("annual.rate")?,
            remaining_spend_usd_m: spend,
        },
    })
}

/// Checks every original quarter box and equation, the replay mapping, the sales assumption and both targets.
/// It does not use the solver or the independently projected aggregate rate/sales intervals.
pub fn check_quarter_witness(
    i: &CompletionInputs,
    h: &QuarterlyHistory,
    w: &QuarterlyWitness,
) -> Result<bool> {
    let quarters = validate(i, h)?;
    if w.quarters.len() != quarters.len() {
        return Ok(false);
    }
    exact((|| {
        let (mut p, mut s, mut n) = (ZERO, ZERO, ZERO);
        for ((q, point), first_month) in quarters
            .iter()
            .zip(&w.quarters)
            .zip((i.remaining_sold.value.period.first_month..=12).step_by(3))
        {
            let replay = Period {
                year: i.remaining_sold.value.period.year,
                first_month,
                last_month: first_month + 2,
            };
            if point.source_period != q.production.value.period
                || point.replay_period != replay
                || !q.production.value.interval.contains(point.production_koz)?
                || !q.sold.value.interval.contains(point.sold_koz)?
                || !q.aisc.value.interval.contains(point.aisc_usd_per_oz)?
                || point.spend_usd_m.checked_mul(THOUSAND)?
                    != point.sold_koz.checked_mul(point.aisc_usd_per_oz)?
            {
                return Ok(false);
            }
            p = p.checked_add(point.production_koz)?;
            s = s.checked_add(point.sold_koz)?;
            n = n.checked_add(point.spend_usd_m)?;
        }
        let c = &w.completion;
        for (range, value) in [
            (i.ytd_production.value.interval, c.ytd_production_koz),
            (i.ytd_sold.value.interval, c.ytd_sold_koz),
            (i.ytd_aisc.value.interval, c.ytd_aisc_usd_per_oz),
            (i.remaining_sold.value.interval, c.remaining_sold_koz),
            (
                i.production_guidance.value.interval,
                c.annual_production_koz,
            ),
            (i.aisc_guidance.value.interval, c.annual_aisc_usd_per_oz),
        ] {
            if !range.contains(value)? {
                return Ok(false);
            }
        }
        Ok(c.remaining_production_koz == p
            && c.remaining_sold_koz == s
            && c.remaining_spend_usd_m == n
            && c.ytd_production_koz.checked_add(p)? == c.annual_production_koz
            && c.remaining_aisc_usd_per_oz.checked_mul(s)? == n.checked_mul(THOUSAND)?
            && c.annual_aisc_usd_per_oz
                .checked_mul(c.ytd_sold_koz.checked_add(s)?)?
                == c.ytd_aisc_usd_per_oz
                    .checked_mul(c.ytd_sold_koz)?
                    .checked_add(n.checked_mul(THOUSAND)?)?)
    })())
}

/// Joint T1 comparison with a finite union of source-quarter windows, conditional on `remaining_sold`.
/// Missing history is T3; a numerical indeterminacy never becomes an exclusion.
pub fn compare_quarters(
    i: &CompletionInputs,
    histories: &[QuarterlyHistory],
) -> Result<QuarterlyHistoryReport> {
    compare_with_caps(i, histories, xf::MAX_VARIABLES, xf::MAX_PIVOTS)
}

fn compare_with_caps(
    i: &CompletionInputs,
    histories: &[QuarterlyHistory],
    max_variables: usize,
    max_pivots: usize,
) -> Result<QuarterlyHistoryReport> {
    validate_inputs(i)?;
    let mut ordered: Vec<_> = histories.iter().collect();
    ordered.sort_by(|a, b| a.id.cmp(&b.id));
    if ordered.windows(2).any(|w| w[0].id == w[1].id) {
        return refuse(
            RefusalCode::BasisMismatch105,
            "historical window ids must be unique",
        );
    }
    let mut checks = Vec::new();
    for h in ordered {
        let quarters = validate(i, h)?;
        let total = totals(&quarters)?;
        let m = model(i, &quarters, &total.value)?;
        let mut check = QuarterlyCheck {
            id: h.id.clone(),
            same_season: total.value.first_period.first_month
                == i.remaining_sold.value.period.first_month
                && total.value.last_period.last_month == 12,
            totals: total,
            production_possible: None,
            cost_possible: None,
            witness: None,
            certificates: Vec::new(),
            indeterminate: Vec::new(),
        };
        let mut values = BTreeMap::new();
        for component in
            xf::solve_with_caps(&m.cells, &m.identities, &m.rules, max_variables, max_pivots)
                .map_err(certificate_error)?
        {
            let production = component
                .identities
                .iter()
                .any(|id| id == "production.annual");
            let name = if production {
                "production"
            } else {
                "cost_and_sales"
            };
            let possible = match component.outcome {
                Outcome::Feasible { witness } => {
                    values.extend(witness);
                    Some(true)
                }
                Outcome::Infeasible { certificate } => {
                    xf::check_certificate(&m.cells, &m.identities, &m.rules, &certificate)
                        .map_err(|e| certificate_error(e.0))?;
                    let digest =
                        xf::certificate_digest(&m.cells, &m.identities, &m.rules, &certificate)
                            .map_err(certificate_error)?;
                    check.certificates.push(CheckedCertificate {
                        component: name.into(),
                        certificate,
                        digest,
                    });
                    Some(false)
                }
                Outcome::Indeterminate { reason } => {
                    check.indeterminate.push(format!("{name}: {reason}"));
                    None
                }
                Outcome::NotRefuted => {
                    check
                        .indeterminate
                        .push(format!("{name}: relaxation supplied no exact decision"));
                    None
                }
            };
            if production {
                check.production_possible = possible;
            } else {
                check.cost_possible = possible;
            }
        }
        if check.production_possible == Some(true) && check.cost_possible == Some(true) {
            xf::check_witness(&m.cells, &m.identities, &m.rules, &values)
                .map_err(|e| certificate_error(e.0))?;
            let witness = witness_from(i, &quarters, &values)?;
            if !check_quarter_witness(i, h, &witness)? {
                return Err(certificate_error("original-quarter witness check failed"));
            }
            let mut enc = Encoder::new_frame(*b"GQWP", 1);
            enc.put_str(&witness.completion.canonical_tag(&h.id));
            for q in &witness.quarters {
                for period in [q.source_period, q.replay_period] {
                    enc.put_u16(period.year);
                    enc.put_u8(period.first_month);
                    enc.put_u8(period.last_month);
                }
                for v in [
                    q.production_koz,
                    q.sold_koz,
                    q.aisc_usd_per_oz,
                    q.spend_usd_m,
                ] {
                    put_rat(&mut enc, v);
                }
            }
            let tag = blake3_hash(&enc.finish()).to_hex();
            check.witness = Some(derived_value(
                "guidance.quarterly_completion_witness/1",
                &source_parents(i, &quarters),
                witness,
                &tag,
                AdditivityClass::NonAdditive,
            ));
        }
        checks.push(check);
    }
    let verdict = if checks.iter().any(|c| c.witness.is_some()) {
        HistoryVerdict::WithinEnvelope
    } else if checks.is_empty()
        || checks
            .iter()
            .any(|c| c.production_possible.is_none() || c.cost_possible.is_none())
    {
        HistoryVerdict::Indeterminate
    } else {
        HistoryVerdict::ExceedsEnvelope
    };
    Ok(QuarterlyHistoryReport {
        tier: if checks.is_empty() {
            EnvelopeTier::T3
        } else {
            EnvelopeTier::T1
        },
        verdict,
        checks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::KuKind;

    fn n(value: i128) -> Rat {
        Rat::from_integer(value)
    }

    fn p(year: u16, first_month: u8, last_month: u8) -> Period {
        Period {
            year,
            first_month,
            last_month,
        }
    }

    fn claim(kind: QuantityKind, period: Period, lo: i128, hi: i128) -> Claim<Quantity> {
        let value = Quantity {
            subject: "Invented quarterly test mine".into(),
            period,
            ownership: Ownership::Full,
            kind,
            definition: if kind == QuantityKind::AiscUsdPerSoldOz {
                "invented/sold".into()
            } else {
                String::new()
            },
            interval: closed(n(lo), n(hi)),
        };
        let tag = value.canonical_tag();
        Claim::new(
            format!("invented.{kind:?}"),
            value,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            kind.additivity(),
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            ConfidentialityTier::Public,
            vec![],
            vec![],
            vec![SourceAnchor::new(
                "synthetic/quarterly-guidance",
                [7; 32],
                &tag,
            )],
        )
    }

    fn inputs() -> CompletionInputs {
        CompletionInputs {
            production_guidance: claim(QuantityKind::ProductionKoz, p(2026, 1, 12), 200, 200),
            aisc_guidance: claim(QuantityKind::AiscUsdPerSoldOz, p(2026, 1, 12), 100, 100),
            ytd_production: claim(QuantityKind::ProductionKoz, p(2026, 1, 6), 50, 50),
            ytd_sold: claim(QuantityKind::SoldKoz, p(2026, 1, 6), 100, 100),
            ytd_aisc: claim(QuantityKind::AiscUsdPerSoldOz, p(2026, 1, 6), 100, 100),
            remaining_sold: claim(QuantityKind::SoldKoz, p(2026, 7, 12), 100, 100),
        }
    }

    fn quarter(
        year: u16,
        number: u8,
        production: i128,
        sold: i128,
        rate: i128,
    ) -> HistoricalContinuation {
        let period = p(year, 3 * number - 2, 3 * number);
        HistoricalContinuation {
            id: format!("{year}Q{number}"),
            production: claim(QuantityKind::ProductionKoz, period, production, production),
            sold: claim(QuantityKind::SoldKoz, period, sold, sold),
            aisc: claim(QuantityKind::AiscUsdPerSoldOz, period, rate, rate),
        }
    }

    fn history() -> QuarterlyHistory {
        QuarterlyHistory {
            id: "2024Q3..2024Q4".into(),
            quarters: vec![quarter(2024, 3, 75, 50, 80), quarter(2024, 4, 75, 50, 120)],
        }
    }

    #[test]
    fn quarter_witness_reaches_both_targets_and_checks_every_original_source() {
        let (i, h) = (inputs(), history());
        let r = compare_quarters(&i, std::slice::from_ref(&h)).unwrap();
        assert_eq!(
            (r.tier, r.verdict),
            (EnvelopeTier::T1, HistoryVerdict::WithinEnvelope)
        );
        let c = &r.checks[0];
        assert!(c.same_season);
        assert_eq!(
            (c.production_possible, c.cost_possible),
            (Some(true), Some(true))
        );
        assert_eq!(c.totals.value.aisc_usd_per_oz, point(n(100)));
        assert_eq!(c.totals.value.aisc_spend_usd_m, point(n(10)));
        let w = c.witness.as_ref().unwrap();
        assert_eq!(w.evidence, EvidenceClass::Assumed);
        assert_eq!(w.parents.len(), 12);
        assert_eq!(w.anchors.len(), 12);
        assert_eq!(w.value.quarters[0].spend_usd_m, n(4));
        assert_eq!(w.value.quarters[1].spend_usd_m, n(6));
        assert_eq!(w.value.quarters[0].source_period, p(2024, 7, 9));
        assert_eq!(w.value.quarters[0].replay_period, p(2026, 7, 9));
        assert_eq!(w.value.completion.annual_production_koz, n(200));
        assert_eq!(w.value.completion.annual_aisc_usd_per_oz, n(100));
        assert!(check_quarter_witness(&i, &h, &w.value).unwrap());
        let mut corrupt = w.value.clone();
        corrupt.quarters[0].spend_usd_m = n(5);
        assert!(!check_quarter_witness(&i, &h, &corrupt).unwrap());
        let mut corrupt = w.value.clone();
        corrupt.quarters[0].source_period.year = 2025;
        assert!(!check_quarter_witness(&i, &h, &corrupt).unwrap());
        let mut corrupt = w.value.clone();
        corrupt.quarters[0].replay_period.first_month = 10;
        assert!(!check_quarter_witness(&i, &h, &corrupt).unwrap());
    }

    #[test]
    fn aggregate_sales_rate_boxes_cannot_manufacture_a_quarterly_witness() {
        let mut i = inputs();
        i.ytd_sold.value.interval = point(n(1));
        i.ytd_aisc.value.interval = point(n(55));
        i.remaining_sold.value.interval = point(n(2));
        i.aisc_guidance.value.interval = point(n(45));
        let mut h = history();
        for (q, rate) in h.quarters.iter_mut().zip([10, 100]) {
            q.sold.value.interval = closed(n(1), n(2));
            q.aisc.value.interval = point(n(rate));
        }
        let total = aggregate_quarters(&i, &h).unwrap();
        assert_eq!(total.value.sold_koz, closed(n(2), n(4)));
        assert_eq!(total.value.aisc_usd_per_oz, closed(n(40), n(70)));
        // An independently supplied aggregate box admits sales=2/rate=40. But source sales=2 forces
        // each quarter's sales to 1 and the true remaining rate to 55. The quarter model retains that link.
        let box_only = HistoricalContinuation {
            id: "independent-marginal-boxes".into(),
            production: claim(QuantityKind::ProductionKoz, p(2024, 7, 12), 150, 150),
            sold: claim(QuantityKind::SoldKoz, p(2024, 7, 12), 2, 4),
            aisc: claim(QuantityKind::AiscUsdPerSoldOz, p(2024, 7, 12), 40, 70),
        };
        assert_eq!(
            compare_history(&i, &[box_only]).unwrap().verdict,
            HistoryVerdict::WithinEnvelope
        );
        let r = compare_quarters(&i, &[h]).unwrap();
        assert_eq!(r.verdict, HistoryVerdict::ExceedsEnvelope);
        assert_eq!(r.checks[0].production_possible, Some(true));
        assert_eq!(r.checks[0].cost_possible, Some(false));
        assert!(r.checks[0].witness.is_none());
        assert_eq!(r.checks[0].certificates.len(), 1);
        assert_eq!(r.checks[0].certificates[0].component, "cost_and_sales");
        assert!(!r.checks[0].certificates[0].digest.is_empty());
    }

    #[test]
    fn cross_year_windows_preserve_source_periods_and_order_is_irrelevant() {
        let i = inputs();
        let cross = QuarterlyHistory {
            id: "2024Q4..2025Q1".into(),
            quarters: vec![quarter(2024, 4, 75, 50, 80), quarter(2025, 1, 75, 50, 120)],
        };
        let report = compare_quarters(&i, &[history(), cross.clone()]).unwrap();
        let c = &report.checks[1];
        assert!(!c.same_season);
        assert_eq!(c.totals.value.first_period, p(2024, 10, 12));
        assert_eq!(c.totals.value.last_period, p(2025, 1, 3));
        let w = &c.witness.as_ref().unwrap().value;
        assert_eq!(w.quarters[1].source_period, p(2025, 1, 3));
        assert_eq!(w.quarters[1].replay_period, p(2026, 10, 12));
        assert!(check_quarter_witness(&i, &cross, w).unwrap());
        let mut reversed = cross;
        reversed.quarters.reverse();
        assert_eq!(
            report,
            compare_quarters(&i, &[reversed, history()]).unwrap()
        );
    }

    #[test]
    fn separate_production_and_cost_successes_do_not_make_one_joint_history() {
        let i = inputs();
        let mut costly = history();
        costly.id = "production-only".into();
        for q in &mut costly.quarters {
            q.aisc.value.interval = point(n(200));
        }
        let mut short = history();
        short.id = "cost-only".into();
        for q in &mut short.quarters {
            q.production.value.interval = point(n(50));
            q.aisc.value.interval = point(n(100));
        }
        let r = compare_quarters(&i, &[costly, short]).unwrap();
        assert_eq!(r.verdict, HistoryVerdict::ExceedsEnvelope);
        assert_eq!(
            (r.checks[0].production_possible, r.checks[0].cost_possible),
            (Some(false), Some(true))
        );
        assert_eq!(
            (r.checks[1].production_possible, r.checks[1].cost_possible),
            (Some(true), Some(false))
        );
        assert!(
            r.checks
                .iter()
                .all(|c| c.witness.is_none() && !c.certificates.is_empty())
        );
    }

    #[test]
    fn missing_history_and_solver_caps_remain_indeterminate() {
        let i = inputs();
        let absent = compare_quarters(&i, &[]).unwrap();
        assert_eq!(
            (absent.tier, absent.verdict),
            (EnvelopeTier::T3, HistoryVerdict::Indeterminate)
        );
        let capped = compare_with_caps(&i, &[history()], 0, xf::MAX_PIVOTS).unwrap();
        assert_eq!(
            (capped.tier, capped.verdict),
            (EnvelopeTier::T1, HistoryVerdict::Indeterminate)
        );
        let c = &capped.checks[0];
        assert_eq!((c.production_possible, c.cost_possible), (None, None));
        assert!(c.witness.is_none() && c.certificates.is_empty());
        assert_eq!(c.indeterminate.len(), 2);
    }

    #[test]
    fn original_quarter_support_basis_domain_and_time_are_guarded() {
        let i = inputs();
        let mut h = history();
        h.quarters[1] = quarter(2025, 2, 75, 50, 120);
        assert_eq!(
            compare_quarters(&i, &[h]).unwrap_err().code,
            "FMINE-REF-102"
        );
        let mut h = history();
        h.quarters[0].sold.value.ownership = Ownership::Attributable {
            share: Rat::new(4, 5).unwrap(),
        };
        assert_eq!(
            compare_quarters(&i, &[h]).unwrap_err().code,
            "FMINE-REF-105"
        );
        let mut h = history();
        h.quarters[0].aisc.value.definition = "different-cost-definition".into();
        assert_eq!(
            compare_quarters(&i, &[h]).unwrap_err().code,
            "FMINE-REF-105"
        );
        let mut h = history();
        h.quarters[0].sold.value.interval = point(ZERO);
        assert_eq!(
            compare_quarters(&i, &[h]).unwrap_err().code,
            "FMINE-REF-109"
        );
        let mut h = history();
        h.quarters = vec![quarter(2026, 3, 75, 50, 80), quarter(2026, 4, 75, 50, 120)];
        assert_eq!(
            compare_quarters(&i, &[h]).unwrap_err().code,
            "FMINE-REF-108"
        );
        assert_eq!(
            compare_quarters(&i, &[history(), history()])
                .unwrap_err()
                .code,
            "FMINE-REF-105"
        );
    }

    #[test]
    fn one_and_three_quarter_remainders_use_the_exact_declared_duration() {
        for (last_ytd_month, quarters) in [
            (9, vec![quarter(2024, 4, 150, 100, 100)]),
            (
                3,
                vec![
                    quarter(2024, 2, 50, 30, 100),
                    quarter(2024, 3, 50, 30, 100),
                    quarter(2024, 4, 50, 40, 100),
                ],
            ),
        ] {
            let mut i = inputs();
            for c in [&mut i.ytd_production, &mut i.ytd_sold, &mut i.ytd_aisc] {
                c.value.period.last_month = last_ytd_month;
            }
            i.remaining_sold.value.period.first_month = last_ytd_month + 1;
            let h = QuarterlyHistory {
                id: format!("remaining-after-month-{last_ytd_month}"),
                quarters,
            };
            let r = compare_quarters(&i, std::slice::from_ref(&h)).unwrap();
            assert_eq!(r.verdict, HistoryVerdict::WithinEnvelope);
            assert!(
                check_quarter_witness(&i, &h, &r.checks[0].witness.as_ref().unwrap().value)
                    .unwrap()
            );
        }
    }

    #[test]
    fn small_point_histories_match_independent_integer_inequalities() {
        let mut i = inputs();
        i.production_guidance.value.interval = closed(n(3), n(4));
        i.aisc_guidance.value.interval = closed(n(8), n(12));
        i.ytd_production.value.interval = point(n(1));
        i.ytd_sold.value.interval = point(n(1));
        i.ytd_aisc.value.interval = point(n(10));
        i.remaining_sold.value.interval = point(n(2));
        let mut seen = 0;
        for p0 in 0..=3 {
            for p1 in 0..=3 {
                for r0 in [5, 10, 15] {
                    for r1 in [5, 10, 15] {
                        let h = QuarterlyHistory {
                            id: format!("{p0}-{p1}-{r0}-{r1}"),
                            quarters: vec![
                                quarter(2024, 3, p0, 1, r0),
                                quarter(2024, 4, p1, 1, r1),
                            ],
                        };
                        let expected =
                            (3..=4).contains(&(1 + p0 + p1)) && (24..=36).contains(&(10 + r0 + r1));
                        let report = compare_quarters(&i, std::slice::from_ref(&h)).unwrap();
                        assert_eq!(
                            report.verdict == HistoryVerdict::WithinEnvelope,
                            expected,
                            "{}",
                            h.id
                        );
                        if let Some(w) = &report.checks[0].witness {
                            assert!(check_quarter_witness(&i, &h, &w.value).unwrap());
                        } else {
                            assert_eq!(report.verdict, HistoryVerdict::ExceedsEnvelope);
                            assert!(!report.checks[0].certificates.is_empty());
                        }
                        seen += 1;
                    }
                }
            }
        }
        assert_eq!(seen, 144);
    }
}
