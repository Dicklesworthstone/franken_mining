//! DESIGN (BD-047/BD-049, plan §10.1): prospective forecast identity and descriptive segment reports.
//! Grouping never selects winners, estimates independence or confers calibration. Legacy records remain
//! explicitly unclassified; their model key is not retrospectively interpreted as an author or quantity.
//! Determinism: DeterministicPerIsa (inherited score arithmetic; canonical ordering and exact input bits).

use std::collections::BTreeMap;

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_core::id::ObjectId;
use fmine_evidence::Refusal;

use super::{
    AssessmentSummary, CoverageSummary, EventLogScoreSummary, ForecastLedger, FrozenForecast,
    MissingReason, OutcomeSelector, ScoreSummary, absent, domain, floats, optional_float,
    score_refusal, summarize,
};

/// Declared labels, frozen with the predictive distribution. Use role/model identifiers for authors;
/// this public-lane record is not a place for personal names. Labels do not authenticate their own truth.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ForecastMetadata {
    pub family: String,
    pub author_role: String,
    pub domain: String,
    pub horizon: String,
    pub site: String,
    /// The measurand and basis, for example `gold_production_100pct` or `aisc_per_sold_oz`.
    pub quantity: String,
    /// Exact declared unit; different unit labels are never silently converted or pooled.
    pub unit: String,
}

impl ForecastMetadata {
    pub fn validate(&self) -> Result<(), Refusal> {
        for (name, value) in self.fields() {
            if value.is_empty()
                || value.trim() != value
                || value.len() > 256
                || value.chars().any(char::is_control)
            {
                return Err(domain(&format!(
                    "forecast {name} must be a nonempty, trimmed label of at most 256 bytes without control characters"
                )));
            }
        }
        Ok(())
    }

    fn fields(&self) -> [(&'static str, &str); 7] {
        [
            ("family", &self.family),
            ("author_role", &self.author_role),
            ("domain", &self.domain),
            ("horizon", &self.horizon),
            ("site", &self.site),
            ("quantity", &self.quantity),
            ("unit", &self.unit),
        ]
    }

    pub(super) fn encode(&self, enc: &mut Encoder) {
        for (_, value) in self.fields() {
            enc.put_str(value);
        }
    }
}

/// Every declared grouping dimension is retained; no author-wide or family-wide score silently pools
/// different sites, horizons, domains, models, quantities or units.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ForecastSegment {
    pub model_key: String,
    pub metadata: ForecastMetadata,
}

/// Arithmetic scores at a saved assessment's frozen alpha, with unresolved members in every bound.
/// No effective-sample estimate, confidence interval or calibration verdict is inferred for a subgroup.
#[derive(Debug, Clone, PartialEq)]
pub struct DescriptiveSummary {
    pub n_members: usize,
    pub n_resolved: usize,
    pub n_unresolved: usize,
    pub unresolved_by_reason: BTreeMap<MissingReason, usize>,
    pub coverage: Option<CoverageSummary>,
    pub crps: Option<ScoreSummary>,
    pub gaussian_crps_bounds: bool,
    pub interval_score: Option<ScoreSummary>,
    pub brier: Option<ScoreSummary>,
    pub event_log: Option<EventLogScoreSummary>,
    pub mean_standardized_bias: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SegmentAssessment {
    pub segment: ForecastSegment,
    pub members: Vec<ObjectId>,
    pub summary: DescriptiveSummary,
}

/// A read-only derivation of an existing assessment. Its separate digest cannot replace or reinterpret
/// the immutable parent assessment. Legacy members have no invented metadata or pooled subgroup score.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentedAssessment {
    pub assessment_digest: Digest32,
    pub cohort: Digest32,
    pub as_of_ns: i64,
    pub selector: OutcomeSelector,
    pub alpha: f64,
    pub n_members: usize,
    pub groups: Vec<SegmentAssessment>,
    pub unclassified_members: Vec<ObjectId>,
    pub digest: Digest32,
}

