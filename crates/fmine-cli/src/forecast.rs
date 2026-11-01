//! DESIGN (BD-343, §10.1): read-only replay of supplied forecast records through the calibration kernel.
//! File order preserves its chronology and assessment seals. This adapter neither obtains authority times
//! nor verifies citations or RFC3161 evidence; the separate forecast-record verifier owns external time proof.

use std::collections::BTreeMap;
use std::path::Path;

use fmine_calib::ledger::binding::{
    PublicMetric, PublicOwnership, PublicPeriod, PublicResolutionPolicy, PublicVintagePolicy,
};
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
mod source;

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
    /// Recorded byte-verified admissions, keyed by the immutable outcome event.
    source_admissions: BTreeMap<(ObjectId, u64), live::SourceAdmission>,
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
                "public_issuer",
                "public_metric",
                "public_period",
                "public_ownership",
                "public_members",
                "public_vintages",
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
        let resolution_rule = self.resolution_rule(&row)?;
        let digest = self.ledger.freeze_cohort(CohortSpec {
            name: name.clone(),
            frozen_at_ns: row.assigned_time("frozen_at", observed_ns)?,
            members,
            resolution_rule,
            eligible_sources: row.identifiers("eligible_sources")?,
            policy,
        })?;
        self.cohorts.insert(name, digest);
        Ok(())
    }

    fn resolution_rule(&self, row: &Row<'_>) -> Result<String, Refusal> {
        let rule = row.string("resolution_rule")?;
        let fields = [
            "public_issuer",
            "public_metric",
            "public_period",
            "public_ownership",
            "public_members",
            "public_vintages",
        ];
        if !fields.iter().any(|key| row.0.get(key).is_some()) {
            if rule == "registered-public-cell" {
                return Err(invalid(
                    "registered-public-cell requires all public target fields",
                ));
            }
            return Ok(rule.to_string());
        }
        if rule != "registered-public-cell" {
            return Err(invalid(
                "public target fields require resolution_rule = registered-public-cell",
            ));
        }
        let mut members = BTreeMap::new();
        for entry in row.string("public_members")?.split(',') {
            let (alias, subject) = entry
                .trim()
                .split_once('=')
                .ok_or_else(|| invalid("public_members needs forecast-alias=subject pairs"))?;
            let forecast = self.forecast_id(alias.trim())?;
            if members
                .insert(forecast, subject.trim().to_string())
                .is_some()
            {
                return Err(invalid("duplicate public forecast binding"));
            }
        }
        PublicResolutionPolicy {
            issuer: row.string("public_issuer")?.to_string(),
            metric: PublicMetric::parse(row.string("public_metric")?)?,
            period: PublicPeriod::parse(row.string("public_period")?)?,
            ownership: PublicOwnership::parse(row.string("public_ownership")?)?,
            members,
        }
        .to_resolution_rule_with_vintages(PublicVintagePolicy::parse(
            row.optional_string("public_vintages")?
                .unwrap_or("original_only"),
        )?)
    }

    fn public_policy(
        &self,
        cohort: &str,
        forecast: ObjectId,
    ) -> Result<PublicResolutionPolicy, Refusal> {
        self.public_policy_with_vintages(cohort, forecast)
            .map(|(policy, _)| policy)
    }

    fn public_policy_with_vintages(
        &self,
        cohort: &str,
        forecast: ObjectId,
    ) -> Result<(PublicResolutionPolicy, PublicVintagePolicy), Refusal> {
        let cohort_id = self
            .cohorts
            .get(cohort)
            .ok_or_else(|| absent(format!("unknown cohort {cohort:?}")))?;
        let cohort = self
            .ledger
            .cohort(*cohort_id)
            .ok_or_else(|| absent("frozen cohort missing"))?;
        let (policy, vintages) = PublicResolutionPolicy::parse_resolution_rule_with_vintages(
            &cohort.spec.resolution_rule,
        )?
        .ok_or_else(|| invalid("cohort has no frozen public source-resolution policy"))?;
        if !policy.members.contains_key(&forecast) {
            return Err(invalid(
                "forecast is outside the frozen public source population",
            ));
        }
        Ok((policy, vintages))
    }

    fn public_bound(&self, forecast: ObjectId) -> Result<bool, Refusal> {
        for id in self.cohorts.values() {
            let cohort = self
                .ledger
                .cohort(*id)
                .ok_or_else(|| absent("frozen cohort missing"))?;
            if PublicResolutionPolicy::parse_resolution_rule(&cohort.spec.resolution_rule)?
                .is_some_and(|policy| policy.members.contains_key(&forecast))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn outcome(&mut self, row: Row<'_>) -> Result<(), Refusal> {
        self.outcome_at(row, None)
    }

    fn outcome_at(&mut self, row: Row<'_>, observed_ns: Option<i64>) -> Result<(), Refusal> {
        if self.public_bound(self.forecast_id(row.string("forecast")?)?)? {
            return Err(invalid(
                "this forecast has a frozen public target; use forecast journal resolve, not caller-supplied outcomes",
            ));
        }
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
            None,
        )
    }

    fn render_origin(
        &self,
        report: &Assessment,
        identity: Digest32,
        identity_key: &str,
        mode: &str,
        authority_times: &str,
        checkpoint_count: Option<usize>,
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
        let checkpoints = self.checkpoints.iter().take(checkpoint_count.unwrap_or(self.checkpoints.len())).map(|c| format!(
            "{{\"cohort\":{},\"as_of\":{},\"selector\":{},\"digest\":{},\"expected_digest_matched\":{}}}",
            js(&c.cohort), utc(c.as_of_ns), js(selector_name(c.selector)), js(&c.digest.to_hex()),
            if c.expected.is_some() { "true" } else { "null" },
        )).collect::<Vec<_>>().join(",");
        Ok(format!(
            "{{\"schema\":\"fmine.forecast-assessment.v1\",\"mode\":{},\"lane\":{},{identity_key}:{},\"authority_times\":{},\"source_citations\":{},\"external_time_proof\":\"not_verified\",\"temporal_validation\":{},\"cohort\":{},\"assessment\":{{\"as_of\":{},\"selector\":{},\"digest\":{},\"summary\":{},\"members\":[{}]}},\"checkpoints\":[{}]}}",
            js(mode),
            js(&self.lane),
            js(&identity.to_hex()),
            js(authority_times),
            js(
                if report
                    .per_member
                    .iter()
                    .any(|member| member.selected_event.is_some_and(|(seq, _)| self
                        .source_admissions
                        .contains_key(&(member.forecast_id, seq))))
                {
                    "per_member_admission_evidence"
                } else {
                    "supplied_not_verified"
                }
            ),
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
        let admission = member
            .selected_event
            .and_then(|(seq, _)| self.source_admissions.get(&(member.forecast_id, seq)))
            .map_or_else(|| "null".to_string(), live::SourceAdmission::to_json);
        Ok(format!(
            "{{\"alias\":{},\"forecast_id\":{},\"model_key\":{},\"registered_at\":{},\"outcome_due\":{},\"payload\":{},\"selected_event\":{selected},\"outcome\":{outcome},\"source_admission\":{admission}}}",
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
    let public = PublicResolutionPolicy::parse_resolution_rule_with_vintages(&spec.resolution_rule)
        .ok().flatten().map_or_else(|| "null".into(), |(policy, vintages)| format!(
            "{{\"issuer\":{},\"metric\":{},\"unit\":{},\"period\":{},\"ownership\":{},\"outcome_selector\":{},\"members\":[{}]}}",
            js(&policy.issuer), js(policy.metric.as_str()), js(policy.metric.unit()),
            js(&policy.period.label()), js(&policy.ownership.label()),
            js(match vintages {
                PublicVintagePolicy::OriginalOnly => "original_release_only",
                PublicVintagePolicy::RegisteredComparatives => "original_then_registered_comparatives",
            }),
            policy.members.iter().map(|(id, subject)| format!("{{\"forecast_id\":{},\"subject\":{}}}", js(&id.to_string()), js(subject))).collect::<Vec<_>>().join(","),
        ));
    format!(
        "{{\"name\":{},\"digest\":{},\"frozen_at\":{},\"resolution_rule\":{},\"public_resolution\":{public},\"eligible_sources\":[{}],\"policy\":{{\"alpha\":{},\"min_calibration_n\":{},\"correlation_range\":{},\"outcome_bounds\":{}}}}}",
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
#[path = "../../xtask/testdata/forecast_replay_controls.rs"]
mod tests;
