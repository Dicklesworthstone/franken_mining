//! DESIGN (BD-067/334): draft empirical quarterly forecasts under the frozen record rules.
//! Exact quantiles and decimal export are distinct: outward rounding changes the interval that is scored.
//! The all-season ratio model's unrounded bounds equal the persistence comparator on the same inputs.
//! The explicitly seasonal model selects only transitions into the forecast's calendar quarter.
//! This kernel authenticates no sources or time, and neither registers nor issues a forecast.
//! Determinism: Exact

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use super::{
    Estimator, ForecastRow, Quarter, Rat, RecordError, RecordRules, SiteStatus, TargetMetric,
    exact, forecasts_header, parse_forecasts, quantile, ratio_quantiles,
};

pub const MODEL_ID: &str = "pooled-quarterly-ratio-empirical-v1";
pub const SEASONAL_MODEL_ID: &str = "seasonal-quarterly-ratio-empirical-v1";
pub const CHANGE_MODEL_ID: &str = "pooled-quarterly-change-empirical-v1";
pub const SEASONAL_CHANGE_MODEL_ID: &str = "seasonal-quarterly-change-empirical-v1";
pub const MAX_DECIMAL_PLACES: u32 = 12;
const MAX_SERIES: usize = 4_096;
const MAX_HISTORY_VALUES: usize = 65_536;

/// DESIGN: declared empirical assumptions, not fitted or calibrated distributions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateModel {
    PooledQuarterlyRatio,
    SeasonalQuarterlyRatio,
    PooledQuarterlyChange,
    SeasonalQuarterlyChange,
}

impl CandidateModel {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::PooledQuarterlyRatio => MODEL_ID,
            Self::SeasonalQuarterlyRatio => SEASONAL_MODEL_ID,
            Self::PooledQuarterlyChange => CHANGE_MODEL_ID,
            Self::SeasonalQuarterlyChange => SEASONAL_CHANGE_MODEL_ID,
        }
    }

    #[must_use]
    pub const fn is_additive(self) -> bool {
        matches!(
            self,
            Self::PooledQuarterlyChange | Self::SeasonalQuarterlyChange
        )
    }

    const fn is_seasonal(self) -> bool {
        matches!(
            self,
            Self::SeasonalQuarterlyRatio | Self::SeasonalQuarterlyChange
        )
    }
}

/// Additive changes have the target's unit; ratios are dimensionless. The distinction
/// is retained in the pool rather than storing unit-bearing changes as ratios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Ratio,
    Additive,
}

/// Values must already have the target's exact unit, support, ownership basis and original-release vintage.
/// `history` has one slot per frozen pool quarter, including missing slots; it must never bridge a gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateSeries {
    pub site: String,
    pub target: String,
    pub history: Vec<Option<Rat>>,
    pub point: Option<Rat>,
}

/// A pair omitted by model admission or declared seasonal selection. Both endpoints
/// remain visible, including missing, zero or negative observations; none is imputed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcludedPair {
    pub site: String,
    pub from: Quarter,
    pub to: Quarter,
    pub before: Option<Rat>,
    pub after: Option<Rat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidatePool {
    pub target: String,
    pub pool_from: Quarter,
    pub pool_through: Quarter,
    pub point_quarter: Quarter,
    pub kind: ChangeKind,
    /// Stable rules-site and quarter order; repeated changes retain their empirical mass.
    pub changes: Vec<Rat>,
    /// Missing or out-of-domain pairs within the selected season (all pairs for pooled models).
    pub excluded_pairs: Vec<ExcludedPair>,
    /// None for the all-season model; otherwise 1..4, derived from the common frozen outcome horizon.
    pub seasonal_destination_quarter: Option<u8>,
    /// Every other-season transition, retaining endpoints even when missing or nonpositive.
    pub off_season_pairs: Vec<ExcludedPair>,
    pub lower_change: Rat,
    pub median_change: Rat,
    pub upper_change: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub model_id: &'static str,
    pub alpha: Rat,
    pub decimal_places: u32,
    /// Latest observation combined with the declared ratio or additive change quantiles.
    pub exact_rows: Vec<ForecastRow>,
    /// The actual decimal rows in `csv`, retained separately because these are the quantities later scored.
    pub serialized_rows: Vec<ForecastRow>,
    pub csv: String,
    pub pools: Vec<CandidatePool>,
    pub not_published_sites: Vec<String>,
}

