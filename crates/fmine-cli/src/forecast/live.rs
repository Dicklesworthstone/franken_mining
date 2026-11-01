//! Live forecast admission and durable replay (BD-047). The locked I/O owner assigns record times.
//! Caller requests supply targets and source availability, never their own registration or recording time.

mod proof;

use std::fmt::Write as _;
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use fmine_claim::forecast_journal::{Entry, Header, MAX_OPERATION_BYTES, Record};
use fmine_core::hash::Digest32;
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::forecast::Journal;
use fmine_numerics::exact_feas::Rat;
use fmine_registry::toml_subset::{TomlDoc, TomlValue};
use fmine_runtime::{HostWallClock, WallClock};

use super::{
    Assessment, AssessmentFailure, MemberOutcome, Replay, Row, absent, cohort_json, invalid, js,
    missing_name, parse_subset, payload_json, selection, selector_name, time, utc,
};
use crate::{DOCTOR_WHY, Envelope, NextAction};

pub const NO_CLAIM: &str = "Local append-only forecast history under an operator-declared host-clock bound. Journal heads identify local history; they are not signatures or external timestamp proofs. Registered-source admissions retain their byte-verification evidence; replay does not re-read source files. Other supplied outcomes and citations are unverified.";
// Legacy no-top-level operations retain observed-time semantics and their original IDs. The writer's
// explicit v2 marker changes authority times to the clock upper bound without rewriting old records.
const OPERATION_SCHEMA: &str = "fmine.forecast-ledger.v1";
const V2_SCHEMA: &str = "fmine.forecast-operation.v2";
const V3_SCHEMA: &str = "fmine.forecast-operation.v3";

/// An offline receipt for a retained prefix, verified under explicitly selected PEM trust roots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimestampProof {
    pub prefix_head: Digest32,
    pub receipt: PathBuf,
    pub trust_roots: PathBuf,
    pub intermediates: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationVersion {
    LegacyV1,
    V2,
    V3,
}

fn operation_version(doc: &TomlDoc) -> Result<OperationVersion, Refusal> {
    if doc.top.is_empty() {
        Ok(OperationVersion::LegacyV1)
    } else if doc.top.len() == 1
        && matches!(doc.top_get("schema").map(|field| &field.value), Some(TomlValue::Str(schema)) if schema == V2_SCHEMA)
    {
        Ok(OperationVersion::V2)
    } else if doc.top.len() == 1
        && matches!(doc.top_get("schema").map(|field| &field.value), Some(TomlValue::Str(schema)) if schema == V3_SCHEMA)
    {
        Ok(OperationVersion::V3)
    } else {
        Err(invalid("unknown stored forecast operation schema"))
    }
}

/// The writer chooses the version; clients retain the existing no-top-level request grammar.
fn version_request(request: &str, expected_table: &str) -> Result<String, Refusal> {
    let doc = parse_subset(request).map_err(|e| invalid(e.to_string()))?;
    if !doc.top.is_empty() || doc.rows.len() != 1 || doc.rows[0].table != expected_table {
        return Err(invalid(format!(
            "a {expected_table} request needs exactly its one table and no top-level fields"
        )));
    }
    let schema = if expected_table == "registered_outcome"
        || doc.rows[0]
            .fields
            .iter()
            .any(|field| field.key.starts_with("public_"))
    {
        V3_SCHEMA
    } else {
        V2_SCHEMA
    };
    let versioned = format!("schema = \"{schema}\"\n{request}");
    if versioned.len() > MAX_OPERATION_BYTES {
        return Err(invalid(
            "versioned forecast request exceeds the operation byte limit",
        ));
    }
    Ok(versioned)
}

/// Mutating variants require explicit acceptance of the host-clock policy at the command-line boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Init {
        journal: PathBuf,
        cell_id: String,
        clock_skew_ns: u64,
    },
    Register {
        journal: PathBuf,
        head: Digest32,
        request: PathBuf,
    },
    Freeze {
        journal: PathBuf,
        head: Digest32,
        request: PathBuf,
    },
    Outcome {
        journal: PathBuf,
        head: Digest32,
        request: PathBuf,
    },
    /// Resolve the member's frozen public target from its registered original-release workbook.
    Resolve {
        journal: PathBuf,
        head: Digest32,
        cohort: String,
        forecast: String,
    },
    Assess {
        journal: PathBuf,
        head: Digest32,
        cohort: String,
        as_of: String,
        selector: String,
        expected_digest: Option<Digest32>,
    },
    Inspect {
        journal: PathBuf,
        head: Option<Digest32>,
    },
    /// Retrieve an existing persisted assessment without sampling a clock or extending the journal.
    Report {
        journal: PathBuf,
        head: Option<Digest32>,
        assessment: Digest32,
        provenance: Option<PathBuf>,
        timestamp: Option<TimestampProof>,
    },
}

impl Command {
    fn operation(&self) -> &'static str {
        match self {
            Self::Init { .. } => "init",
            Self::Register { .. } => "register",
            Self::Freeze { .. } => "freeze",
            Self::Outcome { .. } => "outcome",
            Self::Resolve { .. } => "resolve",
            Self::Assess { .. } => "assess",
            Self::Inspect { .. } => "inspect",
            Self::Report { .. } => "report",
        }
    }
}

/// Execute with the explicit runtime-owned host clock. Record times never come from request files.
#[must_use]
pub fn execute(command: &Command, root: &Path) -> Envelope {
    execute_with_clock(command, root, &HostWallClock)
}

