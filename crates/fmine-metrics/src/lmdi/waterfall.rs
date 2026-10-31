//! Typed public AISC accounting bridge (BD-333.S1, plan §21.6).
//! Determinism: DeterministicPerIsa (exact rates and additive bars; portable-log LMDI enclosures).
//!
//! Quantities use $M, koz, thousand reported tonnes, g/t and a recovery fraction. The absolute metal-balance
//! factor is `produced * 31.1034768 / (tonnes * grade * recovery)` in those units. Only its temporal log
//! change is evaluated here, so the exact statutory grams/troy-ounce constant cancels. No absolute B is
//! emitted. These are descriptive accounting effects, never identified operational causes.
//!
//! Intervals retain independent input boxes and thus conservatively over-enclose shared dependencies.
//! Each floating-point operation rounds outwards; logarithm bounds widen by four ulps. Logarithmic-mean
//! endpoint bounds use interval arithmetic and its proven [min, max] bound, not a guessed error margin
//! on the approximate point evaluation. Exact arithmetic overflow or an unbounded box refuses.

use std::collections::BTreeMap;

use fmine_core::det;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::exact_feas::Rat;
use fmine_units::UnitsError;

use super::{log_ratio, logarithmic_mean};

pub const WATERFALL_RULES_VERSION: &str = "m0-waterfall/1";
pub const WATERFALL_NO_CLAIM: &str =
    "Descriptive accounting bridge; arithmetic closure does not identify operational causes.";

const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
const TWO: Rat = Rat::from_integer(2);
const SCALE: Rat = Rat::from_integer(1000);

/// A displayed midpoint, its half-unit rounding radius, and an authentic source-cell locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintedQty {
    pub value: Rat,
    pub half_step: Rat,
    pub at: String,
}

/// A physical quantity or its preserved non-numeric publication token and source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Factor {
    Value(PrintedQty),
    NotPublished { token: String, at: String },
}

/// The caller authenticates this common basis for all quantities in a period; the kernel checks equality.
/// An attributable 100% observation remains distinct from an explicitly consolidated observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BasisTag {
    Consolidated100,
    Attributable { share: Rat },
    Unprinted,
}

/// One period on one stated ownership basis. Units are fixed by this API, never inferred from magnitudes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaterfallPeriod {
    /// Total AISC, in USD millions. Its radius may be the component-sum k/2 construction.
    pub cost: PrintedQty,
    /// Named signed numerator lines in USD millions, with stable identities across periods.
    pub lines: Vec<(String, PrintedQty)>,
    pub oz_sold: PrintedQty,
    /// Observed USD/oz; it never sets a factor or the logarithmic-mean weight.
    pub printed_rate: PrintedQty,
    pub tonnes: Factor,
    pub grade: Factor,
    pub recovery: Factor,
    pub produced: Factor,
    pub basis: BasisTag,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BarPoint {
    Approx(f64),
    Exact(Rat),
}

impl BarPoint {
    #[must_use]
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Approx(v) => v,
            Self::Exact(v) => v.to_f64(),
        }
    }
}

/// USD/oz effect at the printed midpoints and a conservative enclosure over their rounding boxes.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub id: String,
    pub rule: &'static str,
    pub point: BarPoint,
    pub enclosure: [f64; 2],
    pub sign_not_determined: bool,
    /// All direct source dependencies, including the cost/sales inputs to an LMDI weight.
    pub inputs: Vec<String>,
}

/// Exact recomputation and reconciliation, separate from every factor bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateReconciliation {
    pub aisc_0: Rat,
    pub aisc_1: Rat,
    pub printed_aisc_0: Rat,
    pub printed_aisc_1: Rat,
    pub total_delta: Rat,
    pub printed_rate_gap_0: Rat,
    pub printed_rate_gap_1: Rat,
    pub printed_rate_gap: Rat,
    pub inputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Bridge {
    Lmdi {
        bars: Vec<Bar>,
        gap: RateReconciliation,
        /// Measured midpoint residual, at most 1e-9 USD/oz; never allocated to a factor.
        closure_residual: f64,
    },
    AdditiveFallback {
        reason: String,
        bars: Vec<Bar>,
        gap: RateReconciliation,
    },
}

