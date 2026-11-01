//! Guidance completion requirements (BD-342): exact arithmetic conditional on a separately declared sales
//! continuation. Production is never substituted for sold ounces, and annual AISC is a ratio of summed
//! numerators to summed sales, never an average of quarterly rates.
//! Determinism: Exact
//!
//! DESIGN: target bounds are exact guidance limits; closed input intervals describe rounding or an explicitly
//! stated scenario, not probabilities. All inputs carry a subject, period, ownership basis and source anchors.
//! The positive-rate slice refuses zero/negative AISC and sold intervals touching zero. Production may be zero.
//! Computed remainders and spending requirements remain signed: a negative requirement is not clamped into a
//! plausible physical continuation. Historical checks consider each cited period's joint rounding box separately;
//! failure of that finite envelope says nothing about the mine's physical capacity.

use std::cmp::Ordering;

use fmine_claim::{AdditivityClass, Claim, ConfidentialityTier, Representation, SpatialSupport};
use fmine_core::{Encoder, blake3_hash};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_units::UnitsError;

pub mod quarters;

const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
const THOUSAND: Rat = Rat::from_integer(1000);

type Result<T> = std::result::Result<T, Refusal>;
type Exact<T> = std::result::Result<T, UnitsError>;

/// The ownership basis of every quantity. An attributable share is explicit and lies in `(0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    Full,
    Attributable { share: Rat },
}

/// Inclusive calendar-month support. No averaging or annualization across different supports is implicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub year: u16,
    pub first_month: u8,
    pub last_month: u8,
}

/// Quantity and unit together. Produced and sold ounces are distinct even though both are expressed in koz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityKind {
    ProductionKoz,
    SoldKoz,
    AiscUsdPerSoldOz,
    AiscSpendUsdMillion,
}

impl QuantityKind {
    fn additivity(self) -> AdditivityClass {
        match self {
            Self::AiscUsdPerSoldOz => AdditivityClass::IntensiveWeighted,
            _ => AdditivityClass::ExtensiveAdditive,
        }
    }
}

/// An exact closed interval on one declared support and basis, carried inside a [`Claim`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quantity {
    pub subject: String,
    pub period: Period,
    pub ownership: Ownership,
    pub kind: QuantityKind,
    /// AISC definition, including credits/denominator treatment. Required for costs and unchanged across them.
    /// Physical quantities may leave this empty; their kinds already distinguish produced and sold ounces.
    pub definition: String,
    pub interval: Interval,
}

impl Quantity {
    /// Canonical tag for [`Claim::new`], binding exact bounds and every interpretation field.
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"GCQ1", 1);
        enc.put_str(&self.subject);
        enc.put_u16(self.period.year);
        enc.put_u8(self.period.first_month);
        enc.put_u8(self.period.last_month);
        match self.ownership {
            Ownership::Full => enc.put_u8(0),
            Ownership::Attributable { share } => {
                enc.put_u8(1);
                put_rat(&mut enc, share);
            }
        }
        enc.put_u8(match self.kind {
            QuantityKind::ProductionKoz => 0,
            QuantityKind::SoldKoz => 1,
            QuantityKind::AiscUsdPerSoldOz => 2,
            QuantityKind::AiscSpendUsdMillion => 3,
        });
        enc.put_str(&self.definition);
        put_rat(&mut enc, self.interval.lo);
        put_rat(&mut enc, self.interval.hi);
        enc.put_u8(u8::from(self.interval.lo_strict));
        enc.put_u8(u8::from(self.interval.hi_strict));
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// Annual targets, current YTD actuals, and an independently declared remaining-sales scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionInputs {
    pub production_guidance: Claim<Quantity>,
    pub aisc_guidance: Claim<Quantity>,
    pub ytd_production: Claim<Quantity>,
    pub ytd_sold: Claim<Quantity>,
    pub ytd_aisc: Claim<Quantity>,
    pub remaining_sold: Claim<Quantity>,
}

/// One end of the annual AISC guidance range and its conditional remaining-spend/rate frontier.
#[derive(Debug, Clone, PartialEq)]
pub struct FrontierPoint {
    pub annual_aisc: Claim<Quantity>,
    pub remaining_aisc_spend: Claim<Quantity>,
    pub remaining_aisc_rate: Claim<Quantity>,
}

/// Sign of the arithmetic spending requirement, retained rather than silently replaced with zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendingRequirement {
    Nonnegative,
    IncludesNegative,
    EntirelyNegative,
}

/// Exact requirements; they are neither forecasts nor probabilities of meeting guidance.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionReport {
    /// Guidance minus YTD production. A negative bound means that part of the target was already surpassed.
    pub remaining_production: Claim<Quantity>,
    pub ytd_aisc_spend: Claim<Quantity>,
    /// The range across the entire annual target and declared rounding/scenario boxes, in USD millions.
    pub remaining_aisc_spend: Claim<Quantity>,
    /// The corresponding remaining rate, conditional on `remaining_sold`.
    pub remaining_aisc_rate: Claim<Quantity>,
    /// Lower, then upper annual AISC target; each retains its own spending and rate uncertainty.
    pub frontier: [FrontierPoint; 2],
    pub spending_requirement: SpendingRequirement,
}

fn put_rat(enc: &mut Encoder, x: Rat) {
    enc.put_i128(x.num());
    enc.put_i128(x.den());
}

fn exact<T>(r: Exact<T>) -> Result<T> {
    r.map_err(|e| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!("guidance completion exact arithmetic: {e}"),
        )
    })
}

fn refuse<T>(code: RefusalCode, detail: impl Into<String>) -> Result<T> {
    Err(Refusal::new(code, detail))
}

fn closed(lo: Rat, hi: Rat) -> Interval {
    Interval {
        lo,
        hi,
        lo_strict: false,
        hi_strict: false,
    }
}

fn point(x: Rat) -> Interval {
    closed(x, x)
}

fn max(a: Rat, b: Rat) -> Exact<Rat> {
    Ok(if a.checked_cmp(b)? == Ordering::Less {
        b
    } else {
        a
    })
}

fn min(a: Rat, b: Rat) -> Exact<Rat> {
    Ok(if a.checked_cmp(b)? == Ordering::Greater {
        b
    } else {
        a
    })
}