/// Injected clock capability for deterministic consumers; the caller owns its declared uncertainty policy.
#[must_use]
pub fn execute_with_clock(command: &Command, root: &Path, clock: &dyn WallClock) -> Envelope {
    let verb = format!("forecast journal {}", command.operation());
    let actions = vec![NextAction::new("fmine doctor", DOCTOR_WHY)];
    match perform(command, root, clock) {
        Ok((payload, markdown)) => {
            let mut envelope = Envelope::ok(&verb, payload, actions);
            envelope.no_claim = Some(NO_CLAIM);
            envelope.rendered_markdown = Some(markdown);
            envelope
        }
        Err(failure) => {
            let mut envelope = Envelope::refused(&verb, failure.refusal, actions);
            if let Some(diagnostics) = failure.diagnostics {
                envelope.payload_json = diagnostics;
            }
            envelope
        }
    }
}

fn perform(
    command: &Command,
    root: &Path,
    clock: &dyn WallClock,
) -> Result<(String, String), AssessmentFailure> {
    match command {
        Command::Init {
            journal,
            cell_id,
            clock_skew_ns,
        } => {
            let journal =
                Journal::create(&root.join(journal), cell_id.as_str(), *clock_skew_ns, clock)?;
            return presentation(&journal, &replay(&journal)?, "init", None, false, None)
                .map_err(Into::into);
        }
        Command::Inspect { journal, head } => {
            let journal = Journal::open_read(&root.join(journal), *head)?;
            return presentation(
                &journal,
                &replay(&journal)?,
                "inspect",
                None,
                head.is_some(),
                None,
            )
            .map_err(Into::into);
        }
        Command::Report {
            journal,
            head,
            assessment,
            provenance,
            timestamp,
        } => {
            let journal = Journal::open_read(&root.join(journal), *head)?;
            let mut state = replay(&journal)?;
            let report = persisted_report(&mut state, *assessment)?;
            let verified_timestamp = timestamp
                .as_ref()
                .map(|request| proof::verify(&journal, &state, &report, request, root))
                .transpose()?;
            if let Some(path) = provenance {
                state.temporal_validation = Some(super::provenance::verify(
                    &root.join(path),
                    &state,
                    &report,
                )?);
            }
            return presentation(
                &journal,
                &state,
                "report",
                Some(&report),
                head.is_some(),
                verified_timestamp,
            )
            .map_err(Into::into);
        }
        _ => {}
    }
    let (path, head) = match command {
        Command::Register { journal, head, .. }
        | Command::Freeze { journal, head, .. }
        | Command::Outcome { journal, head, .. }
        | Command::Resolve { journal, head, .. }
        | Command::Assess { journal, head, .. } => (journal, *head),
        _ => return Err(invalid("this operation needs a writer").into()),
    };
    let mut journal = Journal::open_write(&root.join(path), head)?;
    let mut state = replay(&journal)?;
    let (request, table, selected) = match command {
        Command::Register { request, .. } => (read_request(&root.join(request))?, "forecast", None),
        Command::Freeze { request, .. } => (read_request(&root.join(request))?, "cohort", None),
        Command::Outcome { request, .. } => (read_request(&root.join(request))?, "outcome", None),
        Command::Resolve {
            cohort, forecast, ..
        } => {
            let id = state.forecast_id(forecast)?;
            let policy = state.public_policy(cohort, id)?;
            let resolved = super::source::resolve(root, &policy, id)?;
            (
                source_request(&state, cohort, forecast, &resolved)?,
                "registered_outcome",
                None,
            )
        }
        Command::Assess {
            cohort,
            as_of,
            selector,
            expected_digest,
            ..
        } => {
            // Validate before constructing the exact internal operation; this table is persisted and replayed.
            let as_of_ns = time(as_of)?;
            let selected = selection(selector)?;
            let cohort_literal = toml_string(cohort)?;
            // Bind the computed result, not just its query. This state is private to the retained writer
            // lock; the append callback still checks clock bounds before any bytes or receipt are published.
            let digest = state.assessment(cohort, as_of_ns, selected)?.digest;
            if expected_digest.is_some_and(|expected| expected != digest) {
                return Err(Refusal::new(
                    RefusalCode::DigestMismatch505,
                    format!(
                        "assessment checkpoint expected {}, replay produced {digest}",
                        expected_digest.map_or_else(String::new, |value| value.to_hex()),
                    ),
                )
                .into());
            }
            let mut request = format!(
                "[[assessment]]\ncohort = {}\nas_of = {}\nselector = {}\n",
                cohort_literal,
                toml_string(as_of)?,
                toml_string(selector)?,
            );
            let _ = writeln!(request, "expected_digest = {}", js(&digest.to_hex()));
            (request, "assessment", Some(digest))
        }
        _ => return Err(invalid("this operation has no request").into()),
    };
    let request = version_request(&request, table)?;
    let version = operation_version(&parse_subset(&request).map_err(|e| invalid(e.to_string()))?)?;
    let previous = version_counts(journal.records(), &state, selected)?;
    let mut response = None;
    journal.append(&request, clock, |record, header| {
        let assessment = apply(&mut state, record, header, Some(table))?;
        // Public bindings and admitted receipts carry v3; ordinary operations retain v2 semantics.
        let mut versions = previous.versions;
        match version {
            OperationVersion::LegacyV1 => versions.legacy_v1 += 1,
            OperationVersion::V2 => versions.v2 += 1,
            OperationVersion::V3 => versions.v3 += 1,
        }
        let report_origin = match (previous.report_origin, assessment.as_ref()) {
            (Some(origin), _) => Some(origin),
            (None, Some(_)) => Some(ReportOrigin {
                checkpoint_head: record.chain_head()?,
                checkpoint_count: state.checkpoints.len(),
                versions,
            }),
            (None, None) => None,
        };
        // A rendering refusal must precede the durable append. The prepared response is released only
        // after append and sync succeed; a failed append drops this staged state and its response.
        response = Some(presentation_at(
            header,
            record,
            &state,
            command.operation(),
            assessment.as_ref(),
            true,
            PresentationContext {
                versions,
                report_origin,
                timestamp: None,
            },
        )?);
        Ok(())
    })?;
    response
        .ok_or_else(|| absent("synced forecast operation has no prepared response"))
        .map_err(Into::into)
}