fn invalid(reason: impl Into<String>) -> RecordError {
    RecordError::Forecast {
        line: 0,
        reason: format!("draft candidate: {}", reason.into()),
    }
}

fn csv_label(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && !value
            .chars()
            .any(|c| c.is_control() || matches!(c, ',' | '"'))
}

/// Build all live site × target forecasts or refuse the request. The model assumes the pooled historical
/// multiplicative changes apply to each site's latest observation. Production points must be positive;
/// explicitly supported sold-metal cost targets may be zero or signed. It supplies no calibrated
/// probability, independent forecasting skill, fitted site effect, trend or ownership conversion.
///
/// The point is `x × Q(1/2)` of the ratio pool. Bounds are the ordered endpoints of
/// `x × [Q(alpha/2), Q(1-alpha/2)]`, reversing when `x` is negative. They therefore equal the frozen
/// persistence comparator before decimal rounding and receive the same interval score.
/// CSV lower bounds round down, upper bounds round up and points round to nearest, with exact ties upward.
/// The CSV always carries `uncalibrated=true` and is checked by the existing frozen-record parser.
pub fn build_candidate(
    rules: &RecordRules,
    series: &[CandidateSeries],
    decimal_places: u32,
) -> Result<Candidate, RecordError> {
    build_candidate_with_model(
        rules,
        series,
        decimal_places,
        CandidateModel::PooledQuarterlyRatio,
    )
}

