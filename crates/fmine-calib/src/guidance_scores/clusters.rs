//! DESIGN (BD-341.S4): exact cluster summaries and a declared one-axis block bootstrap.
//! Determinism: Exact (integer resampling and inverse empirical CDF on keyed Philox draws).
//!
//! The estimand is the equally weighted mean of cluster means, not the observation-weighted
//! mean. Within-cluster dependence is retained; independence of the selected clusters is an
//! unverified assumption. Selecting years does not account for persistent site effects across
//! years, and selecting sites does not account for common year effects across sites.

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::{Refusal, RefusalCode};

use super::{Guidance, GuidanceScore, Interval, PrintedOutcome, Rat, Side};

pub const RULE: &str = "guidance.cluster-bootstrap/1";
pub const REPLICATES: usize = 1999;
/// DESIGN: a conservative admission floor, not a theorem establishing bootstrap coverage.
/// Synthetic controls exercise 40 clusters; populations below the floor remain descriptive.
pub const MIN_CLUSTERS: usize = 20;
pub const SEED: u64 = 0x0341_5c04_2026;
pub const METHOD: &str = "Equal-cluster mean, selected axis fixed by the caller. Resample the complete selected clusters with replacement using keyed Philox, retaining their members together. Two-sided nominal 90% percentile interval uses inverse empirical CDF at 0.05 and 0.95 without interpolation. At least 20 complete clusters and nonzero between-cluster variation are required; this floor does not guarantee coverage. Mean z uses declared range targets only; point and directional targets remain explicit, and a cluster without range targets is never dropped to obtain a bias interval. Independent, exchangeable selected clusters and a representative registered population are unverified assumptions. This is not a two-way cluster interval, calibrated forecast, site ranking or estimate of effective sample size. Source-rounding enclosures and finite-population missing-outcome bounds are separate from sampling uncertainty.";

const MAX_MEMBERS: usize = 4096;
const MAX_CLUSTERS: usize = 256;
const ZERO: Rat = Rat::from_integer(0);
type Result<T> = std::result::Result<T, Refusal>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Year,
    Site,
}

impl Axis {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Year => "year",
            Self::Site => "site",
        }
    }
}

/// One declared site-year, including unusable or ambiguous guidance and missing outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub id: String,
    pub site: String,
    pub year: u16,
    /// None preserves an ambiguous guidance selection without choosing an arbitrary range.
    pub guidance: Option<Guidance>,
    pub outcome: Option<PrintedOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cluster {
    pub id: String,
    pub members: Vec<String>,
    pub resolved: usize,
    pub unknown_guidance: usize,
    pub range_members: usize,
    pub resolved_ranges: usize,
    pub hit_rate: Option<Rat>,
    pub hit_completion: Interval,
    pub rounding_hit_completion: Interval,
    /// Observed range rows only; these are unavailable as whole-cluster bias estimates if incomplete.
    pub observed_mean_z: Option<Rat>,
    pub observed_mean_z_rounding: Option<Interval>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projection {
    pub point: Option<Rat>,
    pub rounding_bounds: Option<Interval>,
    pub completion_bounds: Option<Interval>,
    pub interval_90: Option<Interval>,
    pub interval_90_rounding_enclosure: Option<Interval>,
    /// Missingness, insufficient clusters and degeneracy are data, not silently smaller samples.
    pub unavailable: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterReport {
    pub selected_axis: Axis,
    pub n_members: usize,
    pub n_unresolved: usize,
    pub years: Vec<Cluster>,
    pub sites: Vec<Cluster>,
    pub hit: Projection,
    pub bias: Projection,
}

fn invalid(message: &str) -> Refusal {
    Refusal::new(
        RefusalCode::BasisMismatch105,
        format!("guidance clusters: {message}"),
    )
}

fn arithmetic() -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        "guidance clusters: exact arithmetic capacity exceeded",
    )
}

fn exact<T, E>(value: std::result::Result<T, E>) -> Result<T> {
    value.map_err(|_| arithmetic())
}

fn ratio(n: usize, d: usize) -> Result<Rat> {
    exact(Rat::new(n as i128, d as i128))
}

fn closed(lo: Rat, hi: Rat) -> Interval {
    Interval {
        lo,
        hi,
        lo_strict: false,
        hi_strict: false,
    }
}

fn sum(values: impl IntoIterator<Item = Rat>) -> Result<Rat> {
    values
        .into_iter()
        .try_fold(ZERO, |sum, value| exact(sum.checked_add(value)))
}

fn mean(values: &[Rat]) -> Result<Rat> {
    exact(sum(values.iter().copied())?.checked_div(Rat::from_integer(values.len() as i128)))
}