fn persisted_report(state: &mut Replay, digest: Digest32) -> Result<Assessment, Refusal> {
    let (cohort, as_of_ns, selector) = state
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.digest == digest)
        .map(|checkpoint| {
            (
                checkpoint.cohort.clone(),
                checkpoint.as_of_ns,
                checkpoint.selector,
            )
        })
        .ok_or_else(|| {
            absent(format!(
                "assessment {digest} was not persisted in this journal"
            ))
        })?;
    // Replay already populated this exact immutable assessment and its seal. This retrieves the cached
    // result; accepting arbitrary cohort/cut inputs here would introduce a new assessment instead.
    let report = state.assessment(&cohort, as_of_ns, selector)?;
    if report.digest != digest {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "persisted assessment {digest} replayed with a different digest {}",
                report.digest
            ),
        ));
    }
    Ok(report)
}

fn digest_is_bound(state: &Replay, digest: Digest32) -> bool {
    state
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.digest == digest)
        .is_some_and(|checkpoint| checkpoint.expected == Some(digest))
}

fn read_request(path: &Path) -> Result<String, Refusal> {
    let failure = |e| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("forecast request {}: {e}", path.display()),
        )
    };
    let metadata = std::fs::metadata(path).map_err(failure)?;
    if !metadata.is_file() || metadata.len() > MAX_OPERATION_BYTES as u64 {
        return Err(invalid(
            "forecast request must be a regular file within the operation byte limit",
        ));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(failure)?
        .take(MAX_OPERATION_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(failure)?;
    if bytes.len() > MAX_OPERATION_BYTES {
        return Err(invalid(
            "forecast request grew past the operation byte limit",
        ));
    }
    String::from_utf8(bytes).map_err(|_| invalid("forecast request must be UTF-8"))
}

/// Evidence recorded at admission. Historical replay verifies this record and its journal hash;
/// it does not assert that the current filesystem still contains the admitted source bytes.
#[derive(Debug)]
pub(super) struct SourceAdmission {
    cohort: Digest32,
    claim: ObjectId,
    exact: Rat,
    displayed: String,
    published_at: String,
    published_at_basis: String,
}

impl SourceAdmission {
    pub(super) fn to_json(&self) -> String {
        format!(
            "{{\"basis\":\"recorded_registered_source_admission\",\"source_bytes_reverified_on_replay\":false,\"cohort_digest\":{},\"source_claim_id\":{},\"exact_value\":{{\"numerator\":{},\"denominator\":{}}},\"displayed\":{},\"published_at\":{},\"published_at_basis\":{}}}",
            js(&self.cohort.to_hex()),
            js(&self.claim.to_string()),
            js(&self.exact.num().to_string()),
            js(&self.exact.den().to_string()),
            js(&self.displayed),
            js(&self.published_at),
            js(&self.published_at_basis),
        )
    }
}

fn bytes_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn claim_body(hex: &str) -> Result<Vec<u8>, Refusal> {
    if hex.is_empty()
        || !hex.len().is_multiple_of(2)
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid(
            "source Claim body needs canonical lowercase hexadecimal bytes",
        ));
    }
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let nibble = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
            Ok(nibble(pair[0]) * 16 + nibble(pair[1]))
        })
        .collect()
}

fn source_request(
    state: &Replay,
    cohort: &str,
    forecast: &str,
    resolved: &super::source::ResolvedPublicOutcome,
) -> Result<String, Refusal> {
    let id = state
        .cohorts
        .get(cohort)
        .ok_or_else(|| absent("frozen cohort missing"))?;
    let anchor = resolved
        .source
        .claim
        .anchors
        .first()
        .ok_or_else(|| absent("source Claim anchor missing"))?;
    let mut request = String::from("[[registered_outcome]]\n");
    for (key, value) in [
        ("forecast", forecast.to_string()),
        ("cohort", cohort.to_string()),
        ("cohort_digest", id.to_hex()),
        ("value", resolved.displayed.clone()),
        ("numerator", resolved.value.num().to_string()),
        ("denominator", resolved.value.den().to_string()),
        ("source", resolved.citation.source.clone()),
        ("vintage", resolved.citation.vintage.clone()),
        ("locator", resolved.citation.locator.clone()),
        ("sha256", Digest32(resolved.citation.sha256).to_hex()),
        ("claim_id", resolved.source.claim.claim_id.to_string()),
        (
            "claim_body",
            bytes_hex(&resolved.source.claim.canonical_body()),
        ),
        ("stored", resolved.stored.clone()),
        ("ownership_source", resolved.ownership_source.to_string()),
        ("anchor_path", anchor.artifact_path.clone()),
        ("published_at", resolved.published_at.clone()),
        ("published_at_basis", resolved.published_at_basis.clone()),
        (
            "available_at",
            fmine_calib::record::format_utc(resolved.available_at_ns / 1_000_000_000),
        ),
    ] {
        let _ = writeln!(request, "{key} = {}", toml_string(&value)?);
    }
    Ok(request)
}