pub(super) fn validate_cohort_metadata(
    records: &BTreeMap<ObjectId, FrozenForecast>,
    members: &[ObjectId],
) -> Result<bool, Refusal> {
    let mut reference = None;
    let mut unknown = false;
    for id in members {
        let record = records
            .get(id)
            .ok_or_else(|| absent("cohort member is not registered"))?;
        if let Some(metadata) = &record.metadata {
            let identity = (&metadata.family, &metadata.quantity, &metadata.unit);
            if reference.is_some_and(|prior| prior != identity) {
                return Err(domain(
                    "a metadata-bearing cohort requires one exact forecast family, quantity and unit; use separate cohorts",
                ));
            }
            reference = Some(identity);
        } else {
            unknown = true;
        }
    }
    if unknown && reference.is_some() {
        return Err(domain(
            "legacy and metadata-bearing forecasts need separate cohorts; unknown quantity identity cannot be inferred",
        ));
    }
    Ok(reference.is_some())
}

impl ForecastLedger {
    /// Derive all full-key groups of a saved assessment without a new clock, outcome selection or seal.
    /// A digest not already present in this ledger refuses, including one from a caller-mutated report.
    pub fn segmented_assessment(
        &self,
        assessment_digest: Digest32,
    ) -> Result<SegmentedAssessment, Refusal> {
        let assessment = self
            .assessments
            .values()
            .find(|assessment| assessment.digest == assessment_digest)
            .ok_or_else(|| absent("segmentation requires an existing immutable assessment"))?;
        let cohort = self
            .cohorts
            .get(&assessment.cohort)
            .ok_or_else(|| absent("assessment cohort is unavailable"))?;
        let mut groups: BTreeMap<ForecastSegment, Vec<_>> = BTreeMap::new();
        let mut unclassified_members = Vec::new();
        for member in &assessment.per_member {
            let record = self
                .records
                .get(&member.forecast_id)
                .ok_or_else(|| absent("assessment member is unavailable"))?;
            if let Some(metadata) = &record.metadata {
                groups
                    .entry(ForecastSegment {
                        model_key: record.model_key.clone(),
                        metadata: metadata.clone(),
                    })
                    .or_default()
                    .push((record, member));
            } else {
                unclassified_members.push(member.forecast_id);
            }
        }
        let groups = groups
            .into_iter()
            .map(|(segment, entries)| {
                let records = entries
                    .iter()
                    .map(|(record, _)| *record)
                    .collect::<Vec<_>>();
                let members = entries
                    .iter()
                    .map(|(_, member)| (*member).clone())
                    .collect::<Vec<_>>();
                let summary =
                    summarize(&records, &members, &cohort.spec.policy).map_err(score_refusal)?;
                Ok(SegmentAssessment {
                    segment,
                    members: members.iter().map(|member| member.forecast_id).collect(),
                    summary: summary.into(),
                })
            })
            .collect::<Result<Vec<_>, Refusal>>()?;
        let mut report = SegmentedAssessment {
            assessment_digest,
            cohort: assessment.cohort,
            as_of_ns: assessment.as_of_ns,
            selector: assessment.selector,
            alpha: cohort.spec.policy.alpha,
            n_members: assessment.per_member.len(),
            groups,
            unclassified_members,
            digest: Digest32::default(),
        };
        report.digest = blake3_hash(&report.canonical_bytes());
        Ok(report)
    }
}

impl From<AssessmentSummary> for DescriptiveSummary {
    fn from(summary: AssessmentSummary) -> Self {
        Self {
            n_members: summary.n_members,
            n_resolved: summary.n_resolved,
            n_unresolved: summary.n_unresolved,
            unresolved_by_reason: summary.unresolved_by_reason,
            coverage: summary.coverage,
            crps: summary.crps,
            gaussian_crps_bounds: summary.gaussian_crps_bounds,
            interval_score: summary.interval_score,
            brier: summary.brier,
            event_log: summary.event_log,
            mean_standardized_bias: summary.mean_standardized_bias,
        }
    }
}

