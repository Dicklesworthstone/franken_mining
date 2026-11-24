//! DESIGN (BD-297): declared permit-clock review and same-state capital admission.
//! Determinism: Exact

use fmine_time::CivilDate;

use super::*;
use crate::render::{js, rq};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeemedRule {
    None,
    ConfirmationRequired,
}

impl DeemedRule {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ConfirmationRequired => "confirmation_required",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "none" => Ok(Self::None),
            "confirmation_required" => Ok(Self::ConfirmationRequired),
            _ => Err(invalid("deemed_rule must be none or confirmation_required")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Pending,
    WrittenGrant,
    WrittenRefusal,
    DeemingConfirmed,
}

impl Disposition {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::WrittenGrant => "written_grant",
            Self::WrittenRefusal => "written_refusal",
            Self::DeemingConfirmed => "deeming_confirmed",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "written_grant" => Ok(Self::WrittenGrant),
            "written_refusal" => Ok(Self::WrittenRefusal),
            "deeming_confirmed" => Ok(Self::DeemingConfirmed),
            _ => Err(invalid(
                "permit status requires pending, written_grant, written_refusal or deeming_confirmed",
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terms {
    pub authority: String,
    pub filed_on: String,
    pub clock_start: String,
    pub clock_days: u64,
    pub deemed_rule: DeemedRule,
    pub reopen_at_pct: Rat,
    pub change_basis: String,
    pub source: String,
    pub known_on: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub disposition: Disposition,
    pub effective_on: String,
    pub scope_change_pct: Rat,
    pub change_basis: String,
    pub source: String,
    pub known_on: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pause {
    pub from: String,
    pub resumed_on: Option<String>,
    pub source: String,
    pub known_on: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateState {
    Running,
    Paused,
    Elapsed,
    PermissionDocumented,
    Refused,
    RequiresReopenReview,
}

impl GateState {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "clock_running",
            Self::Paused => "clock_paused",
            Self::Elapsed => "clock_elapsed",
            Self::PermissionDocumented => "permission_documented",
            Self::Refused => "refused",
            Self::RequiresReopenReview => "requires_reopen_review",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub state: GateState,
    pub clock_deadline: Option<String>,
    pub paused_days: i64,
    pub scope_reopens: bool,
    pub deeming_review_due: bool,
}

fn invalid(why: &str) -> Refusal {
    bad(RefusalCode::BasisMismatch105, why)
}

fn day(s: &str) -> Result<i64> {
    let err = || invalid("permit dates require a valid calendar date YYYY-MM-DD");
    if !fmine_registry::sources::is_date(s) {
        return Err(err());
    }
    CivilDate::new(
        s[..4].parse().map_err(|_| err())?,
        s[5..7].parse().map_err(|_| err())?,
        s[8..].parse().map_err(|_| err())?,
    )
    .map(CivilDate::to_epoch_days)
    .map_err(|_| err())
}

fn deadline(value: i128) -> Result<i64> {
    if value > i128::from(day("9999-12-31")?) {
        return Err(invalid("clock deadline exceeds the worksheet calendar"));
    }
    i64::try_from(value).map_err(|_| invalid("clock deadline is unrepresentable"))
}

fn date_label(epoch_day: i64) -> String {
    let d = CivilDate::from_epoch_days(epoch_day);
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

fn clock(
    terms: &Terms,
    today: i64,
    pauses: &[Pause],
    resolved: Option<i64>,
) -> Result<(Option<i64>, i64)> {
    let start = day(&terms.clock_start)?;
    let mut periods = Vec::with_capacity(pauses.len());
    for pause in pauses {
        let from = day(&pause.from)?;
        let known = day(&pause.known_on)?;
        let end = pause.resumed_on.as_deref().map(day).transpose()?;
        if !named(&pause.source)
            || known > today
            || from > known
            || end.is_some_and(|d| d <= from || d > known)
        {
            return Err(invalid(
                "pauses require available sources and observed, correctly ordered dates",
            ));
        }
        if resolved.is_some_and(|d| from >= d || end.is_some_and(|resume| resume > d)) {
            return Err(invalid(
                "the statutory clock cannot pause or restart after disposition",
            ));
        }
        periods.push((from, end));
    }
    periods.sort_unstable();
    let mut boundary = deadline(i128::from(start) + i128::from(terms.clock_days))?;
    let mut previous_end = start;
    let mut paused_days = 0;
    for (index, (from, end)) in periods.iter().copied().enumerate() {
        if from < previous_end || from >= boundary {
            return Err(invalid(
                "pauses require a running, unexpired clock and cannot overlap",
            ));
        }
        let Some(resume) = end else {
            if index + 1 != periods.len() {
                return Err(invalid("an open pause must be the last pause"));
            }
            return Ok((None, paused_days + resolved.unwrap_or(today) - from));
        };
        let paused = resume - from;
        paused_days += paused;
        boundary = deadline(i128::from(boundary) + i128::from(paused))?;
        previous_end = resume;
    }
    Ok((Some(boundary), paused_days))
}

/// Evaluate the declared terms at the given decision cutoff. Permission always requires a disposition record.
pub fn review(terms: &Terms, record: &Record, pauses: &[Pause], as_of: &str) -> Result<Review> {
    let today = day(as_of)?;
    let filed = day(&terms.filed_on)?;
    let start = day(&terms.clock_start)?;
    let rules_known = day(&terms.known_on)?;
    let effective = day(&record.effective_on)?;
    let status_known = day(&record.known_on)?;
    if !named(&terms.authority)
        || !named(&terms.source)
        || !named(&record.source)
        || !named(&terms.change_basis)
        || terms.change_basis != record.change_basis
    {
        return Err(invalid(
            "permit terms and status require dated sources, an authority and matching scope-change basis",
        ));
    }
    if !(filed <= start
        && start <= rules_known
        && rules_known <= today
        && start <= effective
        && effective <= status_known
        && status_known <= today)
    {
        return Err(invalid(
            "require filing before clock start, effective status by the source date, and all sources available at the decision cutoff",
        ));
    }
    if terms.clock_days == 0
        || terms.reopen_at_pct.signum() <= 0
        || record.scope_change_pct.signum() < 0
    {
        return Err(invalid(
            "clock days and reopening threshold must be positive and scope change nonnegative",
        ));
    }
    let resolved = (record.disposition != Disposition::Pending).then_some(effective);
    let (clock_deadline, paused_days) = clock(terms, today, pauses, resolved)?;
    if record.disposition == Disposition::DeemingConfirmed
        && (terms.deemed_rule != DeemedRule::ConfirmationRequired
            || clock_deadline.is_none_or(|d| effective < d))
    {
        return Err(invalid(
            "confirmed deeming requires the declared rule and an effective date at or after the running clock's deadline",
        ));
    }
    let scope_reopens =
        !arithmetic(record.scope_change_pct.checked_cmp(terms.reopen_at_pct))?.is_lt();
    let state = match record.disposition {
        Disposition::WrittenRefusal => GateState::Refused,
        _ if scope_reopens => GateState::RequiresReopenReview,
        Disposition::WrittenGrant | Disposition::DeemingConfirmed => {
            GateState::PermissionDocumented
        }
        Disposition::Pending if clock_deadline.is_none() => GateState::Paused,
        Disposition::Pending if clock_deadline.is_some_and(|d| today >= d) => GateState::Elapsed,
        Disposition::Pending => GateState::Running,
    };
    Ok(Review {
        state,
        clock_deadline: clock_deadline.map(date_label),
        paused_days,
        scope_reopens,
        deeming_review_due: state == GateState::Elapsed
            && terms.deemed_rule == DeemedRule::ConfirmationRequired,
    })
}

/// Main-works requirements at the shared second-stage decision date, per scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramPermits {
    pub model_key: String,
    pub as_of: String,
    pub applications: BTreeMap<String, Terms>,
    /// Keys are (application, state).
    pub records: BTreeMap<(String, String), Record>,
    pub pauses: BTreeMap<(String, String), Vec<Pause>>,
    /// Entries are (candidate, application). Every required application must pass.
    pub requirements: BTreeSet<(String, String)>,
}

#[derive(Default)]
pub(super) struct Checked {
    pub reviews: BTreeMap<(String, String), Review>,
    pub rejected: BTreeMap<String, Vec<StructuralRejection>>,
}

impl ProgramPermits {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        crate::render::value_tag("capital-stage-permits/v1", &self.to_json())
    }

    pub(super) fn check(
        &self,
        model_key: &str,
        candidates: &BTreeMap<&str, usize>,
        states: &BTreeSet<&str>,
    ) -> Result<Checked> {
        if self.model_key != model_key
            || self.applications.is_empty()
            || self.applications.len() > MAX_CANDIDATES
            || self.records.len() != self.applications.len() * states.len()
        {
            return Err(invalid(
                "permit conditions require the same model and a complete application-by-state matrix",
            ));
        }
        let mut bound = BTreeSet::new();
        for (candidate, application) in &self.requirements {
            if !candidates.contains_key(candidate.as_str())
                || !self.applications.contains_key(application)
            {
                return Err(invalid(
                    "each stage permit requirement must name a known candidate and application",
                ));
            }
            bound.insert(application.as_str());
        }
        if bound.len() != self.applications.len() || self.applications.keys().any(|id| !key(id)) {
            return Err(invalid(
                "each named permit application requires at least one candidate binding",
            ));
        }
        if self
            .pauses
            .keys()
            .any(|key| !self.records.contains_key(key))
        {
            return Err(invalid(
                "permit pauses require a matching application and state record",
            ));
        }
        let mut checked = Checked::default();
        for (key @ (application, state), record) in &self.records {
            let terms = self
                .applications
                .get(application)
                .filter(|_| states.contains(state.as_str()))
                .ok_or_else(|| {
                    invalid("permit status must name a known application and scenario state")
                })?;
            let pauses = self.pauses.get(key).map_or(&[][..], Vec::as_slice);
            let value = review(terms, record, pauses, &self.as_of)?;
            checked.reviews.insert(key.clone(), value);
        }
        for (candidate, application) in &self.requirements {
            for ((permit, state), review) in &checked.reviews {
                if permit == application && review.state != GateState::PermissionDocumented {
                    checked.rejected.entry(candidate.clone()).or_default().push(
                        StructuralRejection::PermitUnavailable {
                            candidate: candidate.clone(),
                            application: application.clone(),
                            state_id: state.clone(),
                            review: review.clone(),
                        },
                    );
                }
            }
        }
        Ok(checked)
    }

    pub(super) fn to_json(&self) -> String {
        let applications = self.applications.iter().map(|(id, t)| format!(
            r#"{{"application":{},"authority":{},"filed_on":{},"clock_start":{},"clock_days":{},"deemed_rule":{},"reopen_at_pct":{},"change_basis":{},"known_on":{},"source":{}}}"#,
            js(id), js(&t.authority), js(&t.filed_on), js(&t.clock_start), t.clock_days, js(t.deemed_rule.label()), rq(t.reopen_at_pct), js(&t.change_basis), js(&t.known_on), js(&t.source),
        )).collect::<Vec<_>>().join(",");
        let records = self.records.iter().map(|((application, state), r)| format!(
            r#"{{"application":{},"state":{},"status":{},"effective_on":{},"scope_change_pct":{},"change_basis":{},"known_on":{},"source":{}}}"#,
            js(application), js(state), js(r.disposition.label()), js(&r.effective_on), rq(r.scope_change_pct), js(&r.change_basis), js(&r.known_on), js(&r.source),
        )).collect::<Vec<_>>().join(",");
        let mut pauses = Vec::new();
        for ((application, state), rows) in &self.pauses {
            let mut rows: Vec<_> = rows.iter().collect();
            rows.sort_unstable();
            for p in rows {
                pauses.push(format!(
                    r#"{{"application":{},"state":{},"from":{},"resumed_on":{},"known_on":{},"source":{}}}"#,
                    js(application), js(state), js(&p.from), optional_date(&p.resumed_on), js(&p.known_on), js(&p.source),
                ));
            }
        }
        let requirements = self
            .requirements
            .iter()
            .map(|(candidate, application)| {
                format!(
                    r#"{{"candidate":{},"application":{}}}"#,
                    js(candidate),
                    js(application),
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"model_key":{},"as_of":{},"applications":[{applications}],"records":[{records}],"pauses":[{}],"requirements":[{requirements}]}}"#,
            js(&self.model_key),
            js(&self.as_of),
            pauses.join(","),
        )
    }
}

fn optional_date(value: &Option<String>) -> String {
    value.as_deref().map_or_else(|| "null".into(), js)
}

impl Review {
    pub(super) fn to_json(&self) -> String {
        format!(
            r#"{{"status":{},"clock_deadline":{},"paused_days":{},"scope_change_requires_review":{},"deeming_review_due":{}}}"#,
            js(self.state.label()),
            optional_date(&self.clock_deadline),
            self.paused_days,
            self.scope_reopens,
            self.deeming_review_due,
        )
    }
}