impl Bridge {
    #[must_use]
    pub fn bars(&self) -> &[Bar] {
        match self {
            Self::Lmdi { bars, .. } | Self::AdditiveFallback { bars, .. } => bars,
        }
    }

    #[must_use]
    pub fn gap(&self) -> &RateReconciliation {
        match self {
            Self::Lmdi { gap, .. } | Self::AdditiveFallback { gap, .. } => gap,
        }
    }
}

type Calc<T> = Result<T, Refusal>;

fn numerical(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::NumericalCertificateFailed207, detail)
}

fn exact<T>(r: Result<T, UnitsError>) -> Calc<T> {
    r.map_err(|_| numerical("waterfall exact arithmetic overflowed; no partial bridge"))
}

fn rat_text(v: Rat) -> String {
    if v.den() == 1 {
        v.num().to_string()
    } else {
        format!("{}/{}", v.num(), v.den())
    }
}

fn locators<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<_> = values.into_iter().map(str::to_owned).collect();
    out.sort();
    out.dedup();
    out
}

fn source(at: &str) -> Calc<()> {
    if at.trim().is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "waterfall input has no source locator",
        ));
    }
    Ok(())
}

fn bounds(q: &PrintedQty) -> Calc<[Rat; 2]> {
    source(&q.at)?;
    if q.half_step.signum() < 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("negative rounding half-step at {}", q.at),
        ));
    }
    Ok([
        exact(q.value.checked_sub(q.half_step))?,
        exact(q.value.checked_add(q.half_step))?,
    ])
}

fn check_basis(b: &BasisTag) -> Calc<()> {
    match b {
        BasisTag::Unprinted => Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "waterfall ownership basis is unprinted or unsupported",
        )),
        BasisTag::Attributable { share } if share.signum() <= 0 || share.num() > share.den() => {
            Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "waterfall attributable share must be in (0,1]",
            ))
        }
        _ => Ok(()),
    }
}

fn reconciliation(p0: &WaterfallPeriod, p1: &WaterfallPeriod) -> Calc<RateReconciliation> {
    let aisc_0 = exact(exact(p0.cost.value.checked_div(p0.oz_sold.value))?.checked_mul(SCALE))?;
    let aisc_1 = exact(exact(p1.cost.value.checked_div(p1.oz_sold.value))?.checked_mul(SCALE))?;
    let total_delta = exact(aisc_1.checked_sub(aisc_0))?;
    let printed_rate_gap_0 = exact(p0.printed_rate.value.checked_sub(aisc_0))?;
    let printed_rate_gap_1 = exact(p1.printed_rate.value.checked_sub(aisc_1))?;
    let printed_rate_gap = exact(printed_rate_gap_1.checked_sub(printed_rate_gap_0))?;
    Ok(RateReconciliation {
        aisc_0,
        aisc_1,
        printed_aisc_0: p0.printed_rate.value,
        printed_aisc_1: p1.printed_rate.value,
        total_delta,
        printed_rate_gap_0,
        printed_rate_gap_1,
        printed_rate_gap,
        inputs: locators([
            p0.cost.at.as_str(),
            &p1.cost.at,
            &p0.oz_sold.at,
            &p1.oz_sold.at,
            &p0.printed_rate.at,
            &p1.printed_rate.at,
        ]),
    })
}

fn factor_reason(f: &Factor, name: &str, period: usize) -> Calc<Option<String>> {
    match f {
        Factor::NotPublished { token, at } => {
            source(at)?;
            Ok(Some(format!(
                "period {period} {name} not published: `{token}` ({at})"
            )))
        }
        Factor::Value(q) => domain_reason(q, name, period, name == "recovery"),
    }
}

