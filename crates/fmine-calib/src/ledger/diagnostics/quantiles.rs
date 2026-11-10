//! DESIGN (BD-048/049): supplemental proper loss for every supplied quantile in a saved assessment.
//! Plan §10.2's frozen central interval score is unchanged. No distribution is interpolated from the table.
//! Determinism: Exact (checked rationals of the frozen IEEE-754 values; capacity failures stay explicit).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_evidence::Refusal;
use fmine_units::{ExactQty, UnitsError};

use super::super::{
    ForecastLedger, ForecastPayload, MemberAssessment, MemberOutcome, OutcomeSelector, absent,
    floats,
};

pub const METHOD: &str = "Supplemental twice-pinball loss: 2*p*(y-q) for y>=q, otherwise 2*(1-p)*(q-y). At p=0.5 this is absolute error; for exactly complementary levels p and 1-p, their summed losses divided by 2p give the interval score. The frozen assessment's interval score is unchanged. Every supplied level is scored separately, with no interpolation, CDF, CRPS approximation or pooled score across different levels. Exact rational arithmetic uses the frozen binary floating-point values, not reinterpreted decimal text. Capacity failures remain explicit and invalidate affected averages rather than dropping members. Completion bounds retain all members supplying that exact level, using only the prospectively frozen outcome domain; without a domain the upper bound is unbounded. Other levels and distributions remain visible. These are descriptive losses and arithmetic bounds, not confidence intervals, calibration verdicts or predictive-skill claims.";
const ZERO: ExactQty = ExactQty::from_integer(0);
const ONE: ExactQty = ExactQty::from_integer(1);
const TWO: ExactQty = ExactQty::from_integer(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantileLossBounds {
    pub lower: ExactQty,
    /// None means unbounded, never zero or an arithmetic failure.
    pub upper: Option<ExactQty>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuantileLoss {
    /// The exact frozen probability and quantile, retained even if arithmetic cannot represent the loss.
    pub level: f64,
    pub value: f64,
    pub loss: Option<ExactQty>,
    /// None means exact arithmetic was unavailable. A resolved loss has identical lower and upper bounds.
    pub completion: Option<QuantileLossBounds>,
}

impl QuantileLoss {
    pub fn status(&self, resolved: bool) -> &'static str {
        if self.completion.is_none() {
            "arithmetic_indeterminate"
        } else if resolved {
            "resolved"
        } else {
            "unresolved"
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuantileLossMember {
    /// The parent's exact outcome selection and citation, never a new selection.
    pub member: MemberAssessment,
    pub distribution: &'static str,
    /// Empty for non-quantile distributions; absent levels are never interpolated.
    pub quantiles: Vec<QuantileLoss>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuantileLossLevel {
    pub level: f64,
    pub n_supplied: usize,
    pub n_not_supplied: usize,
    pub n_resolved: usize,
    pub n_unresolved: usize,
    pub n_scored: usize,
    pub n_arithmetic_indeterminate: usize,
    /// None for no resolved members or any unrepresentable resolved loss/sum/mean.
    pub observed_mean: Option<ExactQty>,
    /// Uses the entire supplied population. None means exact arithmetic was unavailable.
    pub whole_supplied: Option<QuantileLossBounds>,
}

impl QuantileLossLevel {
    pub fn observed_status(&self) -> &'static str {
        if self.n_resolved == 0 {
            "no_resolved_outcomes"
        } else if self.observed_mean.is_some() {
            "exact"
        } else {
            "arithmetic_indeterminate"
        }
    }

    pub fn completion_status(&self) -> &'static str {
        if self.whole_supplied.is_some() {
            "exact"
        } else {
            "arithmetic_indeterminate"
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuantileDiagnostics {
    pub assessment_digest: Digest32,
    pub cohort: Digest32,
    pub as_of_ns: i64,
    pub selector: OutcomeSelector,
    pub outcome_bounds: Option<(f64, f64)>,
    pub members: Vec<QuantileLossMember>,
    pub levels: Vec<QuantileLossLevel>,
    pub digest: Digest32,
}

/// Convert the frozen binary value itself, including its exact significand and exponent.
/// Extremely large/small values may exceed the checked i128 rational representation.
fn exact(value: f64) -> Result<ExactQty, UnitsError> {
    if !value.is_finite() {
        return Err(UnitsError::Overflow);
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mut mantissa = bits & ((1_u64 << 52) - 1);
    let mut power = if exponent == 0 {
        -1074
    } else {
        mantissa |= 1_u64 << 52;
        exponent - 1023 - 52
    };
    if mantissa == 0 {
        return Ok(ZERO);
    }
    let trailing = mantissa.trailing_zeros();
    mantissa >>= trailing;
    power += trailing as i32;
    let mut numerator = i128::from(mantissa);
    let denominator = if power < 0 {
        2_i128
            .checked_pow(power.unsigned_abs())
            .ok_or(UnitsError::Overflow)?
    } else {
        numerator = numerator
            .checked_mul(
                2_i128
                    .checked_pow(power as u32)
                    .ok_or(UnitsError::Overflow)?,
            )
            .ok_or(UnitsError::Overflow)?;
        1
    };
    if value.is_sign_negative() {
        numerator = numerator.checked_neg().ok_or(UnitsError::Overflow)?;
    }
    ExactQty::new(numerator, denominator)
}

fn loss(level: f64, quantile: f64, outcome: f64) -> Result<ExactQty, UnitsError> {
    if outcome == quantile {
        return Ok(ZERO);
    }
    let probability = exact(level)?;
    let (weight, distance) = if outcome > quantile {
        (probability, exact(outcome)?.checked_sub(exact(quantile)?)?)
    } else {
        (
            ONE.checked_sub(probability)?,
            exact(quantile)?.checked_sub(exact(outcome)?)?,
        )
    };
    TWO.checked_mul(weight)?.checked_mul(distance)
}

fn missing_bounds(
    level: f64,
    value: f64,
    domain: Option<(f64, f64)>,
) -> Result<QuantileLossBounds, UnitsError> {
    let Some((lo, hi)) = domain else {
        return Ok(QuantileLossBounds {
            lower: ZERO,
            upper: None,
        });
    };
    let lower = loss(level, value, value.clamp(lo, hi))?;
    let left = loss(level, value, lo)?;
    let right = loss(level, value, hi)?;
    // Subtraction reduces shared factors before comparison, avoiding an unnecessary cross-product.
    let upper = if left.checked_sub(right)?.signum() < 0 {
        right
    } else {
        left
    };
    Ok(QuantileLossBounds {
        lower,
        upper: Some(upper),
    })
}

fn divide(sum: ExactQty, count: usize) -> Result<ExactQty, UnitsError> {
    sum.checked_div(ExactQty::from_integer(count as i128))
}

fn summarize(
    level: f64,
    members: &[QuantileLossMember],
    entries: &[(bool, QuantileLoss)],
) -> QuantileLossLevel {
    let n_resolved = entries.iter().filter(|(resolved, _)| *resolved).count();
    let observed_mean = if n_resolved == 0 {
        None
    } else {
        entries
            .iter()
            .filter(|(resolved, _)| *resolved)
            .try_fold(ZERO, |sum, (_, entry)| {
                sum.checked_add(entry.loss.ok_or(UnitsError::Overflow)?)
            })
            .and_then(|sum| divide(sum, n_resolved))
            .ok()
    };
    let whole_supplied = entries
        .iter()
        .try_fold(
            QuantileLossBounds {
                lower: ZERO,
                upper: Some(ZERO),
            },
            |sum, (_, entry)| -> Result<_, UnitsError> {
                let bound = entry.completion.ok_or(UnitsError::Overflow)?;
                Ok(QuantileLossBounds {
                    lower: sum.lower.checked_add(bound.lower)?,
                    upper: match (sum.upper, bound.upper) {
                        (Some(a), Some(b)) => Some(a.checked_add(b)?),
                        _ => None,
                    },
                })
            },
        )
        .and_then(|sum| {
            Ok(QuantileLossBounds {
                lower: divide(sum.lower, entries.len())?,
                upper: sum
                    .upper
                    .map(|upper| divide(upper, entries.len()))
                    .transpose()?,
            })
        })
        .ok();
    QuantileLossLevel {
        level,
        n_supplied: entries.len(),
        n_not_supplied: members.len() - entries.len(),
        n_resolved,
        n_unresolved: entries.len() - n_resolved,
        n_scored: entries
            .iter()
            .filter(|(_, entry)| entry.loss.is_some())
            .count(),
        n_arithmetic_indeterminate: entries
            .iter()
            .filter(|(_, entry)| entry.completion.is_none())
            .count(),
        observed_mean,
        whole_supplied,
    }
}

impl ForecastLedger {
    /// Read-only QNT1/v1 supplement to an existing saved assessment; no prior identity or score changes.
    pub fn quantile_diagnostics(&self, digest: Digest32) -> Result<QuantileDiagnostics, Refusal> {
        let assessment = self
            .assessments
            .values()
            .find(|a| a.digest == digest)
            .ok_or_else(|| {
                absent("quantile diagnostics require an existing immutable assessment")
            })?;
        let outcome_bounds = self
            .cohorts
            .get(&assessment.cohort)
            .ok_or_else(|| absent("quantile diagnostics require the frozen parent cohort"))?
            .spec
            .policy
            .outcome_bounds;
        let mut members = Vec::with_capacity(assessment.per_member.len());
        let mut by_level: BTreeMap<u64, Vec<(bool, QuantileLoss)>> = BTreeMap::new();
        for member in &assessment.per_member {
            let payload = &self
                .records
                .get(&member.forecast_id)
                .ok_or_else(|| absent("quantile member's frozen forecast is unavailable"))?
                .payload;
            let outcome = match member.outcome {
                MemberOutcome::Resolved { value, .. } => Some(value),
                MemberOutcome::Unresolved { .. } => None,
            };
            let (distribution, quantiles) = match payload {
                ForecastPayload::Quantiles { levels, values } => (
                    "quantiles",
                    levels
                        .iter()
                        .zip(values)
                        .map(|(&level, &value)| {
                            let loss = outcome.and_then(|y| loss(level, value, y).ok());
                            let completion = if outcome.is_some() {
                                loss.map(|score| QuantileLossBounds {
                                    lower: score,
                                    upper: Some(score),
                                })
                            } else {
                                missing_bounds(level, value, outcome_bounds).ok()
                            };
                            let entry = QuantileLoss {
                                level,
                                value,
                                loss,
                                completion,
                            };
                            by_level
                                .entry(level.to_bits())
                                .or_default()
                                .push((outcome.is_some(), entry));
                            entry
                        })
                        .collect(),
                ),
                ForecastPayload::Gaussian { .. } => ("gaussian", Vec::new()),
                ForecastPayload::Interval { .. } => ("interval", Vec::new()),
                ForecastPayload::Bernoulli { .. } => ("bernoulli", Vec::new()),
                ForecastPayload::Empirical { .. } => ("empirical", Vec::new()),
            };
            members.push(QuantileLossMember {
                member: member.clone(),
                distribution,
                quantiles,
            });
        }
        // All levels are strictly positive finite values, so their bit order is their numeric order.
        let levels = by_level
            .into_iter()
            .map(|(bits, entries)| summarize(f64::from_bits(bits), &members, &entries))
            .collect();
        let mut report = QuantileDiagnostics {
            assessment_digest: digest,
            cohort: assessment.cohort,
            as_of_ns: assessment.as_of_ns,
            selector: assessment.selector,
            outcome_bounds,
            members,
            levels,
            digest: Digest32::default(),
        };
        report.digest = blake3_hash(&report.canonical_bytes());
        Ok(report)
    }
}

fn encode_exact(enc: &mut Encoder, value: Option<ExactQty>) {
    enc.put_u8(u8::from(value.is_some()));
    if let Some(value) = value {
        enc.put_fixed(&value.num().to_le_bytes());
        enc.put_fixed(&value.den().to_le_bytes());
    }
}

fn encode_bounds(enc: &mut Encoder, bounds: Option<QuantileLossBounds>) {
    enc.put_u8(u8::from(bounds.is_some()));
    if let Some(bounds) = bounds {
        encode_exact(enc, Some(bounds.lower));
        encode_exact(enc, bounds.upper);
    }
}

fn exact_json(value: Option<ExactQty>) -> String {
    value.map_or_else(
        || "null".into(),
        |v| format!("{{\"num\":\"{}\",\"den\":\"{}\"}}", v.num(), v.den()),
    )
}

fn bounds_json(bounds: Option<QuantileLossBounds>) -> String {
    bounds.map_or_else(
        || "null".into(),
        |b| {
            format!(
                "{{\"lower\":{},\"upper\":{},\"upper_unbounded\":{}}}",
                exact_json(Some(b.lower)),
                exact_json(b.upper),
                b.upper.is_none()
            )
        },
    )
}

fn exact_text(value: Option<ExactQty>) -> String {
    value.map_or_else(
        || "unavailable".into(),
        |v| {
            if v.den() == 1 {
                v.num().to_string()
            } else {
                format!("{}/{}", v.num(), v.den())
            }
        },
    )
}

fn bounds_text(bounds: Option<QuantileLossBounds>) -> String {
    bounds.map_or_else(
        || "arithmetic indeterminate".into(),
        |b| {
            format!(
                "[{}, {}]",
                exact_text(Some(b.lower)),
                b.upper
                    .map_or_else(|| "unbounded".into(), |v| exact_text(Some(v)))
            )
        },
    )
}

impl QuantileDiagnostics {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"QNT1", 1);
        enc.put_str(METHOD);
        enc.put_fixed(&self.assessment_digest.0);
        enc.put_fixed(&self.cohort.0);
        enc.put_i64(self.as_of_ns);
        enc.put_u8(u8::from(self.selector == OutcomeSelector::LatestAsOf));
        enc.put_u8(u8::from(self.outcome_bounds.is_some()));
        if let Some((lo, hi)) = self.outcome_bounds {
            floats(&mut enc, &[lo, hi]);
        }
        enc.put_u64(self.levels.len() as u64);
        for level in &self.levels {
            floats(&mut enc, &[level.level]);
            for count in [
                level.n_supplied,
                level.n_not_supplied,
                level.n_resolved,
                level.n_unresolved,
                level.n_scored,
                level.n_arithmetic_indeterminate,
            ] {
                enc.put_u64(count as u64);
            }
            encode_exact(&mut enc, level.observed_mean);
            encode_bounds(&mut enc, level.whole_supplied);
        }
        enc.put_u64(self.members.len() as u64);
        for member in &self.members {
            member.member.encode(&mut enc);
            enc.put_str(member.distribution);
            enc.put_u64(member.quantiles.len() as u64);
            for q in &member.quantiles {
                floats(&mut enc, &[q.level, q.value]);
                encode_exact(&mut enc, q.loss);
                encode_bounds(&mut enc, q.completion);
            }
        }
        enc.finish()
    }

    /// Rational numerators and denominators are strings so JSON consumers cannot round i128 integers.
    pub fn to_json(&self) -> String {
        let levels = self.levels.iter().map(|s| format!(
            "{{\"level\":{},\"level_bits\":\"{:016x}\",\"n_supplied\":{},\"n_not_supplied\":{},\"n_resolved\":{},\"n_unresolved\":{},\"n_scored\":{},\"n_arithmetic_indeterminate\":{},\"observed_status\":\"{}\",\"observed_mean\":{},\"completion_status\":\"{}\",\"whole_supplied\":{}}}",
            s.level, s.level.to_bits(), s.n_supplied, s.n_not_supplied, s.n_resolved, s.n_unresolved, s.n_scored, s.n_arithmetic_indeterminate,
            s.observed_status(), exact_json(s.observed_mean), s.completion_status(), bounds_json(s.whole_supplied)
        )).collect::<Vec<_>>().join(",");
        let members = self.members.iter().map(|m| {
            let resolved = matches!(m.member.outcome, MemberOutcome::Resolved { .. });
            let quantiles = m.quantiles.iter().map(|q| format!(
                "{{\"level\":{},\"level_bits\":\"{:016x}\",\"value\":{},\"status\":\"{}\",\"loss\":{},\"completion\":{}}}",
                q.level, q.level.to_bits(), q.value, q.status(resolved), exact_json(q.loss), bounds_json(q.completion)
            )).collect::<Vec<_>>().join(",");
            let event = m.member.selected_event.map_or_else(|| "null".into(), |(seq, digest)| format!("{{\"seq\":{seq},\"digest\":\"{digest}\"}}"));
            format!("{{\"forecast_id\":\"{}\",\"selected_event\":{event},\"distribution\":\"{}\",\"status\":\"{}\",\"outcome_resolved\":{resolved},\"quantiles\":[{quantiles}]}}",
                m.member.forecast_id, m.distribution, if m.distribution == "quantiles" { "supplied" } else { "unsupported_distribution" })
        }).collect::<Vec<_>>().join(",");
        let selector = match self.selector {
            OutcomeSelector::FirstPublished => "first-published",
            OutcomeSelector::LatestAsOf => "latest-as-of",
        };
        let domain = self
            .outcome_bounds
            .map_or_else(|| "null".into(), |(lo, hi)| format!("[{lo},{hi}]"));
        let supported = self
            .members
            .iter()
            .filter(|m| m.distribution == "quantiles")
            .count();
        format!(
            "{{\"schema\":\"fmine.forecast-quantile-loss.v1\",\"assessment_digest\":\"{}\",\"cohort\":\"{}\",\"digest\":\"{}\",\"as_of_ns\":{},\"selector\":\"{selector}\",\"method\":\"{METHOD}\",\"outcome_bounds\":{domain},\"n_members\":{},\"n_quantile_members\":{supported},\"n_unsupported_distributions\":{},\"levels\":[{levels}],\"members\":[{members}]}}",
            self.assessment_digest,
            self.cohort,
            self.digest,
            self.as_of_ns,
            self.members.len(),
            self.members.len() - supported
        )
    }

    pub fn to_markdown(&self) -> String {
        let mut out = format!(
            "\n### Supplied quantile losses\n\n{METHOD}\n\nParent assessment: `{}`. Quantile report: `{}`. Exact fractions below use the recorded binary values.\n\n| Level | Supplied / all | Resolved | Unresolved | Exact scores | Arithmetic indeterminate | Observed mean | Whole-supplied completion bounds |\n|---|---:|---:|---:|---:|---:|---|---|\n",
            self.assessment_digest, self.digest
        );
        for s in &self.levels {
            let observed = s.observed_mean.map_or_else(
                || s.observed_status().replace('_', " "),
                |v| exact_text(Some(v)),
            );
            let _ = writeln!(
                out,
                "| {} | {} / {} | {} | {} | {} | {} | {} | {} |",
                s.level,
                s.n_supplied,
                self.members.len(),
                s.n_resolved,
                s.n_unresolved,
                s.n_scored,
                s.n_arithmetic_indeterminate,
                observed,
                bounds_text(s.whole_supplied)
            );
        }
        out.push_str("\nEvery forecast retains its selected outcome and source in the parent assessment. Missing levels are absent, never imputed.\n\n| Forecast | Level | Quantile | Result | Exact loss | Completion bounds |\n|---|---:|---:|---|---|---|\n");
        for m in &self.members {
            if m.quantiles.is_empty() {
                let _ = writeln!(
                    out,
                    "| `{}` | — | — | unsupported distribution ({}) | unavailable | unavailable |",
                    m.member.forecast_id, m.distribution
                );
            }
            let resolved = matches!(m.member.outcome, MemberOutcome::Resolved { .. });
            for q in &m.quantiles {
                let _ = writeln!(
                    out,
                    "| `{}` | {} | {} | {} | {} | {} |",
                    m.member.forecast_id,
                    q.level,
                    q.value,
                    q.status(resolved),
                    exact_text(q.loss),
                    bounds_text(q.completion)
                );
            }
        }
        out
    }
}