impl SegmentedAssessment {
    /// SGM1/v1 binds the exact parent assessment, complete partition and descriptive calculations.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"SGM1", 1);
        enc.put_fixed(&self.assessment_digest.0);
        enc.put_fixed(&self.cohort.0);
        enc.put_i64(self.as_of_ns);
        enc.put_u8(match self.selector {
            OutcomeSelector::FirstPublished => 0,
            OutcomeSelector::LatestAsOf => 1,
        });
        floats(&mut enc, &[self.alpha]);
        enc.put_u64(self.n_members as u64);
        enc.put_u64(self.groups.len() as u64);
        for group in &self.groups {
            enc.put_str(&group.segment.model_key);
            group.segment.metadata.encode(&mut enc);
            enc.put_u64(group.members.len() as u64);
            for id in &group.members {
                enc.put_fixed(&id.bytes);
            }
            group.summary.encode(&mut enc);
        }
        enc.put_u64(self.unclassified_members.len() as u64);
        for id in &self.unclassified_members {
            enc.put_fixed(&id.bytes);
        }
        enc.finish()
    }
}

impl DescriptiveSummary {
    fn encode(&self, enc: &mut Encoder) {
        for count in [self.n_members, self.n_resolved, self.n_unresolved] {
            enc.put_u64(count as u64);
        }
        enc.put_u64(self.unresolved_by_reason.len() as u64);
        for (reason, count) in &self.unresolved_by_reason {
            enc.put_u8(reason.tag());
            enc.put_u64(*count as u64);
        }
        enc.put_u8(u8::from(self.coverage.is_some()));
        if let Some(coverage) = self.coverage {
            enc.put_u64(coverage.inside as u64);
            optional_float(enc, coverage.observed_fraction);
            floats(enc, &[coverage.whole_cohort.0, coverage.whole_cohort.1]);
        }
        enc.put_u8(u8::from(self.gaussian_crps_bounds));
        for score in [self.crps, self.interval_score, self.brier] {
            enc.put_u8(u8::from(score.is_some()));
            if let Some(score) = score {
                encode_score(enc, score);
            }
        }
        enc.put_u8(u8::from(self.event_log.is_some()));
        if let Some(log) = self.event_log {
            floats(enc, &[log.probability_floor]);
            encode_score(enc, log.score);
        }
        optional_float(enc, self.mean_standardized_bias);
    }
}

