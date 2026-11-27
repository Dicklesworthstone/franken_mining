//! Twin holes (`BD-285`, plan §12.1, §12.4): how far a twin is from the hole it checks, how well the matched
//! intervals agree, the `G-TWIN` guard on corrections derived from twins, and the RC smearing monitor.
//! Determinism: BitwisePortable (`fmine_core::det` square roots, logarithms and normal CDF; no `std` transcendental).
//!
//! - **Separation.** [`assess_twin`] places each matched interval's midpoint on its hole's desurveyed
//!   [`Trace`] and measures the distance between the two midpoints. The pair's separation is the largest of
//!   these. Above the policy limit (default 5 m, [`DEFAULT_SEPARATION_LIMIT_MM`]) the pair is a
//!   [`TwinState::WeakTwin`] (E375, E376). A twin collared close by can still drift past the limit at depth.
//! - **Separation uncertainty.** [`separation_uncertainty`] gives each matched pair's separation the one-sigma of
//!   the two holes' survey-error covariances along the line between the midpoints ([`crate::survey_error`]), and
//!   the probability that the true separation is beyond the limit.
//! - **Agreement.** Over the matched grades (original `a`, twin `b`, one unit): the average relative variance
//!   coefficient `CV_AVR% = 100·√((2/N)·Σ(a−b)²/(a+b)²)` over the `N` pairs with `a + b > 0` (plan §12.2); the
//!   reduced-major-axis slope of twin on original, `sign(r)·s_b/s_a`; and a paired t-test on `b − a`, with its
//!   two-sided p-value from the regularized incomplete beta function.
//! - **`G-TWIN` (G-CORE-1, E377).** A twin-derived correction may feed a `Full`-basis internal projection. It
//!   enters a `Public` or disclosure-basis estimate only when recorded [`TwinAcceptance`]s, which the correction
//!   cites, cover every pair it uses ([`admit_twin_correction`], `FMINE-REF-304` otherwise).
//! - **RC smearing (E391).** [`smearing_monitor`] runs a Mann–Kendall trend test down each mineralized interval
//!   of an RC hole. A significant decline is a [`SmearingState::Decline`]: its proposal is a diamond twin.

use std::collections::BTreeSet;

use fmine_core::det;
use fmine_ledger::l0::L0Ledger;

use crate::desurvey::{Direction, Point, Trace};
use crate::join::{Span, self_overlaps};
use crate::records::{
    Collar, DrillError, DrillRecord, HoleType, Interval, RecordKind, TwinPair, check_ids,
    validate_twin,
};
use crate::stats::{normal_cdf, t_two_sided_p};
use crate::store::{StoreError, current};
use crate::survey_error::UncertainTrace;

/// The default separation above which a pair is a [`TwinState::WeakTwin`], mm (plan §12.1).
pub const DEFAULT_SEPARATION_LIMIT_MM: u64 = 5_000;

/// How twins are judged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TwinPolicy {
    /// Separation above which a pair is weak, mm.
    pub separation_limit_mm: u64,
}

impl Default for TwinPolicy {
    fn default() -> Self {
        Self {
            separation_limit_mm: DEFAULT_SEPARATION_LIMIT_MM,
        }
    }
}

/// The grades of one matched pair, in one unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairGrade {
    /// The original hole's interval.
    pub original: f64,
    /// The twin's interval.
    pub twin: f64,
}

/// Whether a pair is close enough to count as a twin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TwinState {
    /// Within the separation limit.
    Twin,
    /// Above it: the pair is reported but does not count as a close twin.
    WeakTwin,
}

/// One matched interval pair, placed on the traces.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchedPair {
    /// The original hole's interval.
    pub original_interval_id: String,
    /// The twin's interval.
    pub twin_interval_id: String,
    /// Distance between the intervals' midpoints on the desurveyed traces, m.
    pub separation_m: f64,
    /// Either midpoint is above the first survey station or below the last one, where the trace is extended.
    pub extrapolated: bool,
    /// The grades.
    pub grade: PairGrade,
}

/// `CV_AVR%` over the matched grades.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Precision {
    /// `100·√((2/N)·Σ(a−b)²/(a+b)²)`.
    pub cv_avr_percent: f64,
    /// `N`: pairs with `a + b > 0`.
    pub pairs_used: usize,
    /// Pairs where both grades are zero, left out.
    pub both_zero: usize,
}

