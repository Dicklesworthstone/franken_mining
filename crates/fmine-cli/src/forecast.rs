//! DESIGN (BD-343, §10.1): read-only replay of supplied forecast records through the calibration kernel.
//! File order preserves its chronology and assessment seals. This adapter neither obtains authority times
//! nor verifies citations or RFC3161 evidence; the separate forecast-record verifier owns external time proof.

use std::collections::BTreeMap;
use std::path::Path;

use fmine_calib::record::{format_utc, parse_hex32, parse_utc};
use fmine_calib::{
    Assessment, AssessmentPolicy, CalibrationFlag, Cohort, CohortSpec, ForecastLedger,
    ForecastPayload, MemberAssessment, MemberOutcome, MissingReason, OutcomeCitation, OutcomeKind,
    OutcomeSelector, ScoreSummary,
};
use fmine_core::hash::{Digest32, sha256};
use fmine_core::id::ObjectId;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_registry::toml_subset::{TomlField, TomlRow, TomlValue, parse_subset};

pub mod live;
mod provenance;
pub mod record_score;
mod record_sources;

const SCHEMA: &str = "fmine.forecast-ledger.v1";

#[derive(Debug)]
pub(super) struct AssessmentFailure {
    pub refusal: Refusal,
    pub diagnostics: Option<String>,
}

impl From<Refusal> for AssessmentFailure {
    fn from(refusal: Refusal) -> Self {
        Self {
            refusal,
            diagnostics: None,
        }
    }
}

pub(super) fn assess(
    path: &Path,
    cohort: &str,
    as_of: &str,
    selector: &str,
    provenance_path: Option<&Path>,
) -> Result<String, AssessmentFailure> {
    let bytes = std::fs::read(path).map_err(|e| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("forecast transcript {}: {e}", path.display()),
        )
    })?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| invalid("forecast transcript must be UTF-8"))?;
    let mut replay = Replay::parse(text)?;
    let report = replay.assessment(cohort, time(as_of)?, selection(selector)?)?;
    if let Some(path) = provenance_path {
        replay.temporal_validation = Some(provenance::verify(path, &replay, &report)?);
    }
    replay.render(&report, sha256(&bytes)).map_err(Into::into)
}

#[derive(Debug)]
struct Checkpoint {
    cohort: String,
    as_of_ns: i64,
    selector: OutcomeSelector,
    digest: Digest32,
    expected: Option<Digest32>,
}

#[derive(Debug, Default)]
struct Replay {
    lane: String,
    ledger: ForecastLedger,
    forecasts: BTreeMap<String, ObjectId>,
    aliases: BTreeMap<ObjectId, String>,
    cohorts: BTreeMap<String, Digest32>,
    checkpoints: Vec<Checkpoint>,
    temporal_validation: Option<String>,
}

impl Replay {
    fn parse(text: &str) -> Result<Self, Refusal> {
        let doc = parse_subset(text).map_err(|e| invalid(e.to_string()))?;
        if doc.top.len() != 2
            || !matches!(doc.top_get("schema").map(|f| &f.value), Some(TomlValue::Str(s)) if s == SCHEMA)
        {
            return Err(invalid(format!(
                "top-level fields must be schema = \"{SCHEMA}\" and lane = \"synthetic\" or \"supplied\""
            )));
        }
        let lane = match doc.top_get("lane").map(|f| &f.value) {
            Some(TomlValue::Str(s)) if s == "synthetic" || s == "supplied" => s.clone(),
            _ => {
                return Err(invalid(
                    "lane must be synthetic or supplied; a caller cannot assert verified public evidence",
                ));
            }
        };
        let mut replay = Self {
            lane,
            ..Self::default()
        };
        for row in &doc.rows {
            // Deliberately do not sort: sorting would turn an attempted backdate into an accepted history.
            let result = match row.table.as_str() {
                "forecast" => replay.forecast(Row(row)),
                "cohort" => replay.cohort(Row(row)),
                "outcome" => replay.outcome(Row(row)),
                "assessment" => replay.checkpoint(Row(row)),
                other => Err(invalid(format!("unknown table {other:?}"))),
            };
            result.map_err(|mut e| {
                e.what = format!("line {} [[{}]]: {}", row.line, row.table, e.what);
                e
            })?;
        }
        Ok(replay)
    }

    fn forecast(&mut self, row: Row<'_>) -> Result<(), Refusal> {
        self.forecast_at(row, None)
    }

