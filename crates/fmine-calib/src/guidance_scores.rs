//! Descriptive issuer-guidance scores (BD-341.S4), with exact printed-value and rounding calculations.
//!
//! DESIGN: ranges have no implied coverage. Missing members stay in the denominator. Completion, rounding
//! and IID reference bounds are separate. Callers bind compatible cited outcomes and select their vintage.
//! Determinism: Exact for scores and completion bounds; DeterministicPerIsa for the explicitly IID-only
//! Clopper-Pearson reference interval, evaluated by a bounded recurrence and the refusing root solver.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;

use fmine_numerics::brent_root;
pub use fmine_numerics::exact_feas::{Interval, Rat};

use crate::CalibrationFlag;

/// Canonical Claim validation and provenance around the exact numerical primitives.
pub mod claims;

const ZERO: Rat = Rat::from_integer(0);
const TWO: Rat = Rat::from_integer(2);
const MAX_BINOMIAL_N: usize = 4096;

pub const CP_ASSUMPTION: &str = "Independent Bernoulli outcomes among resolved members only; no adjustment for site or year dependence, selective missingness, or outcome rounding.";
/// The issuer's stated acceptance set; no variant asserts a nominal probability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guidance {
    Range { lo: Rat, hi: Rat },
    Point { value: Rat },
    AtMost { upper: Rat },
    AtLeast { lower: Rat },
}

/// A printed outcome and the values compatible with its declared rounding convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrintedOutcome {
    pub value: Rat,
    pub interval: Interval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Below,
    Inside,
    Above,
}

impl Side {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Below => "below",
            Self::Inside => "inside",
            Self::Above => "above",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuidanceScore {
    /// Classification of the printed value, without replacing it with an implied value.
    pub side: Side,
    /// Some, but not all, values in the outcome's rounding interval meet guidance.
    pub boundary: bool,
    pub definitely_inside: bool,
    pub possibly_inside: bool,
    /// Range-only quantities; point and one-sided guidance have no standardized bias.
    pub midpoint: Option<Rat>,
    pub half_width: Option<Rat>,
    pub z: Option<Rat>,
    pub z_interval: Option<Interval>,
    pub above_midpoint: Option<bool>,
}

/// One member of a caller-declared cohort. The adapter retains the reason for each absent outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoringMember {
    pub id: String,
    pub guidance: Guidance,
    pub outcome: Option<PrintedOutcome>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuidanceSummary {
    pub n_members: usize,
    pub n_resolved: usize,
    pub n_unresolved: usize,
    pub inside: usize,
    pub below: usize,
    pub above: usize,
    pub boundary: usize,
    pub definitely_inside: usize,
    pub possibly_inside: usize,
    /// Resolved, non-degenerate range rows only; this denominator is printed beside every bias quantity.
    pub n_ranges: usize,
    pub above_midpoint: usize,
    pub observed_hit_rate: Option<Rat>,
    /// IID Bernoulli reference only; not adjusted for dependence and never a calibration verdict.
    pub iid_reference_interval_90: Option<(f64, f64)>,
    /// [printed hits / all members, (printed hits + missing) / all members]. No sampling inference.
    pub whole_cohort_hit_bounds: Option<Interval>,
    /// The same completion bound with definite and possible hits under outcome rounding.
    pub rounding_aware_hit_bounds: Option<Interval>,
    pub above_midpoint_share: Option<Rat>,
    pub mean_z: Option<Rat>,
    /// Arithmetic propagation of printed rounding only, never a confidence interval for bias.
    pub mean_z_rounding_interval: Option<Interval>,
    /// This module returns only `Descriptive` or `Unscored`, at every sample size.
    pub flag: CalibrationFlag,
}

fn rat_json(value: Rat) -> String {
    format!(
        "{{\"num\":\"{}\",\"den\":\"{}\"}}",
        value.num(),
        value.den()
    )
}

fn optional_rat(value: Option<Rat>) -> String {
    value.map_or_else(|| "null".to_owned(), rat_json)
}

fn interval_json(value: Option<Interval>) -> String {
    value.map_or_else(
        || "null".to_owned(),
        |i| {
            format!(
                "{{\"lo\":{},\"hi\":{},\"lo_strict\":{},\"hi_strict\":{}}}",
                rat_json(i.lo),
                rat_json(i.hi),
                i.lo_strict,
                i.hi_strict
            )
        },
    )
}

impl GuidanceScore {
    /// Deterministic numeric rendering; the caller supplies the source citations and arithmetic rule.
    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"side\":\"{}\",\"boundary\":{},\"definitely_inside\":{},\"possibly_inside\":{},\
             \"midpoint\":{},\"half_width\":{},\"z\":{},\"z_interval\":{},\"above_midpoint\":{}}}",
            self.side.as_str(),
            self.boundary,
            self.definitely_inside,
            self.possibly_inside,
            optional_rat(self.midpoint),
            optional_rat(self.half_width),
            optional_rat(self.z),
            interval_json(self.z_interval),
            self.above_midpoint
                .map_or_else(|| "null".to_owned(), |x| x.to_string()),
        )
    }
}