fn intersection(a: Interval, b: Interval) -> Exact<Option<Interval>> {
    let (lo, hi) = (max(a.lo, b.lo)?, min(a.hi, b.hi)?);
    Ok((lo.checked_cmp(hi)? != Ordering::Greater).then(|| closed(lo, hi)))
}

fn hull(values: impl IntoIterator<Item = Rat>) -> Exact<Interval> {
    let mut values = values.into_iter();
    let Some(first) = values.next() else {
        return Err(UnitsError::Overflow);
    };
    let (mut lo, mut hi) = (first, first);
    for x in values {
        lo = min(lo, x)?;
        hi = max(hi, x)?;
    }
    Ok(closed(lo, hi))
}

fn validate_claim(c: &Claim<Quantity>, role: &str, kind: QuantityKind) -> Result<()> {
    if matches!(c.representation, Representation::Placeholder { .. }) {
        return refuse(
            RefusalCode::SourceCellUnusable208,
            format!("{role}: a placeholder cannot supply a numeric completion input"),
        );
    }
    if c.stale || c.state == KnowledgeState::Stale {
        return refuse(RefusalCode::StaleAnchor502, format!("{role}: stale input"));
    }
    if !matches!(c.state, KnowledgeState::Known | KnowledgeState::Estimated) {
        return refuse(
            RefusalCode::NotObservable202,
            format!(
                "{role}: input state {:?} supplies no usable number",
                c.state
            ),
        );
    }
    if c.calibration_failed {
        return refuse(
            RefusalCode::CalibrationRequired205,
            format!("{role}: failed calibration"),
        );
    }
    if c.anchors.is_empty()
        || c.anchors.iter().any(|a| {
            a.artifact_path.trim().is_empty()
                || a.locator.trim().is_empty()
                || a.artifact_sha256 == [0; 32]
        })
    {
        return refuse(
            RefusalCode::NoClaimInDomain201,
            format!(
                "{role}: every input needs a locator and source digest, including a sales scenario"
            ),
        );
    }
    if c.support != SpatialSupport::PeriodAggregate {
        return refuse(
            RefusalCode::SupportMismatch102,
            format!("{role}: needs a period aggregate"),
        );
    }
    if c.additivity != kind.additivity() {
        return refuse(
            RefusalCode::NonAdditiveAverage103,
            format!("{role}: additivity does not match its quantity"),
        );
    }
    let q = &c.value;
    if q.kind != kind {
        return refuse(
            RefusalCode::BasisMismatch105,
            format!(
                "{role}: expected {kind:?}, got {:?}; no unit or metal-stage conversion is implicit",
                q.kind
            ),
        );
    }
    if q.subject.trim().is_empty()
        || (kind == QuantityKind::AiscUsdPerSoldOz && q.definition.trim().is_empty())
    {
        return refuse(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!("{role}: subject or AISC definition is undeclared"),
        );
    }
    if q.period.year == 0
        || q.period.year > 9999
        || q.period.first_month == 0
        || q.period.first_month > q.period.last_month
        || q.period.last_month > 12
    {
        return refuse(
            RefusalCode::SupportMismatch102,
            format!("{role}: invalid calendar-month support"),
        );
    }
    if let Ownership::Attributable { share } = q.ownership
        && (share.signum() <= 0 || exact(share.checked_cmp(ONE))? == Ordering::Greater)
    {
        return refuse(
            RefusalCode::BasisMismatch105,
            format!("{role}: attributable share must be in (0, 1]"),
        );
    }
    if q.interval.lo_strict || q.interval.hi_strict {
        return refuse(
            RefusalCode::OutsideAoa203,
            format!("{role}: this completion kernel requires explicitly closed rounding bounds"),
        );
    }
    if exact(q.interval.lo.checked_cmp(q.interval.hi))? == Ordering::Greater {
        return refuse(
            RefusalCode::OutsidePhysicalDomain109,
            format!("{role}: reversed interval"),
        );
    }
    let strictly_positive = kind != QuantityKind::ProductionKoz;
    if q.interval.lo.signum() < 0 || (strictly_positive && q.interval.lo == ZERO) {
        return refuse(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "{role}: production must be nonnegative; this slice needs strictly positive sales and AISC throughout their intervals"
            ),
        );
    }
    Ok(())
}

fn same_basis(a: &Quantity, b: &Quantity, role: &str) -> Result<()> {
    if a.subject != b.subject || a.ownership != b.ownership {
        return refuse(
            RefusalCode::BasisMismatch105,
            format!("{role}: subject or ownership basis differs; no implicit share scaling"),
        );
    }
    Ok(())
}

fn validate_inputs(i: &CompletionInputs) -> Result<Period> {
    for (c, role, kind) in [
        (
            &i.production_guidance,
            "production_guidance",
            QuantityKind::ProductionKoz,
        ),
        (
            &i.aisc_guidance,
            "aisc_guidance",
            QuantityKind::AiscUsdPerSoldOz,
        ),
        (
            &i.ytd_production,
            "ytd_production",
            QuantityKind::ProductionKoz,
        ),
        (&i.ytd_sold, "ytd_sold", QuantityKind::SoldKoz),
        (&i.ytd_aisc, "ytd_aisc", QuantityKind::AiscUsdPerSoldOz),
        (&i.remaining_sold, "remaining_sold", QuantityKind::SoldKoz),
    ] {
        validate_claim(c, role, kind)?;
        same_basis(&i.production_guidance.value, &c.value, role)?;
    }
    let annual = i.production_guidance.value.period;
    let ytd = i.ytd_production.value.period;
    if annual.first_month != 1
        || annual.last_month != 12
        || i.aisc_guidance.value.period != annual
        || ytd.year != annual.year
        || ytd.first_month != 1
        || ytd.last_month >= 12
        || i.ytd_sold.value.period != ytd
        || i.ytd_aisc.value.period != ytd
    {
        return refuse(
            RefusalCode::SupportMismatch102,
            "guidance must cover one calendar year and all YTD actuals the same strict prefix of it",
        );
    }
    let remaining = Period {
        year: annual.year,
        first_month: ytd.last_month + 1,
        last_month: 12,
    };
    if i.remaining_sold.value.period != remaining {
        return refuse(
            RefusalCode::SupportMismatch102,
            "remaining_sold must cover precisely the months after the YTD actuals",
        );
    }
    if i.aisc_guidance.value.definition != i.ytd_aisc.value.definition {
        return refuse(
            RefusalCode::BasisMismatch105,
            "aisc_guidance and ytd_aisc use different cost definitions",
        );
    }
    Ok(remaining)
}

