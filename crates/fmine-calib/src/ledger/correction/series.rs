//! Complete saved-assessment series for a later forecast. Public supports advance without overlap;
//! repeated forecasts, revisions and cited observations cannot enlarge the fitting population.
//! Determinism: DeterministicPerIsa

use std::collections::BTreeSet;

use super::*;
use crate::ledger::binding::{PublicPeriod, PublicResolutionPolicy, PublicVintagePolicy};
use crate::ledger::{Assessment, Cohort};

pub const SERIES_METHOD: &str = "pooled-log-bias-gaussian-inflation-series-v1";
pub const SERIES_NO_CLAIM: &str = "Unissued correction proposal fitted on an explicitly selected series of complete saved assessments, not a registered forecast, hierarchical fit or calibration result. Every selected member and source remains visible; selecting reports does not establish a complete historical record or independent observations. Public training and target supports use the same declared definition, ownership, vintage permission and calendar period kind, without overlap. Fitted coverage reuses training outcomes and is not out-of-sample evidence. No original forecast, outcome, assessment or journal record is changed; separate prospective registration and later scoring are required.";
pub const ADDITIVE_SERIES_METHOD: &str = "pooled-additive-residual-gaussian-inflation-series-v1";
pub const ADDITIVE_SERIES_NO_CLAIM: &str = "Unissued additive correction proposal fitted on an explicitly selected series of complete saved assessments, not a registered forecast, hierarchical fit or calibration result. Residuals and the mean shift retain the target's declared unit; predictive standard deviations are inflated without rescaling or narrowing. Signed and zero observations remain in every selected complete cohort; selection does not establish a complete historical record or independent observations. Public training and target supports retain the same declared definition, ownership, vintage permission and calendar period kind without overlap. Fitted coverage is not out-of-sample evidence. No original forecast, outcome, assessment or journal record is changed; separate prospective registration and later scoring are required.";

#[derive(Debug, Clone, PartialEq)]
pub struct CorrectionTrainingAssessment {
    pub assessment_digest: Digest32,
    pub cohort: Cohort,
    pub as_of_ns: i64,
    pub selector: OutcomeSelector,
}

/// The full frozen parent and target cohorts retain each definition and population used in admission.
/// Members follow canonical parent order, then the original saved assessment's forecast-id order.
#[derive(Debug, Clone, PartialEq)]
pub struct CorrectionSeriesProposal {
    pub method: CorrectionMethod,
    pub parents: Vec<CorrectionTrainingAssessment>,
    pub target_cohorts: Vec<Cohort>,
    pub training_as_of_ns: i64,
    pub selector: OutcomeSelector,
    pub alpha: f64,
    pub metadata: ForecastMetadata,
    pub members: Vec<CorrectionMember>,
    pub mean_error: f64,
    pub sample_error_std_dev: f64,
    pub adjustment: f64,
    pub inflation: f64,
    pub required_training_hits: usize,
    pub raw_training_hits: usize,
    pub corrected_training_hits: usize,
    pub raw: FrozenForecast,
    pub raw_interval: (f64, f64),
    pub corrected: ForecastPayload,
    pub corrected_interval: (f64, f64),
    pub digest: Digest32,
}

struct Training<'a> {
    parent: CorrectionTrainingAssessment,
    assessment: &'a Assessment,
    public: Option<(PublicResolutionPolicy, PublicVintagePolicy)>,
}

// Calendar month endpoints are half-open. Equal kinds admit unequal quarter/year durations, including
// leap years, while refusing cumulative/quarter support substitution and overlapping observations.
fn support(period: PublicPeriod) -> (u8, i32, i32) {
    match period {
        PublicPeriod::Year(year) => (0, i32::from(year) * 12, i32::from(year) * 12 + 12),
        PublicPeriod::FirstHalf(year) => (1, i32::from(year) * 12, i32::from(year) * 12 + 6),
        PublicPeriod::NineMonths(year) => (2, i32::from(year) * 12, i32::from(year) * 12 + 9),
        PublicPeriod::Quarter { year, quarter } => {
            let start = i32::from(year) * 12 + (i32::from(quarter) - 1) * 3;
            (3, start, start + 3)
        }
    }
}

