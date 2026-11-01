//! Descriptive grade-normalised recovery comparison (bd-srqc, W4 §3.3).
//! Determinism: DeterministicPerIsa (whole-output cross-host agreement has not been established).
//!
//! Grade and recovery use percent units: `0.64` means 0.64% Cu and `91` means 91% recovery.
//! Implied tails are `h * (1 - R/100)`. The two models are a constant mean tails grade and an
//! ordinary least-squares tails-on-head-grade line. Coefficient/projection bounds enclose input
//! rounding, not sampling uncertainty or a confidence interval. No operational cause is identified.

use std::cmp::Ordering;

use fmine_claim::{
    AdditivityClass, Claim, ConfidentialityTier, Representation, SpatialSupport, UuTerm,
};
use fmine_core::{Encoder, blake3_hash, det};
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_units::UnitsError;

pub use crate::guidance::{Ownership, Period};

pub const RULES_VERSION: &str = "m0-recovery/1";
pub const NO_CLAIM: &str = "Descriptive comparison with an annual historical grade and recovery relation; no causal attribution, recoverable-metal estimate, forecast or affiliation implied.";
pub const MODEL_CAVEAT: &str = "The models omit ore type, grind, throughput effects and concentrate mass pull, and apply annual training to quarterly observations. Input-rounding enclosures are not confidence intervals or prediction intervals. Monetary downside is not quantified.";

const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
const HUNDRED: Rat = Rat::from_integer(100);
type Result<T> = std::result::Result<T, Refusal>;

/// The physical unit is part of the type; a recovery fraction cannot silently stand in for percent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityKind {
    HeadGradePercent,
    RecoveryPercent,
    ProcessedKt,
    CopperProducedKt,
}

impl QuantityKind {
    #[must_use]
    pub fn additivity(self) -> AdditivityClass {
        match self {
            Self::HeadGradePercent | Self::RecoveryPercent => AdditivityClass::IntensiveWeighted,
            Self::ProcessedKt | Self::CopperProducedKt => AdditivityClass::ExtensiveAdditive,
        }
    }
}

/// A displayed point and its closed rounding interval, on one full-mine calendar support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quantity {
    pub subject: String,
    pub period: Period,
    pub ownership: Ownership,
    pub kind: QuantityKind,
    pub point: Rat,
    pub interval: Interval,
}

