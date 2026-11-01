//! Exact assessment of the complete universe frozen by a public forecast record.
//! Determinism: Exact (rational interval losses, coverage and effective sample sizes).
//!
//! This kernel neither authenticates timestamps nor reads sources. The adapter verifies the frozen record
//! and binds original-release observations before calling it. An absent observation remains a member.
//! Coverage confidence bounds are an explicitly iid Bernoulli reference, never a calibration certificate.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fmine_numerics::exact_feas::Interval;

use super::{ForecastRow, Rat, RecordRules, SiteStatus, interval_score};
use crate::guidance_scores::clopper_pearson_exact;

pub mod paired;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssessmentError(pub String);

impl fmt::Display for AssessmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AssessmentError {}

type Result<T> = std::result::Result<T, AssessmentError>;

fn exact<T, E: fmt::Display>(value: std::result::Result<T, E>) -> Result<T> {
    value.map_err(|e| AssessmentError(e.to_string()))
}

fn le(a: Rat, b: Rat) -> Result<bool> {
    Ok(exact(a.checked_cmp(b))? != std::cmp::Ordering::Greater)
}

/// Exactly one supplied resolution per frozen live site and target. None is retained, never imputed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeRow {
    pub site: String,
    pub target: String,
    pub value: Option<Rat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberScore {
    pub site: String,
    pub target: String,
    pub forecasted: bool,
    pub outcome: Option<Rat>,
    pub interval_score: Option<Rat>,
    pub covered: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetScore {
    pub target: String,
    pub n_universe: usize,
    pub n_frozen: usize,
    pub n_scored: usize,
    pub n_unresolved: usize,
    pub not_forecast: Vec<String>,
    pub unresolved: Vec<String>,
    pub covered: usize,
    pub observed_coverage: Option<Rat>,
    /// Best/worst possible final coverage over all forecast members, including unresolved members.
    pub coverage_completion: Option<(Rat, Rat)>,
    pub iid_coverage_reference: Interval,
    /// Descriptive mean over resolved members, not a comparison with any baseline.
    pub observed_mean_interval_score: Option<Rat>,
    /// Each unresolved interval contributes at least its width; no finite loss upper bound is invented.
    pub full_mean_interval_score_lower: Option<Rat>,
    pub full_mean_interval_score_upper: Option<Rat>,
    /// Equicorrelation sensitivity over resolved outcomes, at the two frozen rho endpoints.
    pub effective_n: (Rat, Rat),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordScore {
    pub members: Vec<MemberScore>,
    pub targets: Vec<TargetScore>,
}

fn count(n: usize) -> Result<Rat> {
    Ok(Rat::from_integer(i128::try_from(n).map_err(|_| {
        AssessmentError("count outside i128".into())
    })?))
}

fn mean(sum: Rat, n: usize) -> Result<Option<Rat>> {
    if n == 0 {
        Ok(None)
    } else {
        Ok(Some(exact(sum.checked_div(count(n)?))?))
    }
}

/// Shared exact interval loss and inclusive hit rule for the forecast and declared baseline methods.
fn interval_result(lo: Rat, hi: Rat, alpha: Rat, outcome: Rat) -> Result<(Rat, bool)> {
    Ok((
        exact(interval_score(lo, hi, alpha, outcome))?,
        le(lo, outcome)? && le(outcome, hi)?,
    ))
}

/// Shared frozen equicorrelation sensitivity. This is not an estimated dependence model.
fn effective_n(n: usize, rho_low: Rat, rho_high: Rat) -> Result<(Rat, Rat)> {
    let at = |rho: Rat| -> Result<Rat> {
        if n == 0 {
            return Ok(Rat::from_integer(0));
        }
        let denominator =
            exact(Rat::from_integer(1).checked_add(exact(count(n - 1)?.checked_mul(rho))?))?;
        exact(count(n)?.checked_div(denominator))
    };
    Ok((at(rho_high)?, at(rho_low)?))
}

/// Score each frozen live forecast exactly once and list the not-published universe separately. Unknown,
/// duplicate or omitted resolutions refuse instead of silently changing the denominator. Forecast validation
/// is repeated here so callers cannot bypass the CSV parser by constructing structs directly.
pub fn assess(
    rules: &RecordRules,
    forecasts: &[ForecastRow],
    outcomes: &[OutcomeRow],
) -> Result<RecordScore> {
    let site_names: BTreeSet<_> = rules.sites.iter().map(|s| s.name.as_str()).collect();
    let target_ids: BTreeSet<_> = rules.targets.iter().map(|t| t.id.as_str()).collect();
    if site_names.is_empty()
        || site_names.iter().any(|name| name.trim().is_empty())
        || site_names.len() != rules.sites.len()
        || target_ids.iter().any(|id| id.trim().is_empty())
        || target_ids.is_empty()
        || target_ids.len() != rules.targets.len()
    {
        return Err(AssessmentError(
            "the frozen universe needs nonempty, unique site names and target IDs".into(),
        ));
    }
    let mut fs = BTreeMap::new();
    let mut ys = BTreeMap::new();
    for f in forecasts {
        if !rules
            .sites
            .iter()
            .any(|s| s.name == f.site && s.status == SiteStatus::Live)
            || !rules.targets.iter().any(|t| t.id == f.target)
            || f.lo.signum() < 0
            || !le(f.lo, f.point)?
            || !le(f.point, f.hi)?
            || fs.insert((f.site.as_str(), f.target.as_str()), f).is_some()
        {
            return Err(AssessmentError(format!(
                "invalid or duplicate forecast for {} / {}",
                f.site, f.target
            )));
        }
    }
    for y in outcomes {
        if !fs.contains_key(&(y.site.as_str(), y.target.as_str()))
            || ys
                .insert((y.site.as_str(), y.target.as_str()), y.value)
                .is_some()
        {
            return Err(AssessmentError(format!(
                "unknown or duplicate outcome for {} / {}",
                y.site, y.target
            )));
        }
    }
    let zero = Rat::from_integer(0);
    let one = Rat::from_integer(1);
    if rules.rho_low.signum() < 0
        || !le(rules.rho_low, rules.rho_high)?
        || !le(rules.rho_high, one)?
    {
        return Err(AssessmentError("invalid frozen correlation range".into()));
    }
    let mut members = Vec::new();
    let mut targets = Vec::new();
    for target in &rules.targets {
        let mut n_frozen = 0;
        let mut n_scored = 0;
        let mut covered = 0;
        let mut sum = zero;
        let mut minimum_sum = zero;
        let mut unresolved = Vec::new();
        let mut not_forecast = Vec::new();
        for site in &rules.sites {
            if site.status == SiteStatus::NotPublished {
                not_forecast.push(site.name.clone());
                members.push(MemberScore {
                    site: site.name.clone(),
                    target: target.id.clone(),
                    forecasted: false,
                    outcome: None,
                    interval_score: None,
                    covered: None,
                });
                continue;
            }
            let key = (site.name.as_str(), target.id.as_str());
            let f = fs.get(&key).ok_or_else(|| {
                AssessmentError(format!(
                    "missing frozen forecast for {} / {}",
                    site.name, target.id
                ))
            })?;
            let y = *ys.get(&key).ok_or_else(|| {
                AssessmentError(format!(
                    "missing explicit resolution for {} / {}",
                    site.name, target.id
                ))
            })?;
            n_frozen += 1;
            let width = exact(f.hi.checked_sub(f.lo))?;
            // Validates alpha even when every outcome is missing.
            exact(interval_score(f.lo, f.hi, rules.alpha, f.point))?;
            let scored = y
                .map(|y| interval_result(f.lo, f.hi, rules.alpha, y))
                .transpose()?;
            let (loss, hit) = scored.map_or((None, None), |(loss, hit)| (Some(loss), Some(hit)));
            match loss {
                Some(loss) => {
                    n_scored += 1;
                    covered += usize::from(hit == Some(true));
                    sum = exact(sum.checked_add(loss))?;
                    minimum_sum = exact(minimum_sum.checked_add(loss))?;
                }
                None => {
                    unresolved.push(site.name.clone());
                    minimum_sum = exact(minimum_sum.checked_add(width))?;
                }
            }
            members.push(MemberScore {
                site: site.name.clone(),
                target: target.id.clone(),
                forecasted: true,
                outcome: y,
                interval_score: loss,
                covered: hit,
            });
        }
        let n_unresolved = n_frozen - n_scored;
        let observed_mean = mean(sum, n_scored)?;
        targets.push(TargetScore {
            target: target.id.clone(),
            n_universe: rules.sites.len(),
            n_frozen,
            n_scored,
            n_unresolved,
            not_forecast,
            unresolved,
            covered,
            observed_coverage: mean(count(covered)?, n_scored)?,
            coverage_completion: if n_frozen == 0 {
                None
            } else {
                Some((
                    exact(count(covered)?.checked_div(count(n_frozen)?))?,
                    exact(count(covered + n_unresolved)?.checked_div(count(n_frozen)?))?,
                ))
            },
            iid_coverage_reference: exact(clopper_pearson_exact(
                covered,
                n_scored,
                rules.coverage_level,
            ))?,
            observed_mean_interval_score: observed_mean,
            full_mean_interval_score_lower: mean(minimum_sum, n_frozen)?,
            full_mean_interval_score_upper: if n_unresolved == 0 {
                observed_mean
            } else {
                None
            },
            effective_n: effective_n(n_scored, rules.rho_low, rules.rho_high)?,
        });
    }
    Ok(RecordScore { members, targets })
}

/// Comparator intervals resolved under the same frozen rules as the forecast. A missing interval is
/// explicit. Issuer guidance is assessed as a range; no probability meaning is assigned to its bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkRow {
    pub site: String,
    pub target: String,
    pub issuer_guidance: Option<(Rat, Rat)>,
    pub run_rate_persistence: Option<(Rat, Rat)>,
}

/// Every reason that a live forecast member cannot enter the common comparison set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonExclusion {
    pub site: String,
    pub missing_actual: bool,
    pub missing_issuer_guidance: bool,
    pub missing_run_rate_persistence: bool,
}

