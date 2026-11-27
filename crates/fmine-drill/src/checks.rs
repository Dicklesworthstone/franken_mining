//! Campaign data checks (`BD-286`, plan §12.2, §12.4). Each returns its finding with the numbers behind it.
//! Determinism: BitwisePortable (IEEE-754 arithmetic with `fmine_core::det`; ranks by total order).
//!
//! - **Recovery–grade bias test (E378).** Per (domain, campaign, drilling technique, recovery method), Spearman's
//!   rank correlation of grade with core recovery. A significant dependence marks the group's intervals
//!   `Conflicted` and is an investigation finding. Intervals with no recovery, or recovery of unknown method, are
//!   left out and listed: recovery is only comparable within one method.
//! - **Data-generation comparison (E379).** Per domain, every pair of campaigns: means, the Q–Q points at the
//!   deciles, and a two-sided Mann–Whitney rank-sum test. A campaign that differs from every other one is named
//!   for review before it joins an estimate.
//! - **Survey checks (E381).** In/out run consistency per hole: at each depth surveyed both ways, the dip and
//!   azimuth differences against `k` times their combined one-sigma accuracies, and the distance between the two
//!   runs' desurveyed traces at their deepest shared depth. [`run_separation`] tests that distance against `k`
//!   times its one-sigma from the two runs' survey-error covariances ([`crate::survey_error`]); the collar's
//!   error is shared by both runs and cancels. Collar against DTM: the residual of the surveyed collar elevation
//!   against `k` times the combined one-sigma of collar and DTM.
//! - **`qaqc_regime` per campaign.** The regimes its results declare; a campaign that mixes them is reported.
//! - **`CommittedFlagged` and retention (E223, E224).** The results of a flagged lab batch are flagged; each
//!   estimate says explicitly whether they enter. A flagged result whose sample was discarded after QC cannot be
//!   re-assayed, and its state says so.

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::det;

use crate::desurvey::{Direction, Point, desurvey};
use crate::records::{
    AssayResult, BatchFlag, Collar, DrillError, HoleType, Interval, QaqcRegime, RecoveryMethod,
    RunDirection, Sample, SampleDiscard, Survey,
};
use crate::stats::{mann_whitney, quantile, spearman};
use crate::survey_error::{Covariance, ErrorModel, propagate};

/// Thresholds the checks share.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CheckPolicy {
    /// Two-sided significance level of the recovery–grade and campaign tests, in (0, 0.5].
    pub alpha: f64,
    /// Fewest values in a group for a test; at least 3.
    pub min_samples: usize,
    /// Multiple of the combined one-sigma beyond which a survey or collar difference is reported; above 0.
    pub sigma_multiple: f64,
}

impl Default for CheckPolicy {
    fn default() -> Self {
        Self {
            alpha: 0.05,
            min_samples: 10,
            sigma_multiple: 3.0,
        }
    }
}

impl CheckPolicy {
    fn check(&self) -> Result<(), DrillError> {
        if !(self.alpha > 0.0 && self.alpha <= 0.5)
            || self.min_samples < 3
            || !(self.sigma_multiple > 0.0 && self.sigma_multiple.is_finite())
        {
            return Err(DrillError::invalid(
                "checks.policy",
                format!(
                    "alpha {} in (0, 0.5], min_samples {} at least 3, sigma_multiple {} above 0",
                    self.alpha, self.min_samples, self.sigma_multiple
                ),
            ));
        }
        Ok(())
    }
}

/// Collars, intervals and samples by id: who drilled what, in which campaign.
#[derive(Clone, Debug)]
pub struct DrillIndex<'a> {
    collars: BTreeMap<&'a str, &'a Collar>,
    intervals: BTreeMap<&'a str, &'a Interval>,
    samples: BTreeMap<&'a str, &'a Sample>,
}