impl GuidanceSummary {
    /// Numeric JSON; dependence estimates and bias confidence intervals remain explicitly absent.
    #[must_use]
    pub fn to_json(&self) -> String {
        let reference = self.iid_reference_interval_90.map_or_else(|| "null".to_owned(), |(lo, hi)| {
            format!("{{\"lo\":{lo},\"hi\":{hi},\"level\":0.9,\"assumption\":\"independent_identically_distributed_bernoulli_hits\"}}")
        });
        let flag = if self.n_resolved == 0 {
            "unscored"
        } else {
            "descriptive"
        };
        format!(
            "{{\"n_members\":{},\"n_resolved\":{},\"n_unresolved\":{},\"inside\":{},\"below\":{},\
             \"above\":{},\"boundary\":{},\"definitely_inside\":{},\"possibly_inside\":{},\
             \"n_ranges\":{},\"above_midpoint\":{},\"observed_hit_rate\":{},\
             \"iid_reference_interval_90\":{},\"whole_cohort_hit_bounds\":{},\
             \"rounding_aware_hit_bounds\":{},\"above_midpoint_share\":{},\"mean_z\":{},\
             \"mean_z_rounding_interval\":{},\"effective_n\":null,\"bias_confidence_interval\":null,\
             \"flag\":\"{flag}\",\"calibration_verdict\":null}}",
            self.n_members,
            self.n_resolved,
            self.n_unresolved,
            self.inside,
            self.below,
            self.above,
            self.boundary,
            self.definitely_inside,
            self.possibly_inside,
            self.n_ranges,
            self.above_midpoint,
            optional_rat(self.observed_hit_rate),
            reference,
            interval_json(self.whole_cohort_hit_bounds),
            interval_json(self.rounding_aware_hit_bounds),
            optional_rat(self.above_midpoint_share),
            optional_rat(self.mean_z),
            interval_json(self.mean_z_rounding_interval),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuidanceScoreError {
    InvalidRange,
    InvalidOutcomeInterval,
    PrintedOutcomeOutsideInterval,
    InvalidMemberId,
    DuplicateMemberId,
    ArithmeticOverflow,
    ReferenceIntervalFailed,
}

impl fmt::Display for GuidanceScoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidRange => "guidance range needs lo < hi; use Point for a single value",
            Self::InvalidOutcomeInterval => "outcome rounding interval is empty or reversed",
            Self::PrintedOutcomeOutsideInterval => {
                "printed outcome is outside its rounding interval"
            }
            Self::InvalidMemberId => "guidance cohort member id is empty",
            Self::DuplicateMemberId => "guidance cohort has a duplicate member id",
            Self::ArithmeticOverflow => "guidance score exact arithmetic overflowed",
            Self::ReferenceIntervalFailed => "guidance IID reference interval calculation failed",
        })
    }
}

impl std::error::Error for GuidanceScoreError {}

type Result<T> = std::result::Result<T, GuidanceScoreError>;

fn exact<T, E>(result: std::result::Result<T, E>) -> Result<T> {
    result.map_err(|_| GuidanceScoreError::ArithmeticOverflow)
}

fn cmp(a: Rat, b: Rat) -> Result<Ordering> {
    exact(a.checked_cmp(b))
}

fn validate_guidance(guidance: Guidance) -> Result<()> {
    if let Guidance::Range { lo, hi } = guidance
        && cmp(lo, hi)? != Ordering::Less
    {
        return Err(GuidanceScoreError::InvalidRange);
    }
    Ok(())
}