/// Arithmetic bounds over every frozen live member, allowing each unresolved outcome any real value.
/// These are completion bounds, not confidence intervals. A missing comparator leaves the bounds absent;
/// it never reduces the denominator to members with available evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComparisonCompletion {
    pub pending_outcomes: Vec<String>,
    pub missing_benchmarks: Vec<String>,
    pub mean_difference_bounds: Option<(Rat, Rat)>,
    pub arithmetic_refusal: Option<String>,
}

/// Exact global extrema of IS(forecast, y) - IS(benchmark, y), including signed and point intervals.
/// The difference is continuous and piecewise linear, with knots at the four endpoints. When alpha is
/// shared, the coefficients of y cancel in both tails, so extrema occur at those knots. Different alpha
/// values do not have this cancellation and are refused. No physical bound or probability model is assumed.
pub fn interval_score_difference_bounds(
    forecast: (Rat, Rat),
    benchmark: (Rat, Rat),
    forecast_alpha: Rat,
    benchmark_alpha: Rat,
) -> Result<(Rat, Rat)> {
    if forecast_alpha != benchmark_alpha {
        return Err(AssessmentError(
            "finite global interval-loss difference bounds require the same alpha".into(),
        ));
    }
    // The score primitive checks alpha, both interval orders and all arithmetic at every knot.
    let difference = |y| -> Result<Rat> {
        exact(
            exact(interval_score(forecast.0, forecast.1, forecast_alpha, y))?.checked_sub(exact(
                interval_score(benchmark.0, benchmark.1, benchmark_alpha, y),
            )?),
        )
    };
    let initial = difference(forecast.0)?;
    let (mut lo, mut hi) = (initial, initial);
    for y in [forecast.1, benchmark.0, benchmark.1] {
        let value = difference(y)?;
        if le(value, lo)? {
            lo = value;
        }
        if le(hi, value)? {
            hi = value;
        }
    }
    Ok((lo, hi))
}