impl<'a> DrillIndex<'a> {
    /// Index `collars`, `intervals` and `samples`.
    ///
    /// # Errors
    /// [`DrillError::Invalid`] (`FMINE-REF-109`) for a repeated id, an interval of a hole not given, or a sample
    /// of an interval not given or of intervals in two holes.
    pub fn new(
        collars: &'a [Collar],
        intervals: &'a [Interval],
        samples: &'a [Sample],
    ) -> Result<Self, DrillError> {
        fn by_id<'b, T>(
            field: &str,
            items: &'b [T],
            id: impl Fn(&'b T) -> &'b str,
        ) -> Result<BTreeMap<&'b str, &'b T>, DrillError> {
            let mut out = BTreeMap::new();
            for item in items {
                let key = id(item);
                if out.insert(key, item).is_some() {
                    return Err(DrillError::invalid(
                        field,
                        format!("'{key}' is given twice"),
                    ));
                }
            }
            Ok(out)
        }
        let index = Self {
            collars: by_id("checks.collars", collars, |c| c.hole_id.as_str())?,
            intervals: by_id("checks.intervals", intervals, |i| i.interval_id.as_str())?,
            samples: by_id("checks.samples", samples, |s| s.sample_id.as_str())?,
        };
        for i in intervals {
            if !index.collars.contains_key(i.hole_id.as_str()) {
                return Err(DrillError::invalid(
                    "checks.intervals",
                    format!(
                        "interval '{}' is in hole '{}', which is not given",
                        i.interval_id, i.hole_id
                    ),
                ));
            }
        }
        for s in samples {
            index.sample_collar(&s.sample_id)?;
        }
        Ok(index)
    }

    fn interval_collar(&self, interval_id: &str) -> Result<(&'a Interval, &'a Collar), DrillError> {
        let interval = self.intervals.get(interval_id).copied().ok_or_else(|| {
            DrillError::invalid(
                "checks.grades",
                format!("interval '{interval_id}' is not given"),
            )
        })?;
        Ok((interval, self.collars[interval.hole_id.as_str()]))
    }

    /// The collar a sample was drilled from; `None` for inserted QC material, which comes from no interval.
    fn sample_collar(&self, sample_id: &str) -> Result<Option<&'a Collar>, DrillError> {
        let sample = self.samples.get(sample_id).copied().ok_or_else(|| {
            DrillError::invalid(
                "checks.results",
                format!("sample '{sample_id}' is not given"),
            )
        })?;
        let mut holes = BTreeSet::new();
        for id in &sample.interval_ids {
            let interval = self.intervals.get(id.as_str()).ok_or_else(|| {
                DrillError::invalid(
                    "checks.samples",
                    format!("sample '{sample_id}' names interval '{id}', not given"),
                )
            })?;
            holes.insert(interval.hole_id.as_str());
        }
        match holes.len() {
            0 => Ok(None),
            1 => Ok(holes.first().map(|h| self.collars[*h])),
            _ => Err(DrillError::invalid(
                "checks.samples",
                format!("sample '{sample_id}' spans holes {holes:?}"),
            )),
        }
    }
}

/// One interval's domain and grade, as the caller coded and assayed it.
#[derive(Clone, Debug, PartialEq)]
pub struct IntervalGrade {
    /// The interval.
    pub interval_id: String,
    /// Its domain code.
    pub domain: String,
    /// Its grade, in one unit across the call; finite and at least 0.
    pub grade: f64,
}

fn check_grades(grades: &[IntervalGrade]) -> Result<(), DrillError> {
    let mut seen = BTreeSet::new();
    for g in grades {
        if !g.grade.is_finite() || g.grade < 0.0 {
            return Err(DrillError::invalid(
                "checks.grades",
                format!("interval '{}': grade {}", g.interval_id, g.grade),
            ));
        }
        if !seen.insert(g.interval_id.as_str()) {
            return Err(DrillError::invalid(
                "checks.grades",
                format!("interval '{}' is graded twice", g.interval_id),
            ));
        }
    }
    Ok(())
}

/// Whether grade depends on recovery in a group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DependenceState {
    /// Fewer intervals than the policy's minimum: not tested.
    TooFewIntervals,
    /// No significant dependence.
    Independent,
    /// A significant dependence: the group's intervals are `Conflicted`.
    Dependent,
}

