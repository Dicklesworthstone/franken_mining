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
use fmine_public::key::{StatusKind, SubjectStatus};
use fmine_registry::toml_subset::{TomlDoc, TomlValue};
use fmine_runtime::{HostWallClock, WallClock};

use super::source::{PublicMissing, ReceiptClaim, ResolvedOriginalOutcome, StatusEvidence};
use super::{
    Assessment, AssessmentFailure, MemberOutcome, Replay, Row, absent, cohort_json, display_or,
    invalid, js, missing, missing_name, parse_subset, payload_json, selection, selector_name, time,
    utc,
};
use crate::{DOCTOR_WHY, Envelope, NextAction};

pub const NO_CLAIM: &str = "Local append-only forecast history under an operator-declared host-clock bound. Journal heads identify local history; they are not signatures or external timestamp proofs. Registered-source admissions retain their byte-verification evidence; replay does not re-read source files. Other supplied outcomes and citations are unverified.";
// Legacy no-top-level operations retain observed-time semantics and their original IDs. The writer's
// explicit v2 marker changes authority times to the clock upper bound without rewriting old records.
const OPERATION_SCHEMA: &str = "fmine.forecast-ledger.v1";
const V2_SCHEMA: &str = "fmine.forecast-operation.v2";
const V3_SCHEMA: &str = "fmine.forecast-operation.v3";
const V4_SCHEMA: &str = "fmine.forecast-operation.v4";
const V5_SCHEMA: &str = "fmine.forecast-operation.v5";
const V6_SCHEMA: &str = "fmine.forecast-operation.v6";
const V7_SCHEMA: &str = "fmine.forecast-operation.v7";
const V8_SCHEMA: &str = "fmine.forecast-operation.v8";
const V9_SCHEMA: &str = "fmine.forecast-operation.v9";
const V10_SCHEMA: &str = "fmine.forecast-operation.v10";

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
    LegacyV1 = 0,
    V2 = 1,
    V3 = 2,
    V4 = 3,
    V5 = 4,
    V6 = 5,
    V7 = 6,
    V8 = 7,
    V9 = 8,
    V10 = 9,
}