fn completion<'a>(
    alpha: Rat,
    rows: impl IntoIterator<Item = (&'a MemberScore, &'a ForecastRow, Option<(Rat, Rat)>)>,
) -> ComparisonCompletion {
    let rows: Vec<_> = rows.into_iter().collect();
    let mut result = ComparisonCompletion {
        pending_outcomes: rows
            .iter()
            .filter(|(m, _, _)| m.outcome.is_none())
            .map(|(m, _, _)| m.site.clone())
            .collect(),
        missing_benchmarks: rows
            .iter()
            .filter(|(_, _, b)| b.is_none())
            .map(|(m, _, _)| m.site.clone())
            .collect(),
        mean_difference_bounds: None,
        arithmetic_refusal: None,
    };
    if rows.is_empty() || !result.missing_benchmarks.is_empty() {
        return result;
    }
    let bounds = (|| -> Result<Option<(Rat, Rat)>> {
        let (mut lower, mut upper) = (Rat::from_integer(0), Rat::from_integer(0));
        for (member, forecast, baseline) in &rows {
            let (b_lo, b_hi) =
                baseline.ok_or_else(|| AssessmentError("missing comparator".into()))?;
            let (lo, hi) = match member.outcome {
                Some(actual) => {
                    let forecast_loss = member.interval_score.ok_or_else(|| {
                        AssessmentError("resolved forecast has no interval score".into())
                    })?;
                    let baseline_loss = exact(interval_score(b_lo, b_hi, alpha, actual))?;
                    let difference = exact(forecast_loss.checked_sub(baseline_loss))?;
                    (difference, difference)
                }
                None => interval_score_difference_bounds(
                    (forecast.lo, forecast.hi),
                    (b_lo, b_hi),
                    alpha,
                    alpha,
                )?,
            };
            lower = exact(lower.checked_add(lo))?;
            upper = exact(upper.checked_add(hi))?;
        }
        Ok(match (mean(lower, rows.len())?, mean(upper, rows.len())?) {
            (Some(lo), Some(hi)) => Some((lo, hi)),
            _ => None,
        })
    })();
    // This optional completion must not erase valid observed means or the other baseline's bounds.
    match bounds {
        Ok(bounds) => result.mean_difference_bounds = bounds,
        Err(error) => result.arithmetic_refusal = Some(error.to_string()),
    };
    result
}

/// Exact descriptive losses on one identical member set. Negative differences favor the forecast
/// under the frozen interval loss; they do not establish predictive skill or calibration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetComparison {
    pub target: String,
    pub n_frozen: usize,
    pub common_members: Vec<String>,
    pub exclusions: Vec<ComparisonExclusion>,
    pub mean_forecast_score: Option<Rat>,
    pub mean_issuer_guidance_score: Option<Rat>,
    pub mean_run_rate_persistence_score: Option<Rat>,
    pub forecast_minus_issuer_guidance: Option<Rat>,
    pub forecast_minus_run_rate_persistence: Option<Rat>,
    pub issuer_guidance_completion: ComparisonCompletion,
    pub run_rate_persistence_completion: ComparisonCompletion,
}

/// Compare all three intervals only where the actual and both comparators are available. Every live
/// forecast must have exactly one explicit benchmark row, including rows whose intervals are absent.
/// Interval order is validated even without an actual; signed and zero comparator bounds are valid.
/// The ordinary assessment retains the full universe and its not-published members separately.
pub fn compare(
    rules: &RecordRules,
    forecasts: &[ForecastRow],
    outcomes: &[OutcomeRow],
    benchmarks: &[BenchmarkRow],
) -> Result<Vec<TargetComparison>> {
    let assessment = assess(rules, forecasts, outcomes)?;
    compare_assessment(rules, forecasts, &assessment, benchmarks)
}