/// The recovery–grade test of one (domain, campaign, technique, recovery method) group.
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryGradeCheck {
    /// The domain.
    pub domain: String,
    /// The campaign (the collars' program).
    pub campaign: String,
    /// The drilling technique.
    pub hole_type: HoleType,
    /// How recovery was measured.
    pub recovery_method: RecoveryMethod,
    /// The group's intervals, ascending.
    pub interval_ids: Vec<String>,
    /// Spearman's ρ of grade with recovery; `None` when not tested or a side is constant.
    pub rho: Option<f64>,
    /// Its two-sided p.
    pub p_two_sided: Option<f64>,
    /// The outcome.
    pub state: DependenceState,
}

/// The recovery–grade test over all groups.
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryGradeReport {
    /// One check per group, ordered by domain, campaign, technique and recovery method.
    pub checks: Vec<RecoveryGradeCheck>,
    /// Graded intervals with no recovery or recovery of unknown method, ascending.
    pub unmeasured: Vec<String>,
}

impl RecoveryGradeReport {
    /// Intervals marked `Conflicted`: those of groups with a significant dependence.
    pub fn conflicted(&self) -> impl Iterator<Item = &str> {
        self.checks
            .iter()
            .filter(|c| c.state == DependenceState::Dependent)
            .flat_map(|c| c.interval_ids.iter().map(String::as_str))
    }
}

/// The recovery–grade bias test (plan §12.2).
///
/// # Errors
/// [`DrillError::Invalid`] (`FMINE-REF-109`) for a policy out of range, an interval not in `index`, an interval
/// graded twice, or a grade that is negative or not finite.
pub fn recovery_grade(
    index: &DrillIndex<'_>,
    grades: &[IntervalGrade],
    policy: &CheckPolicy,
) -> Result<RecoveryGradeReport, DrillError> {
    policy.check()?;
    check_grades(grades)?;
    type Key<'k> = (&'k str, &'k str, HoleType, RecoveryMethod);
    let mut groups: BTreeMap<Key<'_>, Vec<(&str, f64, f64)>> = BTreeMap::new();
    let mut unmeasured = Vec::new();
    for g in grades {
        let (interval, collar) = index.interval_collar(&g.interval_id)?;
        match &interval.recovery {
            Some(r) if r.method != RecoveryMethod::Unknown => {
                groups
                    .entry((
                        g.domain.as_str(),
                        collar.program.as_str(),
                        collar.hole_type,
                        r.method,
                    ))
                    .or_default()
                    .push((g.interval_id.as_str(), r.percent, g.grade));
            }
            _ => unmeasured.push(g.interval_id.clone()),
        }
    }
    unmeasured.sort();
    let checks = groups
        .into_iter()
        .map(
            |((domain, campaign, hole_type, recovery_method), mut rows)| {
                rows.sort_by(|a, b| a.0.cmp(b.0));
                let (rho, p, state) = if rows.len() < policy.min_samples {
                    (None, None, DependenceState::TooFewIntervals)
                } else {
                    let recovery: Vec<f64> = rows.iter().map(|r| r.1).collect();
                    let grade: Vec<f64> = rows.iter().map(|r| r.2).collect();
                    match spearman(&recovery, &grade) {
                        Some(c) if c.p_two_sided < policy.alpha => {
                            (Some(c.rho), Some(c.p_two_sided), DependenceState::Dependent)
                        }
                        Some(c) => (
                            Some(c.rho),
                            Some(c.p_two_sided),
                            DependenceState::Independent,
                        ),
                        None => (None, None, DependenceState::Independent),
                    }
                };
                RecoveryGradeCheck {
                    domain: domain.to_string(),
                    campaign: campaign.to_string(),
                    hole_type,
                    recovery_method,
                    interval_ids: rows.iter().map(|r| r.0.to_string()).collect(),
                    rho,
                    p_two_sided: p,
                    state,
                }
            },
        )
        .collect();
    Ok(RecoveryGradeReport { checks, unmeasured })
}

/// How two campaigns compare in a domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparisonState {
    /// Fewer intervals than the policy's minimum on either side: not tested.
    TooFewIntervals,
    /// No significant difference.
    Comparable,
    /// A significant difference.
    Differs,
}