    fn forecast_at(&mut self, row: Row<'_>, observed_ns: Option<i64>) -> Result<(), Refusal> {
        let kind = row.string("kind")?;
        let payload_fields: &[&str] = match kind {
            "gaussian" => &["mean", "std_dev"],
            "interval" => &["lo", "hi", "nominal"],
            "quantiles" => &["levels", "values"],
            "bernoulli" => &["probability"],
            _ => return Err(invalid(format!("unknown forecast kind {kind:?}"))),
        };
        row.allowed(
            &["id", "model_key", "registered_at", "outcome_due", "kind"],
            payload_fields,
        )?;
        let alias = identifier(row.string("id")?)?;
        if self.forecasts.contains_key(alias) {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                format!("duplicate forecast alias {alias:?}"),
            ));
        }
        let payload = match kind {
            "gaussian" => ForecastPayload::Gaussian {
                mean: row.number("mean")?,
                std_dev: row.number("std_dev")?,
            },
            "interval" => ForecastPayload::Interval {
                lo: row.number("lo")?,
                hi: row.number("hi")?,
                nominal: row.optional_number("nominal")?,
            },
            "quantiles" => ForecastPayload::Quantiles {
                levels: row.numbers("levels")?,
                values: row.numbers("values")?,
            },
            "bernoulli" => ForecastPayload::Bernoulli {
                probability: row.number("probability")?,
            },
            _ => return Err(invalid("unsupported forecast kind")),
        };
        let id = self.ledger.register_payload(
            row.string("model_key")?,
            row.assigned_time("registered_at", observed_ns)?,
            row.time("outcome_due")?,
            payload,
        )?;
        self.forecasts.insert(alias.to_string(), id);
        self.aliases.insert(id, alias.to_string());
        Ok(())
    }

    fn cohort(&mut self, row: Row<'_>) -> Result<(), Refusal> {
        self.cohort_at(row, None)
    }

    fn cohort_at(&mut self, row: Row<'_>, observed_ns: Option<i64>) -> Result<(), Refusal> {
        row.allowed(
            &[
                "name",
                "frozen_at",
                "members",
                "resolution_rule",
                "eligible_sources",
                "alpha",
                "min_calibration_n",
                "rho_low",
                "rho_high",
                "outcome_lo",
                "outcome_hi",
            ],
            &[],
        )?;
        let mut policy = AssessmentPolicy::default();
        if let Some(alpha) = row.optional_number("alpha")? {
            policy.alpha = alpha;
        }
        if let Some(n) = row.optional_integer("min_calibration_n")? {
            policy.min_calibration_n = usize::try_from(n)
                .map_err(|_| invalid("min_calibration_n must be nonnegative and fit usize"))?;
        }
        policy.correlation_range = (
            row.optional_number("rho_low")?
                .unwrap_or(policy.correlation_range.0),
            row.optional_number("rho_high")?
                .unwrap_or(policy.correlation_range.1),
        );
        policy.outcome_bounds = match (
            row.optional_number("outcome_lo")?,
            row.optional_number("outcome_hi")?,
        ) {
            (Some(lo), Some(hi)) => Some((lo, hi)),
            (None, None) => None,
            _ => {
                return Err(invalid(
                    "outcome_lo and outcome_hi must be supplied together",
                ));
            }
        };
        let name = row.string("name")?.to_string();
        let members = row
            .identifiers("members")?
            .into_iter()
            .map(|alias| self.forecast_id(&alias))
            .collect::<Result<Vec<_>, _>>()?;
        let digest = self.ledger.freeze_cohort(CohortSpec {
            name: name.clone(),
            frozen_at_ns: row.assigned_time("frozen_at", observed_ns)?,
            members,
            resolution_rule: row.string("resolution_rule")?.to_string(),
            eligible_sources: row.identifiers("eligible_sources")?,
            policy,
        })?;
        self.cohorts.insert(name, digest);
        Ok(())
    }

    fn outcome(&mut self, row: Row<'_>) -> Result<(), Refusal> {
        self.outcome_at(row, None)
    }

    fn outcome_at(&mut self, row: Row<'_>, observed_ns: Option<i64>) -> Result<(), Refusal> {
        let kind = row.string("kind")?;
        let extra: &[&str] = match kind {
            "published" => &["value", "source", "vintage", "locator", "sha256"],
            "restated" => &[
                "value",
                "source",
                "vintage",
                "locator",
                "sha256",
                "supersedes",
            ],
            "missing" => &["reason"],
            _ => return Err(invalid(format!("unknown outcome kind {kind:?}"))),
        };
        row.allowed(&["forecast", "available_at", "recorded_at", "kind"], extra)?;
        let value = match kind {
            "missing" => OutcomeKind::Missing {
                reason: missing(row.string("reason")?)?,
            },
            _ => {
                let value = row.number("value")?;
                let citation = OutcomeCitation {
                    source: identifier(row.string("source")?)?.to_string(),
                    vintage: row.string("vintage")?.to_string(),
                    locator: row.string("locator")?.to_string(),
                    sha256: hex(row.string("sha256")?)?.0,
                };
                if kind == "published" {
                    OutcomeKind::Published { value, citation }
                } else {
                    let seq = row
                        .optional_integer("supersedes")?
                        .ok_or_else(|| invalid("restatement needs supersedes"))?;
                    OutcomeKind::Restated {
                        value,
                        citation,
                        supersedes: u64::try_from(seq)
                            .map_err(|_| invalid("supersedes must be a positive integer"))?,
                    }
                }
            }
        };
        self.ledger.append_outcome(
            self.forecast_id(row.string("forecast")?)?,
            row.time("available_at")?,
            row.assigned_time("recorded_at", observed_ns)?,
            value,
        )?;
        Ok(())
    }

    fn checkpoint(&mut self, row: Row<'_>) -> Result<(), Refusal> {
        self.checkpoint_report(row).map(|_| ())
    }

    fn checkpoint_report(&mut self, row: Row<'_>) -> Result<Assessment, Refusal> {
        row.allowed(&["cohort", "as_of", "selector", "expected_digest"], &[])?;
        let cohort = row.string("cohort")?;
        let as_of_ns = row.time("as_of")?;
        let selector = selection(row.string("selector")?)?;
        let expected = row
            .optional_string("expected_digest")?
            .map(hex)
            .transpose()?;
        let report = self.assessment(cohort, as_of_ns, selector)?;
        if expected.is_some_and(|digest| digest != report.digest) {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "assessment checkpoint expected {}, replay produced {}",
                    expected.map_or_else(String::new, |d| d.to_hex()),
                    report.digest
                ),
            ));
        }
        self.checkpoints.push(Checkpoint {
            cohort: cohort.to_string(),
            as_of_ns,
            selector,
            digest: report.digest,
            expected,
        });
        Ok(report)
    }

    fn forecast_id(&self, alias: &str) -> Result<ObjectId, Refusal> {
        self.forecasts
            .get(alias)
            .copied()
            .ok_or_else(|| absent(format!("unknown forecast alias {alias:?}")))
    }

    fn assessment(
        &mut self,
        name: &str,
        as_of_ns: i64,
        selector: OutcomeSelector,
    ) -> Result<Assessment, Refusal> {
        let cohort = self
            .cohorts
            .get(name)
            .copied()
            .ok_or_else(|| absent(format!("unknown cohort {name:?}")))?;
        self.ledger.assess(cohort, as_of_ns, selector).cloned()
    }

    fn render(&self, report: &Assessment, transcript: Digest32) -> Result<String, Refusal> {
        self.render_origin(
            report,
            transcript,
            "transcript_sha256",
            "read_only_replay",
            "supplied_not_verified",
        )
    }

    fn render_origin(
        &self,
        report: &Assessment,
        identity: Digest32,
        identity_key: &str,
        mode: &str,
        authority_times: &str,
    ) -> Result<String, Refusal> {
        let cohort = self
            .ledger
            .cohort(report.cohort)
            .ok_or_else(|| absent("assessment cohort missing"))?;
        let members = report
            .per_member
            .iter()
            .map(|m| self.member_json(m))
            .collect::<Result<Vec<_>, _>>()?
            .join(",");
        let checkpoints = self.checkpoints.iter().map(|c| format!(
            "{{\"cohort\":{},\"as_of\":{},\"selector\":{},\"digest\":{},\"expected_digest_matched\":{}}}",
            js(&c.cohort), utc(c.as_of_ns), js(selector_name(c.selector)), js(&c.digest.to_hex()),
            if c.expected.is_some() { "true" } else { "null" },
        )).collect::<Vec<_>>().join(",");
        Ok(format!(
            "{{\"schema\":\"fmine.forecast-assessment.v1\",\"mode\":{},\"lane\":{},{identity_key}:{},\"authority_times\":{},\"source_citations\":\"supplied_not_verified\",\"external_time_proof\":\"not_verified\",\"temporal_validation\":{},\"cohort\":{},\"assessment\":{{\"as_of\":{},\"selector\":{},\"digest\":{},\"summary\":{},\"members\":[{}]}},\"checkpoints\":[{}]}}",
            js(mode),
            js(&self.lane),
            js(&identity.to_hex()),
            js(authority_times),
            self.temporal_validation
                .as_deref()
                .unwrap_or(provenance::NOT_CHECKED),
            cohort_json(cohort),
            utc(report.as_of_ns),
            js(selector_name(report.selector)),
            js(&report.digest.to_hex()),
            summary_json(report),
            members,
            checkpoints,
            identity_key = js(identity_key),
        ))
    }

    fn member_json(&self, member: &MemberAssessment) -> Result<String, Refusal> {
        let record = self
            .ledger
            .forecast(member.forecast_id)
            .ok_or_else(|| absent("assessment member missing"))?;
        let alias = self
            .aliases
            .get(&member.forecast_id)
            .ok_or_else(|| absent("assessment member alias missing"))?;
        let selected = if let Some((seq, digest)) = member.selected_event {
            let event = self
                .ledger
                .outcomes(member.forecast_id)
                .iter()
                .find(|e| e.seq == seq && e.digest == digest)
                .ok_or_else(|| absent("selected outcome event missing"))?;
            let (kind, supersedes) = match event.kind {
                OutcomeKind::Published { .. } => ("published", "null".to_string()),
                OutcomeKind::Restated { supersedes, .. } => ("restated", supersedes.to_string()),
                OutcomeKind::Missing { .. } => ("missing", "null".to_string()),
            };
            format!(
                "{{\"seq\":{seq},\"digest\":{},\"previous_digest\":{},\"kind\":{},\"supersedes\":{supersedes},\"available_at\":{},\"recorded_at\":{}}}",
                js(&digest.to_hex()),
                js(&event.previous_digest.to_hex()),
                js(kind),
                utc(event.availability_time_ns),
                utc(event.record_time_ns)
            )
        } else {
            "null".to_string()
        };
        let outcome = match &member.outcome {
            MemberOutcome::Resolved {
                value, citation, ..
            } => format!(
                "{{\"status\":\"resolved\",\"value\":{value},\"citation\":{{\"source\":{},\"vintage\":{},\"locator\":{},\"sha256\":{}}}}}",
                js(&citation.source),
                js(&citation.vintage),
                js(&citation.locator),
                js(&Digest32(citation.sha256).to_hex())
            ),
            MemberOutcome::Unresolved { reason } => format!(
                "{{\"status\":\"unresolved\",\"reason\":{}}}",
                js(missing_name(*reason))
            ),
        };
        Ok(format!(
            "{{\"alias\":{},\"forecast_id\":{},\"model_key\":{},\"registered_at\":{},\"outcome_due\":{},\"payload\":{},\"selected_event\":{selected},\"outcome\":{outcome}}}",
            js(alias),
            js(&record.forecast_id.to_string()),
            js(&record.model_key),
            utc(record.registered_ns),
            utc(record.freeze_time_ns),
            payload_json(&record.payload)
        ))
    }
}

