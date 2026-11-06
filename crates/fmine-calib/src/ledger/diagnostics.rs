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
const EDGES: [f64; 11] = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiagnosticValue {
    /// Absent only when this Gaussian member has no selected outcome.
    GaussianPit(Option<f64>),
    /// The unchanged forecast probability, including for unresolved members.
    EventProbability(f64),
    /// A distribution without a supported diagnostic; no CDF is invented from sparse quantiles.
    Unsupported(&'static str),
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
                ForecastPayload::Empirical { .. } => DiagnosticValue::Unsupported("empirical"),
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
            digest: Digest32::default(),
        };
        report.digest = blake3_hash(&report.canonical_bytes());
        Ok(report)
    }
}

impl AssessmentDiagnostics {
    /// DGN1/v1 binds the saved parent, complete member partition, exact result bits and diagnostic rules.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"DGN1", 1);
        for rule in [SCOPE, BIN_RULE, PIT_METHOD] {
            enc.put_str(rule);
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
            let item = report
                .members
                .iter()
                .find(|item| item.member.forecast_id == *id)
                .unwrap();
            let DiagnosticValue::GaussianPit(Some(actual)) = item.value else {
                panic!("resolved Gaussian");
            };
            assert!((actual - expected).abs() < 7.5e-8);
        }
        let pending = report
            .members
            .iter()
            .find(|item| item.member.forecast_id == ids[3])
            .unwrap();
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
        let parent = ledger
            .assess(cohort, 101, OutcomeSelector::FirstPublished)
            .unwrap()
            .digest;
        let report = ledger.assessment_diagnostics(parent).unwrap();
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
        let parent = ledger
            .assess(cohort, 101, OutcomeSelector::FirstPublished)
            .unwrap()
            .digest;
        let report = ledger.assessment_diagnostics(parent).unwrap();
        let pit = report.gaussian_pit.unwrap();
        assert_eq!(pit.bins[0].count, 1);
        assert_eq!(pit.bins[9].count, 1);
    }

    #[test]
    fn newer_outcomes_and_restatements_cannot_rewrite_saved_diagnostics() {
        let (mut ledger, ids, cohort) = build_ledger(vec![gaussian(); 2]);
        publish(&mut ledger, ids[0], 10.0);
        let old = ledger
            .assess(cohort, 101, OutcomeSelector::LatestAsOf)
            .unwrap()
            .digest;
        let original = ledger.assessment_diagnostics(old).unwrap();
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
        let new = ledger
            .assess(cohort, 103, OutcomeSelector::LatestAsOf)
            .unwrap()
            .digest;
        let latest = ledger.assessment_diagnostics(new).unwrap();
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
    fn mixed_continuous_payloads_keep_unsupported_members_without_fabricating_pit() {
        let (mut ledger, ids, cohort) = build_ledger(vec![
            gaussian(),
            ForecastPayload::Empirical {
                samples: vec![8.0, 10.0, 12.0],
            },
        ]);
        publish(&mut ledger, ids[0], 10.0);
        publish(&mut ledger, ids[1], 10.0);
        let parent = ledger
            .assess(cohort, 101, OutcomeSelector::FirstPublished)
            .unwrap()
            .digest;
        let report = ledger.assessment_diagnostics(parent).unwrap();
        assert_eq!(report.members.len(), 2);
        assert_eq!(report.gaussian_pit.unwrap().n_members, 1);
        assert_eq!(
            report
                .members
                .iter()
                .find(|item| item.member.forecast_id == ids[1])
                .unwrap()
                .value,
            DiagnosticValue::Unsupported("empirical")
        );
        let (mut ledger, _, cohort) = build_ledger(vec![ForecastPayload::Quantiles {
            levels: vec![0.1, 0.9],
            values: vec![0.0, 1.0],
        }]);
        let parent = ledger
            .assess(cohort, 3, OutcomeSelector::FirstPublished)
            .unwrap()
            .digest;
        let report = ledger.assessment_diagnostics(parent).unwrap();
        assert!(report.gaussian_pit.is_none());
        assert!(report.bernoulli_reliability.is_none());
        assert_eq!(
            report.members[0].value,
            DiagnosticValue::Unsupported("quantiles")
        );
    }
}