/// Two campaigns compared in one domain.
#[derive(Clone, Debug, PartialEq)]
pub struct CampaignComparison {
    /// The domain.
    pub domain: String,
    /// The first campaign, by name.
    pub campaign: String,
    /// The second.
    pub other: String,
    /// Intervals of each in the domain.
    pub n: (usize, usize),
    /// Mean grade of each.
    pub mean: (f64, f64),
    /// Q–Q points `(campaign, other)` at 10%, 20% … 90%.
    pub qq: Vec<(f64, f64)>,
    /// Two-sided Mann–Whitney p; `None` when not tested or every value is equal.
    pub p_two_sided: Option<f64>,
    /// The outcome.
    pub state: ComparisonState,
}

/// The data-generation comparison of every pair of campaigns in every domain.
#[derive(Clone, Debug, PartialEq)]
pub struct ComparisonReport {
    /// One comparison per domain and pair of campaigns, by domain then names.
    pub comparisons: Vec<CampaignComparison>,
}

impl ComparisonReport {
    /// `(domain, campaign)` for each campaign that differs from every other campaign of its domain, with at least
    /// one other tested: review it before it joins an estimate. Of two campaigns that differ, both are named.
    #[must_use]
    pub fn outliers(&self) -> Vec<(String, String)> {
        let mut seen: BTreeMap<(&str, &str), (usize, usize)> = BTreeMap::new();
        for c in &self.comparisons {
            if c.state == ComparisonState::TooFewIntervals {
                continue;
            }
            let differs = usize::from(c.state == ComparisonState::Differs);
            for name in [&c.campaign, &c.other] {
                let e = seen.entry((c.domain.as_str(), name.as_str())).or_default();
                e.0 += 1;
                e.1 += differs;
            }
        }
        seen.into_iter()
            .filter(|(_, (tested, differs))| *tested > 0 && tested == differs)
            .map(|((d, c), _)| (d.to_string(), c.to_string()))
            .collect()
    }
}

/// The data-generation comparison (plan §12.2, E379): per domain, every pair of campaigns, by mean, Q–Q points and
/// a Mann–Whitney rank-sum test. Pairwise, so that one biased campaign does not shift a pooled reference.
///
/// # Errors
/// As [`recovery_grade`].
pub fn compare_campaigns(
    index: &DrillIndex<'_>,
    grades: &[IntervalGrade],
    policy: &CheckPolicy,
) -> Result<ComparisonReport, DrillError> {
    policy.check()?;
    check_grades(grades)?;
    let mut by_domain: BTreeMap<&str, BTreeMap<&str, Vec<f64>>> = BTreeMap::new();
    for g in grades {
        let (_, collar) = index.interval_collar(&g.interval_id)?;
        by_domain
            .entry(g.domain.as_str())
            .or_default()
            .entry(collar.program.as_str())
            .or_default()
            .push(g.grade);
    }
    let mut comparisons = Vec::new();
    for (domain, campaigns) in &mut by_domain {
        for values in campaigns.values_mut() {
            values.sort_by(f64::total_cmp);
        }
        let named: Vec<(&str, &Vec<f64>)> = campaigns.iter().map(|(c, v)| (*c, v)).collect();
        for (i, (campaign, a)) in named.iter().enumerate() {
            for (other, b) in &named[i + 1..] {
                let qq = (1..=9_u32)
                    .filter_map(|k| {
                        let p = f64::from(k) / 10.0;
                        Some((quantile(a, p)?, quantile(b, p)?))
                    })
                    .collect();
                let (p, state) = if a.len() < policy.min_samples || b.len() < policy.min_samples {
                    (None, ComparisonState::TooFewIntervals)
                } else {
                    match mann_whitney(a, b) {
                        Some(r) if r.p_two_sided < policy.alpha => {
                            (Some(r.p_two_sided), ComparisonState::Differs)
                        }
                        Some(r) => (Some(r.p_two_sided), ComparisonState::Comparable),
                        None => (None, ComparisonState::Comparable),
                    }
                };
                comparisons.push(CampaignComparison {
                    domain: (*domain).to_string(),
                    campaign: (*campaign).to_string(),
                    other: (*other).to_string(),
                    n: (a.len(), b.len()),
                    mean: (mean(a).unwrap_or(0.0), mean(b).unwrap_or(0.0)),
                    qq,
                    p_two_sided: p,
                    state,
                });
            }
        }
    }
    Ok(ComparisonReport { comparisons })
}

