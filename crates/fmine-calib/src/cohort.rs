//! Forecast cohorts and outcome vintages (BD-343; plan §10.1, §21.6). A cohort's membership is frozen before any
//! outcome is known, outcomes arrive as append-only events that each carry their vintage, and every assessment
//! shows the whole frozen denominator and how far the unresolved members could still move the result.
//! Determinism: Exact (rationals, integers and content hashes; no floating point).
//!
//! - A member without an outcome stays in every denominator, `Unresolved` with its reason. The whole-cohort bounds
//!   range over every outcome it could still have: they bound what is missing, they are not confidence intervals.
//! - A restatement is a new event that names the one it supersedes. No event is edited or removed.
//! - An assessment, once taken, is kept and never recomputed in place. Taken again at the same `as_of` it renders
//!   byte for byte the same, because once it exists an event recorded at or before its `as_of` is refused.
//! - Times are UTC seconds from the caller. They order the ledger; they are not a time proof (R7-C061, BD-068).

use std::cmp::Ordering;
use std::collections::BTreeMap;

use fmine_core::Encoder;
use fmine_core::hash::sha256;
use fmine_evidence::{Refusal, RefusalCode};

use crate::record::{Rat, RecordRules, interval_score, parse_hex32};

/// `z` of the central 80% normal interval, to the digits `ForecastLedger::register` uses.
const Z80: &str = "1.2815515655446004";

fn domain(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::OutsidePhysicalDomain109, detail)
}

fn ex<T, E>(r: Result<T, E>) -> Result<T, Refusal> {
    r.map_err(|_| domain("exact arithmetic overflowed i128"))
}

fn less(a: Rat, b: Rat) -> Result<bool, Refusal> {
    Ok(ex(a.checked_cmp(b))? == Ordering::Less)
}

fn int(n: u64) -> Rat {
    Rat::from_integer(i128::from(n))
}

fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// `7/10`, or `7` when the denominator is 1.
fn show(q: Rat) -> String {
    if q.den() == 1 {
        q.num().to_string()
    } else {
        format!("{}/{}", q.num(), q.den())
    }
}

fn put_rat(enc: &mut Encoder, q: Rat) {
    enc.put_i128(q.num());
    enc.put_i128(q.den());
}

/// What a frozen forecast states. Every number is exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForecastPayload {
    /// A normal forecast. Its scored event is the central 80% interval `mean ± z·sd`, `z` = 1.2815515655446004.
    Gaussian { mean: Rat, sd: Rat },
    /// A range. `nominal` is the probability it states; issuer guidance states none.
    Interval {
        lo: Rat,
        hi: Rat,
        nominal: Option<Rat>,
    },
    /// Values at strictly increasing levels in (0, 1). The scored event is the outermost pair, which must be
    /// central (its two levels sum to 1); its probability is their difference.
    Quantiles { levels: Vec<Rat>, values: Vec<Rat> },
}

/// The event a member is scored on: its outcome falls in `[lo, hi]`, with probability `level` when one is stated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoredEvent {
    pub lo: Rat,
    pub hi: Rat,
    pub level: Option<Rat>,
}

impl ForecastPayload {
    /// The scored event. Refuses `FMINE-REF-109` for `sd <= 0`, `lo > hi`, a probability outside (0, 1),
    /// quantile levels that do not increase or values that decrease, and an outermost pair that is not central.
    pub fn scored_event(&self) -> Result<ScoredEvent, Refusal> {
        let one = Rat::from_integer(1);
        let event = match self {
            Self::Gaussian { mean, sd } => {
                if sd.signum() <= 0 {
                    return Err(domain(format!(
                        "a Gaussian forecast needs sd > 0, got {}",
                        show(*sd)
                    )));
                }
                let half = ex(ex(Rat::from_decimal_str(Z80))?.checked_mul(*sd))?;
                ScoredEvent {
                    lo: ex(mean.checked_sub(half))?,
                    hi: ex(mean.checked_add(half))?,
                    level: Some(ex(Rat::new(4, 5))?),
                }
            }
            Self::Interval { lo, hi, nominal } => ScoredEvent {
                lo: *lo,
                hi: *hi,
                level: *nominal,
            },
            Self::Quantiles { levels, values } => {
                let (Some(&a), Some(&b), Some(&lo), Some(&hi)) =
                    (levels.first(), levels.last(), values.first(), values.last())
                else {
                    return Err(domain("a quantile forecast needs at least two levels"));
                };
                if levels.len() < 2 || levels.len() != values.len() {
                    return Err(domain(
                        "a quantile forecast needs two or more levels, one value each",
                    ));
                }
                for w in levels.windows(2) {
                    if !less(w[0], w[1])? {
                        return Err(domain("quantile levels must strictly increase"));
                    }
                }
                for w in values.windows(2) {
                    if less(w[1], w[0])? {
                        return Err(domain("quantile values must not decrease"));
                    }
                }
                if ex(a.checked_add(b))? != one {
                    return Err(domain(format!(
                        "the outermost quantiles {} and {} are not central",
                        show(a),
                        show(b)
                    )));
                }
                ScoredEvent {
                    lo,
                    hi,
                    level: Some(ex(b.checked_sub(a))?),
                }
            }
        };
        if less(event.hi, event.lo)? {
            return Err(domain(format!(
                "inverted interval [{}, {}]",
                show(event.lo),
                show(event.hi)
            )));
        }
        if let Some(p) = event.level
            && (p.signum() <= 0 || !less(p, one)?)
        {
            return Err(domain(format!(
                "a stated probability must lie in (0, 1), got {}",
                show(p)
            )));
        }
        Ok(event)
    }

    fn encode(&self, enc: &mut Encoder) {
        match self {
            Self::Gaussian { mean, sd } => {
                enc.put_u8(0);
                put_rat(enc, *mean);
                put_rat(enc, *sd);
            }
            Self::Interval { lo, hi, nominal } => {
                enc.put_u8(1);
                put_rat(enc, *lo);
                put_rat(enc, *hi);
                match nominal {
                    None => enc.put_u8(0),
                    Some(p) => {
                        enc.put_u8(1);
                        put_rat(enc, *p);
                    }
                }
            }
            Self::Quantiles { levels, values } => {
                enc.put_u8(2);
                enc.put_u32(u32::try_from(levels.len()).unwrap_or(u32::MAX));
                for (l, v) in levels.iter().zip(values) {
                    put_rat(enc, *l);
                    put_rat(enc, *v);
                }
            }
        }
    }
}

/// One frozen forecast in a cohort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CohortMember {
    /// The forecast's id: a ledger `ObjectId`'s text, or a record row's `site/target`.
    pub forecast_id: String,
    /// When the forecast was registered (UTC seconds).
    pub registered_at: i64,
    pub payload: ForecastPayload,
}

/// A set of forecasts whose membership is fixed when it is frozen. To change it, freeze a new cohort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cohort {
    name: String,
    frozen_at: i64,
    members: Vec<CohortMember>,
    events: Vec<ScoredEvent>,
    resolution_rule: String,
    eligible_sources: Vec<String>,
    digest: String,
}

