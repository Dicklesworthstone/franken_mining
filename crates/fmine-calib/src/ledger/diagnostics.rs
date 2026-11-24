//! DESIGN (BD-048/049, plan §10.2): descriptive diagnostics of an immutable saved assessment.
//! No outcome is reselected, no authority time is advanced, and no independence or calibration is inferred.
//! Determinism: DeterministicPerIsa
//! Uses the existing deterministic normal-CDF owner and fixed bin edges.

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_evidence::Refusal;
use fmine_numerics::norm_cdf;
use fmine_units::ExactQty;

use crate::CalibrationFlag;

use super::{
    Assessment, ForecastLedger, ForecastPayload, MemberAssessment, MemberOutcome, OutcomeSelector,
    absent, domain, floats, optional_float,
};

pub mod coverage;
pub mod gaussian_log;
pub mod quantiles;

pub const SCOPE: &str = "Descriptive diagnostics within one frozen cohort, not calibration verdicts, independence estimates, confidence intervals or predictive-skill claims. Unsupported distributions and unresolved members remain visible.";
pub const BIN_RULE: &str = "Fixed deciles [lower, upper), with the final bin including 1; exact edge values enter the bin on their right. Completion bounds retain every unresolved member and are not simultaneous confidence intervals.";
pub const PIT_METHOD: &str = "Gaussian Phi((outcome-mean)/std_dev), using fmine_numerics::norm_cdf; approximate CDF values and floating-point tail saturation are retained without clipping or randomization.";
pub const GAUSSIAN_CALIBRATION_METHOD: &str = "One frozen Gaussian model and metadata key at 80% nominal coverage. The score subtracts the greater of the scaled coverage error and the two-sided PIT CDF distance from one, with the penalty capped at one. The PIT distance requires a complete cohort. A non-calibrated flag gives zero. This diagnostic leaves the saved flag unchanged.";
pub const EMPIRICAL_METHOD: &str = "Uniform empirical CDF bounds [F(y-),F(y)] = [below/n,(below+equal)/n], retaining exact sample counts and ties. Closed rank intervals are counted in each fixed decile as definite when wholly contained, and possible when intersecting. Every unresolved member contributes to each whole-population upper bound. Integer comparisons determine bin membership; no randomized PIT, uniformity or calibration is inferred.";
pub const WEIGHTED_METHOD: &str = "Weighted empirical CDF bounds [F(y-),F(y)] retain the exact declared probability mass below, equal to and above the selected outcome. No resampling, normalization or floating-point probability conversion occurs. Closed intervals have definite/possible decile membership; every unresolved forecast remains in whole-population completion bounds. This population is separate from uniform empirical and Gaussian diagnostics. No randomized PIT, uniformity or calibration is inferred.";
const EDGES: [f64; 11] = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiagnosticValue {
    /// Absent only when this Gaussian member has no selected outcome.
    GaussianPit(Option<f64>),
    /// The unchanged forecast probability, including for unresolved members.
    EventProbability(f64),
    /// Exact finite-sample CDF bounds; absent counts retain an unresolved forecast's sample size.
    EmpiricalRank {
        n_samples: usize,
        counts: Option<EmpiricalRankCounts>,
    },
    /// Exact declared probability masses, rather than counts of unequally weighted scenarios.
    WeightedEmpiricalRank {
        n_samples: usize,
        masses: Option<WeightedRankMasses>,
    },
    /// A distribution without a supported diagnostic; no CDF is invented from sparse quantiles.
    Unsupported(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmpiricalRankCounts {
    pub below: usize,
    pub equal: usize,
    pub above: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeightedRankMasses {
    pub below: ExactQty,
    pub equal: ExactQty,
    pub above: ExactQty,
}

impl WeightedRankMasses {
    /// Closed CDF interval, preserving all mass tied at the observed value.
    pub fn cdf_bounds(self) -> Result<(ExactQty, ExactQty), Refusal> {
        let upper = ExactQty::from_integer(1)
            .checked_sub(self.above)
            .map_err(|_| domain("exact weighted diagnostic CDF capacity"))?;
        Ok((self.below, upper))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmpiricalRankBin {
    /// Exact lower bound lower_decile / 10 (inclusive).
    pub lower_decile: u8,
    /// Exact upper bound upper_decile / 10 (exclusive, except at 1).
    pub upper_decile: u8,
    /// Resolved members whose entire closed CDF interval is in this bin.
    pub definite_count: usize,
    /// Resolved members whose closed CDF interval intersects this bin, including definite members.
    pub possible_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmpiricalRankDiagram {
    pub n_members: usize,
    pub n_resolved: usize,
    pub n_unresolved: usize,
    /// Whole-population fraction bounds are definite_count / n_members through
    /// (possible_count + n_unresolved) / n_members; they are not confidence intervals.
    pub bins: [EmpiricalRankBin; 10],
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticMember {
    pub member: MemberAssessment,
    pub value: DiagnosticValue,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PitBin {
    pub lower: f64,
    pub upper: f64,
    pub count: usize,
    /// Bounds on this bin's final fraction of the complete Gaussian population, not a confidence interval.
    pub whole_population_fraction: (f64, f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PitHistogram {
    pub n_members: usize,
    pub n_resolved: usize,
    pub n_unresolved: usize,
    pub bins: [PitBin; 10],
}

/// Supplementary NL-8 calculation from one homogeneous Gaussian assessment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaussianCalibrationScore {
    pub n_members: usize,
    pub n_resolved: usize,
    pub inside_80: usize,
    /// Two-sided distance to the uniform CDF, computed only for complete cohorts.
    pub pit_ks: Option<f64>,
    pub flag: CalibrationFlag,
    pub score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReliabilityBin {
    pub lower: f64,
    pub upper: f64,
    pub n_members: usize,
    pub n_resolved: usize,
    pub n_events: usize,
    pub mean_probability: Option<f64>,
    /// Use this alongside the observed event frequency: both have the same resolved population.
    pub mean_resolved_probability: Option<f64>,
    pub observed_frequency: Option<f64>,
    /// Absent for an empty bin. Missing outcomes may be either 0 or 1; forecast probabilities are not imputations.
    pub whole_bin_frequency: Option<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReliabilityDiagram {
    pub n_members: usize,
    pub n_resolved: usize,
    pub n_unresolved: usize,
    pub bins: [ReliabilityBin; 10],
}

/// A separate projection bound to its exact saved parent. It never replaces the parent's durable bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct AssessmentDiagnostics {
    pub assessment_digest: Digest32,
    pub cohort: Digest32,
    pub as_of_ns: i64,
    pub selector: OutcomeSelector,
    pub members: Vec<DiagnosticMember>,
    pub gaussian_pit: Option<PitHistogram>,
    /// Recomputable from the bound assessment, frozen members and PIT values.
    pub gaussian_calibration_score: Option<GaussianCalibrationScore>,
    pub bernoulli_reliability: Option<ReliabilityDiagram>,
    pub empirical_rank: Option<EmpiricalRankDiagram>,
    /// Counts of forecasts in each rank bin; scenario probabilities are retained per member.
    pub weighted_empirical_rank: Option<EmpiricalRankDiagram>,
    pub digest: Digest32,
}

fn bin_index(probability: f64) -> Result<usize, Refusal> {
    if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
        return Err(domain("diagnostic probability must be finite and in [0,1]"));
    }
    Ok(EDGES[1..]
        .iter()
        .position(|edge| probability < *edge)
        .unwrap_or(9))
}

// Forecast admission already bounds nonempty finite samples at MAX_EMPIRICAL_SAMPLES (4096).
// A single pass needs no sorting, scratch copy, subtraction or interpolation, including at f64 extremes.
fn empirical_counts(samples: &[f64], outcome: f64) -> EmpiricalRankCounts {
    let (mut below, mut equal) = (0, 0);
    for sample in samples {
        if *sample < outcome {
            below += 1;
        } else if *sample == outcome {
            equal += 1;
        }
    }
    EmpiricalRankCounts {
        below,
        equal,
        above: samples.len() - below - equal,
    }
}

fn empirical_bin(numerator: usize, n_samples: usize) -> usize {
    // Both counts are at most 4096. Exact rational edges enter the bin on their right; 1 stays in bin 9.
    (10 * numerator / n_samples).min(9)
}

fn weighted_masses(
    samples: &[f64],
    weights: &[ExactQty],
    outcome: f64,
) -> Result<WeightedRankMasses, Refusal> {
    let mut masses = WeightedRankMasses {
        below: ExactQty::from_integer(0),
        equal: ExactQty::from_integer(0),
        above: ExactQty::from_integer(0),
    };
    // Admission checked finite samples and positive masses summing exactly to one. Comparing values
    // needs no subtraction, sorting or interpolation, including signed zero and extreme outcomes.
    for (&sample, &weight) in samples.iter().zip(weights) {
        let mass = if sample < outcome {
            &mut masses.below
        } else if sample == outcome {
            &mut masses.equal
        } else {
            &mut masses.above
        };
        *mass = mass
            .checked_add(weight)
            .map_err(|_| domain("exact weighted diagnostic mass capacity"))?;
    }
    Ok(masses)
}

fn weighted_bin(mass: ExactQty) -> usize {
    // Compare n/d to i/10 as n < ceil(i*d/10). Splitting d before multiplication keeps every
    // intermediate representable even at i128::MAX; a probability that rounds onto an edge stays
    // on its exact side. Admission guarantees 0 <= n <= d, d > 0.
    (1_i128..10)
        .position(|i| mass.num() < (mass.den() / 10) * i + (mass.den() % 10 * i + 9) / 10)
        .unwrap_or(9)
}

fn uniform_pit_distance(values: &mut [f64]) -> f64 {
    values.sort_unstable_by(f64::total_cmp);
    let n = values.len() as f64;
    values
        .iter()
        .enumerate()
        .fold(0.0_f64, |distance, (i, &pit)| {
            distance
                .max((i + 1) as f64 / n - pit)
                .max(pit - i as f64 / n)
        })
}

impl ForecastLedger {
    fn gaussian_calibration_score(
        &self,
        assessment: &Assessment,
        members: &[DiagnosticMember],
        histogram: &PitHistogram,
    ) -> Option<GaussianCalibrationScore> {
        if histogram.n_members != members.len()
            || self.cohort(assessment.cohort)?.spec.policy.alpha != 0.2
        {
            return None;
        }
        let first = self.forecast(members.first()?.member.forecast_id)?;
        let metadata = first.metadata.as_ref()?;
        if !members.iter().all(|item| {
            self.forecast(item.member.forecast_id)
                .is_some_and(|member| {
                    member.model_key == first.model_key
                        && member.metadata.as_ref() == Some(metadata)
                })
        }) {
            return None;
        }
        let inside_80 = assessment.summary.coverage?.inside;
        let pit_ks = if histogram.n_unresolved == 0 {
            // The histogram checked every PIT. Require the whole population, including ties.
            let mut values = members
                .iter()
                .map(|item| match item.value {
                    DiagnosticValue::GaussianPit(pit) => pit,
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            Some(uniform_pit_distance(&mut values))
        } else {
            None
        };
        let flag = assessment.summary.flag;
        let score = if let (CalibrationFlag::Calibrated, Some(ks)) = (flag, pit_ks) {
            // |coverage - 4/5| / (1/5), with exact integer cancellation before conversion.
            let n = histogram.n_members as u128;
            let error = (5 * inside_80 as u128).abs_diff(4 * n) as f64 / n as f64;
            1.0 - error.max(ks).min(1.0)
        } else {
            0.0
        };
        Some(GaussianCalibrationScore {
            n_members: histogram.n_members,
            n_resolved: histogram.n_resolved,
            inside_80,
            pit_ks,
            flag,
            score,
        })
    }

    /// Derive diagnostics only from a saved assessment's selected publications and missing outcomes.
    /// Cohort admission already keeps declared family, quantity and unit compatible. The dimensionless
    /// views do not pool across cohorts or retrospectively infer absent legacy metadata.
    pub fn assessment_diagnostics(
        &self,
        assessment_digest: Digest32,
    ) -> Result<AssessmentDiagnostics, Refusal> {
        let assessment = self
            .assessments
            .values()
            .find(|a| a.digest == assessment_digest)
            .ok_or_else(|| absent("diagnostics require an existing immutable assessment"))?;
        let mut members = Vec::with_capacity(assessment.per_member.len());
        let mut pit_counts = [0; 10];
        let (mut gaussian_n, mut gaussian_resolved) = (0, 0);
        let (mut empirical_n, mut empirical_resolved) = (0, 0);
        let (mut weighted_n, mut weighted_resolved) = (0, 0);
        let mut rank_bins = std::array::from_fn(|i| EmpiricalRankBin {
            lower_decile: i as u8,
            upper_decile: (i + 1) as u8,
            definite_count: 0,
            possible_count: 0,
        });
        let mut weighted_bins = rank_bins;
        let mut reliability = std::array::from_fn(|i| ReliabilityBin {
            lower: EDGES[i],
            upper: EDGES[i + 1],
            n_members: 0,
            n_resolved: 0,
            n_events: 0,
            mean_probability: None,
            mean_resolved_probability: None,
            observed_frequency: None,
            whole_bin_frequency: None,
        });
        let (mut probability_sums, mut resolved_sums) = ([0.0; 10], [0.0; 10]);
        for member in &assessment.per_member {
            let forecast = self
                .records
                .get(&member.forecast_id)
                .ok_or_else(|| absent("diagnostic member's frozen forecast is unavailable"))?;
            let outcome = match &member.outcome {
                MemberOutcome::Resolved { value, .. } => Some(*value),
                MemberOutcome::Unresolved { .. } => None,
            };
            let value = match &forecast.payload {
                ForecastPayload::Gaussian { mean, std_dev } => {
                    gaussian_n += 1;
                    let pit = outcome
                        .map(|y| {
                            let z = (y - mean) / std_dev;
                            if !z.is_finite() {
                                return Err(domain(
                                    "Gaussian diagnostic standardization is not finite",
                                ));
                            }
                            let pit = norm_cdf(z);
                            pit_counts[bin_index(pit)?] += 1;
                            gaussian_resolved += 1;
                            Ok(pit)
                        })
                        .transpose()?;
                    DiagnosticValue::GaussianPit(pit)
                }
                ForecastPayload::Bernoulli { probability } => {
                    let index = bin_index(*probability)?;
                    let bin = &mut reliability[index];
                    bin.n_members += 1;
                    probability_sums[index] += probability;
                    if let Some(y) = outcome {
                        if y != 0.0 && y != 1.0 {
                            return Err(domain("Bernoulli diagnostic outcomes must be 0 or 1"));
                        }
                        bin.n_resolved += 1;
                        bin.n_events += usize::from(y == 1.0);
                        resolved_sums[index] += probability;
                    }
                    DiagnosticValue::EventProbability(*probability)
                }
                ForecastPayload::Interval { .. } => DiagnosticValue::Unsupported("interval"),
                ForecastPayload::Quantiles { .. } => DiagnosticValue::Unsupported("quantiles"),
                ForecastPayload::WeightedEmpirical { samples, weights } => {
                    weighted_n += 1;
                    let masses = outcome
                        .map(|y| {
                            let masses = weighted_masses(samples, weights, y)?;
                            let (lower, upper) = masses.cdf_bounds()?;
                            let (lower, upper) = (weighted_bin(lower), weighted_bin(upper));
                            for bin in &mut weighted_bins[lower..=upper] {
                                bin.possible_count += 1;
                            }
                            if lower == upper {
                                weighted_bins[lower].definite_count += 1;
                            }
                            weighted_resolved += 1;
                            Ok(masses)
                        })
                        .transpose()?;
                    DiagnosticValue::WeightedEmpiricalRank {
                        n_samples: samples.len(),
                        masses,
                    }
                }
                ForecastPayload::Empirical { samples } => {
                    empirical_n += 1;
                    let counts = outcome.map(|y| {
                        let counts = empirical_counts(samples, y);
                        let lower = empirical_bin(counts.below, samples.len());
                        let upper = empirical_bin(counts.below + counts.equal, samples.len());
                        for bin in &mut rank_bins[lower..=upper] {
                            bin.possible_count += 1;
                        }
                        if lower == upper {
                            rank_bins[lower].definite_count += 1;
                        }
                        empirical_resolved += 1;
                        counts
                    });
                    DiagnosticValue::EmpiricalRank {
                        n_samples: samples.len(),
                        counts,
                    }
                }
            };
            members.push(DiagnosticMember {
                member: member.clone(),
                value,
            });
        }
        let gaussian_pit = (gaussian_n > 0).then(|| {
            let n_unresolved = gaussian_n - gaussian_resolved;
            PitHistogram {
                n_members: gaussian_n,
                n_resolved: gaussian_resolved,
                n_unresolved,
                bins: std::array::from_fn(|i| PitBin {
                    lower: EDGES[i],
                    upper: EDGES[i + 1],
                    count: pit_counts[i],
                    whole_population_fraction: (
                        pit_counts[i] as f64 / gaussian_n as f64,
                        (pit_counts[i] + n_unresolved) as f64 / gaussian_n as f64,
                    ),
                }),
            }
        });
        for (index, bin) in reliability.iter_mut().enumerate() {
            if bin.n_members > 0 {
                bin.mean_probability = Some(probability_sums[index] / bin.n_members as f64);
                bin.whole_bin_frequency = Some((
                    bin.n_events as f64 / bin.n_members as f64,
                    (bin.n_events + (bin.n_members - bin.n_resolved)) as f64 / bin.n_members as f64,
                ));
            }
            if bin.n_resolved > 0 {
                bin.mean_resolved_probability = Some(resolved_sums[index] / bin.n_resolved as f64);
                bin.observed_frequency = Some(bin.n_events as f64 / bin.n_resolved as f64);
            }
        }
        let event_n = reliability.iter().map(|bin| bin.n_members).sum::<usize>();
        let event_resolved = reliability.iter().map(|bin| bin.n_resolved).sum::<usize>();
        let bernoulli_reliability = (event_n > 0).then_some(ReliabilityDiagram {
            n_members: event_n,
            n_resolved: event_resolved,
            n_unresolved: event_n - event_resolved,
            bins: reliability,
        });
        let mut report = AssessmentDiagnostics {
            assessment_digest,
            cohort: assessment.cohort,
            as_of_ns: assessment.as_of_ns,
            selector: assessment.selector,
            gaussian_calibration_score: gaussian_pit
                .as_ref()
                .and_then(|pit| self.gaussian_calibration_score(assessment, &members, pit)),
            members,
            gaussian_pit,
            bernoulli_reliability,
            empirical_rank: (empirical_n > 0).then_some(EmpiricalRankDiagram {
                n_members: empirical_n,
                n_resolved: empirical_resolved,
                n_unresolved: empirical_n - empirical_resolved,
                bins: rank_bins,
            }),
            weighted_empirical_rank: (weighted_n > 0).then_some(EmpiricalRankDiagram {
                n_members: weighted_n,
                n_resolved: weighted_resolved,
                n_unresolved: weighted_n - weighted_resolved,
                bins: weighted_bins,
            }),
            digest: Digest32::default(),
        };
        report.digest = blake3_hash(&report.canonical_bytes());
        Ok(report)
    }
}

impl AssessmentDiagnostics {
    pub fn version(&self) -> u16 {
        if self.weighted_empirical_rank.is_some() {
            3
        } else if self.empirical_rank.is_some() {
            2
        } else {
            1
        }
    }

    /// DGN1/v3 adds weighted ranks only when present; other reports retain DGN1/v1-v2 bytes.
    /// Each version binds the saved parent, complete member partition, values and diagnostic rules.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let version = self.version();
        let mut enc = Encoder::new_frame(*b"DGN1", version);
        for rule in [SCOPE, BIN_RULE, PIT_METHOD] {
            enc.put_str(rule);
        }
        if version >= 2 {
            enc.put_str(EMPIRICAL_METHOD);
        }
        if version == 3 {
            enc.put_str(WEIGHTED_METHOD);
        }
        enc.put_fixed(&self.assessment_digest.0);
        enc.put_fixed(&self.cohort.0);
        enc.put_i64(self.as_of_ns);
        enc.put_u8(match self.selector {
            OutcomeSelector::FirstPublished => 0,
            OutcomeSelector::LatestAsOf => 1,
        });
        enc.put_u64(self.members.len() as u64);
        for item in &self.members {
            item.member.encode(&mut enc);
            match &item.value {
                DiagnosticValue::GaussianPit(pit) => {
                    enc.put_u8(0);
                    optional_float(&mut enc, *pit);
                }
                DiagnosticValue::EventProbability(p) => {
                    enc.put_u8(1);
                    floats(&mut enc, &[*p]);
                }
                DiagnosticValue::Unsupported(kind) => {
                    enc.put_u8(2);
                    enc.put_str(kind);
                }
                DiagnosticValue::EmpiricalRank { n_samples, counts } => {
                    enc.put_u8(3);
                    enc.put_u64(*n_samples as u64);
                    enc.put_u8(u8::from(counts.is_some()));
                    if let Some(counts) = counts {
                        for count in [counts.below, counts.equal, counts.above] {
                            enc.put_u64(count as u64);
                        }
                    }
                }
                DiagnosticValue::WeightedEmpiricalRank { n_samples, masses } => {
                    enc.put_u8(4);
                    enc.put_u64(*n_samples as u64);
                    enc.put_u8(u8::from(masses.is_some()));
                    if let Some(masses) = masses {
                        for mass in [masses.below, masses.equal, masses.above] {
                            enc.put_i128(mass.num());
                            enc.put_i128(mass.den());
                        }
                    }
                }
            }
        }
        enc.put_u8(u8::from(self.gaussian_pit.is_some()));
        if let Some(pit) = &self.gaussian_pit {
            for count in [pit.n_members, pit.n_resolved, pit.n_unresolved] {
                enc.put_u64(count as u64);
            }
            for bin in &pit.bins {
                floats(&mut enc, &[bin.lower, bin.upper]);
                enc.put_u64(bin.count as u64);
                floats(
                    &mut enc,
                    &[
                        bin.whole_population_fraction.0,
                        bin.whole_population_fraction.1,
                    ],
                );
            }
        }
        enc.put_u8(u8::from(self.bernoulli_reliability.is_some()));
        if let Some(reliability) = &self.bernoulli_reliability {
            for count in [
                reliability.n_members,
                reliability.n_resolved,
                reliability.n_unresolved,
            ] {
                enc.put_u64(count as u64);
            }
            for bin in &reliability.bins {
                floats(&mut enc, &[bin.lower, bin.upper]);
                for count in [bin.n_members, bin.n_resolved, bin.n_events] {
                    enc.put_u64(count as u64);
                }
                for value in [
                    bin.mean_probability,
                    bin.mean_resolved_probability,
                    bin.observed_frequency,
                ] {
                    optional_float(&mut enc, value);
                }
                enc.put_u8(u8::from(bin.whole_bin_frequency.is_some()));
                if let Some((lower, upper)) = bin.whole_bin_frequency {
                    floats(&mut enc, &[lower, upper]);
                }
            }
        }
        if version == 3 {
            enc.put_u8(u8::from(self.empirical_rank.is_some()));
        }
        for ranks in [
            self.empirical_rank.as_ref(),
            self.weighted_empirical_rank.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            for count in [ranks.n_members, ranks.n_resolved, ranks.n_unresolved] {
                enc.put_u64(count as u64);
            }
            for bin in &ranks.bins {
                enc.put_u8(bin.lower_decile);
                enc.put_u8(bin.upper_decile);
                enc.put_u64(bin.definite_count as u64);
                enc.put_u64(bin.possible_count as u64);
            }
        }
        enc.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_pit_distance_checks_both_sides_of_tied_ecdf_jumps() {
        assert_eq!(uniform_pit_distance(&mut [0.75, 0.75, 0.75, 0.75]), 0.75);
        assert_eq!(uniform_pit_distance(&mut [0.25, 0.25, 0.25, 0.25]), 0.75);
        assert_eq!(
            uniform_pit_distance(&mut [0.875, 0.125, 0.625, 0.375]),
            0.125
        );
        assert_eq!(uniform_pit_distance(&mut [1.0, 0.0]), 0.5);
    }

    #[test]
    fn weighted_edges_keep_sub_float_mass_and_the_largest_denominator_exact() {
        let denominator = 100_000_000_000_000_000_000_i128;
        for edge in 1..10 {
            for offset in [-1, 0, 1] {
                let mass = ExactQty::new(denominator / 10 * edge + offset, denominator).unwrap();
                assert_eq!(mass.to_f64(), edge as f64 / 10.0);
                assert_eq!(weighted_bin(mass), (edge - i128::from(offset < 0)) as usize);
            }
        }
        for (n, expected) in [(0, 0), (1, 0), (i128::MAX / 2, 4), (i128::MAX, 9)] {
            assert_eq!(weighted_bin(ExactQty::new(n, i128::MAX).unwrap()), expected);
        }
        let half_up = ExactQty::new(i128::MAX / 2 + 1, i128::MAX).unwrap();
        assert_eq!(weighted_bin(half_up), 5);
    }

    #[test]
    fn weighted_ties_signed_zero_and_extremes_require_no_value_arithmetic() {
        let samples = [-f64::MAX, -0.0, 0.0, f64::MAX];
        let weights = [1, 2, 5, 2].map(|n| ExactQty::new(n, 10).unwrap());
        for zero in [-0.0, 0.0] {
            let mass = weighted_masses(&samples, &weights, zero).unwrap();
            assert_eq!(mass.below, weights[0]);
            assert_eq!(mass.equal, ExactQty::new(7, 10).unwrap());
            assert_eq!(mass.above, weights[3]);
            assert_eq!(
                mass.cdf_bounds().unwrap(),
                (weights[0], ExactQty::new(4, 5).unwrap())
            );
        }
        assert_eq!(
            weighted_masses(&samples, &weights, f64::MAX)
                .unwrap()
                .cdf_bounds()
                .unwrap(),
            (ExactQty::new(4, 5).unwrap(), ExactQty::from_integer(1)),
        );
    }

    #[test]
    fn empirical_edges_signed_zero_extremes_and_degenerate_samples_need_no_interpolation() {
        for numerator in 0..=10 {
            assert_eq!(empirical_bin(numerator, 10), numerator.min(9));
        }
        assert_eq!(empirical_bin(1, 3), 3);
        assert_eq!(empirical_bin(2, 3), 6);
        let samples = [-f64::MAX, -0.0, 0.0, f64::MAX];
        for zero in [-0.0, 0.0] {
            assert_eq!(
                empirical_counts(&samples, zero),
                EmpiricalRankCounts {
                    below: 1,
                    equal: 2,
                    above: 1,
                }
            );
        }
        assert_eq!(
            empirical_counts(&samples, -f64::MAX),
            EmpiricalRankCounts {
                below: 0,
                equal: 1,
                above: 3,
            }
        );
        assert_eq!(
            empirical_counts(&samples, f64::MAX),
            EmpiricalRankCounts {
                below: 3,
                equal: 1,
                above: 0,
            }
        );
    }

    #[test]
    fn decile_edges_tails_and_invalid_probabilities_have_explicit_semantics() {
        for (index, edge) in EDGES.iter().enumerate() {
            assert_eq!(bin_index(*edge).unwrap(), index.min(9));
        }
        assert_eq!(bin_index(f64::from_bits(0.1_f64.to_bits() - 1)).unwrap(), 0);
        assert_eq!(bin_index(f64::from_bits(0.1_f64.to_bits() + 1)).unwrap(), 1);
        for value in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            assert!(bin_index(value).is_err());
        }
    }
}
