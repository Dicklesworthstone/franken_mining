//! Explicit prospective admission of a reviewed correction. Replay reconstructs the proposal at this
//! operation's historical prefix, then retains its presentation without fitting against later outcomes.
//! Determinism: DeterministicPerIsa

use std::collections::BTreeMap;
use std::fmt::Write as _;

use fmine_calib::ledger::binding::{
    PublicMetric, PublicOwnership, PublicPeriod, PublicResolutionPolicy, PublicVintagePolicy,
};
use fmine_calib::ledger::correction::CorrectionMethod;
use fmine_calib::{Cohort, ForecastMetadata, ForecastPayload};
use fmine_core::{Digest32, ObjectId};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_registry::toml_subset::parse_subset;

use super::{Proposal, proposal};
use crate::forecast::{Replay, Row, absent, hex, identifier, invalid, js, payload_json};

pub(in crate::forecast) const ROBOT_DOCS_JSON: &str = r#"{"command":"forecast journal register-correction --journal <path> --head <hex> (--assessment <digest> | --assessments <digest,digest,...>) --forecast <raw-alias> [--method multiplicative-log|additive-residual] --id <new-alias> --expected-proposal <digest> --model-key <distinct-model> --author-role <role> --trust-host-clock","operation":"Explicit new forecast registration from the exact reviewed correction, using the same explicit method; the raw forecast remains unchanged","bindings":"All parents must be persisted assessments. Raw target cohorts must exist and agree on exact alpha, source policy and eligible sources. Public issuer, metric, period, ownership, subject and vintage permission are inherited before a corrected cohort is frozen; supplied training cannot create a public-bound correction","time":"Writer-assigned upper-bound registration precedes the unchanged outcome deadline and any earlier inherited plant start","replay":"Default multiplicative-log preserves version eleven bytes and semantics. Explicit additive-residual uses version twelve, with a required exclusive correction_method tag. Both retain canonical proposal/parent/target references, exact corrected Gaussian bits and deadline. Replay verifies the reviewed method and proposal at its historical prefix and caches the original presentation; later outcomes never trigger refitting","scope":"Registration remains uncalibrated, with explicit new model/author labels and no external time proof, source authentication or execution authority"}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::forecast) struct Registration {
    pub id: String,
    pub forecast: String,
    pub assessments: Vec<Digest32>,
    pub series: bool,
    pub method: CorrectionMethod,
    pub expected_proposal: Digest32,
    pub model_key: String,
    pub author_role: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SourceBinding {
    Supplied(String),
    Public {
        issuer: String,
        metric: PublicMetric,
        period: PublicPeriod,
        ownership: PublicOwnership,
        subject: String,
        vintages: PublicVintagePolicy,
    },
}

#[derive(Debug, Clone)]
struct Inherited {
    alpha: f64,
    eligible_sources: Vec<String>,
    source: SourceBinding,
}

impl Inherited {
    fn from_cohort(cohort: &Cohort, id: ObjectId) -> Result<Self, Refusal> {
        let source = match PublicResolutionPolicy::parse_resolution_rule_with_vintages(
            &cohort.spec.resolution_rule,
        )? {
            None => SourceBinding::Supplied(cohort.spec.resolution_rule.clone()),
            Some((policy, vintages)) => SourceBinding::Public {
                subject: policy
                    .members
                    .get(&id)
                    .ok_or_else(|| {
                        absent("correction forecast is absent from the frozen public target")
                    })?
                    .clone(),
                issuer: policy.issuer,
                metric: policy.metric,
                period: policy.period,
                ownership: policy.ownership,
                vintages,
            },
        };
        Ok(Self {
            alpha: cohort.spec.policy.alpha,
            eligible_sources: cohort.spec.eligible_sources.clone(),
            source,
        })
    }

    fn matches(&self, other: &Self) -> bool {
        self.alpha.to_bits() == other.alpha.to_bits()
            && self.eligible_sources == other.eligible_sources
            && self.source == other.source
    }