fn operation_version(doc: &TomlDoc) -> Result<OperationVersion, Refusal> {
    if doc.top.is_empty() {
        return Ok(OperationVersion::LegacyV1);
    }
    let schema = doc.top_get("schema").filter(|_| doc.top.len() == 1);
    match schema.map(|field| &field.value) {
        Some(TomlValue::Str(schema)) => match schema.as_str() {
            V2_SCHEMA => Ok(OperationVersion::V2),
            V3_SCHEMA => Ok(OperationVersion::V3),
            V4_SCHEMA => Ok(OperationVersion::V4),
            V5_SCHEMA => Ok(OperationVersion::V5),
            V6_SCHEMA => Ok(OperationVersion::V6),
            V7_SCHEMA => Ok(OperationVersion::V7),
            V8_SCHEMA => Ok(OperationVersion::V8),
            V9_SCHEMA => Ok(OperationVersion::V9),
            V10_SCHEMA => Ok(OperationVersion::V10),
            _ => Err(invalid("unknown stored forecast operation schema")),
        },
        _ => Err(invalid("unknown stored forecast operation schema")),
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
    let schema =
        if expected_table == "registered_missing" && doc.rows[0].get("status_kind").is_some() {
            V10_SCHEMA
        } else if expected_table == "plant_forecast" {
            V9_SCHEMA
        } else if super::cusum::FIELDS
            .iter()
            .any(|key| doc.rows[0].get(key).is_some())
        {
            V8_SCHEMA
        } else if super::segments::FIELDS
            .iter()
            .any(|key| doc.rows[0].get(key).is_some())
        {
            V7_SCHEMA
        } else if doc.rows[0].get("gaussian_crps_bounds").is_some() {
            V6_SCHEMA
        } else if super::MONITOR_FIELDS
            .iter()
            .any(|key| doc.rows[0].get(key).is_some())
            || doc.rows[0].get("event_log_floor").is_some()
        {
            V5_SCHEMA
        } else if expected_table == "registered_restatement"
            || doc.rows[0].get("public_vintages").is_some()
        {
            V4_SCHEMA
        } else if expected_table == "registered_outcome"
            || expected_table == "registered_missing"
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
    /// Register conditional plant quantiles from the admitted shared-scenario production receipt.
    RegisterPlant {
        journal: PathBuf,
        head: Digest32,
        input: PathBuf,
        grinding: PathBuf,
        recovery: PathBuf,
        id: String,
        target: super::plant::Target,
        author_role: String,
        time_basis: String,
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
    /// Append a permitted later registered comparative without replacing the original outcome.
    Restate {
        journal: PathBuf,
        head: Digest32,
        cohort: String,
        forecast: String,
        release: String,
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
    /// Compare two persisted assessments without sampling time or adding journal records.
    Compare {
        journal: PathBuf,
        head: Option<Digest32>,
        left: Digest32,
        right: Digest32,
    },
}

impl Command {
    fn operation(&self) -> &'static str {
        match self {
            Self::Init { .. } => "init",
            Self::Register { .. } => "register",
            Self::RegisterPlant { .. } => "register-plant",
            Self::Freeze { .. } => "freeze",
            Self::Outcome { .. } => "outcome",
            Self::Resolve { .. } => "resolve",
            Self::Restate { .. } => "restate",
            Self::Assess { .. } => "assess",
            Self::Inspect { .. } => "inspect",
            Self::Report { .. } => "report",
            Self::Compare { .. } => "compare",
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
        Command::Compare {
            journal,
            head,
            left,
            right,
        } => {
            let journal = Journal::open_read(&root.join(journal), *head)?;
            let mut state = replay(&journal)?;
            persisted_report(&mut state, *left)?;
            persisted_report(&mut state, *right)?;
            let report = state.ledger.compare_assessments(*left, *right)?;
            let head_digest = journal
                .records()
                .last()
                .ok_or_else(|| absent("journal genesis missing"))?
                .chain_head()?;
            let payload = format!(
                "{{\"operation\":\"compare\",\"head\":{},\"head_pinned\":{},\"comparison\":{}}}",
                js(&head_digest.to_hex()),
                head.is_some(),
                super::comparison::json(&state, &report)?,
            );
            return Ok((payload, super::comparison::markdown(&report)));
        }
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
        | Command::RegisterPlant { journal, head, .. }
        | Command::Freeze { journal, head, .. }
        | Command::Outcome { journal, head, .. }
        | Command::Resolve { journal, head, .. }
        | Command::Restate { journal, head, .. }
        | Command::Assess { journal, head, .. } => (journal, *head),
        _ => return Err(invalid("this operation needs a writer").into()),
    };
    let mut journal = Journal::open_write(&root.join(path), head)?;
    let mut state = replay(&journal)?;
    let mut plant_evidence = None;
    let (request, table, selected) = match command {
        Command::Register { request, .. } => (read_request(&root.join(request))?, "forecast", None),
        Command::RegisterPlant {
            input,
            grinding,
            recovery,
            id,
            target,
            author_role,
            time_basis,
            ..
        } => {
            let prepared = super::plant::prepare(
                &super::plant::Registration {
                    input,
                    grinding,
                    recovery,
                    id,
                    target: *target,
                    author_role,
                    time_basis,
                },
                root,
            )?;
            plant_evidence = Some(prepared.evidence_json);
            (prepared.request, "plant_forecast", None)
        }
        Command::Freeze { request, .. } => (read_request(&root.join(request))?, "cohort", None),
        Command::Outcome { request, .. } => (read_request(&root.join(request))?, "outcome", None),
        Command::Resolve {
            cohort, forecast, ..
        } => {
            let id = state.forecast_id(forecast)?;
            let policy = state.public_policy(cohort, id)?;
            let resolved = super::source::resolve(root, &policy, id)?;
            let table = match &resolved {
                ResolvedOriginalOutcome::Published(_) => "registered_outcome",
                ResolvedOriginalOutcome::Missing(_) => "registered_missing",
            };
            (
                source_request(&state, cohort, forecast, &resolved, None)?,
                table,
                None,
            )
        }
        Command::Restate {
            cohort,
            forecast,
            release,
            ..
        } => {
            let id = state.forecast_id(forecast)?;
            let policy = state.public_policy(cohort, id)?;
            let predecessor = restatement_predecessor(&state, cohort, forecast, release)?;
            let resolved = super::source::resolve_release(root, &policy, id, release)?;
            (
                source_request(
                    &state,
                    cohort,
                    forecast,
                    &ResolvedOriginalOutcome::Published(resolved),
                    Some(predecessor),
                )?,
                "registered_restatement",
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
        // New opt-in policies retain distinct versions; older operation meanings stay frozen.
        let mut versions = previous.versions;
        versions.record(version);
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
        let mut presentation = presentation_at(
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
        )?;
        if let Some(evidence) = &plant_evidence {
            super::plant::attach_evidence(&mut presentation, evidence)?;
        }
        response = Some(presentation);
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
enum SourceAdmissionValue {
    Numeric {
        exact: Rat,
        displayed: String,
    },
    Missing {
        receipt: PublicMissing,
        available_at_ns: i64,
    },
}

#[derive(Debug)]
pub(super) struct SourceAdmission {
    cohort: Digest32,
    claim: ObjectId,
    citation: fmine_calib::OutcomeCitation,
    value: SourceAdmissionValue,
    published_at: String,
    published_at_basis: String,
}

impl SourceAdmission {
    pub(super) fn to_json(&self) -> String {
        let value = match &self.value {
            SourceAdmissionValue::Numeric { exact, displayed } => format!(
                "\"exact_value\":{{\"numerator\":{},\"denominator\":{}}},\"displayed\":{}",
                js(&exact.num().to_string()),
                js(&exact.den().to_string()),
                js(displayed),
            ),
            SourceAdmissionValue::Missing {
                receipt,
                available_at_ns,
            } => {
                let mut value = format!(
                    "\"exact_value\":null,\"displayed\":null,\"source_value_kind\":{},\"source_token\":{},\"missing_reason\":{}",
                    js(&receipt.value_kind),
                    js(&receipt.token),
                    js(missing_name(receipt.reason)),
                );
                if let Some(status) = &receipt.status {
                    let _ = write!(
                        value,
                        ",\"status_interpretation\":{{\"subject\":{},\"kind\":{},\"first_full_month\":{},\"last_month\":{},\"evidence\":{},\"registry_sha256\":{},\"available_at_ns\":{available_at_ns},\"availability_basis\":\"recorded_admission_clock_upper_bound\",\"source_locator\":{},\"source_sha256\":{},\"source_displayed\":{}}}",
                        js(&status.status.subject),
                        js(status_name(status.status.kind)),
                        js(&month_text(status.status.first)),
                        status
                            .status
                            .last
                            .map_or_else(|| "null".into(), |last| js(&month_text(last))),
                        js(&status.status.evidence),
                        js(&status.registry_sha256.to_hex()),
                        js(&self.citation.locator),
                        js(&Digest32(self.citation.sha256).to_hex()),
                        receipt
                            .displayed
                            .as_deref()
                            .map_or_else(|| "null".into(), js),
                    );
                }
                value
            }
        };
        format!(
            "{{\"basis\":\"recorded_registered_source_admission\",\"source_bytes_reverified_on_replay\":false,\"cohort_digest\":{},\"source_claim_id\":{},{value},\"published_at\":{},\"published_at_basis\":{}}}",
            js(&self.cohort.to_hex()),
            js(&self.claim.to_string()),
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

/// Select under the writer lock, and recheck during replay. A comparative cannot stand in for a
/// missing original, skip the numeric predecessor, or import an outcome admitted without source evidence.
fn restatement_predecessor(
    state: &Replay,
    cohort: &str,
    forecast: &str,
    release: &str,
) -> Result<u64, Refusal> {
    use fmine_calib::OutcomeKind;

    let id = state.forecast_id(forecast)?;
    let (policy, vintages) = state.public_policy_with_vintages(cohort, id)?;
    vintages.validate_release(policy.period, release, true)?;
    let events = state.ledger.outcomes(id);
    if !events.iter().any(|event| {
        matches!(&event.kind, OutcomeKind::Published { citation, .. }
            if citation.source == policy.issuer
                && citation.vintage == policy.period.original_release()
                && state.source_admissions.contains_key(&(id, event.seq)))
    }) {
        return Err(invalid(
            "registered restatement requires its source-verified original outcome first",
        ));
    }
    let (event, citation) = events
        .iter()
        .rev()
        .find_map(|event| match &event.kind {
            OutcomeKind::Published { citation, .. } | OutcomeKind::Restated { citation, .. } => {
                Some((event, citation))
            }
            OutcomeKind::Missing { .. } => None,
        })
        .ok_or_else(|| invalid("registered restatement has no numeric predecessor"))?;
    if !state.source_admissions.contains_key(&(id, event.seq))
        || citation.source != policy.issuer
        || citation.vintage.as_str() >= release
    {
        return Err(invalid(
            "registered restatement requires a later release than its source-verified predecessor",
        ));
    }
    Ok(event.seq)
}

fn source_request(
    state: &Replay,
    cohort: &str,
    forecast: &str,
    resolved: &ResolvedOriginalOutcome,
    supersedes: Option<u64>,
) -> Result<String, Refusal> {
    let (receipt, table) = match resolved {
        ResolvedOriginalOutcome::Published(outcome) => (
            &outcome.receipt,
            if supersedes.is_some() {
                "registered_restatement"
            } else {
                "registered_outcome"
            },
        ),
        ResolvedOriginalOutcome::Missing(outcome) => (&outcome.receipt, "registered_missing"),
    };
    let id = state
        .cohorts
        .get(cohort)
        .ok_or_else(|| absent("frozen cohort missing"))?;
    let anchor = receipt
        .source
        .claim
        .anchors
        .first()
        .ok_or_else(|| absent("source Claim anchor missing"))?;
    let mut request = format!("[[{table}]]\n");
    if let Some(sequence) = supersedes {
        let _ = writeln!(request, "supersedes = {sequence}");
    }
    let mut put = |key: &str, value: &str| -> Result<(), Refusal> {
        let _ = writeln!(request, "{key} = {}", toml_string(value)?);
        Ok(())
    };
    put("forecast", forecast)?;
    put("cohort", cohort)?;
    put("cohort_digest", &id.to_hex())?;
    match resolved {
        ResolvedOriginalOutcome::Published(outcome) => {
            put("value", &outcome.value.displayed)?;
            put("numerator", &outcome.value.exact.num().to_string())?;
            put("denominator", &outcome.value.exact.den().to_string())?;
        }
        ResolvedOriginalOutcome::Missing(outcome) => {
            put("reason", missing_name(outcome.value.reason))?
        }
    }
    put("source", &receipt.citation.source)?;
    put("vintage", &receipt.citation.vintage)?;
    put("locator", &receipt.citation.locator)?;
    put("sha256", &Digest32(receipt.citation.sha256).to_hex())?;
    put("claim_id", &receipt.source.claim.claim_id.to_string())?;
    put(
        "claim_body",
        &bytes_hex(&receipt.source.claim.canonical_body()),
    )?;
    match resolved {
        ResolvedOriginalOutcome::Published(outcome) => put("stored", &outcome.value.stored)?,
        ResolvedOriginalOutcome::Missing(outcome) => {
            let missing = &outcome.value;
            put("value_kind", &missing.value_kind)?;
            put("token", &missing.token)?;
            if let Some(status) = &missing.status {
                put("status_subject", &status.status.subject)?;
                put("status_kind", status_name(status.status.kind))?;
                put("status_first", &month_text(status.status.first))?;
                put(
                    "status_last",
                    &status.status.last.map_or_else(String::new, month_text),
                )?;
                put("status_evidence", &status.status.evidence)?;
                put("status_registry_sha256", &status.registry_sha256.to_hex())?;
                if let Some(displayed) = &missing.displayed {
                    put("displayed", displayed)?;
                }
            }
        }
    }
    put("ownership_source", receipt.ownership_source)?;
    put("anchor_path", &anchor.artifact_path)?;
    put("published_at", &receipt.published_at)?;
    put("published_at_basis", &receipt.published_at_basis)?;
    put(
        "available_at",
        &fmine_calib::record::format_utc(receipt.available_at_ns / 1_000_000_000),
    )?;
    Ok(request)
}

fn status_name(status: StatusKind) -> &'static str {
    match status {
        StatusKind::Disposed => "disposed",
        StatusKind::Withheld => "withheld",
        StatusKind::CareAndMaintenance => "care_and_maintenance",
    }
}

fn month_text((year, month): (u16, u8)) -> String {
    format!("{year:04}-{month:02}")
}

fn status_month(value: &str) -> Result<(u16, u8), Refusal> {
    let invalid = || invalid("status month must be YYYY-MM");
    let (year, month) = value.split_once('-').ok_or_else(invalid)?;
    let month = (
        year.parse().map_err(|_| invalid())?,
        month.parse().map_err(|_| invalid())?,
    );
    if month_text(month) != value {
        return Err(invalid());
    }
    Ok(month)
}

fn publication_metadata(row: &Row<'_>) -> Result<(String, String, i64), Refusal> {
    let published_at = row.string("published_at")?.to_string();
    let published_at_basis = row.string("published_at_basis")?;
    let publication_start = super::source::publication_start(&published_at, published_at_basis)
        .map_err(|(_, reason)| invalid(reason))?;
    Ok((
        published_at,
        published_at_basis.to_string(),
        publication_start,
    ))
}

fn apply_source(state: &mut Replay, row: Row<'_>, recorded_ns: i64) -> Result<(), Refusal> {
    let is_missing = row.0.table == "registered_missing";
    let restatement = row.0.table == "registered_restatement";
    row.allowed(
        &[
            "forecast",
            "cohort",
            "cohort_digest",
            "source",
            "vintage",
            "locator",
            "sha256",
            "claim_id",
            "claim_body",
            "published_at",
            "published_at_basis",
            "available_at",
            "ownership_source",
            "anchor_path",
        ],
        if is_missing {
            &[
                "reason",
                "value_kind",
                "token",
                "displayed",
                "status_subject",
                "status_kind",
                "status_first",
                "status_last",
                "status_evidence",
                "status_registry_sha256",
            ]
        } else if restatement {
            &["value", "numerator", "denominator", "stored", "supersedes"]
        } else {
            &["value", "numerator", "denominator", "stored"]
        },
    )?;
    let forecast = state.forecast_id(row.string("forecast")?)?;
    let cohort = row.string("cohort")?;
    let (policy, vintages) = state.public_policy_with_vintages(cohort, forecast)?;
    let release = row.string("vintage")?;
    vintages.validate_release(policy.period, release, restatement)?;
    let cohort_digest = super::hex(row.string("cohort_digest")?)?;
    if state.cohorts.get(cohort) != Some(&cohort_digest) || row.string("source")? != policy.issuer {
        return Err(invalid(
            "source admission differs from the frozen cohort or issuer",
        ));
    }
    let (published_at, published_at_basis, publication_start) = publication_metadata(&row)?;
    let available_ns = publication_start
        .checked_add(86_400_000_000_000)
        .ok_or_else(|| invalid("publication availability overflows"))?;
    if available_ns != row.time("available_at")?
        || publication_start < policy.period.end_ns()?
        || publication_start < super::PublicPeriod::parse(release)?.end_ns()?
    {
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
    let reconstruction = ReceiptClaim {
        policy: &policy,
        forecast_id: forecast,
        citation: &citation,
        ownership_source: row.string("ownership_source")?,
        anchor_path: row.string("anchor_path")?,
    };
    let (expected_body, kind, value, event_available_ns) = if is_missing {
        if !state.ledger.outcomes(forecast).is_empty() {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                "the original public outcome status was already recorded",
            ));
        }
        let status = if row
            .0
            .fields
            .iter()
            .any(|field| field.key.starts_with("status_"))
        {
            let kind = row.string("status_kind")?;
            let Some(TomlValue::Str(last)) = row.0.get("status_last").map(|field| &field.value)
            else {
                return Err(invalid(
                    "status_last must declare a quoted end month or an empty open interval",
                ));
            };
            Some(StatusEvidence {
                status: SubjectStatus {
                    subject: row.string("status_subject")?.into(),
                    kind: match kind {
                        "disposed" => StatusKind::Disposed,
                        "withheld" => StatusKind::Withheld,
                        _ => return Err(invalid("unsupported target-status interpretation")),
                    },
                    first: status_month(row.string("status_first")?)?,
                    last: if last.is_empty() {
                        None
                    } else {
                        Some(status_month(last)?)
                    },
                    evidence: row.string("status_evidence")?.into(),
                },
                registry_sha256: super::hex(row.string("status_registry_sha256")?)?,
            })
        } else {
            None
        };
        let missing = PublicMissing {
            reason: missing(row.string("reason")?)?,
            value_kind: row.string("value_kind")?.into(),
            token: row.string("token")?.into(),
            displayed: row.optional_string("displayed")?.map(str::to_string),
            status,
        };
        if missing.status.is_none() && missing.reason != fmine_calib::MissingReason::NotPublished {
            return Err(invalid(
                "registered source dashes map only to not_published",
            ));
        }
        let body = reconstruction.missing(&missing)?;
        // Effective dates and a registry digest do not prove when the interpretation became known.
        // Only the persisted admission clock bound supplies its availability; source publication stays separate.
        let available_at_ns = if missing.status.is_some() {
            recorded_ns
        } else {
            available_ns
        };
        (
            body,
            fmine_calib::OutcomeKind::Missing {
                reason: missing.reason,
            },
            SourceAdmissionValue::Missing {
                receipt: missing,
                available_at_ns,
            },
            available_at_ns,
        )
    } else {
        let supersedes = if restatement {
            let expected =
                restatement_predecessor(state, cohort, row.string("forecast")?, release)?;
            if row
                .optional_integer("supersedes")?
                .and_then(|n| u64::try_from(n).ok())
                != Some(expected)
            {
                return Err(invalid(
                    "registered restatement must name its latest numeric predecessor",
                ));
            }
            Some(expected)
        } else {
            None
        };
        let integer = |key| {
            row.string(key)?.parse::<i128>().map_err(|_| {
                invalid("source exact value needs signed integer numerator and denominator")
            })
        };
        let exact = Rat::new(integer("numerator")?, integer("denominator")?)
            .map_err(|_| invalid("invalid exact source value"))?;
        let displayed = row.string("value")?.to_string();
        let printed = if policy.metric.is_site_physical() {
            super::source::displayed_value(policy.metric, &displayed)?
        } else {
            let fmine_registry::transcriptions::Token::Point(decimal) =
                fmine_registry::transcriptions::parse_token(&displayed).map_err(invalid)?
            else {
                return Err(invalid("registered outcome needs one printed number"));
            };
            Rat::new(decimal.mantissa, 10_i128.pow(decimal.scale))
                .map_err(|_| invalid("invalid printed value"))?
        };
        if printed != exact {
            return Err(invalid(
                "printed source value differs from its exact rational",
            ));
        }
        if policy.metric.is_production() && exact.num() < 0 {
            return Err(invalid("production cannot be negative"));
        }
        let body = reconstruction.number(row.string("stored")?, &displayed, restatement)?;
        let number = exact.num() as f64 / exact.den() as f64;
        let kind = match supersedes {
            Some(supersedes) => fmine_calib::OutcomeKind::Restated {
                value: number,
                citation: citation.clone(),
                supersedes,
            },
            None => fmine_calib::OutcomeKind::Published {
                value: number,
                citation: citation.clone(),
            },
        };
        (
            body,
            kind,
            SourceAdmissionValue::Numeric { exact, displayed },
            available_ns,
        )
    };
    if body != expected_body {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            "source Claim does not bind the frozen target, exact printed value and cited anchor",
        ));
    }
    let event = state
        .ledger
        .append_outcome(forecast, event_available_ns, recorded_ns, kind)?;
    state.source_admissions.insert(
        (forecast, event),
        SourceAdmission {
            cohort: cohort_digest,
            claim,
            citation,
            value,
            published_at,
            published_at_basis,
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
    let status_declared = row
        .fields
        .iter()
        .any(|field| field.key.starts_with("status_"));
    let metadata_declared = super::segments::FIELDS
        .iter()
        .any(|key| row.get(key).is_some());
    let monitor_declared = super::MONITOR_FIELDS
        .iter()
        .any(|key| row.get(key).is_some());
    let cusum_declared = super::cusum::FIELDS
        .iter()
        .any(|key| row.get(key).is_some());
    let event_log_declared = row.get("event_log_floor").is_some();
    let gaussian_bounds_declared = row.get("gaussian_crps_bounds").is_some();
    let vintage_declared = row.get("public_vintages").is_some();
    use OperationVersion::{V4, V5, V6, V7, V8, V9, V10};
    // A feature's accepted schemas and each schema's required operation are separate constraints.
    // Keeping both directions explicit prevents incomplete receipts from downgrading their semantics.
    let features: &[(bool, &[OperationVersion], &str)] = &[
        (metadata_declared, &[V7, V9], "forecast metadata"),
        (cusum_declared, &[V8], "CUSUM policy"),
        (gaussian_bounds_declared, &[V6, V8], "Gaussian CRPS bounds"),
        (
            monitor_declared || event_log_declared,
            &[V5, V6, V8],
            "coverage or event-log policy",
        ),
        (vintage_declared, &[V4, V5, V6, V8], "public vintage policy"),
        (status_declared, &[V10], "subject-status interpretation"),
        (
            row.table == "plant_forecast",
            &[V9],
            "plant source registration",
        ),
    ];
    for (declared, allowed, feature) in features {
        if *declared && !allowed.contains(&version) {
            return Err(invalid(format!(
                "{feature} is not supported by this operation schema"
            )));
        }
    }
    let required = match version {
        V4 => row.table == "registered_restatement" || (row.table == "cohort" && vintage_declared),
        V5 => row.table == "cohort" && (monitor_declared || event_log_declared),
        V6 => row.table == "cohort" && gaussian_bounds_declared,
        V7 => row.table == "forecast" && metadata_declared,
        V8 => row.table == "cohort" && cusum_declared,
        V9 => row.table == "plant_forecast" && metadata_declared,
        V10 => row.table == "registered_missing" && status_declared,
        _ => true,
    };
    if !required {
        return Err(invalid(
            "operation schema requires its declared policy and matching table",
        ));
    }
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
        _ => upper,
    };
    let row = Row(row);
    match row.0.table.as_str() {
        "plant_forecast" => {
            let start = super::plant::registration_start(&row, request)?;
            if upper >= start {
                return Err(late(
                    "plant registration's latest possible clock time must precede the physical interval's start",
                ));
            }
            let mut forecast = row.0.clone();
            forecast.table = "forecast".into();
            forecast
                .fields
                .retain(|field| field.key != "registration_before");
            state.forecast_at(Row(&forecast), Some(authority_ns))?;
            let id = state.forecast_id(row.string("id")?)?;
            state.plant_registration_starts.insert(id, start);
        }
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
                if state
                    .plant_registration_starts
                    .get(&id)
                    .is_some_and(|start| upper >= *start)
                {
                    return Err(late(format!(
                        "cohort freeze's latest possible clock time reaches {alias}'s plant physical start"
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
        "registered_outcome" | "registered_missing" | "registered_restatement" => {
            let required = match row.0.table.as_str() {
                "registered_restatement" => OperationVersion::V4,
                "registered_missing" if status_declared => OperationVersion::V10,
                _ => OperationVersion::V3,
            };
            if version != required {
                return Err(invalid("registered source operation has the wrong version"));
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
struct VersionCounts([usize; 10]);

impl VersionCounts {
    fn record(&mut self, version: OperationVersion) {
        self.0[version as usize] += 1;
    }

    fn policy(self) -> &'static str {
        match (self.0[0], self.0[1..].iter().sum::<usize>()) {
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
        let mut counts = format!("\"legacy_v1\":{},\"v2\":{}", self.0[0], self.0[1]);
        for (index, count) in self.0.iter().enumerate().skip(2) {
            if *count != 0 {
                let _ = write!(counts, ",\"v{}\":{count}", index + 1);
            }
        }
        format!(
            "\"operation_version_counts\":{{{counts}}},\"record_time_policy\":{}",
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
        counts.record(operation_version(&doc)?);
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
        let metadata = super::segments::metadata_field(row.metadata.as_ref());
        let registration_before = state.plant_registration_starts.get(id).map_or_else(String::new, |start| format!(
            ",\"registration_before_ns\":{start},\"registration_before\":{}", utc(*start)));
        Ok(format!("{{\"alias\":{},\"forecast_id\":{},\"model_key\":{},\"registered_ns\":{},\"registered_at\":{},\"outcome_due_ns\":{},\"outcome_due\":{},\"payload\":{}{metadata}{registration_before}}}",
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
    )?;
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
) -> Result<String, Refusal> {
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
            let alias = state.alias(*id);
            let value = match &source.value {
                SourceAdmissionValue::Numeric { exact, .. } => {
                    format!("{}/{}", exact.num(), exact.den())
                }
                SourceAdmissionValue::Missing {
                    receipt,
                    available_at_ns,
                } => {
                    let mut value = format!(
                        "missing: {} ({}; token {})",
                        missing_name(receipt.reason),
                        receipt.value_kind,
                        receipt.token
                    );
                    if let Some(status) = &receipt.status {
                        let _ = write!(
                            value,
                            "; {} from {}, through {}; evidence {}; registry SHA-256 {}; source {} / SHA-256 {}; displayed {}; interpretation available at admission {available_at_ns} ns",
                            status_name(status.status.kind),
                            month_text(status.status.first),
                            status.status.last.map_or_else(|| "open".into(), month_text),
                            status.status.evidence,
                            status.registry_sha256,
                            source.citation.locator,
                            Digest32(source.citation.sha256),
                            receipt.displayed.as_deref().unwrap_or("none")
                        );
                    }
                    value
                }
            };
            let _ = writeln!(
                out,
                "| {} | {} | `{}` | {}; {} |",
                text(alias),
                text(&value),
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
            let alias = state.alias(member.forecast_id);
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
        super::segments::assessment_markdown(&mut out, state, report)?;
        super::diagnostics::assessment_markdown(&mut out, state, report)?;
        if let Some(log) = report.summary.event_log {
            let score = log.score;
            let _ = writeln!(
                out,
                "\n## Event log score\n\nFrozen probability floor: {}. Loss: -ln(max(P(observed outcome), floor)), in nats. Scored: {} of {}; observed mean: {}. Whole-cohort completion bounds: [{}, {}].\n\nEvery unresolved member remains in these completion bounds. The explicit floor caps extreme losses and removes strict propriety in the floored tails; the original probabilities and proper Brier score are unchanged. This descriptive score does not establish calibration.",
                log.probability_floor,
                score.n_scored,
                report.summary.n_members,
                display_or(score.observed_mean, "unscored"),
                score.whole_cohort.lower,
                display_or(score.whole_cohort.upper, "unbounded"),
            );
        }
        if let Some(monitor) = &report.cusum_monitor {
            out.push_str(&super::cusum::markdown(state, monitor));
        }
        if let Some(monitor) = &report.coverage_monitor {
            let snapshot = monitor.snapshot;
            let _ = writeln!(
                out,
                "\n## Sequential coverage monitor\n\nDeclared support: {}. Declared assumption: {}. The exact conditional null is P(interval hit | past) = {}; this assumption is supplied, not independently verified. Alarm alpha: {}.\n\nOriginal publications in frozen order: {} observed, {} hits. Log e-value: {}; running maximum: {}; log alarm threshold: {}. First crossing: {}. Alarmed: {}.\n\nRestatements do not replace this sequence. An alarm does not recalibrate the forecasts.\n\n| Order | Forecast | Frozen interval | Original outcome and citation | Hit | Event digest | Source Claim |\n|---|---|---|---|---|---|---|",
                text(&monitor.policy.support),
                text(&monitor.policy.conditional_null),
                monitor.null_probability,
                monitor.policy.alarm_alpha,
                snapshot.n,
                snapshot.hits,
                snapshot.log_e_value,
                snapshot.max_log_e_value,
                snapshot.log_threshold,
                display_or(snapshot.first_crossing, "none"),
                snapshot.alarmed(),
            );
            for (index, observation) in monitor.observations.iter().enumerate() {
                let member = &observation.member;
                let alias = state.alias(member.forecast_id);
                if let MemberOutcome::Resolved {
                    value, citation, ..
                } = &member.outcome
                {
                    let event = member
                        .selected_event
                        .map_or_else(|| "none".into(), |(_, digest)| digest.to_hex());
                    let claim = member
                        .selected_event
                        .and_then(|(seq, _)| {
                            state.source_admissions.get(&(member.forecast_id, seq))
                        })
                        .map_or_else(|| "not verified".into(), |source| source.claim.to_string());
                    let _ = writeln!(
                        out,
                        "| {} | {} | [{}, {}] | {}; {} / {} / {}; SHA-256 `{}` | {} | `{}` | {} |",
                        index + 1,
                        text(alias),
                        observation.interval.0,
                        observation.interval.1,
                        value,
                        text(&citation.source),
                        text(&citation.vintage),
                        text(&citation.locator),
                        Digest32(citation.sha256).to_hex(),
                        observation.hit,
                        event,
                        text(&claim),
                    );
                }
            }
            if let Some(blocker) = &monitor.blocked_by {
                let alias = state.alias(blocker.forecast_id);
                let event = blocker
                    .selected_event
                    .map_or_else(|| "none".into(), |(_, digest)| digest.to_hex());
                let reason = match &blocker.outcome {
                    MemberOutcome::Unresolved { reason } => missing_name(*reason),
                    MemberOutcome::Resolved { .. } => "resolved",
                };
                let _ = writeln!(
                    out,
                    "\nSequence stops at unresolved forecast {} (reason: {}; retained event digest: `{event}`). {} later members are deferred, including {} with resolved original publications. No later member was skipped into the sequence.",
                    text(alias),
                    reason,
                    monitor.n_deferred,
                    monitor.n_deferred_resolved,
                );
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "../../../xtask/testdata/forecast_live_controls.rs"]
mod tests;