fn encode_score(enc: &mut Encoder, score: ScoreSummary) {
    enc.put_u64(score.n_scored as u64);
    optional_float(enc, score.observed_mean);
    floats(enc, &[score.whole_cohort.lower]);
    optional_float(enc, score.whole_cohort.upper);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AssessmentPolicy, CohortSpec, ForecastPayload, OutcomeCitation, OutcomeKind};

    fn metadata() -> ForecastMetadata {
        ForecastMetadata {
            family: "production".into(),
            author_role: "model:baseline-v1".into(),
            domain: "oxide".into(),
            horizon: "one-quarter".into(),
            site: "synthetic-mine".into(),
            quantity: "gold_production_100pct".into(),
            unit: "koz".into(),
        }
    }

    fn interval() -> ForecastPayload {
        ForecastPayload::Interval {
            lo: 0.0,
            hi: 2.0,
            nominal: Some(0.8),
        }
    }

    fn register(
        ledger: &mut ForecastLedger,
        model: &str,
        due: i64,
        metadata: ForecastMetadata,
    ) -> ObjectId {
        ledger
            .register_payload_with_metadata(model, 1, due, interval(), metadata)
            .unwrap()
    }

    fn spec(members: Vec<ObjectId>) -> CohortSpec {
        CohortSpec {
            name: "synthetic segments".into(),
            frozen_at_ns: 2,
            members,
            resolution_rule: "fixed synthetic quarterly production".into(),
            eligible_sources: vec!["synthetic".into()],
            policy: AssessmentPolicy {
                outcome_bounds: Some((0.0, 4.0)),
                ..AssessmentPolicy::default()
            },
        }
    }

    fn citation(vintage: &str) -> OutcomeCitation {
        OutcomeCitation {
            source: "synthetic".into(),
            vintage: vintage.into(),
            locator: "synthetic:whole-mine".into(),
            sha256: fmine_core::hash::sha256(vintage.as_bytes()).0,
        }
    }

    fn publish(ledger: &mut ForecastLedger, id: ObjectId, value: f64) {
        ledger
            .append_outcome(
                id,
                200,
                200,
                OutcomeKind::Published {
                    value,
                    citation: citation("original"),
                },
            )
            .unwrap();
    }

    #[test]
    fn every_metadata_dimension_changes_the_frozen_identity_and_stays_immutable() {
        let original = metadata();
        let mut variants = vec![original.clone(); 7];
        variants[0].family = "throughput".into();
        variants[1].author_role = "agent:other@v1".into();
        variants[2].domain = "sulfide".into();
        variants[3].horizon = "one-year".into();
        variants[4].site = "other-synthetic-mine".into();
        variants[5].quantity = "gold_production_attributable".into();
        variants[6].unit = "oz".into();
        let mut ledger = ForecastLedger::new();
        let first = register(&mut ledger, "model-v1", 100, original.clone());
        let mut ids = std::collections::BTreeSet::from([first]);
        for variant in variants {
            assert!(ids.insert(register(&mut ledger, "model-v1", 100, variant)));
        }
        assert_eq!(
            ledger.forecast(first).unwrap().metadata,
            Some(original.clone())
        );
        assert!(
            ledger
                .register_payload_with_metadata("model-v1", 1, 100, interval(), original)
                .is_err()
        );
        let legacy = ledger
            .register_payload("model-v1", 1, 100, interval())
            .unwrap();
        assert!(ids.insert(legacy));
        assert!(ledger.forecast(legacy).unwrap().metadata.is_none());
    }

    #[test]
    fn bad_metadata_and_incompatible_cohorts_refuse_without_advancing_authority() {
        for bad in [
            "".to_string(),
            " padded".into(),
            "bad\nrole".into(),
            "x".repeat(257),
        ] {
            let mut ledger = ForecastLedger::new();
            let mut invalid = metadata();
            invalid.author_role = bad;
            assert!(
                ledger
                    .register_payload_with_metadata("m", 50, 100, interval(), invalid)
                    .is_err()
            );
            register(&mut ledger, "m", 100, metadata());
        }
        for mismatch in 0..4 {
            let mut ledger = ForecastLedger::new();
            let a = register(&mut ledger, "m", 100, metadata());
            let b = if mismatch == 3 {
                ledger
                    .register_payload("other", 1, 100, interval())
                    .unwrap()
            } else {
                let mut changed = metadata();
                match mismatch {
                    0 => changed.family = "other-family".into(),
                    1 => changed.quantity = "different-measurand".into(),
                    _ => changed.unit = "oz".into(),
                }
                register(&mut ledger, "other", 100, changed)
            };
            let mut invalid = spec(vec![a, b]);
            invalid.frozen_at_ns = 50;
            assert!(ledger.freeze_cohort(invalid).is_err());
            // A failed freeze at 50 leaves the legitimate authority write at 2 available.
            ledger.freeze_cohort(spec(vec![a])).unwrap();
        }
        let mut ledger = ForecastLedger::new();
        assert!(
            ledger
                .register_payload_with_metadata("m", 100, 100, interval(), metadata())
                .is_err()
        );
        register(&mut ledger, "m", 100, metadata());
    }

    #[test]
    fn segments_keep_missing_members_exact_scores_and_historical_vintages() {
        let mut ledger = ForecastLedger::new();
        let a = register(&mut ledger, "m", 100, metadata());
        let b = register(&mut ledger, "m", 101, metadata());
        let mut other_site = metadata();
        other_site.site = "second-mine".into();
        let c = register(&mut ledger, "m", 100, other_site);
        let cohort = ledger.freeze_cohort(spec(vec![c, b, a])).unwrap();
        publish(&mut ledger, a, 3.0);
        publish(&mut ledger, c, 1.0);
        ledger
            .append_outcome(
                b,
                200,
                200,
                OutcomeKind::Missing {
                    reason: MissingReason::NotPublished,
                },
            )
            .unwrap();
        let assessment = ledger
            .assess(cohort, 201, OutcomeSelector::FirstPublished)
            .unwrap()
            .clone();
        let report = ledger.segmented_assessment(assessment.digest).unwrap();
        assert_eq!(report.n_members, 3);
        assert_eq!(report.groups.len(), 2);
        assert!(report.unclassified_members.is_empty());
        let group = report
            .groups
            .iter()
            .find(|g| g.members.contains(&a))
            .unwrap();
        assert_eq!(group.members.len(), 2);
        assert_eq!(group.summary.n_resolved, 1);
        assert_eq!(group.summary.n_unresolved, 1);
        assert_eq!(
            group.summary.unresolved_by_reason[&MissingReason::NotPublished],
            1
        );
        let score = group.summary.interval_score.unwrap();
        // Independent arithmetic: width 2; miss by 1 with 2/alpha=10 gives 12. Pending loss is [2,22].
        assert_eq!(score.observed_mean, Some(12.0));
        assert_eq!(score.whole_cohort.lower, 7.0);
        assert_eq!(score.whole_cohort.upper, Some(17.0));
        assert_eq!(group.summary.coverage.unwrap().whole_cohort, (0.0, 0.5));
        assert_eq!(report.digest, blake3_hash(&report.canonical_bytes()));
        ledger
            .append_outcome(
                a,
                202,
                202,
                OutcomeKind::Restated {
                    value: 1.0,
                    citation: citation("restated"),
                    supersedes: 1,
                },
            )
            .unwrap();
        let latest = ledger
            .assess(cohort, 203, OutcomeSelector::LatestAsOf)
            .unwrap()
            .digest;
        let changed = ledger.segmented_assessment(latest).unwrap();
        let changed_group = changed
            .groups
            .iter()
            .find(|g| g.members.contains(&a))
            .unwrap();
        assert_eq!(
            changed_group.summary.interval_score.unwrap().observed_mean,
            Some(2.0)
        );
        assert_ne!(report.digest, changed.digest);
        assert_eq!(
            ledger.segmented_assessment(assessment.digest).unwrap(),
            report
        );
        assert_eq!(
            ledger
                .assess(cohort, 201, OutcomeSelector::FirstPublished)
                .unwrap(),
            &assessment
        );
        assert!(ledger.segmented_assessment(Digest32::default()).is_err());
    }

    #[test]
    fn full_declared_key_partitions_every_member_without_inferring_legacy_labels() {
        let mut ledger = ForecastLedger::new();
        let original = metadata();
        let mut variants = vec![original.clone(); 5];
        variants[1].author_role = "model:other-v1".into();
        variants[2].domain = "sulfide".into();
        variants[3].horizon = "one-year".into();
        variants[4].site = "second-mine".into();
        let mut ids = variants
            .into_iter()
            .map(|m| register(&mut ledger, "model-v1", 100, m))
            .collect::<Vec<_>>();
        ids.push(register(&mut ledger, "model-v2", 100, original));
        let cohort = ledger.freeze_cohort(spec(ids.clone())).unwrap();
        let digest = ledger
            .assess(cohort, 3, OutcomeSelector::FirstPublished)
            .unwrap()
            .digest;
        let report = ledger.segmented_assessment(digest).unwrap();
        assert_eq!(report.groups.len(), ids.len());
        assert!(
            report
                .groups
                .windows(2)
                .all(|pair| pair[0].segment < pair[1].segment)
        );
        let all_members = report
            .groups
            .iter()
            .flat_map(|g| &g.members)
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(all_members, ids.into_iter().collect());
        assert!(report.groups.iter().all(|g| g.summary.n_unresolved == 1));

        let mut legacy = ForecastLedger::new();
        let id = legacy
            .register_payload("model:looks-like-author", 1, 100, interval())
            .unwrap();
        let cohort = legacy.freeze_cohort(spec(vec![id])).unwrap();
        let digest = legacy
            .assess(cohort, 3, OutcomeSelector::FirstPublished)
            .unwrap()
            .digest;
        let report = legacy.segmented_assessment(digest).unwrap();
        assert_eq!(report.unclassified_members, vec![id]);
        assert!(report.groups.is_empty());
        // Read-only segmentation neither creates an assessment nor advances the sealed authority cut.
        legacy
            .register_payload("later", 4, 100, interval())
            .unwrap();
    }

    #[test]
    fn public_binding_refuses_conflicting_metadata_before_freezing() {
        use crate::ledger::binding::{
            PublicMetric, PublicOwnership, PublicPeriod, PublicResolutionPolicy,
        };

        for mismatch in 0..4 {
            let mut ledger = ForecastLedger::new();
            let mut labels = metadata();
            labels.quantity = "production_koz".into();
            match mismatch {
                0 => labels.site = "a-different-mine".into(),
                1 => labels.quantity = "aisc_usd_per_oz".into(),
                2 => labels.unit = "oz".into(),
                _ => {}
            }
            let id = register(&mut ledger, "m", 100, labels);
            let public = PublicResolutionPolicy {
                issuer: "barrick".into(),
                metric: PublicMetric::ProductionKoz,
                period: PublicPeriod::Quarter {
                    year: 2026,
                    quarter: 1,
                },
                ownership: PublicOwnership::Full,
                members: BTreeMap::from([(id, "synthetic-mine".into())]),
            };
            let mut cohort = spec(vec![id]);
            cohort.resolution_rule = public.to_resolution_rule().unwrap();
            cohort.eligible_sources = vec!["barrick".into()];
            let result = ledger.freeze_cohort(cohort);
            assert_eq!(result.is_ok(), mismatch == 3);
        }
    }

    #[test]
    fn event_groups_retain_full_brier_and_log_loss_denominators() {
        let mut ledger = ForecastLedger::new();
        let mut labels = metadata();
        labels.quantity = "annual_target_met".into();
        labels.unit = "event".into();
        let a = ledger
            .register_payload_with_metadata(
                "event-model",
                1,
                100,
                ForecastPayload::Bernoulli { probability: 0.25 },
                labels.clone(),
            )
            .unwrap();
        let b = ledger
            .register_payload_with_metadata(
                "event-model",
                1,
                101,
                ForecastPayload::Bernoulli { probability: 0.75 },
                labels,
            )
            .unwrap();
        let mut cohort = spec(vec![a, b]);
        cohort.policy.event_log_floor = Some(0.01);
        let cohort = ledger.freeze_cohort(cohort).unwrap();
        publish(&mut ledger, b, 1.0);
        let assessment = ledger
            .assess(cohort, 201, OutcomeSelector::FirstPublished)
            .unwrap()
            .digest;
        let grouped = ledger.segmented_assessment(assessment).unwrap();
        let summary = &grouped.groups[0].summary;
        assert!(summary.coverage.is_none());
        assert!(summary.interval_score.is_none());
        let brier = summary.brier.unwrap();
        assert_eq!(brier.observed_mean, Some(0.0625));
        assert_eq!(brier.whole_cohort.lower, 0.0625);
        assert_eq!(brier.whole_cohort.upper, Some(0.3125));
        let event_log = summary.event_log.unwrap();
        assert_eq!(event_log.probability_floor, 0.01);
        assert!((event_log.score.observed_mean.unwrap() + 0.75_f64.ln()).abs() < 1e-14);
        assert!((event_log.score.whole_cohort.lower + 0.75_f64.ln()).abs() < 1e-14);
        let upper = -(0.75_f64.ln() + 0.25_f64.ln()) / 2.0;
        assert!((event_log.score.whole_cohort.upper.unwrap() - upper).abs() < 1e-14);
    }
}