/// Score one outcome against the stated guidance set, retaining its printed rounding ambiguity.
pub fn score(guidance: Guidance, outcome: PrintedOutcome) -> Result<GuidanceScore> {
    validate_guidance(guidance)?;
    let interval = outcome.interval;
    match cmp(interval.lo, interval.hi)? {
        Ordering::Greater => return Err(GuidanceScoreError::InvalidOutcomeInterval),
        Ordering::Equal if interval.lo_strict || interval.hi_strict => {
            return Err(GuidanceScoreError::InvalidOutcomeInterval);
        }
        _ => {}
    }
    if !exact(interval.contains(outcome.value))? {
        return Err(GuidanceScoreError::PrintedOutcomeOutsideInterval);
    }
    let (lower, upper) = match guidance {
        Guidance::Range { lo, hi } => (Some(lo), Some(hi)),
        Guidance::Point { value } => (Some(value), Some(value)),
        Guidance::AtMost { upper } => (None, Some(upper)),
        Guidance::AtLeast { lower } => (Some(lower), None),
    };
    let mut side = Side::Inside;
    let (mut definitely_inside, mut possibly_inside) = (true, true);
    if let Some(lo) = lower {
        if cmp(outcome.value, lo)? == Ordering::Less {
            side = Side::Below;
        }
        definitely_inside &= cmp(interval.lo, lo)? != Ordering::Less;
        let high = cmp(interval.hi, lo)?;
        possibly_inside &=
            high == Ordering::Greater || (high == Ordering::Equal && !interval.hi_strict);
    }
    if let Some(hi) = upper {
        if cmp(outcome.value, hi)? == Ordering::Greater {
            side = Side::Above;
        }
        definitely_inside &= cmp(interval.hi, hi)? != Ordering::Greater;
        let low = cmp(interval.lo, hi)?;
        possibly_inside &= low == Ordering::Less || (low == Ordering::Equal && !interval.lo_strict);
    }
    let mut result = GuidanceScore {
        side,
        boundary: possibly_inside && !definitely_inside,
        definitely_inside,
        possibly_inside,
        midpoint: None,
        half_width: None,
        z: None,
        z_interval: None,
        above_midpoint: None,
    };
    if let Guidance::Range { lo, hi } = guidance {
        let half = exact(exact(hi.checked_sub(lo))?.checked_div(TWO))?;
        let midpoint = exact(lo.checked_add(half))?;
        let standardized =
            |value: Rat| exact(exact(value.checked_sub(midpoint))?.checked_div(half));
        result.midpoint = Some(midpoint);
        result.half_width = Some(half);
        result.z = Some(standardized(outcome.value)?);
        result.z_interval = Some(Interval {
            lo: standardized(interval.lo)?,
            hi: standardized(interval.hi)?,
            lo_strict: interval.lo_strict,
            hi_strict: interval.hi_strict,
        });
        result.above_midpoint = Some(cmp(outcome.value, midpoint)? == Ordering::Greater);
    }
    Ok(result)
}

/// Binomial tail from a recurrence centred at a mode. Relative weights are at most one; normalizing their
/// sum avoids factorial overflow and a vanishing endpoint seed. This evaluates the binomial polynomial with
/// fixed-order floating arithmetic, with no platform transcendental functions.
fn binomial_tail(n: usize, k: usize, p: f64, upper: bool) -> f64 {
    if p == 0.0 {
        return if !upper || k == 0 { 1.0 } else { 0.0 };
    }
    if p == 1.0 {
        return if upper || k == n { 1.0 } else { 0.0 };
    }
    let mode = (((n + 1) as f64 * p) as usize).min(n);
    let selected = |i: usize| if upper { i >= k } else { i <= k };
    let (mut total, mut tail) = (1.0, if selected(mode) { 1.0 } else { 0.0 });
    let mut weight = 1.0;
    for i in (1..=mode).rev() {
        weight *= i as f64 / (n - i + 1) as f64 * ((1.0 - p) / p);
        total += weight;
        if selected(i - 1) {
            tail += weight;
        }
    }
    weight = 1.0;
    for i in mode..n {
        weight *= (n - i) as f64 / (i + 1) as f64 * (p / (1.0 - p));
        total += weight;
        if selected(i + 1) {
            tail += weight;
        }
    }
    tail / total
}