#[allow(clippy::cast_precision_loss)]
fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

/// A depth surveyed both going in and coming out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StationPair {
    /// Depth along the hole, mm.
    pub depth_mm: u64,
    /// Out-run dip minus in-run dip, degrees.
    pub dip_difference_deg: f64,
    /// Out-run azimuth minus in-run azimuth, wrapped to (−180, 180], degrees; `None` within 5° of vertical,
    /// where azimuth carries no direction.
    pub azimuth_difference_deg: Option<f64>,
    /// The larger of the two differences as a multiple of `k` times its combined one-sigma; above 1 is
    /// inconsistent.
    pub ratio: f64,
}

/// In/out run consistency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    /// No depth was surveyed both ways.
    NoPairedStations,
    /// Every paired depth agrees within `k` sigma.
    Consistent,
    /// At least one paired depth does not.
    Inconsistent,
}

/// One hole's in/out run check.
#[derive(Clone, Debug, PartialEq)]
pub struct RunCheck {
    /// The hole.
    pub hole_id: String,
    /// The paired depths, ascending.
    pub pairs: Vec<StationPair>,
    /// Distance between the two runs' desurveyed traces at their deepest shared depth, m; `None` unless both runs
    /// have surveyed stations.
    pub separation_m: Option<f64>,
    /// That depth, mm.
    pub at_depth_mm: Option<u64>,
    /// The outcome.
    pub state: RunState,
}

/// In/out run consistency of `collar`'s hole from its `surveys` (plan §12.2, E381).
///
/// # Errors
/// [`DrillError::Invalid`] (`FMINE-REF-109`) for a policy out of range or a survey of another hole; desurvey's
/// refusals for a run that cannot be desurveyed.
pub fn survey_runs(
    collar: &Collar,
    surveys: &[Survey],
    policy: &CheckPolicy,
) -> Result<RunCheck, DrillError> {
    policy.check()?;
    if let Some(s) = surveys.iter().find(|s| s.hole_id != collar.hole_id) {
        return Err(DrillError::invalid(
            "checks.surveys",
            format!(
                "survey '{}' is of hole '{}', not '{}'",
                s.survey_id, s.hole_id, collar.hole_id
            ),
        ));
    }
    let run = |direction: RunDirection| -> Vec<Survey> {
        let mut v: Vec<Survey> = surveys
            .iter()
            .filter(|s| s.run == direction && s.orientation.is_some())
            .cloned()
            .collect();
        v.sort_by_key(|s| s.depth_mm);
        v
    };
    let (inward, outward) = (run(RunDirection::In), run(RunDirection::Out));
    let mut pairs = Vec::new();
    for a in &inward {
        let Some(b) = outward.iter().find(|b| b.depth_mm == a.depth_mm) else {
            continue;
        };
        let (Some(oa), Some(ob)) = (a.orientation, b.orientation) else {
            continue;
        };
        let k = policy.sigma_multiple;
        let dip = ob.dip_deg - oa.dip_deg;
        let dip_ratio = ratio(dip, k * det::hypot(oa.sigma_dip_deg, ob.sigma_dip_deg));
        let steep = oa.dip_deg.abs().max(ob.dip_deg.abs()) > 85.0;
        let azimuth = (!steep).then(|| {
            let mut d = ob.azimuth_deg - oa.azimuth_deg;
            if d > 180.0 {
                d -= 360.0;
            } else if d <= -180.0 {
                d += 360.0;
            }
            d
        });
        let azimuth_ratio = azimuth.map_or(0.0, |d| {
            ratio(
                d,
                k * det::hypot(oa.sigma_azimuth_deg, ob.sigma_azimuth_deg),
            )
        });
        pairs.push(StationPair {
            depth_mm: a.depth_mm,
            dip_difference_deg: dip,
            azimuth_difference_deg: azimuth,
            ratio: dip_ratio.max(azimuth_ratio),
        });
    }
    let (separation_m, at_depth_mm) = match (inward.last(), outward.last()) {
        (Some(a), Some(b)) => {
            let depth = a.depth_mm.min(b.depth_mm);
            let (ta, tb) = (desurvey(collar, &inward)?, desurvey(collar, &outward)?);
            (
                Some(distance(ta.at(depth).point, tb.at(depth).point)),
                Some(depth),
            )
        }
        _ => (None, None),
    };
    let state = if pairs.is_empty() {
        RunState::NoPairedStations
    } else if pairs.iter().any(|p| p.ratio > 1.0) {
        RunState::Inconsistent
    } else {
        RunState::Consistent
    };
    Ok(RunCheck {
        hole_id: collar.hole_id.clone(),
        pairs,
        separation_m,
        at_depth_mm,
        state,
    })
}