impl Quantity {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut e = Encoder::new_frame(*b"RCQ1", 1);
        support(&mut e, &self.subject, self.period, self.ownership);
        e.put_u8(self.kind as u8);
        put_estimate(&mut e, self.estimate());
        blake3_hash(&e.finish()).to_hex()
    }

    fn estimate(&self) -> Estimate {
        Estimate {
            point: self.point,
            interval: self.interval,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrainingRow {
    pub head: Claim<Quantity>,
    pub recovery: Claim<Quantity>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperatingRow {
    pub head: Claim<Quantity>,
    pub recovery: Claim<Quantity>,
    pub processed: Claim<Quantity>,
    pub produced: Claim<Quantity>,
}

/// Exact displayed-input result and a conservative enclosure over all declared input-rounding boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estimate {
    pub point: Rat,
    pub interval: Interval,
}

/// Numerical enclosure of the square root of an exact printed-input variance. This is not a
/// rounding-sensitivity range, confidence interval or prediction interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SquareRoot {
    pub approx: f64,
    pub enclosure: [f64; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    pub subject: String,
    pub ownership: Ownership,
    pub training_years: Vec<u16>,
    /// Retained with mean tails to propagate the centred regression without discarding dependencies.
    pub mean_head: Estimate,
    /// M-a, in percent Cu.
    pub constant_tails: Estimate,
    /// M-b intercept, in percent Cu.
    pub intercept: Estimate,
    /// M-b slope, percent Cu tails per percent Cu head (dimensionless).
    pub slope: Estimate,
    /// SSE/(n-1), the descriptive sample variance of the fitted residuals used in W4.
    pub residual_sample_variance_at_printed: Rat,
    pub residual_sample_sd_at_printed: SquareRoot,
    /// SSE/(n-2), the conventional OLS residual-error variance; distinct from the sample summary.
    pub regression_error_variance_at_printed: Rat,
    pub regression_error_sd_at_printed: SquareRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    ConstantTails,
    GradeDependentTails,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelProjection {
    pub subject: String,
    pub period: Period,
    pub ownership: Ownership,
    pub model: Model,
    pub expected_recovery_percent: Estimate,
    /// Expected minus reported recovery, in percentage points; the sign is retained.
    pub gap_pp: Estimate,
    /// Processed feed times head grade times the signed recovery gap. Not a recoverable-metal estimate.
    pub copper_difference_kt: Estimate,
    /// Signed model difference divided by reported copper production, not by a modelled denominator.
    pub fraction_of_reported_output: Estimate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Projection {
    pub constant_tails: Claim<ModelProjection>,
    pub grade_dependent_tails: Claim<ModelProjection>,
}

fn fail<T>(code: RefusalCode, detail: impl Into<String>) -> Result<T> {
    Err(Refusal::new(
        code,
        format!("recovery comparison: {}", detail.into()),
    ))
}

fn exact<T>(result: std::result::Result<T, UnitsError>) -> Result<T> {
    result.map_err(|e| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!("recovery comparison exact arithmetic: {e}"),
        )
    })
}

fn bounds(lo: Rat, hi: Rat) -> Interval {
    Interval {
        lo,
        hi,
        lo_strict: false,
        hi_strict: false,
    }
}

fn min(a: Rat, b: Rat) -> Result<Rat> {
    Ok(if exact(a.checked_cmp(b))? == Ordering::Greater {
        b
    } else {
        a
    })
}

fn max(a: Rat, b: Rat) -> Result<Rat> {
    Ok(if exact(a.checked_cmp(b))? == Ordering::Less {
        b
    } else {
        a
    })
}

impl Estimate {
    fn exact(value: Rat) -> Self {
        Self {
            point: value,
            interval: bounds(value, value),
        }
    }

    fn add(self, rhs: Self) -> Result<Self> {
        Ok(Self {
            point: exact(self.point.checked_add(rhs.point))?,
            interval: bounds(
                exact(self.interval.lo.checked_add(rhs.interval.lo))?,
                exact(self.interval.hi.checked_add(rhs.interval.hi))?,
            ),
        })
    }

    fn sub(self, rhs: Self) -> Result<Self> {
        Ok(Self {
            point: exact(self.point.checked_sub(rhs.point))?,
            interval: bounds(
                exact(self.interval.lo.checked_sub(rhs.interval.hi))?,
                exact(self.interval.hi.checked_sub(rhs.interval.lo))?,
            ),
        })
    }

    fn mul(self, rhs: Self) -> Result<Self> {
        let products = [
            exact(self.interval.lo.checked_mul(rhs.interval.lo))?,
            exact(self.interval.lo.checked_mul(rhs.interval.hi))?,
            exact(self.interval.hi.checked_mul(rhs.interval.lo))?,
            exact(self.interval.hi.checked_mul(rhs.interval.hi))?,
        ];
        let (mut lo, mut hi) = (products[0], products[0]);
        for x in &products[1..] {
            lo = min(lo, *x)?;
            hi = max(hi, *x)?;
        }
        Ok(Self {
            point: exact(self.point.checked_mul(rhs.point))?,
            interval: bounds(lo, hi),
        })
    }

    fn div(self, rhs: Self) -> Result<Self> {
        if rhs.interval.lo.signum() <= 0 {
            return fail(
                RefusalCode::NonIdentifiableAttribution104,
                "a denominator's rounding interval must be strictly positive",
            );
        }
        self.mul(Self {
            point: exact(ONE.checked_div(rhs.point))?,
            interval: bounds(
                exact(ONE.checked_div(rhs.interval.hi))?,
                exact(ONE.checked_div(rhs.interval.lo))?,
            ),
        })
    }

    fn square(self) -> Result<Self> {
        let lo2 = exact(self.interval.lo.checked_mul(self.interval.lo))?;
        let hi2 = exact(self.interval.hi.checked_mul(self.interval.hi))?;
        // Squaring an interval that contains zero has the exact lower bound zero. This is the
        // range of the square operation, not a clamp on a signed model result.
        let lo = if self.interval.lo.signum() <= 0 && self.interval.hi.signum() >= 0 {
            ZERO
        } else {
            min(lo2, hi2)?
        };
        Ok(Self {
            point: exact(self.point.checked_mul(self.point))?,
            interval: bounds(lo, max(lo2, hi2)?),
        })
    }
}

fn put_rat(e: &mut Encoder, x: Rat) {
    e.put_i128(x.num());
    e.put_i128(x.den());
}
fn put_estimate(e: &mut Encoder, x: Estimate) {
    put_rat(e, x.point);
    put_rat(e, x.interval.lo);
    put_rat(e, x.interval.hi);
    e.put_u8(u8::from(x.interval.lo_strict));
    e.put_u8(u8::from(x.interval.hi_strict));
}
fn support(e: &mut Encoder, subject: &str, period: Period, ownership: Ownership) {
    e.put_str(subject);
    e.put_u16(period.year);
    e.put_u8(period.first_month);
    e.put_u8(period.last_month);
    match ownership {
        Ownership::Full => e.put_u8(0),
        Ownership::Attributable { share } => {
            e.put_u8(1);
            put_rat(e, share);
        }
    }
}

fn envelope<T>(c: &Claim<T>) -> Result<()> {
    if c.stale || c.state == KnowledgeState::Stale {
        return fail(RefusalCode::StaleAnchor502, "stale input");
    }
    if c.calibration_failed {
        return fail(
            RefusalCode::CalibrationRequired205,
            "input failed calibration",
        );
    }
    if !matches!(c.state, KnowledgeState::Known | KnowledgeState::Estimated) {
        return fail(
            RefusalCode::NotObservable202,
            "input has no usable known or estimated value",
        );
    }
    if matches!(c.representation, Representation::Placeholder { .. }) {
        return fail(RefusalCode::SourceCellUnusable208, "placeholder input");
    }
    if c.anchors.is_empty()
        || c.anchors.iter().any(|a| {
            a.artifact_path.trim().is_empty()
                || a.locator.trim().is_empty()
                || a.artifact_sha256 == [0; 32]
        })
    {
        return fail(
            RefusalCode::NoClaimInDomain201,
            "every input needs a nonempty source path, locator and digest",
        );
    }
    if c.support != SpatialSupport::PeriodAggregate {
        return fail(
            RefusalCode::SupportMismatch102,
            "period aggregate support required",
        );
    }
    Ok(())
}

fn check_estimate(e: Estimate) -> Result<()> {
    if e.interval.lo_strict || e.interval.hi_strict {
        return fail(RefusalCode::OutsideAoa203, "rounding bounds must be closed");
    }
    if exact(e.interval.lo.checked_cmp(e.interval.hi))? == Ordering::Greater
        || !exact(e.interval.contains(e.point))?
    {
        return fail(
            RefusalCode::OutsidePhysicalDomain109,
            "reversed bounds or displayed point outside its bounds",
        );
    }
    Ok(())
}

fn quantity(c: &Claim<Quantity>, kind: QuantityKind) -> Result<()> {
    envelope(c)?;
    let q = &c.value;
    if q.subject.trim().is_empty() {
        return fail(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "subject is undeclared",
        );
    }
    if q.kind != kind || q.ownership != Ownership::Full {
        return fail(
            RefusalCode::BasisMismatch105,
            "explicit percent/kt units and full-mine basis required",
        );
    }
    if c.additivity != kind.additivity() {
        return fail(
            RefusalCode::NonAdditiveAverage103,
            "quantity additivity mismatch",
        );
    }
    if q.period.year == 0
        || q.period.year > 9999
        || q.period.first_month == 0
        || q.period.last_month > 12
        || q.period.first_month > q.period.last_month
    {
        return fail(
            RefusalCode::SupportMismatch102,
            "invalid calendar-month support",
        );
    }
    check_estimate(q.estimate())?;
    let lower_invalid = match kind {
        QuantityKind::RecoveryPercent | QuantityKind::ProcessedKt => q.interval.lo.signum() < 0,
        _ => q.interval.lo.signum() <= 0,
    };
    if lower_invalid
        || (matches!(
            kind,
            QuantityKind::HeadGradePercent | QuantityKind::RecoveryPercent
        ) && exact(q.interval.hi.checked_cmp(HUNDRED))? == Ordering::Greater)
    {
        return fail(
            RefusalCode::OutsidePhysicalDomain109,
            "physical input outside its declared percent or mass domain",
        );
    }
    Ok(())
}

fn same_row(a: &Quantity, b: &Quantity) -> Result<()> {
    if a.subject != b.subject || a.ownership != b.ownership {
        return fail(
            RefusalCode::BasisMismatch105,
            "row subject or ownership differs",
        );
    }
    if a.period != b.period {
        return fail(
            RefusalCode::SupportMismatch102,
            "row inputs cover different periods",
        );
    }
    Ok(())
}

/// Bracket positive integer casts before division, then correctly rounded sqrt before outward steps.
/// The exact rational radicand remains in the Fit for independent checking.
fn square_root(x: Rat) -> Result<SquareRoot> {
    if x.signum() < 0 {
        return fail(
            RefusalCode::NumericalCertificateFailed207,
            "negative exact residual variance",
        );
    }
    if x == ZERO {
        return Ok(SquareRoot {
            approx: 0.0,
            enclosure: [0.0, 0.0],
        });
    }
    let (n, d) = (x.num() as f64, x.den() as f64);
    let lower = (n.next_down() / d.next_up()).next_down();
    let upper = (n.next_up() / d.next_down()).next_up();
    let out = SquareRoot {
        approx: det::sqrt(x.to_f64()),
        enclosure: [det::sqrt(lower).next_down(), det::sqrt(upper).next_up()],
    };
    if !out.approx.is_finite()
        || !out.enclosure.iter().all(|v| v.is_finite())
        || out.enclosure[0] > out.approx
        || out.enclosure[1] < out.approx
    {
        return fail(
            RefusalCode::NumericalCertificateFailed207,
            "invalid numerical square-root enclosure",
        );
    }
    Ok(out)
}

/// Aggregate heterogeneous source envelopes without strengthening evidence or dropping confidentiality.
struct Provenance {
    evidence: EvidenceClass,
    epistemic: EpistemicStatus,
    state: KnowledgeState,
    confidentiality: ConfidentialityTier,
    uu: Vec<UuTerm>,
    anchors: Vec<fmine_core::anchor::SourceAnchor>,
    parents: Vec<fmine_claim::ClaimId>,
}

impl Provenance {
    fn new() -> Self {
        Self { evidence: EvidenceClass::ModelledUncalibrated, epistemic: EpistemicStatus::KnownUnknown(KuKind::Parametric), state: KnowledgeState::Estimated, confidentiality: ConfidentialityTier::Public, uu: vec![UuTerm { scenario_id: RULES_VERSION.into(), description: MODEL_CAVEAT.into(), downside_exposure_usd_m: 0.0, trigger_indicator: "Ore mix, grind, throughput, concentrate mass pull or annual-to-quarter support differs; monetary downside remains unquantified.".into() }], anchors: Vec::new(), parents: Vec::new() }
    }
    fn include<T>(&mut self, c: &Claim<T>) {
        self.evidence = self.evidence.meet(c.evidence);
        self.epistemic = self.epistemic.and(c.epistemic);
        self.state = self.state.and(c.state);
        self.confidentiality = self.confidentiality.combine(c.confidentiality);
        for term in &c.uu_terms {
            if !self.uu.iter().any(|t| t == term) {
                self.uu.push(term.clone());
            }
        }
        self.anchors.extend(c.anchors.iter().cloned());
        self.parents.push(c.claim_id);
    }
    fn claim<T>(mut self, metric: &str, value: T, tag: &str) -> Claim<T> {
        self.anchors.sort();
        self.anchors.dedup();
        self.parents.sort();
        self.parents.dedup();
        self.uu.sort_by(|a, b| {
            a.scenario_id
                .cmp(&b.scenario_id)
                .then(a.description.cmp(&b.description))
                .then(
                    a.downside_exposure_usd_m
                        .total_cmp(&b.downside_exposure_usd_m),
                )
                .then(a.trigger_indicator.cmp(&b.trigger_indicator))
        });
        Claim::new(
            metric,
            value,
            tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::NonAdditive,
            self.evidence,
            self.epistemic,
            self.state,
            self.confidentiality,
            self.uu,
            self.parents,
            self.anchors,
        )
    }
}

impl Fit {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut e = Encoder::new_frame(*b"RCF1", 1);
        e.put_str(RULES_VERSION);
        e.put_str(&self.subject);
        e.put_u8(u8::from(self.ownership == Ownership::Full));
        e.put_u64(self.training_years.len() as u64);
        for year in &self.training_years {
            e.put_u16(*year);
        }
        for estimate in [
            self.mean_head,
            self.constant_tails,
            self.intercept,
            self.slope,
        ] {
            put_estimate(&mut e, estimate);
        }
        for (variance, sd) in [
            (
                self.residual_sample_variance_at_printed,
                self.residual_sample_sd_at_printed,
            ),
            (
                self.regression_error_variance_at_printed,
                self.regression_error_sd_at_printed,
            ),
        ] {
            put_rat(&mut e, variance);
            e.put_u64(sd.approx.to_bits());
            for end in sd.enclosure {
                e.put_u64(end.to_bits());
            }
        }
        blake3_hash(&e.finish()).to_hex()
    }
}

/// Fit exactly the two historical tails models. Rows must be distinct, complete calendar years on
/// one full-mine basis. Any rounding box that cannot establish a positive grade-design variance refuses.
pub fn fit(rows: &[TrainingRow]) -> Result<Claim<Fit>> {
    if rows.len() < 3 {
        return fail(
            RefusalCode::NonIdentifiableAttribution104,
            "at least three distinct annual observations required",
        );
    }
    let mut rows = rows.iter().collect::<Vec<_>>();
    rows.sort_by_key(|r| r.head.value.period.year);
    let subject = rows[0].head.value.subject.clone();
    let mut years = Vec::new();
    let mut provenance = Provenance::new();
    let mut heads = Vec::new();
    let mut tails = Vec::new();
    let (mut sum_h, mut sum_t) = (Estimate::exact(ZERO), Estimate::exact(ZERO));
    for row in rows {
        quantity(&row.head, QuantityKind::HeadGradePercent)?;
        quantity(&row.recovery, QuantityKind::RecoveryPercent)?;
        same_row(&row.head.value, &row.recovery.value)?;
        let period = row.head.value.period;
        if period.first_month != 1 || period.last_month != 12 {
            return fail(
                RefusalCode::SupportMismatch102,
                "training observations must cover complete calendar years",
            );
        }
        if row.head.value.subject != subject {
            return fail(RefusalCode::BasisMismatch105, "training subjects differ");
        }
        if years.last() == Some(&period.year) {
            return fail(
                RefusalCode::NonIdentifiableAttribution104,
                "duplicate annual observation",
            );
        }
        years.push(period.year);
        let h = row.head.value.estimate();
        let t = h.mul(
            Estimate::exact(ONE).sub(
                row.recovery
                    .value
                    .estimate()
                    .div(Estimate::exact(HUNDRED))?,
            )?,
        )?;
        sum_h = sum_h.add(h)?;
        sum_t = sum_t.add(t)?;
        heads.push(h);
        tails.push(t);
        provenance.include(&row.head);
        provenance.include(&row.recovery);
    }
    let n = Rat::from_integer(years.len() as i128);
    let mean_h = sum_h.div(Estimate::exact(n))?;
    let mean_t = sum_t.div(Estimate::exact(n))?;
    let (mut pair_xx, mut pair_xy) = (Estimate::exact(ZERO), Estimate::exact(ZERO));
    let mut pair_yy = ZERO;
    for i in 0..heads.len() {
        for j in 0..i {
            let dh = heads[i].sub(heads[j])?;
            let dt = tails[i].sub(tails[j])?;
            pair_xx = pair_xx.add(dh.square()?)?;
            pair_xy = pair_xy.add(dh.mul(dt)?)?;
            pair_yy = exact(pair_yy.checked_add(exact(dt.point.checked_mul(dt.point))?))?;
        }
    }
    // Sxx and Sxy both equal their pairwise sums divided by n; that common factor cancels.
    // The sum of squares has no cancellation-prone n*sum(h^2)-sum(h)^2 denominator.
    let slope = pair_xy.div(pair_xx)?;
    let intercept = mean_t.sub(slope.mul(mean_h)?)?;
    let explained =
        exact(exact(pair_xy.point.checked_mul(pair_xy.point))?.checked_div(pair_xx.point))?;
    let sse = exact(exact(pair_yy.checked_sub(explained))?.checked_div(n))?;
    let residual_sample_variance_at_printed = exact(sse.checked_div(exact(n.checked_sub(ONE))?))?;
    let regression_error_variance_at_printed =
        exact(sse.checked_div(exact(n.checked_sub(Rat::from_integer(2)))?))?;
    let value = Fit {
        subject,
        ownership: Ownership::Full,
        training_years: years,
        mean_head: mean_h,
        constant_tails: mean_t,
        intercept,
        slope,
        residual_sample_variance_at_printed,
        residual_sample_sd_at_printed: square_root(residual_sample_variance_at_printed)?,
        regression_error_variance_at_printed,
        regression_error_sd_at_printed: square_root(regression_error_variance_at_printed)?,
    };
    for e in [
        value.mean_head,
        value.constant_tails,
        value.intercept,
        value.slope,
    ] {
        check_estimate(e)?;
    }
    let tag = value.canonical_tag();
    Ok(provenance.claim("recovery.fit/1", value, &tag))
}

impl ModelProjection {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut e = Encoder::new_frame(*b"RCP1", 1);
        e.put_str(RULES_VERSION);
        support(&mut e, &self.subject, self.period, self.ownership);
        e.put_u8(self.model as u8);
        for estimate in [
            self.expected_recovery_percent,
            self.gap_pp,
            self.copper_difference_kt,
            self.fraction_of_reported_output,
        ] {
            put_estimate(&mut e, estimate);
        }
        blake3_hash(&e.finish()).to_hex()
    }
}