impl Cohort {
    /// Freezes a cohort; members are kept sorted by id. Refuses `FMINE-REF-109` for no members or a payload
    /// without a valid scored event, `FMINE-REF-507` for a forecast listed twice, and `FMINE-REF-206` for a member
    /// registered after `frozen_at`.
    pub fn freeze(
        name: impl Into<String>,
        frozen_at: i64,
        mut members: Vec<CohortMember>,
        resolution_rule: impl Into<String>,
        mut eligible_sources: Vec<String>,
    ) -> Result<Self, Refusal> {
        let name = name.into();
        if members.is_empty() {
            return Err(domain(format!("cohort {name} has no members")));
        }
        members.sort_by(|a, b| a.forecast_id.cmp(&b.forecast_id));
        if let Some(w) = members
            .windows(2)
            .find(|w| w[0].forecast_id == w[1].forecast_id)
        {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                format!(
                    "forecast {} is listed twice in cohort {name}",
                    w[0].forecast_id
                ),
            ));
        }
        if let Some(m) = members.iter().find(|m| m.registered_at > frozen_at) {
            return Err(Refusal::new(
                RefusalCode::LateRegistration206,
                format!(
                    "forecast {} was registered at {} after cohort {name} froze at {frozen_at}",
                    m.forecast_id, m.registered_at
                ),
            ));
        }
        let events = members
            .iter()
            .map(|m| m.payload.scored_event())
            .collect::<Result<Vec<_>, _>>()?;
        eligible_sources.sort();
        eligible_sources.dedup();
        let resolution_rule = resolution_rule.into();
        let mut enc = Encoder::new_frame(*b"FCOH", 1);
        enc.put_str(&name);
        enc.put_i64(frozen_at);
        enc.put_str(&resolution_rule);
        enc.put_u32(u32::try_from(eligible_sources.len()).unwrap_or(u32::MAX));
        for s in &eligible_sources {
            enc.put_str(s);
        }
        enc.put_u32(u32::try_from(members.len()).unwrap_or(u32::MAX));
        for m in &members {
            enc.put_str(&m.forecast_id);
            enc.put_i64(m.registered_at);
            m.payload.encode(&mut enc);
        }
        let digest = sha256(&enc.finish()).to_hex();
        Ok(Self {
            name,
            frozen_at,
            members,
            events,
            resolution_rule,
            eligible_sources,
            digest,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn frozen_at(&self) -> i64 {
        self.frozen_at
    }

    #[must_use]
    pub fn members(&self) -> &[CohortMember] {
        &self.members
    }

    #[must_use]
    pub fn resolution_rule(&self) -> &str {
        &self.resolution_rule
    }

    #[must_use]
    pub fn eligible_sources(&self) -> &[String] {
        &self.eligible_sources
    }

    /// sha256 of the cohort's `FCOH` v1 frame: name, freeze time, rule, sources, then each member and payload.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    fn holds(&self, forecast_id: &str) -> bool {
        self.members
            .binary_search_by(|m| m.forecast_id.as_str().cmp(forecast_id))
            .is_ok()
    }
}

/// Why an outcome has no value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MissingReason {
    NotPublished,
    Withdrawn,
    DefinitionChanged,
    Divested,
    Overdue,
}

impl MissingReason {
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::NotPublished => "not_published",
            Self::Withdrawn => "withdrawn",
            Self::DefinitionChanged => "definition_changed",
            Self::Divested => "divested",
            Self::Overdue => "overdue",
        }
    }
}

/// What an outcome event says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutcomeKind {
    /// The value as first published: its vintage (`2024Q1`), its cell locator and the file's sha256.
    Published {
        value: Rat,
        vintage: String,
        locator: String,
        sha256: String,
    },
    /// A later value of the same outcome, naming the event it supersedes. The superseded event stays.
    Restated {
        value: Rat,
        vintage: String,
        locator: String,
        sha256: String,
        supersedes: u64,
    },
    /// No value, and why, as of this event.
    Missing { reason: MissingReason },
}

impl OutcomeKind {
    fn value(&self) -> Option<Rat> {
        match self {
            Self::Published { value, .. } | Self::Restated { value, .. } => Some(*value),
            Self::Missing { .. } => None,
        }
    }
}

/// One append-only outcome event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeEvent {
    /// Its place in the ledger, from 0.
    pub seq: u64,
    pub forecast_id: String,
    /// When the outcome became public (UTC seconds).
    pub availability_time: i64,
    /// When the ledger recorded it (UTC seconds). An assessment's `as_of` cuts on this time.
    pub record_time: i64,
    pub kind: OutcomeKind,
}

/// Which value of an outcome an assessment scores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selector {
    /// The value as first published; a later restatement never replaces it (the FY2026 rules' `outcome_vintage`).
    FirstPublished,
    /// The latest event recorded by `as_of`; an outcome withdrawn after publication is unresolved again.
    LatestAsOf,
}

impl Selector {
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::FirstPublished => "first_published",
            Self::LatestAsOf => "latest_as_of",
        }
    }
}

/// Why a member has no scored value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Unresolved {
    Missing(MissingReason),
    /// No event recorded by `as_of`.
    Pending,
}

impl Unresolved {
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Missing(r) => r.tag(),
            Self::Pending => "pending",
        }
    }
}

/// A member's state in an assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberResult {
    Resolved {
        value: Rat,
        event_seq: u64,
        inside: bool,
    },
    Unresolved(Unresolved),
}

/// The rules an assessment is taken under: the n_eff correlation range and the minimum resolved count for a
/// positive calibration claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssessParams {
    pub rho_low: Rat,
    pub rho_high: Rat,
    pub min_n: u64,
}

impl AssessParams {
    /// The values a record's frozen rules state (`rho_low`, `rho_high`, `calibration_claim_min_n`).
    #[must_use]
    pub fn from_rules(rules: &RecordRules) -> Self {
        Self {
            rho_low: rules.rho_low,
            rho_high: rules.rho_high,
            min_n: rules.calibration_claim_min_n,
        }
    }
}

/// What an assessment states about its cohort. Every bound is a range over the unresolved members' outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub n_members: u64,
    pub n_resolved: u64,
    /// Unresolved members by reason; they sum to `n_members − n_resolved`.
    pub unresolved: BTreeMap<Unresolved, u64>,
    /// Resolved members whose outcome fell inside their scored event.
    pub covered: u64,
    /// `[covered / n_members, (covered + unresolved) / n_members]`: where whole-cohort coverage can still finish.
    pub coverage_bound: (Rat, Rat),
    /// Members whose scored event states a probability. Only they have a Brier or an interval score.
    pub n_with_level: u64,
    /// The mean Brier score of those members.
    pub brier_bound: Option<(Rat, Rat)>,
    /// The mean interval score of those members: an upper end only when none is unresolved, because a missing
    /// outcome can lie arbitrarily far away.
    pub interval_score_bound: Option<(Rat, Option<Rat>)>,
    /// `n / (1 + (n − 1)·ρ)` over the resolved members, at `rho_high` then `rho_low`.
    pub n_eff: Option<(Rat, Rat)>,
    /// `n_resolved >= min_n`: below it no positive calibration claim is made (§21.6).
    pub min_n_met: bool,
}