struct Row<'a>(&'a TomlRow);

impl<'a> Row<'a> {
    fn allowed(&self, common: &[&str], extra: &[&str]) -> Result<(), Refusal> {
        for field in &self.0.fields {
            if !common.contains(&field.key.as_str()) && !extra.contains(&field.key.as_str()) {
                return Err(invalid(format!(
                    "line {}: unknown field {:?}",
                    field.line, field.key
                )));
            }
        }
        Ok(())
    }
    fn optional_string(&self, key: &str) -> Result<Option<&'a str>, Refusal> {
        match self.0.get(key) {
            Some(TomlField {
                value: TomlValue::Str(s),
                ..
            }) if !s.trim().is_empty() => Ok(Some(s)),
            Some(_) => Err(invalid(format!("{key} must be a nonempty quoted string"))),
            None => Ok(None),
        }
    }
    fn string(&self, key: &str) -> Result<&'a str, Refusal> {
        self.optional_string(key)?
            .ok_or_else(|| invalid(format!("missing {key}")))
    }
    fn optional_integer(&self, key: &str) -> Result<Option<i64>, Refusal> {
        match self.0.get(key) {
            Some(TomlField {
                value: TomlValue::Int(n),
                ..
            }) => Ok(Some(*n)),
            Some(_) => Err(invalid(format!("{key} must be an integer"))),
            None => Ok(None),
        }
    }
    fn number(&self, key: &str) -> Result<f64, Refusal> {
        number(self.string(key)?)
    }
    fn optional_number(&self, key: &str) -> Result<Option<f64>, Refusal> {
        self.optional_string(key)?.map(number).transpose()
    }
    fn time(&self, key: &str) -> Result<i64, Refusal> {
        time(self.string(key)?)
    }
    fn assigned_time(&self, key: &str, observed_ns: Option<i64>) -> Result<i64, Refusal> {
        match observed_ns {
            Some(_) if self.0.get(key).is_some() => Err(invalid(format!(
                "live journal requests cannot supply {key}; the writer assigns its observed clock time"
            ))),
            Some(now) => Ok(now),
            None => self.time(key),
        }
    }
    fn numbers(&self, key: &str) -> Result<Vec<f64>, Refusal> {
        self.string(key)?
            .split(',')
            .map(|s| number(s.trim()))
            .collect()
    }
    fn identifiers(&self, key: &str) -> Result<Vec<String>, Refusal> {
        self.string(key)?
            .split(',')
            .map(|s| identifier(s.trim()).map(str::to_string))
            .collect()
    }
}

fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, detail)
}
fn absent(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::NoClaimInDomain201, detail)
}
fn identifier(s: &str) -> Result<&str, Refusal> {
    if s.is_empty()
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
    {
        Err(invalid(format!(
            "identifier {s:?} must contain only ASCII letters, digits, _, ., :, or -"
        )))
    } else {
        Ok(s)
    }
}
fn number(s: &str) -> Result<f64, Refusal> {
    s.parse::<f64>()
        .ok()
        .filter(|x| x.is_finite())
        .ok_or_else(|| {
            invalid(format!(
                "{s:?} must be a finite floating-point number in a quoted string"
            ))
        })
}
fn hex(s: &str) -> Result<Digest32, Refusal> {
    parse_hex32(s)
        .map(Digest32)
        .ok_or_else(|| invalid("digest must contain exactly 64 hexadecimal characters"))
}
fn time(s: &str) -> Result<i64, Refusal> {
    parse_utc(s)
        .and_then(|n| n.checked_mul(1_000_000_000))
        .ok_or_else(|| {
            invalid(format!(
                "{s:?} must be YYYY-MM-DDTHH:MM:SSZ within the signed nanosecond range"
            ))
        })
}
fn selection(s: &str) -> Result<OutcomeSelector, Refusal> {
    match s {
        "first-published" => Ok(OutcomeSelector::FirstPublished),
        "latest-as-of" => Ok(OutcomeSelector::LatestAsOf),
        _ => Err(invalid("selector must be first-published or latest-as-of")),
    }
}
fn selector_name(s: OutcomeSelector) -> &'static str {
    match s {
        OutcomeSelector::FirstPublished => "first-published",
        OutcomeSelector::LatestAsOf => "latest-as-of",
    }
}
fn missing(s: &str) -> Result<MissingReason, Refusal> {
    match s {
        "pending" => Ok(MissingReason::Pending),
        "not_published" => Ok(MissingReason::NotPublished),
        "withdrawn" => Ok(MissingReason::Withdrawn),
        "definition_changed" => Ok(MissingReason::DefinitionChanged),
        "divested" => Ok(MissingReason::Divested),
        "overdue" => Ok(MissingReason::Overdue),
        _ => Err(invalid(format!("unknown missing reason {s:?}"))),
    }
}
fn missing_name(s: MissingReason) -> &'static str {
    match s {
        MissingReason::Pending => "pending",
        MissingReason::NotPublished => "not_published",
        MissingReason::Withdrawn => "withdrawn",
        MissingReason::DefinitionChanged => "definition_changed",
        MissingReason::Divested => "divested",
        MissingReason::Overdue => "overdue",
    }
}
fn js(s: &str) -> String {
    format!("\"{}\"", crate::escape_json(s))
}
fn utc(ns: i64) -> String {
    let text = format_utc(ns.div_euclid(1_000_000_000));
    let nanos = ns.rem_euclid(1_000_000_000);
    if nanos == 0 {
        js(&text)
    } else {
        js(&format!("{}.{nanos:09}Z", text.trim_end_matches('Z')))
    }
}
fn optional_number(n: Option<f64>) -> String {
    n.map_or_else(|| "null".to_string(), |v| v.to_string())
}
fn pair((lo, hi): (f64, f64)) -> String {
    format!("[{lo},{hi}]")
}