fn domain_reason(
    q: &PrintedQty,
    name: &str,
    period: usize,
    fraction: bool,
) -> Calc<Option<String>> {
    let [lo, hi] = bounds(q)?;
    if lo.signum() <= 0 || (fraction && hi.num() > hi.den()) {
        return Ok(Some(format!(
            "period {period} {name} rounding interval [{},{}] is outside {} ({})",
            rat_text(lo),
            rat_text(hi),
            if fraction {
                "(0,1]"
            } else {
                "strictly positive values"
            },
            q.at
        )));
    }
    Ok(None)
}

/// Compute a descriptive bridge on exact displayed quantities and a declared common ownership basis.
/// Missing/non-positive physical factors select a labelled signed additive fallback. The fallback refuses
/// if its named lines do not sum exactly to each supplied cost; no rounding discrepancy becomes a plug.
pub fn decompose_aisc_waterfall(p0: &WaterfallPeriod, p1: &WaterfallPeriod) -> Calc<Bridge> {
    check_basis(&p0.basis)?;
    check_basis(&p1.basis)?;
    if p0.basis != p1.basis {
        return Err(Refusal::new(
            RefusalCode::BasisMismatch105,
            "waterfall periods have different ownership bases",
        ));
    }
    let mut reasons = Vec::new();
    for (period, p) in [p0, p1].into_iter().enumerate() {
        let q = bounds(&p.oz_sold)?;
        if q[0].signum() <= 0 {
            return Err(Refusal::new(
                RefusalCode::NonIdentifiableAttribution104,
                format!(
                    "period {period} oz_sold must have a strictly positive rounding interval ({})",
                    p.oz_sold.at
                ),
            ));
        }
        bounds(&p.printed_rate)?;
        if let Some(reason) = domain_reason(&p.cost, "cost", period, false)? {
            reasons.push(reason);
        }
        for (name, f) in [
            ("tonnes", &p.tonnes),
            ("grade", &p.grade),
            ("recovery", &p.recovery),
            ("produced", &p.produced),
        ] {
            if let Some(reason) = factor_reason(f, name, period)? {
                reasons.push(reason);
            }
        }
    }
    let gap = reconciliation(p0, p1)?;
    if reasons.is_empty() {
        lmdi(p0, p1, gap)
    } else {
        additive(p0, p1, gap, reasons.join("; "))
    }
}

/// Closed floating interval. Inputs are finite; operations fail instead of carrying infinities or NaN.
#[derive(Clone, Copy, Debug)]
struct Iv {
    lo: f64,
    hi: f64,
}

impl Iv {
    fn new(lo: f64, hi: f64) -> Calc<Self> {
        if !lo.is_finite() || !hi.is_finite() || lo > hi {
            return Err(numerical("waterfall interval is non-finite or inverted"));
        }
        Ok(Self { lo, hi })
    }

    fn point(v: f64) -> Calc<Self> {
        Self::new(v, v)
    }

    fn rat(v: Rat) -> Calc<Self> {
        if v.signum() == 0 {
            return Self::point(0.0);
        }
        // Each nearest-rounded integer cast is bracketed by adjacent floats; the division also rounds out.
        let n = v.num() as f64;
        let d = v.den() as f64;
        Self::new(n.next_down(), n.next_up())?.div(Self::new(d.next_down(), d.next_up())?)
    }

    fn qty(q: &PrintedQty) -> Calc<Self> {
        let [lo, hi] = bounds(q)?;
        Self::new(Self::rat(lo)?.lo, Self::rat(hi)?.hi)
    }

    fn add(self, rhs: Self) -> Calc<Self> {
        Self::new((self.lo + rhs.lo).next_down(), (self.hi + rhs.hi).next_up())
    }

    fn sub(self, rhs: Self) -> Calc<Self> {
        Self::new((self.lo - rhs.hi).next_down(), (self.hi - rhs.lo).next_up())
    }