fn derived_value<T>(
    metric: &str,
    parents: &[&Claim<Quantity>],
    value: T,
    tag: &str,
    additivity: AdditivityClass,
) -> Claim<T> {
    let mut evidence = EvidenceClass::Derived;
    let mut epistemic = EpistemicStatus::KnownKnown;
    let mut state = KnowledgeState::Known;
    let mut confidentiality = ConfidentialityTier::Public;
    let mut uu_terms = Vec::new();
    let mut anchors = Vec::new();
    let mut parent_ids = Vec::new();
    for p in parents {
        evidence = evidence.meet(p.evidence);
        epistemic = epistemic.and(p.epistemic);
        state = state.and(p.state);
        confidentiality = confidentiality.combine(p.confidentiality);
        for term in &p.uu_terms {
            if !uu_terms
                .iter()
                .any(|u: &fmine_claim::UuTerm| u.scenario_id == term.scenario_id)
            {
                uu_terms.push(term.clone());
            }
        }
        anchors.extend(p.anchors.iter().cloned());
        parent_ids.push(p.claim_id);
    }
    anchors.sort();
    anchors.dedup();
    parent_ids.sort();
    parent_ids.dedup();
    Claim::new(
        metric,
        value,
        tag,
        // These are exact, deterministically computed sets, not probabilistic confidence intervals. Their
        // rational bounds live in the value; rounding them through Representation::Interval would lose data.
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        evidence,
        epistemic,
        state,
        confidentiality,
        uu_terms,
        parent_ids,
        anchors,
    )
}

fn derived(
    metric: &str,
    parents: &[&Claim<Quantity>],
    template: &Quantity,
    period: Period,
    kind: QuantityKind,
    interval: Interval,
) -> Claim<Quantity> {
    let value = Quantity {
        period,
        kind,
        interval,
        ..template.clone()
    };
    let tag = value.canonical_tag();
    derived_value(metric, parents, value, &tag, kind.additivity())
}

/// Exact extrema of `N_rem = [A*R + (A-r)*S]/1000` and `r_rem = A + (A-r)*S/R`.
/// A, r, S, R vary over closed boxes, with S,R positive. Both functions are monotone in each variable when the
/// others are fixed, so their extrema occur at corners. Keeping the same S in both terms avoids treating the
/// YTD numerator and annual sales as independent measurements.
fn frontier_bounds(
    target: Interval,
    rate: Interval,
    sold: Interval,
    remaining: Interval,
) -> Exact<(Interval, Interval)> {
    let mut spends = Vec::with_capacity(16);
    let mut rates = Vec::with_capacity(16);
    for a in [target.lo, target.hi] {
        for r in [rate.lo, rate.hi] {
            for s in [sold.lo, sold.hi] {
                for rem in [remaining.lo, remaining.hi] {
                    let numerator = a
                        .checked_mul(rem)?
                        .checked_add(a.checked_sub(r)?.checked_mul(s)?)?;
                    spends.push(numerator.checked_div(THOUSAND)?);
                    rates.push(numerator.checked_div(rem)?);
                }
            }
        }
    }
    Ok((hull(spends)?, hull(rates)?))
}

/// Computes the exact required remaining production and the AISC frontier conditional on `remaining_sold`.
/// All emitted quantities retain input evidence classes, citations, confidentiality, and parent claim ids.
pub fn complete(i: &CompletionInputs) -> Result<CompletionReport> {
    let remaining = validate_inputs(i)?;
    let production = &i.production_guidance.value;
    let cost = &i.aisc_guidance.value;
    let p = i.ytd_production.value.interval;
    let r = i.ytd_aisc.value.interval;
    let s = i.ytd_sold.value.interval;
    let rem = i.remaining_sold.value.interval;
    let remaining_production = exact((|| {
        Ok(closed(
            production.interval.lo.checked_sub(p.hi)?,
            production.interval.hi.checked_sub(p.lo)?,
        ))
    })())?;
    let ytd_spend = exact((|| {
        Ok(closed(
            r.lo.checked_mul(s.lo)?.checked_div(THOUSAND)?,
            r.hi.checked_mul(s.hi)?.checked_div(THOUSAND)?,
        ))
    })())?;
    let cost_parents = [
        &i.aisc_guidance,
        &i.ytd_aisc,
        &i.ytd_sold,
        &i.remaining_sold,
    ];
    let (spend, rate) = exact(frontier_bounds(cost.interval, r, s, rem))?;
    let endpoint = |a| -> Result<FrontierPoint> {
        let (n, rate) = exact(frontier_bounds(point(a), r, s, rem))?;
        Ok(FrontierPoint {
            annual_aisc: derived(
                "guidance.annual_aisc_endpoint/1",
                &[&i.aisc_guidance],
                cost,
                cost.period,
                QuantityKind::AiscUsdPerSoldOz,
                point(a),
            ),
            remaining_aisc_spend: derived(
                "guidance.remaining_aisc_spend/1",
                &cost_parents,
                cost,
                remaining,
                QuantityKind::AiscSpendUsdMillion,
                n,
            ),
            remaining_aisc_rate: derived(
                "guidance.remaining_aisc_rate/1",
                &cost_parents,
                cost,
                remaining,
                QuantityKind::AiscUsdPerSoldOz,
                rate,
            ),
        })
    };
    Ok(CompletionReport {
        remaining_production: derived(
            "guidance.remaining_production/1",
            &[&i.production_guidance, &i.ytd_production],
            production,
            remaining,
            QuantityKind::ProductionKoz,
            remaining_production,
        ),
        ytd_aisc_spend: derived(
            "guidance.ytd_aisc_spend/1",
            &[&i.ytd_aisc, &i.ytd_sold],
            cost,
            i.ytd_aisc.value.period,
            QuantityKind::AiscSpendUsdMillion,
            ytd_spend,
        ),
        remaining_aisc_spend: derived(
            "guidance.remaining_aisc_spend/1",
            &cost_parents,
            cost,
            remaining,
            QuantityKind::AiscSpendUsdMillion,
            spend,
        ),
        remaining_aisc_rate: derived(
            "guidance.remaining_aisc_rate/1",
            &cost_parents,
            cost,
            remaining,
            QuantityKind::AiscUsdPerSoldOz,
            rate,
        ),
        frontier: [endpoint(cost.interval.lo)?, endpoint(cost.interval.hi)?],
        spending_requirement: if spend.hi.signum() < 0 {
            SpendingRequirement::EntirelyNegative
        } else if spend.lo.signum() < 0 {
            SpendingRequirement::IncludesNegative
        } else {
            SpendingRequirement::Nonnegative
        },
    })
}