fn cohort_json(cohort: &Cohort) -> String {
    let spec = &cohort.spec;
    let p = &spec.policy;
    format!(
        "{{\"name\":{},\"digest\":{},\"frozen_at\":{},\"resolution_rule\":{},\"eligible_sources\":[{}],\"policy\":{{\"alpha\":{},\"min_calibration_n\":{},\"correlation_range\":{},\"outcome_bounds\":{}}}}}",
        js(&spec.name),
        js(&cohort.digest.to_hex()),
        utc(spec.frozen_at_ns),
        js(&spec.resolution_rule),
        spec.eligible_sources
            .iter()
            .map(|s| js(s))
            .collect::<Vec<_>>()
            .join(","),
        p.alpha,
        p.min_calibration_n,
        pair(p.correlation_range),
        p.outcome_bounds.map_or_else(|| "null".to_string(), pair)
    )
}

fn payload_json(payload: &ForecastPayload) -> String {
    match payload {
        ForecastPayload::Gaussian { mean, std_dev } => {
            format!("{{\"kind\":\"gaussian\",\"mean\":{mean},\"std_dev\":{std_dev}}}")
        }
        ForecastPayload::Interval { lo, hi, nominal } => format!(
            "{{\"kind\":\"interval\",\"lo\":{lo},\"hi\":{hi},\"nominal\":{}}}",
            optional_number(*nominal)
        ),
        ForecastPayload::Quantiles { levels, values } => {
            let array = |v: &[f64]| v.iter().map(f64::to_string).collect::<Vec<_>>().join(",");
            format!(
                "{{\"kind\":\"quantiles\",\"levels\":[{}],\"values\":[{}]}}",
                array(levels),
                array(values)
            )
        }
        ForecastPayload::Bernoulli { probability } => {
            format!("{{\"kind\":\"bernoulli\",\"probability\":{probability}}}")
        }
    }
}

fn score_json(score: Option<ScoreSummary>) -> String {
    score.map_or_else(|| "null".to_string(), |s| format!(
        "{{\"n_scored\":{},\"observed_mean\":{},\"whole_cohort\":{{\"lower\":{},\"upper\":{},\"upper_unbounded\":{}}}}}",
        s.n_scored, optional_number(s.observed_mean), s.whole_cohort.lower, optional_number(s.whole_cohort.upper), s.whole_cohort.upper.is_none()))
}