/// Apply both annual models to a later complete calendar quarter. Signed gaps and differences are
/// retained even when a model extrapolates outside physical recovery limits; none is clamped.
pub fn project(fitted: &Claim<Fit>, row: &OperatingRow) -> Result<Projection> {
    envelope(fitted)?;
    if fitted.additivity != AdditivityClass::NonAdditive {
        return fail(
            RefusalCode::NonAdditiveAverage103,
            "fit must remain non-additive",
        );
    }
    for (c, kind) in [
        (&row.head, QuantityKind::HeadGradePercent),
        (&row.recovery, QuantityKind::RecoveryPercent),
        (&row.processed, QuantityKind::ProcessedKt),
        (&row.produced, QuantityKind::CopperProducedKt),
    ] {
        quantity(c, kind)?;
        same_row(&row.head.value, &c.value)?;
    }
    let f = &fitted.value;
    let q = &row.head.value;
    if f.subject != q.subject || f.ownership != Ownership::Full {
        return fail(
            RefusalCode::BasisMismatch105,
            "fit and operating subject or full-mine basis differ",
        );
    }
    if f.training_years.len() < 3
        || f.training_years.iter().any(|y| *y == 0 || *y > 9999)
        || f.training_years.windows(2).any(|w| w[0] >= w[1])
    {
        return fail(
            RefusalCode::SupportMismatch102,
            "fit needs distinct ordered annual training years",
        );
    }
    if !matches!(q.period.first_month, 1 | 4 | 7 | 10)
        || q.period.last_month != q.period.first_month + 2
    {
        return fail(
            RefusalCode::SupportMismatch102,
            "application requires one complete calendar quarter",
        );
    }
    if f.training_years
        .last()
        .is_some_and(|year| *year >= q.period.year)
    {
        return fail(
            RefusalCode::AssayLookahead108,
            "quarter must follow every training year",
        );
    }
    for e in [f.mean_head, f.constant_tails, f.intercept, f.slope] {
        check_estimate(e)?;
    }
    let h = q.estimate();
    let r = row.recovery.value.estimate();
    let make = |model: Model| -> Result<Claim<ModelProjection>> {
        let tails = match model {
            Model::ConstantTails => f.constant_tails,
            Model::GradeDependentTails => {
                f.constant_tails.add(f.slope.mul(h.sub(f.mean_head)?)?)?
            }
        };
        let expected = Estimate::exact(HUNDRED).mul(Estimate::exact(ONE).sub(tails.div(h)?)?)?;
        let gap = expected.sub(r)?;
        let difference = row
            .processed
            .value
            .estimate()
            .mul(h)?
            .mul(gap)?
            .div(Estimate::exact(Rat::from_integer(10_000)))?;
        let fraction = difference.div(row.produced.value.estimate())?;
        let value = ModelProjection {
            subject: q.subject.clone(),
            period: q.period,
            ownership: Ownership::Full,
            model,
            expected_recovery_percent: expected,
            gap_pp: gap,
            copper_difference_kt: difference,
            fraction_of_reported_output: fraction,
        };
        for e in [expected, gap, difference, fraction] {
            check_estimate(e)?;
        }
        let mut provenance = Provenance::new();
        provenance.include(fitted);
        for c in [&row.head, &row.recovery, &row.processed, &row.produced] {
            provenance.include(c);
        }
        let tag = value.canonical_tag();
        Ok(provenance.claim(
            match model {
                Model::ConstantTails => "recovery.constant_tails/1",
                Model::GradeDependentTails => "recovery.grade_dependent_tails/1",
            },
            value,
            &tag,
        ))
    };
    Ok(Projection {
        constant_tails: make(Model::ConstantTails)?,
        grade_dependent_tails: make(Model::GradeDependentTails)?,
    })
}

#[cfg(test)]
mod tests;
