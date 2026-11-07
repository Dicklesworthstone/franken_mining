//! DESIGN (BD-048/049, plan §10.2): descriptive diagnostics of an immutable saved assessment.
//! No outcome is reselected, no authority time is advanced, and no independence or calibration is inferred.
//! Determinism: DeterministicPerIsa
//! Uses the existing deterministic normal-CDF owner and fixed bin edges.

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_evidence::Refusal;
use fmine_numerics::norm_cdf;

use super::{
    ForecastLedger, ForecastPayload, MemberAssessment, MemberOutcome, OutcomeSelector, absent,
    domain, floats, optional_float,
};

pub const SCOPE: &str = "Descriptive diagnostics within one frozen cohort, not calibration verdicts, independence estimates, confidence intervals or predictive-skill claims. Unsupported distributions and unresolved members remain visible.";
pub const BIN_RULE: &str = "Fixed deciles [lower, upper), with the final bin including 1; exact edge values enter the bin on their right. Completion bounds retain every unresolved member and are not simultaneous confidence intervals.";
pub const PIT_METHOD: &str = "Gaussian Phi((outcome-mean)/std_dev), using fmine_numerics::norm_cdf; approximate CDF values and floating-point tail saturation are retained without clipping or randomization.";
pub const EMPIRICAL_METHOD: &str = "Uniform empirical CDF bounds [F(y-),F(y)] = [below/n,(below+equal)/n], retaining exact sample counts and ties. Closed rank intervals are counted in each fixed decile as definite when wholly contained, and possible when intersecting. Every unresolved member contributes to each whole-population upper bound. Integer comparisons determine bin membership; no randomized PIT, uniformity or calibration is inferred.";
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
    pub bernoulli_reliability: Option<ReliabilityDiagram>,
    pub empirical_rank: Option<EmpiricalRankDiagram>,
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

impl ForecastLedger {
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
        let mut rank_bins = std::array::from_fn(|i| EmpiricalRankBin {
            lower_decile: i as u8,
            upper_decile: (i + 1) as u8,
            definite_count: 0,
            possible_count: 0,
        });
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
            members,
            gaussian_pit,
            bernoulli_reliability,
            empirical_rank: (empirical_n > 0).then_some(EmpiricalRankDiagram {
                n_members: empirical_n,
                n_resolved: empirical_resolved,
                n_unresolved: empirical_n - empirical_resolved,
                bins: rank_bins,
            }),
            digest: Digest32::default(),
        };
        report.digest = blake3_hash(&report.canonical_bytes());
        Ok(report)
    }
}