/// A paired t-test on the differences twin − original.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairedTest {
    /// Pairs.
    pub n: usize,
    /// Mean of twin − original.
    pub mean_difference: f64,
    /// Sample standard deviation of the differences (`n − 1` denominator).
    pub sd_difference: f64,
    /// `mean / (sd / √n)`; infinite when every difference is the same non-zero value.
    pub t_statistic: f64,
    /// `n − 1`.
    pub degrees_of_freedom: u32,
    /// Two-sided p-value.
    pub p_two_sided: f64,
}

/// What a twin pair shows.
#[derive(Clone, Debug, PartialEq)]
pub struct TwinAssessment {
    /// The pair.
    pub twin_id: String,
    /// Close twin or weak.
    pub state: TwinState,
    /// The largest midpoint separation, m.
    pub separation_m: f64,
    /// The policy limit, m.
    pub limit_m: f64,
    /// Each matched pair.
    pub pairs: Vec<MatchedPair>,
    /// `None` when every pair is zero on both sides.
    pub precision: Option<Precision>,
    /// Reduced-major-axis slope of twin on original; `None` with fewer than two pairs, or with either side
    /// constant, or uncorrelated.
    pub rma_slope: Option<f64>,
    /// `None` with fewer than two pairs.
    pub paired: Option<PairedTest>,
}

/// Assess `pair` from the two holes' traces, their interval records and the matched grades, one per match in
/// the record's order.
///
/// # Errors
/// [`DrillError::Invalid`] (`FMINE-REF-109`) for a pair the record checks refuse, traces of other holes or in
/// two CRSs, a missing interval or one of another hole, a grade count that is not the match count, or a grade
/// that is negative or not finite.
pub fn assess_twin(
    pair: &TwinPair,
    original: &Trace,
    twin: &Trace,
    intervals: &[Interval],
    grades: &[PairGrade],
    policy: &TwinPolicy,
) -> Result<TwinAssessment, DrillError> {
    validate_twin(pair)?;
    for (trace, hole, field) in [
        (original, &pair.original_hole_id, "twin.original_hole_id"),
        (twin, &pair.twin_hole_id, "twin.twin_hole_id"),
    ] {
        if trace.hole_id() != hole {
            return Err(DrillError::invalid(
                field,
                format!("trace of hole '{}' offered for '{hole}'", trace.hole_id()),
            ));
        }
    }
    if original.crs() != twin.crs() {
        return Err(DrillError::invalid(
            "twin.crs",
            format!(
                "traces in {} and {}; one CRS is required",
                original.crs(),
                twin.crs()
            ),
        ));
    }
    if grades.len() != pair.matches.len() {
        return Err(DrillError::invalid(
            "twin.grades",
            format!(
                "{} grades for {} matched intervals",
                grades.len(),
                pair.matches.len()
            ),
        ));
    }
    let interval = |id: &str, hole: &str| -> Result<&Interval, DrillError> {
        match intervals.iter().find(|i| i.interval_id == id) {
            Some(i) if i.hole_id == hole => Ok(i),
            Some(i) => Err(DrillError::invalid(
                "twin.matches",
                format!("interval '{id}' is in hole '{}', not '{hole}'", i.hole_id),
            )),
            None => Err(DrillError::invalid(
                "twin.matches",
                format!("interval '{id}' not offered"),
            )),
        }
    };
    let mut pairs = Vec::with_capacity(grades.len());
    for (m, grade) in pair.matches.iter().zip(grades) {
        for v in [grade.original, grade.twin] {
            if !v.is_finite() || v < 0.0 {
                return Err(DrillError::invalid(
                    "twin.grades",
                    format!(
                        "pair '{}'/'{}': grade {v}",
                        m.original_interval_id, m.twin_interval_id
                    ),
                ));
            }
        }
        let a = original.at(midpoint_mm(interval(
            &m.original_interval_id,
            &pair.original_hole_id,
        )?));
        let b = twin.at(midpoint_mm(interval(
            &m.twin_interval_id,
            &pair.twin_hole_id,
        )?));
        pairs.push(MatchedPair {
            original_interval_id: m.original_interval_id.clone(),
            twin_interval_id: m.twin_interval_id.clone(),
            separation_m: distance(a.point, b.point),
            extrapolated: a.extrapolated || b.extrapolated,
            grade: *grade,
        });
    }
    let separation_m = pairs.iter().map(|p| p.separation_m).fold(0.0, f64::max);
    let limit_m = mm_to_m(policy.separation_limit_mm);
    Ok(TwinAssessment {
        twin_id: pair.twin_id.clone(),
        state: if separation_m > limit_m {
            TwinState::WeakTwin
        } else {
            TwinState::Twin
        },
        separation_m,
        limit_m,
        precision: precision(grades),
        rma_slope: rma_slope(grades),
        paired: paired_test(grades),
        pairs,
    })
}