/// `|difference| / limit`; a zero limit makes any difference infinitely large.
fn ratio(difference: f64, limit: f64) -> f64 {
    if limit > 0.0 {
        difference.abs() / limit
    } else if difference == 0.0 {
        0.0
    } else {
        f64::INFINITY
    }
}

fn distance(a: Point, b: Point) -> f64 {
    det::hypot(
        det::hypot(a.easting_m - b.easting_m, a.northing_m - b.northing_m),
        a.elevation_m - b.elevation_m,
    )
}

/// The distance between a hole's in-run and out-run traces against its survey error.
#[derive(Clone, Debug, PartialEq)]
pub struct RunSeparation {
    /// The hole.
    pub hole_id: String,
    /// The deepest depth both runs surveyed, mm.
    pub at_depth_mm: u64,
    /// The distance between the runs' points there, m.
    pub separation_m: f64,
    /// Its one-sigma along the line between them from both runs' angle errors, m; the runs' errors are taken as
    /// independent. At zero separation, the root of the summed covariance's trace.
    pub sigma_m: f64,
    /// `separation / (k·σ)`; above 1 is inconsistent.
    pub ratio: f64,
    /// [`RunState::Consistent`] or [`RunState::Inconsistent`].
    pub state: RunState,
}

/// The in/out run separation of `collar`'s hole tested against its survey error under `model` (`bd-zx57`); `None`
/// when either run has no surveyed station.
///
/// # Errors
/// As [`survey_runs`], and the model refusals of [`propagate`].
pub fn run_separation(
    collar: &Collar,
    surveys: &[Survey],
    model: &ErrorModel,
    policy: &CheckPolicy,
) -> Result<Option<RunSeparation>, DrillError> {
    policy.check()?;
    if let Some(s) = surveys.iter().find(|s| s.hole_id != collar.hole_id) {
        return Err(DrillError::invalid(
            "checks.surveys",
            format!(
                "survey '{}' is of hole '{}', not '{}'",
                s.survey_id, s.hole_id, collar.hole_id
            ),
        ));
    }
    let run = |direction: RunDirection| -> Vec<Survey> {
        surveys
            .iter()
            .filter(|s| s.run == direction && s.orientation.is_some())
            .cloned()
            .collect()
    };
    let (inward, outward) = (run(RunDirection::In), run(RunDirection::Out));
    let deepest = |v: &[Survey]| v.iter().map(|s| s.depth_mm).max();
    let (Some(a), Some(b)) = (deepest(&inward), deepest(&outward)) else {
        return Ok(None);
    };
    let depth = a.min(b);
    let (ti, to) = (
        propagate(collar, &inward, model)?,
        propagate(collar, &outward, model)?,
    );
    let (pi, po) = (ti.trace().at(depth).point, to.trace().at(depth).point);
    let separation = distance(pi, po);
    // The collar is one point for both runs: only the angle terms move them apart.
    let angles = |c: &crate::survey_error::PositionCovariance| -> Covariance {
        let mut m = c.total;
        for (row, collar_row) in m.enu_m2.iter_mut().zip(&c.collar.enu_m2) {
            for (v, k) in row.iter_mut().zip(collar_row) {
                *v -= k;
            }
        }
        m
    };
    let (ci, co) = (
        angles(&ti.covariance_at(depth)),
        angles(&to.covariance_at(depth)),
    );
    let variance = if separation > 0.0 {
        let u = Direction {
            east: (po.easting_m - pi.easting_m) / separation,
            north: (po.northing_m - pi.northing_m) / separation,
            up: (po.elevation_m - pi.elevation_m) / separation,
        };
        ci.variance_along(u) + co.variance_along(u)
    } else {
        (0..3).map(|i| ci.enu_m2[i][i] + co.enu_m2[i][i]).sum()
    };
    let sigma = det::sqrt(variance.max(0.0));
    let r = ratio(separation, policy.sigma_multiple * sigma);
    Ok(Some(RunSeparation {
        hole_id: collar.hole_id.clone(),
        at_depth_mm: depth,
        separation_m: separation,
        sigma_m: sigma,
        ratio: r,
        state: if r > 1.0 {
            RunState::Inconsistent
        } else {
            RunState::Consistent
        },
    }))
}