impl AssessmentDiagnostics {
    /// DGN1/v2 adds exact empirical ranks only when present. Other reports retain their DGN1/v1 bytes.
    /// Both bind the saved parent, complete member partition, result values and diagnostic rules.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let version = if self.empirical_rank.is_some() { 2 } else { 1 };
        let mut enc = Encoder::new_frame(*b"DGN1", version);
        for rule in [SCOPE, BIN_RULE, PIT_METHOD] {
            enc.put_str(rule);
        }
        if version == 2 {
            enc.put_str(EMPIRICAL_METHOD);
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
        if let Some(ranks) = &self.empirical_rank {
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
    use crate::{AssessmentPolicy, CohortSpec, MissingReason, OutcomeCitation, OutcomeKind};
    use fmine_core::id::ObjectId;

    fn build_ledger(payloads: Vec<ForecastPayload>) -> (ForecastLedger, Vec<ObjectId>, Digest32) {
        let mut ledger = ForecastLedger::new();
        let ids = payloads
            .into_iter()
            .enumerate()
            .map(|(i, payload)| {
                // Publications at 100 must be strictly after the registered resolution deadline.
                ledger
                    .register_payload(format!("synthetic-{i}"), 1, 99, payload)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let cohort = ledger
            .freeze_cohort(CohortSpec {
                name: "diagnostic cohort".into(),
                frozen_at_ns: 2,
                members: ids.clone(),
                resolution_rule: "synthetic fixed population".into(),
                eligible_sources: vec!["synthetic".into()],
                policy: AssessmentPolicy::default(),
            })
            .unwrap();
        (ledger, ids, cohort)
    }

    fn citation(vintage: &str) -> OutcomeCitation {
        OutcomeCitation {
            source: "synthetic".into(),
            vintage: vintage.into(),
            locator: "synthetic:whole".into(),
            sha256: fmine_core::hash::sha256(vintage.as_bytes()).0,
        }
    }

    fn publish(ledger: &mut ForecastLedger, id: ObjectId, value: f64) {
        ledger
            .append_outcome(
                id,
                100,
                100,
                OutcomeKind::Published {
                    value,
                    citation: citation("original"),
                },
            )
            .unwrap();
    }

    fn gaussian() -> ForecastPayload {
        ForecastPayload::Gaussian {
            mean: 10.0,
            std_dev: 2.0,
        }
    }

    fn diagnostics(
        ledger: &mut ForecastLedger,
        cohort: Digest32,
        as_of_ns: i64,
        selector: OutcomeSelector,
    ) -> AssessmentDiagnostics {
        let parent = ledger.assess(cohort, as_of_ns, selector).unwrap().digest;
        ledger.assessment_diagnostics(parent).unwrap()
    }

    fn member(report: &AssessmentDiagnostics, id: ObjectId) -> &DiagnosticMember {
        report
            .members
            .iter()
            .find(|item| item.member.forecast_id == id)
            .unwrap()
    }

    #[test]
    fn non_empirical_diagnostics_match_the_prior_v1_wire_layout() {
        for (payload, probability) in [
            (gaussian(), None),
            (ForecastPayload::Bernoulli { probability: 0.25 }, Some(0.25_f64)),
        ] {
            let (mut ledger, ids, cohort) = build_ledger(vec![payload]);
            let parent = ledger
                .assess(cohort, 3, OutcomeSelector::FirstPublished)
                .unwrap()
                .digest;
            let report = ledger.assessment_diagnostics(parent).unwrap();
            assert!(report.empirical_rank.is_none());
            // FACT: the pre-extension layout at 24848416. This single unresolved fixture spells out
            // its prior wire values independently, including absent empirical extension markers.
            let mut expected = Encoder::new_frame(*b"DGN1", 1);
            for rule in [SCOPE, BIN_RULE, PIT_METHOD] {
                expected.put_str(rule);
            }
            expected.put_fixed(&parent.0);
            expected.put_fixed(&cohort.0);
            expected.put_i64(3);
            expected.put_u8(0); // first-published
            expected.put_u64(1); // one member
            expected.put_fixed(&ids[0].bytes);
            expected.put_u8(0); // no selected event
            expected.put_u8(1); // unresolved
            expected.put_u8(0); // pending
            if let Some(probability) = probability {
                expected.put_u8(1); // event diagnostic
                expected.put_u64(probability.to_bits());
                expected.put_u8(0); // no Gaussian histogram
                expected.put_u8(1); // reliability diagram
                for count in [1, 0, 1] {
                    expected.put_u64(count);
                }
                for (index, edges) in EDGES.windows(2).enumerate() {
                    for edge in edges {
                        expected.put_u64(edge.to_bits());
                    }
                    let populated = index == 2;
                    expected.put_u64(u64::from(populated));
                    expected.put_u64(0); // resolved
                    expected.put_u64(0); // events
                    expected.put_u8(u8::from(populated));
                    if populated {
                        expected.put_u64(0.25_f64.to_bits());
                    }
                    expected.put_u8(0); // no resolved mean
                    expected.put_u8(0); // no observed frequency
                    expected.put_u8(u8::from(populated));
                    if populated {
                        expected.put_u64(0.0_f64.to_bits());
                        expected.put_u64(1.0_f64.to_bits());
                    }
                }
            } else {
                expected.put_u8(0); // Gaussian diagnostic
                expected.put_u8(0); // no selected PIT
                expected.put_u8(1); // Gaussian histogram
                for count in [1, 0, 1] {
                    expected.put_u64(count);
                }
                for edges in EDGES.windows(2) {
                    for edge in edges {
                        expected.put_u64(edge.to_bits());
                    }
                    expected.put_u64(0); // resolved count
                    expected.put_u64(0.0_f64.to_bits());
                    expected.put_u64(1.0_f64.to_bits());
                }
                expected.put_u8(0); // no reliability diagram
            }
            assert_eq!(report.canonical_bytes(), expected.finish());
            assert_eq!(report.digest, blake3_hash(&report.canonical_bytes()));
        }
    }

    #[test]
    fn empirical_ties_and_missing_members_have_exact_rank_counts_and_completion_bounds() {
        let payload = ForecastPayload::Empirical {
            samples: vec![-2.0, 0.0, 0.0, 4.0],
        };
        let (mut ledger, ids, cohort) = build_ledger(vec![payload; 8]);
        for (id, outcome) in ids.iter().zip([-3.0, -2.0, -1.0, 0.0, 4.0, 5.0]) {
            publish(&mut ledger, *id, outcome);
        }
        ledger
            .append_outcome(
                ids[6],
                100,
                100,
                OutcomeKind::Missing {
                    reason: MissingReason::NotPublished,
                },
            )
            .unwrap();
        let parent = ledger
            .assess(cohort, 101, OutcomeSelector::FirstPublished)
            .unwrap()
            .clone();
        let parent_bytes = parent.canonical_bytes();
        let report = ledger.assessment_diagnostics(parent.digest).unwrap();
        let ranks = report.empirical_rank.as_ref().unwrap();
        assert_eq!(
            (ranks.n_members, ranks.n_resolved, ranks.n_unresolved),
            (8, 6, 2)
        );
        assert_eq!(
            ranks.bins.map(|bin| bin.definite_count),
            [1, 0, 1, 0, 0, 0, 0, 0, 0, 1]
        );
        assert_eq!(
            ranks.bins.map(|bin| bin.possible_count),
            [2, 1, 3, 1, 1, 1, 1, 2, 1, 2]
        );
        // The [2/10,3/10) bin has full-population bounds [1/8,5/8], not [1/6,3/6].
        assert_eq!(
            (
                ranks.bins[2].definite_count,
                ranks.bins[2].possible_count + ranks.n_unresolved
            ),
            (1, 5)
        );
        for (index, (below, equal, above)) in [
            (0, 0, 4),
            (0, 1, 3),
            (1, 0, 3),
            (1, 2, 1),
            (3, 1, 0),
            (4, 0, 0),
        ]
        .into_iter()
        .enumerate()
        {
            let item = member(&report, ids[index]);
            assert_eq!(
                item.value,
                DiagnosticValue::EmpiricalRank {
                    n_samples: 4,
                    counts: Some(EmpiricalRankCounts {
                        below,
                        equal,
                        above,
                    }),
                }
            );
        }
        for (index, selected_event) in [(6, true), (7, false)] {
            let item = member(&report, ids[index]);
            assert_eq!(
                item.value,
                DiagnosticValue::EmpiricalRank {
                    n_samples: 4,
                    counts: None,
                }
            );
            assert_eq!(item.member.selected_event.is_some(), selected_event);
        }
        assert_eq!(
            report
                .members
                .iter()
                .map(|item| &item.member)
                .collect::<Vec<_>>(),
            parent.per_member.iter().collect::<Vec<_>>()
        );
        assert!(report.gaussian_pit.is_none());
        assert!(report.bernoulli_reliability.is_none());
        assert_eq!(&report.canonical_bytes()[8..10], &2_u16.to_le_bytes());
        assert_eq!(report.digest, blake3_hash(&report.canonical_bytes()));
        assert_eq!(
            ledger
                .assess(cohort, 101, OutcomeSelector::FirstPublished)
                .unwrap()
                .canonical_bytes(),
            parent_bytes
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
        for samples in [vec![7.0], vec![7.0; crate::scoring::MAX_EMPIRICAL_SAMPLES]] {
            let n_samples = samples.len();
            let (mut ledger, ids, cohort) =
                build_ledger(vec![ForecastPayload::Empirical { samples }]);
            publish(&mut ledger, ids[0], 7.0);
            let report = diagnostics(&mut ledger, cohort, 101, OutcomeSelector::FirstPublished);
            assert_eq!(
                report.members[0].value,
                DiagnosticValue::EmpiricalRank {
                    n_samples,
                    counts: Some(EmpiricalRankCounts {
                        below: 0,
                        equal: n_samples,
                        above: 0,
                    }),
                }
            );
            let ranks = report.empirical_rank.unwrap();
            assert!(
                ranks
                    .bins
                    .iter()
                    .all(|bin| bin.definite_count == 0 && bin.possible_count == 1)
            );
        }
    }

    #[test]
    fn empirical_sample_permutations_preserve_ranks_but_keep_distinct_frozen_identities() {
        let reports = [
            vec![-2.0, 0.0, 0.0, 4.0],
            vec![0.0, 4.0, -2.0, 0.0],
        ]
        .map(|samples| {
            let (mut ledger, ids, cohort) =
                build_ledger(vec![ForecastPayload::Empirical { samples }]);
            publish(&mut ledger, ids[0], 0.0);
            diagnostics(&mut ledger, cohort, 101, OutcomeSelector::FirstPublished)
        });
        assert_eq!(reports[0].empirical_rank, reports[1].empirical_rank);
        assert_eq!(reports[0].members[0].value, reports[1].members[0].value);
        assert_ne!(
            reports[0].members[0].member.forecast_id,
            reports[1].members[0].member.forecast_id
        );
        assert_ne!(reports[0].assessment_digest, reports[1].assessment_digest);
        assert_ne!(reports[0].digest, reports[1].digest);
    }

    #[test]
    fn empirical_restatements_preserve_saved_counts_sources_and_parent_bytes() {
        let payload = ForecastPayload::Empirical {
            samples: vec![-2.0, 0.0, 0.0, 4.0],
        };
        let (mut ledger, ids, cohort) = build_ledger(vec![payload; 2]);
        publish(&mut ledger, ids[0], 0.0);
        let old = ledger
            .assess(cohort, 101, OutcomeSelector::LatestAsOf)
            .unwrap()
            .clone();
        let old_bytes = old.canonical_bytes();
        let saved = ledger.assessment_diagnostics(old.digest).unwrap();
        ledger
            .append_outcome(
                ids[0],
                102,
                102,
                OutcomeKind::Restated {
                    value: 4.0,
                    citation: citation("revision"),
                    supersedes: 1,
                },
            )
            .unwrap();
        ledger
            .append_outcome(
                ids[1],
                102,
                102,
                OutcomeKind::Published {
                    value: 1.0,
                    citation: citation("original"),
                },
            )
            .unwrap();
        let changed = diagnostics(&mut ledger, cohort, 103, OutcomeSelector::LatestAsOf);
        assert_ne!(changed.digest, saved.digest);
        assert_eq!(changed.empirical_rank.as_ref().unwrap().n_unresolved, 0);
        let original = member(&saved, ids[0]);
        let revised = member(&changed, ids[0]);
        assert!(
            matches!(&original.member.outcome, MemberOutcome::Resolved { citation, .. } if citation.vintage == "original")
        );
        assert!(
            matches!(&revised.member.outcome, MemberOutcome::Resolved { citation, .. } if citation.vintage == "revision")
        );
        assert_eq!(ledger.assessment_diagnostics(old.digest).unwrap(), saved);
        assert_eq!(
            ledger
                .assess(cohort, 101, OutcomeSelector::LatestAsOf)
                .unwrap()
                .canonical_bytes(),
            old_bytes
        );
        let first = diagnostics(&mut ledger, cohort, 103, OutcomeSelector::FirstPublished);
        let item = member(&first, ids[0]);
        assert_eq!(item.value, original.value);
        assert_eq!(item.member.selected_event, original.member.selected_event);
    }

    #[test]
    fn gaussian_pit_matches_known_normal_values_and_retains_missing_members() {
        let (mut ledger, ids, cohort) = build_ledger(vec![gaussian(); 4]);
        for (id, value) in ids.iter().zip([8.0, 10.0, 12.0]) {
            publish(&mut ledger, *id, value);
        }
        ledger
            .append_outcome(
                ids[3],
                100,
                100,
                OutcomeKind::Missing {
                    reason: MissingReason::NotPublished,
                },
            )
            .unwrap();
        let parent = ledger
            .assess(cohort, 101, OutcomeSelector::FirstPublished)
            .unwrap()
            .clone();
        let bytes = parent.canonical_bytes();
        let report = ledger.assessment_diagnostics(parent.digest).unwrap();
        let pit = report.gaussian_pit.as_ref().unwrap();
        assert_eq!((pit.n_members, pit.n_resolved, pit.n_unresolved), (4, 3, 1));
        assert_eq!(
            pit.bins.map(|bin| bin.count),
            [0, 1, 0, 0, 0, 1, 0, 0, 1, 0]
        );
        assert_eq!(pit.bins[5].whole_population_fraction, (0.25, 0.5));
        assert_eq!(pit.bins[0].whole_population_fraction, (0.0, 0.25));
        for (id, expected) in
            ids.iter()
                .zip([0.158_655_253_931_457_07, 0.5, 0.841_344_746_068_542_9])
        {
            let item = member(&report, *id);
            let DiagnosticValue::GaussianPit(Some(actual)) = item.value else {
                panic!("resolved Gaussian");
            };
            assert!((actual - expected).abs() < 7.5e-8);
        }
        let pending = member(&report, ids[3]);
        assert_eq!(pending.value, DiagnosticValue::GaussianPit(None));
        assert!(pending.member.selected_event.is_some());
        assert!(report.bernoulli_reliability.is_none());
        assert_eq!(report.digest, blake3_hash(&report.canonical_bytes()));
        assert_eq!(
            ledger
                .assess(cohort, 101, OutcomeSelector::FirstPublished)
                .unwrap()
                .canonical_bytes(),
            bytes
        );
    }

    #[test]
    fn reliability_keeps_resolved_and_full_populations_distinct_and_empty_bins_absent() {
        let (mut ledger, ids, cohort) = build_ledger(
            [0.1, 0.15, 0.19, 0.0, 1.0]
                .map(|probability| ForecastPayload::Bernoulli { probability })
                .to_vec(),
        );
        for (index, value) in [(0, 1.0), (1, 0.0), (3, 0.0), (4, 1.0)] {
            publish(&mut ledger, ids[index], value);
        }
        let report = diagnostics(&mut ledger, cohort, 101, OutcomeSelector::FirstPublished);
        let diagram = report.bernoulli_reliability.unwrap();
        assert_eq!(
            (diagram.n_members, diagram.n_resolved, diagram.n_unresolved),
            (5, 4, 1)
        );
        let bin = diagram.bins[1];
        assert_eq!((bin.n_members, bin.n_resolved, bin.n_events), (3, 2, 1));
        assert!((bin.mean_probability.unwrap() - 0.44 / 3.0).abs() < 1e-15);
        assert_eq!(bin.mean_resolved_probability, Some(0.125));
        assert_eq!(bin.observed_frequency, Some(0.5));
        assert_eq!(bin.whole_bin_frequency, Some((1.0 / 3.0, 2.0 / 3.0)));
        assert_eq!(diagram.bins[0].whole_bin_frequency, Some((0.0, 0.0)));
        assert_eq!(diagram.bins[9].whole_bin_frequency, Some((1.0, 1.0)));
        let empty = diagram.bins[2];
        assert_eq!(
            (
                empty.mean_probability,
                empty.mean_resolved_probability,
                empty.observed_frequency,
                empty.whole_bin_frequency
            ),
            (None, None, None, None)
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
        let (mut ledger, ids, cohort) = build_ledger(vec![gaussian(); 2]);
        publish(&mut ledger, ids[0], -90.0);
        publish(&mut ledger, ids[1], 110.0);
        let report = diagnostics(&mut ledger, cohort, 101, OutcomeSelector::FirstPublished);
        let pit = report.gaussian_pit.unwrap();
        assert_eq!(pit.bins[0].count, 1);
        assert_eq!(pit.bins[9].count, 1);
    }

    #[test]
    fn newer_outcomes_and_restatements_cannot_rewrite_saved_diagnostics() {
        let (mut ledger, ids, cohort) = build_ledger(vec![gaussian(); 2]);
        publish(&mut ledger, ids[0], 10.0);
        let original = diagnostics(&mut ledger, cohort, 101, OutcomeSelector::LatestAsOf);
        let old = original.assessment_digest;
        ledger
            .append_outcome(
                ids[0],
                102,
                102,
                OutcomeKind::Restated {
                    value: 8.0,
                    citation: citation("revision"),
                    supersedes: 1,
                },
            )
            .unwrap();
        ledger
            .append_outcome(
                ids[1],
                102,
                102,
                OutcomeKind::Published {
                    value: 12.0,
                    citation: citation("original"),
                },
            )
            .unwrap();
        let latest = diagnostics(&mut ledger, cohort, 103, OutcomeSelector::LatestAsOf);
        assert_ne!(latest.digest, original.digest);
        assert_eq!(ledger.assessment_diagnostics(old).unwrap(), original);
        assert_eq!(latest.gaussian_pit.unwrap().n_unresolved, 0);
        assert!(ledger.assessment_diagnostics(Digest32::default()).is_err());
        // Projection is read-only and cannot close a later authority cut.
        ledger
            .register_payload("later", 104, 200, gaussian())
            .unwrap();
    }

    #[test]
    fn mixed_continuous_payloads_keep_empirical_ranks_separate_and_sparse_quantiles_unsupported() {
        let (mut ledger, ids, cohort) = build_ledger(vec![
            gaussian(),
            ForecastPayload::Empirical {
                samples: vec![8.0, 10.0, 12.0],
            },
        ]);
        publish(&mut ledger, ids[0], 10.0);
        publish(&mut ledger, ids[1], 10.0);
        let report = diagnostics(&mut ledger, cohort, 101, OutcomeSelector::FirstPublished);
        assert_eq!(report.members.len(), 2);
        assert_eq!(report.gaussian_pit.as_ref().unwrap().n_members, 1);
        assert_eq!(
            member(&report, ids[1]).value,
            DiagnosticValue::EmpiricalRank {
                n_samples: 3,
                counts: Some(EmpiricalRankCounts {
                    below: 1,
                    equal: 1,
                    above: 1,
                }),
            }
        );
        assert_eq!(report.empirical_rank.as_ref().unwrap().n_members, 1);
        assert!(report.bernoulli_reliability.is_none());
        let (mut ledger, _, cohort) = build_ledger(vec![ForecastPayload::Quantiles {
            levels: vec![0.1, 0.9],
            values: vec![0.0, 1.0],
        }]);
        let report = diagnostics(&mut ledger, cohort, 3, OutcomeSelector::FirstPublished);
        assert!(report.gaussian_pit.is_none());
        assert!(report.bernoulli_reliability.is_none());
        assert_eq!(
            report.members[0].value,
            DiagnosticValue::Unsupported("quantiles")
        );
    }
}