/// One matched pair's separation with its survey-error uncertainty.
#[derive(Clone, Debug, PartialEq)]
pub struct SeparationUncertainty {
    /// The original hole's interval.
    pub original_interval_id: String,
    /// The twin's interval.
    pub twin_interval_id: String,
    /// The separation, m, as assessed.
    pub separation_m: f64,
    /// One-sigma of the separation along the line between the midpoints, m: the two holes' errors are taken as
    /// independent. At zero separation, the root of the summed covariance's trace.
    pub sigma_m: f64,
    /// `Φ((separation − limit)/σ)`, the normal approximation to the probability that the true separation is
    /// beyond the policy limit; 0 or 1 when `σ` is zero.
    pub p_beyond_limit: f64,
}

/// The survey-error uncertainty of each matched pair's separation in `assessment` (`bd-zx57`), from the two
/// holes' traces with their error models.
///
/// # Errors
/// [`DrillError::Invalid`] (`FMINE-REF-109`) for a matched interval not offered or not in its trace's hole, or an
/// assessment that these traces do not reproduce: a pair's separation differs, so it was made from other surveys.
pub fn separation_uncertainty(
    assessment: &TwinAssessment,
    original: &UncertainTrace,
    twin: &UncertainTrace,
    intervals: &[Interval],
) -> Result<Vec<SeparationUncertainty>, DrillError> {
    let midpoint = |id: &str, trace: &UncertainTrace| -> Result<u64, DrillError> {
        let hole = trace.trace().hole_id();
        match intervals.iter().find(|i| i.interval_id == id) {
            Some(i) if i.hole_id == hole => Ok(midpoint_mm(i)),
            _ => Err(DrillError::invalid(
                "twin.matches",
                format!("interval '{id}' is not offered in hole '{hole}'"),
            )),
        }
    };
    let mut out = Vec::with_capacity(assessment.pairs.len());
    for pair in &assessment.pairs {
        let (da, db) = (
            midpoint(&pair.original_interval_id, original)?,
            midpoint(&pair.twin_interval_id, twin)?,
        );
        let (a, b) = (original.trace().at(da).point, twin.trace().at(db).point);
        let separation = distance(a, b);
        if separation.to_bits() != pair.separation_m.to_bits() {
            return Err(DrillError::invalid(
                "twin.separation",
                format!(
                    "pair '{}'/'{}' is {separation} m apart on these traces, {} m as assessed",
                    pair.original_interval_id, pair.twin_interval_id, pair.separation_m
                ),
            ));
        }
        let (ca, cb) = (
            original.covariance_at(da).total,
            twin.covariance_at(db).total,
        );
        let variance = if separation > 0.0 {
            let u = Direction {
                east: (b.easting_m - a.easting_m) / separation,
                north: (b.northing_m - a.northing_m) / separation,
                up: (b.elevation_m - a.elevation_m) / separation,
            };
            ca.variance_along(u) + cb.variance_along(u)
        } else {
            (0..3).map(|i| ca.enu_m2[i][i] + cb.enu_m2[i][i]).sum()
        };
        let sigma = det::sqrt(variance.max(0.0));
        let beyond = separation - assessment.limit_m;
        out.push(SeparationUncertainty {
            original_interval_id: pair.original_interval_id.clone(),
            twin_interval_id: pair.twin_interval_id.clone(),
            separation_m: separation,
            sigma_m: sigma,
            p_beyond_limit: if sigma > 0.0 {
                normal_cdf(beyond / sigma)
            } else if beyond > 0.0 {
                1.0
            } else {
                0.0
            },
        });
    }
    Ok(out)
}