    fn neg(self) -> Self {
        Self {
            lo: -self.hi,
            hi: -self.lo,
        }
    }

    fn products(values: [f64; 4]) -> Calc<Self> {
        if !values.into_iter().all(f64::is_finite) {
            return Err(numerical("waterfall interval operation overflowed"));
        }
        let lo = values.into_iter().fold(f64::INFINITY, f64::min);
        let hi = values.into_iter().fold(f64::NEG_INFINITY, f64::max);
        Self::new(lo.next_down(), hi.next_up())
    }

    fn mul(self, rhs: Self) -> Calc<Self> {
        Self::products([
            self.lo * rhs.lo,
            self.lo * rhs.hi,
            self.hi * rhs.lo,
            self.hi * rhs.hi,
        ])
    }

    fn div(self, rhs: Self) -> Calc<Self> {
        if rhs.lo <= 0.0 && rhs.hi >= 0.0 {
            return Err(numerical("waterfall interval divisor contains zero"));
        }
        Self::products([
            self.lo / rhs.lo,
            self.lo / rhs.hi,
            self.hi / rhs.lo,
            self.hi / rhs.hi,
        ])
    }

    fn ln(self) -> Calc<Self> {
        if self.lo <= 0.0 {
            return Err(numerical(
                "waterfall logarithm interval is not strictly positive",
            ));
        }
        let mut lo = det::ln(self.lo);
        let mut hi = det::ln(self.hi);
        for _ in 0..4 {
            lo = lo.next_down();
            hi = hi.next_up();
        }
        Self::new(lo, hi)
    }
}

/// Rigorous interval evaluation under the portable-log four-ulp allowance. Near equality the proven
/// [min,max] inequality remains valid even if cancellation prevents division by a positive log difference.
fn mean_at(a: f64, b: f64) -> Calc<Iv> {
    let lo = a.min(b);
    let hi = a.max(b);
    if lo <= 0.0 {
        return Err(numerical(
            "waterfall logarithmic-mean endpoint is not positive",
        ));
    }
    let coarse = Iv::new(lo, hi)?;
    if lo == hi {
        return Ok(coarse);
    }
    let lower = Iv::point(lo)?;
    let upper = Iv::point(hi)?;
    let denominator = upper.ln()?.sub(lower.ln()?)?;
    if denominator.lo <= 0.0 {
        return Ok(coarse);
    }
    let Ok(quotient) = upper.sub(lower)?.div(denominator) else {
        return Ok(coarse);
    };
    Iv::new(coarse.lo.max(quotient.lo), coarse.hi.min(quotient.hi))
}

fn mean_box(a: Iv, b: Iv) -> Calc<Iv> {
    Iv::new(mean_at(a.lo, b.lo)?.lo, mean_at(a.hi, b.hi)?.hi)
}

fn bar(
    id: &str,
    rule: &'static str,
    point: BarPoint,
    enclosure: Iv,
    inputs: Vec<String>,
) -> Calc<Bar> {
    let value = point.as_f64();
    if !value.is_finite() || value < enclosure.lo || value > enclosure.hi {
        return Err(numerical(format!(
            "waterfall {id} midpoint is non-finite or outside its numerical enclosure"
        )));
    }
    Ok(Bar {
        id: id.to_owned(),
        rule,
        point,
        enclosure: [enclosure.lo, enclosure.hi],
        sign_not_determined: enclosure.lo <= 0.0 && enclosure.hi >= 0.0,
        inputs,
    })
}

fn value(f: &Factor) -> Calc<&PrintedQty> {
    match f {
        Factor::Value(q) => Ok(q),
        Factor::NotPublished { .. } => Err(numerical("unpublished factor entered LMDI branch")),
    }
}

fn weight_inputs<'a>(p0: &'a WaterfallPeriod, p1: &'a WaterfallPeriod) -> Vec<&'a str> {
    vec![&p0.cost.at, &p1.cost.at, &p0.oz_sold.at, &p1.oz_sold.at]
}