/// One immutable assessment of a cohort at an as-of cut under a selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    cohort: String,
    cohort_digest: String,
    as_of: i64,
    selector: Selector,
    params: AssessParams,
    per_member: Vec<(String, MemberResult)>,
    summary: Summary,
    render: String,
    digest: String,
}

impl Assessment {
    #[must_use]
    pub fn cohort(&self) -> &str {
        &self.cohort
    }

    #[must_use]
    pub fn cohort_digest(&self) -> &str {
        &self.cohort_digest
    }

    #[must_use]
    pub fn as_of(&self) -> i64 {
        self.as_of
    }

    #[must_use]
    pub fn selector(&self) -> Selector {
        self.selector
    }

    #[must_use]
    pub fn params(&self) -> AssessParams {
        self.params
    }

    /// Each member in id order with its state.
    #[must_use]
    pub fn per_member(&self) -> &[(String, MemberResult)] {
        &self.per_member
    }

    #[must_use]
    pub fn summary(&self) -> &Summary {
        &self.summary
    }

    /// The canonical text: a header, one line per member, then the summary, each line ending in LF.
    #[must_use]
    pub fn render(&self) -> &str {
        &self.render
    }

    /// sha256 of [`Self::render`].
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// The append-only store of cohorts, outcome events and assessments.
#[derive(Debug, Clone, Default)]
pub struct CohortLedger {
    cohorts: BTreeMap<String, Cohort>,
    events: Vec<OutcomeEvent>,
    assessments: Vec<Assessment>,
}

impl CohortLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a frozen cohort; the same cohort again changes nothing. Refuses `FMINE-REF-507` for another cohort
    /// under a name already taken (membership is never edited), and `FMINE-REF-206` for a cohort frozen at or
    /// before the latest event already recorded, which could have been chosen knowing it.
    pub fn add_cohort(&mut self, cohort: Cohort) -> Result<(), Refusal> {
        if let Some(have) = self.cohorts.get(cohort.name()) {
            if have.digest() == cohort.digest() {
                return Ok(());
            }
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                format!(
                    "cohort {} is frozen as {}; a changed membership is a new cohort under a new name",
                    cohort.name(),
                    have.digest()
                ),
            ));
        }
        if let Some(last) = self.events.last()
            && cohort.frozen_at() <= last.record_time
        {
            return Err(Refusal::new(
                RefusalCode::LateRegistration206,
                format!(
                    "cohort {} froze at {}, not after the event recorded at {}",
                    cohort.name(),
                    cohort.frozen_at(),
                    last.record_time
                ),
            ));
        }
        self.cohorts.insert(cohort.name().to_string(), cohort);
        Ok(())
    }

    /// Appends an outcome event and returns its `seq`. Refuses:
    /// - `FMINE-REF-201`: a forecast in no cohort, or a restatement that supersedes no earlier value of it;
    /// - `FMINE-REF-206`: an outcome available at or before the freeze of a cohort that holds it;
    /// - `FMINE-REF-507`: a record time before the last event's or at or before an assessment's `as_of`
    ///   (backdating), or a second `Published` for one forecast (a later value is `Restated`);
    /// - `FMINE-REF-109`: a record time before the availability time, or a sha256 that is not 64 hex digits.
    pub fn append(
        &mut self,
        forecast_id: &str,
        availability_time: i64,
        record_time: i64,
        kind: OutcomeKind,
    ) -> Result<u64, Refusal> {
        let latest_freeze = self
            .cohorts
            .values()
            .filter(|c| c.holds(forecast_id))
            .map(Cohort::frozen_at)
            .max()
            .ok_or_else(|| {
                Refusal::new(
                    RefusalCode::NoClaimInDomain201,
                    format!("forecast {forecast_id} is in no cohort"),
                )
            })?;
        if availability_time <= latest_freeze {
            return Err(Refusal::new(
                RefusalCode::LateRegistration206,
                format!(
                    "the outcome of {forecast_id} was available at {availability_time}, not after its cohort froze at \
                     {latest_freeze}"
                ),
            ));
        }
        if record_time < availability_time {
            return Err(domain(format!(
                "the outcome of {forecast_id} is recorded at {record_time}, before it was available at \
                 {availability_time}"
            )));
        }
        let backdated = |at: i64| {
            Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                format!("an event recorded at {record_time} would be backdated behind {at}"),
            )
        };
        if let Some(last) = self.events.last()
            && record_time < last.record_time
        {
            return Err(backdated(last.record_time));
        }
        if let Some(as_of) = self.assessments.iter().map(Assessment::as_of).max()
            && record_time <= as_of
        {
            return Err(backdated(as_of));
        }
        let earlier = |seq: u64| {
            self.events
                .iter()
                .find(|e| e.seq == seq && e.forecast_id == forecast_id && e.kind.value().is_some())
        };
        match &kind {
            OutcomeKind::Published { sha256, .. } => {
                if self.events.iter().any(|e| {
                    e.forecast_id == forecast_id && matches!(e.kind, OutcomeKind::Published { .. })
                }) {
                    return Err(Refusal::new(
                        RefusalCode::FrozenRecordMutation507,
                        format!(
                            "{forecast_id} already has a first publication; record a later value as Restated"
                        ),
                    ));
                }
                check_sha(sha256)?;
            }
            OutcomeKind::Restated {
                sha256, supersedes, ..
            } => {
                if earlier(*supersedes).is_none() {
                    return Err(Refusal::new(
                        RefusalCode::NoClaimInDomain201,
                        format!("event {supersedes} is no earlier value of {forecast_id}"),
                    ));
                }
                check_sha(sha256)?;
            }
            OutcomeKind::Missing { .. } => {}
        }
        let seq = count(self.events.len());
        self.events.push(OutcomeEvent {
            seq,
            forecast_id: forecast_id.to_string(),
            availability_time,
            record_time,
            kind,
        });
        Ok(seq)
    }

    /// Assesses a cohort from the events recorded by `as_of`, keeps the assessment and returns it. Refuses
    /// `FMINE-REF-201` for an unknown cohort and `FMINE-REF-109` for a correlation range outside
    /// `0 <= rho_low <= rho_high <= 1`.
    pub fn assess(
        &mut self,
        cohort: &str,
        as_of: i64,
        selector: Selector,
        params: AssessParams,
    ) -> Result<Assessment, Refusal> {
        let c = self.cohorts.get(cohort).ok_or_else(|| {
            Refusal::new(
                RefusalCode::NoClaimInDomain201,
                format!("no cohort {cohort}"),
            )
        })?;
        let (zero, one) = (Rat::from_integer(0), Rat::from_integer(1));
        if less(params.rho_low, zero)?
            || less(params.rho_high, params.rho_low)?
            || less(one, params.rho_high)?
        {
            return Err(domain(format!(
                "correlation range [{}, {}] is not inside [0, 1]",
                show(params.rho_low),
                show(params.rho_high)
            )));
        }
        let mut per_member = Vec::with_capacity(c.members.len());
        for (m, ev) in c.members.iter().zip(&c.events) {
            let known = self
                .events
                .iter()
                .filter(|e| e.forecast_id == m.forecast_id && e.record_time <= as_of);
            let chosen = match selector {
                Selector::FirstPublished => known
                    .clone()
                    .find(|e| matches!(e.kind, OutcomeKind::Published { .. }))
                    .or_else(|| known.clone().rfind(|e| e.kind.value().is_none())),
                Selector::LatestAsOf => known.clone().last(),
            };
            let result = match chosen {
                None => MemberResult::Unresolved(Unresolved::Pending),
                Some(e) => match (&e.kind, e.kind.value()) {
                    (OutcomeKind::Missing { reason }, _) => {
                        MemberResult::Unresolved(Unresolved::Missing(*reason))
                    }
                    (_, Some(value)) => MemberResult::Resolved {
                        value,
                        event_seq: e.seq,
                        inside: !less(value, ev.lo)? && !less(ev.hi, value)?,
                    },
                    (_, None) => MemberResult::Unresolved(Unresolved::Pending),
                },
            };
            per_member.push((m.forecast_id.clone(), result));
        }
        let summary = summarise(&c.events, &per_member, params)?;
        let render = render(c, as_of, selector, params, &per_member, &summary);
        let assessment = Assessment {
            cohort: c.name.clone(),
            cohort_digest: c.digest.clone(),
            as_of,
            selector,
            params,
            per_member,
            summary,
            digest: sha256(render.as_bytes()).to_hex(),
            render,
        };
        self.assessments.push(assessment.clone());
        Ok(assessment)
    }

    #[must_use]
    pub fn cohort(&self, name: &str) -> Option<&Cohort> {
        self.cohorts.get(name)
    }

    #[must_use]
    pub fn events(&self) -> &[OutcomeEvent] {
        &self.events
    }

    /// Every assessment taken, in order. None is ever changed or removed.
    #[must_use]
    pub fn assessments(&self) -> &[Assessment] {
        &self.assessments
    }
}