// The paired report already holds the validated full-population assessment. Share this comparison
// rather than re-evaluating its model losses and certified coverage bounds.
fn compare_assessment(
    rules: &RecordRules,
    forecasts: &[ForecastRow],
    assessment: &RecordScore,
    benchmarks: &[BenchmarkRow],
) -> Result<Vec<TargetComparison>> {
    let by_forecast: BTreeMap<_, _> = forecasts
        .iter()
        .map(|f| ((f.site.as_str(), f.target.as_str()), f))
        .collect();
    let forecast_keys: BTreeSet<_> = forecasts
        .iter()
        .map(|f| (f.site.as_str(), f.target.as_str()))
        .collect();
    let mut by_member = BTreeMap::new();
    for benchmark in benchmarks {
        let key = (benchmark.site.as_str(), benchmark.target.as_str());
        if !forecast_keys.contains(&key) || by_member.insert(key, benchmark).is_some() {
            return Err(AssessmentError(format!(
                "unknown or duplicate benchmark row for {} / {}",
                benchmark.site, benchmark.target
            )));
        }
        for (name, interval) in [
            ("issuer guidance", benchmark.issuer_guidance),
            ("run-rate persistence", benchmark.run_rate_persistence),
        ] {
            if let Some((lo, hi)) = interval
                && !le(lo, hi)?
            {
                return Err(AssessmentError(format!(
                    "inverted {name} interval for {} / {}",
                    benchmark.site, benchmark.target
                )));
            }
        }
    }
    for (site, target) in forecast_keys {
        if !by_member.contains_key(&(site, target)) {
            return Err(AssessmentError(format!(
                "missing explicit benchmark row for {site} / {target}"
            )));
        }
    }

    let zero = Rat::from_integer(0);
    let mut comparisons = Vec::new();
    for target in &assessment.targets {
        let mut common_members = Vec::new();
        let mut exclusions = Vec::new();
        let (mut forecast_sum, mut guidance_sum, mut persistence_sum) = (zero, zero, zero);
        let mut completion_rows = Vec::new();
        for member in assessment
            .members
            .iter()
            .filter(|m| m.forecasted && m.target == target.target)
        {
            let benchmark = by_member
                .get(&(member.site.as_str(), member.target.as_str()))
                .ok_or_else(|| {
                    AssessmentError(format!(
                        "missing explicit benchmark row for {} / {}",
                        member.site, member.target
                    ))
                })?;
            let forecast = by_forecast
                .get(&(member.site.as_str(), member.target.as_str()))
                .ok_or_else(|| AssessmentError("frozen member has no forecast".into()))?;
            completion_rows.push((member, *forecast, *benchmark));
            if let (Some(actual), Some((g_lo, g_hi)), Some((p_lo, p_hi))) = (
                member.outcome,
                benchmark.issuer_guidance,
                benchmark.run_rate_persistence,
            ) {
                let forecast_loss = member.interval_score.ok_or_else(|| {
                    AssessmentError(format!(
                        "resolved forecast has no interval score for {} / {}",
                        member.site, member.target
                    ))
                })?;
                let guidance_loss = exact(interval_score(g_lo, g_hi, rules.alpha, actual))?;
                let persistence_loss = exact(interval_score(p_lo, p_hi, rules.alpha, actual))?;
                forecast_sum = exact(forecast_sum.checked_add(forecast_loss))?;
                guidance_sum = exact(guidance_sum.checked_add(guidance_loss))?;
                persistence_sum = exact(persistence_sum.checked_add(persistence_loss))?;
                common_members.push(member.site.clone());
            } else {
                exclusions.push(ComparisonExclusion {
                    site: member.site.clone(),
                    missing_actual: member.outcome.is_none(),
                    missing_issuer_guidance: benchmark.issuer_guidance.is_none(),
                    missing_run_rate_persistence: benchmark.run_rate_persistence.is_none(),
                });
            }
        }
        let common_count = common_members.len();
        let issuer_guidance_completion = completion(
            rules.alpha,
            completion_rows.iter().map(|(member, forecast, benchmark)| {
                (*member, *forecast, benchmark.issuer_guidance)
            }),
        );
        let run_rate_persistence_completion = completion(
            rules.alpha,
            completion_rows.iter().map(|(member, forecast, benchmark)| {
                (*member, *forecast, benchmark.run_rate_persistence)
            }),
        );
        comparisons.push(TargetComparison {
            target: target.target.clone(),
            n_frozen: target.n_frozen,
            common_members,
            exclusions,
            mean_forecast_score: mean(forecast_sum, common_count)?,
            mean_issuer_guidance_score: mean(guidance_sum, common_count)?,
            mean_run_rate_persistence_score: mean(persistence_sum, common_count)?,
            forecast_minus_issuer_guidance: mean(
                exact(forecast_sum.checked_sub(guidance_sum))?,
                common_count,
            )?,
            forecast_minus_run_rate_persistence: mean(
                exact(forecast_sum.checked_sub(persistence_sum))?,
                common_count,
            )?,
            issuer_guidance_completion,
            run_rate_persistence_completion,
        });
    }
    Ok(comparisons)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> RecordRules {
        let mut r =
            super::super::parse_rules(include_str!("../../../../forecasts/fy2026/rules.toml"))
                .unwrap();
        r.targets.truncate(1);
        r.sites.truncate(3);
        r.sites[2].status = SiteStatus::NotPublished;
        r
    }

    fn data() -> (RecordRules, Vec<ForecastRow>, Vec<OutcomeRow>) {
        let r = rules();
        let f = r.sites[..2]
            .iter()
            .map(|s| ForecastRow {
                site: s.name.clone(),
                target: "production".into(),
                point: Rat::from_integer(100),
                lo: Rat::from_integer(90),
                hi: Rat::from_integer(110),
            })
            .collect::<Vec<_>>();
        let y = f
            .iter()
            .map(|f| OutcomeRow {
                site: f.site.clone(),
                target: f.target.clone(),
                value: Some(Rat::from_integer(120)),
            })
            .collect();
        (r, f, y)
    }

    #[test]
    fn exact_losses_keep_the_frozen_denominator_and_open_loss_bound() {
        let (r, f, mut y) = data();
        y[0].value = Some(Rat::from_integer(100));
        y[1].value = None;
        let report = assess(&r, &f, &y).unwrap();
        let s = &report.targets[0];
        assert_eq!(
            (s.n_universe, s.n_frozen, s.n_scored, s.n_unresolved),
            (3, 2, 1, 1)
        );
        assert_eq!(s.not_forecast, [r.sites[2].name.clone()]);
        assert_eq!(s.observed_mean_interval_score, Some(Rat::from_integer(20)));
        assert_eq!(
            s.coverage_completion,
            Some((Rat::new(1, 2).unwrap(), Rat::from_integer(1)))
        );
        assert_eq!(
            s.full_mean_interval_score_lower,
            Some(Rat::from_integer(20))
        );
        assert_eq!(s.full_mean_interval_score_upper, None);
        y[1].value = Some(Rat::from_integer(120));
        let report = assess(&r, &f, &y).unwrap();
        let s = &report.targets[0];
        // Width 20; the second miss is 10 and 2/alpha is 10: losses 20 and 120.
        assert_eq!(s.observed_mean_interval_score, Some(Rat::from_integer(70)));
        assert_eq!(
            s.full_mean_interval_score_lower,
            s.full_mean_interval_score_upper
        );
        assert_eq!(
            s.effective_n,
            (Rat::new(4, 3).unwrap(), Rat::from_integer(2))
        );
    }

    #[test]
    fn omitted_duplicate_and_unknown_members_cannot_shrink_the_cohort() {
        let (r, f, y) = data();
        assert!(assess(&r, &f, &y[..1]).is_err());
        assert!(assess(&r, &f[..1], &y).is_err());
        let mut duplicate = y.clone();
        duplicate.push(y[0].clone());
        assert!(assess(&r, &f, &duplicate).is_err());
        let mut unknown = y.clone();
        unknown[0].site = "not in the frozen universe".into();
        assert!(assess(&r, &f, &unknown).is_err());
        let mut invalid = f.clone();
        invalid[0].point = Rat::from_integer(200);
        assert!(assess(&r, &invalid, &y).is_err());
    }

    #[test]
    fn all_missing_is_unscored_with_vacuous_coverage_and_exact_width_floor() {
        let (r, f, mut y) = data();
        for row in &mut y {
            row.value = None;
        }
        let report = assess(&r, &f, &y).unwrap();
        let s = &report.targets[0];
        assert_eq!(s.n_unresolved, 2);
        assert_eq!(s.observed_coverage, None);
        assert_eq!(s.observed_mean_interval_score, None);
        assert_eq!(s.iid_coverage_reference.lo, Rat::from_integer(0));
        assert_eq!(s.iid_coverage_reference.hi, Rat::from_integer(1));
    }
    #[test]
    fn duplicate_or_empty_frozen_dimensions_cannot_repeat_forecast_members() {
        let (r, f, y) = data();
        let mut duplicate_site = r.clone();
        duplicate_site.sites.push(r.sites[0].clone());
        assert!(assess(&duplicate_site, &f, &y).is_err());
        let mut duplicate_target = r.clone();
        duplicate_target.targets.push(r.targets[0].clone());
        assert!(assess(&duplicate_target, &f, &y).is_err());
        let mut no_sites = r.clone();
        no_sites.sites.clear();
        assert!(assess(&no_sites, &[], &[]).is_err());
        let mut no_targets = r.clone();
        no_targets.targets.clear();
        assert!(assess(&no_targets, &[], &[]).is_err());
    }

    fn benchmarks(forecasts: &[ForecastRow]) -> Vec<BenchmarkRow> {
        forecasts
            .iter()
            .map(|f| BenchmarkRow {
                site: f.site.clone(),
                target: f.target.clone(),
                issuer_guidance: Some((Rat::from_integer(95), Rat::from_integer(105))),
                run_rate_persistence: Some((Rat::from_integer(80), Rat::from_integer(100))),
            })
            .collect()
    }

    #[test]
    fn comparisons_use_one_common_set_and_keep_every_exclusion() {
        let (mut r, mut f, mut y) = data();
        r.sites[2].status = SiteStatus::Live;
        let mut omitted = r.sites[0].clone();
        omitted.name = "Not published fixture".into();
        omitted.status = SiteStatus::NotPublished;
        r.sites.push(omitted);
        let mut third_forecast = f[0].clone();
        third_forecast.site = r.sites[2].name.clone();
        f.push(third_forecast);
        y[0].value = Some(Rat::from_integer(100));
        y.push(OutcomeRow {
            site: r.sites[2].name.clone(),
            target: r.targets[0].id.clone(),
            value: Some(Rat::from_integer(1000)),
        });
        let mut b = benchmarks(&f);
        b[1].issuer_guidance = Some((Rat::from_integer(100), Rat::from_integer(110)));
        b[1].run_rate_persistence = Some((Rat::from_integer(110), Rat::from_integer(110)));
        b[2].issuer_guidance = None;
        let comparison = compare(&r, &f, &y, &b).unwrap();
        let scored = &comparison[0];
        assert_eq!(scored.n_frozen, 3);
        assert_eq!(
            scored.common_members,
            [r.sites[0].name.clone(), r.sites[1].name.clone()]
        );
        assert_eq!(
            scored.exclusions,
            [ComparisonExclusion {
                site: r.sites[2].name.clone(),
                missing_actual: false,
                missing_issuer_guidance: true,
                missing_run_rate_persistence: false,
            }]
        );
        // The first two sites give forecast losses 20 and 120, guidance 10 and 110, and
        // persistence 20 and 100. The third site's forecast loss 8920 must not enter any mean.
        assert_eq!(scored.mean_forecast_score, Some(Rat::from_integer(70)));
        assert_eq!(
            scored.mean_issuer_guidance_score,
            Some(Rat::from_integer(60))
        );
        assert_eq!(
            scored.mean_run_rate_persistence_score,
            Some(Rat::from_integer(60))
        );
        assert_eq!(
            scored.forecast_minus_issuer_guidance,
            Some(Rat::from_integer(10))
        );
        assert_eq!(
            scored.forecast_minus_run_rate_persistence,
            Some(Rat::from_integer(10))
        );
        let full = assess(&r, &f, &y).unwrap();
        assert_eq!(
            full.targets[0].observed_mean_interval_score,
            Some(Rat::from_integer(3020))
        );
        assert_eq!(full.targets[0].not_forecast, [r.sites[3].name.clone()]);
        f.reverse();
        y.reverse();
        b.reverse();
        assert_eq!(comparison, compare(&r, &f, &y, &b).unwrap());
    }

    #[test]
    fn comparison_with_no_common_members_retains_all_missing_reasons() {
        let (r, f, mut y) = data();
        let mut b = benchmarks(&f);
        y[0].value = None;
        b[0].issuer_guidance = None;
        b[0].run_rate_persistence = None;
        b[1].run_rate_persistence = None;
        let comparison = compare(&r, &f, &y, &b).unwrap();
        let scored = &comparison[0];
        assert!(scored.common_members.is_empty());
        assert_eq!(scored.n_frozen, 2);
        assert_eq!(
            scored.exclusions,
            [
                ComparisonExclusion {
                    site: r.sites[0].name.clone(),
                    missing_actual: true,
                    missing_issuer_guidance: true,
                    missing_run_rate_persistence: true,
                },
                ComparisonExclusion {
                    site: r.sites[1].name.clone(),
                    missing_actual: false,
                    missing_issuer_guidance: false,
                    missing_run_rate_persistence: true,
                },
            ]
        );
        assert_eq!(scored.mean_forecast_score, None);
        assert_eq!(scored.mean_issuer_guidance_score, None);
        assert_eq!(scored.mean_run_rate_persistence_score, None);
        assert_eq!(scored.forecast_minus_issuer_guidance, None);
        assert_eq!(scored.forecast_minus_run_rate_persistence, None);
    }

    #[test]
    fn benchmark_rows_cannot_omit_duplicate_or_extend_the_frozen_membership() {
        let (r, f, y) = data();
        let b = benchmarks(&f);
        assert!(compare(&r, &f, &y, &b[..1]).is_err());
        let mut duplicate = b.clone();
        duplicate.push(b[0].clone());
        assert!(compare(&r, &f, &y, &duplicate).is_err());
        for (site, target) in [
            ("unknown site", f[0].target.as_str()),
            (r.sites[2].name.as_str(), f[0].target.as_str()),
            (f[0].site.as_str(), "unknown target"),
        ] {
            let mut extra = b.clone();
            extra.push(BenchmarkRow {
                site: site.into(),
                target: target.into(),
                issuer_guidance: None,
                run_rate_persistence: None,
            });
            assert!(compare(&r, &f, &y, &extra).is_err());
        }
        assert!(compare(&r, &f, &y[..1], &b).is_err());
        let missing_actuals: Vec<_> = y
            .iter()
            .cloned()
            .map(|mut row| {
                row.value = None;
                row
            })
            .collect();
        for guidance in [true, false] {
            let mut inverted = b.clone();
            let bad = Some((Rat::from_integer(1), Rat::from_integer(0)));
            if guidance {
                inverted[0].issuer_guidance = bad;
            } else {
                inverted[0].run_rate_persistence = bad;
            }
            assert!(compare(&r, &f, &missing_actuals, &inverted).is_err());
        }
    }

    #[test]
    fn signed_comparator_bounds_and_point_ranges_are_valid() {
        let (mut r, mut f, mut y) = data();
        r.sites.truncate(1);
        f.truncate(1);
        y.truncate(1);
        y[0].value = Some(Rat::from_integer(-5));
        let b = [BenchmarkRow {
            site: f[0].site.clone(),
            target: f[0].target.clone(),
            issuer_guidance: Some((Rat::from_integer(-10), Rat::from_integer(0))),
            run_rate_persistence: Some((Rat::from_integer(-5), Rat::from_integer(-5))),
        }];
        let comparison = compare(&r, &f, &y, &b).unwrap();
        let scored = &comparison[0];
        assert_eq!(scored.common_members, [f[0].site.clone()]);
        assert!(scored.exclusions.is_empty());
        assert_eq!(scored.mean_forecast_score, Some(Rat::from_integer(970)));
        assert_eq!(
            scored.mean_issuer_guidance_score,
            Some(Rat::from_integer(10))
        );
        assert_eq!(
            scored.mean_run_rate_persistence_score,
            Some(Rat::from_integer(0))
        );
        assert_eq!(
            scored.forecast_minus_issuer_guidance,
            Some(Rat::from_integer(960))
        );
        assert_eq!(
            scored.forecast_minus_run_rate_persistence,
            Some(Rat::from_integer(970))
        );
    }

    #[test]
    fn completion_knots_include_inner_endpoints_and_constant_tails() {
        let r = Rat::from_integer;
        let alpha = Rat::new(1, 5).unwrap();
        for (f, b, expected) in [
            ((90, 110), (95, 105), (-40, 10)),
            ((90, 110), (100, 120), (-100, 100)),
            ((0, 0), (5, 5), (-50, 50)),
        ] {
            let f = (r(f.0), r(f.1));
            let b = (r(b.0), r(b.1));
            let bounds = interval_score_difference_bounds(f, b, alpha, alpha).unwrap();
            assert_eq!(bounds, (r(expected.0), r(expected.1)));
            assert_eq!(
                interval_score_difference_bounds(b, f, alpha, alpha).unwrap(),
                (r(-expected.1), r(-expected.0))
            );
        }
        // Enumerate signed, overlapping, disjoint and point intervals. Direct losses at a half-step
        // grid attain both extrema; independent tail points verify cancellation outside all knots.
        let intervals: Vec<_> = (-2..=2)
            .flat_map(|lo| (lo..=2).map(move |hi| (r(lo), r(hi))))
            .collect();
        for &f in &intervals {
            for &b in &intervals {
                let (lo, hi) = interval_score_difference_bounds(f, b, alpha, alpha).unwrap();
                let difference = |y| {
                    interval_score(f.0, f.1, alpha, y)
                        .unwrap()
                        .checked_sub(interval_score(b.0, b.1, alpha, y).unwrap())
                        .unwrap()
                };
                let values: Vec<_> = (-12..=12)
                    .map(|i| difference(Rat::new(i, 2).unwrap()))
                    .collect();
                assert!(
                    values
                        .iter()
                        .all(|&v| le(lo, v).unwrap() && le(v, hi).unwrap())
                );
                assert!(values.contains(&lo) && values.contains(&hi));
                assert_eq!(difference(r(-10)), difference(r(-11)));
                assert_eq!(difference(r(10)), difference(r(11)));
            }
        }
    }

    #[test]
    fn pending_outcomes_can_reverse_an_observed_lead_or_leave_it_robust() {
        let (r, f, mut y) = data();
        let int = Rat::from_integer;
        y[0].value = Some(int(100));
        y[1].value = None;
        let mut b = benchmarks(&f);
        for row in &mut b {
            row.issuer_guidance = Some((int(80), int(120)));
        }
        let report = compare(&r, &f, &y, &b).unwrap();
        let score = &report[0];
        assert_eq!(score.forecast_minus_issuer_guidance, Some(int(-20)));
        assert_eq!(
            score.issuer_guidance_completion.mean_difference_bounds,
            Some((int(-20), int(30)))
        );
        assert_eq!(
            score.issuer_guidance_completion.pending_outcomes,
            [f[1].site.clone()]
        );
        assert_eq!(score.n_frozen, 2);
        assert_eq!(
            assess(&r, &f, &y).unwrap().targets[0].full_mean_interval_score_upper,
            None
        );

        b[0].issuer_guidance = Some((int(0), int(200)));
        let robust = compare(&r, &f, &y, &b).unwrap();
        assert_eq!(
            robust[0].issuer_guidance_completion.mean_difference_bounds,
            Some((int(-100), int(-50)))
        );
        y[1].value = Some(int(120));
        let resolved = compare(&r, &f, &y, &b).unwrap();
        assert_eq!(
            resolved[0]
                .issuer_guidance_completion
                .mean_difference_bounds,
            Some((int(-50), int(-50)))
        );
        assert_eq!(resolved[0].forecast_minus_issuer_guidance, Some(int(-50)));
        assert!(
            resolved[0]
                .issuer_guidance_completion
                .pending_outcomes
                .is_empty()
        );
    }

    #[test]
    fn missing_comparator_keeps_the_full_denominator_and_other_completion_available() {
        let (r, f, mut y) = data();
        y[1].value = None;
        let mut b = benchmarks(&f);
        for (row, forecast) in b.iter_mut().zip(&f) {
            row.issuer_guidance = Some((forecast.lo, forecast.hi));
            row.run_rate_persistence = Some((forecast.lo, forecast.hi));
        }
        b[1].issuer_guidance = None;
        let score = compare(&r, &f, &y, &b).unwrap().remove(0);
        assert_eq!(score.n_frozen, 2);
        assert_eq!(
            score.issuer_guidance_completion.mean_difference_bounds,
            None
        );
        assert_eq!(
            score.issuer_guidance_completion.missing_benchmarks,
            [f[1].site.clone()]
        );
        let zero = Rat::from_integer(0);
        assert_eq!(
            score.run_rate_persistence_completion.mean_difference_bounds,
            Some((zero, zero))
        );
        b[1].issuer_guidance = b[1].run_rate_persistence;
        y[0].value = None;
        let all_pending = compare(&r, &f, &y, &b).unwrap().remove(0);
        assert!(all_pending.common_members.is_empty());
        assert_eq!(all_pending.mean_forecast_score, None);
        assert_eq!(
            all_pending
                .issuer_guidance_completion
                .pending_outcomes
                .len(),
            2
        );
        assert_eq!(
            all_pending
                .issuer_guidance_completion
                .mean_difference_bounds,
            Some((zero, zero))
        );
    }

    #[test]
    fn completion_overflow_is_local_and_never_discards_existing_observed_losses() {
        let (r, f, mut y) = data();
        y[0].value = Some(Rat::from_integer(100));
        y[1].value = None;
        let mut b = benchmarks(&f);
        for (row, forecast) in b.iter_mut().zip(&f) {
            row.issuer_guidance = Some((forecast.lo, forecast.hi));
            row.run_rate_persistence = Some((forecast.lo, forecast.hi));
        }
        let huge = Rat::from_integer(i128::MAX);
        b[1].issuer_guidance = Some((huge, huge));
        let score = compare(&r, &f, &y, &b).unwrap().remove(0);
        assert_eq!(score.mean_forecast_score, Some(Rat::from_integer(20)));
        assert_eq!(score.mean_issuer_guidance_score, score.mean_forecast_score);
        assert_eq!(
            score.mean_run_rate_persistence_score,
            score.mean_forecast_score
        );
        assert!(
            score
                .issuer_guidance_completion
                .arithmetic_refusal
                .is_some()
        );
        assert_eq!(
            score.issuer_guidance_completion.mean_difference_bounds,
            None
        );
        assert_eq!(
            score.issuer_guidance_completion.pending_outcomes,
            [f[1].site.clone()]
        );
        assert_eq!(
            score.run_rate_persistence_completion.mean_difference_bounds,
            Some((Rat::from_integer(0), Rat::from_integer(0)))
        );
    }

    #[test]
    fn completion_bounds_scale_translate_and_ignore_input_permutations() {
        let (r, mut f, mut y) = data();
        y[1].value = None;
        let mut b = benchmarks(&f);
        let original = compare(&r, &f, &y, &b).unwrap();
        f.reverse();
        y.reverse();
        b.reverse();
        assert_eq!(compare(&r, &f, &y, &b).unwrap(), original);
        let scale = Rat::new(3, 2).unwrap();
        let transform = |v: Rat| {
            v.checked_mul(scale)
                .unwrap()
                .checked_add(Rat::from_integer(7))
                .unwrap()
        };
        for row in &mut f {
            row.point = transform(row.point);
            row.lo = transform(row.lo);
            row.hi = transform(row.hi);
        }
        for row in &mut y {
            row.value = row.value.map(transform);
        }
        for row in &mut b {
            row.issuer_guidance = row
                .issuer_guidance
                .map(|(lo, hi)| (transform(lo), transform(hi)));
            row.run_rate_persistence = row
                .run_rate_persistence
                .map(|(lo, hi)| (transform(lo), transform(hi)));
        }
        let changed = compare(&r, &f, &y, &b).unwrap();
        for (old, new) in [
            (
                &original[0].issuer_guidance_completion,
                &changed[0].issuer_guidance_completion,
            ),
            (
                &original[0].run_rate_persistence_completion,
                &changed[0].run_rate_persistence_completion,
            ),
        ] {
            assert_eq!(
                new.mean_difference_bounds,
                old.mean_difference_bounds.map(|(lo, hi)| (
                    lo.checked_mul(scale).unwrap(),
                    hi.checked_mul(scale).unwrap()
                ))
            );
            assert_eq!(new.pending_outcomes, old.pending_outcomes);
        }
    }

    #[test]
    fn completion_primitive_refuses_unequal_alpha_invalid_inputs_and_overflow() {
        let r = Rat::from_integer;
        let alpha = Rat::new(1, 5).unwrap();
        assert!(
            interval_score_difference_bounds(
                (r(0), r(1)),
                (r(0), r(1)),
                alpha,
                Rat::new(1, 4).unwrap()
            )
            .is_err()
        );
        for invalid in [r(-1), r(0), r(1), r(2)] {
            assert!(
                interval_score_difference_bounds((r(0), r(1)), (r(0), r(1)), invalid, invalid)
                    .is_err()
            );
        }
        for (f, b) in [
            ((1, 0), (0, 1)),
            ((0, 1), (1, 0)),
            ((0, 1), (i128::MAX, i128::MAX)),
        ] {
            assert!(
                interval_score_difference_bounds((r(f.0), r(f.1)), (r(b.0), r(b.1)), alpha, alpha)
                    .is_err()
            );
        }
    }
}