const fn midpoint_mm(interval: &Interval) -> u64 {
    interval.from_mm + (interval.to_mm - interval.from_mm) / 2
}

#[allow(clippy::cast_precision_loss)]
fn mm_to_m(mm: u64) -> f64 {
    mm as f64 / 1_000.0
}

fn distance(a: Point, b: Point) -> f64 {
    det::hypot(
        det::hypot(a.easting_m - b.easting_m, a.northing_m - b.northing_m),
        a.elevation_m - b.elevation_m,
    )
}

#[allow(clippy::cast_precision_loss)]
fn precision(grades: &[PairGrade]) -> Option<Precision> {
    let mut sum = 0.0;
    let mut used = 0_usize;
    for g in grades {
        let total = g.original + g.twin;
        if total > 0.0 {
            let d = (g.original - g.twin) / total;
            sum += d * d;
            used += 1;
        }
    }
    (used > 0).then(|| Precision {
        cv_avr_percent: 100.0 * det::sqrt(2.0 * sum / used as f64),
        pairs_used: used,
        both_zero: grades.len() - used,
    })
}

#[allow(clippy::cast_precision_loss)]
fn rma_slope(grades: &[PairGrade]) -> Option<f64> {
    if grades.len() < 2 {
        return None;
    }
    let n = grades.len() as f64;
    let mean_a = grades.iter().map(|g| g.original).sum::<f64>() / n;
    let mean_b = grades.iter().map(|g| g.twin).sum::<f64>() / n;
    let (mut saa, mut sbb, mut sab) = (0.0, 0.0, 0.0);
    for g in grades {
        let (da, db) = (g.original - mean_a, g.twin - mean_b);
        saa += da * da;
        sbb += db * db;
        sab += da * db;
    }
    (saa > 0.0 && sbb > 0.0 && sab != 0.0).then(|| det::sqrt(sbb / saa).copysign(sab))
}

#[allow(clippy::cast_precision_loss)]
fn paired_test(grades: &[PairGrade]) -> Option<PairedTest> {
    let n = grades.len();
    let df = u32::try_from(n.checked_sub(1)?).ok().filter(|&df| df > 0)?;
    let nf = n as f64;
    let mean = grades.iter().map(|g| g.twin - g.original).sum::<f64>() / nf;
    let ss: f64 = grades
        .iter()
        .map(|g| {
            let e = g.twin - g.original - mean;
            e * e
        })
        .sum();
    let sd = det::sqrt(ss / f64::from(df));
    let (t, p) = if sd > 0.0 {
        let t = mean / (sd / det::sqrt(nf));
        (t, t_two_sided_p(t, df))
    } else if mean == 0.0 {
        (0.0, 1.0)
    } else {
        (f64::INFINITY.copysign(mean), 0.0)
    };
    Some(PairedTest {
        n,
        mean_difference: mean,
        sd_difference: sd,
        t_statistic: t,
        degrees_of_freedom: df,
        p_two_sided: p,
    })
}

/// The estimate a twin-derived correction is offered to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstimateBasis {
    /// An internal projection on all the data, carrying the correction's uncertainty.
    Full,
    /// A public-lane estimate.
    Public,
    /// An estimate prepared for disclosure, such as a reported resource.
    Disclosure,
}

/// A twin-derived correction offered to an estimate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TwinCorrection<'a> {
    /// Where it would go.
    pub basis: EstimateBasis,
    /// The twin pairs it is derived from, ascending.
    pub twin_ids: &'a [String],
    /// The recorded acceptances it cites, ascending; none are needed on a `Full` basis.
    pub acceptance_ids: &'a [String],
}

/// Why a correction was admitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Admission {
    /// On a `Full` basis, where no acceptance is needed.
    Internal,
    /// On a public or disclosure basis, under these acceptances.
    Accepted {
        /// The cited acceptances.
        acceptance_ids: Vec<String>,
    },
}