fn cluster(id: String, members: &[(&Member, Option<GuidanceScore>)]) -> Result<Cluster> {
    let mut resolved = 0;
    let (mut hits, mut definite, mut possible) = (0, 0, 0);
    let (mut z, mut rounding) = (Vec::new(), Vec::new());
    for (_, score) in members {
        let Some(score) = score else { continue };
        resolved += 1;
        hits += usize::from(score.side == Side::Inside);
        definite += usize::from(score.definitely_inside);
        possible += usize::from(score.possibly_inside);
        if let (Some(value), Some(bounds)) = (score.z, score.z_interval) {
            z.push(value);
            rounding.push(bounds);
        }
    }
    let missing = members.len() - resolved;
    Ok(Cluster {
        id,
        members: members.iter().map(|(m, _)| m.id.clone()).collect(),
        resolved,
        unknown_guidance: members.iter().filter(|(m, _)| m.guidance.is_none()).count(),
        range_members: members
            .iter()
            .filter(|(m, _)| matches!(m.guidance, Some(Guidance::Range { .. })))
            .count(),
        resolved_ranges: z.len(),
        hit_rate: (resolved != 0).then(|| ratio(hits, resolved)).transpose()?,
        hit_completion: closed(
            ratio(hits, members.len())?,
            ratio(hits + missing, members.len())?,
        ),
        rounding_hit_completion: closed(
            ratio(definite, members.len())?,
            ratio(possible + missing, members.len())?,
        ),
        observed_mean_z: (!z.is_empty()).then(|| mean(&z)).transpose()?,
        observed_mean_z_rounding: (!z.is_empty())
            .then(|| mean_bounds(&rounding))
            .transpose()?,
    })
}

fn mean_bounds(values: &[Interval]) -> Result<Interval> {
    Ok(Interval {
        lo: mean(&values.iter().map(|i| i.lo).collect::<Vec<_>>())?,
        hi: mean(&values.iter().map(|i| i.hi).collect::<Vec<_>>())?,
        lo_strict: values.iter().any(|i| i.lo_strict),
        hi_strict: values.iter().any(|i| i.hi_strict),
    })
}

/// Common integer coordinates avoid platform floats and repeated rational work in the resampling loop.
fn coordinates(values: &[(Rat, Interval)]) -> Result<(i128, Vec<[i128; 3]>)> {
    fn gcd(mut a: i128, mut b: i128) -> i128 {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    }
    let mut denominator = 1_i128;
    for (point, bounds) in values {
        for value in [*point, bounds.lo, bounds.hi] {
            denominator = (denominator / gcd(denominator, value.den()))
                .checked_mul(value.den())
                .ok_or_else(arithmetic)?;
        }
    }
    let mut scaled = Vec::with_capacity(values.len());
    for (point, bounds) in values {
        let scale = |r: Rat| {
            r.num()
                .checked_mul(denominator / r.den())
                .ok_or_else(arithmetic)
        };
        scaled.push([scale(*point)?, scale(bounds.lo)?, scale(bounds.hi)?]);
    }
    Ok((denominator, scaled))
}

fn bootstrap(
    values: &[(Rat, Interval)],
    axis: Axis,
    statistic: u32,
    replicates: usize,
) -> Result<(Interval, Interval)> {
    let (denominator, values) = coordinates(values)?;
    let denominator = denominator
        .checked_mul(values.len() as i128)
        .ok_or_else(arithmetic)?;
    let mut draws = PhiloxStream::new(RngKey {
        seed_root: SEED,
        partition: 341,
        domain: if axis == Axis::Year { 0 } else { 1 },
        var_group: statistic,
        realization: 0,
        brick: values.len() as u64,
    });
    let n = values.len() as u64;
    // Reject the incomplete residue class: modulo alone would give a biased cluster draw.
    let threshold = n.wrapping_neg() % n;
    let mut point = Vec::with_capacity(replicates);
    let mut lower = Vec::with_capacity(replicates);
    let mut upper = Vec::with_capacity(replicates);
    for _ in 0..replicates {
        let mut total = [0_i128; 3];
        for _ in 0..n {
            let index = loop {
                let x = draws.next_u64();
                if x >= threshold {
                    break (x % n) as usize;
                }
            };
            for (sum, value) in total.iter_mut().zip(values[index]) {
                *sum = sum.checked_add(value).ok_or_else(arithmetic)?;
            }
        }
        point.push(total[0]);
        lower.push(total[1]);
        upper.push(total[2]);
    }
    point.sort_unstable();
    lower.sort_unstable();
    upper.sort_unstable();
    let lo = replicates.div_ceil(20) - 1;
    let hi = (replicates * 19).div_ceil(20) - 1;
    let at = |values: &[i128], i| exact(Rat::new(values[i], denominator));
    Ok((
        closed(at(&point, lo)?, at(&point, hi)?),
        closed(at(&lower, lo)?, at(&upper, hi)?),
    ))
}

