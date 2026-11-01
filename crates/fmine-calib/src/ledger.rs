//! Frozen forecast cohorts and append-only outcome vintages (BD-343, §10.1, §21.6).
//! Determinism: DeterministicPerIsa (canonical frames contain exact floating-point bits; scoring uses `det`).
//!
//! DESIGN: this pure kernel enforces ordering, not wall-clock authenticity. The caller must obtain record times
//! from its authority adapter; an external timestamp proof remains BD-068. Assessments seal their as-of cuts:
//! a later append cannot backdate an event into a report already produced. No missing outcome is imputed.

use std::collections::BTreeMap;

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::norm_inv;

use crate::{
    CalibrationFlag, CalibrationReport, ScoreError, gaussian_crps, winkler_interval_score,
};

pub mod binding;

/// Plan §21.6's floor on independent outcomes for a positive calibration claim. A policy may be stricter.
pub const MIN_CALIBRATION_N: usize = 35;

/// A frozen predictive distribution, or a guidance range whose probability level is explicitly unstated.
#[derive(Debug, Clone, PartialEq)]
pub enum ForecastPayload {
    Gaussian {
        mean: f64,
        std_dev: f64,
    },
    Interval {
        lo: f64,
        hi: f64,
        nominal: Option<f64>,
    },
    Quantiles {
        levels: Vec<f64>,
        values: Vec<f64>,
    },
    /// An event probability, needed for bounded Brier-score assessments.
    Bernoulli {
        probability: f64,
    },
}

impl ForecastPayload {
    fn validate(&self) -> Result<(), Refusal> {
        let ok = match self {
            Self::Gaussian { mean, std_dev } => {
                mean.is_finite()
                    && std_dev.is_finite()
                    && *std_dev > 0.0
                    && (mean - 1.281_551_565_544_600_4 * std_dev).is_finite()
                    && (mean + 1.281_551_565_544_600_4 * std_dev).is_finite()
            }
            Self::Interval { lo, hi, nominal } => {
                lo.is_finite()
                    && hi.is_finite()
                    && lo <= hi
                    && (hi - lo).is_finite()
                    && nominal.is_none_or(|p| p.is_finite() && p > 0.0 && p < 1.0)
            }
            Self::Quantiles { levels, values } => {
                levels.len() >= 2
                    && levels.len() == values.len()
                    && levels.iter().all(|p| p.is_finite() && *p > 0.0 && *p < 1.0)
                    && levels.windows(2).all(|w| w[0] < w[1])
                    && values.iter().all(|x| x.is_finite())
                    && values.windows(2).all(|w| w[0] <= w[1])
            }
            Self::Bernoulli { probability } => {
                probability.is_finite() && (0.0..=1.0).contains(probability)
            }
        };
        if ok {
            Ok(())
        } else {
            Err(domain("invalid predictive distribution"))
        }
    }

    fn interval(&self, alpha: f64) -> Result<Option<(f64, f64)>, ScoreError> {
        let pair = match self {
            Self::Gaussian { mean, std_dev } => {
                // Preserve the established 80% interval's bits (the existing score oracle uses this constant).
                let z = if alpha == 0.2 {
                    1.281_551_565_544_600_4
                } else {
                    norm_inv(1.0 - alpha / 2.0)
                };
                (*mean - z * std_dev, *mean + z * std_dev)
            }
            Self::Interval { lo, hi, nominal } => {
                if nominal.is_some_and(|p| (p - (1.0 - alpha)).abs() > 1e-12) {
                    return Err(ScoreError::InvalidParameter {
                        name: "nominal_level",
                        value: nominal.unwrap_or(0.0),
                    });
                }
                (*lo, *hi)
            }
            Self::Quantiles { levels, values } => {
                let at = |p: f64| {
                    levels
                        .iter()
                        .position(|x| (*x - p).abs() < 1e-12)
                        .and_then(|i| values.get(i).copied())
                        .ok_or(ScoreError::InvalidParameter {
                            name: "missing_quantile_level",
                            value: p,
                        })
                };
                (at(alpha / 2.0)?, at(1.0 - alpha / 2.0)?)
            }
            Self::Bernoulli { .. } => return Ok(None),
        };
        if pair.0.is_finite() && pair.1.is_finite() && (pair.1 - pair.0).is_finite() {
            Ok(Some(pair))
        } else {
            Err(ScoreError::NonFinite)
        }
    }

    fn encode(&self, enc: &mut Encoder) {
        match self {
            Self::Gaussian { mean, std_dev } => {
                enc.put_u8(0);
                floats(enc, &[*mean, *std_dev]);
            }
            Self::Interval { lo, hi, nominal } => {
                enc.put_u8(1);
                floats(enc, &[*lo, *hi]);
                optional_float(enc, *nominal);
            }
            Self::Quantiles { levels, values } => {
                enc.put_u8(2);
                enc.put_u64(levels.len() as u64);
                for (p, x) in levels.iter().zip(values) {
                    floats(enc, &[*p, *x]);
                }
            }
            Self::Bernoulli { probability } => {
                enc.put_u8(3);
                floats(enc, &[*probability]);
            }
        }
    }
}

/// Frozen input; outcomes are separate events and never mutate this record.
#[derive(Debug, Clone, PartialEq)]
pub struct FrozenForecast {
    pub forecast_id: ObjectId,
    pub model_key: String,
    pub registered_ns: i64,
    pub freeze_time_ns: i64,
    pub payload: ForecastPayload,
}

impl FrozenForecast {
    fn encode(&self, enc: &mut Encoder) {
        enc.put_fixed(&self.forecast_id.bytes);
        enc.put_str(&self.model_key);
        enc.put_i64(self.registered_ns);
        enc.put_i64(self.freeze_time_ns);
        self.payload.encode(enc);
    }
}

/// All scoring assumptions are fixed with membership, before outcomes.
#[derive(Debug, Clone, PartialEq)]
pub struct AssessmentPolicy {
    /// Central interval tail probability (0.2 means an 80% interval).
    pub alpha: f64,
    pub min_calibration_n: usize,
    /// Equicorrelation range; the lower effective sample uses the upper correlation.
    pub correlation_range: (f64, f64),
    /// A declared physical outcome domain can make interval scores bounded. Observations outside it refuse.
    pub outcome_bounds: Option<(f64, f64)>,
}

impl Default for AssessmentPolicy {
    fn default() -> Self {
        Self {
            alpha: 0.2,
            min_calibration_n: MIN_CALIBRATION_N,
            correlation_range: (0.0, 0.5),
            outcome_bounds: None,
        }
    }
}

impl AssessmentPolicy {
    fn validate(&self) -> Result<(), Refusal> {
        let (a, b) = self.correlation_range;
        if !(self.alpha.is_finite()
            && self.alpha > 0.0
            && self.alpha < 1.0
            && self.min_calibration_n >= MIN_CALIBRATION_N
            && a.is_finite()
            && b.is_finite()
            && a >= 0.0
            && a <= b
            && b <= 1.0)
            || self
                .outcome_bounds
                .is_some_and(|(lo, hi)| !lo.is_finite() || !hi.is_finite() || lo > hi)
        {
            return Err(domain(
                "invalid frozen assessment policy; the calibration minimum must be at least 35",
            ));
        }
        Ok(())
    }