/// The `G-TWIN` guard: admit `correction` only where it may go, reading its pairs and acceptances from the drill
/// cell `cell_id`.
///
/// # Errors
/// [`StoreError::Record`] with [`DrillError::Invalid`] (`FMINE-REF-109`) for malformed or unrecorded pairs, or
/// with [`DrillError::MissingAcceptance`] (`FMINE-REF-304`) on a public or disclosure basis when a cited
/// acceptance is not recorded or the acceptances leave a pair uncovered; [`StoreError::Ledger`] for an unknown
/// cell.
pub fn admit_twin_correction(
    ledger: &L0Ledger,
    cell_id: &str,
    correction: &TwinCorrection<'_>,
) -> Result<Admission, StoreError> {
    check_ids("correction.twin_ids", correction.twin_ids)?;
    for id in correction.twin_ids {
        if current(ledger, cell_id, RecordKind::TwinPair, id)?.is_none() {
            return Err(DrillError::invalid(
                "correction.twin_ids",
                format!("twin '{id}' is not recorded"),
            )
            .into());
        }
    }
    if correction.basis == EstimateBasis::Full {
        return Ok(Admission::Internal);
    }
    let basis = match correction.basis {
        EstimateBasis::Public => "public",
        EstimateBasis::Disclosure | EstimateBasis::Full => "disclosure-basis",
    };
    let missing =
        |detail: String| -> StoreError { DrillError::MissingAcceptance { detail }.into() };
    if correction.acceptance_ids.is_empty() {
        return Err(missing(format!(
            "a {basis} estimate takes a twin-derived correction only under a recorded QP acceptance; none is cited"
        )));
    }
    check_ids("correction.acceptance_ids", correction.acceptance_ids)?;
    let mut covered: BTreeSet<&str> = BTreeSet::new();
    let mut accepted = Vec::new();
    for id in correction.acceptance_ids {
        match current(ledger, cell_id, RecordKind::TwinAcceptance, id)? {
            Some(v) => {
                if let DrillRecord::TwinAcceptance(a) = v.record {
                    accepted.push(a);
                }
            }
            None => return Err(missing(format!("acceptance '{id}' is not recorded"))),
        }
    }
    for a in &accepted {
        covered.extend(a.twin_ids.iter().map(String::as_str));
    }
    let uncovered: Vec<&str> = correction
        .twin_ids
        .iter()
        .map(String::as_str)
        .filter(|id| !covered.contains(id))
        .collect();
    if !uncovered.is_empty() {
        return Err(missing(format!(
            "a {basis} estimate takes a twin-derived correction only under a recorded QP acceptance of each pair; \
             not accepted: {}",
            uncovered.join(", ")
        )));
    }
    Ok(Admission::Accepted {
        acceptance_ids: correction.acceptance_ids.to_vec(),
    })
}

/// How the smearing monitor judges a decline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmearingPolicy {
    /// One-sided significance level, in (0, 0.5].
    pub alpha: f64,
    /// Fewest samples in a mineralized interval for a test; at least 3.
    pub min_samples: usize,
}

impl Default for SmearingPolicy {
    fn default() -> Self {
        Self {
            alpha: 0.05,
            min_samples: 10,
        }
    }
}

/// One sample's span and grade down a hole.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DownholeGrade {
    /// The sample's span.
    pub span: Span,
    /// Its grade.
    pub value: f64,
}

/// What one mineralized interval shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmearingState {
    /// Fewer samples inside it than the policy's minimum: not tested.
    TooFewSamples,
    /// No significant down-hole decline.
    NoDecline,
    /// A significant down-hole decline: propose a diamond twin.
    Decline,
}

/// The test on one mineralized interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmearingCheck {
    /// The mineralized interval.
    pub domain: Span,
    /// Samples lying wholly inside it.
    pub n: usize,
    /// Mann–Kendall `S = Σ_{i<j} sign(x_j − x_i)`, samples in depth order; negative for a decline.
    pub kendall_s: i64,
    /// The continuity-corrected normal score, with the tie-corrected variance; `None` when not tested.
    pub z: Option<f64>,
    /// One-sided p-value for a decline, `Φ(z)`; `None` when not tested.
    pub p_decline: Option<f64>,
    /// The outcome.
    pub state: SmearingState,
}

/// The smearing monitor's result for one hole.
#[derive(Clone, Debug, PartialEq)]
pub struct SmearingReport {
    /// The hole.
    pub hole_id: String,
    /// One check per mineralized interval, in depth order.
    pub checks: Vec<SmearingCheck>,
}

impl SmearingReport {
    /// The intervals where a diamond twin is proposed.
    pub fn proposals(&self) -> impl Iterator<Item = &SmearingCheck> {
        self.checks
            .iter()
            .filter(|c| c.state == SmearingState::Decline)
    }
}