    fn public_policy(&self, id: ObjectId) -> Option<(PublicResolutionPolicy, PublicVintagePolicy)> {
        let SourceBinding::Public {
            issuer,
            metric,
            period,
            ownership,
            subject,
            vintages,
        } = &self.source
        else {
            return None;
        };
        Some((
            PublicResolutionPolicy {
                issuer: issuer.clone(),
                metric: *metric,
                period: *period,
                ownership: *ownership,
                members: BTreeMap::from([(id, subject.clone())]),
            },
            *vintages,
        ))
    }

    fn json(&self) -> String {
        let sources = self
            .eligible_sources
            .iter()
            .map(|source| js(source))
            .collect::<Vec<_>>()
            .join(",");
        let policy = match &self.source {
            SourceBinding::Supplied(rule) => {
                format!("\"kind\":\"supplied\",\"resolution_rule\":{}", js(rule))
            }
            SourceBinding::Public {
                issuer,
                metric,
                period,
                ownership,
                subject,
                vintages,
            } => format!(
                "\"kind\":\"registered_public\",\"issuer\":{},\"metric\":{},\"unit\":{},\"period\":{},\"ownership\":{},\"subject\":{},\"vintages\":{}",
                js(issuer),
                js(metric.as_str()),
                js(metric.unit()),
                js(&period.label()),
                js(&ownership.label()),
                js(subject),
                js(vintages.as_str()),
            ),
        };
        format!(
            "{{\"alpha\":{},\"eligible_sources\":[{sources}],{policy}}}",
            self.alpha
        )
    }
}

#[derive(Debug, Clone)]
pub(in crate::forecast) struct Admission {
    forecast_id: ObjectId,
    inherited: Inherited,
    compact: String,
    full: String,
    markdown: String,
}

impl Admission {
    pub(in crate::forecast) fn public_bound(&self) -> bool {
        self.inherited_public_policy().is_some()
    }

    pub(in crate::forecast) fn inherited_public_policy(
        &self,
    ) -> Option<(PublicResolutionPolicy, PublicVintagePolicy)> {
        self.inherited.public_policy(self.forecast_id)
    }

    pub(in crate::forecast) fn inherited_alpha(&self) -> f64 {
        self.inherited.alpha
    }

    pub(in crate::forecast) fn compact_json(&self) -> &str {
        &self.compact
    }

    pub(in crate::forecast) fn full_json(&self) -> &str {
        &self.full
    }

    pub(in crate::forecast) fn markdown(&self) -> &str {
        &self.markdown
    }
}

struct Prepared {
    operation: String,
    proposal: Proposal,
    assessments: Vec<Digest32>,
    targets: Vec<Digest32>,
    metadata: ForecastMetadata,
    inherited: Inherited,
    plant_start: Option<i64>,
    proposal_json: String,
    proposal_markdown: String,
}

fn digests(values: &[Digest32]) -> String {
    values
        .iter()
        .map(Digest32::to_hex)
        .collect::<Vec<_>>()
        .join(",")
}

fn digest_array(values: &[Digest32]) -> String {
    values
        .iter()
        .map(|digest| js(&digest.to_hex()))
        .collect::<Vec<_>>()
        .join(",")
}

fn label(value: &str) -> Result<(), Refusal> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > 256
        || value.chars().any(char::is_control)
    {
        return Err(invalid(
            "corrected model and author labels must be nonempty, trimmed and bounded without control characters",
        ));
    }
    Ok(())
}

fn changed(detail: &str) -> Refusal {
    Refusal::new(RefusalCode::DigestMismatch505, detail)
}