fn check_sha(hex: &str) -> Result<(), Refusal> {
    match parse_hex32(hex) {
        Some(_) => Ok(()),
        None => Err(domain(format!("{hex:?} is not a sha256 of 64 hex digits"))),
    }
}

/// `(p − o)²` for outcome `o` in {0, 1}.
fn brier(p: Rat, inside: bool) -> Result<Rat, Refusal> {
    let miss = if inside {
        ex(Rat::from_integer(1).checked_sub(p))?
    } else {
        p
    };
    ex(miss.checked_mul(miss))
}

fn mean(sum: Rat, n: u64) -> Result<Rat, Refusal> {
    ex(sum.checked_div(int(n)))
}

fn summarise(
    events: &[ScoredEvent],
    per_member: &[(String, MemberResult)],
    params: AssessParams,
) -> Result<Summary, Refusal> {
    let zero = Rat::from_integer(0);
    let one = Rat::from_integer(1);
    let n_members = count(per_member.len());
    let mut unresolved = BTreeMap::new();
    let (mut n_resolved, mut covered, mut n_with_level) = (0_u64, 0_u64, 0_u64);
    let (mut brier_lo, mut brier_hi, mut is_lo, mut is_hi) = (zero, zero, zero, Some(zero));
    for (ev, (_, result)) in events.iter().zip(per_member) {
        match result {
            MemberResult::Resolved { value, inside, .. } => {
                n_resolved += 1;
                covered += u64::from(*inside);
                if let Some(p) = ev.level {
                    let b = brier(p, *inside)?;
                    brier_lo = ex(brier_lo.checked_add(b))?;
                    brier_hi = ex(brier_hi.checked_add(b))?;
                    let is = interval_score(ev.lo, ev.hi, ex(one.checked_sub(p))?, *value)
                        .map_err(|e| domain(format!("interval score: {e:?}")))?;
                    is_lo = ex(is_lo.checked_add(is))?;
                    is_hi = is_hi.map(|s| ex(s.checked_add(is))).transpose()?;
                }
            }
            MemberResult::Unresolved(why) => {
                *unresolved.entry(*why).or_insert(0) += 1;
                if let Some(p) = ev.level {
                    let (a, b) = (brier(p, true)?, brier(p, false)?);
                    let (min, max) = if less(a, b)? { (a, b) } else { (b, a) };
                    brier_lo = ex(brier_lo.checked_add(min))?;
                    brier_hi = ex(brier_hi.checked_add(max))?;
                    // The least an unresolved member can score is its width, when the outcome lands inside.
                    is_lo = ex(is_lo.checked_add(ex(ev.hi.checked_sub(ev.lo))?))?;
                    is_hi = None;
                }
            }
        }
        n_with_level += u64::from(ev.level.is_some());
    }
    let n_unresolved = n_members - n_resolved;
    let coverage_bound = (
        mean(int(covered), n_members)?,
        mean(int(covered + n_unresolved), n_members)?,
    );
    let (brier_bound, interval_score_bound) = if n_with_level == 0 {
        (None, None)
    } else {
        (
            Some((mean(brier_lo, n_with_level)?, mean(brier_hi, n_with_level)?)),
            Some((
                mean(is_lo, n_with_level)?,
                is_hi.map(|s| mean(s, n_with_level)).transpose()?,
            )),
        )
    };
    let n_eff = if n_resolved == 0 {
        None
    } else {
        let n = int(n_resolved);
        let at = |rho: Rat| -> Result<Rat, Refusal> {
            let inflation = ex(one.checked_add(ex(int(n_resolved - 1).checked_mul(rho))?))?;
            ex(n.checked_div(inflation))
        };
        Some((at(params.rho_high)?, at(params.rho_low)?))
    };
    Ok(Summary {
        n_members,
        n_resolved,
        unresolved,
        covered,
        coverage_bound,
        n_with_level,
        brier_bound,
        interval_score_bound,
        n_eff,
        min_n_met: n_resolved >= params.min_n,
    })
}