fn projection(
    clusters: &[Cluster],
    axis: Axis,
    bias: bool,
    replicates: usize,
) -> Result<Projection> {
    let mut result = Projection {
        point: None,
        rounding_bounds: None,
        completion_bounds: None,
        interval_90: None,
        interval_90_rounding_enclosure: None,
        unavailable: None,
    };
    if clusters.is_empty() {
        result.unavailable = Some("empty_population");
        return Ok(result);
    }
    if !bias {
        result.completion_bounds = Some(mean_bounds(
            &clusters
                .iter()
                .map(|c| c.hit_completion)
                .collect::<Vec<_>>(),
        )?);
        result.rounding_bounds = Some(mean_bounds(
            &clusters
                .iter()
                .map(|c| c.rounding_hit_completion)
                .collect::<Vec<_>>(),
        )?);
    }
    let incomplete = if bias {
        clusters
            .iter()
            .any(|c| c.unknown_guidance != 0 || c.range_members != c.resolved_ranges)
    } else {
        clusters.iter().any(|c| c.resolved != c.members.len())
    };
    if incomplete {
        result.unavailable = Some("unresolved_members");
        return Ok(result);
    }
    if bias && clusters.iter().any(|c| c.range_members == 0) {
        result.unavailable = Some("cluster_without_range_guidance");
        return Ok(result);
    }
    let values: Vec<_> = clusters
        .iter()
        .map(|c| {
            if bias {
                c.observed_mean_z.zip(c.observed_mean_z_rounding)
            } else {
                c.hit_rate.map(|point| (point, c.rounding_hit_completion))
            }
            .ok_or_else(|| invalid("complete cluster is missing its score"))
        })
        .collect::<Result<_>>()?;
    result.point = Some(mean(&values.iter().map(|(v, _)| *v).collect::<Vec<_>>())?);
    result.rounding_bounds = Some(mean_bounds(
        &values.iter().map(|(_, b)| *b).collect::<Vec<_>>(),
    )?);
    if clusters.len() < MIN_CLUSTERS {
        result.unavailable = Some("fewer_than_20_clusters");
        return Ok(result);
    }
    if values.iter().all(|(v, _)| *v == values[0].0) {
        result.unavailable = Some("no_between_cluster_variation");
        return Ok(result);
    }
    let (interval, rounded) = bootstrap(&values, axis, u32::from(bias), replicates)?;
    result.interval_90 = Some(interval);
    result.interval_90_rounding_enclosure = Some(rounded);
    Ok(result)
}

/// Summarize both partitions, and condition inference only on the explicitly selected one.
/// The source adapter owns compatible metric/basis selection and source authentication.
pub fn assess(members: &[Member], axis: Axis) -> Result<ClusterReport> {
    assess_with_replicates(members, axis, REPLICATES)
}

fn assess_with_replicates(
    members: &[Member],
    axis: Axis,
    replicates: usize,
) -> Result<ClusterReport> {
    if members.len() > MAX_MEMBERS {
        return Err(invalid("at most 4096 declared members are supported"));
    }
    let mut ids = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut ordered = members.iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| a.id.cmp(&b.id));
    let (mut years, mut sites) = (BTreeMap::new(), BTreeMap::new());
    let mut n_unresolved = 0;
    for member in ordered {
        if member.id.trim().is_empty()
            || member.site.trim().is_empty()
            || member.year == 0
            || member
                .id
                .chars()
                .chain(member.site.chars())
                .any(char::is_control)
            || !ids.insert(&member.id)
            || !keys.insert((&member.site, member.year))
        {
            return Err(invalid(
                "each member needs a distinct id and site/year, nonempty site and positive year",
            ));
        }
        if member.guidance.is_none() && member.outcome.is_some() {
            return Err(invalid("unknown guidance cannot have a scored outcome"));
        }
        if let Some(guidance) = member.guidance {
            exact(super::validate_guidance(guidance))?;
        }
        let score = member
            .guidance
            .zip(member.outcome)
            .map(|(g, o)| exact(super::score(g, o)))
            .transpose()?;
        n_unresolved += usize::from(score.is_none());
        years
            .entry(member.year.to_string())
            .or_insert_with(Vec::new)
            .push((member, score));
        sites
            .entry(member.site.clone())
            .or_insert_with(Vec::new)
            .push((member, score));
    }
    if years.len() > MAX_CLUSTERS || sites.len() > MAX_CLUSTERS {
        return Err(invalid("at most 256 clusters per axis are supported"));
    }
    let years = years
        .into_iter()
        .map(|(id, rows)| cluster(id, &rows))
        .collect::<Result<Vec<_>>>()?;
    let sites = sites
        .into_iter()
        .map(|(id, rows)| cluster(id, &rows))
        .collect::<Result<Vec<_>>>()?;
    let selected = if axis == Axis::Year { &years } else { &sites };
    Ok(ClusterReport {
        selected_axis: axis,
        n_members: members.len(),
        n_unresolved,
        hit: projection(selected, axis, false, replicates)?,
        bias: projection(selected, axis, true, replicates)?,
        years,
        sites,
    })
}

#[cfg(test)]
mod tests;