/// Build with an explicit model while retaining the frozen source, site, target and unit universe.
/// DESIGN: the seasonal model assumes positive historical transitions into the same calendar quarter
/// apply to the forecast horizon. It pools across live sites and years, with no weighting, trend fit,
/// fallback to other seasons or claim that seasonality or future coverage has been established.
/// The registered all-season persistence comparator is unchanged. Exact quantiles, signed cost support
/// and outward decimal enclosure are shared with the default candidate.
/// Additive models instead pool `after - before` in each target's unit and add its quantiles to the
/// latest observation. Signed cost/capital histories are admissible, as are zero production histories;
/// missing and negative production histories are excluded without bridging. Production points still
/// require positive observations, and negative production forecasts refuse rather than clamp.
/// Pooling absolute changes across sites is an explicit exchangeability assumption, not a fitted effect.
pub fn build_candidate_with_model(
    rules: &RecordRules,
    series: &[CandidateSeries],
    decimal_places: u32,
    model: CandidateModel,
) -> Result<Candidate, RecordError> {
    let seasonal_destination_quarter = if model.is_seasonal() {
        Some(rules.outcome_quarter()?.q)
    } else {
        None
    };
    if seasonal_destination_quarter.is_none()
        && rules.targets.iter().any(|t| t.outcome.is_quarterly())
    {
        rules.outcome_quarter()?;
    }
    if decimal_places > MAX_DECIMAL_PLACES {
        return Err(invalid("decimal_places must be between zero and twelve"));
    }
    if rules.estimator != Estimator::InverseEcdf {
        return Err(RecordError::NotExact);
    }
    if rules.alpha.signum() <= 0
        || exact(rules.alpha.checked_cmp(Rat::from_integer(1)))? != Ordering::Less
    {
        return Err(RecordError::AlphaOutOfDomain);
    }
    let header = forecasts_header(rules)?;
    let targets: BTreeSet<&str> = rules.targets.iter().map(|t| t.id.as_str()).collect();
    let sites: BTreeSet<&str> = rules.sites.iter().map(|s| s.name.as_str()).collect();
    if targets.is_empty()
        || targets.len() != rules.targets.len()
        || sites.len() != rules.sites.len()
        || !targets.iter().chain(&sites).all(|s| csv_label(s))
    {
        return Err(invalid(
            "rules need unique, nonempty, CSV-safe sites and targets",
        ));
    }
    let live: Vec<&str> = rules
        .sites
        .iter()
        .filter(|s| s.status == SiteStatus::Live)
        .map(|s| s.name.as_str())
        .collect();
    let expected = live
        .len()
        .checked_mul(targets.len())
        .ok_or(RecordError::Overflow)?;
    let history_len = series.iter().try_fold(0_usize, |n, s| {
        n.checked_add(s.history.len()).ok_or(RecordError::Overflow)
    })?;
    if expected == 0
        || expected > MAX_SERIES
        || series.len() > MAX_SERIES
        || history_len > MAX_HISTORY_VALUES
    {
        return Err(invalid(
            "need 1..4096 live members and at most 65536 history values",
        ));
    }
    let mut indexed = BTreeMap::new();
    for s in series {
        if !live.contains(&s.site.as_str()) || !targets.contains(s.target.as_str()) {
            return Err(invalid(format!(
                "{} / {} is outside the live rules universe",
                s.site, s.target
            )));
        }
        if indexed
            .insert((s.site.as_str(), s.target.as_str()), s)
            .is_some()
        {
            return Err(invalid(format!(
                "duplicate {} / {} series",
                s.site, s.target
            )));
        }
        let signed = rules
            .targets
            .iter()
            .any(|target| target.id == s.target && target.permits_signed_forecast());
        let nonnegative = model.is_additive()
            && rules.targets.iter().any(|target| {
                target.id == s.target
                    && TargetMetric::from_target(target).is_some_and(TargetMetric::is_production)
            });
        if s.point
            .is_none_or(|x| !signed && (x.signum() < 0 || (x.signum() == 0 && !nonnegative)))
        {
            return Err(invalid(format!(
                "{} / {} needs a {} point-quarter observation",
                s.site,
                s.target,
                if signed {
                    "numeric"
                } else if nonnegative {
                    "nonnegative"
                } else {
                    "positive"
                }
            )));
        }
    }
    for site in &live {
        for target in &rules.targets {
            if !indexed.contains_key(&(*site, target.id.as_str())) {
                return Err(RecordError::MissingForecast {
                    site: (*site).to_string(),
                    target: target.id.clone(),
                });
            }
        }
    }
    if rules
        .baselines
        .iter()
        .any(|b| !targets.contains(b.target.as_str()))
    {
        return Err(invalid(
            "a baseline names a target outside the rules universe",
        ));
    }
    let mut pools = Vec::with_capacity(rules.targets.len());
    for target in &rules.targets {
        if model.is_additive() && TargetMetric::from_target(target).is_none() {
            return Err(invalid(format!(
                "{} needs a supported field/outcome for additive changes",
                target.id
            )));
        }
        let (pool_from, pool_through, point) = rules.persistence_pool(&target.id)?;
        let quarters = pool_from.through(pool_through);
        let mut changes = Vec::new();
        let mut excluded_pairs = Vec::new();
        let mut off_season_pairs = Vec::new();
        for site in &live {
            let s = indexed
                .get(&(*site, target.id.as_str()))
                .ok_or_else(|| invalid("missing live series after universe validation"))?;
            if s.history.len() != quarters.len() {
                return Err(invalid(format!(
                    "{} / {} has {} history slots; the frozen pool requires {}",
                    site,
                    target.id,
                    s.history.len(),
                    quarters.len()
                )));
            }
            for (dates, values) in quarters.windows(2).zip(s.history.windows(2)) {
                if seasonal_destination_quarter.is_some_and(|q| dates[1].q != q) {
                    off_season_pairs.push(ExcludedPair {
                        site: (*site).to_string(),
                        from: dates[0],
                        to: dates[1],
                        before: values[0],
                        after: values[1],
                    });
                    continue;
                }
                match (values[0], values[1]) {
                    (Some(a), Some(b))
                        if model.is_additive()
                            && (target.permits_signed_forecast()
                                || (a.signum() >= 0 && b.signum() >= 0)) =>
                    {
                        changes.push(exact(b.checked_sub(a))?);
                    }
                    (Some(a), Some(b))
                        if !model.is_additive() && a.signum() > 0 && b.signum() > 0 =>
                    {
                        changes.push(exact(b.checked_div(a))?);
                    }
                    (before, after) => excluded_pairs.push(ExcludedPair {
                        site: (*site).to_string(),
                        from: dates[0],
                        to: dates[1],
                        before,
                        after,
                    }),
                }
            }
        }
        if changes.is_empty() {
            return Err(invalid(format!(
                "{} has no {}{}",
                target.id,
                if model.is_additive() {
                    "admissible adjacent-quarter change"
                } else {
                    "positive adjacent-quarter ratio"
                },
                seasonal_destination_quarter.map_or_else(String::new, |q| format!(" into Q{q}")),
            )));
        }
        let (lower_change, upper_change) = if model.is_additive() {
            let lower_level = exact(rules.alpha.checked_div(Rat::from_integer(2)))?;
            (
                quantile(&changes, lower_level, rules.estimator)?,
                quantile(
                    &changes,
                    exact(Rat::from_integer(1).checked_sub(lower_level))?,
                    rules.estimator,
                )?,
            )
        } else {
            ratio_quantiles(&changes, rules.alpha, rules.estimator)?
        };
        let median_change = quantile(&changes, exact(Rat::new(1, 2))?, rules.estimator)?;
        pools.push(CandidatePool {
            target: target.id.clone(),
            pool_from,
            pool_through,
            point_quarter: point,
            kind: if model.is_additive() {
                ChangeKind::Additive
            } else {
                ChangeKind::Ratio
            },
            changes,
            excluded_pairs,
            seasonal_destination_quarter,
            off_season_pairs,
            lower_change,
            median_change,
            upper_change,
        });
    }
    let mut exact_rows = Vec::with_capacity(expected);
    let mut csv = format!("{header}\n");
    for site in &live {
        for pool in &pools {
            let point = indexed
                .get(&(*site, pool.target.as_str()))
                .and_then(|s| s.point)
                .ok_or_else(|| invalid("missing point observation after validation"))?;
            let (lower, upper) = if pool.kind == ChangeKind::Ratio && point.signum() < 0 {
                (pool.upper_change, pool.lower_change)
            } else {
                (pool.lower_change, pool.upper_change)
            };
            let predict = |change| {
                exact(match pool.kind {
                    ChangeKind::Ratio => point.checked_mul(change),
                    ChangeKind::Additive => point.checked_add(change),
                })
            };
            let row = ForecastRow {
                site: (*site).to_string(),
                target: pool.target.clone(),
                point: predict(pool.median_change)?,
                lo: predict(lower)?,
                hi: predict(upper)?,
            };
            if model.is_additive()
                && row.lo.signum() < 0
                && !rules
                    .targets
                    .iter()
                    .any(|target| target.id == pool.target && target.permits_signed_forecast())
            {
                return Err(invalid(format!(
                    "{} / {} additive interval leaves the nonnegative production domain",
                    site, pool.target
                )));
            }
            csv.push_str(&format!(
                "{},{},{},{},{},true\n",
                row.site,
                row.target,
                decimal(row.point, decimal_places, Round::Nearest)?,
                decimal(row.lo, decimal_places, Round::Down)?,
                decimal(row.hi, decimal_places, Round::Up)?,
            ));
            exact_rows.push(row);
        }
    }
    let serialized_rows = parse_forecasts(csv.as_bytes(), rules)?;
    Ok(Candidate {
        model_id: model.id(),
        alpha: rules.alpha,
        decimal_places,
        exact_rows,
        serialized_rows,
        csv,
        pools,
        not_published_sites: rules
            .sites
            .iter()
            .filter(|s| s.status == SiteStatus::NotPublished)
            .map(|s| s.name.clone())
            .collect(),
    })
}