fn apply_source(state: &mut Replay, row: Row<'_>, recorded_ns: i64) -> Result<(), Refusal> {
    row.allowed(
        &[
            "forecast",
            "cohort",
            "cohort_digest",
            "value",
            "numerator",
            "denominator",
            "source",
            "vintage",
            "locator",
            "sha256",
            "claim_id",
            "claim_body",
            "published_at",
            "published_at_basis",
            "available_at",
            "stored",
            "ownership_source",
            "anchor_path",
        ],
        &[],
    )?;
    let forecast = state.forecast_id(row.string("forecast")?)?;
    let cohort = row.string("cohort")?;
    let policy = state.public_policy(cohort, forecast)?;
    let cohort_digest = super::hex(row.string("cohort_digest")?)?;
    if state.cohorts.get(cohort) != Some(&cohort_digest)
        || row.string("source")? != policy.issuer
        || row.string("vintage")? != policy.period.original_release()
    {
        return Err(invalid(
            "source admission differs from the frozen cohort, issuer or original release",
        ));
    }
    let integer = |key| {
        row.string(key)?.parse::<i128>().map_err(|_| {
            invalid("source exact value needs signed integer numerator and denominator")
        })
    };
    let exact = Rat::new(integer("numerator")?, integer("denominator")?)
        .map_err(|_| invalid("invalid exact source value"))?;
    let displayed = row.string("value")?.to_string();
    let fmine_registry::transcriptions::Token::Point(decimal) =
        fmine_registry::transcriptions::parse_token(&displayed).map_err(invalid)?
    else {
        return Err(invalid("registered outcome needs one printed number"));
    };
    if Rat::new(decimal.mantissa, 10_i128.pow(decimal.scale))
        .map_err(|_| invalid("invalid printed value"))?
        != exact
    {
        return Err(invalid(
            "printed source value differs from its exact rational",
        ));
    }
    let published_at = row.string("published_at")?.to_string();
    if !fmine_registry::sources::is_date(&published_at) {
        return Err(invalid("source publication date must be YYYY-MM-DD"));
    }
    let publication_start = time(&format!("{published_at}T00:00:00Z"))?;
    let published_at_basis = row.string("published_at_basis")?;
    if !fmine_registry::sources::PUBLISHED_AT_BASES
        .iter()
        .any(|prefix| {
            published_at_basis
                .strip_prefix(prefix)
                .is_some_and(|detail| !detail.trim().is_empty())
        })
    {
        return Err(invalid(
            "publication basis needs FACT: or INFERENCE: followed by its evidence",
        ));
    }
    if policy.metric == fmine_calib::ledger::binding::PublicMetric::ProductionKoz && exact.num() < 0
    {
        return Err(invalid("production cannot be negative"));
    }
    let available_ns = publication_start
        .checked_add(86_400_000_000_000)
        .ok_or_else(|| invalid("publication availability overflows"))?;
    if available_ns != row.time("available_at")? || publication_start < policy.period.end_ns()? {
        return Err(invalid(
            "source availability must be after the target period and conservatively follow its publication day",
        ));
    }
    let body = claim_body(row.string("claim_body")?)?;
    let claim = row
        .string("claim_id")?
        .parse::<ObjectId>()
        .map_err(|e| invalid(e.to_string()))?;
    if claim != ObjectId::from_canonical_bytes(PrefixKind::Claim, &body) {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            "source Claim body differs from its stored identity",
        ));
    }
    let citation = fmine_calib::OutcomeCitation {
        source: policy.issuer.clone(),
        vintage: row.string("vintage")?.to_string(),
        locator: row.string("locator")?.to_string(),
        sha256: super::hex(row.string("sha256")?)?.0,
    };
    let expected_body = super::source::receipt_claim_body(
        &policy,
        forecast,
        &citation,
        row.string("stored")?,
        &displayed,
        row.string("ownership_source")?,
        row.string("anchor_path")?,
    )?;
    if body != expected_body {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            "source Claim does not bind the frozen target, exact printed value and cited anchor",
        ));
    }
    let value = exact.num() as f64 / exact.den() as f64;
    let event = state.ledger.append_outcome(
        forecast,
        available_ns,
        recorded_ns,
        fmine_calib::OutcomeKind::Published { value, citation },
    )?;
    state.source_admissions.insert(
        (forecast, event),
        SourceAdmission {
            cohort: cohort_digest,
            claim,
            exact,
            displayed,
            published_at,
            published_at_basis: published_at_basis.to_string(),
        },
    );
    Ok(())
}

fn late(message: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::LateRegistration206, message)
}