fn binomial_interval(k: usize, n: usize, max_iter: usize) -> Result<(f64, f64)> {
    if n == 0 || k > n || n > MAX_BINOMIAL_N {
        return Err(GuidanceScoreError::ReferenceIntervalFailed);
    }
    let solve = |upper: bool| {
        brent_root(0.0, 1.0, 1e-13, max_iter, |p| {
            binomial_tail(n, k, p, upper) - 0.05
        })
        .map(|root| root.x)
        .map_err(|_| GuidanceScoreError::ReferenceIntervalFailed)
    };
    let lo = if k == 0 { 0.0 } else { solve(true)? };
    let hi = if k == n { 1.0 } else { solve(false)? };
    if !lo.is_finite() || !hi.is_finite() || lo < 0.0 || hi > 1.0 || lo > hi {
        return Err(GuidanceScoreError::ReferenceIntervalFailed);
    }
    Ok((lo, hi))
}

/// A two-sided 90% Clopper-Pearson interval. The numerical roots must converge; zero outcomes, invalid counts,
/// or a sample above the fixed computation budget refuse instead of returning an estimate.
pub fn clopper_pearson_90(k: usize, n: usize) -> Result<(f64, f64)> {
    binomial_interval(k, n, 200)
}

fn count_ratio(numerator: usize, denominator: usize) -> Result<Rat> {
    let numerator =
        i128::try_from(numerator).map_err(|_| GuidanceScoreError::ArithmeticOverflow)?;
    let denominator =
        i128::try_from(denominator).map_err(|_| GuidanceScoreError::ArithmeticOverflow)?;
    exact(Rat::new(numerator, denominator))
}

