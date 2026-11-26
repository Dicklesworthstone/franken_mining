//! DESIGN (BD-051, plan §10.5): explicit response to a retained forecast-monitor alarm.
//! Determinism: BitwisePortable (canonical source bits, checked integer time and declared text).
//!
//! This kernel consumes already persisted assessments through an authority adapter. It binds the first
//! original-publication crossing, prevents duplicate cases for the same alarm, and requires an explicit
//! human-role resolution before linking a new prospectively frozen monitor cohort. Roles, hypotheses,
//! probe order and resolution evidence remain declarations; none authenticates a person or retrains a model.

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::{Digest32, Encoder, ObjectId, blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};

use super::{Assessment, ForecastLedger, MemberAssessment, MemberOutcome, absent, frozen, late};

const MAX_CASES: usize = 4096;
const MAX_ITEMS: usize = 16;
const MAX_TEXT: usize = 2048;

/// The selected frozen forecast monitor; a case never silently switches its triggering test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MonitorKind {
    Coverage,
    Cusum,
}

impl MonitorKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Coverage => "coverage",
            Self::Cusum => "cusum",
        }
    }

    pub fn parse(value: &str) -> Result<Self, Refusal> {
        match value {
            "coverage" => Ok(Self::Coverage),
            "cusum" => Ok(Self::Cusum),
            _ => Err(invalid("investigation monitor must be coverage or cusum")),
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Coverage => 0,
            Self::Cusum => 1,
        }
    }
}

/// Explicit declared review outcomes from plan §10.5; none performs an automatic model update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionKind {
    HypothesisAdded,
    Recalibrated,
    DataDefect,
    FalseAlarmLogged,
}

impl ResolutionKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HypothesisAdded => "hypothesis_added",
            Self::Recalibrated => "recalibrated",
            Self::DataDefect => "data_defect",
            Self::FalseAlarmLogged => "false_alarm_logged",
        }
    }

    pub fn parse(value: &str) -> Result<Self, Refusal> {
        match value {
            "hypothesis_added" => Ok(Self::HypothesisAdded),
            "recalibrated" => Ok(Self::Recalibrated),
            "data_defect" => Ok(Self::DataDefect),
            "false_alarm_logged" => Ok(Self::FalseAlarmLogged),
            _ => Err(invalid("unknown investigation resolution kind")),
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::HypothesisAdded => 0,
            Self::Recalibrated => 1,
            Self::DataDefect => 2,
            Self::FalseAlarmLogged => 3,
        }
    }
}

/// Opening declarations. Times come from the authority adapter, never the request's wall clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenCase {
    pub case_id: String,
    pub owner_role: String,
    pub deadline_ns: i64,
    pub opened_ns: i64,
    /// Distinct HYP-labelled competing explanations, supplied in the reviewer's declared order.
    pub hypotheses: Vec<String>,
    /// Supplied ranking of proposed probes; this kernel computes no value of information.
    pub probes: Vec<String>,
}

/// A declared human review. Identity and the referenced evidence are not authenticated here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveCase {
    pub kind: ResolutionKind,
    pub human_role: String,
    pub rationale: String,
    pub evidence: String,
    pub recorded_ns: i64,
}