/// Test each mineralized interval of RC hole `collar` for a down-hole grade decline (E391). Only samples lying
/// wholly inside an interval are tested.
///
/// # Errors
/// [`DrillError::Invalid`] (`FMINE-REF-109`) for a hole that is not RC, a policy out of range, overlapping
/// samples or mineralized intervals, an empty span, or a grade that is negative or not finite.
pub fn smearing_monitor(
    collar: &Collar,
    mineralized: &[Span],
    grades: &[DownholeGrade],
    policy: &SmearingPolicy,
) -> Result<SmearingReport, DrillError> {
    if collar.hole_type != HoleType::ReverseCirculation {
        return Err(DrillError::invalid(
            "smearing.hole_type",
            format!(
                "the smearing monitor checks RC holes; '{}' is {}",
                collar.hole_id,
                collar.hole_type.code()
            ),
        ));
    }
    if !(policy.alpha > 0.0 && policy.alpha <= 0.5) || policy.min_samples < 3 {
        return Err(DrillError::invalid(
            "smearing.policy",
            format!(
                "alpha {} must be in (0, 0.5] and min_samples {} at least 3",
                policy.alpha, policy.min_samples
            ),
        ));
    }
    if let Some(g) = grades
        .iter()
        .find(|g| !g.value.is_finite() || g.value < 0.0)
    {
        return Err(DrillError::invalid(
            "smearing.grades",
            format!("grade {}", g.value),
        ));
    }
    let sample_spans: Vec<Span> = grades.iter().map(|g| g.span).collect();
    for (what, spans) in [
        ("smearing.grades", &sample_spans[..]),
        ("smearing.mineralized", mineralized),
    ] {
        if let Some(o) = self_overlaps(spans)?.first() {
            return Err(DrillError::invalid(
                what,
                format!(
                    "spans {}..{} and {}..{} mm overlap",
                    spans[o.left].from_mm,
                    spans[o.left].to_mm,
                    spans[o.right].from_mm,
                    spans[o.right].to_mm
                ),
            ));
        }
    }
    let mut ordered: Vec<&DownholeGrade> = grades.iter().collect();
    ordered.sort_by_key(|g| g.span.from_mm);
    let mut domains = mineralized.to_vec();
    domains.sort_by_key(|d| d.from_mm);
    let checks = domains
        .into_iter()
        .map(|domain| {
            let values: Vec<f64> = ordered
                .iter()
                .filter(|g| g.span.from_mm >= domain.from_mm && g.span.to_mm <= domain.to_mm)
                .map(|g| g.value)
                .collect();
            mann_kendall(domain, &values, policy)
        })
        .collect();
    Ok(SmearingReport {
        hole_id: collar.hole_id.clone(),
        checks,
    })
}

#[allow(clippy::cast_precision_loss)]
fn mann_kendall(domain: Span, values: &[f64], policy: &SmearingPolicy) -> SmearingCheck {
    let n = values.len();
    let mut s: i64 = 0;
    for (i, a) in values.iter().enumerate() {
        for b in &values[i + 1..] {
            s += match b.partial_cmp(a) {
                Some(std::cmp::Ordering::Greater) => 1,
                Some(std::cmp::Ordering::Less) => -1,
                _ => 0,
            };
        }
    }
    if n < policy.min_samples {
        return SmearingCheck {
            domain,
            n,
            kendall_s: s,
            z: None,
            p_decline: None,
            state: SmearingState::TooFewSamples,
        };
    }
    let term = |t: i128| t * (t - 1) * (2 * t + 5);
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let ties: i128 = sorted
        .chunk_by(|a, b| a == b)
        .map(|run| term(run.len() as i128))
        .sum();
    let variance = (term(n as i128) - ties) as f64 / 18.0;
    let z = if variance <= 0.0 || s == 0 {
        0.0
    } else {
        let shifted = if s > 0 { s - 1 } else { s + 1 };
        shifted as f64 / det::sqrt(variance)
    };
    let p = normal_cdf(z);
    SmearingCheck {
        domain,
        n,
        kendall_s: s,
        z: Some(z),
        p_decline: Some(p),
        state: if p < policy.alpha {
            SmearingState::Decline
        } else {
            SmearingState::NoDecline
        },
    }
}
