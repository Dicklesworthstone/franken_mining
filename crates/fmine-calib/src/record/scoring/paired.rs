//! Exact scoring of a frozen public forecast record (BD-076, plan §21.6).
//! The adapter owns source selection, citations, basis transformations and verification of the record's time
//! proof. This kernel keeps every declared member, compares methods on one paired population, and never
//! promotes a descriptive comparison to evidence of calibration. Results use frozen rules order.
//! Determinism: Exact

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Write as _};

use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::exact_feas::Interval;

use crate::CalibrationFlag;
use crate::guidance_scores::clopper_pearson_exact;
use crate::record::{
    BaselineKind, ForecastRow, Quarter, Rat, RecordRules, RulesSite, SiteStatus, Target,
};

const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
pub const CP_ASSUMPTION: &str = "Independent Bernoulli hits among this method's scored members only; no adjustment for site or year dependence, unavailable baselines, selective missingness, or outcome rounding.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability<T> {
    Available(T),
    /// A named unavailable quantity, never an omitted row or an imputed zero.
    Unavailable(String),
}

impl<T> Availability<T> {
    fn available(&self) -> Option<&T> {
        match self {
            Self::Available(value) => Some(value),
            Self::Unavailable(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub lo: Rat,
    pub hi: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeRow {
    pub site: String,
    pub target: String,
    pub value: Availability<Rat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineRow {
    pub site: String,
    pub target: String,
    pub baseline: String,
    pub value: Availability<Bounds>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Score {
    pub interval_score: Rat,
    pub inside: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoredBaseline {
    pub baseline: String,
    pub value: Availability<Bounds>,
    pub score: Option<Score>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAssessment {
    pub site: String,
    pub target: String,
    pub status: SiteStatus,
    pub outcome: Availability<Rat>,
    pub forecast: Option<ForecastRow>,
    pub forecast_score: Option<Score>,
    pub baselines: Vec<ScoredBaseline>,
    /// True only when this outcome, its forecast and every declared baseline have a score.
    pub paired: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSummary {
    pub method: String,
    pub n_available: usize,
    pub n_scored: usize,
    pub n_unscored: usize,
    pub n_inside: usize,
    pub mean_interval_score: Option<Rat>,
    pub observed_coverage: Option<Rat>,
    /// Bounds for hypothetical completion of the frozen live cohort, not a confidence interval.
    pub whole_cohort_coverage: Option<Interval>,
    /// Resolved members only, under the explicit iid Bernoulli reference assumption.
    pub iid_reference_interval: Option<Interval>,
    /// Sensitivity n / (1 + (n - 1) rho) over the frozen range, not an estimate of dependence.
    pub effective_n: Interval,
    /// Every method's paired mean uses precisely the same all-method-available members.
    pub paired_mean_interval_score: Option<Rat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSummary {
    pub target: String,
    pub n_members: usize,
    pub n_live: usize,
    pub n_not_published: usize,
    pub n_resolved: usize,
    pub n_unresolved: usize,
    pub n_paired: usize,
    /// Differences on the common comparison set; negative values favor the forecast loss.
    pub forecast_minus_issuer_guidance: Option<Rat>,
    pub forecast_minus_run_rate_persistence: Option<Rat>,
    /// Forecast first, then the target's baselines in frozen rules order.
    pub methods: Vec<MethodSummary>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecordAssessment {
    pub record_id: String,
    pub rules_id: String,
    pub alpha: Rat,
    pub coverage_level: Rat,
    pub rho_low: Rat,
    pub rho_high: Rat,
    pub calibration_claim_min_n: u64,
    /// Site-major, target-minor order from the frozen rules, including NotPublished sites.
    pub members: Vec<MemberAssessment>,
    pub targets: Vec<TargetSummary>,
    pub flag: CalibrationFlag,
}

fn invalid(why: impl Display) -> Refusal {
    Refusal::new(
        RefusalCode::UndeclaredUnitBasisOrTz107,
        format!("forecast record: {why}"),
    )
}

fn domain(why: impl Display) -> Refusal {
    Refusal::new(
        RefusalCode::OutsidePhysicalDomain109,
        format!("forecast record: {why}"),
    )
}

fn exact<T, E: Display>(result: Result<T, E>) -> Result<T, Refusal> {
    result.map_err(|e| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!("forecast record: {e}"),
        )
    })
}

fn le(left: Rat, right: Rat) -> Result<bool, Refusal> {
    Ok(exact(left.checked_cmp(right))? != Ordering::Greater)
}

fn fraction(numerator: usize, denominator: usize) -> Result<Rat, Refusal> {
    exact(Rat::new(numerator as i128, denominator as i128))
}

fn closed(lo: Rat, hi: Rat) -> Interval {
    Interval {
        lo,
        hi,
        lo_strict: false,
        hi_strict: false,
    }
}

fn validate_rules(rules: &RecordRules) -> Result<(), Refusal> {
    if rules.record_id.trim().is_empty()
        || rules.rules_id.trim().is_empty()
        || rules.issuer.trim().is_empty()
        || rules.targets.is_empty()
        || rules.sites.is_empty()
        || rules.calibration_claim_min_n == 0
    {
        return Err(invalid(
            "rules need named record, issuer, nonempty targets/sites and a positive calibration threshold",
        ));
    }
    for value in [rules.alpha, rules.coverage_level] {
        if value.signum() <= 0 || le(ONE, value)? {
            return Err(domain("alpha and coverage_level must lie in (0, 1)"));
        }
    }
    if rules.rho_low.signum() < 0 || !le(rules.rho_low, rules.rho_high)? || le(ONE, rules.rho_high)?
    {
        return Err(domain(
            "correlation range must satisfy 0 <= rho_low <= rho_high < 1",
        ));
    }
    let mut targets = BTreeSet::new();
    for target in &rules.targets {
        if target.id.trim().is_empty()
            || target.field.trim().is_empty()
            || target.quantity.trim().is_empty()
            || target.scored_quantity.trim().is_empty()
            || !targets.insert(target.id.as_str())
        {
            return Err(invalid("rules contain an empty or duplicate target"));
        }
    }
    let mut sites = BTreeSet::new();
    for site in &rules.sites {
        if site.name.trim().is_empty()
            || site.locator.trim().is_empty()
            || !sites.insert(site.name.as_str())
        {
            return Err(invalid("rules contain an empty or duplicate site"));
        }
        match (site.status, site.reason.as_deref()) {
            (SiteStatus::Live, None) => {}
            (SiteStatus::NotPublished, Some(reason)) if !reason.trim().is_empty() => {}
            _ => {
                return Err(invalid(format!(
                    "{}: reason must be present exactly for NotPublished",
                    site.name
                )));
            }
        }
    }
    let mut baselines = BTreeSet::new();
    for baseline in &rules.baselines {
        if !targets.contains(baseline.target.as_str())
            || !baselines.insert((baseline.target.as_str(), baseline.id.as_str()))
        {
            return Err(invalid(
                "rules contain a duplicate baseline or an unknown baseline target",
            ));
        }
        match baseline.kind {
            BaselineKind::IssuerGuidance if baseline.id == "issuer_guidance" => {}
            BaselineKind::Persistence {
                pool_from,
                pool_through,
                point,
            } if baseline.id == "run_rate_persistence" => {
                if !(pool_from < pool_through && pool_through < point)
                    || [pool_from, pool_through, point]
                        .iter()
                        .any(|q| Quarter::parse(&q.to_string()) != Some(*q))
                {
                    return Err(invalid(
                        "persistence quarters must be valid and satisfy pool_from < pool_through < point",
                    ));
                }
            }
            _ => return Err(invalid("baseline id does not match its declared kind")),
        }
    }
    if targets.iter().any(|target| {
        !["issuer_guidance", "run_rate_persistence"]
            .iter()
            .all(|id| baselines.contains(&(*target, *id)))
    }) {
        return Err(invalid(
            "every target must declare issuer_guidance and run_rate_persistence baselines",
        ));
    }
    Ok(())
}

fn member<'a>(
    rules: &'a RecordRules,
    site: &str,
    target: &str,
) -> Result<(&'a RulesSite, &'a Target), Refusal> {
    let site = rules
        .sites
        .iter()
        .find(|s| s.name == site)
        .ok_or_else(|| invalid(format!("unknown site {site:?}")))?;
    let target = rules
        .targets
        .iter()
        .find(|t| t.id == target)
        .ok_or_else(|| invalid(format!("unknown target {target:?}")))?;
    Ok((site, target))
}

fn availability<T>(value: &Availability<T>, status: SiteStatus) -> Result<(), Refusal> {
    match value {
        Availability::Unavailable(reason) if reason.trim().is_empty() => {
            Err(invalid("unavailable values need a nonempty reason"))
        }
        Availability::Available(_) if status == SiteStatus::NotPublished => Err(invalid(
            "a frozen NotPublished site cannot acquire a scored value",
        )),
        _ => Ok(()),
    }
}

fn bounds(bounds: Bounds) -> Result<(), Refusal> {
    if !le(bounds.lo, bounds.hi)? {
        return Err(domain("interval lower bound exceeds its upper bound"));
    }
    Ok(())
}

fn score(interval: Bounds, alpha: Rat, outcome: Rat) -> Result<Score, Refusal> {
    let (interval_score, inside) = exact(super::interval_result(
        interval.lo,
        interval.hi,
        alpha,
        outcome,
    ))?;
    Ok(Score {
        interval_score,
        inside,
    })
}

/// Score exactly the declared population. A missing input row refuses: unavailable data must have its own
/// named row. Forecasts cover live members only; every site/target has an outcome and each declared baseline.
/// Signed FY-minus-9M residuals, net unit-cost credits and negative implied guidance bounds are retained.
/// The frozen subtraction rule is not a physical Q4 observation; adapters validate its underlying totals
/// and flag inconsistent original releases rather than excluding a miss after seeing its sign.
pub fn assess(
    rules: &RecordRules,
    forecasts: &[ForecastRow],
    outcomes: &[OutcomeRow],
    baselines: &[BaselineRow],
) -> Result<RecordAssessment, Refusal> {
    validate_rules(rules)?;
    let mut forecast_map = BTreeMap::new();
    for row in forecasts {
        let (site, _) = member(rules, &row.site, &row.target)?;
        if site.status != SiteStatus::Live {
            return Err(invalid(format!(
                "{}: no forecast is permitted for a frozen NotPublished site",
                row.site
            )));
        }
        if row.lo.signum() < 0 || !le(row.lo, row.point)? || !le(row.point, row.hi)? {
            return Err(domain(format!(
                "{} / {}: forecast needs 0 <= lower <= point <= upper",
                row.site, row.target
            )));
        }
        if forecast_map
            .insert((row.site.as_str(), row.target.as_str()), row)
            .is_some()
        {
            return Err(invalid(format!(
                "duplicate forecast for {} / {}",
                row.site, row.target
            )));
        }
    }
    let mut outcome_map = BTreeMap::new();
    for row in outcomes {
        let (site, _) = member(rules, &row.site, &row.target)?;
        availability(&row.value, site.status)?;
        if outcome_map
            .insert((row.site.as_str(), row.target.as_str()), row)
            .is_some()
        {
            return Err(invalid(format!(
                "duplicate outcome for {} / {}",
                row.site, row.target
            )));
        }
    }
    let mut baseline_map = BTreeMap::new();
    for row in baselines {
        let (site, _) = member(rules, &row.site, &row.target)?;
        if !rules
            .baselines
            .iter()
            .any(|b| b.target == row.target && b.id == row.baseline)
        {
            return Err(invalid(format!(
                "undeclared baseline {} for {}",
                row.baseline, row.target
            )));
        }
        availability(&row.value, site.status)?;
        if let Availability::Available(value) = row.value {
            bounds(value)?;
        }
        if baseline_map
            .insert(
                (
                    row.site.as_str(),
                    row.target.as_str(),
                    row.baseline.as_str(),
                ),
                row,
            )
            .is_some()
        {
            return Err(invalid(format!(
                "duplicate {} baseline for {} / {}",
                row.baseline, row.site, row.target
            )));
        }
    }
    let mut members = Vec::new();
    for site in &rules.sites {
        for target in &rules.targets {
            let key = (site.name.as_str(), target.id.as_str());
            let outcome = outcome_map.get(&key).ok_or_else(|| {
                invalid(format!(
                    "missing outcome row for {} / {}",
                    site.name, target.id
                ))
            })?;
            let forecast = if site.status == SiteStatus::Live {
                Some(*forecast_map.get(&key).ok_or_else(|| {
                    invalid(format!(
                        "missing forecast for {} / {}",
                        site.name, target.id
                    ))
                })?)
            } else {
                None
            };
            let mut scored_baselines = Vec::new();
            for baseline in rules.baselines.iter().filter(|b| b.target == target.id) {
                let input = baseline_map
                    .get(&(key.0, key.1, baseline.id.as_str()))
                    .ok_or_else(|| {
                        invalid(format!(
                            "missing {} baseline for {} / {}",
                            baseline.id, site.name, target.id
                        ))
                    })?;
                let scored = match (input.value.available(), outcome.value.available()) {
                    (Some(interval), Some(y)) => Some(score(*interval, rules.alpha, *y)?),
                    _ => None,
                };
                scored_baselines.push(ScoredBaseline {
                    baseline: baseline.id.clone(),
                    value: input.value.clone(),
                    score: scored,
                });
            }
            members.push(MemberAssessment {
                site: site.name.clone(),
                target: target.id.clone(),
                status: site.status,
                outcome: outcome.value.clone(),
                forecast: forecast.cloned(),
                forecast_score: None,
                baselines: scored_baselines,
                paired: false,
            });
        }
    }
    // The existing forecast-only kernel remains the sole owner of model scoring and its frozen cohort.
    let model_outcomes: Vec<_> = members
        .iter()
        .filter(|m| m.status == SiteStatus::Live)
        .map(|m| super::OutcomeRow {
            site: m.site.clone(),
            target: m.target.clone(),
            value: m.outcome.available().copied(),
        })
        .collect();
    let model = exact(super::assess(rules, forecasts, &model_outcomes))?;
    let benchmarks = members
        .iter()
        .filter(|m| m.status == SiteStatus::Live)
        .map(|member| {
            let interval = |name: &str| {
                member
                    .baselines
                    .iter()
                    .find(|b| b.baseline == name)
                    .map(|b| b.value.available().map(|b| (b.lo, b.hi)))
                    .ok_or_else(|| invalid("assessment lost a declared comparison baseline"))
            };
            Ok(super::BenchmarkRow {
                site: member.site.clone(),
                target: member.target.clone(),
                issuer_guidance: interval("issuer_guidance")?,
                run_rate_persistence: interval("run_rate_persistence")?,
            })
        })
        .collect::<Result<Vec<_>, Refusal>>()?;
    let comparisons = exact(super::compare_assessment(
        rules,
        forecasts,
        &model,
        &benchmarks,
    ))?;
    let paired_members: BTreeSet<_> = comparisons
        .iter()
        .flat_map(|comparison| {
            comparison
                .common_members
                .iter()
                .map(move |site| (site.as_str(), comparison.target.as_str()))
        })
        .collect();
    let model_members: BTreeMap<_, _> = model
        .members
        .iter()
        .map(|m| ((m.site.as_str(), m.target.as_str()), m))
        .collect();
    for member in &mut members {
        let scored = model_members
            .get(&(member.site.as_str(), member.target.as_str()))
            .ok_or_else(|| invalid("model assessment lost a declared member"))?;
        member.forecast_score = match (scored.interval_score, scored.covered) {
            (Some(interval_score), Some(inside)) => Some(Score {
                interval_score,
                inside,
            }),
            (None, None) => None,
            _ => return Err(invalid("model assessment has an incomplete score")),
        };
        member.paired = paired_members.contains(&(member.site.as_str(), member.target.as_str()));
    }
    let mut targets = Vec::new();
    for target in &rules.targets {
        let live: Vec<_> = members
            .iter()
            .filter(|m| m.target == target.id && m.status == SiteStatus::Live)
            .collect();
        let comparison = comparisons
            .iter()
            .find(|c| c.target == target.id)
            .ok_or_else(|| invalid("comparison lost a declared target"))?;
        let n_paired = comparison.common_members.len();
        let model = model
            .targets
            .iter()
            .find(|t| t.target == target.id)
            .ok_or_else(|| invalid("model assessment lost a declared target"))?;
        let mut methods = vec![forecast_summary(model, comparison.mean_forecast_score)];
        for name in rules
            .baselines
            .iter()
            .filter(|b| b.target == target.id)
            .map(|b| b.id.as_str())
        {
            let paired_mean = match name {
                "issuer_guidance" => comparison.mean_issuer_guidance_score,
                "run_rate_persistence" => comparison.mean_run_rate_persistence_score,
                _ => return Err(invalid("comparison has an unsupported baseline")),
            };
            methods.push(summarize_baseline(name, &live, paired_mean, rules)?);
        }
        targets.push(TargetSummary {
            target: target.id.clone(),
            n_members: model.n_universe,
            n_live: model.n_frozen,
            n_not_published: model.not_forecast.len(),
            n_resolved: model.n_scored,
            n_unresolved: model.n_unresolved,
            n_paired,
            forecast_minus_issuer_guidance: comparison.forecast_minus_issuer_guidance,
            forecast_minus_run_rate_persistence: comparison.forecast_minus_run_rate_persistence,
            methods,
        });
    }
    let flag = if targets.iter().any(|t| t.n_resolved > 0) {
        CalibrationFlag::Descriptive
    } else {
        CalibrationFlag::Unscored
    };
    Ok(RecordAssessment {
        record_id: rules.record_id.clone(),
        rules_id: rules.rules_id.clone(),
        alpha: rules.alpha,
        coverage_level: rules.coverage_level,
        rho_low: rules.rho_low,
        rho_high: rules.rho_high,
        calibration_claim_min_n: rules.calibration_claim_min_n,
        members,
        targets,
        flag,
    })
}

fn forecast_summary(
    model: &super::TargetScore,
    paired_mean_interval_score: Option<Rat>,
) -> MethodSummary {
    MethodSummary {
        method: "forecast".into(),
        n_available: model.n_frozen,
        n_scored: model.n_scored,
        n_unscored: model.n_unresolved,
        n_inside: model.covered,
        mean_interval_score: model.observed_mean_interval_score,
        observed_coverage: model.observed_coverage,
        whole_cohort_coverage: model.coverage_completion.map(|(lo, hi)| closed(lo, hi)),
        iid_reference_interval: (model.n_scored > 0).then_some(model.iid_coverage_reference),
        effective_n: closed(model.effective_n.0, model.effective_n.1),
        paired_mean_interval_score,
    }
}

fn summarize_baseline(
    method: &str,
    members: &[&MemberAssessment],
    paired_mean_interval_score: Option<Rat>,
    rules: &RecordRules,
) -> Result<MethodSummary, Refusal> {
    let (mut n_available, mut n_scored, mut n_inside) = (0, 0, 0);
    let mut sum = ZERO;
    for member in members {
        let baseline = member
            .baselines
            .iter()
            .find(|b| b.baseline == method)
            .ok_or_else(|| invalid("assessment lost a declared baseline"))?;
        let (available, score) = (baseline.value.available().is_some(), baseline.score);
        n_available += usize::from(available);
        if let Some(score) = score {
            n_scored += 1;
            n_inside += usize::from(score.inside);
            sum = exact(sum.checked_add(score.interval_score))?;
        }
    }
    let n_unscored = members.len() - n_scored;
    let whole_cohort_coverage = if members.is_empty() {
        None
    } else {
        Some(closed(
            fraction(n_inside, members.len())?,
            fraction(n_inside + n_unscored, members.len())?,
        ))
    };
    let (observed_coverage, iid_reference_interval) = if n_scored == 0 {
        (None, None)
    } else {
        (
            Some(fraction(n_inside, n_scored)?),
            Some(clopper_pearson_exact(
                n_inside,
                n_scored,
                rules.coverage_level,
            )?),
        )
    };
    let (effective_lo, effective_hi) =
        exact(super::effective_n(n_scored, rules.rho_low, rules.rho_high))?;
    Ok(MethodSummary {
        method: method.to_string(),
        n_available,
        n_scored,
        n_unscored,
        n_inside,
        mean_interval_score: exact(super::mean(sum, n_scored))?,
        observed_coverage,
        whole_cohort_coverage,
        iid_reference_interval,
        effective_n: closed(effective_lo, effective_hi),
        paired_mean_interval_score,
    })
}

fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c < '\u{20}' => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn rat_json(value: Rat) -> String {
    format!(
        "{{\"num\":\"{}\",\"den\":\"{}\"}}",
        value.num(),
        value.den()
    )
}
fn optional_rat(value: Option<Rat>) -> String {
    value.map_or_else(|| "null".into(), rat_json)
}
fn interval_json(value: Interval) -> String {
    format!(
        "{{\"lo\":{},\"hi\":{},\"lo_strict\":false,\"hi_strict\":false}}",
        rat_json(value.lo),
        rat_json(value.hi)
    )
}
fn optional_interval(value: Option<Interval>) -> String {
    value.map_or_else(|| "null".into(), interval_json)
}
fn bounds_json(value: Bounds) -> String {
    format!(
        "{{\"lo\":{},\"hi\":{}}}",
        rat_json(value.lo),
        rat_json(value.hi)
    )
}

fn availability_json<T: Copy>(value: &Availability<T>, render: fn(T) -> String) -> String {
    match value {
        Availability::Available(value) => {
            format!("{{\"status\":\"available\",\"value\":{}}}", render(*value))
        }
        Availability::Unavailable(reason) => format!(
            "{{\"status\":\"unavailable\",\"reason\":{}}}",
            quote(reason)
        ),
    }
}

fn score_json(value: Option<Score>) -> String {
    value.map_or_else(
        || "null".into(),
        |s| {
            format!(
                "{{\"interval_score\":{},\"inside\":{}}}",
                rat_json(s.interval_score),
                s.inside
            )
        },
    )
}

impl RecordAssessment {
    /// Deterministic exact JSON. The enclosing adapter binds it to the frozen record and source citations.
    #[must_use]
    pub fn to_json(&self) -> String {
        let members = self.members.iter().map(|m| {
            let forecast = m.forecast.as_ref().map_or_else(|| "null".into(), |f| format!(
                "{{\"point\":{},\"lo\":{},\"hi\":{},\"uncalibrated\":true}}", rat_json(f.point), rat_json(f.lo), rat_json(f.hi)));
            let baselines = m.baselines.iter().map(|b| format!("{{\"baseline\":{},\"value\":{},\"score\":{}}}",
                quote(&b.baseline), availability_json(&b.value, bounds_json), score_json(b.score))).collect::<Vec<_>>().join(",");
            format!("{{\"site\":{},\"target\":{},\"status\":{},\"outcome\":{},\"forecast\":{forecast},\"forecast_score\":{},\"baselines\":[{baselines}],\"paired\":{}}}",
                quote(&m.site), quote(&m.target), quote(if m.status == SiteStatus::Live { "live" } else { "not_published" }),
                availability_json(&m.outcome, rat_json), score_json(m.forecast_score), m.paired)
        }).collect::<Vec<_>>().join(",");
        let targets = self.targets.iter().map(|t| {
            let methods = t.methods.iter().map(|m| format!(
                "{{\"method\":{},\"n_available\":{},\"n_scored\":{},\"n_unscored\":{},\"n_inside\":{},\"mean_interval_score\":{},\"observed_coverage\":{},\"whole_cohort_coverage\":{},\"iid_reference_interval\":{},\"effective_n\":{},\"paired_mean_interval_score\":{}}}",
                quote(&m.method), m.n_available, m.n_scored, m.n_unscored, m.n_inside, optional_rat(m.mean_interval_score),
                optional_rat(m.observed_coverage), optional_interval(m.whole_cohort_coverage), optional_interval(m.iid_reference_interval),
                interval_json(m.effective_n), optional_rat(m.paired_mean_interval_score))).collect::<Vec<_>>().join(",");
            format!("{{\"target\":{},\"n_members\":{},\"n_live\":{},\"n_not_published\":{},\"n_resolved\":{},\"n_unresolved\":{},\"n_paired\":{},\"forecast_minus_issuer_guidance\":{},\"forecast_minus_run_rate_persistence\":{},\"methods\":[{methods}]}}",
                quote(&t.target), t.n_members, t.n_live, t.n_not_published, t.n_resolved, t.n_unresolved, t.n_paired,
                optional_rat(t.forecast_minus_issuer_guidance), optional_rat(t.forecast_minus_run_rate_persistence))
        }).collect::<Vec<_>>().join(",");
        format!(
            "{{\"schema\":\"fmine.forecast-record.scores.v1\",\"record_id\":{},\"rules_id\":{},\"alpha\":{},\"coverage_level\":{},\"rho_low\":{},\"rho_high\":{},\"calibration_claim_min_n\":{},\"calibration_flag\":{},\"cp_assumption\":{},\"coverage_denominator\":\"frozen_live_members\",\"completion_bounds\":\"hypothetical_whole_cohort_coverage_not_confidence_intervals\",\"effective_n_assumption\":\"equicorrelation_sensitivity_over_frozen_rho_range_not_dependence_estimate\",\"members\":[{members}],\"targets\":[{targets}]}}",
            quote(&self.record_id),
            quote(&self.rules_id),
            rat_json(self.alpha),
            rat_json(self.coverage_level),
            rat_json(self.rho_low),
            rat_json(self.rho_high),
            self.calibration_claim_min_n,
            quote(if self.flag == CalibrationFlag::Unscored {
                "unscored"
            } else {
                "descriptive"
            }),
            quote(CP_ASSUMPTION)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{BaselineRule, Estimator, Outcome, RecordDates};

    fn r(n: i128, d: i128) -> Rat {
        Rat::new(n, d).expect("rational")
    }
    fn i(n: i128) -> Rat {
        Rat::from_integer(n)
    }
    fn unavailable<T>(reason: &str) -> Availability<T> {
        Availability::Unavailable(reason.into())
    }
    fn interval(lo: i128, hi: i128) -> Availability<Bounds> {
        Availability::Available(Bounds {
            lo: i(lo),
            hi: i(hi),
        })
    }

    fn fixture() -> (
        RecordRules,
        Vec<ForecastRow>,
        Vec<OutcomeRow>,
        Vec<BaselineRow>,
    ) {
        let rules = RecordRules {
            rules_id: "synthetic-rules".into(),
            record_id: "synthetic-record".into(),
            issuer: "synthetic".into(),
            alpha: r(1, 5),
            estimator: Estimator::InverseEcdf,
            rho_low: ZERO,
            rho_high: r(1, 2),
            coverage_level: r(19, 20),
            calibration_claim_min_n: 35,
            dates: RecordDates {
                nowcast_from: 1,
                proof_by: 2,
            },
            targets: vec![Target {
                id: "production".into(),
                quantity: "synthetic koz at 100%".into(),
                scored_quantity: "synthetic Q4".into(),
                field: "au_prod_100_koz".into(),
                outcome: Outcome::FyMinus9m,
            }],
            basis_rules: vec![],
            baselines: vec![
                BaselineRule {
                    id: "issuer_guidance".into(),
                    target: "production".into(),
                    kind: BaselineKind::IssuerGuidance,
                    rule: "synthetic interval".into(),
                    source: "synthetic".into(),
                    line: 1,
                },
                BaselineRule {
                    id: "run_rate_persistence".into(),
                    target: "production".into(),
                    kind: BaselineKind::Persistence {
                        pool_from: Quarter { year: 2024, q: 1 },
                        pool_through: Quarter { year: 2026, q: 2 },
                        point: Quarter { year: 2026, q: 3 },
                    },
                    rule: "synthetic persistence".into(),
                    source: "synthetic".into(),
                    line: 2,
                },
            ],
            sites: ["A", "B", "C", "D"]
                .into_iter()
                .map(|name| RulesSite {
                    name: name.into(),
                    status: if name == "D" {
                        SiteStatus::NotPublished
                    } else {
                        SiteStatus::Live
                    },
                    locator: format!("synthetic:{name}"),
                    reason: (name == "D").then(|| "synthetic disposal".into()),
                })
                .collect(),
        };
        let forecasts = [("A", 40, 45, 50), ("B", 20, 22, 24), ("C", 0, 5, 10)]
            .into_iter()
            .map(|(site, lo, point, hi)| ForecastRow {
                site: site.into(),
                target: "production".into(),
                point: i(point),
                lo: i(lo),
                hi: i(hi),
            })
            .collect();
        let outcomes = [
            ("A", Availability::Available(i(52))),
            ("B", Availability::Available(i(22))),
            ("C", unavailable("pending")),
            ("D", unavailable("synthetic disposal")),
        ]
        .into_iter()
        .map(|(site, value)| OutcomeRow {
            site: site.into(),
            target: "production".into(),
            value,
        })
        .collect();
        let baselines = [
            ("A", "issuer_guidance", interval(44, 54)),
            ("A", "run_rate_persistence", interval(50, 54)),
            (
                "B",
                "issuer_guidance",
                unavailable("guidance not available"),
            ),
            ("B", "run_rate_persistence", interval(20, 26)),
            ("C", "issuer_guidance", interval(0, 10)),
            ("C", "run_rate_persistence", interval(2, 12)),
            ("D", "issuer_guidance", unavailable("synthetic disposal")),
            (
                "D",
                "run_rate_persistence",
                unavailable("synthetic disposal"),
            ),
        ]
        .into_iter()
        .map(|(site, baseline, value)| BaselineRow {
            site: site.into(),
            target: "production".into(),
            baseline: baseline.into(),
            value,
        })
        .collect();
        (rules, forecasts, outcomes, baselines)
    }

    #[test]
    fn exact_scores_pair_the_same_population_and_keep_every_declared_member() {
        let (rules, forecasts, outcomes, baselines) = fixture();
        let report = assess(&rules, &forecasts, &outcomes, &baselines).expect("scored record");
        let target = &report.targets[0];
        assert_eq!(
            (target.n_members, target.n_live, target.n_not_published),
            (4, 3, 1)
        );
        assert_eq!(
            (target.n_resolved, target.n_unresolved, target.n_paired),
            (2, 1, 1)
        );
        assert_eq!(report.members.len(), 4);
        assert_eq!(report.members[3].site, "D");
        assert_eq!(report.members[3].forecast, None);
        assert_eq!(report.members[2].outcome, unavailable("pending"));
        assert_eq!(report.flag, CalibrationFlag::Descriptive);
        // Hand answers: A = (50-40) + 10*(52-50) = 30; B = 24-20 = 4, mean 17.
        let model = &target.methods[0];
        assert_eq!(model.method, "forecast");
        assert_eq!(
            (model.n_available, model.n_scored, model.n_inside),
            (3, 2, 1)
        );
        assert_eq!(model.mean_interval_score, Some(i(17)));
        assert_eq!(model.observed_coverage, Some(r(1, 2)));
        assert_eq!(model.whole_cohort_coverage, Some(closed(r(1, 3), r(2, 3))));
        assert_eq!(model.effective_n, closed(r(4, 3), i(2)));
        // B lacks guidance, so only A enters every paired mean; its three scores are 30, 10 and 4.
        assert_eq!(model.paired_mean_interval_score, Some(i(30)));
        let guidance = &target.methods[1];
        assert_eq!(guidance.mean_interval_score, Some(i(10)));
        assert_eq!(guidance.paired_mean_interval_score, Some(i(10)));
        assert_eq!(guidance.whole_cohort_coverage, Some(closed(r(1, 3), ONE)));
        let persistence = &target.methods[2];
        assert_eq!(persistence.mean_interval_score, Some(i(5)));
        assert_eq!(persistence.paired_mean_interval_score, Some(i(4)));
        assert_eq!(target.forecast_minus_issuer_guidance, Some(i(20)));
        assert_eq!(target.forecast_minus_run_rate_persistence, Some(i(26)));
        assert_eq!(
            persistence.whole_cohort_coverage,
            Some(closed(r(2, 3), ONE))
        );
        assert_eq!(report.coverage_level, r(19, 20));
        // A single covered observation has the exact 95% lower endpoint 1/40, outward rounded.
        let cp = guidance.iid_reference_interval.expect("95% reference");
        assert!(le(cp.lo, r(1, 40)).expect("compare"));
        assert!(
            le(
                r(1, 40).checked_sub(cp.lo).expect("difference"),
                r(1, 1_i128 << 40)
            )
            .expect("compare")
        );
        assert_eq!(cp.hi, ONE);
        let json = report.to_json();
        assert!(json.contains("\"mean_interval_score\":{\"num\":\"17\",\"den\":\"1\"}"));
        assert!(json.contains("\"coverage_level\":{\"num\":\"19\",\"den\":\"20\"}"));
        assert!(json.contains("\"forecast_minus_issuer_guidance\":{\"num\":\"20\",\"den\":\"1\"}"));
        assert!(json.contains("\"coverage_denominator\":\"frozen_live_members\""));
        assert!(json.contains("guidance not available"));
    }

    #[test]
    fn input_order_does_not_change_the_frozen_report_or_its_json() {
        let (rules, mut forecasts, mut outcomes, mut baselines) = fixture();
        let before = assess(&rules, &forecasts, &outcomes, &baselines).expect("score");
        forecasts.reverse();
        outcomes.reverse();
        baselines.reverse();
        let after = assess(&rules, &forecasts, &outcomes, &baselines).expect("permutation");
        assert_eq!(before, after);
        assert_eq!(before.to_json(), after.to_json());
    }

    #[test]
    fn unavailable_outcomes_and_empty_live_population_remain_unscored() {
        let (mut rules, mut forecasts, mut outcomes, mut baselines) = fixture();
        for row in &mut outcomes {
            row.value = unavailable("pending");
        }
        let report =
            assess(&rules, &forecasts, &outcomes, &baselines).expect("unresolved population");
        assert_eq!(report.flag, CalibrationFlag::Unscored);
        assert_eq!(report.targets[0].n_paired, 0);
        for method in &report.targets[0].methods {
            assert_eq!(method.n_scored, 0);
            assert_eq!(method.n_unscored, 3);
            assert_eq!(method.mean_interval_score, None);
            assert_eq!(method.paired_mean_interval_score, None);
            assert_eq!(method.iid_reference_interval, None);
            assert_eq!(method.whole_cohort_coverage, Some(closed(ZERO, ONE)));
            assert_eq!(method.effective_n, closed(ZERO, ZERO));
        }
        rules.sites.retain(|s| s.status == SiteStatus::NotPublished);
        forecasts.clear();
        outcomes.retain(|r| r.site == "D");
        baselines.retain(|r| r.site == "D");
        let report =
            assess(&rules, &forecasts, &outcomes, &baselines).expect("only excluded member");
        assert_eq!(
            (report.targets[0].n_members, report.targets[0].n_live),
            (1, 0)
        );
        assert!(
            report.targets[0]
                .methods
                .iter()
                .all(|m| m.whole_cohort_coverage.is_none())
        );
    }

    #[test]
    fn missing_duplicate_unknown_or_frozen_excluded_inputs_refuse() {
        let (rules, forecasts, outcomes, baselines) = fixture();
        let code = |f: &[ForecastRow], o: &[OutcomeRow], b: &[BaselineRow]| {
            assess(&rules, f, o, b).expect_err("bad population").code
        };
        let mut bad = outcomes.clone();
        bad.push(outcomes[0].clone());
        assert_eq!(code(&forecasts, &bad, &baselines), "FMINE-REF-107");
        assert_eq!(
            code(&forecasts, &outcomes[..3], &baselines),
            "FMINE-REF-107"
        );
        let mut bad = outcomes.clone();
        bad[0].site = "outside universe".into();
        assert_eq!(code(&forecasts, &bad, &baselines), "FMINE-REF-107");
        let mut bad = outcomes.clone();
        bad[3].value = Availability::Available(ZERO);
        assert_eq!(code(&forecasts, &bad, &baselines), "FMINE-REF-107");
        let mut bad = baselines.clone();
        bad.push(baselines[0].clone());
        assert_eq!(code(&forecasts, &outcomes, &bad), "FMINE-REF-107");
        assert_eq!(
            code(&forecasts, &outcomes, &baselines[..7]),
            "FMINE-REF-107"
        );
        let mut bad = baselines.clone();
        bad[0].baseline = "post_hoc".into();
        assert_eq!(code(&forecasts, &outcomes, &bad), "FMINE-REF-107");
        let mut bad = baselines.clone();
        bad[0].value = unavailable(" ");
        assert_eq!(code(&forecasts, &outcomes, &bad), "FMINE-REF-107");
        let mut bad = forecasts.clone();
        bad.push(forecasts[0].clone());
        assert_eq!(code(&bad, &outcomes, &baselines), "FMINE-REF-107");
        assert_eq!(
            code(&forecasts[..2], &outcomes, &baselines),
            "FMINE-REF-107"
        );
        let mut missing_comparator = rules.clone();
        missing_comparator.baselines.pop();
        assert_eq!(
            assess(&missing_comparator, &forecasts, &outcomes, &baselines)
                .expect_err("both canonical comparator kinds are required")
                .code,
            "FMINE-REF-107"
        );
    }

    #[test]
    fn signed_frozen_residuals_and_cost_credits_are_scored_without_post_hoc_exclusion() {
        let (mut rules, mut forecasts, mut outcomes, mut baselines) = fixture();
        outcomes[0].value = Availability::Available(i(-2));
        // Original-release FY minus original-release 9M is the frozen mathematical target, even when
        // the issuer's two releases imply a negative residual. It is not a physical Q4 observation.
        let residual =
            assess(&rules, &forecasts, &outcomes, &baselines).expect("signed frozen residual");
        assert_eq!(
            residual.members[0]
                .forecast_score
                .expect("residual score")
                .interval_score,
            i(430)
        );
        assert_eq!(residual.targets[0].n_resolved, 2);
        rules.targets[0].id = "unit_cost".into();
        rules.targets[0].outcome = Outcome::Q4Qtd;
        rules.targets[0].field = "aisc_usd_per_oz".into();
        for baseline in &mut rules.baselines {
            baseline.target = "unit_cost".into();
        }
        for row in &mut forecasts {
            row.target = "unit_cost".into();
        }
        for row in &mut outcomes {
            row.target = "unit_cost".into();
        }
        for row in &mut baselines {
            row.target = "unit_cost".into();
        }
        baselines[0].value = interval(-3, -1);
        let report = assess(&rules, &forecasts, &outcomes, &baselines).expect("negative net cost");
        assert_eq!(
            report.members[0]
                .forecast_score
                .expect("forecast score")
                .interval_score,
            i(430)
        );
        assert_eq!(
            report.members[0].baselines[0].score,
            Some(Score {
                interval_score: i(2),
                inside: true
            })
        );
    }

    #[test]
    fn negative_implied_guidance_is_not_clamped_and_invalid_ranges_never_score() {
        let (mut rules, forecasts, mut outcomes, mut baselines) = fixture();
        baselines[0].value = interval(-10, -5);
        let report =
            assess(&rules, &forecasts, &outcomes, &baselines).expect("negative implied guidance");
        assert_eq!(
            report.members[0].baselines[0]
                .score
                .expect("score")
                .interval_score,
            i(575)
        );
        baselines[0].value = interval(10, 5);
        assert_eq!(
            assess(&rules, &forecasts, &outcomes, &baselines)
                .expect_err("inverted")
                .code,
            "FMINE-REF-109"
        );
        baselines[0].value = interval(40, 50);
        outcomes[0].value = Availability::Available(i(i128::MAX));
        assert_eq!(
            assess(&rules, &forecasts, &outcomes, &baselines)
                .expect_err("overflow")
                .code,
            "FMINE-REF-207"
        );
        outcomes[0].value = Availability::Available(i(52));
        rules.coverage_level = ONE;
        assert_eq!(
            assess(&rules, &forecasts, &outcomes, &baselines)
                .expect_err("confidence level")
                .code,
            "FMINE-REF-109"
        );
    }

    #[test]
    fn sample_threshold_cannot_create_a_calibration_claim() {
        let (mut rules, forecasts, mut outcomes, mut baselines) = fixture();
        rules.calibration_claim_min_n = 1;
        outcomes[2].value = Availability::Available(i(6));
        baselines[2].value = interval(20, 25);
        let report =
            assess(&rules, &forecasts, &outcomes, &baselines).expect("complete live population");
        assert_eq!(report.targets[0].n_paired, 3);
        assert_eq!(report.flag, CalibrationFlag::Descriptive);
        baselines[2].value = unavailable("missing \"range\"\nnext line");
        let json = assess(&rules, &forecasts, &outcomes, &baselines)
            .expect("unavailable")
            .to_json();
        assert!(json.contains("missing \\\"range\\\"\\u000anext line"));
        assert!(!json.contains('\n'));
    }
}