/// One historical continuation, whose production, sales and AISC all describe the same earlier period.
/// Its month span must equal the target's remaining span; a quarter is not silently doubled to represent H2.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoricalContinuation {
    pub id: String,
    pub production: Claim<Quantity>,
    pub sold: Claim<Quantity>,
    pub aisc: Claim<Quantity>,
}

/// A rational witness inside one historical period's rounding box and every declared current input interval.
/// Quantities are named with their units; the witness itself is returned inside a provenance-bearing Claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionWitness {
    pub ytd_production_koz: Rat,
    pub remaining_production_koz: Rat,
    pub ytd_sold_koz: Rat,
    pub remaining_sold_koz: Rat,
    pub ytd_aisc_usd_per_oz: Rat,
    pub remaining_aisc_usd_per_oz: Rat,
    pub annual_production_koz: Rat,
    pub annual_aisc_usd_per_oz: Rat,
    pub remaining_spend_usd_m: Rat,
}

impl CompletionWitness {
    fn canonical_tag(&self, history: &str) -> String {
        let mut enc = Encoder::new_frame(*b"GCW1", 1);
        enc.put_str(history);
        for x in [
            self.ytd_production_koz,
            self.remaining_production_koz,
            self.ytd_sold_koz,
            self.remaining_sold_koz,
            self.ytd_aisc_usd_per_oz,
            self.remaining_aisc_usd_per_oz,
            self.annual_production_koz,
            self.annual_aisc_usd_per_oz,
            self.remaining_spend_usd_m,
        ] {
            put_rat(&mut enc, x);
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// A check against one paired historical period. Separate marginal successes never substitute for `witness`.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuationCheck {
    pub id: String,
    pub production_possible: bool,
    pub cost_possible: bool,
    pub witness: Option<Claim<CompletionWitness>>,
}

/// Which stated envelope a comparison uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeTier {
    /// The finite union of the supplied, cited, same-period historical rounding boxes.
    T1,
    /// No historical or technical-report bound: necessary arithmetic conditions only.
    T3,
}

/// Conditional verdict about the supplied envelope, never about the mine's physical ability to meet guidance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryVerdict {
    WithinEnvelope,
    ExceedsEnvelope,
    Indeterminate,
}

/// Joint historical comparisons, with a witness for each possible continuation.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryReport {
    pub tier: EnvelopeTier,
    pub verdict: HistoryVerdict,
    pub checks: Vec<ContinuationCheck>,
}