fn compatible_public(
    expected: &(PublicResolutionPolicy, PublicVintagePolicy),
    actual: &(PublicResolutionPolicy, PublicVintagePolicy),
) -> Result<(), Refusal> {
    if expected.0.issuer != actual.0.issuer
        || expected.0.metric != actual.0.metric
        || expected.0.ownership != actual.0.ownership
        || expected.1 != actual.1
        || support(expected.0.period).0 != support(actual.0.period).0
    {
        return Err(domain(
            "correction series requires one public issuer, metric, ownership, vintage permission and calendar period kind",
        ));
    }
    Ok(())
}

fn validate_definitions(training: &[Training<'_>], targets: &[Cohort]) -> Result<(), Refusal> {
    let first = training
        .first()
        .ok_or_else(|| domain("correction series has no training assessments"))?;
    if let Some(expected) = &first.public {
        if targets.is_empty() {
            return Err(domain(
                "public correction series requires a frozen public target cohort",
            ));
        }
        let mut previous_end = None;
        for row in training {
            let actual = row.public.as_ref().ok_or_else(|| {
                domain("correction series cannot mix public and supplied resolution policies")
            })?;
            compatible_public(expected, actual)?;
            let (_, start, end) = support(actual.0.period);
            if previous_end.is_some_and(|previous| start < previous) {
                return Err(domain(
                    "correction public training supports must advance without overlap",
                ));
            }
            previous_end = Some(end);
        }
        for target in targets {
            let actual = PublicResolutionPolicy::parse_resolution_rule_with_vintages(
                &target.spec.resolution_rule,
            )?
            .ok_or_else(|| {
                domain("public correction target requires a public resolution policy")
            })?;
            compatible_public(expected, &actual)?;
            if previous_end.is_some_and(|end| support(actual.0.period).1 < end) {
                return Err(late(
                    "correction public target support must follow every training support",
                ));
            }
        }
    } else {
        let spec = &first.parent.cohort.spec;
        for row in training {
            if row.public.is_some()
                || row.parent.cohort.spec.resolution_rule != spec.resolution_rule
                || row.parent.cohort.spec.eligible_sources != spec.eligible_sources
            {
                return Err(domain(
                    "supplied correction series requires the same resolution rule and eligible sources",
                ));
            }
        }
        for target in targets {
            if target.spec.resolution_rule != spec.resolution_rule
                || target.spec.eligible_sources != spec.eligible_sources
            {
                return Err(domain(
                    "supplied correction target has a different resolution rule or eligible sources",
                ));
            }
        }
    }
    Ok(())
}

impl ForecastLedger {
    /// Fit the existing correction method over complete explicitly selected saved assessments.
    /// Public training periods and the already frozen target retain one compatible physical definition.
    pub fn correction_series_proposal(
        &self,
        assessment_digests: &[Digest32],
        raw_forecast_id: ObjectId,
    ) -> Result<CorrectionSeriesProposal, Refusal> {
        self.correction_series_proposal_with_method(
            assessment_digests,
            raw_forecast_id,
            CorrectionMethod::MultiplicativeLog,
        )
    }

    /// Retain the same complete source-bound series while explicitly choosing log ratios or signed
    /// additive residuals. Method selection never changes the training population or source policy.
    pub fn correction_series_proposal_with_method(
        &self,
        assessment_digests: &[Digest32],
        raw_forecast_id: ObjectId,
        method: CorrectionMethod,
    ) -> Result<CorrectionSeriesProposal, Refusal> {
        if assessment_digests.len() < 2 {
            return Err(domain(
                "correction series requires at least two saved assessments",
            ));
        }
        let raw = self
            .records
            .get(&raw_forecast_id)
            .ok_or_else(|| absent("correction target forecast is not registered"))?;
        if raw.freeze_time_ns <= raw.registered_ns || !self.outcomes(raw_forecast_id).is_empty() {
            return Err(late(
                "correction target must precede its deadline and have no recorded outcome event",
            ));
        }
        let metadata = raw
            .metadata
            .as_ref()
            .ok_or_else(|| domain("correction requires prospectively declared target metadata"))?;
        gaussian(&raw.payload, method)?;
        let mut reports = BTreeSet::new();
        let mut cohorts = BTreeSet::new();
        let mut training = Vec::new();
        for digest in assessment_digests {
            if !reports.insert(*digest) {
                return Err(domain("correction series repeats a saved assessment"));
            }
            let assessment = self
                .assessments
                .values()
                .find(|report| report.digest == *digest)
                .ok_or_else(|| absent("correction requires an existing immutable assessment"))?;
            if !cohorts.insert(assessment.cohort) {
                return Err(domain(
                    "correction series repeats a cohort or its revised assessment",
                ));
            }
            let cohort = self
                .cohorts
                .get(&assessment.cohort)
                .ok_or_else(|| absent("correction training cohort is unavailable"))?;
            if assessment.as_of_ns >= raw.registered_ns {
                return Err(late(
                    "correction target must follow every saved training cut",
                ));
            }
            if assessment.summary.n_unresolved != 0
                || assessment
                    .per_member
                    .iter()
                    .map(|m| m.forecast_id)
                    .collect::<Vec<_>>()
                    != cohort.spec.members
            {
                return Err(domain(
                    "correction series requires every member of every complete cohort",
                ));
            }
            training.push(Training {
                parent: CorrectionTrainingAssessment {
                    assessment_digest: *digest,
                    cohort: cohort.clone(),
                    as_of_ns: assessment.as_of_ns,
                    selector: assessment.selector,
                },
                assessment,
                public: PublicResolutionPolicy::parse_resolution_rule_with_vintages(
                    &cohort.spec.resolution_rule,
                )?,
            });
        }
        training.sort_by_key(|row| {
            (
                row.public
                    .as_ref()
                    .map(|(policy, _)| support(policy.period).1),
                row.parent.as_of_ns,
                row.parent.assessment_digest,
            )
        });
        let first = training
            .first()
            .ok_or_else(|| domain("correction series has no training assessments"))?;
        let alpha = first.parent.cohort.spec.policy.alpha;
        let selector = first.parent.selector;
        let target_cohorts = self
            .cohorts
            .values()
            .filter(|cohort| cohort.spec.members.contains(&raw_forecast_id))
            .cloned()
            .collect::<Vec<_>>();
        if training.iter().any(|row| {
            row.parent.selector != selector
                || row.parent.cohort.spec.policy.alpha.to_bits() != alpha.to_bits()
        }) || target_cohorts
            .iter()
            .any(|cohort| cohort.spec.policy.alpha.to_bits() != alpha.to_bits())
        {
            return Err(domain(
                "correction series and target require matching selector and exact alpha",
            ));
        }
        validate_definitions(&training, &target_cohorts)?;
        let mut seen_forecasts = BTreeSet::new();
        let mut seen_observations = BTreeSet::new();
        let mut selected = Vec::new();
        let mut training_as_of_ns = i64::MIN;
        for row in &training {
            training_as_of_ns = training_as_of_ns.max(row.parent.as_of_ns);
            for member in &row.assessment.per_member {
                if !seen_forecasts.insert(member.forecast_id) {
                    return Err(domain("correction series repeats a training forecast"));
                }
                let MemberOutcome::Resolved {
                    value,
                    event_seq,
                    event_digest,
                    citation,
                } = &member.outcome
                else {
                    return Err(domain(
                        "an unresolved member cannot be excluded from correction fitting",
                    ));
                };
                let event = self
                    .outcomes(member.forecast_id)
                    .iter()
                    .find(|event| event.seq == *event_seq && event.digest == *event_digest)
                    .ok_or_else(|| absent("correction selected outcome event is unavailable"))?;
                if event.availability_time_ns >= raw.registered_ns
                    || event.record_time_ns >= raw.registered_ns
                {
                    return Err(late(
                        "correction target must follow every selected outcome's availability and record time",
                    ));
                }
                if !event.kind.publication().is_some_and(|(actual, source)| {
                    actual.to_bits() == value.to_bits() && source == citation
                }) {
                    return Err(domain(
                        "correction selected source differs from its retained outcome event",
                    ));
                }
                if !seen_observations.insert((
                    citation.source.clone(),
                    citation.vintage.clone(),
                    citation.locator.clone(),
                    citation.sha256,
                )) {
                    return Err(domain(
                        "correction series repeats the same cited observation",
                    ));
                }
                selected.push(member.clone());
            }
        }
        let fit = self.fit_correction(raw, metadata, alpha, &selected, method)?;
        let mut report = CorrectionSeriesProposal {
            method,
            parents: training.into_iter().map(|row| row.parent).collect(),
            target_cohorts,
            training_as_of_ns,
            selector,
            alpha,
            metadata: metadata.clone(),
            members: fit.members,
            mean_error: fit.mean_error,
            sample_error_std_dev: fit.sample_error_std_dev,
            adjustment: fit.adjustment,
            inflation: fit.inflation,
            required_training_hits: fit.required_training_hits,
            raw_training_hits: fit.raw_training_hits,
            corrected_training_hits: fit.corrected_training_hits,
            raw: raw.clone(),
            raw_interval: fit.raw_interval,
            corrected: fit.corrected,
            corrected_interval: fit.corrected_interval,
            digest: Digest32([0; 32]),
        };
        report.digest = series_digest(&report);
        Ok(report)
    }
}

fn series_digest(report: &CorrectionSeriesProposal) -> Digest32 {
    let mut enc = Encoder::new_frame(*b"COR2", report.method.encoding_version());
    enc.put_str(report.method.series_name());
    enc.put_str(report.method.series_no_claim());
    enc.put_u64(report.parents.len() as u64);
    for parent in &report.parents {
        enc.put_fixed(&parent.assessment_digest.0);
        enc.put_fixed(&parent.cohort.digest.0);
        enc.put_i64(parent.as_of_ns);
        enc.put_u8(match parent.selector {
            OutcomeSelector::FirstPublished => 0,
            OutcomeSelector::LatestAsOf => 1,
        });
    }
    enc.put_u64(report.target_cohorts.len() as u64);
    for cohort in &report.target_cohorts {
        enc.put_fixed(&cohort.digest.0);
    }
    enc.put_i64(report.training_as_of_ns);
    enc.put_u8(match report.selector {
        OutcomeSelector::FirstPublished => 0,
        OutcomeSelector::LatestAsOf => 1,
    });
    report.metadata.encode(&mut enc);
    report.raw.encode(&mut enc);
    report.corrected.encode(&mut enc);
    floats(
        &mut enc,
        &[
            report.alpha,
            report.mean_error,
            report.sample_error_std_dev,
            report.adjustment,
            report.inflation,
            report.raw_interval.0,
            report.raw_interval.1,
            report.corrected_interval.0,
            report.corrected_interval.1,
        ],
    );
    for count in [
        report.required_training_hits,
        report.raw_training_hits,
        report.corrected_training_hits,
        report.members.len(),
    ] {
        enc.put_u64(count as u64);
    }
    for member in &report.members {
        member.raw.encode(&mut enc);
        floats(
            &mut enc,
            &[
                member.error,
                member.corrected_interval.0,
                member.corrected_interval.1,
            ],
        );
        enc.put_u8(u8::from(member.raw_inside));
        enc.put_u8(u8::from(member.corrected_inside));
        member.selected.encode(&mut enc);
    }
    blake3_hash(&enc.finish())
}

#[cfg(test)]
mod tests;