    fn encode(&self, enc: &mut Encoder) {
        floats(
            enc,
            &[
                self.alpha,
                self.correlation_range.0,
                self.correlation_range.1,
            ],
        );
        enc.put_u64(self.min_calibration_n as u64);
        enc.put_u8(u8::from(self.outcome_bounds.is_some()));
        if let Some((lo, hi)) = self.outcome_bounds {
            floats(enc, &[lo, hi]);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CohortSpec {
    pub name: String,
    pub frozen_at_ns: i64,
    pub members: Vec<ObjectId>,
    pub resolution_rule: String,
    pub eligible_sources: Vec<String>,
    pub policy: AssessmentPolicy,
}

/// The ledger exposes only shared references to its cohort records.
#[derive(Debug, Clone, PartialEq)]
pub struct Cohort {
    pub digest: Digest32,
    pub spec: CohortSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeCitation {
    pub source: String,
    pub vintage: String,
    pub locator: String,
    pub sha256: [u8; 32],
}

impl OutcomeCitation {
    fn validate(&self) -> Result<(), Refusal> {
        if self.source.trim().is_empty()
            || self.vintage.trim().is_empty()
            || self.locator.trim().is_empty()
            || self.sha256 == [0; 32]
        {
            Err(domain(
                "a published outcome needs a source, vintage, locator and SHA-256",
            ))
        } else {
            Ok(())
        }
    }

    fn encode(&self, enc: &mut Encoder) {
        enc.put_str(&self.source);
        enc.put_str(&self.vintage);
        enc.put_str(&self.locator);
        enc.put_fixed(&self.sha256);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MissingReason {
    Pending,
    NotPublished,
    Withdrawn,
    DefinitionChanged,
    Divested,
    Overdue,
}

impl MissingReason {
    fn tag(self) -> u8 {
        match self {
            Self::Pending => 0,
            Self::NotPublished => 1,
            Self::Withdrawn => 2,
            Self::DefinitionChanged => 3,
            Self::Divested => 4,
            Self::Overdue => 5,
        }
    }

    fn terminal(self) -> bool {
        matches!(
            self,
            Self::Withdrawn | Self::DefinitionChanged | Self::Divested
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutcomeKind {
    Published {
        value: f64,
        citation: OutcomeCitation,
    },
    Restated {
        value: f64,
        citation: OutcomeCitation,
        supersedes: u64,
    },
    Missing {
        reason: MissingReason,
    },
}

impl OutcomeKind {
    fn publication(&self) -> Option<(f64, &OutcomeCitation)> {
        match self {
            Self::Published { value, citation }
            | Self::Restated {
                value, citation, ..
            } => Some((*value, citation)),
            Self::Missing { .. } => None,
        }
    }

    fn encode(&self, enc: &mut Encoder) {
        match self {
            Self::Published { value, citation } => {
                enc.put_u8(0);
                floats(enc, &[*value]);
                citation.encode(enc);
            }
            Self::Restated {
                value,
                citation,
                supersedes,
            } => {
                enc.put_u8(1);
                floats(enc, &[*value]);
                citation.encode(enc);
                enc.put_u64(*supersedes);
            }
            Self::Missing { reason } => {
                enc.put_u8(2);
                enc.put_u8(reason.tag());
            }
        }
    }
}

/// Hash-chained event; `record_time_ns` and `availability_time_ns` are independently checked by as-of selection.
#[derive(Debug, Clone, PartialEq)]
pub struct OutcomeEvent {
    pub forecast_id: ObjectId,
    pub seq: u64,
    pub availability_time_ns: i64,
    pub record_time_ns: i64,
    pub kind: OutcomeKind,
    pub previous_digest: Digest32,
    pub digest: Digest32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OutcomeSelector {
    FirstPublished,
    LatestAsOf,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MemberOutcome {
    Resolved {
        value: f64,
        event_seq: u64,
        event_digest: Digest32,
        citation: OutcomeCitation,
    },
    Unresolved {
        reason: MissingReason,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemberAssessment {
    pub forecast_id: ObjectId,
    /// Also binds an explicit missing-outcome event; an inferred Pending/Overdue status has no event.
    pub selected_event: Option<(u64, Digest32)>,
    pub outcome: MemberOutcome,
}

/// Bounds on the final whole-cohort average, not a statistical confidence interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoreBounds {
    pub lower: f64,
    /// `None` means no finite upper bound, never zero or a forecast-imputed outcome.
    pub upper: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoreSummary {
    pub n_scored: usize,
    pub observed_mean: Option<f64>,
    pub whole_cohort: ScoreBounds,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoverageSummary {
    pub inside: usize,
    pub observed_fraction: Option<f64>,
    pub whole_cohort: (f64, f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectiveSampleSize {
    pub lower: f64,
    pub upper: f64,
    pub correlation_range: (f64, f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssessmentSummary {
    pub n_members: usize,
    pub n_resolved: usize,
    pub n_unresolved: usize,
    pub unresolved_by_reason: BTreeMap<MissingReason, usize>,
    pub coverage: Option<CoverageSummary>,
    /// Available only for an all-Gaussian cohort, never computed on a hidden subset.
    pub crps: Option<ScoreSummary>,
    pub interval_score: Option<ScoreSummary>,
    /// Available only for an all-Bernoulli cohort.
    pub brier: Option<ScoreSummary>,
    pub effective_n: EffectiveSampleSize,
    pub mean_standardized_bias: Option<f64>,
    pub flag: CalibrationFlag,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Assessment {
    pub cohort: Digest32,
    pub as_of_ns: i64,
    pub selector: OutcomeSelector,
    pub per_member: Vec<MemberAssessment>,
    pub summary: AssessmentSummary,
    pub digest: Digest32,
}

#[derive(Debug, Default, Clone)]
pub struct ForecastLedger {
    records: BTreeMap<ObjectId, FrozenForecast>,
    outcomes: BTreeMap<ObjectId, Vec<OutcomeEvent>>,
    cohorts: BTreeMap<Digest32, Cohort>,
    cohort_names: BTreeMap<String, Digest32>,
    assessments: BTreeMap<(Digest32, i64, OutcomeSelector), Assessment>,
    assessed_through: BTreeMap<ObjectId, i64>,
    /// One authority chronology across forecasts, not merely separate histories for selected members.
    latest_record_ns: Option<i64>,
    sealed_through_ns: Option<i64>,
}

impl ForecastLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Compatibility registration for existing Gaussian callers. BD-343 keeps its historical ids; a cohort's
    /// digest additionally binds the full unrounded payload. New callers use `register_payload` for exact ids.
    pub fn register(
        &mut self,
        model_key: impl Into<String>,
        registered_ns: i64,
        freeze_time_ns: i64,
        mean: f64,
        std_dev: f64,
    ) -> Result<ObjectId, Refusal> {
        let model_key = model_key.into();
        let old = format!("{model_key}:{registered_ns}:{freeze_time_ns}:{mean:.8}:{std_dev:.8}");
        self.insert_forecast(FrozenForecast {
            forecast_id: ObjectId::from_canonical_bytes(PrefixKind::Forecast, old.as_bytes()),
            model_key,
            registered_ns,
            freeze_time_ns,
            payload: ForecastPayload::Gaussian { mean, std_dev },
        })
    }

    /// Exact canonical registration. `freeze_time_ns` is the earliest outcome time / resolution deadline.
    pub fn register_payload(
        &mut self,
        model_key: impl Into<String>,
        registered_ns: i64,
        freeze_time_ns: i64,
        payload: ForecastPayload,
    ) -> Result<ObjectId, Refusal> {
        if registered_ns >= freeze_time_ns {
            return Err(late("registration must precede the outcome time"));
        }
        let model_key = model_key.into();
        let mut enc = Encoder::new_frame(*b"FOR2", 1);
        enc.put_str(&model_key);
        enc.put_i64(registered_ns);
        enc.put_i64(freeze_time_ns);
        payload.encode(&mut enc);
        self.insert_forecast(FrozenForecast {
            forecast_id: ObjectId::from_canonical_bytes(PrefixKind::Forecast, &enc.finish()),
            model_key,
            registered_ns,
            freeze_time_ns,
            payload,
        })
    }

    fn insert_forecast(&mut self, record: FrozenForecast) -> Result<ObjectId, Refusal> {
        if record.registered_ns > record.freeze_time_ns {
            return Err(late("registration is after the outcome time"));
        }
        if record.model_key.trim().is_empty() {
            return Err(domain("forecast model key is empty"));
        }
        record.payload.validate()?;
        if self.records.contains_key(&record.forecast_id) {
            return Err(frozen(
                "forecast id already exists; no duplicate or changed payload may replace it",
            ));
        }
        self.check_record_time(record.registered_ns)?;
        let id = record.forecast_id;
        self.latest_record_ns = Some(record.registered_ns);
        self.records.insert(id, record);
        Ok(id)
    }

    #[must_use]
    pub fn forecast(&self, id: ObjectId) -> Option<&FrozenForecast> {
        self.records.get(&id)
    }

    #[must_use]
    pub fn outcomes(&self, id: ObjectId) -> &[OutcomeEvent] {
        self.outcomes.get(&id).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn cohort(&self, id: Digest32) -> Option<&Cohort> {
        self.cohorts.get(&id)
    }

    /// Freezes a named membership set and policy before any of its outcomes are recorded or due. Sorting inputs
    /// is canonical; duplicate members are a refusal, never counted twice. An existing name cannot be reused.
    pub fn freeze_cohort(&mut self, mut spec: CohortSpec) -> Result<Digest32, Refusal> {
        spec.policy.validate()?;
        if spec.name.trim().is_empty()
            || spec.resolution_rule.trim().is_empty()
            || spec.members.is_empty()
            || spec.eligible_sources.is_empty()
            || spec.eligible_sources.iter().any(|s| s.trim().is_empty())
        {
            return Err(domain(
                "a cohort needs a name, rule, members and eligible sources",
            ));
        }
        if spec.eligible_sources.iter().any(|s| s == "legacy:uncited") {
            return Err(domain(
                "an uncited legacy outcome cannot be an eligible cohort source",
            ));
        }
        spec.members.sort();
        spec.eligible_sources.sort();
        if spec.members.windows(2).any(|w| w[0] == w[1])
            || spec.eligible_sources.windows(2).any(|w| w[0] == w[1])
        {
            return Err(domain("duplicate member or eligible source in a cohort"));
        }
        if self.cohort_names.contains_key(&spec.name) {
            return Err(frozen("cohort name already frozen; create a new cohort"));
        }
        let public_policy = binding::PublicResolutionPolicy::parse_resolution_rule_with_vintages(
            &spec.resolution_rule,
        )?;
        if let Some((policy, vintages)) = &public_policy {
            policy.validate_cohort(&spec)?;
            for cohort in self.cohorts.values() {
                if let Some((prior, prior_vintages)) =
                    binding::PublicResolutionPolicy::parse_resolution_rule_with_vintages(
                        &cohort.spec.resolution_rule,
                    )?
                {
                    policy.validate_shared_targets(&prior)?;
                    if *vintages != prior_vintages
                        && policy
                            .members
                            .keys()
                            .any(|id| prior.members.contains_key(id))
                    {
                        return Err(frozen(
                            "a forecast already has a different frozen public outcome-vintage policy",
                        ));
                    }
                }
            }
        }
        self.check_record_time(spec.frozen_at_ns)?;
        let mut enc = Encoder::new_frame(*b"COH1", 1);
        enc.put_str(&spec.name);
        enc.put_i64(spec.frozen_at_ns);
        enc.put_str(&spec.resolution_rule);
        spec.policy.encode(&mut enc);
        enc.put_u64(spec.eligible_sources.len() as u64);
        for source in &spec.eligible_sources {
            enc.put_str(source);
        }
        enc.put_u64(spec.members.len() as u64);
        let mut bernoulli = None;
        for id in &spec.members {
            let rec = self
                .records
                .get(id)
                .ok_or_else(|| absent("cohort member is not registered"))?;
            if rec.registered_ns >= spec.frozen_at_ns
                || spec.frozen_at_ns >= rec.freeze_time_ns
                || !self.outcomes(*id).is_empty()
            {
                return Err(late(
                    "cohort must freeze after registration and before any outcome is due or recorded",
                ));
            }
            if let Some((policy, _)) = &public_policy {
                policy.validate_forecast(rec)?;
            }
            let event = matches!(rec.payload, ForecastPayload::Bernoulli { .. });
            if event
                && spec
                    .policy
                    .outcome_bounds
                    .is_some_and(|(lo, hi)| lo > 0.0 || hi < 1.0)
            {
                return Err(domain(
                    "a binary outcome domain must include both zero and one",
                ));
            }
            if bernoulli.is_some_and(|b| b != event) {
                return Err(domain(
                    "event and continuous forecasts need separate cohorts",
                ));
            }
            bernoulli = Some(event);
            rec.payload
                .interval(spec.policy.alpha)
                .map_err(score_refusal)?;
            rec.encode(&mut enc);
        }
        let digest = blake3_hash(&enc.finish());
        self.latest_record_ns = Some(spec.frozen_at_ns);
        self.cohort_names.insert(spec.name.clone(), digest);
        self.cohorts.insert(digest, Cohort { digest, spec });
        Ok(digest)
    }

    /// Equal times allow a batch of distinct records; an assessed cut is closed and requires a later time.
    /// Failed writes must leave both watermarks unchanged.
    fn check_record_time(&self, record_ns: i64) -> Result<(), Refusal> {
        if self.latest_record_ns.is_some_and(|t| record_ns < t)
            || self.sealed_through_ns.is_some_and(|t| record_ns <= t)
        {
            return Err(late(
                "write regresses the ledger record time or backdates an already assessed cut",
            ));
        }
        Ok(())
    }

    /// Append a publication, explicit missing status, or a cited restatement of the latest numeric vintage.
    /// Both availability and recording must follow the freeze; recording cannot precede availability, a prior
    /// event, or an already assessed cut. Source eligibility and physical bounds are enforced for every cohort.
    pub fn append_outcome(
        &mut self,
        forecast_id: ObjectId,
        availability_time_ns: i64,
        record_time_ns: i64,
        kind: OutcomeKind,
    ) -> Result<u64, Refusal> {
        let rec = self
            .records
            .get(&forecast_id)
            .ok_or_else(|| absent("outcome names an unknown forecast"))?;
        self.check_record_time(record_time_ns)?;
        if availability_time_ns <= rec.freeze_time_ns
            || record_time_ns < availability_time_ns
            || self
                .assessed_through
                .get(&forecast_id)
                .is_some_and(|t| record_time_ns <= *t)
        {
            return Err(late(
                "outcome is early, backdated, or enters an already assessed cut",
            ));
        }
        let prior = self.outcomes(forecast_id);
        if prior.last().is_some_and(|e| {
            record_time_ns <= e.record_time_ns || availability_time_ns < e.availability_time_ns
        }) {
            return Err(late("outcome timestamps regress behind a recorded event"));
        }
        let last_value = prior.iter().rev().find(|e| e.kind.publication().is_some());
        match &kind {
            OutcomeKind::Published { .. } if last_value.is_some() => {
                return Err(frozen(
                    "a later value must explicitly restate its predecessor",
                ));
            }
            OutcomeKind::Restated { supersedes, .. }
                if last_value.is_none_or(|e| e.seq != *supersedes) =>
            {
                return Err(frozen(
                    "restatement must supersede the latest numeric event of this forecast",
                ));
            }
            _ => {}
        }
        if let Some((value, citation)) = kind.publication() {
            if !value.is_finite() {
                return Err(domain("outcome must be finite"));
            }
            citation.validate()?;
            if matches!(rec.payload, ForecastPayload::Bernoulli { .. })
                && value != 0.0
                && value != 1.0
            {
                return Err(domain("an event outcome must be exactly zero or one"));
            }
            if prior
                .iter()
                .any(|e| matches!(e.kind, OutcomeKind::Missing { reason } if reason.terminal()))
            {
                return Err(frozen(
                    "withdrawal, definition change or divestment cannot be reinterpreted as the old target",
                ));
            }
            if prior
                .iter()
                .filter_map(|e| e.kind.publication())
                .any(|(_, c)| c.source == citation.source && c.vintage == citation.vintage)
            {
                return Err(frozen("a published source vintage cannot be replaced"));
            }
            for cohort in self
                .cohorts
                .values()
                .filter(|c| c.spec.members.binary_search(&forecast_id).is_ok())
            {
                if cohort
                    .spec
                    .eligible_sources
                    .binary_search(&citation.source)
                    .is_err()
                {
                    return Err(absent(
                        "outcome source is outside the cohort's frozen eligible sources",
                    ));
                }
                if cohort
                    .spec
                    .policy
                    .outcome_bounds
                    .is_some_and(|(lo, hi)| value < lo || value > hi)
                {
                    return Err(domain(
                        "outcome is outside the cohort's predeclared physical domain",
                    ));
                }
            }
        }
        let seq = u64::try_from(prior.len())
            .ok()
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| domain("outcome sequence overflow"))?;
        let previous_digest = prior.last().map_or(Digest32::default(), |e| e.digest);
        let mut enc = Encoder::new_frame(*b"OUT1", 1);
        enc.put_fixed(&previous_digest.0);
        enc.put_fixed(&forecast_id.bytes);
        enc.put_u64(seq);
        enc.put_i64(availability_time_ns);
        enc.put_i64(record_time_ns);
        kind.encode(&mut enc);
        let event = OutcomeEvent {
            forecast_id,
            seq,
            availability_time_ns,
            record_time_ns,
            kind,
            previous_digest,
            digest: blake3_hash(&enc.finish()),
        };
        self.latest_record_ns = Some(record_time_ns);
        self.outcomes.entry(forecast_id).or_default().push(event);
        Ok(seq)
    }

    /// Compatibility one-shot outcome. It appends an explicitly uncited internal event, never mutates a
    /// forecast. A frozen public cohort cannot use this path: `legacy:uncited` is refused at cohort creation.
    pub fn record_outcome(
        &mut self,
        forecast_id: ObjectId,
        outcome_ns: i64,
        realized: f64,
    ) -> Result<(), Refusal> {
        if !self.outcomes(forecast_id).is_empty() {
            return Err(frozen("the outcome is already recorded"));
        }
        let mut enc = Encoder::new_frame(*b"LEG1", 1);
        floats(&mut enc, &[realized]);
        self.append_outcome(
            forecast_id,
            outcome_ns,
            outcome_ns,
            OutcomeKind::Published {
                value: realized,
                citation: OutcomeCitation {
                    source: "legacy:uncited".into(),
                    vintage: outcome_ns.to_string(),
                    locator: "caller-supplied; no public source claim".into(),
                    sha256: fmine_core::hash::sha256(&enc.finish()).0,
                },
            },
        )
        .map(|_| ())
    }

    /// Records an immutable assessment. `as_of_ns` is a bitemporal cut: an event must have been both available
    /// and recorded by then. Repeating the same query returns the original record, including its digest.
    pub fn assess(
        &mut self,
        cohort: Digest32,
        as_of_ns: i64,
        selector: OutcomeSelector,
    ) -> Result<&Assessment, Refusal> {
        let key = (cohort, as_of_ns, selector);
        if !self.assessments.contains_key(&key) {
            let frozen = self
                .cohorts
                .get(&cohort)
                .ok_or_else(|| absent("unknown frozen cohort"))?;
            if as_of_ns < frozen.spec.frozen_at_ns {
                return Err(late("assessment precedes the cohort freeze"));
            }
            let records: Vec<&FrozenForecast> = frozen
                .spec
                .members
                .iter()
                .filter_map(|id| self.records.get(id))
                .collect();
            if records.len() != frozen.spec.members.len() {
                return Err(absent("cohort member unavailable"));
            }
            let per_member = self.select_members(&records, as_of_ns, selector);
            let summary =
                summarize(&records, &per_member, &frozen.spec.policy).map_err(score_refusal)?;
            let mut assessment = Assessment {
                cohort,
                as_of_ns,
                selector,
                per_member,
                summary,
                digest: Digest32::default(),
            };
            assessment.digest = blake3_hash(&assessment.canonical_bytes());
            for id in &frozen.spec.members {
                self.assessed_through
                    .entry(*id)
                    .and_modify(|t| *t = (*t).max(as_of_ns))
                    .or_insert(as_of_ns);
            }
            self.sealed_through_ns = Some(
                self.sealed_through_ns
                    .map_or(as_of_ns, |old| old.max(as_of_ns)),
            );
            self.assessments.insert(key, assessment);
        }
        self.assessments
            .get(&key)
            .ok_or_else(|| absent("assessment unavailable"))
    }

    fn select_members(
        &self,
        records: &[&FrozenForecast],
        as_of: i64,
        selector: OutcomeSelector,
    ) -> Vec<MemberAssessment> {
        records
            .iter()
            .map(|rec| {
                let eligible = || {
                    self.outcomes(rec.forecast_id)
                        .iter()
                        .filter(|e| e.availability_time_ns <= as_of && e.record_time_ns <= as_of)
                };
                let selected = match selector {
                    OutcomeSelector::FirstPublished => eligible()
                        .find(|e| matches!(e.kind, OutcomeKind::Published { .. }))
                        .or_else(|| eligible().next_back()),
                    OutcomeSelector::LatestAsOf => eligible().next_back(),
                };
                let outcome = match selected {
                    Some(event) => match event.kind.publication() {
                        Some((value, citation)) => MemberOutcome::Resolved {
                            value,
                            event_seq: event.seq,
                            event_digest: event.digest,
                            citation: citation.clone(),
                        },
                        None => match event.kind {
                            OutcomeKind::Missing { reason } => MemberOutcome::Unresolved { reason },
                            _ => MemberOutcome::Unresolved {
                                reason: MissingReason::NotPublished,
                            },
                        },
                    },
                    None => MemberOutcome::Unresolved {
                        reason: if as_of > rec.freeze_time_ns {
                            MissingReason::Overdue
                        } else {
                            MissingReason::Pending
                        },
                    },
                };
                MemberAssessment {
                    forecast_id: rec.forecast_id,
                    selected_event: selected.map(|e| (e.seq, e.digest)),
                    outcome,
                }
            })
            .collect()
    }

    /// Compatibility report delegates to the same assessment engine and exposes the entire current model
    /// denominator. This is a descriptive snapshot, not a frozen cohort or an external timestamp proof.
    pub fn evaluate_model(&self, model_key: &str) -> Result<CalibrationReport, ScoreError> {
        let records: Vec<&FrozenForecast> = self
            .records
            .values()
            .filter(|r| r.model_key == model_key)
            .collect();
        if records
            .iter()
            .any(|r| !matches!(r.payload, ForecastPayload::Gaussian { .. }))
        {
            return Err(ScoreError::InvalidParameter {
                name: "evaluate_model_requires_gaussian_cohort",
                value: 0.0,
            });
        }
        let members = self.select_members(&records, i64::MAX, OutcomeSelector::FirstPublished);
        let s = summarize(&records, &members, &AssessmentPolicy::default())?;
        Ok(CalibrationReport {
            n_members: s.n_members,
            n_scored: s.n_resolved,
            n_unresolved: s.n_unresolved,
            whole_cohort_coverage_80: s.coverage.map(|c| c.whole_cohort),
            mean_crps: s.crps.and_then(|x| x.observed_mean).unwrap_or(0.0),
            mean_winkler_80: s
                .interval_score
                .and_then(|x| x.observed_mean)
                .unwrap_or(0.0),
            empirical_coverage_80: s.coverage.and_then(|c| c.observed_fraction).unwrap_or(0.0),
            mean_standardized_bias: s.mean_standardized_bias.unwrap_or(0.0),
            flag: if s.n_resolved == 0 {
                CalibrationFlag::Unscored
            } else {
                CalibrationFlag::Descriptive
            },
        })
    }
}

#[derive(Default)]
struct ScoreAccumulator {
    observed: f64,
    lower: f64,
    upper: Option<f64>,
}

impl ScoreAccumulator {
    fn new() -> Self {
        Self {
            upper: Some(0.0),
            ..Self::default()
        }
    }
    fn add(&mut self, observed: Option<f64>, bounds: (f64, Option<f64>)) -> Result<(), ScoreError> {
        if let Some(value) = observed {
            self.observed = finite(self.observed + finite(value)?)?;
        }
        self.lower = finite(self.lower + finite(bounds.0)?)?;
        self.upper = match (self.upper, bounds.1) {
            (Some(a), Some(b)) => Some(finite(a + finite(b)?)?),
            _ => None,
        };
        Ok(())
    }
    fn finish(&self, n: usize, scored: usize) -> ScoreSummary {
        ScoreSummary {
            n_scored: scored,
            observed_mean: (scored > 0).then(|| self.observed / scored as f64),
            whole_cohort: ScoreBounds {
                lower: self.lower / n as f64,
                upper: self.upper.map(|u| u / n as f64),
            },
        }
    }
}

fn summarize(
    records: &[&FrozenForecast],
    members: &[MemberAssessment],
    policy: &AssessmentPolicy,
) -> Result<AssessmentSummary, ScoreError> {
    let n = records.len();
    let mut missing = BTreeMap::new();
    let mut resolved = 0;
    let mut inside = 0;
    let mut z_sum = 0.0;
    let mut crps = ScoreAccumulator::new();
    let mut interval = ScoreAccumulator::new();
    let mut brier = ScoreAccumulator::new();
    let all_gaussian = n > 0
        && records
            .iter()
            .all(|r| matches!(r.payload, ForecastPayload::Gaussian { .. }));
    let events = n > 0
        && records
            .iter()
            .all(|r| matches!(r.payload, ForecastPayload::Bernoulli { .. }));
    let has_nominal = records
        .iter()
        .all(|r| !matches!(r.payload, ForecastPayload::Interval { nominal: None, .. }));
    for (rec, member) in records.iter().zip(members) {
        let y = match &member.outcome {
            MemberOutcome::Resolved { value, .. } => {
                resolved += 1;
                Some(*value)
            }
            MemberOutcome::Unresolved { reason } => {
                *missing.entry(*reason).or_default() += 1;
                None
            }
        };
        if let Some((lo, hi)) = rec.payload.interval(policy.alpha)? {
            let score = y
                .map(|v| winkler_interval_score(lo, hi, policy.alpha, v))
                .transpose()?;
            if y.is_some_and(|v| lo <= v && v <= hi) {
                inside += 1;
            }
            let bounds = if let Some(s) = score {
                (s, Some(s))
            } else if let Some((yl, yu)) = policy.outcome_bounds {
                let left = winkler_interval_score(lo, hi, policy.alpha, yl)?;
                let right = winkler_interval_score(lo, hi, policy.alpha, yu)?;
                let min = if yu < lo {
                    right
                } else if yl > hi {
                    left
                } else {
                    hi - lo
                };
                (min, Some(left.max(right)))
            } else {
                (hi - lo, None)
            };
            interval.add(score, bounds)?;
        }
        if let ForecastPayload::Gaussian { mean, std_dev } = rec.payload {
            let score = y.map(|v| gaussian_crps(mean, std_dev, v)).transpose()?;
            if let Some(y) = y {
                z_sum = finite(z_sum + finite((y - mean) / std_dev)?)?;
            }
            crps.add(score, score.map_or((0.0, None), |s| (s, Some(s))))?;
        }
        if let ForecastPayload::Bernoulli { probability: p } = rec.payload {
            let loss0 = p * p;
            let loss1 = (1.0 - p) * (1.0 - p);
            let score = y.map(|v| if v == 0.0 { loss0 } else { loss1 });
            brier.add(
                score,
                score.map_or((loss0.min(loss1), Some(loss0.max(loss1))), |s| (s, Some(s))),
            )?;
        }
    }
    let n_unresolved = n - resolved;
    let coverage = (n > 0 && !events).then(|| CoverageSummary {
        inside,
        observed_fraction: (resolved > 0).then(|| inside as f64 / resolved as f64),
        whole_cohort: (
            inside as f64 / n as f64,
            (inside + n_unresolved) as f64 / n as f64,
        ),
    });
    let effective = |rho: f64| {
        if resolved == 0 {
            0.0
        } else {
            resolved as f64 / (1.0 + (resolved - 1) as f64 * rho)
        }
    };
    let effective_n = EffectiveSampleSize {
        lower: effective(policy.correlation_range.1),
        upper: effective(policy.correlation_range.0),
        correlation_range: policy.correlation_range,
    };
    let bias = (all_gaussian && resolved > 0).then(|| z_sum / resolved as f64);
    let flag = if resolved == 0 {
        CalibrationFlag::Unscored
    } else if events
        || !has_nominal
        || n_unresolved > 0
        || effective_n.lower < policy.min_calibration_n as f64
    {
        CalibrationFlag::Descriptive
    } else if bias.is_some_and(|b| b.abs() > 0.5) {
        CalibrationFlag::Biased
    } else {
        let cov = inside as f64 / resolved as f64;
        if cov < 1.0 - policy.alpha - 0.15 {
            CalibrationFlag::Overconfident
        } else if cov > 1.0 - policy.alpha + 0.15 {
            CalibrationFlag::Descriptive
        } else {
            CalibrationFlag::Calibrated
        }
    };
    Ok(AssessmentSummary {
        n_members: n,
        n_resolved: resolved,
        n_unresolved,
        unresolved_by_reason: missing,
        coverage,
        crps: all_gaussian.then(|| crps.finish(n, resolved)),
        interval_score: (n > 0 && !events).then(|| interval.finish(n, resolved)),
        brier: events.then(|| brier.finish(n, resolved)),
        effective_n,
        mean_standardized_bias: bias,
        flag,
    })
}

impl Assessment {
    /// Canonical report body, suitable for retaining beside its digest. It commits to the frozen inputs, policy,
    /// selected event hashes, all unresolved reasons, every score and the complete denominator.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"ASM1", 1);
        enc.put_fixed(&self.cohort.0);
        enc.put_i64(self.as_of_ns);
        enc.put_u8(match self.selector {
            OutcomeSelector::FirstPublished => 0,
            OutcomeSelector::LatestAsOf => 1,
        });
        enc.put_u64(self.per_member.len() as u64);
        for member in &self.per_member {
            enc.put_fixed(&member.forecast_id.bytes);
            enc.put_u8(u8::from(member.selected_event.is_some()));
            if let Some((seq, digest)) = member.selected_event {
                enc.put_u64(seq);
                enc.put_fixed(&digest.0);
            }
            match &member.outcome {
                MemberOutcome::Resolved {
                    value,
                    event_seq,
                    event_digest,
                    citation,
                } => {
                    enc.put_u8(0);
                    floats(&mut enc, &[*value]);
                    enc.put_u64(*event_seq);
                    enc.put_fixed(&event_digest.0);
                    citation.encode(&mut enc);
                }
                MemberOutcome::Unresolved { reason } => {
                    enc.put_u8(1);
                    enc.put_u8(reason.tag());
                }
            }
        }
        let s = &self.summary;
        for n in [s.n_members, s.n_resolved, s.n_unresolved] {
            enc.put_u64(n as u64);
        }
        enc.put_u64(s.unresolved_by_reason.len() as u64);
        for (reason, n) in &s.unresolved_by_reason {
            enc.put_u8(reason.tag());
            enc.put_u64(*n as u64);
        }
        enc.put_u8(u8::from(s.coverage.is_some()));
        if let Some(c) = s.coverage {
            enc.put_u64(c.inside as u64);
            optional_float(&mut enc, c.observed_fraction);
            floats(&mut enc, &[c.whole_cohort.0, c.whole_cohort.1]);
        }
        for score in [s.crps, s.interval_score, s.brier] {
            enc.put_u8(u8::from(score.is_some()));
            if let Some(score) = score {
                enc.put_u64(score.n_scored as u64);
                optional_float(&mut enc, score.observed_mean);
                floats(&mut enc, &[score.whole_cohort.lower]);
                optional_float(&mut enc, score.whole_cohort.upper);
            }
        }
        floats(
            &mut enc,
            &[
                s.effective_n.lower,
                s.effective_n.upper,
                s.effective_n.correlation_range.0,
                s.effective_n.correlation_range.1,
            ],
        );
        optional_float(&mut enc, s.mean_standardized_bias);
        enc.put_u8(match s.flag {
            CalibrationFlag::Unscored => 0,
            CalibrationFlag::Descriptive => 1,
            CalibrationFlag::Calibrated => 2,
            CalibrationFlag::Overconfident => 3,
            CalibrationFlag::Biased => 4,
        });
        enc.finish()
    }
}

fn floats(enc: &mut Encoder, values: &[f64]) {
    for x in values {
        enc.put_u64(x.to_bits());
    }
}
fn optional_float(enc: &mut Encoder, x: Option<f64>) {
    enc.put_u8(u8::from(x.is_some()));
    if let Some(x) = x {
        floats(enc, &[x]);
    }
}
fn finite(x: f64) -> Result<f64, ScoreError> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(ScoreError::NonFinite)
    }
}
fn domain(what: &str) -> Refusal {
    Refusal::new(RefusalCode::OutsidePhysicalDomain109, what)
}
fn late(what: &str) -> Refusal {
    Refusal::new(RefusalCode::LateRegistration206, what)
}
fn frozen(what: &str) -> Refusal {
    Refusal::new(RefusalCode::FrozenRecordMutation507, what)
}
fn absent(what: &str) -> Refusal {
    Refusal::new(RefusalCode::NoClaimInDomain201, what)
}
fn score_refusal(error: ScoreError) -> Refusal {
    domain(&format!("cohort score refused: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::parse_hex32;

    fn citation(vintage: &str) -> OutcomeCitation {
        OutcomeCitation {
            source: "synthetic".into(),
            vintage: vintage.into(),
            locator: "synthetic:outcome".into(),
            sha256: fmine_core::hash::sha256(vintage.as_bytes()).0,
        }
    }

    fn published(value: f64) -> OutcomeKind {
        OutcomeKind::Published {
            value,
            citation: citation("v1"),
        }
    }

    fn make_cohort(
        payloads: Vec<ForecastPayload>,
        policy: AssessmentPolicy,
    ) -> (ForecastLedger, Digest32, Vec<ObjectId>) {
        let mut ledger = ForecastLedger::new();
        let ids = payloads
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                ledger
                    .register_payload(format!("model:{i}"), 1, 100, p)
                    .expect("forecast")
            })
            .collect::<Vec<_>>();
        let cohort = ledger
            .freeze_cohort(CohortSpec {
                name: "synthetic cohort".into(),
                frozen_at_ns: 2,
                members: ids.clone(),
                resolution_rule: "synthetic outcomes, same quantity and support".into(),
                eligible_sources: vec!["synthetic".into()],
                policy,
            })
            .expect("freeze");
        (ledger, cohort, ids)
    }

    fn interval() -> ForecastPayload {
        ForecastPayload::Interval {
            lo: 0.0,
            hi: 1.0,
            nominal: Some(0.8),
        }
    }

    #[test]
    fn whole_cohort_denominator_keeps_missing_outcomes() {
        let (mut ledger, cohort, ids) =
            make_cohort(vec![interval(); 10], AssessmentPolicy::default());
        for (i, id) in ids[..8].iter().enumerate() {
            ledger
                .append_outcome(*id, 101, 102, published(if i < 7 { 0.5 } else { 2.0 }))
                .expect("outcome");
        }
        ledger
            .append_outcome(
                ids[8],
                101,
                102,
                OutcomeKind::Missing {
                    reason: MissingReason::Divested,
                },
            )
            .expect("missing");
        let a = ledger
            .assess(cohort, 103, OutcomeSelector::FirstPublished)
            .expect("assessment");
        assert_eq!(
            (
                a.summary.n_members,
                a.summary.n_resolved,
                a.summary.n_unresolved
            ),
            (10, 8, 2)
        );
        let c = a.summary.coverage.expect("coverage");
        assert_eq!(c.observed_fraction, Some(7.0 / 8.0));
        assert_eq!(c.whole_cohort, (0.7, 0.9));
        assert_eq!(a.summary.unresolved_by_reason[&MissingReason::Divested], 1);
        assert_eq!(a.summary.unresolved_by_reason[&MissingReason::Overdue], 1);
        assert_eq!(a.summary.flag, CalibrationFlag::Descriptive);
        assert_eq!(
            a.summary.interval_score.expect("score").whole_cohort.upper,
            None
        );
    }

    #[test]
    fn e474_first_publication_and_restatement_are_distinct_assessments() {
        // FACT: independently captured cells in planning/research/W1_barrick_mine_stats.md, W1-DR-08 / E474.
        // SHA-256s are the registered source bytes, not output from this scorer. The forecast is synthetic.
        let original = OutcomeCitation {
            source: "Barrick_Q1_2024_Mine_Stats.xlsx".into(),
            vintage: "2024Q1".into(),
            locator: "Q1_2024:QTD!G23".into(),
            sha256: parse_hex32("e2e1243ec0549f4c487c773869876e2acade401a0b6cd1141adf4b9296aec840")
                .expect("digest"),
        };
        let restatement = OutcomeCitation {
            source: "Barrick_Q1_2025_Mine_Stats.xlsx".into(),
            vintage: "2025Q1".into(),
            locator: "Q1_2025:QTD!N22".into(),
            sha256: parse_hex32("ec5158f8b4624643ee83d0099e4f3354dc550729ebb443a2dbfacfc29315998b")
                .expect("digest"),
        };
        let mut ledger = ForecastLedger::new();
        let fid = ledger
            .register_payload(
                "synthetic AISC interval",
                1,
                100,
                ForecastPayload::Interval {
                    lo: 1476.0,
                    hi: 1490.0,
                    nominal: Some(0.8),
                },
            )
            .expect("forecast");
        let cohort = ledger
            .freeze_cohort(CohortSpec {
                name: "E474 synthetic forecast".into(),
                frozen_at_ns: 2,
                members: vec![fid],
                resolution_rule: "Total Gold 2024Q1 AISC, US$/oz sold".into(),
                eligible_sources: vec![original.source.clone(), restatement.source.clone()],
                policy: AssessmentPolicy::default(),
            })
            .expect("cohort");
        let first_seq = ledger
            .append_outcome(
                fid,
                101,
                102,
                OutcomeKind::Published {
                    value: 1479.0,
                    citation: original,
                },
            )
            .expect("first");
        let before = ledger
            .assess(cohort, 150, OutcomeSelector::LatestAsOf)
            .expect("before")
            .clone();
        assert_eq!(before.summary.coverage.expect("coverage").inside, 1);
        ledger
            .append_outcome(
                fid,
                201,
                202,
                OutcomeKind::Restated {
                    value: 1474.0,
                    citation: restatement,
                    supersedes: first_seq,
                },
            )
            .expect("restated");
        let first = ledger
            .assess(cohort, 250, OutcomeSelector::FirstPublished)
            .expect("original selector")
            .clone();
        let latest = ledger
            .assess(cohort, 250, OutcomeSelector::LatestAsOf)
            .expect("latest selector")
            .clone();
        assert_eq!(first.summary.coverage.expect("coverage").inside, 1);
        assert_eq!(latest.summary.coverage.expect("coverage").inside, 0);
        assert_eq!(
            first.summary.interval_score.expect("score").observed_mean,
            Some(14.0)
        );
        assert_eq!(
            latest.summary.interval_score.expect("score").observed_mean,
            Some(34.0)
        );
        assert_ne!(first.digest, latest.digest);
        let replay = ledger
            .assess(cohort, 150, OutcomeSelector::LatestAsOf)
            .expect("as-of replay");
        assert_eq!(replay.canonical_bytes(), before.canonical_bytes());
        assert_eq!(replay.digest, before.digest);
        assert_eq!(
            ledger.outcomes(fid)[1].previous_digest,
            ledger.outcomes(fid)[0].digest
        );
    }

    #[test]
    fn as_of_requires_recorded_and_available_and_rejects_backdating() {
        let (mut ledger, cohort, ids) = make_cohort(vec![interval()], AssessmentPolicy::default());
        ledger
            .append_outcome(ids[0], 101, 200, published(0.5))
            .expect("delayed recording");
        let before = ledger
            .assess(cohort, 150, OutcomeSelector::FirstPublished)
            .expect("as-of");
        assert_eq!(before.summary.n_resolved, 0);
        assert_eq!(
            before.summary.unresolved_by_reason[&MissingReason::Overdue],
            1
        );
        let after = ledger
            .assess(cohort, 200, OutcomeSelector::FirstPublished)
            .expect("recorded");
        assert_eq!(after.summary.n_resolved, 1);
        let bad = ledger
            .append_outcome(
                ids[0],
                151,
                199,
                OutcomeKind::Restated {
                    value: 2.0,
                    citation: citation("v2"),
                    supersedes: 1,
                },
            )
            .expect_err("record time regresses");
        assert_eq!(bad.code, "FMINE-REF-206");
        let (mut other, cohort, ids) = make_cohort(vec![interval()], AssessmentPolicy::default());
        other
            .assess(cohort, 200, OutcomeSelector::FirstPublished)
            .expect("missing report");
        assert_eq!(
            other
                .append_outcome(ids[0], 151, 199, published(0.5))
                .expect_err("sealed cut")
                .code,
            "FMINE-REF-206"
        );
        other
            .append_outcome(ids[0], 151, 201, published(0.5))
            .expect("late recording preserves old report");
        assert_eq!(
            other
                .assess(cohort, 200, OutcomeSelector::FirstPublished)
                .expect("old report")
                .summary
                .n_resolved,
            0
        );
        assert_eq!(
            other
                .assess(cohort, 201, OutcomeSelector::FirstPublished)
                .expect("new report")
                .summary
                .n_resolved,
            1
        );
    }

    #[test]
    fn frozen_membership_sources_and_supersession_fail_closed() {
        let (mut ledger, cohort, ids) = make_cohort(vec![interval()], AssessmentPolicy::default());
        let frozen_spec = ledger.cohort(cohort).expect("cohort").spec.clone();
        assert_eq!(
            ledger
                .freeze_cohort(frozen_spec.clone())
                .expect_err("no mutation")
                .code,
            "FMINE-REF-507"
        );
        let mut changed = frozen_spec.clone();
        changed.name = "late cohort".into();
        changed.frozen_at_ns = 100;
        assert_eq!(
            ledger.freeze_cohort(changed).expect_err("late freeze").code,
            "FMINE-REF-206"
        );
        let mut bad_source = citation("v1");
        bad_source.source = "unregistered".into();
        assert_eq!(
            ledger
                .append_outcome(
                    ids[0],
                    101,
                    102,
                    OutcomeKind::Published {
                        value: 0.5,
                        citation: bad_source
                    }
                )
                .expect_err("source")
                .code,
            "FMINE-REF-201"
        );
        assert!(ledger.outcomes(ids[0]).is_empty());
        assert_eq!(
            ledger
                .record_outcome(ids[0], 102, 0.5)
                .expect_err("uncited legacy outcome")
                .code,
            "FMINE-REF-201"
        );
        assert_eq!(
            ledger
                .append_outcome(ids[0], 100, 102, published(0.5))
                .expect_err("early outcome")
                .code,
            "FMINE-REF-206"
        );
        assert_eq!(
            ledger
                .append_outcome(ids[0], 102, 101, published(0.5))
                .expect_err("recorded before availability")
                .code,
            "FMINE-REF-206"
        );
        ledger
            .append_outcome(ids[0], 101, 102, published(0.5))
            .expect("first");
        assert_eq!(
            ledger
                .append_outcome(ids[0], 103, 104, published(2.0))
                .expect_err("silent replacement")
                .code,
            "FMINE-REF-507"
        );
        assert_eq!(
            ledger
                .append_outcome(
                    ids[0],
                    103,
                    104,
                    OutcomeKind::Restated {
                        value: 2.0,
                        citation: citation("v2"),
                        supersedes: 99
                    }
                )
                .expect_err("wrong predecessor")
                .code,
            "FMINE-REF-507"
        );
        let mut new = frozen_spec;
        new.name = "selected after outcome".into();
        assert_eq!(
            ledger
                .freeze_cohort(new)
                .expect_err("selection after outcome")
                .code,
            "FMINE-REF-206"
        );
        ledger
            .append_outcome(
                ids[0],
                103,
                104,
                OutcomeKind::Missing {
                    reason: MissingReason::DefinitionChanged,
                },
            )
            .expect("changed target");
        assert_eq!(
            ledger
                .append_outcome(
                    ids[0],
                    105,
                    106,
                    OutcomeKind::Restated {
                        value: 2.0,
                        citation: citation("v2"),
                        supersedes: 1
                    }
                )
                .expect_err("no implicit target fallback")
                .code,
            "FMINE-REF-507"
        );
    }

    #[test]
    fn coverage_and_brier_bounds_match_exhaustive_binary_completions_to_twelve_members() {
        for n in 1..=12 {
            let observed = n / 2;
            for prefix in 0..(1usize << observed) {
                let probs: Vec<f64> = (0..n).map(|i| (i + 1) as f64 / (n + 1) as f64).collect();
                let (mut events, ec, eids) = make_cohort(
                    probs
                        .iter()
                        .map(|p| ForecastPayload::Bernoulli { probability: *p })
                        .collect(),
                    AssessmentPolicy::default(),
                );
                let (mut ranges, rc, rids) =
                    make_cohort(vec![interval(); n], AssessmentPolicy::default());
                for i in 0..observed {
                    let y = ((prefix >> i) & 1) as f64;
                    events
                        .append_outcome(eids[i], 101, 102, published(y))
                        .expect("event");
                    ranges
                        .append_outcome(
                            rids[i],
                            101,
                            102,
                            published(if y == 1.0 { 0.5 } else { 2.0 }),
                        )
                        .expect("range");
                }
                let es = events
                    .assess(ec, 103, OutcomeSelector::FirstPublished)
                    .expect("score");
                let rs = ranges
                    .assess(rc, 103, OutcomeSelector::FirstPublished)
                    .expect("coverage");
                let mut min_score = f64::INFINITY;
                let mut max_score = f64::NEG_INFINITY;
                let mut min_hits = n;
                let mut max_hits = 0;
                for suffix in 0..(1usize << (n - observed)) {
                    let pattern = prefix | (suffix << observed);
                    let hits = pattern.count_ones() as usize;
                    let score = probs
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let error = *p - ((pattern >> i) & 1) as f64;
                            error * error
                        })
                        .sum::<f64>()
                        / n as f64;
                    min_hits = min_hits.min(hits);
                    max_hits = max_hits.max(hits);
                    min_score = min_score.min(score);
                    max_score = max_score.max(score);
                }
                assert_eq!(rs.summary.n_members, n);
                assert_eq!(
                    rs.summary.coverage.expect("coverage").whole_cohort,
                    (min_hits as f64 / n as f64, max_hits as f64 / n as f64)
                );
                let b = es.summary.brier.expect("Brier").whole_cohort;
                assert!(
                    (b.lower - min_score).abs() < 1e-12,
                    "n={n}, prefix={prefix}"
                );
                assert!(
                    (b.upper.expect("bounded") - max_score).abs() < 1e-12,
                    "n={n}, prefix={prefix}"
                );
            }
        }
    }

    #[test]
    fn interval_bounds_use_registered_domain_and_crps_never_invents_an_upper_bound() {
        let policy = AssessmentPolicy {
            outcome_bounds: Some((0.0, 3.0)),
            ..AssessmentPolicy::default()
        };
        let p = ForecastPayload::Interval {
            lo: 1.0,
            hi: 2.0,
            nominal: Some(0.8),
        };
        let (mut ledger, cohort, ids) = make_cohort(vec![p; 2], policy);
        assert_eq!(
            ledger
                .append_outcome(ids[0], 101, 102, published(4.0))
                .expect_err("physical domain")
                .code,
            "FMINE-REF-109"
        );
        ledger
            .append_outcome(ids[0], 101, 102, published(2.5))
            .expect("score 6");
        let s = ledger
            .assess(cohort, 103, OutcomeSelector::FirstPublished)
            .expect("assessment")
            .summary
            .interval_score
            .expect("score");
        assert_eq!(s.observed_mean, Some(6.0));
        assert_eq!(
            s.whole_cohort,
            ScoreBounds {
                lower: 3.5,
                upper: Some(8.5)
            }
        );
        let (mut normal, cohort, ids) = make_cohort(
            vec![
                ForecastPayload::Gaussian {
                    mean: 0.0,
                    std_dev: 1.0
                };
                2
            ],
            AssessmentPolicy::default(),
        );
        normal
            .append_outcome(ids[0], 101, 102, published(0.0))
            .expect("outcome");
        let s = &normal
            .assess(cohort, 103, OutcomeSelector::FirstPublished)
            .expect("assessment")
            .summary;
        assert_eq!(s.crps.expect("CRPS").whole_cohort.upper, None);
        assert_eq!(s.interval_score.expect("interval").whole_cohort.upper, None);
        assert_eq!(
            s.crps.expect("CRPS").whole_cohort.lower,
            s.crps.expect("CRPS").observed_mean.expect("observed") / 2.0
        );
    }

    fn calibrated_case(
        n: usize,
        rho_high: f64,
        residual: impl Fn(usize) -> f64,
    ) -> CalibrationFlag {
        let policy = AssessmentPolicy {
            correlation_range: (0.0, rho_high),
            ..AssessmentPolicy::default()
        };
        let (mut ledger, cohort, ids) = make_cohort(
            vec![
                ForecastPayload::Gaussian {
                    mean: 0.0,
                    std_dev: 1.0
                };
                n
            ],
            policy,
        );
        for (i, id) in ids.iter().enumerate() {
            ledger
                .append_outcome(*id, 101, 102, published(residual(i)))
                .expect("outcome");
        }
        ledger
            .assess(cohort, 103, OutcomeSelector::FirstPublished)
            .expect("score")
            .summary
            .flag
    }

    #[test]
    fn calibration_needs_frozen_minimum_effective_sample_and_all_outcomes() {
        let nominal = |i: usize| {
            if i.is_multiple_of(5) {
                if i.is_multiple_of(2) { 2.0 } else { -2.0 }
            } else {
                0.0
            }
        };
        assert_eq!(
            calibrated_case(34, 0.0, nominal),
            CalibrationFlag::Descriptive
        );
        assert_eq!(
            calibrated_case(35, 0.0, nominal),
            CalibrationFlag::Calibrated
        );
        assert_eq!(
            calibrated_case(35, 0.5, nominal),
            CalibrationFlag::Descriptive
        );
        assert_eq!(calibrated_case(35, 0.0, |_| 2.0), CalibrationFlag::Biased);
        assert_eq!(
            calibrated_case(35, 0.0, |i| if i.is_multiple_of(2) { 2.0 } else { -2.0 }),
            CalibrationFlag::Overconfident
        );
        let (mut ledger, cohort, ids) = make_cohort(
            vec![interval(); 40],
            AssessmentPolicy {
                correlation_range: (0.0, 0.0),
                ..AssessmentPolicy::default()
            },
        );
        for (i, id) in ids[..35].iter().enumerate() {
            ledger
                .append_outcome(
                    *id,
                    101,
                    102,
                    published(if i.is_multiple_of(5) { 2.0 } else { 0.5 }),
                )
                .expect("outcome");
        }
        assert_eq!(
            ledger
                .assess(cohort, 103, OutcomeSelector::FirstPublished)
                .expect("missing five")
                .summary
                .flag,
            CalibrationFlag::Descriptive
        );
    }

    #[test]
    fn guidance_has_no_fabricated_level_and_quantiles_are_scored_at_declared_levels() {
        let guidance = ForecastPayload::Interval {
            lo: 0.0,
            hi: 1.0,
            nominal: None,
        };
        let (mut ledger, cohort, ids) = make_cohort(
            vec![guidance; 40],
            AssessmentPolicy {
                correlation_range: (0.0, 0.0),
                ..AssessmentPolicy::default()
            },
        );
        for (i, id) in ids.iter().enumerate() {
            ledger
                .append_outcome(
                    *id,
                    101,
                    102,
                    published(if i.is_multiple_of(5) { 2.0 } else { 0.5 }),
                )
                .expect("guidance outcome");
        }
        let a = ledger
            .assess(cohort, 103, OutcomeSelector::FirstPublished)
            .expect("guidance report");
        assert_eq!(
            a.summary.coverage.expect("hit rate").observed_fraction,
            Some(0.8)
        );
        assert_eq!(a.summary.flag, CalibrationFlag::Descriptive);
        let quantiles = ForecastPayload::Quantiles {
            levels: vec![0.1, 0.5, 0.9],
            values: vec![10.0, 20.0, 30.0],
        };
        let (mut ledger, cohort, ids) = make_cohort(vec![quantiles], AssessmentPolicy::default());
        ledger
            .append_outcome(ids[0], 101, 102, published(35.0))
            .expect("quantile outcome");
        assert_eq!(
            ledger
                .assess(cohort, 103, OutcomeSelector::FirstPublished)
                .expect("quantile score")
                .summary
                .interval_score
                .expect("score")
                .observed_mean,
            Some(70.0)
        );
    }

    #[test]
    fn exact_payload_and_policy_are_bound_even_for_legacy_ids() {
        let freeze = |mean: f64, policy: AssessmentPolicy| {
            let mut ledger = ForecastLedger::new();
            let id = ledger
                .register("legacy", 1, 100, mean, 1.0)
                .expect("forecast");
            let digest = ledger
                .freeze_cohort(CohortSpec {
                    name: "same".into(),
                    frozen_at_ns: 2,
                    members: vec![id],
                    resolution_rule: "same".into(),
                    eligible_sources: vec!["synthetic".into()],
                    policy,
                })
                .expect("freeze");
            (id, digest)
        };
        let a = freeze(1.0, AssessmentPolicy::default());
        let b = freeze(1.0 + 1e-10, AssessmentPolicy::default());
        assert_eq!(a.0, b.0, "legacy ID compatibility");
        assert_ne!(a.1, b.1, "full unrounded payload frozen");
        let c = freeze(
            1.0,
            AssessmentPolicy {
                min_calibration_n: 36,
                ..AssessmentPolicy::default()
            },
        );
        assert_ne!(a.1, c.1, "calibration policy frozen");
        let mut ledger = ForecastLedger::new();
        let a = ledger
            .register_payload(
                "new",
                1,
                100,
                ForecastPayload::Gaussian {
                    mean: 1.0,
                    std_dev: 1.0,
                },
            )
            .expect("new id");
        let b = ledger
            .register_payload(
                "new",
                1,
                100,
                ForecastPayload::Gaussian {
                    mean: 1.0 + 1e-10,
                    std_dev: 1.0,
                },
            )
            .expect("distinct new id");
        assert_ne!(a, b);
    }

    #[test]
    fn legacy_model_report_cannot_hide_unresolved_members() {
        let mut ledger = ForecastLedger::new();
        let a = ledger.register("m", 1, 100, 0.0, 1.0).expect("forecast");
        ledger
            .register("m", 2, 100, 1000.0, 1.0)
            .expect("unresolved forecast");
        ledger.record_outcome(a, 101, 0.0).expect("outcome");
        let r = ledger.evaluate_model("m").expect("report");
        assert_eq!((r.n_members, r.n_scored, r.n_unresolved), (2, 1, 1));
        assert_eq!(r.whole_cohort_coverage_80, Some((0.5, 1.0)));
        assert_eq!(r.flag, CalibrationFlag::Descriptive);
    }

    #[test]
    fn canonical_cohort_event_and_assessment_golden() {
        // DESIGN: schema golden independently assembled from FOR2/COH1/OUT1/ASM1 frames using the codec and
        // hash primitives, without calling ForecastLedger. The hand-known Brier loss is (1 - 0.25)^2 = 0.5625.
        let mut ledger = ForecastLedger::new();
        let id = ledger
            .register_payload(
                "golden",
                1,
                100,
                ForecastPayload::Bernoulli { probability: 0.25 },
            )
            .expect("forecast");
        assert_eq!(id.to_string(), "for_e2f937292ccd78013f38f746550e29a5");
        let cohort = ledger
            .freeze_cohort(CohortSpec {
                name: "golden".into(),
                frozen_at_ns: 2,
                members: vec![id],
                resolution_rule: "binary event".into(),
                eligible_sources: vec!["synthetic".into()],
                policy: AssessmentPolicy {
                    correlation_range: (0.0, 0.0),
                    ..AssessmentPolicy::default()
                },
            })
            .expect("cohort");
        assert_eq!(
            cohort.to_hex(),
            "9aabc96a1dd23f0ee2e95c0f02924b21cab9eeb5e9fa647d01cad06ce98107d2"
        );
        ledger
            .append_outcome(
                id,
                101,
                102,
                OutcomeKind::Published {
                    value: 1.0,
                    citation: OutcomeCitation {
                        source: "synthetic".into(),
                        vintage: "v1".into(),
                        locator: "synthetic:event".into(),
                        sha256: [7; 32],
                    },
                },
            )
            .expect("outcome");
        assert_eq!(
            ledger.outcomes(id)[0].digest.to_hex(),
            "08098a5c7f6f1b1be8342cc82168b45671489f123a55a16d0b3e5698e4c252df"
        );
        let a = ledger
            .assess(cohort, 103, OutcomeSelector::FirstPublished)
            .expect("assessment");
        assert_eq!(a.summary.brier.expect("Brier").observed_mean, Some(0.5625));
        assert_eq!(
            a.digest.to_hex(),
            "7d8c61e468bab0d43ed0cbe695da8dc63a0c11a94a1fed2d5628d9229ede0af9"
        );
    }

    #[test]
    fn a_cohort_cannot_omit_an_observed_miss_and_backdate_its_freeze() {
        let mut ledger = ForecastLedger::new();
        let a = ledger.register_payload("a", 1, 100, interval()).expect("a");
        let b = ledger.register_payload("b", 1, 100, interval()).expect("b");
        ledger
            .append_outcome(a, 101, 102, published(100.0))
            .expect("observed miss");
        let selected = CohortSpec {
            name: "selected after the miss".into(),
            frozen_at_ns: 2,
            members: vec![b],
            resolution_rule: "synthetic".into(),
            eligible_sources: vec!["synthetic".into()],
            policy: AssessmentPolicy::default(),
        };
        assert_eq!(
            ledger
                .freeze_cohort(selected)
                .expect_err("omitted member does not erase history")
                .code,
            "FMINE-REF-206"
        );
        assert_eq!(
            ledger
                .register_payload("retrofitted", 1, 100, interval())
                .expect_err("backdated payload")
                .code,
            "FMINE-REF-206"
        );
        assert_eq!(
            ledger
                .register("retrofitted legacy", 1, 100, 100.0, 1.0)
                .expect_err("backdated legacy")
                .code,
            "FMINE-REF-206"
        );
        assert!(ledger.cohorts.is_empty());
        assert_eq!(ledger.records.len(), 2);

        // The next genuinely future forecast is still admitted; no repair of history is needed.
        let c = ledger
            .register_payload("next period", 103, 200, interval())
            .expect("future forecast");
        ledger
            .freeze_cohort(CohortSpec {
                name: "next period".into(),
                frozen_at_ns: 104,
                members: vec![c],
                resolution_rule: "synthetic".into(),
                eligible_sources: vec!["synthetic".into()],
                policy: AssessmentPolicy::default(),
            })
            .expect("future cohort");
    }

    #[test]
    fn an_assessed_cut_seals_the_whole_ledger_and_historical_queries_do_not_reopen_it() {
        let mut ledger = ForecastLedger::new();
        let a = ledger.register_payload("a", 1, 100, interval()).expect("a");
        let b = ledger.register_payload("b", 1, 100, interval()).expect("b");
        let cohort = ledger
            .freeze_cohort(CohortSpec {
                name: "first cohort".into(),
                frozen_at_ns: 2,
                members: vec![a],
                resolution_rule: "synthetic".into(),
                eligible_sources: vec!["synthetic".into()],
                policy: AssessmentPolicy::default(),
            })
            .expect("cohort");
        let original = ledger
            .assess(cohort, 200, OutcomeSelector::FirstPublished)
            .expect("seal")
            .clone();
        ledger
            .assess(cohort, 150, OutcomeSelector::LatestAsOf)
            .expect("historical report");

        // B was not in the assessed cohort: its timestamps must still respect the authority's closed cut.
        assert_eq!(
            ledger
                .append_outcome(b, 151, 152, published(0.5))
                .expect_err("unseen member backdate")
                .code,
            "FMINE-REF-206"
        );
        assert_eq!(
            ledger
                .register_payload("backdated", 199, 300, interval())
                .expect_err("closed registration cut")
                .code,
            "FMINE-REF-206"
        );
        assert_eq!(
            ledger
                .freeze_cohort(CohortSpec {
                    name: "backdated cohort".into(),
                    frozen_at_ns: 3,
                    members: vec![b],
                    resolution_rule: "synthetic".into(),
                    eligible_sources: vec!["synthetic".into()],
                    policy: AssessmentPolicy::default(),
                })
                .expect_err("closed membership cut")
                .code,
            "FMINE-REF-206"
        );
        ledger
            .append_outcome(b, 151, 201, published(0.5))
            .expect("late arrival with honest record time");
        assert_eq!(
            ledger
                .assess(cohort, 200, OutcomeSelector::FirstPublished)
                .expect("unchanged report"),
            &original
        );
        assert_eq!(ledger.outcomes(b).len(), 1);
    }

    #[test]
    fn record_time_is_monotone_across_forecasts_but_equal_time_batches_are_allowed() {
        let mut ledger = ForecastLedger::new();
        let a = ledger.register_payload("a", 1, 100, interval()).expect("a");
        let b = ledger.register_payload("b", 1, 100, interval()).expect("b");
        ledger
            .append_outcome(a, 101, 200, published(0.5))
            .expect("first member");
        assert_eq!(
            ledger
                .append_outcome(b, 101, 199, published(0.5))
                .expect_err("cross-forecast regression")
                .code,
            "FMINE-REF-206"
        );
        ledger
            .append_outcome(b, 101, 200, published(0.5))
            .expect("same record-time batch");
        assert_eq!(ledger.outcomes(a).len(), 1);
        assert_eq!(ledger.outcomes(b).len(), 1);
    }

    #[test]
    fn refused_writes_do_not_advance_the_shared_record_watermark() {
        let (mut ledger, _, ids) = make_cohort(vec![interval()], AssessmentPolicy::default());
        let mut wrong = citation("v1");
        wrong.source = "ineligible".into();
        assert_eq!(
            ledger
                .append_outcome(
                    ids[0],
                    101,
                    1000,
                    OutcomeKind::Published {
                        value: 0.5,
                        citation: wrong,
                    }
                )
                .expect_err("source refused")
                .code,
            "FMINE-REF-201"
        );
        ledger
            .append_outcome(ids[0], 101, 102, published(0.5))
            .expect("failed source did not move time");
        assert!(
            ledger
                .register_payload(
                    "invalid",
                    2000,
                    3000,
                    ForecastPayload::Gaussian {
                        mean: f64::NAN,
                        std_dev: 1.0,
                    }
                )
                .is_err()
        );
        let next = ledger
            .register_payload("next", 103, 200, interval())
            .expect("failed payload did not move time");
        let mut spec = CohortSpec {
            name: "next".into(),
            frozen_at_ns: 9999,
            members: vec![next],
            resolution_rule: "synthetic".into(),
            eligible_sources: vec!["synthetic".into()],
            policy: AssessmentPolicy::default(),
        };
        assert_eq!(
            ledger
                .freeze_cohort(spec.clone())
                .expect_err("late freeze")
                .code,
            "FMINE-REF-206"
        );
        spec.frozen_at_ns = 104;
        ledger
            .freeze_cohort(spec)
            .expect("failed cohort did not move time or reserve the name");
    }

    #[test]
    fn policy_cannot_lower_the_minimum_below_thirty_five_independent_outcomes() {
        let mut ledger = ForecastLedger::new();
        let id = ledger
            .register_payload("m", 1, 100, interval())
            .expect("forecast");
        let mut spec = CohortSpec {
            name: "minimum sample".into(),
            frozen_at_ns: 2,
            members: vec![id],
            resolution_rule: "synthetic".into(),
            eligible_sources: vec!["synthetic".into()],
            policy: AssessmentPolicy::default(),
        };
        for minimum in [0, 1, 5, 34] {
            spec.policy.min_calibration_n = minimum;
            assert_eq!(
                ledger
                    .freeze_cohort(spec.clone())
                    .expect_err("no positive small-n calibration")
                    .code,
                "FMINE-REF-109"
            );
        }
        spec.policy.min_calibration_n = MIN_CALIBRATION_N;
        ledger
            .freeze_cohort(spec)
            .expect("registered minimum at the plan floor");
    }
}