/// Aggregate a declared cohort in stable id order, keeping missing members. Callers group identical metric,
/// ownership, period and outcome-selector semantics; none are inferred from the numbers.
pub fn summarize(members: &[ScoringMember]) -> Result<GuidanceSummary> {
    let mut ids = BTreeSet::new();
    let mut ordered = Vec::with_capacity(members.len());
    for member in members {
        if member.id.trim().is_empty() {
            return Err(GuidanceScoreError::InvalidMemberId);
        }
        if !ids.insert(&member.id) {
            return Err(GuidanceScoreError::DuplicateMemberId);
        }
        validate_guidance(member.guidance)?;
        ordered.push(member);
    }
    ordered.sort_by(|a, b| a.id.cmp(&b.id));
    let mut summary = GuidanceSummary {
        n_members: members.len(),
        n_resolved: 0,
        n_unresolved: 0,
        inside: 0,
        below: 0,
        above: 0,
        boundary: 0,
        definitely_inside: 0,
        possibly_inside: 0,
        n_ranges: 0,
        above_midpoint: 0,
        observed_hit_rate: None,
        iid_reference_interval_90: None,
        whole_cohort_hit_bounds: None,
        rounding_aware_hit_bounds: None,
        above_midpoint_share: None,
        mean_z: None,
        mean_z_rounding_interval: None,
        flag: CalibrationFlag::Unscored,
    };
    let (mut sum_z, mut sum_lo, mut sum_hi) = (ZERO, ZERO, ZERO);
    let (mut lo_strict, mut hi_strict) = (false, false);
    for member in ordered {
        let Some(outcome) = member.outcome else {
            summary.n_unresolved += 1;
            continue;
        };
        let scored = score(member.guidance, outcome)?;
        summary.n_resolved += 1;
        match scored.side {
            Side::Below => summary.below += 1,
            Side::Inside => summary.inside += 1,
            Side::Above => summary.above += 1,
        }
        summary.boundary += usize::from(scored.boundary);
        summary.definitely_inside += usize::from(scored.definitely_inside);
        summary.possibly_inside += usize::from(scored.possibly_inside);
        if let (Some(z), Some(interval)) = (scored.z, scored.z_interval) {
            summary.n_ranges += 1;
            summary.above_midpoint += usize::from(scored.above_midpoint == Some(true));
            sum_z = exact(sum_z.checked_add(z))?;
            sum_lo = exact(sum_lo.checked_add(interval.lo))?;
            sum_hi = exact(sum_hi.checked_add(interval.hi))?;
            lo_strict |= interval.lo_strict;
            hi_strict |= interval.hi_strict;
        }
    }
    if summary.n_members > 0 {
        let bounds = |lo, hi| -> Result<Interval> {
            Ok(Interval {
                lo: count_ratio(lo, summary.n_members)?,
                hi: count_ratio(hi, summary.n_members)?,
                lo_strict: false,
                hi_strict: false,
            })
        };
        summary.whole_cohort_hit_bounds = Some(bounds(
            summary.inside,
            summary.inside + summary.n_unresolved,
        )?);
        summary.rounding_aware_hit_bounds = Some(bounds(
            summary.definitely_inside,
            summary.possibly_inside + summary.n_unresolved,
        )?);
    }
    if summary.n_resolved > 0 {
        summary.flag = CalibrationFlag::Descriptive;
        summary.observed_hit_rate = Some(count_ratio(summary.inside, summary.n_resolved)?);
        summary.iid_reference_interval_90 =
            Some(clopper_pearson_90(summary.inside, summary.n_resolved)?);
    }
    if summary.n_ranges > 0 {
        let n = count_ratio(summary.n_ranges, 1)?;
        summary.mean_z = Some(exact(sum_z.checked_div(n))?);
        summary.mean_z_rounding_interval = Some(Interval {
            lo: exact(sum_lo.checked_div(n))?,
            hi: exact(sum_hi.checked_div(n))?,
            lo_strict,
            hi_strict,
        });
        summary.above_midpoint_share = Some(count_ratio(summary.above_midpoint, summary.n_ranges)?);
    }
    Ok(summary)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn q(num: i128, den: i128) -> Rat {
        Rat::new(num, den).unwrap()
    }

    fn printed(value: i128, half_step: Rat) -> PrintedOutcome {
        let value = Rat::from_integer(value);
        PrintedOutcome {
            value,
            interval: Interval {
                lo: value.checked_sub(half_step).unwrap(),
                hi: value.checked_add(half_step).unwrap(),
                lo_strict: false,
                hi_strict: false,
            },
        }
    }

    fn range() -> Guidance {
        Guidance::Range {
            lo: q(100, 1),
            hi: q(120, 1),
        }
    }

    fn member(id: &str, outcome: Option<i128>) -> ScoringMember {
        ScoringMember {
            id: id.to_owned(),
            guidance: range(),
            outcome: outcome.map(|value| printed(value, q(1, 2))),
        }
    }

    #[test]
    fn test_range_score_known_answer_z_three_halves() {
        let score = score(range(), printed(125, q(1, 2))).unwrap();
        assert_eq!(score.midpoint, Some(q(110, 1)));
        assert_eq!(score.half_width, Some(q(10, 1)));
        assert_eq!(score.z, Some(q(3, 2)));
        assert_eq!(score.side, Side::Above);
        assert!(!score.boundary);
        assert!(!score.possibly_inside);
        assert_eq!(score.z_interval.unwrap().lo, q(29, 20));
        assert_eq!(score.z_interval.unwrap().hi, q(31, 20));
    }

    #[test]
    fn test_point_and_bound_rows_have_no_z() {
        for (guidance, expected) in [
            (Guidance::Point { value: q(110, 1) }, Side::Inside),
            (Guidance::AtMost { upper: q(100, 1) }, Side::Above),
            (Guidance::AtLeast { lower: q(120, 1) }, Side::Below),
        ] {
            let score = score(guidance, printed(110, ZERO)).unwrap();
            assert_eq!(score.side, expected);
            assert_eq!(score.z, None);
            assert_eq!(score.z_interval, None);
            assert_eq!(score.midpoint, None);
        }
    }

    #[test]
    fn test_boundary_rounding_and_open_endpoint() {
        let s = score(range(), printed(120, q(1, 2))).unwrap();
        assert_eq!(s.side, Side::Inside);
        assert!(s.boundary && s.possibly_inside && !s.definitely_inside);
        let mut outside = printed(121, q(1, 1));
        assert!(score(range(), outside).unwrap().boundary);
        outside.interval.lo_strict = true;
        let s = score(range(), outside).unwrap();
        assert!(!s.boundary && !s.possibly_inside);
        let point = Guidance::Point { value: q(120, 1) };
        assert!(score(point, printed(120, q(1, 2))).unwrap().boundary);
        assert!(score(point, printed(120, ZERO)).unwrap().definitely_inside);
    }

    #[test]
    fn test_scores_invariant_under_exact_unit_change() {
        let original = score(range(), printed(125, q(1, 2))).unwrap();
        let factor = q(311_034_768, 10_000_000); // Exact statutory g per troy oz.
        let scale = |value: Rat| value.checked_mul(factor).unwrap();
        let outcome = printed(125, q(1, 2));
        let scaled = score(
            Guidance::Range {
                lo: scale(q(100, 1)),
                hi: scale(q(120, 1)),
            },
            PrintedOutcome {
                value: scale(outcome.value),
                interval: Interval {
                    lo: scale(outcome.interval.lo),
                    hi: scale(outcome.interval.hi),
                    ..outcome.interval
                },
            },
        )
        .unwrap();
        assert_eq!(scaled.z, original.z);
        assert_eq!(scaled.z_interval, original.z_interval);
        assert_eq!(scaled.side, original.side);
        assert_eq!(scaled.boundary, original.boundary);
    }

    #[test]
    fn test_withholding_outcome_keeps_n_and_bounds_widen() {
        let members = vec![
            member("a", Some(110)),
            member("b", Some(125)),
            member("c", None),
        ];
        let all = summarize(&members).unwrap();
        assert_eq!((all.n_members, all.n_resolved, all.n_unresolved), (3, 2, 1));
        assert_eq!(all.observed_hit_rate, Some(q(1, 2)));
        let bound = all.whole_cohort_hit_bounds.unwrap();
        assert_eq!((bound.lo, bound.hi), (q(1, 3), q(2, 3)));
        let mut withheld = members.clone();
        withheld[1].outcome = None;
        let after = summarize(&withheld).unwrap();
        assert_eq!(after.n_members, all.n_members);
        assert_eq!(after.n_unresolved, 2);
        assert_eq!(after.whole_cohort_hit_bounds.unwrap().hi, q(1, 1));
        let mut reversed = members;
        reversed.reverse();
        assert_eq!(summarize(&reversed).unwrap(), all);
    }

    #[test]
    fn test_rounding_completion_bounds_and_mean_bias_have_named_denominators() {
        let rows = [
            member("a", Some(120)),
            member("b", Some(125)),
            member("c", None),
        ];
        let s = summarize(&rows).unwrap();
        assert_eq!(s.boundary, 1);
        assert_eq!(s.n_ranges, 2);
        assert_eq!(s.mean_z, Some(q(5, 4)));
        assert_eq!(s.above_midpoint_share, Some(q(1, 1)));
        let bounds = s.rounding_aware_hit_bounds.unwrap();
        assert_eq!((bounds.lo, bounds.hi), (ZERO, q(2, 3)));
        let bias = s.mean_z_rounding_interval.unwrap();
        assert_eq!((bias.lo, bias.hi), (q(6, 5), q(13, 10)));
    }

    #[test]
    fn test_clopper_pearson_closed_form_edges() {
        let rows: Vec<_> = (0..10).map(|i| member(&i.to_string(), Some(125))).collect();
        let (lo, hi) = summarize(&rows).unwrap().iid_reference_interval_90.unwrap();
        let expected = 1.0 - fmine_core::det::exp(fmine_core::det::ln(0.05) / 10.0);
        assert_eq!(lo, 0.0);
        assert!((hi - expected).abs() < 1e-12);
        let rows: Vec<_> = (0..10).map(|i| member(&i.to_string(), Some(110))).collect();
        let (lo, hi) = summarize(&rows).unwrap().iid_reference_interval_90.unwrap();
        assert!((lo - (1.0 - expected)).abs() < 1e-12);
        assert_eq!(hi, 1.0);
    }

    #[test]
    fn test_no_calibration_verdict_at_any_sample_size() {
        for n in [0, 1, 34, 35, 100] {
            let rows: Vec<_> = (0..n).map(|i| member(&i.to_string(), Some(110))).collect();
            assert_eq!(
                summarize(&rows).unwrap().flag,
                if n == 0 {
                    CalibrationFlag::Unscored
                } else {
                    CalibrationFlag::Descriptive
                }
            );
        }
        let missing = summarize(&[member("a", None)]).unwrap();
        assert_eq!(missing.flag, CalibrationFlag::Unscored);
        assert_eq!(missing.iid_reference_interval_90, None);
        assert_eq!(missing.observed_hit_rate, None);
        assert_eq!(missing.whole_cohort_hit_bounds.unwrap().lo, ZERO);
        assert_eq!(missing.whole_cohort_hit_bounds.unwrap().hi, q(1, 1));
    }

    #[test]
    fn test_malformed_inputs_refuse_including_unresolved_members() {
        let invalid = Guidance::Range {
            lo: q(120, 1),
            hi: q(100, 1),
        };
        assert_eq!(
            score(invalid, printed(110, ZERO)),
            Err(GuidanceScoreError::InvalidRange)
        );
        assert_eq!(
            score(Guidance::Range { lo: ZERO, hi: ZERO }, printed(0, ZERO)),
            Err(GuidanceScoreError::InvalidRange)
        );
        let mut outcome = printed(110, ZERO);
        outcome.interval.hi_strict = true;
        assert_eq!(
            score(range(), outcome),
            Err(GuidanceScoreError::InvalidOutcomeInterval)
        );
        outcome = printed(110, ZERO);
        outcome.value = q(125, 1);
        assert_eq!(
            score(range(), outcome),
            Err(GuidanceScoreError::PrintedOutcomeOutsideInterval)
        );
        let mut missing = member("a", None);
        missing.guidance = invalid;
        assert_eq!(summarize(&[missing]), Err(GuidanceScoreError::InvalidRange));
        assert_eq!(
            summarize(&[member("a", None), member("a", None)]),
            Err(GuidanceScoreError::DuplicateMemberId)
        );
        assert_eq!(
            summarize(&[member(" ", None)]),
            Err(GuidanceScoreError::InvalidMemberId)
        );
        assert_eq!(
            score(
                Guidance::Range {
                    lo: q(i128::MIN + 1, 1),
                    hi: q(i128::MAX, 1)
                },
                printed(0, ZERO)
            ),
            Err(GuidanceScoreError::ArithmeticOverflow)
        );
    }

    #[test]
    fn test_one_sided_guidance_boundary_and_excluded_lower_endpoint() {
        for guidance in [
            Guidance::AtMost { upper: q(100, 1) },
            Guidance::AtLeast { lower: q(100, 1) },
        ] {
            let boundary = score(guidance, printed(100, q(1, 2))).unwrap();
            assert_eq!(boundary.side, Side::Inside);
            assert!(boundary.boundary && boundary.possibly_inside && !boundary.definitely_inside);
            assert!(
                score(guidance, printed(100, ZERO))
                    .unwrap()
                    .definitely_inside
            );
        }
        let mut below = printed(99, q(1, 1));
        assert!(score(range(), below).unwrap().possibly_inside);
        below.interval.hi_strict = true;
        assert!(!score(range(), below).unwrap().possibly_inside);
    }

    #[test]
    fn test_clopper_pearson_single_member_and_maximum_budget() {
        let one = clopper_pearson_90(1, 1).unwrap();
        assert!((one.0 - 0.05).abs() < 1e-12);
        assert_eq!(one.1, 1.0);
        let zero = clopper_pearson_90(0, MAX_BINOMIAL_N).unwrap();
        let expected =
            1.0 - fmine_core::det::exp(fmine_core::det::ln(0.05) / MAX_BINOMIAL_N as f64);
        assert_eq!(zero.0, 0.0);
        assert!((zero.1 - expected).abs() < 1e-12);
        let central = clopper_pearson_90(MAX_BINOMIAL_N / 2, MAX_BINOMIAL_N).unwrap();
        assert!(central.0 < 0.5 && central.1 > 0.5);
        assert!((central.0 + central.1 - 1.0).abs() < 1e-12);
    }

    #[test]
    fn binomial_bounds_match_the_existing_independent_tail_implementation() {
        for n in [2, 7, 10, 40, 100, 300] {
            for k in [0, 1, n / 2, n - 1, n] {
                let actual = clopper_pearson_90(k, n).expect("interval");
                let reference =
                    fmine_numerics::exact_feas::power::clopper_pearson(k as u64, n as u64, 0.1)
                        .expect("reference");
                assert!((actual.0 - reference.0).abs() < 1e-10, "{k}/{n} lower");
                assert!((actual.1 - reference.1).abs() < 1e-10, "{k}/{n} upper");
            }
        }
    }

    #[test]
    fn clopper_pearson_refuses_nonconvergence_and_invalid_populations() {
        assert!(binomial_interval(3, 10, 0).is_err());
        assert!(clopper_pearson_90(0, 0).is_err());
        assert!(clopper_pearson_90(11, 10).is_err());
        assert!(clopper_pearson_90(1, MAX_BINOMIAL_N + 1).is_err());
    }
}
