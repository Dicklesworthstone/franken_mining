//! DESIGN (BD-067/334): a draft empirical quarterly-ratio forecast under the frozen record rules.
//! Exact quantiles and decimal export are distinct: outward rounding changes the interval that is scored.
//! The unrounded bounds equal the persistence comparator on the same inputs; only its point estimate differs.
//! This kernel authenticates no sources or time, and neither registers nor issues a forecast.
//! Determinism: Exact.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use super::{
    BaselineKind, Estimator, ForecastRow, Quarter, Rat, RecordError, RecordRules, SiteStatus,
    exact, forecasts_header, parse_forecasts, quantile, ratio_quantiles,
};

pub const MODEL_ID: &str = "pooled-quarterly-ratio-empirical-v1";
pub const MAX_DECIMAL_PLACES: u32 = 12;
const MAX_SERIES: usize = 4_096;
const MAX_HISTORY_VALUES: usize = 65_536;

/// Values must already have the target's exact unit, support, ownership basis and original-release vintage.
/// `history` has one slot per frozen pool quarter, including missing slots; it must never bridge a gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateSeries {
    pub site: String,
    pub target: String,
    pub history: Vec<Option<Rat>>,
    pub point: Option<Rat>,
}

/// A pair omitted by the frozen positive-ratio rule. Both endpoints remain visible, including zero or negative
/// observations; neither becomes a zero forecast or an imputed positive observation.
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
    /// Stable rules-site and quarter order, with repeated ratios retaining their empirical mass.
    pub ratios: Vec<Rat>,
    pub excluded_pairs: Vec<ExcludedPair>,
    pub lower_ratio: Rat,
    pub median_ratio: Rat,
    pub upper_ratio: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub model_id: &'static str,
    pub alpha: Rat,
    pub decimal_places: u32,
    /// Supplied latest observation times the inverse empirical CDF's lower, median and upper ratios.
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

fn quarter_index(q: Quarter) -> Result<i32, RecordError> {
    if !(0..=9_999).contains(&q.year) || !(1..=4).contains(&q.q) {
        return Err(invalid(
            "pool and point quarters require valid YYYYQ1..4 dates",
        ));
    }
    Ok(q.year * 4 + i32::from(q.q) - 1)
}

/// Build all live site × target forecasts or refuse the request. The model assumes the pooled historical
/// multiplicative changes apply to each site's latest positive observation. It supplies no calibrated
/// probability, independent forecasting skill, fitted site effect, trend or ownership conversion.
///
/// For observation `x`, lower/point/upper are `x × Q(alpha/2, 1/2, 1-alpha/2)` of the pool. Bounds therefore
/// equal the frozen persistence comparator before decimal rounding and receive the same interval score.
/// CSV lower bounds round down, upper bounds round up and points round to nearest, with exact ties upward.
/// The CSV always carries `uncalibrated=true` and is checked by the existing frozen-record parser.
pub fn build_candidate(
    rules: &RecordRules,
    series: &[CandidateSeries],
    decimal_places: u32,
) -> Result<Candidate, RecordError> {
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
        if s.point.is_none_or(|x| x.signum() <= 0) {
            return Err(invalid(format!(
                "{} / {} needs a positive point-quarter observation",
                s.site, s.target
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
        let mut baselines = rules.baselines.iter().filter(|b| {
            b.target == target.id && matches!(b.kind, BaselineKind::Persistence { .. })
        });
        let baseline = baselines.next().ok_or_else(|| {
            invalid(format!(
                "{} needs one frozen run_rate_persistence pool",
                target.id
            ))
        })?;
        if baseline.id != "run_rate_persistence" || baselines.next().is_some() {
            return Err(invalid(format!(
                "{} needs exactly one persistence rule",
                target.id
            )));
        }
        let BaselineKind::Persistence {
            pool_from,
            pool_through,
            point,
        } = baseline.kind
        else {
            return Err(invalid("a persistence rule must declare its pool"));
        };
        let (first, last, latest) = (
            quarter_index(pool_from)?,
            quarter_index(pool_through)?,
            quarter_index(point)?,
        );
        if first >= last || last >= latest {
            return Err(invalid(
                "pool_from < pool_through < point_quarter is required",
            ));
        }
        let quarters = pool_from.through(pool_through);
        let mut ratios = Vec::new();
        let mut excluded_pairs = Vec::new();
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
                match (values[0], values[1]) {
                    (Some(a), Some(b)) if a.signum() > 0 && b.signum() > 0 => {
                        ratios.push(exact(b.checked_div(a))?);
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
        if ratios.is_empty() {
            return Err(invalid(format!(
                "{} has no positive adjacent-quarter ratio",
                target.id
            )));
        }
        let (lower_ratio, upper_ratio) = ratio_quantiles(&ratios, rules.alpha, rules.estimator)?;
        let median_ratio = quantile(&ratios, exact(Rat::new(1, 2))?, rules.estimator)?;
        pools.push(CandidatePool {
            target: target.id.clone(),
            pool_from,
            pool_through,
            point_quarter: point,
            ratios,
            excluded_pairs,
            lower_ratio,
            median_ratio,
            upper_ratio,
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
            let row = ForecastRow {
                site: (*site).to_string(),
                target: pool.target.clone(),
                point: exact(point.checked_mul(pool.median_ratio))?,
                lo: exact(point.checked_mul(pool.lower_ratio))?,
                hi: exact(point.checked_mul(pool.upper_ratio))?,
            };
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
        model_id: MODEL_ID,
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
    if value.signum() < 0 {
        return Err(invalid("negative candidate value cannot be serialized"));
    }
    let scale = 10_i128.checked_pow(places).ok_or(RecordError::Overflow)?;
    let scaled = exact(value.checked_mul(Rat::from_integer(scale)))?;
    let (whole, remainder) = (scaled.num() / scaled.den(), scaled.num() % scaled.den());
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
        Ok(format!(
            "{}.{:0width$}",
            rounded / scale,
            rounded % scale,
            width = places as usize
        ))
    }
}

#[cfg(test)]
mod tests;