fn validate_history(i: &CompletionInputs, h: &HistoricalContinuation) -> Result<()> {
    if h.id.trim().is_empty() {
        return refuse(
            RefusalCode::NoClaimInDomain201,
            "historical continuation has no id",
        );
    }
    for (claim, role, kind) in [
        (
            &h.production,
            "historical production",
            QuantityKind::ProductionKoz,
        ),
        (&h.sold, "historical sold", QuantityKind::SoldKoz),
        (&h.aisc, "historical AISC", QuantityKind::AiscUsdPerSoldOz),
    ] {
        validate_claim(claim, role, kind)?;
        same_basis(&i.production_guidance.value, &claim.value, role)?;
    }
    let hp = h.production.value.period;
    let rp = i.remaining_sold.value.period;
    if hp != h.sold.value.period
        || hp != h.aisc.value.period
        || hp.first_month != rp.first_month
        || hp.last_month != rp.last_month
    {
        return refuse(
            RefusalCode::SupportMismatch102,
            format!(
                "{}: historical production, sales and cost must describe one period with the same remaining-month span",
                h.id
            ),
        );
    }
    if hp.year >= rp.year {
        return refuse(
            RefusalCode::AssayLookahead108,
            format!(
                "{}: a historical continuation must precede the guidance year",
                h.id
            ),
        );
    }
    if h.aisc.value.definition != i.aisc_guidance.value.definition {
        return refuse(
            RefusalCode::BasisMismatch105,
            format!("{}: historical and guided AISC definitions differ", h.id),
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct CostPoint {
    ytd_sold: Rat,
    ytd_numerator: Rat,
    remaining_sold: Rat,
    remaining_numerator: Rat,
    annual_rate: Rat,
}

fn cost_points(
    i: &CompletionInputs,
    h: &HistoricalContinuation,
    remaining: Interval,
) -> Exact<Vec<CostPoint>> {
    let mut points = Vec::with_capacity(16);
    for s in [i.ytd_sold.value.interval.lo, i.ytd_sold.value.interval.hi] {
        for r in [i.ytd_aisc.value.interval.lo, i.ytd_aisc.value.interval.hi] {
            for rem in [remaining.lo, remaining.hi] {
                for rem_r in [h.aisc.value.interval.lo, h.aisc.value.interval.hi] {
                    let ytd_numerator = s.checked_mul(r)?;
                    let remaining_numerator = rem.checked_mul(rem_r)?;
                    points.push(CostPoint {
                        ytd_sold: s,
                        ytd_numerator,
                        remaining_sold: rem,
                        remaining_numerator,
                        annual_rate: ytd_numerator
                            .checked_add(remaining_numerator)?
                            .checked_div(s.checked_add(rem)?)?,
                    });
                }
            }
        }
    }
    Ok(points)
}

/// Feasible sales/numerator pairs form a convex wedge `r_lo*S <= N <= r_hi*S`. Interpolating those pairs,
/// rather than independently interpolating rates and sales, constructs an exact witness for any annual rate
/// between the minimum and maximum. No square root, probability model or relaxation is needed.
fn cost_witness(
    i: &CompletionInputs,
    h: &HistoricalContinuation,
    remaining: Interval,
) -> Exact<Option<CostPoint>> {
    let points = cost_points(i, h, remaining)?;
    let Some(first) = points.first().copied() else {
        return Err(UnitsError::Overflow);
    };
    let (mut low, mut high) = (first, first);
    for p in points {
        if p.annual_rate.checked_cmp(low.annual_rate)? == Ordering::Less {
            low = p;
        }
        if p.annual_rate.checked_cmp(high.annual_rate)? == Ordering::Greater {
            high = p;
        }
    }
    let Some(target) = intersection(
        closed(low.annual_rate, high.annual_rate),
        i.aisc_guidance.value.interval,
    )?
    else {
        return Ok(None);
    };
    if target.lo == low.annual_rate {
        return Ok(Some(low));
    }
    let a = target.lo;
    let n0 = low.ytd_numerator.checked_add(low.remaining_numerator)?;
    let n1 = high.ytd_numerator.checked_add(high.remaining_numerator)?;
    let s0 = low.ytd_sold.checked_add(low.remaining_sold)?;
    let s1 = high.ytd_sold.checked_add(high.remaining_sold)?;
    let weight = a.checked_mul(s0)?.checked_sub(n0)?.checked_div(
        n1.checked_sub(n0)?
            .checked_sub(a.checked_mul(s1.checked_sub(s0)?)?)?,
    )?;
    let blend =
        |left: Rat, right: Rat| left.checked_add(weight.checked_mul(right.checked_sub(left)?)?);
    Ok(Some(CostPoint {
        ytd_sold: blend(low.ytd_sold, high.ytd_sold)?,
        ytd_numerator: blend(low.ytd_numerator, high.ytd_numerator)?,
        remaining_sold: blend(low.remaining_sold, high.remaining_sold)?,
        remaining_numerator: blend(low.remaining_numerator, high.remaining_numerator)?,
        annual_rate: a,
    }))
}

fn build_witness(
    i: &CompletionInputs,
    h: &HistoricalContinuation,
) -> Exact<(bool, bool, Option<CompletionWitness>)> {
    let p = i.ytd_production.value.interval;
    let rem_p = h.production.value.interval;
    let total = closed(p.lo.checked_add(rem_p.lo)?, p.hi.checked_add(rem_p.hi)?);
    let production = intersection(total, i.production_guidance.value.interval)?;
    let sold = intersection(h.sold.value.interval, i.remaining_sold.value.interval)?;
    let cost = match sold {
        Some(sold) => cost_witness(i, h, sold)?,
        None => None,
    };
    let witness = match (production, cost) {
        (Some(total), Some(cost)) => {
            let ytd_p = max(p.lo, total.lo.checked_sub(rem_p.hi)?)?;
            Some(CompletionWitness {
                ytd_production_koz: ytd_p,
                remaining_production_koz: total.lo.checked_sub(ytd_p)?,
                ytd_sold_koz: cost.ytd_sold,
                remaining_sold_koz: cost.remaining_sold,
                ytd_aisc_usd_per_oz: cost.ytd_numerator.checked_div(cost.ytd_sold)?,
                remaining_aisc_usd_per_oz: cost
                    .remaining_numerator
                    .checked_div(cost.remaining_sold)?,
                annual_production_koz: total.lo,
                annual_aisc_usd_per_oz: cost.annual_rate,
                remaining_spend_usd_m: cost.remaining_numerator.checked_div(THOUSAND)?,
            })
        }
        _ => None,
    };
    Ok((production.is_some(), cost.is_some(), witness))
}

/// Independently checks a witness against the original intervals and the production and annual cost equations.
/// Returns false for an invalid witness; malformed inputs or exact arithmetic overflow return typed refusals.
pub fn check_witness(
    i: &CompletionInputs,
    h: &HistoricalContinuation,
    w: &CompletionWitness,
) -> Result<bool> {
    validate_inputs(i)?;
    validate_history(i, h)?;
    exact((|| {
        for (range, value) in [
            (i.ytd_production.value.interval, w.ytd_production_koz),
            (h.production.value.interval, w.remaining_production_koz),
            (i.ytd_sold.value.interval, w.ytd_sold_koz),
            (h.sold.value.interval, w.remaining_sold_koz),
            (i.remaining_sold.value.interval, w.remaining_sold_koz),
            (i.ytd_aisc.value.interval, w.ytd_aisc_usd_per_oz),
            (h.aisc.value.interval, w.remaining_aisc_usd_per_oz),
            (
                i.production_guidance.value.interval,
                w.annual_production_koz,
            ),
            (i.aisc_guidance.value.interval, w.annual_aisc_usd_per_oz),
        ] {
            if !range.contains(value)? {
                return Ok(false);
            }
        }
        if w.ytd_production_koz
            .checked_add(w.remaining_production_koz)?
            != w.annual_production_koz
        {
            return Ok(false);
        }
        let ytd_n = w.ytd_sold_koz.checked_mul(w.ytd_aisc_usd_per_oz)?;
        let rem_n = w
            .remaining_sold_koz
            .checked_mul(w.remaining_aisc_usd_per_oz)?;
        if rem_n.checked_div(THOUSAND)? != w.remaining_spend_usd_m {
            return Ok(false);
        }
        Ok(ytd_n.checked_add(rem_n)?
            == w.annual_aisc_usd_per_oz
                .checked_mul(w.ytd_sold_koz.checked_add(w.remaining_sold_koz)?)?)
    })())
}

/// Compares the targets with each supplied historical continuation jointly. Empty history returns T3 and
/// Indeterminate; otherwise T1 is precisely the finite union of those period boxes, under the explicit sales
/// scenario. It never assembles a witness from the production of one period and the costs of another.
pub fn compare_history(
    i: &CompletionInputs,
    history: &[HistoricalContinuation],
) -> Result<HistoryReport> {
    validate_inputs(i)?;
    if history.is_empty() {
        return Ok(HistoryReport {
            tier: EnvelopeTier::T3,
            verdict: HistoryVerdict::Indeterminate,
            checks: Vec::new(),
        });
    }
    let mut ordered: Vec<_> = history.iter().collect();
    ordered.sort_by(|a, b| a.id.cmp(&b.id));
    if ordered.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return refuse(
            RefusalCode::BasisMismatch105,
            "historical continuation ids must be unique",
        );
    }
    let mut checks = Vec::with_capacity(history.len());
    for h in ordered {
        validate_history(i, h)?;
        let (production_possible, cost_possible, witness) = exact(build_witness(i, h))?;
        let witness = if let Some(witness) = witness {
            if !check_witness(i, h, &witness)? {
                return refuse(
                    RefusalCode::NumericalCertificateFailed207,
                    format!("{}: independently checked completion witness failed", h.id),
                );
            }
            let tag = witness.canonical_tag(&h.id);
            Some(derived_value(
                "guidance.historical_completion_witness/1",
                &[
                    &i.production_guidance,
                    &i.aisc_guidance,
                    &i.ytd_production,
                    &i.ytd_sold,
                    &i.ytd_aisc,
                    &i.remaining_sold,
                    &h.production,
                    &h.sold,
                    &h.aisc,
                ],
                witness,
                &tag,
                AdditivityClass::NonAdditive,
            ))
        } else {
            None
        };
        checks.push(ContinuationCheck {
            id: h.id.clone(),
            production_possible,
            cost_possible,
            witness,
        });
    }
    Ok(HistoryReport {
        tier: EnvelopeTier::T1,
        verdict: if checks.iter().any(|c| c.witness.is_some()) {
            HistoryVerdict::WithinEnvelope
        } else {
            HistoryVerdict::ExceedsEnvelope
        },
        checks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::anchor::SourceAnchor;

    fn n(x: i128) -> Rat {
        Rat::from_integer(x)
    }
    fn q(a: i128, b: i128) -> Rat {
        Rat::new(a, b).unwrap()
    }

    fn period(first_month: u8, last_month: u8) -> Period {
        Period {
            year: 2026,
            first_month,
            last_month,
        }
    }

    fn claim(key: &str, kind: QuantityKind, period: Period, lo: Rat, hi: Rat) -> Claim<Quantity> {
        let value = Quantity {
            subject: "Synthetic site".into(),
            period,
            ownership: Ownership::Attributable { share: q(4, 5) },
            kind,
            definition: if kind == QuantityKind::AiscUsdPerSoldOz {
                "synthetic/by_product/sold".into()
            } else {
                String::new()
            },
            interval: closed(lo, hi),
        };
        let tag = value.canonical_tag();
        Claim::new(
            key,
            value,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            kind.additivity(),
            EvidenceClass::ModelledUncalibrated,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            ConfidentialityTier::Public,
            Vec::new(),
            Vec::new(),
            vec![SourceAnchor::new("synthetic/guidance", [7; 32], key)],
        )
    }

    /// Synthetic arithmetic uses the bead's printed-shape example, including its explicitly invented sold
    /// ounces. This is not a reproduction of North Mara's sold-ounces cell or a live forecast.
    fn inputs() -> CompletionInputs {
        CompletionInputs {
            production_guidance: claim(
                "guidance.production",
                QuantityKind::ProductionKoz,
                period(1, 12),
                n(200),
                n(230),
            ),
            aisc_guidance: claim(
                "guidance.aisc",
                QuantityKind::AiscUsdPerSoldOz,
                period(1, 12),
                n(1520),
                n(1680),
            ),
            ytd_production: claim(
                "ytd.production",
                QuantityKind::ProductionKoz,
                period(1, 6),
                n(68),
                n(68),
            ),
            ytd_sold: claim(
                "ytd.sold",
                QuantityKind::SoldKoz,
                period(1, 6),
                n(68),
                n(68),
            ),
            ytd_aisc: claim(
                "ytd.aisc",
                QuantityKind::AiscUsdPerSoldOz,
                period(1, 6),
                n(2283),
                n(2283),
            ),
            remaining_sold: claim(
                "scenario.sold",
                QuantityKind::SoldKoz,
                period(7, 12),
                n(132),
                n(162),
            ),
        }
    }

    fn history(id: &str, production: i128, sold: i128, rate: i128) -> HistoricalContinuation {
        let p = Period {
            year: 2025,
            first_month: 7,
            last_month: 12,
        };
        HistoricalContinuation {
            id: id.into(),
            production: claim(
                &format!("{id}.production"),
                QuantityKind::ProductionKoz,
                p,
                n(production),
                n(production),
            ),
            sold: claim(
                &format!("{id}.sold"),
                QuantityKind::SoldKoz,
                p,
                n(sold),
                n(sold),
            ),
            aisc: claim(
                &format!("{id}.aisc"),
                QuantityKind::AiscUsdPerSoldOz,
                p,
                n(rate),
                n(rate),
            ),
        }
    }

    #[test]
    fn synthetic_completion_known_answers_are_exact() {
        let i = inputs();
        let r = complete(&i).unwrap();
        assert_eq!(
            r.remaining_production.value.interval,
            closed(n(132), n(162))
        );
        assert_eq!(r.ytd_aisc_spend.value.interval, point(q(155244, 1000)));
        assert_eq!(
            r.remaining_aisc_spend.value.interval,
            closed(q(148756, 1000), q(231156, 1000))
        );
        assert_eq!(r.frontier[1].annual_aisc.value.interval, point(n(1680)));
        assert_eq!(
            r.frontier[1].remaining_aisc_spend.value.interval,
            closed(q(180756, 1000), q(231156, 1000))
        );
        assert_eq!(
            r.frontier[1].remaining_aisc_rate.value.interval,
            closed(q(180756, 132), q(231156, 162))
        );
        assert_eq!(r.remaining_production.value.period, period(7, 12));
        assert_eq!(r.spending_requirement, SpendingRequirement::Nonnegative);
    }

    #[test]
    fn sold_and_produced_are_separate_and_rates_are_not_averaged() {
        let mut i = inputs();
        i.ytd_sold.value.interval = point(n(60));
        i.remaining_sold.value.interval = point(n(100));
        let r = complete(&i).unwrap();
        assert_eq!(
            r.remaining_production.value.interval,
            closed(n(132), n(162))
        );
        assert_eq!(r.ytd_aisc_spend.value.interval, point(q(13698, 100)));
        assert_eq!(
            r.frontier[1].remaining_aisc_rate.value.interval,
            point(q(13182, 10))
        );
        assert_ne!(
            r.frontier[1].remaining_aisc_rate.value.interval,
            point(n(2 * 1680 - 2283))
        );
    }

    #[test]
    fn correlated_sales_and_numerator_do_not_create_false_uncertainty() {
        let mut i = inputs();
        i.aisc_guidance.value.interval = point(n(100));
        i.ytd_aisc.value.interval = point(n(100));
        i.ytd_sold.value.interval = closed(n(10), n(20));
        i.remaining_sold.value.interval = point(n(10));
        let r = complete(&i).unwrap();
        assert_eq!(r.remaining_aisc_spend.value.interval, point(n(1)));
        assert_eq!(r.remaining_aisc_rate.value.interval, point(n(100)));
    }

    #[test]
    fn rounding_extrema_enclose_every_small_interior_combination() {
        let target = closed(n(4), n(8));
        let rate = closed(n(2), n(6));
        let sold = closed(n(1), n(3));
        let remaining = closed(n(1), n(4));
        let (ns, rs) = frontier_bounds(target, rate, sold, remaining).unwrap();
        let (mut seen_n_lo, mut seen_n_hi, mut seen_r_lo, mut seen_r_hi) =
            (false, false, false, false);
        for a in 4..=8 {
            for r in 2..=6 {
                for s in 1..=3 {
                    for rem in 1..=4 {
                        let numerator = a * rem + (a - r) * s;
                        let spend = q(numerator, 1000);
                        let rate = q(numerator, rem);
                        assert!(ns.contains(spend).unwrap());
                        assert!(rs.contains(rate).unwrap());
                        seen_n_lo |= spend == ns.lo;
                        seen_n_hi |= spend == ns.hi;
                        seen_r_lo |= rate == rs.lo;
                        seen_r_hi |= rate == rs.hi;
                    }
                }
            }
        }
        assert!(seen_n_lo && seen_n_hi && seen_r_lo && seen_r_hi);
    }

    #[test]
    fn signed_derived_requirements_are_never_clamped() {
        let mut i = inputs();
        i.production_guidance.value.interval = closed(n(20), n(30));
        i.ytd_production.value.interval = point(n(40));
        i.aisc_guidance.value.interval = point(n(1));
        i.ytd_aisc.value.interval = point(n(100));
        i.ytd_sold.value.interval = point(n(100));
        i.remaining_sold.value.interval = point(n(1));
        let r = complete(&i).unwrap();
        assert_eq!(
            r.remaining_production.value.interval,
            closed(n(-20), n(-10))
        );
        assert_eq!(r.remaining_aisc_spend.value.interval, point(q(-9899, 1000)));
        assert_eq!(
            r.spending_requirement,
            SpendingRequirement::EntirelyNegative
        );
        i.ytd_production.value.interval = point(ZERO);
        assert!(complete(&i).is_ok());
    }

    #[test]
    fn unsupported_domains_and_zero_denominators_refuse_by_name() {
        for role in ["ytd_sold", "remaining_sold", "ytd_aisc", "aisc_guidance"] {
            let mut i = inputs();
            let c = match role {
                "ytd_sold" => &mut i.ytd_sold,
                "remaining_sold" => &mut i.remaining_sold,
                "ytd_aisc" => &mut i.ytd_aisc,
                _ => &mut i.aisc_guidance,
            };
            c.value.interval.lo = ZERO;
            let refusal = complete(&i).unwrap_err();
            assert_eq!(refusal.code, "FMINE-REF-109");
            assert!(refusal.what.contains(role));
        }
        let mut i = inputs();
        i.ytd_production.value.interval.lo = n(-1);
        assert_eq!(complete(&i).unwrap_err().code, "FMINE-REF-109");
        let mut i = inputs();
        i.remaining_sold.value.interval = closed(n(10), n(1));
        assert_eq!(complete(&i).unwrap_err().code, "FMINE-REF-109");
        let mut i = inputs();
        i.remaining_sold.value.interval.lo_strict = true;
        assert_eq!(complete(&i).unwrap_err().code, "FMINE-REF-203");
    }

    #[test]
    fn support_basis_units_definitions_and_evidence_are_checked() {
        let mut cases: Vec<(CompletionInputs, &str)> = Vec::new();
        let mut i = inputs();
        i.ytd_production.value.ownership = Ownership::Full;
        cases.push((i, "FMINE-REF-105"));
        let mut i = inputs();
        i.ytd_sold.value.kind = QuantityKind::ProductionKoz;
        cases.push((i, "FMINE-REF-105"));
        let mut i = inputs();
        i.ytd_sold.value.subject = "Another site".into();
        cases.push((i, "FMINE-REF-105"));
        let mut i = inputs();
        i.ytd_aisc.value.definition = "co_product".into();
        cases.push((i, "FMINE-REF-105"));
        let mut i = inputs();
        i.ytd_aisc.value.definition.clear();
        cases.push((i, "FMINE-REF-107"));
        let mut i = inputs();
        i.ytd_aisc.support = SpatialSupport::PlantLot;
        cases.push((i, "FMINE-REF-102"));
        let mut i = inputs();
        i.ytd_sold.value.period.last_month = 3;
        cases.push((i, "FMINE-REF-102"));
        let mut i = inputs();
        i.remaining_sold.value.period.first_month = 6;
        cases.push((i, "FMINE-REF-102"));
        let mut i = inputs();
        i.ytd_aisc.additivity = AdditivityClass::ExtensiveAdditive;
        cases.push((i, "FMINE-REF-103"));
        let mut i = inputs();
        i.ytd_production.anchors.clear();
        cases.push((i, "FMINE-REF-201"));
        let mut i = inputs();
        i.ytd_production.state = KnowledgeState::NotObservable;
        cases.push((i, "FMINE-REF-202"));
        let mut i = inputs();
        i.ytd_production.stale = true;
        cases.push((i, "FMINE-REF-502"));
        let mut i = inputs();
        i.ytd_production.calibration_failed = true;
        cases.push((i, "FMINE-REF-205"));
        let mut i = inputs();
        i.ytd_production.representation = Representation::Placeholder {
            token: "n/a".into(),
        };
        cases.push((i, "FMINE-REF-208"));
        for (i, code) in cases {
            assert_eq!(complete(&i).unwrap_err().code, code);
        }
    }

    #[test]
    fn exact_overflow_is_a_numerical_refusal() {
        let mut i = inputs();
        i.ytd_sold.value.interval = point(n(i128::MAX));
        i.ytd_aisc.value.interval = point(n(i128::MAX));
        assert_eq!(complete(&i).unwrap_err().code, "FMINE-REF-207");
    }

    #[test]
    fn derived_claims_preserve_provenance_and_never_upgrade_evidence() {
        let mut i = inputs();
        i.ytd_sold.confidentiality = ConfidentialityTier::Internal;
        let r = complete(&i).unwrap();
        let cost = &r.remaining_aisc_rate;
        assert_eq!(cost.evidence, EvidenceClass::ModelledUncalibrated);
        assert_eq!(cost.confidentiality, ConfidentialityTier::Internal);
        assert_eq!(cost.parents.len(), 4);
        assert_eq!(cost.anchors.len(), 4);
        assert!(cost.parents.contains(&i.remaining_sold.claim_id));
        assert_eq!(cost.representation, Representation::Deterministic);
        i.production_guidance.evidence = EvidenceClass::Measured;
        i.ytd_production.evidence = EvidenceClass::Measured;
        assert_eq!(
            complete(&i).unwrap().remaining_production.evidence,
            EvidenceClass::Derived
        );
        i.production_guidance.evidence = EvidenceClass::ISSUER;
        assert_eq!(
            complete(&i).unwrap().remaining_production.evidence,
            EvidenceClass::ISSUER
        );
        assert!(r.remaining_production.value.canonical_tag() != cost.value.canonical_tag());
    }

    #[test]
    fn separate_marginal_successes_are_not_a_joint_history_witness() {
        let i = inputs();
        let fast_expensive = history("fast", 140, 140, 2000);
        let slow_cheap = history("cheap", 100, 140, 1200);
        let r = compare_history(&i, &[fast_expensive, slow_cheap]).unwrap();
        assert_eq!(r.tier, EnvelopeTier::T1);
        assert_eq!(r.verdict, HistoryVerdict::ExceedsEnvelope);
        assert!(r.checks.iter().any(|c| c.production_possible));
        assert!(r.checks.iter().any(|c| c.cost_possible));
        assert!(r.checks.iter().all(|c| c.witness.is_none()));
        let good = history("good", 140, 140, 1300);
        let r = compare_history(&i, std::slice::from_ref(&good)).unwrap();
        assert_eq!(r.verdict, HistoryVerdict::WithinEnvelope);
        assert!(check_witness(&i, &good, &r.checks[0].witness.as_ref().unwrap().value).unwrap());
    }

    #[test]
    fn fractional_history_witness_hits_a_boundary_exactly() {
        let mut i = inputs();
        i.production_guidance.value.interval = point(n(100));
        i.ytd_production.value.interval = point(n(50));
        i.ytd_sold.value.interval = point(n(100));
        i.ytd_aisc.value.interval = point(n(100));
        i.aisc_guidance.value.interval = point(n(150));
        i.remaining_sold.value.interval = closed(n(50), n(200));
        let mut h = history("varying", 50, 50, 160);
        h.sold.value.interval = closed(n(50), n(200));
        h.aisc.value.interval = closed(n(160), n(220));
        let r = compare_history(&i, std::slice::from_ref(&h)).unwrap();
        let w = &r.checks[0].witness.as_ref().unwrap().value;
        assert_eq!(w.annual_aisc_usd_per_oz, n(150));
        assert_eq!(w.remaining_sold_koz, n(100));
        assert_eq!(w.remaining_aisc_usd_per_oz, n(200));
        assert!(check_witness(&i, &h, w).unwrap());
        let mut forged = w.clone();
        forged.remaining_spend_usd_m = forged.remaining_spend_usd_m.checked_add(ONE).unwrap();
        assert!(!check_witness(&i, &h, &forged).unwrap());
    }

    #[test]
    fn historical_verdict_matches_exhaustive_discrete_continuations() {
        let mut i = inputs();
        i.production_guidance.value.interval = closed(n(4), n(5));
        i.ytd_production.value.interval = point(ONE);
        i.ytd_sold.value.interval = point(ONE);
        i.ytd_aisc.value.interval = point(n(10));
        i.aisc_guidance.value.interval = closed(n(5), n(15));
        i.remaining_sold.value.interval = closed(ONE, n(3));
        for p in 0..=4 {
            for s in 1..=3 {
                for r in 1..=20 {
                    let h = history("enumerated", p, s, r);
                    let annual_p = 1 + p;
                    let numerator = 10 + s * r;
                    let denominator = 1 + s;
                    let expected = (4..=5).contains(&annual_p)
                        && 5 * denominator <= numerator
                        && numerator <= 15 * denominator;
                    let actual = compare_history(&i, std::slice::from_ref(&h)).unwrap();
                    assert_eq!(
                        actual.verdict == HistoryVerdict::WithinEnvelope,
                        expected,
                        "p={p} s={s} r={r}"
                    );
                    if let Some(w) = &actual.checks[0].witness {
                        assert!(check_witness(&i, &h, &w.value).unwrap());
                    }
                }
            }
        }
    }

    #[test]
    fn history_cannot_mix_periods_bases_or_undeclared_sales() {
        let i = inputs();
        let h = history("history", 140, 140, 1300);
        let mut wrong = h.clone();
        wrong.aisc.value.period.year = 2024;
        assert_eq!(
            compare_history(&i, &[wrong]).unwrap_err().code,
            "FMINE-REF-102"
        );
        let mut wrong = h.clone();
        wrong.sold.value.ownership = Ownership::Full;
        assert_eq!(
            compare_history(&i, &[wrong]).unwrap_err().code,
            "FMINE-REF-105"
        );
        let mut wrong = h.clone();
        wrong.sold.value.interval = point(n(100));
        let r = compare_history(&i, &[wrong]).unwrap();
        assert_eq!(r.verdict, HistoryVerdict::ExceedsEnvelope);
        assert!(!r.checks[0].cost_possible);
        let mut future = h.clone();
        for c in [&mut future.production, &mut future.sold, &mut future.aisc] {
            c.value.period.year = 2026;
        }
        assert_eq!(
            compare_history(&i, &[future]).unwrap_err().code,
            "FMINE-REF-108"
        );
        assert_eq!(
            compare_history(&i, &[h.clone(), h]).unwrap_err().code,
            "FMINE-REF-105"
        );
    }

    #[test]
    fn missing_history_is_t3_and_order_is_deterministic() {
        let i = inputs();
        let empty = compare_history(&i, &[]).unwrap();
        assert_eq!(empty.tier, EnvelopeTier::T3);
        assert_eq!(empty.verdict, HistoryVerdict::Indeterminate);
        let (a, b) = (history("a", 140, 140, 1300), history("b", 150, 150, 1250));
        let forward = compare_history(&i, &[a.clone(), b.clone()]).unwrap();
        let reverse = compare_history(&i, &[b, a]).unwrap();
        assert_eq!(forward, reverse);
        assert_eq!(complete(&i).unwrap(), complete(&i).unwrap());
    }
}