/// A retained original-event crossing, independently of an assessment's restatement selector.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseTrigger {
    pub cohort: Digest32,
    pub monitor: MonitorKind,
    /// One-based position in the monitor's prospectively frozen observation order.
    pub first_crossing: u64,
    pub member: MemberAssessment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseResolution {
    pub kind: ResolutionKind,
    pub human_role: String,
    pub rationale: String,
    pub evidence: String,
    pub recorded_ns: i64,
    pub digest: Digest32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseRearm {
    pub cohort: Digest32,
    pub recorded_ns: i64,
    pub digest: Digest32,
}

/// Append-only opening, resolution and optional link to a new monitor cohort.
#[derive(Debug, Clone, PartialEq)]
pub struct InvestigationCase {
    pub case_id: String,
    pub owner_role: String,
    pub deadline_ns: i64,
    pub opened_ns: i64,
    pub hypotheses: Vec<String>,
    pub probes: Vec<String>,
    pub assessment_digest: Digest32,
    pub trigger: CaseTrigger,
    pub opening_digest: Digest32,
    pub resolution: Option<CaseResolution>,
    pub rearm: Option<CaseRearm>,
    pub digest: Digest32,
}

impl InvestigationCase {
    #[must_use]
    pub const fn state(&self) -> &'static str {
        if self.rearm.is_some() {
            "rearmed"
        } else if self.resolution.is_some() {
            "resolved"
        } else {
            "open"
        }
    }

    #[must_use]
    pub fn is_overdue(&self, as_of_ns: i64) -> bool {
        self.resolution.is_none() && as_of_ns > self.deadline_ns
    }

    fn opening_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"INVO", 1);
        enc.put_str(&self.case_id);
        enc.put_str(&self.owner_role);
        enc.put_i64(self.deadline_ns);
        enc.put_i64(self.opened_ns);
        for declarations in [&self.hypotheses, &self.probes] {
            enc.put_u64(declarations.len() as u64);
            for declaration in declarations {
                enc.put_str(declaration);
            }
        }
        enc.put_fixed(&self.assessment_digest.0);
        enc.put_fixed(&self.trigger.cohort.0);
        enc.put_u8(self.trigger.monitor.tag());
        enc.put_u64(self.trigger.first_crossing);
        self.trigger.member.encode(&mut enc);
        enc.finish()
    }

    /// Full retained declarations are encoded, not merely their separately stored digest fields.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"INVS", 1);
        enc.put_bytes(&self.opening_bytes());
        enc.put_fixed(&self.opening_digest.0);
        enc.put_u8(u8::from(self.resolution.is_some()));
        if let Some(resolution) = &self.resolution {
            enc.put_u8(resolution.kind.tag());
            enc.put_str(&resolution.human_role);
            enc.put_str(&resolution.rationale);
            enc.put_str(&resolution.evidence);
            enc.put_i64(resolution.recorded_ns);
            enc.put_fixed(&resolution.digest.0);
        }
        enc.put_u8(u8::from(self.rearm.is_some()));
        if let Some(rearm) = &self.rearm {
            enc.put_fixed(&rearm.cohort.0);
            enc.put_i64(rearm.recorded_ns);
            enc.put_fixed(&rearm.digest.0);
        }
        enc.finish()
    }

    fn refresh_digest(&mut self) {
        self.digest = blake3_hash(&self.canonical_bytes());
    }
}

type TriggerKey = (Digest32, MonitorKind, ObjectId, u64, Digest32);

/// Bounded pure replay state. The authority adapter persists operations and verifies source membership.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct InvestigationLedger {
    cases: BTreeMap<String, InvestigationCase>,
    triggers: BTreeSet<TriggerKey>,
    latest_record_ns: Option<i64>,
}