/// The DTM's elevation at a collar's position, with its one-sigma accuracy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DtmElevation {
    /// Elevation, m, in the collar's vertical datum.
    pub elevation_m: f64,
    /// One-sigma accuracy, m; at least 0.
    pub sigma_m: f64,
}

/// A collar against the DTM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollarDtmState {
    /// The collar has no surveyed position to compare.
    NoActualPosition,
    /// Within `k` combined sigma.
    Agrees,
    /// Beyond it.
    Disagrees,
}

/// The collar-vs-DTM residual of one hole.
#[derive(Clone, Debug, PartialEq)]
pub struct CollarDtmCheck {
    /// The hole.
    pub hole_id: String,
    /// Surveyed collar elevation minus DTM elevation, m.
    pub residual_m: Option<f64>,
    /// `k·√(σ_collar² + σ_DTM²)`, m.
    pub limit_m: Option<f64>,
    /// The outcome.
    pub state: CollarDtmState,
}

/// The collar-vs-DTM residual of `collar` (plan §12.2, E381).
///
/// # Errors
/// [`DrillError::Invalid`] (`FMINE-REF-109`) for a policy out of range or a DTM elevation or accuracy that is not
/// finite or a negative accuracy.
pub fn collar_dtm(
    collar: &Collar,
    dtm: &DtmElevation,
    policy: &CheckPolicy,
) -> Result<CollarDtmCheck, DrillError> {
    policy.check()?;
    if !dtm.elevation_m.is_finite() || !dtm.sigma_m.is_finite() || dtm.sigma_m < 0.0 {
        return Err(DrillError::invalid(
            "checks.dtm",
            format!(
                "elevation {} and sigma {} must be finite, sigma at least 0",
                dtm.elevation_m, dtm.sigma_m
            ),
        ));
    }
    let Some(actual) = &collar.actual else {
        return Ok(CollarDtmCheck {
            hole_id: collar.hole_id.clone(),
            residual_m: None,
            limit_m: None,
            state: CollarDtmState::NoActualPosition,
        });
    };
    let residual = actual.elevation_m - dtm.elevation_m;
    let limit = policy.sigma_multiple * det::hypot(actual.sigma_vertical_m, dtm.sigma_m);
    Ok(CollarDtmCheck {
        hole_id: collar.hole_id.clone(),
        residual_m: Some(residual),
        limit_m: Some(limit),
        state: if ratio(residual, limit) > 1.0 {
            CollarDtmState::Disagrees
        } else {
            CollarDtmState::Agrees
        },
    })
}

/// Whether a campaign's results declare one QA/QC regime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegimeState {
    /// One regime.
    Uniform,
    /// More than one.
    Mixed,
}

/// The regimes one campaign's results declare.
#[derive(Clone, Debug, PartialEq)]
pub struct RegimeCheck {
    /// The campaign.
    pub campaign: String,
    /// Results per declared regime, in regime order.
    pub counts: Vec<(QaqcRegime, usize)>,
    /// The outcome.
    pub state: RegimeState,
}