fn lmdi(p0: &WaterfallPeriod, p1: &WaterfallPeriod, gap: RateReconciliation) -> Calc<Bridge> {
    let a0 = Iv::qty(&p0.cost)?
        .mul(Iv::point(1000.0)?)?
        .div(Iv::qty(&p0.oz_sold)?)?;
    let a1 = Iv::qty(&p1.cost)?
        .mul(Iv::point(1000.0)?)?
        .div(Iv::qty(&p1.oz_sold)?)?;
    let weight_box = mean_box(a1, a0)?;
    let weight = logarithmic_mean(gap.aisc_1.to_f64(), gap.aisc_0.to_f64());
    if !weight.is_finite() || weight <= 0.0 {
        return Err(numerical(
            "waterfall logarithmic mean is not finite and positive",
        ));
    }
    let q0 = [
        &p0.cost,
        value(&p0.tonnes)?,
        value(&p0.grade)?,
        value(&p0.recovery)?,
        value(&p0.produced)?,
        &p0.oz_sold,
    ];
    let q1 = [
        &p1.cost,
        value(&p1.tonnes)?,
        value(&p1.grade)?,
        value(&p1.recovery)?,
        value(&p1.produced)?,
        &p1.oz_sold,
    ];
    let mut changes = Vec::with_capacity(6);
    let mut boxes = Vec::with_capacity(6);
    for (v0, v1) in q0.iter().zip(&q1) {
        changes.push(log_ratio(v1.value.to_f64(), v0.value.to_f64()));
        boxes.push(Iv::qty(v1)?.ln()?.sub(Iv::qty(v0)?.ln()?)?);
    }
    // B's exact statutory troy conversion cancels in this difference; no absolute B is inferred.
    let balance = changes[4] - changes[1] - changes[2] - changes[3];
    let balance_box = boxes[4].sub(boxes[1])?.sub(boxes[2])?.sub(boxes[3])?;
    let timing = changes[5] - changes[4];
    let timing_box = boxes[5].sub(boxes[4])?;
    let mut bars = Vec::with_capacity(6);
    let specifications = [
        ("cost", "waterfall.lmdi.cost", changes[0], boxes[0], vec![0]),
        (
            "tonnes",
            "waterfall.lmdi.tonnes",
            -changes[1],
            boxes[1].neg(),
            vec![1],
        ),
        (
            "grade",
            "waterfall.lmdi.grade",
            -changes[2],
            boxes[2].neg(),
            vec![2],
        ),
        (
            "recovery",
            "waterfall.lmdi.recovery",
            -changes[3],
            boxes[3].neg(),
            vec![3],
        ),
        (
            "balance",
            "waterfall.lmdi.balance",
            -balance,
            balance_box.neg(),
            vec![1, 2, 3, 4],
        ),
        (
            "sales_timing",
            "waterfall.lmdi.sales_timing",
            -timing,
            timing_box.neg(),
            vec![4, 5],
        ),
    ];
    for (id, rule, log_change, log_box, dependencies) in specifications {
        let mut inputs = weight_inputs(p0, p1);
        for i in dependencies {
            inputs.extend([q0[i].at.as_str(), q1[i].at.as_str()]);
        }
        bars.push(bar(
            id,
            rule,
            BarPoint::Approx(weight * log_change),
            weight_box.mul(log_box)?,
            locators(inputs),
        )?);
    }
    let closure_residual =
        gap.total_delta.to_f64() - bars.iter().map(|b| b.point.as_f64()).sum::<f64>();
    if !closure_residual.is_finite() || closure_residual.abs() > 1e-9 {
        return Err(numerical(
            "waterfall LMDI closure residual exceeds 1e-9 USD/oz",
        ));
    }
    Ok(Bridge::Lmdi {
        bars,
        gap,
        closure_residual,
    })
}