impl InvestigationLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn get(&self, case_id: &str) -> Option<&InvestigationCase> {
        self.cases.get(case_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &InvestigationCase> {
        self.cases.values()
    }

    /// The adapter must prove that this exact assessment was persisted earlier in its journal.
    /// The trigger is derived from its retained original-publication monitor; callers cannot supply it.
    pub fn open(
        &mut self,
        assessment: &Assessment,
        monitor: MonitorKind,
        request: OpenCase,
    ) -> Result<&InvestigationCase, Refusal> {
        self.check_time(request.opened_ns)?;
        validate_open(&request)?;
        if self.cases.len() >= MAX_CASES {
            return Err(invalid("investigation ledger exceeds its bounded case capacity"));
        }
        if self.cases.contains_key(&request.case_id) {
            return Err(frozen("investigation case id is already retained"));
        }
        if request.opened_ns < assessment.as_of_ns {
            return Err(late("investigation cannot open before its assessment cut"));
        }
        if blake3_hash(&assessment.canonical_bytes()) != assessment.digest {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                "investigation source assessment digest does not match its retained contents",
            ));
        }
        let trigger = trigger(assessment, monitor)?;
        let (event_seq, event_digest) = trigger
            .member
            .selected_event
            .ok_or_else(|| invalid("investigation trigger has no original event"))?;
        let key = (
            trigger.cohort,
            trigger.monitor,
            trigger.member.forecast_id,
            event_seq,
            event_digest,
        );
        if self.triggers.contains(&key) {
            return Err(frozen(
                "the same original monitor alarm already has an investigation case",
            ));
        }
        let case_id = request.case_id.clone();
        let mut case = InvestigationCase {
            case_id: request.case_id,
            owner_role: request.owner_role,
            deadline_ns: request.deadline_ns,
            opened_ns: request.opened_ns,
            hypotheses: request.hypotheses,
            probes: request.probes,
            assessment_digest: assessment.digest,
            trigger,
            opening_digest: Digest32([0; 32]),
            resolution: None,
            rearm: None,
            digest: Digest32([0; 32]),
        };
        case.opening_digest = blake3_hash(&case.opening_bytes());
        case.refresh_digest();
        self.latest_record_ns = Some(case.opened_ns);
        self.triggers.insert(key);
        self.cases.insert(case_id.clone(), case);
        self.cases
            .get(&case_id)
            .ok_or_else(|| absent("new investigation case is unavailable"))
    }

    /// Records one explicit human-role review without removing the opening or changing any forecast.
    pub fn resolve(
        &mut self,
        case_id: &str,
        request: ResolveCase,
    ) -> Result<&InvestigationCase, Refusal> {
        self.check_time(request.recorded_ns)?;
        clean(&request.human_role, "human role", 128)?;
        clean(&request.rationale, "resolution rationale", MAX_TEXT)?;
        clean(&request.evidence, "resolution evidence", MAX_TEXT)?;
        let case = self
            .cases
            .get(case_id)
            .ok_or_else(|| absent("unknown investigation case"))?;
        if case.resolution.is_some() {
            return Err(frozen("investigation resolution is already retained"));
        }
        if request.recorded_ns < case.opened_ns {
            return Err(late("investigation resolution precedes its opening"));
        }
        let mut enc = Encoder::new_frame(*b"INVR", 1);
        enc.put_fixed(&case.opening_digest.0);
        enc.put_u8(request.kind.tag());
        enc.put_str(&request.human_role);
        enc.put_str(&request.rationale);
        enc.put_str(&request.evidence);
        enc.put_i64(request.recorded_ns);
        let resolution = CaseResolution {
            kind: request.kind,
            human_role: request.human_role,
            rationale: request.rationale,
            evidence: request.evidence,
            recorded_ns: request.recorded_ns,
            digest: blake3_hash(&enc.finish()),
        };
        let case = self
            .cases
            .get_mut(case_id)
            .ok_or_else(|| absent("unknown investigation case"))?;
        case.resolution = Some(resolution);
        case.refresh_digest();
        self.latest_record_ns = Some(request.recorded_ns);
        Ok(case)
    }

    /// Links a fresh prospective cohort after review. No alarm history is reset and no model is trained.
    /// Adapters must also enforce any stronger physical support-start boundary of their source records.
    pub fn rearm(
        &mut self,
        case_id: &str,
        forecasts: &ForecastLedger,
        cohort: Digest32,
        recorded_ns: i64,
    ) -> Result<&InvestigationCase, Refusal> {
        self.check_time(recorded_ns)?;
        let case = self
            .cases
            .get(case_id)
            .ok_or_else(|| absent("unknown investigation case"))?;
        if case.rearm.is_some() {
            return Err(frozen("investigation rearm is already retained"));
        }
        let resolution = case
            .resolution
            .as_ref()
            .ok_or_else(|| invalid("investigation requires an explicit human-role resolution"))?;
        let next = forecasts
            .cohort(cohort)
            .ok_or_else(|| absent("investigation rearm names an unknown cohort"))?;
        if cohort == case.trigger.cohort
            || next.spec.frozen_at_ns <= resolution.recorded_ns
            || next.spec.frozen_at_ns > recorded_ns
        {
            return Err(late(
                "rearm needs a different cohort frozen after resolution and before the rearm record",
            ));
        }
        let monitored = match case.trigger.monitor {
            MonitorKind::Coverage => next.spec.policy.coverage_monitor.is_some(),
            MonitorKind::Cusum => next.spec.policy.cusum_monitor.is_some(),
        };
        if !monitored {
            return Err(invalid(
                "rearm cohort must declare the same monitor kind as the original alarm",
            ));
        }
        for id in &next.spec.members {
            let forecast = forecasts
                .records
                .get(id)
                .ok_or_else(|| absent("rearm cohort member is unavailable"))?;
            if recorded_ns >= forecast.freeze_time_ns || !forecasts.outcomes(*id).is_empty() {
                return Err(late(
                    "rearm must precede every new cohort outcome time and recorded outcome",
                ));
            }
        }
        let mut enc = Encoder::new_frame(*b"INVA", 1);
        enc.put_fixed(&case.opening_digest.0);
        enc.put_fixed(&resolution.digest.0);
        enc.put_fixed(&cohort.0);
        enc.put_i64(recorded_ns);
        let rearm = CaseRearm {
            cohort,
            recorded_ns,
            digest: blake3_hash(&enc.finish()),
        };
        let case = self
            .cases
            .get_mut(case_id)
            .ok_or_else(|| absent("unknown investigation case"))?;
        case.rearm = Some(rearm);
        case.refresh_digest();
        self.latest_record_ns = Some(recorded_ns);
        Ok(case)
    }

    fn check_time(&self, recorded_ns: i64) -> Result<(), Refusal> {
        if recorded_ns < 0 || self.latest_record_ns.is_some_and(|prior| recorded_ns < prior) {
            Err(late("investigation authority chronology cannot regress"))
        } else {
            Ok(())
        }
    }
}