fn prepare_inner(state: &mut Replay, request: &Registration) -> Result<Prepared, Refusal> {
    identifier(&request.id)?;
    identifier(&request.forecast)?;
    label(&request.model_key)?;
    label(&request.author_role)?;
    if state.forecasts.contains_key(&request.id) {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            "corrected forecast alias is already registered",
        ));
    }
    for assessment in &request.assessments {
        crate::forecast::live::persisted_report(state, *assessment)?;
    }
    let raw_id = state.forecast_id(&request.forecast)?;
    let proposal = proposal(
        state,
        &request.assessments,
        request.series,
        raw_id,
        request.method,
    )?;
    if proposal.digest() != request.expected_proposal {
        return Err(changed(
            "reviewed correction digest differs from the proposal at this journal prefix",
        ));
    }
    let raw = proposal.raw();
    if request.model_key == raw.model_key {
        return Err(invalid(
            "a corrected forecast requires an explicit model key distinct from its raw model",
        ));
    }
    let mut metadata = raw
        .metadata
        .clone()
        .ok_or_else(|| absent("correction target metadata is unavailable"))?;
    metadata.author_role.clone_from(&request.author_role);
    metadata.validate()?;
    let mut target_cohorts = Vec::new();
    for digest in state.cohorts.values() {
        let cohort = state
            .ledger
            .cohort(*digest)
            .ok_or_else(|| absent("frozen target cohort is unavailable"))?;
        if cohort.spec.members.contains(&raw_id) {
            target_cohorts.push(cohort.clone());
        }
    }
    target_cohorts.sort_by_key(|cohort| cohort.digest);
    let first = target_cohorts
        .first()
        .ok_or_else(|| invalid("correction registration requires a frozen raw target cohort"))?;
    let inherited = Inherited::from_cohort(first, raw_id)?;
    if !request.series && matches!(inherited.source, SourceBinding::Public { .. }) {
        return Err(invalid(
            "public correction registration requires a complete assessment series with verified advancing source supports",
        ));
    }
    if inherited.alpha.to_bits() != proposal.alpha().to_bits() {
        return Err(invalid(
            "corrected forecast must inherit the proposal's exact alpha",
        ));
    }
    for cohort in &target_cohorts {
        if !inherited.matches(&Inherited::from_cohort(cohort, raw_id)?) {
            return Err(invalid(
                "raw target cohorts disagree on alpha, eligible sources or resolution policy",
            ));
        }
    }
    let (assessments, training) = match &proposal {
        Proposal::Single(report) => (vec![report.assessment_digest], vec![report.cohort_digest]),
        Proposal::Series(report) => (
            report
                .parents
                .iter()
                .map(|parent| parent.assessment_digest)
                .collect(),
            report
                .parents
                .iter()
                .map(|parent| parent.cohort.digest)
                .collect(),
        ),
    };
    for digest in training {
        let cohort = state
            .ledger
            .cohort(digest)
            .ok_or_else(|| absent("correction training cohort is unavailable"))?;
        let public = PublicResolutionPolicy::parse_resolution_rule_with_vintages(
            &cohort.spec.resolution_rule,
        )?
        .is_some();
        if public != inherited.public_policy(raw_id).is_some() {
            return Err(invalid(
                "correction registration cannot mix supplied training and public targets, or public training and supplied targets",
            ));
        }
        if let SourceBinding::Supplied(rule) = &inherited.source
            && (cohort.spec.resolution_rule != *rule
                || cohort.spec.eligible_sources != inherited.eligible_sources)
        {
            return Err(invalid(
                "correction training and target need the same supplied resolution rule and eligible sources",
            ));
        }
    }
    let targets = target_cohorts
        .iter()
        .map(|cohort| cohort.digest)
        .collect::<Vec<_>>();
    let ForecastPayload::Gaussian { mean, std_dev } = proposal.corrected() else {
        return Err(invalid(
            "correction registration requires the admitted Gaussian proposal",
        ));
    };
    let plant_start = state.plant_registration_starts.get(&raw_id).copied();
    let mut operation = format!(
        "[[corrected_forecast]]\nid = {}\nraw_forecast = {}\nraw_forecast_id = {}\nselection = {}\nassessments = {}\nproposal_digest = {}\ntarget_cohorts = {}\nmodel_key = {}\ncorrected_author_role = {}\nmean_bits = \"{:016x}\"\nstd_dev_bits = \"{:016x}\"\noutcome_due_ns = {}\n",
        js(&request.id),
        js(&request.forecast),
        js(&raw_id.to_string()),
        js(if request.series { "series" } else { "single" }),
        js(&digests(&assessments)),
        js(&proposal.digest().to_hex()),
        js(&digests(&targets)),
        js(&request.model_key),
        js(&request.author_role),
        mean.to_bits(),
        std_dev.to_bits(),
        raw.freeze_time_ns,
    );
    if let Some(start) = plant_start {
        let _ = writeln!(operation, "registration_before_ns = {start}");
    }
    if request.method == CorrectionMethod::AdditiveResidual {
        operation.push_str("correction_method = \"additive-residual\"\n");
    }
    let proposal_json = proposal.json(state)?;
    let proposal_markdown = proposal.markdown(state);
    Ok(Prepared {
        operation,
        proposal,
        assessments,
        targets,
        metadata,
        inherited,
        plant_start,
        proposal_json,
        proposal_markdown,
    })
}