fn lines(p: &WaterfallPeriod, period: usize) -> Calc<BTreeMap<&str, &PrintedQty>> {
    let mut out = BTreeMap::new();
    let mut sum = ZERO;
    for (id, q) in &p.lines {
        bounds(q)?;
        if id.trim().is_empty() || out.insert(id.as_str(), q).is_some() {
            return Err(Refusal::new(
                RefusalCode::NonIdentifiableAttribution104,
                format!(
                    "period {period} additive numerator has an empty or repeated line id: {id}"
                ),
            ));
        }
    }
    for q in out.values() {
        sum = exact(sum.checked_add(q.value))?;
    }
    if out.is_empty() || sum != p.cost.value {
        return Err(Refusal::new(
            RefusalCode::NonIdentifiableAttribution104,
            format!(
                "period {period} signed additive lines sum to {}, supplied cost is {}; no plug ({})",
                rat_text(sum),
                rat_text(p.cost.value),
                p.cost.at
            ),
        ));
    }
    Ok(out)
}

fn additive(
    p0: &WaterfallPeriod,
    p1: &WaterfallPeriod,
    gap: RateReconciliation,
    reason: String,
) -> Calc<Bridge> {
    let lines0 = lines(p0, 0)?;
    let lines1 = lines(p1, 1)?;
    if !lines0.keys().eq(lines1.keys()) {
        return Err(Refusal::new(
            RefusalCode::NonIdentifiableAttribution104,
            "additive numerator line identities differ; an absent line is not an observed zero",
        ));
    }
    let w0 = exact(ONE.checked_div(p0.oz_sold.value))?;
    let w1 = exact(ONE.checked_div(p1.oz_sold.value))?;
    let w_mean = exact(exact(w0.checked_add(w1))?.checked_div(TWO))?;
    let w_delta = exact(w1.checked_sub(w0))?;
    let c_mean = exact(exact(p0.cost.value.checked_add(p1.cost.value))?.checked_div(TWO))?;
    let w0_box = Iv::point(1.0)?.div(Iv::qty(&p0.oz_sold)?)?;
    let w1_box = Iv::point(1.0)?.div(Iv::qty(&p1.oz_sold)?)?;
    let w_mean_box = w0_box.add(w1_box)?.mul(Iv::point(0.5)?)?;
    let w_delta_box = w1_box.sub(w0_box)?;
    let scale = Iv::point(1000.0)?;
    let mut bars = Vec::with_capacity(lines0.len() + 1);
    let mut total = ZERO;
    for ((id, q0), (_, q1)) in lines0.iter().zip(&lines1) {
        let change = exact(q1.value.checked_sub(q0.value))?;
        let point = exact(exact(change.checked_mul(w_mean))?.checked_mul(SCALE))?;
        total = exact(total.checked_add(point))?;
        let enclosure = Iv::qty(q1)?
            .sub(Iv::qty(q0)?)?
            .mul(w_mean_box)?
            .mul(scale)?;
        bars.push(bar(
            &format!("line:{id}"),
            "waterfall.additive.line",
            BarPoint::Exact(point),
            enclosure,
            locators([q0.at.as_str(), &q1.at, &p0.oz_sold.at, &p1.oz_sold.at]),
        )?);
    }
    let point = exact(exact(c_mean.checked_mul(w_delta))?.checked_mul(SCALE))?;
    total = exact(total.checked_add(point))?;
    let enclosure = Iv::qty(&p0.cost)?
        .add(Iv::qty(&p1.cost)?)?
        .mul(Iv::point(0.5)?)?
        .mul(w_delta_box)?
        .mul(scale)?;
    bars.push(bar(
        "volume",
        "waterfall.additive.volume",
        BarPoint::Exact(point),
        enclosure,
        locators(weight_inputs(p0, p1)),
    )?);
    if total != gap.total_delta {
        return Err(numerical("waterfall exact additive bars do not close"));
    }
    Ok(Bridge::AdditiveFallback { reason, bars, gap })
}

#[cfg(test)]
mod tests;