fn trigger(assessment: &Assessment, monitor: MonitorKind) -> Result<CaseTrigger, Refusal> {
    let (first_crossing, member, expected) = match monitor {
        MonitorKind::Coverage => {
            let report = assessment
                .coverage_monitor
                .as_ref()
                .ok_or_else(|| absent("assessment has no frozen coverage monitor"))?;
            let first = report
                .snapshot
                .first_crossing
                .ok_or_else(|| absent("coverage monitor has no retained alarm"))?;
            let index = crossing_index(first, report.observations.len())?;
            let expected = report
                .policy
                .order
                .get(index)
                .ok_or_else(|| invalid("coverage crossing is outside the frozen order"))?;
            (first, &report.observations[index].member, *expected)
        }
        MonitorKind::Cusum => {
            let report = assessment
                .cusum_monitor
                .as_ref()
                .ok_or_else(|| absent("assessment has no frozen CUSUM monitor"))?;
            let first = report
                .snapshot
                .first_crossing()
                .ok_or_else(|| absent("CUSUM monitor has no retained alarm"))?;
            let index = crossing_index(first, report.observations.len())?;
            let expected = report
                .policy
                .order
                .get(index)
                .ok_or_else(|| invalid("CUSUM crossing is outside the frozen order"))?;
            (first, &report.observations[index].member, *expected)
        }
    };
    let MemberOutcome::Resolved {
        value,
        event_seq,
        event_digest,
        citation,
    } = &member.outcome
    else {
        return Err(invalid("monitor crossing must retain a resolved original event"));
    };
    citation.validate()?;
    if !value.is_finite()
        || member.forecast_id != expected
        || member.selected_event != Some((*event_seq, *event_digest))
    {
        return Err(invalid(
            "monitor crossing does not match its original event or frozen observation order",
        ));
    }
    Ok(CaseTrigger {
        cohort: assessment.cohort,
        monitor,
        first_crossing,
        member: member.clone(),
    })
}

fn crossing_index(first_crossing: u64, count: usize) -> Result<usize, Refusal> {
    first_crossing
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .filter(|&index| index < count)
        .ok_or_else(|| invalid("monitor crossing is outside its retained observations"))
}

fn validate_open(request: &OpenCase) -> Result<(), Refusal> {
    clean(&request.case_id, "case id", 128)?;
    clean(&request.owner_role, "owner role", 128)?;
    if request.deadline_ns <= request.opened_ns {
        return Err(late("investigation deadline must follow its opening"));
    }
    if !(2..=MAX_ITEMS).contains(&request.hypotheses.len())
        || !(1..=MAX_ITEMS).contains(&request.probes.len())
    {
        return Err(invalid(
            "investigation needs 2..16 labelled hypotheses and 1..16 proposed probes",
        ));
    }
    let mut labels = BTreeSet::new();
    for hypothesis in &request.hypotheses {
        clean(hypothesis, "hypothesis", MAX_TEXT)?;
        let (label, description) = hypothesis
            .split_once(':')
            .ok_or_else(|| invalid("hypothesis needs a HYP-labelled description"))?;
        let suffix = label
            .strip_prefix("HYP-")
            .ok_or_else(|| invalid("hypothesis label must begin HYP-"))?;
        if suffix.is_empty()
            || !suffix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            || description.trim().is_empty()
            || !labels.insert(label)
        {
            return Err(invalid(
                "hypotheses need distinct HYP labels and nonempty descriptions",
            ));
        }
    }
    let mut probes = BTreeSet::new();
    for probe in &request.probes {
        clean(probe, "probe", MAX_TEXT)?;
        if !probes.insert(probe) {
            return Err(invalid("investigation probe declarations must be distinct"));
        }
    }
    Ok(())
}

fn clean(value: &str, label: &str, limit: usize) -> Result<(), Refusal> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > limit
        || value.chars().any(char::is_control)
    {
        Err(invalid(format!(
            "{label} must be nonempty, trimmed, at most {limit} bytes and without control characters"
        )))
    } else {
        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, message)
}

#[cfg(test)]
mod tests;