pub(in crate::forecast) fn prepare(
    state: &mut Replay,
    request: &Registration,
) -> Result<String, Refusal> {
    prepare_inner(state, request).map(|prepared| prepared.operation)
}

pub(in crate::forecast) fn apply(
    state: &mut Replay,
    row: Row<'_>,
    authority_ns: i64,
    additive: bool,
) -> Result<(), Refusal> {
    row.allowed(
        &[
            "id",
            "raw_forecast",
            "raw_forecast_id",
            "selection",
            "assessments",
            "proposal_digest",
            "target_cohorts",
            "model_key",
            "corrected_author_role",
            "mean_bits",
            "std_dev_bits",
            "outcome_due_ns",
            "registration_before_ns",
            "correction_method",
        ],
        &[],
    )?;
    let method = if additive {
        if row.string("correction_method")? != "additive-residual" {
            return Err(invalid(
                "version twelve requires the additive-residual correction method",
            ));
        }
        CorrectionMethod::AdditiveResidual
    } else {
        if row.0.get("correction_method").is_some() {
            return Err(invalid(
                "legacy correction registration cannot declare a new method",
            ));
        }
        CorrectionMethod::MultiplicativeLog
    };
    let request = Registration {
        id: row.string("id")?.into(),
        forecast: row.string("raw_forecast")?.into(),
        assessments: row
            .string("assessments")?
            .split(',')
            .map(hex)
            .collect::<Result<Vec<_>, _>>()?,
        series: match row.string("selection")? {
            "single" => false,
            "series" => true,
            _ => return Err(invalid("unknown correction selection mode")),
        },
        method,
        expected_proposal: hex(row.string("proposal_digest")?)?,
        model_key: row.string("model_key")?.into(),
        author_role: row.string("corrected_author_role")?.into(),
    };
    let prepared = prepare_inner(state, &request)?;
    let expected = parse_subset(&prepared.operation).map_err(|error| invalid(error.to_string()))?;
    let expected = expected
        .rows
        .first()
        .ok_or_else(|| invalid("prepared correction operation is absent"))?;
    if row.0.fields.len() != expected.fields.len()
        || expected.fields.iter().any(|field| {
            row.0
                .get(&field.key)
                .is_none_or(|actual| actual.value != field.value)
        })
    {
        return Err(changed(
            "stored correction payload, deadline or parent/target bindings differ from the exact reviewed proposal",
        ));
    }
    let raw = prepared.proposal.raw();
    if authority_ns >= raw.freeze_time_ns
        || prepared
            .plant_start
            .is_some_and(|start| authority_ns >= start)
    {
        return Err(Refusal::new(
            RefusalCode::LateRegistration206,
            "corrected registration must precede the raw outcome deadline and inherited plant start",
        ));
    }
    let raw_id = raw.forecast_id;
    let due = raw.freeze_time_ns;
    let corrected = prepared.proposal.corrected().clone();
    let id = state.ledger.register_payload_with_metadata(
        &request.model_key,
        authority_ns,
        due,
        corrected.clone(),
        prepared.metadata,
    )?;
    let (adjustment, inflation, unit) = match &prepared.proposal {
        Proposal::Single(report) => (report.adjustment, report.inflation, &report.metadata.unit),
        Proposal::Series(report) => (report.adjustment, report.inflation, &report.metadata.unit),
    };
    let version = super::version(method);
    let adjustment = match method {
        CorrectionMethod::MultiplicativeLog => format!("\"bias_factor\":{adjustment}"),
        CorrectionMethod::AdditiveResidual => format!(
            "\"method\":{},\"bias_shift\":{adjustment},\"adjustment_unit\":{}",
            js(if request.series { method.series_name() } else { method.single_name() }),
            js(unit)
        ),
    };
    let compact = format!(
        "{{\"schema\":\"fmine.forecast-correction-admission.v{version}\",\"status\":\"registered_corrected_forecast\",\"uncalibrated\":true,\"forecast\":{},\"forecast_id\":{},\"raw_forecast\":{},\"raw_forecast_id\":{},\"proposal_digest\":{},\"selection\":{},\"assessment_digests\":[{}],\"raw_target_cohorts\":[{}],\"model_key\":{},\"author_role\":{},\"registered_ns\":{authority_ns},\"outcome_due_ns\":{due},\"registration_before_ns\":{},\"raw\":{},{adjustment},\"inflation\":{inflation},\"corrected\":{},\"inherited_source_policy\":{}}}",
        js(&request.id),
        js(&id.to_string()),
        js(&request.forecast),
        js(&raw_id.to_string()),
        js(&prepared.proposal.digest().to_hex()),
        js(if request.series { "series" } else { "single" }),
        digest_array(&prepared.assessments),
        digest_array(&prepared.targets),
        js(&request.model_key),
        js(&request.author_role),
        prepared
            .plant_start
            .map_or_else(|| "null".into(), |start| start.to_string()),
        payload_json(&raw.payload),
        payload_json(&corrected),
        prepared.inherited.json(),
    );
    let full = format!(
        "{},\"proposal\":{}}}",
        compact
            .strip_suffix('}')
            .ok_or_else(|| invalid("prepared correction admission is not a JSON object"))?,
        prepared.proposal_json,
    );
    let text = crate::guidance::text;
    let markdown = format!(
        "# Registered corrected forecast\n\n**Uncalibrated.** This is a new prospectively registered forecast; the raw forecast is unchanged. The retained proposal below is the exact review artifact captured at admission, not a new fit or a calibration claim.\n\nCorrected alias: {} (`{id}`). Model: {}. Author role: {}.\n\nRaw alias: {} (`{raw_id}`). Reviewed proposal: `{}`.\n\nRegistered at nanoseconds {authority_ns}; unchanged outcome deadline {due}; inherited earlier plant start: {}.\n\nInherited source policy:\n\n```json\n{}\n```\n\n{}",
        text(&request.id),
        text(&request.model_key),
        text(&request.author_role),
        text(&request.forecast),
        prepared.proposal.digest(),
        prepared
            .plant_start
            .map_or_else(|| "none".into(), |start| start.to_string()),
        prepared.inherited.json(),
        prepared.proposal_markdown,
    );
    state.forecasts.insert(request.id.clone(), id);
    state.aliases.insert(id, request.id);
    if let Some(start) = prepared.plant_start {
        state.plant_registration_starts.insert(id, start);
    }
    state.correction_admissions.insert(
        id,
        Admission {
            forecast_id: id,
            inherited: prepared.inherited,
            compact,
            full,
            markdown,
        },
    );
    Ok(())
}

/// Every later corrected cohort retains the original raw target's interpretation. This check runs
/// after a staged freeze but before the operation can append; refusal discards the staged replay state.
pub(in crate::forecast) fn validate_cohort(state: &Replay, name: &str) -> Result<(), Refusal> {
    let digest = state
        .cohorts
        .get(name)
        .ok_or_else(|| absent("corrected cohort is unavailable"))?;
    let cohort = state
        .ledger
        .cohort(*digest)
        .ok_or_else(|| absent("corrected cohort is unavailable"))?;
    for id in &cohort.spec.members {
        if let Some(admission) = state.correction_admissions.get(id) {
            let inherited = Inherited::from_cohort(cohort, *id)?;
            if inherited.alpha.to_bits() != admission.inherited_alpha().to_bits()
                || !admission.inherited.matches(&inherited)
            {
                return Err(invalid(
                    "corrected cohorts must retain the raw target's exact alpha, eligible sources and resolution/public policy",
                ));
            }
        }
    }
    Ok(())
}