fn toml_string(value: &str) -> Result<String, Refusal> {
    if value.chars().any(char::is_control) {
        return Err(invalid(
            "live cohort names cannot contain control characters",
        ));
    }
    Ok(format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

fn replay(journal: &Journal) -> Result<Replay, Refusal> {
    let mut state = Replay {
        lane: "supplied".into(),
        ..Replay::default()
    };
    for record in journal.records() {
        if matches!(record.entry, Entry::Operation(_)) {
            apply(&mut state, record, journal.header(), None)?;
        }
    }
    Ok(state)
}

/// Recheck both clock bounds and the existing ledger transitions on every reopen, without a fresh clock.
fn apply(
    state: &mut Replay,
    record: &Record,
    header: &Header,
    expected_table: Option<&str>,
) -> Result<Option<Assessment>, Refusal> {
    if super::SCHEMA != OPERATION_SCHEMA {
        return Err(invalid(
            "FJR1 version 1 requires its frozen forecast-ledger.v1 row grammar",
        ));
    }
    let Entry::Operation(request) = &record.entry else {
        return Err(invalid("only operation records have forecast semantics"));
    };
    let doc = parse_subset(request).map_err(|e| invalid(e.to_string()))?;
    let version = operation_version(&doc)?;
    if doc.rows.len() != 1 {
        return Err(invalid("a stored live request needs exactly one table"));
    }
    let row = &doc.rows[0];
    if expected_table.is_some_and(|expected| row.table != expected) {
        return Err(invalid(format!(
            "{} request cannot contain [[{}]]",
            expected_table.unwrap_or("live"),
            row.table
        )));
    }
    for key in ["registered_at", "frozen_at", "recorded_at"] {
        if row.get(key).is_some() {
            return Err(invalid(format!(
                "live journal requests cannot supply {key}; the stored operation version assigns its clock time"
            )));
        }
    }
    let (lower, upper) = record.time_bounds(header)?;
    let authority_ns = match version {
        OperationVersion::LegacyV1 => record.observed_ns,
        OperationVersion::V2 | OperationVersion::V3 => upper,
    };
    let row = Row(row);
    match row.0.table.as_str() {
        "forecast" => {
            if upper >= row.time("outcome_due")? {
                return Err(late(
                    "registration's latest possible clock time must precede the outcome deadline",
                ));
            }
            state.forecast_at(row, Some(authority_ns))?;
        }
        "cohort" => {
            toml_string(row.string("name")?)?;
            for alias in row.identifiers("members")? {
                let id = state.forecast_id(&alias)?;
                let forecast = state
                    .ledger
                    .forecast(id)
                    .ok_or_else(|| absent("cohort member missing"))?;
                if upper >= forecast.freeze_time_ns {
                    return Err(late(format!(
                        "cohort freeze's latest possible clock time reaches {alias}'s outcome deadline"
                    )));
                }
            }
            state.cohort_at(row, Some(authority_ns))?;
        }
        "outcome" => {
            if row.time("available_at")? > lower {
                return Err(late(
                    "outcome availability exceeds the writer's earliest possible clock time",
                ));
            }
            state.outcome_at(row, Some(authority_ns))?;
        }
        "registered_outcome" => {
            if version != OperationVersion::V3 {
                return Err(invalid(
                    "registered source outcomes require operation version three",
                ));
            }
            if row.time("available_at")? > lower {
                return Err(late(
                    "registered publication availability exceeds the writer's earliest possible clock time",
                ));
            }
            apply_source(state, row, authority_ns)?;
        }
        "assessment" => {
            if version != OperationVersion::LegacyV1 {
                row.string("expected_digest")?;
            }
            if row.time("as_of")? > lower {
                return Err(late(
                    "assessment cut exceeds the writer's earliest possible clock time",
                ));
            }
            return state.checkpoint_report(row).map(Some);
        }
        other => return Err(invalid(format!("unknown live forecast table {other:?}"))),
    }
    Ok(None)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct VersionCounts {
    legacy_v1: usize,
    v2: usize,
    v3: usize,
}

impl VersionCounts {
    fn policy(self) -> &'static str {
        match (self.legacy_v1, self.v2 + self.v3) {
            (0, 0) => "none",
            (_, 0) => "legacy_observed",
            (0, _) => "clock_upper_bound",
            _ => "mixed",
        }
    }

    fn basis(self) -> &'static str {
        match self.policy() {
            "legacy_observed" => "observed_host_clock_with_declared_uncertainty",
            "clock_upper_bound" => "clock_upper_bound_with_declared_uncertainty",
            "mixed" => "mixed_versioned_record_times",
            _ => "no_operations",
        }
    }

    fn fields(self) -> String {
        let v3 = if self.v3 == 0 {
            String::new()
        } else {
            format!(",\"v3\":{}", self.v3)
        };
        format!(
            "\"operation_version_counts\":{{\"legacy_v1\":{},\"v2\":{}{v3}}},\"record_time_policy\":{}",
            self.legacy_v1,
            self.v2,
            js(self.policy())
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct ReportOrigin {
    checkpoint_head: Digest32,
    checkpoint_count: usize,
    versions: VersionCounts,
}

#[derive(Debug, Clone, Copy)]
struct PresentationContext {
    versions: VersionCounts,
    report_origin: Option<ReportOrigin>,
    timestamp: Option<proof::Evidence>,
}

/// Compact origin metadata preserves a report's original head and prefix without caching growing JSON.
fn version_counts(
    records: &[Record],
    state: &Replay,
    selected: Option<Digest32>,
) -> Result<PresentationContext, Refusal> {
    let mut counts = VersionCounts::default();
    let mut report_origin = None;
    let mut checkpoint_index = 0;
    for record in records {
        let Entry::Operation(request) = &record.entry else {
            continue;
        };
        let doc = parse_subset(request).map_err(|e| invalid(e.to_string()))?;
        match operation_version(&doc)? {
            OperationVersion::LegacyV1 => counts.legacy_v1 += 1,
            OperationVersion::V2 => counts.v2 += 1,
            OperationVersion::V3 => counts.v3 += 1,
        }
        if doc
            .rows
            .first()
            .is_some_and(|row| row.table == "assessment")
        {
            let checkpoint = state
                .checkpoints
                .get(checkpoint_index)
                .ok_or_else(|| absent("replayed checkpoint metadata missing"))?;
            if report_origin.is_none() && selected == Some(checkpoint.digest) {
                report_origin = Some(ReportOrigin {
                    checkpoint_head: record.chain_head()?,
                    checkpoint_count: checkpoint_index + 1,
                    versions: counts,
                });
            }
            checkpoint_index += 1;
        }
    }
    Ok(PresentationContext {
        versions: counts,
        report_origin,
        timestamp: None,
    })
}

fn presentation(
    journal: &Journal,
    state: &Replay,
    operation: &str,
    assessment: Option<&Assessment>,
    pinned: bool,
    timestamp: Option<proof::Evidence>,
) -> Result<(String, String), Refusal> {
    let last = journal
        .records()
        .last()
        .ok_or_else(|| absent("journal genesis missing"))?;
    let mut context = version_counts(journal.records(), state, assessment.map(|a| a.digest))?;
    context.timestamp = timestamp;
    presentation_at(
        journal.header(),
        last,
        state,
        operation,
        assessment,
        pinned,
        context,
    )
}

fn presentation_at(
    header: &Header,
    last: &Record,
    state: &Replay,
    operation: &str,
    assessment: Option<&Assessment>,
    pinned: bool,
    context: PresentationContext,
) -> Result<(String, String), Refusal> {
    let (earliest, latest) = last.time_bounds(header)?;
    let head = last.chain_head()?;
    let PresentationContext {
        versions,
        report_origin,
        timestamp,
    } = context;
    let commitment = fmine_io::forecast::prefix::PrefixCommitment::for_record(last)?;
    let timestamp_commitment = proof::commitment_json(commitment);
    let timestamp_verification = timestamp.map_or_else(|| "null".into(), proof::Evidence::json);
    let external_time_proof = if timestamp.is_some() {
        "verified_caller_selected_roots"
    } else {
        "not_verified"
    };
    let outcomes: usize = state
        .forecasts
        .values()
        .map(|id| state.ledger.outcomes(*id).len())
        .sum();
    let forecasts = state.forecasts.iter().map(|(alias, id)| {
        let row = state.ledger.forecast(*id).ok_or_else(|| absent("registered forecast missing"))?;
        Ok(format!("{{\"alias\":{},\"forecast_id\":{},\"model_key\":{},\"registered_ns\":{},\"registered_at\":{},\"outcome_due_ns\":{},\"outcome_due\":{},\"payload\":{}}}",
            js(alias), js(&id.to_string()), js(&row.model_key), row.registered_ns, utc(row.registered_ns), row.freeze_time_ns, utc(row.freeze_time_ns), payload_json(&row.payload)))
    }).collect::<Result<Vec<_>, Refusal>>()?.join(",");
    let cohorts = state
        .cohorts
        .values()
        .map(|id| {
            state
                .ledger
                .cohort(*id)
                .map(cohort_json)
                .ok_or_else(|| absent("frozen cohort missing"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    let checkpoints = state
        .checkpoints
        .iter()
        .map(|checkpoint| {
            format!(
                "{{\"cohort\":{},\"as_of_ns\":{},\"selector\":{},\"digest\":{},\"result_digest_binding\":{}}}",
                js(&checkpoint.cohort),
                checkpoint.as_of_ns,
                js(selector_name(checkpoint.selector)),
                js(&checkpoint.digest.to_hex()),
                js(if checkpoint.expected.is_some() {
                    "persisted_checkpoint"
                } else {
                    "legacy_unbound"
                }),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let source_admissions = state
        .source_admissions
        .iter()
        .map(|((id, seq), admission)| {
            format!(
                "{{\"forecast_id\":{},\"event_seq\":{seq},\"evidence\":{}}}",
                js(&id.to_string()),
                admission.to_json()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let report = assessment
        .map(|a| -> Result<String, Refusal> {
            let origin = report_origin
                .ok_or_else(|| absent("selected assessment has no stored operation prefix"))?;
            let report = state.render_origin(
                a,
                origin.checkpoint_head,
                "journal_head",
                "durable_journal",
                "operator_declared_bounded_host_clock",
                Some(origin.checkpoint_count),
            )?;
            let prefix = report
                .strip_suffix('}')
                .ok_or_else(|| invalid("invalid rendered forecast report"))?;
            Ok(format!("{prefix},{}}}", origin.versions.fields()))
        })
        .transpose()?
        .unwrap_or_else(|| "null".into());
    let retrieval = if operation == "report" {
        let selected = assessment.ok_or_else(|| absent("persisted assessment report missing"))?;
        format!(
            ",\"report_origin\":\"persisted_assessment\",\"selected_assessment_digest\":{},\"result_digest_binding\":{},\"read_only\":true,\"record_time_basis\":{}",
            js(&selected.digest.to_hex()),
            js(if digest_is_bound(state, selected.digest) {
                "persisted_checkpoint"
            } else {
                "caller_requested_digest_only"
            }),
            js(report_origin
                .ok_or_else(|| absent("report time policy missing"))?
                .versions
                .basis()),
        )
    } else {
        String::new()
    };
    let payload = format!(
        "{{\"schema\":\"fmine.forecast-journal.v1\",\"operation_schema\":\"{OPERATION_SCHEMA}\",\"operation\":{},\"head\":{},\"expected_head_checked\":{pinned},\"cell_id\":{},\"clock_skew_ns\":{},\"sequence\":{},\"observed_ns\":{},\"observed_at\":{},\"clock_bounds\":{{\"earliest_ns\":{earliest},\"latest_ns\":{latest}}},\"n_operations\":{},\"n_forecasts\":{},\"n_cohorts\":{},\"n_outcomes\":{outcomes},\"n_assessments\":{},{},\"clock_basis\":\"operator_declared_bounded_host_clock\",\"external_time_proof\":\"{external_time_proof}\",\"timestamp_commitment\":{timestamp_commitment},\"timestamp_verification\":{timestamp_verification},\"source_citations\":{},\"source_admissions\":[{source_admissions}],\"forecasts\":[{forecasts}],\"cohorts\":[{cohorts}],\"assessments\":[{checkpoints}]{retrieval},\"report\":{report}}}",
        js(operation),
        js(&head.to_hex()),
        js(&header.cell_id),
        header.clock_skew_ns,
        last.sequence,
        last.observed_ns,
        utc(last.observed_ns),
        last.sequence,
        state.forecasts.len(),
        state.cohorts.len(),
        state.checkpoints.len(),
        versions.fields(),
        js(if state.source_admissions.is_empty() {
            "supplied_not_verified"
        } else {
            "per_outcome_admission_evidence"
        }),
    );
    let mut rendered_markdown = markdown(
        header,
        head,
        state,
        operation,
        assessment,
        outcomes,
        report_origin.map_or(versions, |origin| origin.versions),
    );
    let _ = writeln!(
        rendered_markdown,
        "\nTimestamp commitment (SHA-256): `{}`. This imprint alone is not external time proof.",
        commitment.imprint_sha256
    );
    if let Some(evidence) = timestamp {
        rendered_markdown.push_str(&evidence.markdown());
    }
    Ok((payload, rendered_markdown))
}

fn markdown(
    header: &Header,
    head: Digest32,
    state: &Replay,
    operation: &str,
    assessment: Option<&Assessment>,
    outcomes: usize,
    versions: VersionCounts,
) -> String {
    use crate::guidance::text;
    let mut out = format!(
        "# Forecast journal: {operation}\n\nCell: {}. Head: `{}`.\n\n{NO_CLAIM}\n\nClock uncertainty bound: {} ns. Forecasts: {}; frozen cohorts: {}; outcome events: {outcomes}; persisted assessments: {}.\n",
        text(&header.cell_id),
        head.to_hex(),
        header.clock_skew_ns,
        state.forecasts.len(),
        state.cohorts.len(),
        state.checkpoints.len(),
    );
    if state.temporal_validation.is_some() {
        out.push_str("\nDeclared ancestry was checked at each forecast's registration. Supplied time metadata is not independently verified; generator replay was not performed. JSON includes the provenance-file hash and per-forecast cone digests.\n");
    }
    if !state.source_admissions.is_empty() {
        out.push_str("\n## Registered source admissions\n\nSource files were verified at admission. Replay checks retained Claim evidence without reopening those files.\n\n| Forecast | Exact printed value | Source Claim | Publication |\n|---|---|---|---|\n");
        for ((id, seq), source) in &state.source_admissions {
            if assessment.is_some_and(|report| {
                !report.per_member.iter().any(|member| {
                    member.forecast_id == *id
                        && member
                            .selected_event
                            .is_some_and(|(selected, _)| selected == *seq)
                })
            }) {
                continue;
            }
            let alias = state.aliases.get(id).map_or("unknown", String::as_str);
            let _ = writeln!(
                out,
                "| {} | {}/{} | `{}` | {}; {} |",
                text(alias),
                source.exact.num(),
                source.exact.den(),
                source.claim,
                text(&source.published_at),
                text(&source.published_at_basis)
            );
        }
    }
    if let Some(report) = assessment {
        if operation == "report" {
            let _ = writeln!(
                out,
                "\nRetrieved persisted assessment `{}` read-only. Record-time policy: {}.",
                report.digest,
                versions.policy(),
            );
            let binding = if digest_is_bound(state, report.digest) {
                "The persisted checkpoint binds this result digest."
            } else {
                "The legacy checkpoint did not bind a result digest; this retrieval verifies the caller-requested digest."
            };
            let _ = writeln!(out, "\n{binding}");
        }
        let _ = writeln!(
            out,
            "\n## Assessment\n\nCut: {}; selector: {}. Members: {}; resolved: {}; unresolved: {}.\n\nAssessment digest: `{}`.\n\n| Forecast | Outcome |\n|---|---|",
            utc(report.as_of_ns),
            selector_name(report.selector),
            report.summary.n_members,
            report.summary.n_resolved,
            report.summary.n_unresolved,
            report.digest.to_hex(),
        );
        for member in &report.per_member {
            let alias = state
                .aliases
                .get(&member.forecast_id)
                .map_or("unknown", String::as_str);
            let outcome = match &member.outcome {
                MemberOutcome::Resolved {
                    value, citation, ..
                } => format!(
                    "{value}; {} / {}; SHA-256 `{}`",
                    text(&citation.source),
                    text(&citation.locator),
                    Digest32(citation.sha256).to_hex(),
                ),
                MemberOutcome::Unresolved { reason, .. } => {
                    format!("unresolved: {}", missing_name(*reason))
                }
            };
            let _ = writeln!(out, "| {} | {outcome} |", text(alias));
        }
        if let Some(coverage) = report.summary.coverage {
            let _ = writeln!(
                out,
                "\nWhole-cohort coverage completion bounds: [{}, {}]. These bounds retain unresolved members and are not confidence intervals.",
                coverage.whole_cohort.0, coverage.whole_cohort.1
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTER: &str = "[[forecast]]\nid = \"one\"\nmodel_key = \"synthetic\"\noutcome_due = \"2030-01-02T00:00:00Z\"\nkind = \"interval\"\nlo = \"40\"\nhi = \"50\"\nnominal = \"0.8\"\n";
    const FREEZE: &str = "[[cohort]]\nname = \"test\"\nmembers = \"one\"\nresolution_rule = \"Invented source value\"\neligible_sources = \"synthetic:issuer\"\n";

    fn semantic_record(request: String, observed_ns: i64) -> Record {
        Record {
            sequence: 1,
            previous: Digest32::default(),
            observed_ns,
            entry: Entry::Operation(request),
        }
    }

    #[test]
    fn writer_adds_v2_but_clients_cannot_select_a_stored_version() {
        let stored = version_request(REGISTER, "forecast").expect("writer v2 marker");
        assert_eq!(stored, format!("schema = \"{V2_SCHEMA}\"\n{REGISTER}"));
        assert_eq!(
            operation_version(&parse_subset(&stored).expect("v2 doc")).expect("v2"),
            OperationVersion::V2
        );
        assert_eq!(
            operation_version(&parse_subset(REGISTER).expect("legacy doc")).expect("v1"),
            OperationVersion::LegacyV1
        );
        for request in [
            stored,
            format!("schema = \"unknown\"\n{REGISTER}"),
            format!("{REGISTER}\n{FREEZE}"),
            FREEZE.to_string(),
        ] {
            assert_eq!(
                version_request(&request, "forecast")
                    .expect_err("client grammar")
                    .code,
                "FMINE-REF-107"
            );
        }
        let unknown = parse_subset(&format!("schema = \"unknown\"\n{REGISTER}"))
            .expect("unknown schema syntax");
        assert_eq!(
            operation_version(&unknown)
                .expect_err("unknown stored policy")
                .code,
            "FMINE-REF-107"
        );
        let oversized = format!(
            "{REGISTER}#{}",
            "x".repeat(MAX_OPERATION_BYTES - REGISTER.len() - 1)
        );
        assert_eq!(oversized.len(), MAX_OPERATION_BYTES);
        assert_eq!(
            version_request(&oversized, "forecast")
                .expect_err("header also counts")
                .code,
            "FMINE-REF-107"
        );
    }

    #[test]
    fn versioned_authority_times_change_ids_without_reinterpreting_legacy_records() {
        let now = time("2030-01-01T00:00:00Z").expect("date") + 123;
        let header = Header {
            cell_id: "synthetic".into(),
            clock_skew_ns: 7,
        };
        let mut ids = Vec::new();
        for v2 in [false, true] {
            let mut state = Replay::default();
            let request = if v2 {
                version_request(REGISTER, "forecast").expect("v2")
            } else {
                REGISTER.into()
            };
            apply(
                &mut state,
                &semantic_record(request, now),
                &header,
                Some("forecast"),
            )
            .expect("registration");
            let id = state.forecast_id("one").expect("id");
            let authority = now + if v2 { 7 } else { 0 };
            assert_eq!(
                state.ledger.forecast(id).expect("forecast").registered_ns,
                authority
            );
            ids.push(id);
            let request = if v2 {
                version_request(FREEZE, "cohort").expect("v2")
            } else {
                FREEZE.into()
            };
            apply(
                &mut state,
                &semantic_record(request, now + 20),
                &header,
                Some("cohort"),
            )
            .expect("freeze");
            let cohort = state.cohorts["test"];
            assert_eq!(
                state
                    .ledger
                    .cohort(cohort)
                    .expect("cohort")
                    .spec
                    .frozen_at_ns,
                authority + 20
            );
        }
        assert_ne!(ids[0], ids[1]);
        let mut legacy = Replay::default();
        apply(
            &mut legacy,
            &semantic_record(REGISTER.into(), now),
            &header,
            None,
        )
        .expect("legacy reread");
        assert_eq!(
            legacy.forecast_id("one").expect("unchanged legacy id"),
            ids[0]
        );
    }

    #[test]
    fn v2_assessments_require_a_checked_digest_and_legacy_unbound_history_stays_readable() {
        let header = Header {
            cell_id: "synthetic".into(),
            clock_skew_ns: 7,
        };
        let start = time("2030-01-01T00:00:00Z").expect("date");
        let cut = time("2030-01-03T00:00:00Z").expect("cut");
        let mut state = Replay::default();
        apply(
            &mut state,
            &semantic_record(REGISTER.into(), start),
            &header,
            None,
        )
        .expect("legacy forecast");
        apply(
            &mut state,
            &semantic_record(FREEZE.into(), start + 20),
            &header,
            None,
        )
        .expect("legacy cohort");
        let request = "[[assessment]]\ncohort = \"test\"\nas_of = \"2030-01-03T00:00:00Z\"\nselector = \"first-published\"\n";
        let missing = version_request(request, "assessment").expect("v2 request");
        assert_eq!(
            apply(
                &mut state,
                &semantic_record(missing, cut + 7),
                &header,
                None
            )
            .expect_err("v2 must pin")
            .code,
            "FMINE-REF-107"
        );
        assert!(state.checkpoints.is_empty());
        let report = apply(
            &mut state,
            &semantic_record(request.into(), cut + 7),
            &header,
            None,
        )
        .expect("legacy checkpoint")
        .expect("assessment");
        assert!(state.checkpoints[0].expected.is_none());
        let bound = format!(
            "{request}expected_digest = {}\n",
            js(&report.digest.to_hex())
        );
        let bound = version_request(&bound, "assessment").expect("v2 pin");
        let replayed = apply(&mut state, &semantic_record(bound, cut + 8), &header, None)
            .expect("v2 checked checkpoint")
            .expect("assessment");
        assert_eq!(replayed.digest, report.digest);
        let wrong = format!("{request}expected_digest = \"{}\"\n", "00".repeat(32));
        let wrong = version_request(&wrong, "assessment").expect("v2 wrong pin syntax");
        assert_eq!(
            apply(&mut state, &semantic_record(wrong, cut + 9), &header, None)
                .expect_err("wrong pin")
                .code,
            "FMINE-REF-505"
        );
    }

    #[test]
    fn generated_assessment_values_preserve_names_without_interpreting_toml() {
        let name = "cohort \\\"first\\\" \\ desk #1";
        let request = format!(
            "[[assessment]]\ncohort = {}\n",
            toml_string(name).expect("safe name")
        );
        let doc = parse_subset(&request).expect("one assessment table");
        assert!(doc.top.is_empty());
        assert_eq!(doc.rows.len(), 1);
        assert_eq!(Row(&doc.rows[0]).string("cohort").expect("cohort"), name);
        for value in [
            "name\n[[forecast]]",
            "name\rnext",
            "name\tpart",
            "name\0tail",
        ] {
            assert_eq!(
                toml_string(value).expect_err("control character").code,
                "FMINE-REF-107"
            );
        }
    }

    #[test]
    fn observed_registration_nanoseconds_are_preserved_without_timestamp_reparsing() {
        let observed_ns = time("2030-01-02T00:00:00Z").expect("date") + 123_456_789;
        let mut state = Replay::default();
        let header = Header {
            cell_id: "synthetic".into(),
            clock_skew_ns: 0,
        };
        let record = Record {
            sequence: 1,
            previous: Digest32::default(),
            observed_ns,
            entry: Entry::Operation("[[forecast]]\nid = \"one\"\nmodel_key = \"synthetic\"\noutcome_due = \"2031-01-01T00:00:00Z\"\nkind = \"bernoulli\"\nprobability = \"0.5\"\n".into()),
        };
        apply(&mut state, &record, &header, Some("forecast")).expect("live registration");
        let id = state.forecast_id("one").expect("registered id");
        let registered = state.ledger.forecast(id).expect("registered forecast");
        assert_eq!(registered.registered_ns, observed_ns);
        assert_eq!(
            utc(registered.registered_ns),
            "\"2030-01-02T00:00:00.123456789Z\""
        );
    }
}