fn render(
    c: &Cohort,
    as_of: i64,
    selector: Selector,
    params: AssessParams,
    per_member: &[(String, MemberResult)],
    s: &Summary,
) -> String {
    let mut out = format!(
        "assessment v1\ncohort {} {}\nas_of {as_of} selector {}\nparams rho {} {} min_n {}\n",
        c.name,
        c.digest,
        selector.tag(),
        show(params.rho_low),
        show(params.rho_high),
        params.min_n
    );
    for (id, result) in per_member {
        out.push_str(&match result {
            MemberResult::Resolved {
                value,
                event_seq,
                inside,
            } => format!(
                "member {id} resolved {} seq {event_seq} {}\n",
                show(*value),
                if *inside { "inside" } else { "outside" }
            ),
            MemberResult::Unresolved(why) => format!("member {id} unresolved {}\n", why.tag()),
        });
    }
    let reasons: Vec<String> = s
        .unresolved
        .iter()
        .map(|(why, n)| format!("{}:{n}", why.tag()))
        .collect();
    out.push_str(&format!(
        "n_members {} n_resolved {} unresolved {}\n",
        s.n_members,
        s.n_resolved,
        if reasons.is_empty() {
            "none".to_string()
        } else {
            reasons.join(",")
        }
    ));
    let observed = if s.n_resolved == 0 {
        "none".to_string()
    } else {
        format!("{}/{}", s.covered, s.n_resolved)
    };
    out.push_str(&format!(
        "coverage observed {observed} whole_cohort [{}, {}]\n",
        show(s.coverage_bound.0),
        show(s.coverage_bound.1)
    ));
    match (s.brier_bound, s.interval_score_bound) {
        (Some((b_lo, b_hi)), Some((i_lo, i_hi))) => out.push_str(&format!(
            "brier over {} [{}, {}]\ninterval_score over {} [{}, {}]\n",
            s.n_with_level,
            show(b_lo),
            show(b_hi),
            s.n_with_level,
            show(i_lo),
            i_hi.map_or_else(|| "no finite bound".to_string(), show)
        )),
        _ => out.push_str("brier none: no member states a probability\n"),
    }
    out.push_str(&match s.n_eff {
        Some((lo, hi)) => format!("n_eff [{}, {}]\n", show(lo), show(hi)),
        None => "n_eff none\n".to_string(),
    });
    out.push_str(&format!(
        "positive_calibration_claim {} (n_resolved {} of min_n {})\n",
        if s.min_n_met {
            "permitted"
        } else {
            "not_permitted"
        },
        s.n_resolved,
        params.min_n
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::parse_utc;
    use fmine_core::rand::{PhiloxStream, RngKey};

    fn r(n: i128, d: i128) -> Rat {
        Rat::new(n, d).expect("rational")
    }

    fn i(n: i128) -> Rat {
        Rat::from_integer(n)
    }

    fn t(s: &str) -> i64 {
        parse_utc(s).expect("utc")
    }

    fn interval(id: &str, lo: i128, hi: i128, nominal: Option<Rat>) -> CohortMember {
        CohortMember {
            forecast_id: id.to_string(),
            registered_at: 0,
            payload: ForecastPayload::Interval {
                lo: i(lo),
                hi: i(hi),
                nominal,
            },
        }
    }

    fn published(value: Rat) -> OutcomeKind {
        OutcomeKind::Published {
            value,
            vintage: "2026Q4".into(),
            locator: "synthetic:Sheet!A1".into(),
            sha256: "ab".repeat(32),
        }
    }

    const PARAMS: AssessParams = AssessParams {
        rho_low: Rat::from_integer(0),
        rho_high: Rat::from_integer(1),
        min_n: 35,
    };

    fn half_params() -> AssessParams {
        AssessParams {
            rho_high: r(1, 2),
            ..PARAMS
        }
    }

    fn code(r: Refusal) -> (&'static str, u8) {
        (r.code, r.exit_code)
    }

    /// The bead's worked example 1: ten frozen 80% intervals, eight outcomes, seven inside. Observed coverage is
    /// 7/8 and whole-cohort coverage can still finish anywhere in [7/10, 9/10]. Every other number below was
    /// worked by hand: Brier (7·(1/5)² + (4/5)² + 2·(1/5)²)/10 = 1/10 up to (7/25 + 16/25 + 2·16/25)/10 = 11/50;
    /// interval score (7·10 + 110 + 2·10)/10 = 20 with no finite upper end; n_eff 8/(1 + 7/2) = 16/9 and 8.
    #[test]
    fn test_worked_example_unresolved_members_stay_in_the_denominator() {
        let ids: Vec<String> = (0..10).map(|k| format!("site{k:02}")).collect();
        let members = ids
            .iter()
            .map(|id| interval(id, 0, 10, Some(r(4, 5))))
            .collect();
        let cohort =
            Cohort::freeze("ex1", 100, members, "first publication", vec![]).expect("freeze");
        let mut ledger = CohortLedger::new();
        ledger.add_cohort(cohort).expect("add");
        for (k, id) in ids.iter().take(8).enumerate() {
            let y = if k == 7 { i(20) } else { i(5) };
            ledger.append(id, 200, 300, published(y)).expect("append");
        }
        let a = ledger
            .assess("ex1", 400, Selector::FirstPublished, half_params())
            .expect("assess");
        let s = a.summary();
        assert_eq!((s.n_members, s.n_resolved, s.covered), (10, 8, 7));
        assert_eq!(s.unresolved.get(&Unresolved::Pending), Some(&2));
        assert_eq!(s.coverage_bound, (r(7, 10), r(9, 10)));
        assert_eq!(s.brier_bound, Some((r(1, 10), r(11, 50))));
        assert_eq!(s.interval_score_bound, Some((i(20), None)));
        assert_eq!(s.n_eff, Some((r(16, 9), i(8))));
        assert!(!s.min_n_met);
        let tail: Vec<&str> = a.render().lines().skip(14).collect();
        assert_eq!(
            tail,
            [
                "n_members 10 n_resolved 8 unresolved pending:2",
                "coverage observed 7/8 whole_cohort [7/10, 9/10]",
                "brier over 10 [1/10, 11/50]",
                "interval_score over 10 [20, no finite bound]",
                "n_eff [16/9, 8]",
                "positive_calibration_claim not_permitted (n_resolved 8 of min_n 35)",
            ]
        );
    }

    /// E474 (W1-DR-08): Total Gold 2024Q1 AISC was first published as 1,479 (`Q1_2024:QTD!G23`) and later shown
    /// as 1,474 (`Q1_2025:QTD!N22`) with no footnote. The two cells are cited with their files' sha256 from
    /// `registries/sources/barrick.toml`; the forecast [1,476, 1,490] is synthetic. It is inside under
    /// FirstPublished and outside under LatestAsOf, both assessments are kept, and the first one re-renders byte
    /// for byte. No issuer value is printed (RISK-31).
    #[test]
    fn test_e474_restatement_is_scored_separately_and_overwrites_nothing() {
        let id = "barrick/total-gold/aisc/2024Q1";
        let member = interval(id, 1_476, 1_490, Some(r(4, 5)));
        let frozen = t("2024-03-31T00:00:00Z");
        let cohort = Cohort::freeze(
            "e474",
            frozen,
            vec![member],
            "as first published",
            vec!["barrick".into()],
        )
        .expect("freeze");
        let mut ledger = CohortLedger::new();
        ledger.add_cohort(cohort).expect("add");
        let first = ledger
            .append(
                id,
                t("2024-05-01T00:00:00Z"),
                t("2024-05-02T00:00:00Z"),
                OutcomeKind::Published {
                    value: i(1_479),
                    vintage: "2024Q1".into(),
                    locator: "Q1_2024:QTD!G23".into(),
                    sha256: "e2e1243ec0549f4c487c773869876e2acade401a0b6cd1141adf4b9296aec840"
                        .into(),
                },
            )
            .expect("first publication");
        let at_2024 = t("2024-06-01T00:00:00Z");
        let before = ledger
            .assess("e474", at_2024, Selector::FirstPublished, half_params())
            .expect("assess 2024");
        ledger
            .append(
                id,
                t("2025-05-01T00:00:00Z"),
                t("2025-05-02T00:00:00Z"),
                OutcomeKind::Restated {
                    value: i(1_474),
                    vintage: "2025Q1".into(),
                    locator: "Q1_2025:QTD!N22".into(),
                    sha256: "ec5158f8b4624643ee83d0099e4f3354dc550729ebb443a2dbfacfc29315998b"
                        .into(),
                    supersedes: first,
                },
            )
            .expect("restatement");
        let at_2025 = t("2025-06-01T00:00:00Z");
        let first_published = ledger
            .assess("e474", at_2025, Selector::FirstPublished, half_params())
            .expect("first published");
        let latest = ledger
            .assess("e474", at_2025, Selector::LatestAsOf, half_params())
            .expect("latest");
        let inside = |a: &Assessment| match a.per_member()[0].1 {
            MemberResult::Resolved {
                inside, event_seq, ..
            } => (inside, event_seq),
            MemberResult::Unresolved(why) => panic!("unresolved: {}", why.tag()),
        };
        assert_eq!(inside(&first_published), (true, 0));
        assert_eq!(inside(&latest), (false, 1));
        assert_eq!(first_published.summary().coverage_bound, (i(1), i(1)));
        assert_eq!(latest.summary().coverage_bound, (i(0), i(0)));
        let again = ledger
            .assess("e474", at_2024, Selector::FirstPublished, half_params())
            .expect("re-assess 2024");
        assert_eq!(again.render(), before.render());
        assert_eq!(again.digest(), before.digest());
        assert_eq!(ledger.assessments().len(), 4);
        assert_eq!(ledger.assessments()[0], before);
        assert_eq!(ledger.events().len(), 2);
        eprintln!(
            "{{\"test\":\"e474\",\"assessments\":4,\"first_published\":\"{}\",\"latest\":\"{}\"}}",
            first_published.digest(),
            latest.digest()
        );
    }

    /// Exhaustive: for cohorts of 1 to 12 members with seeded levels (some stating none) and outcomes (some
    /// pending), the coverage and Brier bounds equal the minimum and maximum over all 2^u ways the u pending
    /// members could still resolve, computed here without the module's code; the interval score's lower end does
    /// too, and its upper end is `None` exactly when a pending member states a probability.
    #[test]
    fn test_bounds_equal_exhaustive_enumeration_up_to_12_members() {
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 0x636f_686f,
            partition: 3,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let mut next = |n: u64| rng.next_u64() % n;
        let (mut cases, mut completions) = (0_u64, 0_u64);
        for n in 1..=12_usize {
            for _ in 0..6 {
                // Per member: its level (k/10, or none), and 0 pending, 1 inside (y = 5), 2 outside (y = 20).
                let spec: Vec<(Option<i128>, u64)> = (0..n)
                    .map(|_| {
                        let k = next(10);
                        ((k > 0).then_some(i128::from(k)), next(3))
                    })
                    .collect();
                let members = spec
                    .iter()
                    .enumerate()
                    .map(|(j, (k, _))| interval(&format!("m{j:02}"), 0, 10, k.map(|k| r(k, 10))))
                    .collect();
                let mut ledger = CohortLedger::new();
                ledger
                    .add_cohort(Cohort::freeze("x", 100, members, "rule", vec![]).expect("freeze"))
                    .expect("add");
                for (j, (_, state)) in spec.iter().enumerate() {
                    if *state > 0 {
                        let y = if *state == 1 { i(5) } else { i(20) };
                        ledger
                            .append(&format!("m{j:02}"), 200, 300, published(y))
                            .expect("append");
                    }
                }
                let s = ledger
                    .assess("x", 400, Selector::FirstPublished, PARAMS)
                    .expect("assess")
                    .summary()
                    .clone();
                let pending: Vec<usize> = (0..n).filter(|&j| spec[j].1 == 0).collect();
                let leveled = spec.iter().filter(|(k, _)| k.is_some()).count();
                let (mut cov, mut bri, mut isl): (Vec<Rat>, Vec<Rat>, Vec<Rat>) =
                    (vec![], vec![], vec![]);
                for mask in 0..(1_u32 << pending.len()) {
                    let inside: Vec<bool> = (0..n)
                        .map(|j| match pending.iter().position(|&p| p == j) {
                            Some(b) => mask & (1 << b) != 0,
                            None => spec[j].1 == 1,
                        })
                        .collect();
                    let hits = inside.iter().filter(|&&x| x).count() as i128;
                    cov.push(r(hits, n as i128));
                    let (mut b_sum, mut is_sum) = (i(0), i(0));
                    for (j, (k, _)) in spec.iter().enumerate() {
                        if let Some(k) = k {
                            let miss = if inside[j] { r(10 - k, 10) } else { r(*k, 10) };
                            b_sum = b_sum
                                .checked_add(miss.checked_mul(miss).expect("sq"))
                                .expect("sum");
                            // Width 10, plus (2/α)·10 for an outcome at 20, with α = 1 − k/10.
                            let is = if inside[j] {
                                i(10)
                            } else {
                                i(10).checked_add(r(200, 10 - k)).expect("is")
                            };
                            is_sum = is_sum.checked_add(is).expect("sum");
                        }
                    }
                    if leveled > 0 {
                        bri.push(b_sum.checked_div(i(leveled as i128)).expect("mean"));
                        isl.push(is_sum.checked_div(i(leveled as i128)).expect("mean"));
                    }
                    completions += 1;
                }
                let min = |v: &[Rat]| {
                    *v.iter()
                        .min_by(|a, b| a.checked_cmp(**b).expect("cmp"))
                        .expect("min")
                };
                let max = |v: &[Rat]| {
                    *v.iter()
                        .max_by(|a, b| a.checked_cmp(**b).expect("cmp"))
                        .expect("max")
                };
                assert_eq!(s.n_members, n as u64);
                assert_eq!(
                    s.coverage_bound,
                    (min(&cov), max(&cov)),
                    "n {n} spec {spec:?}"
                );
                if leveled == 0 {
                    assert_eq!((s.brier_bound, s.interval_score_bound), (None, None));
                } else {
                    assert_eq!(
                        s.brier_bound,
                        Some((min(&bri), max(&bri))),
                        "n {n} spec {spec:?}"
                    );
                    let (lo, hi) = s.interval_score_bound.expect("interval score");
                    assert_eq!(lo, min(&isl), "n {n} spec {spec:?}");
                    let pending_leveled = pending.iter().any(|&j| spec[j].0.is_some());
                    assert_eq!(
                        hi,
                        (!pending_leveled).then(|| max(&isl)),
                        "n {n} spec {spec:?}"
                    );
                }
                cases += 1;
            }
        }
        eprintln!(
            "{{\"test\":\"exhaustive_bounds\",\"cases\":{cases},\"completions\":{completions}}}"
        );
        assert_eq!(cases, 72);
    }

    /// Withholding any outcome leaves `n_members` unchanged and the whole-cohort lower bound exactly where an
    /// observed miss would put it, so leaving out an extreme member can never lift a model's pessimistic score.
    #[test]
    fn test_withholding_an_outcome_never_shrinks_the_denominator_or_lifts_the_lower_bound() {
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 0x636f_686f,
            partition: 4,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let mut checked = 0;
        for _ in 0..200 {
            let n = 1 + (rng.next_u64() % 12) as usize;
            let hits: Vec<bool> = (0..n).map(|_| rng.next_u64().is_multiple_of(2)).collect();
            let withheld = (rng.next_u64() % n as u64) as usize;
            let assess = |skip: Option<usize>, miss_at: Option<usize>| {
                let members = (0..n)
                    .map(|j| interval(&format!("m{j:02}"), 0, 10, Some(r(4, 5))))
                    .collect();
                let mut ledger = CohortLedger::new();
                ledger
                    .add_cohort(Cohort::freeze("w", 100, members, "rule", vec![]).expect("freeze"))
                    .expect("add");
                for (j, hit) in hits.iter().enumerate() {
                    if skip == Some(j) {
                        let missing = OutcomeKind::Missing {
                            reason: MissingReason::Withdrawn,
                        };
                        ledger
                            .append(&format!("m{j:02}"), 200, 300, missing)
                            .expect("missing");
                        continue;
                    }
                    let y = if *hit && miss_at != Some(j) {
                        i(5)
                    } else {
                        i(20)
                    };
                    ledger
                        .append(&format!("m{j:02}"), 200, 300, published(y))
                        .expect("append");
                }
                ledger
                    .assess("w", 400, Selector::LatestAsOf, PARAMS)
                    .expect("assess")
                    .summary()
                    .clone()
            };
            let full = assess(None, None);
            let without = assess(Some(withheld), None);
            let as_miss = assess(None, Some(withheld));
            assert_eq!(without.n_members, full.n_members);
            assert_eq!(without.n_resolved + 1, full.n_resolved);
            assert_eq!(without.coverage_bound.0, as_miss.coverage_bound.0);
            let not_above = |a: Rat, b: Rat| a.checked_cmp(b).expect("cmp") != Ordering::Greater;
            assert!(not_above(without.coverage_bound.0, full.coverage_bound.0));
            assert!(not_above(full.coverage_bound.1, without.coverage_bound.1));
            checked += 1;
        }
        eprintln!("{{\"test\":\"withholding\",\"cases\":{checked}}}");
    }

    #[test]
    fn test_payloads_give_their_scored_events_and_malformed_ones_refuse() {
        let q = ForecastPayload::Quantiles {
            levels: vec![r(1, 10), r(1, 2), r(9, 10)],
            values: vec![i(3), i(5), i(8)],
        };
        assert_eq!(
            q.scored_event().expect("central"),
            ScoredEvent {
                lo: i(3),
                hi: i(8),
                level: Some(r(4, 5))
            }
        );
        let g = ForecastPayload::Gaussian {
            mean: i(10),
            sd: i(2),
        };
        let z2 = r(25_631_031_310_892_008, 10_000_000_000_000_000);
        let e = g.scored_event().expect("gaussian");
        assert_eq!(
            (e.lo, e.hi),
            (
                i(10).checked_sub(z2).expect("lo"),
                i(10).checked_add(z2).expect("hi")
            )
        );
        let refused = [
            ForecastPayload::Gaussian {
                mean: i(1),
                sd: i(0),
            },
            ForecastPayload::Interval {
                lo: i(2),
                hi: i(1),
                nominal: None,
            },
            ForecastPayload::Interval {
                lo: i(1),
                hi: i(2),
                nominal: Some(i(1)),
            },
            ForecastPayload::Quantiles {
                levels: vec![r(1, 10), r(4, 5)],
                values: vec![i(1), i(2)],
            },
            ForecastPayload::Quantiles {
                levels: vec![r(1, 2), r(1, 2)],
                values: vec![i(1), i(2)],
            },
            ForecastPayload::Quantiles {
                levels: vec![r(1, 10), r(9, 10)],
                values: vec![i(2), i(1)],
            },
            ForecastPayload::Quantiles {
                levels: vec![r(1, 10)],
                values: vec![i(2)],
            },
        ];
        for p in refused {
            assert_eq!(
                code(p.scored_event().expect_err("refused")),
                ("FMINE-REF-109", 3),
                "{p:?}"
            );
        }
    }

    #[test]
    fn test_cohort_and_ledger_refusals_by_condition() {
        let one = |id: &str| vec![interval(id, 0, 10, Some(r(4, 5)))];
        assert_eq!(
            code(Cohort::freeze("e", 100, vec![], "r", vec![]).expect_err("empty")),
            ("FMINE-REF-109", 3)
        );
        let twice = [one("a"), one("a")].concat();
        assert_eq!(
            code(Cohort::freeze("d", 100, twice, "r", vec![]).expect_err("twice")),
            ("FMINE-REF-507", 6)
        );
        let mut late = one("a");
        late[0].registered_at = 101;
        assert_eq!(
            code(Cohort::freeze("l", 100, late, "r", vec![]).expect_err("late")),
            ("FMINE-REF-206", 3)
        );

        let mut ledger = CohortLedger::new();
        let c = Cohort::freeze("c", 100, one("a"), "r", vec![]).expect("freeze");
        ledger.add_cohort(c.clone()).expect("add");
        ledger
            .add_cohort(c)
            .expect("the same cohort again changes nothing");
        let edited =
            Cohort::freeze("c", 100, [one("a"), one("b")].concat(), "r", vec![]).expect("freeze");
        assert_eq!(
            code(ledger.add_cohort(edited).expect_err("edited")),
            ("FMINE-REF-507", 6)
        );

        assert_eq!(
            code(
                ledger
                    .append("zz", 200, 300, published(i(1)))
                    .expect_err("no cohort")
            ),
            ("FMINE-REF-201", 3)
        );
        assert_eq!(
            code(
                ledger
                    .append("a", 100, 300, published(i(1)))
                    .expect_err("at freeze")
            ),
            ("FMINE-REF-206", 3)
        );
        assert_eq!(
            code(
                ledger
                    .append("a", 200, 199, published(i(1)))
                    .expect_err("early")
            ),
            ("FMINE-REF-109", 3)
        );
        let bad_sha = OutcomeKind::Published {
            value: i(1),
            vintage: "v".into(),
            locator: "l".into(),
            sha256: "abc".into(),
        };
        assert_eq!(
            code(ledger.append("a", 200, 300, bad_sha).expect_err("sha")),
            ("FMINE-REF-109", 3)
        );
        let seq = ledger
            .append("a", 200, 300, published(i(1)))
            .expect("first");
        assert_eq!(
            code(
                ledger
                    .append("a", 200, 300, published(i(2)))
                    .expect_err("second")
            ),
            ("FMINE-REF-507", 6)
        );
        let restate = |supersedes| OutcomeKind::Restated {
            value: i(2),
            vintage: "v2".into(),
            locator: "l2".into(),
            sha256: "cd".repeat(32),
            supersedes,
        };
        assert_eq!(
            code(
                ledger
                    .append("a", 200, 300, restate(9))
                    .expect_err("nothing")
            ),
            ("FMINE-REF-201", 3)
        );
        assert_eq!(
            code(
                ledger
                    .append("a", 200, 299, restate(seq))
                    .expect_err("backdated")
            ),
            ("FMINE-REF-507", 6)
        );
        let later = Cohort::freeze("k", 300, one("b"), "r", vec![]).expect("freeze");
        assert_eq!(
            code(ledger.add_cohort(later).expect_err("frozen too late")),
            ("FMINE-REF-206", 3)
        );

        ledger
            .assess("c", 500, Selector::FirstPublished, PARAMS)
            .expect("assess");
        assert_eq!(
            code(
                ledger
                    .append("a", 400, 500, restate(seq))
                    .expect_err("behind as_of")
            ),
            ("FMINE-REF-507", 6)
        );
        ledger
            .append("a", 400, 501, restate(seq))
            .expect("after the assessment");
        assert_eq!(
            code(
                ledger
                    .assess("nope", 600, Selector::FirstPublished, PARAMS)
                    .expect_err("cohort")
            ),
            ("FMINE-REF-201", 3)
        );
        let bad_rho = AssessParams {
            rho_low: r(1, 2),
            rho_high: r(1, 4),
            min_n: 35,
        };
        assert_eq!(
            code(
                ledger
                    .assess("c", 600, Selector::FirstPublished, bad_rho)
                    .expect_err("rho")
            ),
            ("FMINE-REF-109", 3)
        );
    }

    /// The bead's library e2e: freeze, two vintages, assess, restate, re-assess, then the refused edits. One JSON
    /// line per step. The first assessment's text is written out by hand below: north scores Brier (1/5)² and
    /// interval score 10; south states no probability; west is overdue, so its Brier spans (1/5)² to (4/5)² and
    /// its interval score is at least its width 2.
    #[test]
    fn test_e2e_cohort_lifecycle_logs_each_step() {
        let log = |step: &str, cohort: &str, a: Option<&Assessment>, refusal: Option<&str>| {
            let s = a.map(Assessment::summary);
            eprintln!(
                "{{\"step\":\"{step}\",\"cohort_digest\":\"{cohort}\",\"assessment_digest\":\"{}\",\"n_members\":{},\
                 \"n_resolved\":{},\"n_overdue\":{},\"refusal\":\"{}\"}}",
                a.map_or("", Assessment::digest),
                s.map_or(0, |s| s.n_members),
                s.map_or(0, |s| s.n_resolved),
                s.and_then(|s| s
                    .unresolved
                    .get(&Unresolved::Missing(MissingReason::Overdue))
                    .copied())
                    .unwrap_or(0),
                refusal.unwrap_or("")
            );
        };
        let members = vec![
            interval("north", 10, 20, Some(r(4, 5))),
            interval("south", 30, 40, None),
            CohortMember {
                forecast_id: "west".into(),
                registered_at: 50,
                payload: ForecastPayload::Quantiles {
                    levels: vec![r(1, 10), r(9, 10)],
                    values: vec![i(1), i(3)],
                },
            },
        ];
        let cohort = Cohort::freeze(
            "e2e",
            100,
            members,
            "first publication",
            vec!["synthetic".into()],
        )
        .expect("freeze");
        let digest = cohort.digest().to_string();
        log("freeze", &digest, None, None);
        let mut ledger = CohortLedger::new();
        ledger.add_cohort(cohort).expect("add");
        let north = ledger
            .append("north", 200, 210, published(i(15)))
            .expect("north");
        ledger
            .append("south", 200, 210, published(i(45)))
            .expect("south");
        let overdue = OutcomeKind::Missing {
            reason: MissingReason::Overdue,
        };
        ledger
            .append("west", 250, 260, overdue)
            .expect("west overdue");
        let first = ledger
            .assess("e2e", 300, Selector::FirstPublished, half_params())
            .expect("assess");
        log("assess", &digest, Some(&first), None);
        let expected = format!(
            "assessment v1\ncohort e2e {digest}\nas_of 300 selector first_published\nparams rho 0 1/2 min_n 35\n\
             member north resolved 15 seq 0 inside\nmember south resolved 45 seq 1 outside\n\
             member west unresolved overdue\nn_members 3 n_resolved 2 unresolved overdue:1\n\
             coverage observed 1/2 whole_cohort [1/3, 2/3]\nbrier over 2 [1/25, 17/50]\n\
             interval_score over 2 [6, no finite bound]\nn_eff [4/3, 2]\n\
             positive_calibration_claim not_permitted (n_resolved 2 of min_n 35)\n"
        );
        assert_eq!(first.render(), expected);
        let restated = OutcomeKind::Restated {
            value: i(25),
            vintage: "2027Q1".into(),
            locator: "synthetic:Sheet!B2".into(),
            sha256: "cd".repeat(32),
            supersedes: north,
        };
        ledger.append("north", 400, 410, restated).expect("restate");
        log("restate", &digest, None, None);
        let latest = ledger
            .assess("e2e", 500, Selector::LatestAsOf, half_params())
            .expect("latest");
        log("assess_latest", &digest, Some(&latest), None);
        assert_eq!(latest.summary().covered, 0);
        let again = ledger
            .assess("e2e", 300, Selector::FirstPublished, half_params())
            .expect("re-assess");
        log("reassess", &digest, Some(&again), None);
        assert_eq!(again.digest(), first.digest());
        let edited = Cohort::freeze(
            "e2e",
            100,
            vec![interval("north", 10, 20, None)],
            "first publication",
            vec![],
        )
        .expect("freeze");
        let refusal = ledger.add_cohort(edited).expect_err("edit refused");
        log("edit_membership", &digest, None, Some(refusal.code));
        assert_eq!(refusal.code, "FMINE-REF-507");
        let late = ledger
            .append("west", 50, 600, published(i(2)))
            .expect_err("before the freeze");
        log("outcome_before_freeze", &digest, None, Some(late.code));
        assert_eq!(late.code, "FMINE-REF-206");
        assert_eq!(ledger.assessments().len(), 3);
    }
}