/// `qaqc_regime` per campaign.
#[derive(Clone, Debug, PartialEq)]
pub struct RegimeReport {
    /// One check per campaign, by campaign.
    pub checks: Vec<RegimeCheck>,
    /// Results on inserted QC material, which belongs to no campaign.
    pub without_campaign: usize,
}

/// The QA/QC regimes each campaign's results declare.
///
/// # Errors
/// [`DrillError::Invalid`] (`FMINE-REF-109`) for a result whose sample is not in `index`.
pub fn qaqc_regimes(
    index: &DrillIndex<'_>,
    results: &[AssayResult],
) -> Result<RegimeReport, DrillError> {
    let mut by_campaign: BTreeMap<&str, BTreeMap<QaqcRegime, usize>> = BTreeMap::new();
    let mut without_campaign = 0;
    for r in results {
        match index.sample_collar(&r.sample_id)? {
            Some(collar) => {
                *by_campaign
                    .entry(collar.program.as_str())
                    .or_default()
                    .entry(r.qaqc_regime)
                    .or_default() += 1;
            }
            None => without_campaign += 1,
        }
    }
    let checks = by_campaign
        .into_iter()
        .map(|(campaign, counts)| RegimeCheck {
            campaign: campaign.to_string(),
            state: if counts.len() > 1 {
                RegimeState::Mixed
            } else {
                RegimeState::Uniform
            },
            counts: counts.into_iter().collect(),
        })
        .collect();
    Ok(RegimeReport {
        checks,
        without_campaign,
    })
}

/// Whether a flagged result can be re-assayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reassay {
    /// No discard of its sample is recorded.
    NoDiscardRecorded,
    /// Its sample's material was discarded after QC: re-assay is impossible.
    Impossible {
        /// When it was discarded, ns since the Unix epoch, UTC.
        discarded_at_ns: i64,
    },
}

/// A result of a lab batch committed with an unresolved failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlaggedResult {
    /// The result.
    pub result_id: String,
    /// The flag on its batch.
    pub flag_id: String,
    /// Whether it can be re-assayed.
    pub reassay: Reassay,
}

/// Whether flagged results enter an estimate. There is no default: each estimate decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlaggedEntry {
    /// They enter.
    Include,
    /// They are left out.
    Exclude,
}

/// The results an estimate takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    /// The decision taken.
    pub decision: FlaggedEntry,
    /// Results entering, ascending.
    pub included: Vec<String>,
    /// Every flagged result, ascending, whichever way the decision went.
    pub flagged: Vec<FlaggedResult>,
}

/// Select `results` for an estimate under `decision`, flagging those of batches in `flags` (plan §12.4,
/// `CommittedFlagged`) and stating, from `discards`, whether each flagged one can be re-assayed.
#[must_use]
pub fn select_for_estimate(
    results: &[AssayResult],
    flags: &[BatchFlag],
    discards: &[SampleDiscard],
    decision: FlaggedEntry,
) -> Selection {
    let flag_of: BTreeMap<(&str, &str), &str> = flags
        .iter()
        .map(|f| ((f.lab.as_str(), f.lab_batch.as_str()), f.flag_id.as_str()))
        .collect();
    let discarded: BTreeMap<&str, i64> = discards
        .iter()
        .map(|d| (d.sample_id.as_str(), d.discarded_at_ns))
        .collect();
    let mut included = Vec::new();
    let mut flagged = Vec::new();
    for r in results {
        match flag_of.get(&(r.lab.as_str(), r.lab_batch.as_str())) {
            Some(flag_id) => {
                flagged.push(FlaggedResult {
                    result_id: r.result_id.clone(),
                    flag_id: (*flag_id).to_string(),
                    reassay: discarded.get(r.sample_id.as_str()).map_or(
                        Reassay::NoDiscardRecorded,
                        |&at| Reassay::Impossible {
                            discarded_at_ns: at,
                        },
                    ),
                });
                if decision == FlaggedEntry::Include {
                    included.push(r.result_id.clone());
                }
            }
            None => included.push(r.result_id.clone()),
        }
    }
    included.sort();
    flagged.sort_by(|a, b| a.result_id.cmp(&b.result_id));
    Selection {
        decision,
        included,
        flagged,
    }
}