fn summary_json(report: &Assessment) -> String {
    let s = &report.summary;
    let flag = match s.flag {
        CalibrationFlag::Calibrated => "calibrated",
        CalibrationFlag::Overconfident => "overconfident",
        CalibrationFlag::Biased => "biased",
        CalibrationFlag::Unscored => "unscored",
        CalibrationFlag::Descriptive => "descriptive",
    };
    let coverage = s.coverage.map_or_else(
        || "null".to_string(),
        |c| {
            format!(
                "{{\"inside\":{},\"observed_fraction\":{},\"whole_cohort\":{}}}",
                c.inside,
                optional_number(c.observed_fraction),
                pair(c.whole_cohort)
            )
        },
    );
    let reasons = s
        .unresolved_by_reason
        .iter()
        .map(|(reason, n)| format!("{}:{n}", js(missing_name(*reason))))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"n_members\":{},\"n_resolved\":{},\"n_unresolved\":{},\"unresolved_by_reason\":{{{reasons}}},\"bounds_kind\":\"whole_cohort_completion_bounds_not_confidence_intervals\",\"coverage\":{coverage},\"crps\":{},\"interval_score\":{},\"brier\":{},\"effective_n\":{{\"lower\":{},\"upper\":{},\"correlation_range\":{}}},\"mean_standardized_bias\":{},\"calibration_flag\":{}}}",
        s.n_members,
        s.n_resolved,
        s.n_unresolved,
        score_json(s.crps),
        score_json(s.interval_score),
        score_json(s.brier),
        s.effective_n.lower,
        s.effective_n.upper,
        pair(s.effective_n.correlation_range),
        optional_number(s.mean_standardized_bias),
        js(flag)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../tests/forecasts/cohort_synthetic.toml");
    const CUT: &str = "2026-10-31T00:00:00Z";
    const EARLY_CUT: &str = "2026-09-30T00:00:00Z";

    fn report(text: &str, cutoff: &str, selector: OutcomeSelector) -> (Replay, Assessment) {
        let mut replay = Replay::parse(text).expect("valid synthetic transcript");
        let report = replay
            .assessment("synthetic-demo", time(cutoff).expect("cut"), selector)
            .expect("assessment");
        (replay, report)
    }

    #[test]
    fn synthetic_restatements_keep_the_frozen_denominator_and_change_the_selected_score() {
        let (replay, first) = report(FIXTURE, CUT, OutcomeSelector::FirstPublished);
        let (_, latest) = report(FIXTURE, CUT, OutcomeSelector::LatestAsOf);
        assert_eq!(first.summary.n_members, 3);
        assert_eq!(first.summary.n_resolved, 2);
        assert_eq!(first.summary.n_unresolved, 1);
        assert_eq!(
            first.summary.unresolved_by_reason[&MissingReason::NotPublished],
            1
        );
        assert_eq!(
            first.summary.coverage.expect("coverage").whole_cohort,
            (2.0 / 3.0, 1.0)
        );
        assert_eq!(
            latest.summary.coverage.expect("coverage").whole_cohort,
            (1.0 / 3.0, 2.0 / 3.0)
        );
        assert_eq!(
            first.summary.interval_score.expect("score").observed_mean,
            Some(17.0)
        );
        assert_eq!(
            latest.summary.interval_score.expect("score").observed_mean,
            Some(27.0)
        );
        assert_eq!(
            first
                .summary
                .interval_score
                .expect("score")
                .whole_cohort
                .lower,
            18.0
        );
        assert_eq!(
            latest
                .summary
                .interval_score
                .expect("score")
                .whole_cohort
                .lower,
            74.0 / 3.0
        );
        assert_eq!(
            latest
                .summary
                .interval_score
                .expect("score")
                .whole_cohort
                .upper,
            None
        );
        assert_eq!(latest.summary.effective_n.lower, 2.0 / 1.5);
        assert_eq!(latest.summary.effective_n.upper, 2.0);
        assert_eq!(latest.summary.flag, CalibrationFlag::Descriptive);
        let id = replay.forecast_id("north-unit-cost").expect("north");
        let value = |a: &Assessment| match a
            .per_member
            .iter()
            .find(|m| m.forecast_id == id)
            .expect("member")
            .outcome
        {
            MemberOutcome::Resolved {
                value, event_seq, ..
            } => (value, event_seq),
            _ => panic!("published member"),
        };
        assert_eq!(value(&first), (1479.0, 1));
        assert_eq!(value(&latest), (1474.0, 2));
        assert_ne!(first.digest, latest.digest);
    }

    #[test]
    fn checkpoints_replay_and_validate_a_stable_historical_digest() {
        let prefix = FIXTURE.rsplit_once("[[outcome]]").expect("restatement").0;
        let (_, before) = report(prefix, EARLY_CUT, OutcomeSelector::FirstPublished);
        let (after_replay, after) = report(FIXTURE, EARLY_CUT, OutcomeSelector::FirstPublished);
        assert_eq!(before.canonical_bytes(), after.canonical_bytes());
        assert_eq!(before.digest, after.digest);
        assert_eq!(after_replay.checkpoints[0].digest, after.digest);
        assert_ne!(sha256(prefix.as_bytes()), sha256(FIXTURE.as_bytes()));
        let pinned = FIXTURE.replace(
            "selector = \"first-published\"",
            &format!(
                "selector = \"first-published\"\nexpected_digest = \"{}\"",
                before.digest
            ),
        );
        let (replay, report) = report(&pinned, CUT, OutcomeSelector::LatestAsOf);
        assert_eq!(replay.checkpoints[0].expected, Some(before.digest));
        assert!(
            replay
                .render(&report, sha256(pinned.as_bytes()))
                .expect("JSON")
                .contains("\"expected_digest_matched\":true")
        );
        let bad = pinned.replace(&before.digest.to_hex(), &"0".repeat(64));
        assert_eq!(
            Replay::parse(&bad).expect_err("altered checkpoint").code,
            "FMINE-REF-505"
        );
    }

    #[test]
    fn assessment_checkpoint_seals_survive_replay_without_sorting_away_backdates() {
        let backdated = FIXTURE
            .replace("2026-10-15T00:00:00Z", "2026-09-20T00:00:00Z")
            .replace("2026-10-15T01:00:00Z", "2026-09-20T01:00:00Z");
        let refusal = Replay::parse(&backdated).expect_err("write into sealed September cut");
        assert_eq!(refusal.code, "FMINE-REF-206");
        assert!(refusal.what.contains("[[outcome]]"));
        assert!(refusal.what.contains("assessed cut"));
        let added_forecast = "\n[[forecast]]\nid = \"hidden\"\nmodel_key = \"hidden/model\"\nregistered_at = \"2026-09-01T00:00:00Z\"\noutcome_due = \"2026-12-31T00:00:00Z\"\nkind = \"bernoulli\"\nprobability = \"0.5\"\n";
        let prefix = FIXTURE.rsplit_once("[[outcome]]").expect("last outcome").0;
        assert_eq!(
            Replay::parse(&format!("{prefix}{added_forecast}"))
                .expect_err("global sealed cut")
                .code,
            "FMINE-REF-206"
        );
    }

    fn single(payload: &str, policy: &str, outcome: &str) -> String {
        format!(
            r#"schema = "fmine.forecast-ledger.v1"
lane = "synthetic"
[[forecast]]
id = "one"
model_key = "synthetic/model"
registered_at = "2026-01-01T00:00:00Z"
outcome_due = "2026-03-31T00:00:00Z"
{payload}
[[cohort]]
name = "synthetic-demo"
frozen_at = "2026-01-02T00:00:00Z"
members = "one"
resolution_rule = "Synthetic exact outcome"
eligible_sources = "synthetic"
{policy}
{outcome}
"#
        )
    }

    fn published(value: &str) -> String {
        format!(
            r#"[[outcome]]
forecast = "one"
kind = "published"
available_at = "2026-04-01T00:00:00Z"
recorded_at = "2026-04-02T00:00:00Z"
value = "{value}"
source = "synthetic"
vintage = "v1"
locator = "Invented!A1"
sha256 = "1111111111111111111111111111111111111111111111111111111111111111"
"#
        )
    }

    #[test]
    fn all_distribution_types_use_the_existing_kernel_and_frozen_policy() {
        let gaussian = single(
            "kind = \"gaussian\"\nmean = \"0\"\nstd_dev = \"1\"",
            "",
            &published("0"),
        );
        let (_, a) = report(&gaussian, CUT, OutcomeSelector::LatestAsOf);
        let crps = a.summary.crps.expect("CRPS").observed_mean.expect("scored");
        assert!((crps - 0.233_694_977_255_109_13).abs() < 1e-12);
        assert_eq!(
            a.summary.coverage.expect("coverage").observed_fraction,
            Some(1.0)
        );
        let quantiles = single(
            "kind = \"quantiles\"\nlevels = \"0.1,0.5,0.9\"\nvalues = \"-1,0,1\"",
            "",
            &published("0"),
        );
        let (replay, a) = report(&quantiles, CUT, OutcomeSelector::LatestAsOf);
        assert_eq!(
            a.summary.interval_score.expect("score").observed_mean,
            Some(2.0)
        );
        assert_eq!(a.summary.crps, None);
        assert!(
            replay
                .render(&a, sha256(quantiles.as_bytes()))
                .expect("render")
                .contains("\"levels\":[0.1,0.5,0.9]")
        );
        let bernoulli = single(
            "kind = \"bernoulli\"\nprobability = \"0.25\"",
            "",
            &published("1"),
        );
        let (_, a) = report(&bernoulli, CUT, OutcomeSelector::LatestAsOf);
        assert_eq!(a.summary.brier.expect("Brier").observed_mean, Some(0.5625));
        assert_eq!(a.summary.coverage, None);
        assert_eq!(a.summary.flag, CalibrationFlag::Descriptive);
        let bounded = single(
            "kind = \"interval\"\nlo = \"-1\"\nhi = \"1\"",
            "outcome_lo = \"-2\"\noutcome_hi = \"2\"",
            "",
        );
        let (_, a) = report(&bounded, CUT, OutcomeSelector::LatestAsOf);
        let score = a.summary.interval_score.expect("bounded score");
        assert_eq!(score.n_scored, 0);
        assert_eq!(score.observed_mean, None);
        assert_eq!(score.whole_cohort.lower, 2.0);
        assert_eq!(score.whole_cohort.upper, Some(12.0));
        assert_eq!(a.summary.flag, CalibrationFlag::Unscored);
    }

    #[test]
    fn unobserved_members_remain_pending_or_overdue_with_bounded_brier() {
        let input = single("kind = \"bernoulli\"\nprobability = \"0.25\"", "", "");
        let (_, pending) = report(&input, "2026-02-01T00:00:00Z", OutcomeSelector::LatestAsOf);
        let (_, overdue) = report(&input, CUT, OutcomeSelector::LatestAsOf);
        assert_eq!(
            pending.summary.unresolved_by_reason[&MissingReason::Pending],
            1
        );
        assert_eq!(
            overdue.summary.unresolved_by_reason[&MissingReason::Overdue],
            1
        );
        assert_eq!(overdue.per_member[0].selected_event, None);
        let score = overdue.summary.brier.expect("Brier bounds");
        assert_eq!(score.whole_cohort.lower, 0.0625);
        assert_eq!(score.whole_cohort.upper, Some(0.5625));
        assert_eq!(score.observed_mean, None);
    }

    #[test]
    fn selection_needs_both_availability_and_record_time() {
        let input = single(
            "kind = \"bernoulli\"\nprobability = \"0.25\"",
            "",
            &published("1"),
        );
        let (_, before) = report(&input, "2026-04-01T12:00:00Z", OutcomeSelector::LatestAsOf);
        let (_, after) = report(&input, "2026-04-02T00:00:00Z", OutcomeSelector::LatestAsOf);
        assert_eq!(before.summary.n_resolved, 0);
        assert_eq!(after.summary.n_resolved, 1);
        assert_ne!(before.digest, after.digest);
    }

    #[test]
    fn strict_import_rejects_ambiguous_or_unsupported_transcripts() {
        let cases = [
            FIXTURE.replace(SCHEMA, "v0"),
            FIXTURE.replace("lane = \"synthetic\"", "lane = \"public\""),
            FIXTURE.replace("lane = \"synthetic\"", ""),
            FIXTURE.replacen("[[forecast]]", "[[forecats]]", 1),
            FIXTURE.replacen("lo = \"1476\"", "lo = 1476", 1),
            FIXTURE.replacen("lo = \"1476\"", "lo = \"NaN\"", 1),
            FIXTURE.replacen("lo = \"1476\"", "lo = \"inf\"", 1),
            FIXTURE.replacen("lo = \"1476\"", "probability = \"0.2\"", 1),
            FIXTURE.replacen("id = \"north-unit-cost\"", "id = \"north,unit-cost\"", 1),
            FIXTURE.replacen("2026-07-01T00:00:00Z", "2026-02-30T00:00:00Z", 1),
            FIXTURE.replacen("2026-07-01T00:00:00Z", "2026-07-01T00:00:00+00:00", 1),
            FIXTURE.replacen("2026-07-01T00:00:00Z", "9999-07-01T00:00:00Z", 1),
            FIXTURE.replace(
                "eligible_sources = \"synthetic:issuer\"",
                "eligible_sources = \"synthetic:issuer,\"",
            ),
            FIXTURE.replace("min_calibration_n = 35", "min_calibration_n = \"35\""),
            FIXTURE.replace("min_calibration_n = 35", "min_calibration_n = -1"),
            FIXTURE.replace("rho_low = \"0\"", "outcome_lo = \"0\""),
            FIXTURE.replace("reason = \"not_published\"", "reason = \"lost\""),
            FIXTURE.replace("supersedes = 1", "supersedes = \"1\""),
            FIXTURE.replace("supersedes = 1", "supersedes = -1"),
            FIXTURE.replace("selector = \"first-published\"", "selector = \"latest\""),
            FIXTURE.replace(
                "sha256 = \"2222222222222222222222222222222222222222222222222222222222222222\"",
                "sha256 = \"short\"",
            ),
        ];
        for input in cases {
            let e = Replay::parse(&input).expect_err("invalid transcript");
            assert_eq!(e.code, "FMINE-REF-107", "{}", e.what);
        }
    }

    #[test]
    fn kernel_refusals_keep_their_original_codes_at_the_io_boundary() {
        let cases = [
            (
                FIXTURE.replacen("id = \"central-unit-cost\"", "id = \"north-unit-cost\"", 1),
                "FMINE-REF-507",
            ),
            (
                FIXTURE.replace(
                    "members = \"north-unit-cost, central-unit-cost, south-unit-cost\"",
                    "members = \"unknown\"",
                ),
                "FMINE-REF-201",
            ),
            (
                FIXTURE.replace(
                    "members = \"north-unit-cost, central-unit-cost, south-unit-cost\"",
                    "members = \"north-unit-cost,north-unit-cost\"",
                ),
                "FMINE-REF-109",
            ),
            (
                FIXTURE.replacen(
                    "source = \"synthetic:issuer\"",
                    "source = \"ineligible\"",
                    1,
                ),
                "FMINE-REF-201",
            ),
            (
                FIXTURE.replace("min_calibration_n = 35", "min_calibration_n = 1"),
                "FMINE-REF-109",
            ),
            (
                FIXTURE.replace("supersedes = 1", "supersedes = 0"),
                "FMINE-REF-507",
            ),
            (
                FIXTURE.replace("supersedes = 1", "supersedes = 2"),
                "FMINE-REF-507",
            ),
            (
                FIXTURE.replace("2026-10-15T00:00:00Z", "2026-10-16T00:00:00Z"),
                "FMINE-REF-206",
            ),
            (
                FIXTURE.replace(
                    "frozen_at = \"2026-07-02T00:00:00Z\"",
                    "frozen_at = \"2026-09-01T00:00:00Z\"",
                ),
                "FMINE-REF-206",
            ),
        ];
        for (input, code) in cases {
            let refusal = Replay::parse(&input).expect_err("kernel rejection");
            assert_eq!(refusal.code, code, "{}", refusal.what);
            assert!(refusal.what.contains("line "));
        }
        let mut replay = Replay::parse(FIXTURE).expect("fixture");
        assert_eq!(
            replay
                .assessment(
                    "absent",
                    time(CUT).expect("time"),
                    OutcomeSelector::FirstPublished
                )
                .expect_err("unknown cohort")
                .code,
            "FMINE-REF-201"
        );
        assert_eq!(
            replay
                .assessment(
                    "synthetic-demo",
                    time("2026-01-01T00:00:00Z").expect("time"),
                    OutcomeSelector::FirstPublished
                )
                .expect_err("before freeze")
                .code,
            "FMINE-REF-206"
        );
    }

    #[test]
    fn file_adapter_is_deterministic_read_only_and_explicit_about_unverified_inputs() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/forecasts/cohort_synthetic.toml");
        let before = std::fs::read(&path).expect("fixture bytes");
        let output =
            assess(&path, "synthetic-demo", CUT, "latest-as-of", None).expect("file assessment");
        assert_eq!(
            output,
            assess(&path, "synthetic-demo", CUT, "latest-as-of", None).expect("repeated")
        );
        assert_eq!(before, std::fs::read(&path).expect("still identical"));
        for expected in [
            "\"lane\":\"synthetic\"",
            "\"mode\":\"read_only_replay\"",
            "\"authority_times\":\"supplied_not_verified\"",
            "\"source_citations\":\"supplied_not_verified\"",
            "\"external_time_proof\":\"not_verified\"",
            "\"temporal_validation\":{\"status\":\"not_checked\"",
            "\"calibration_flag\":\"descriptive\"",
            "\"upper\":null,\"upper_unbounded\":true",
            "\"bounds_kind\":\"whole_cohort_completion_bounds_not_confidence_intervals\"",
            "\"reason\":\"not_published\"",
            "\"kind\":\"restated\",\"supersedes\":1",
        ] {
            assert!(output.contains(expected), "missing {expected}");
        }
        assert!(output.contains(&sha256(&before).to_hex()));
        let supplied = FIXTURE.replace("lane = \"synthetic\"", "lane = \"supplied\"");
        let (replay, a) = report(&supplied, CUT, OutcomeSelector::LatestAsOf);
        assert!(
            replay
                .render(&a, sha256(supplied.as_bytes()))
                .expect("supplied report")
                .contains("\"lane\":\"supplied\"")
        );
        assert_eq!(
            assess(
                &path.with_file_name("does-not-exist.toml"),
                "synthetic-demo",
                CUT,
                "latest-as-of",
                None
            )
            .expect_err("absent file")
            .refusal
            .code,
            "FMINE-REF-506"
        );
        assert_eq!(
            assess(&path, "synthetic-demo", CUT, "latest", None)
                .expect_err("bad selector")
                .refusal
                .code,
            "FMINE-REF-107"
        );
        assert_eq!(
            js("quoted \"locator\"\n\\"),
            "\"quoted \\\"locator\\\"\\n\\\\\""
        );
    }
}