#[derive(Clone, Copy)]
enum Round {
    Down,
    Nearest,
    Up,
}

fn decimal(value: Rat, places: u32, direction: Round) -> Result<String, RecordError> {
    let scale = 10_i128.checked_pow(places).ok_or(RecordError::Overflow)?;
    let scaled = exact(value.checked_mul(Rat::from_integer(scale)))?;
    // Euclidean division starts at the mathematical floor even for a signed value; a tie increments
    // toward positive infinity. No negation of a possibly minimal signed numerator is required.
    let (whole, remainder) = (
        scaled.num().div_euclid(scaled.den()),
        scaled.num().rem_euclid(scaled.den()),
    );
    // Comparing r with d-r avoids overflowing 2*r at exact half ties.
    let increment = match direction {
        Round::Down => false,
        Round::Up => remainder != 0,
        Round::Nearest => remainder >= scaled.den() - remainder,
    };
    let rounded = whole
        .checked_add(i128::from(increment))
        .ok_or(RecordError::Overflow)?;
    if places == 0 {
        Ok(rounded.to_string())
    } else {
        let sign = if rounded < 0 { "-" } else { "" };
        let magnitude = rounded.unsigned_abs();
        let scale = scale as u128;
        Ok(format!(
            "{sign}{}.{:0width$}",
            magnitude / scale,
            magnitude % scale,
            width = places as usize
        ))
    }
}

#[cfg(test)]
mod tests;
