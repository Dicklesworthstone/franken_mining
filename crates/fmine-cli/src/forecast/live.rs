//! Live forecast admission and durable replay (BD-047). The locked I/O owner assigns record times.
//! Caller requests supply targets and source availability, never their own registration or recording time.

use std::fmt::Write as _;
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use fmine_claim::forecast_journal::{Entry, Header, MAX_OPERATION_BYTES, Record};
use fmine_core::hash::Digest32;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::forecast::Journal;
use fmine_runtime::{HostWallClock, WallClock};

use super::{
    Assessment, MemberOutcome, Replay, Row, absent, cohort_json, invalid, js, missing_name,
    parse_subset, payload_json, selection, selector_name, time, utc,
};
use crate::{DOCTOR_WHY, Envelope, NextAction};

pub const NO_CLAIM: &str = "Local append-only forecast history under an operator-declared host-clock bound. Journal heads identify local history; they are not signatures or external timestamp proofs. Supplied outcomes and citations are not independently verified.";
// FJR1 version 1 binds this exact row grammar, with writer-assigned record times. A later transcript
// grammar must introduce an explicit journal-version migration, never reinterpret these opaque bytes.
const OPERATION_SCHEMA: &str = "fmine.forecast-ledger.v1";

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
    },
}

impl Command {
    fn operation(&self) -> &'static str {
        match self {
            Self::Init { .. } => "init",
            Self::Register { .. } => "register",
            Self::Freeze { .. } => "freeze",
            Self::Outcome { .. } => "outcome",
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
        Err(refusal) => Envelope::refused(&verb, refusal, actions),
    }
}

fn perform(
    command: &Command,
    root: &Path,
    clock: &dyn WallClock,
) -> Result<(String, String), Refusal> {
    match command {
        Command::Init {
            journal,
            cell_id,
            clock_skew_ns,
        } => {
            let journal =
                Journal::create(&root.join(journal), cell_id.as_str(), *clock_skew_ns, clock)?;
            return presentation(&journal, &replay(&journal)?, "init", None, false);
        }
        Command::Inspect { journal, head } => {
            let journal = Journal::open_read(&root.join(journal), *head)?;
            return presentation(
                &journal,
                &replay(&journal)?,
                "inspect",
                None,
                head.is_some(),
            );
        }
        Command::Report {
            journal,
            head,
            assessment,
        } => {
            let journal = Journal::open_read(&root.join(journal), *head)?;
            let mut state = replay(&journal)?;
            let report = persisted_report(&mut state, *assessment)?;
            return presentation(&journal, &state, "report", Some(&report), head.is_some());
        }
        _ => {}
    }
    let (path, head) = match command {
        Command::Register { journal, head, .. }
        | Command::Freeze { journal, head, .. }
        | Command::Outcome { journal, head, .. }
        | Command::Assess { journal, head, .. } => (journal, *head),
        _ => return Err(invalid("this operation needs a writer")),
    };
    let mut journal = Journal::open_write(&root.join(path), head)?;
    let mut state = replay(&journal)?;
    let (request, table) = match command {
        Command::Register { request, .. } => (read_request(&root.join(request))?, "forecast"),
        Command::Freeze { request, .. } => (read_request(&root.join(request))?, "cohort"),
        Command::Outcome { request, .. } => (read_request(&root.join(request))?, "outcome"),
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
                ));
            }
            let mut request = format!(
                "[[assessment]]\ncohort = {}\nas_of = {}\nselector = {}\n",
                cohort_literal,
                toml_string(as_of)?,
                toml_string(selector)?,
            );
            let _ = writeln!(request, "expected_digest = {}", js(&digest.to_hex()));
            (request, "assessment")
        }
        _ => return Err(invalid("this operation has no request")),
    };
    let mut assessment = None;
    journal.append(&request, clock, |record, header| {
        assessment = apply(&mut state, record, header, Some(table))?;
        Ok(())
    })?;
    presentation(
        &journal,
        &state,
        command.operation(),
        assessment.as_ref(),
        true,
    )
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
        .any(|checkpoint| checkpoint.digest == digest && checkpoint.expected == Some(digest))
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
    if !doc.top.is_empty() || doc.rows.len() != 1 {
        return Err(invalid(
            "a live request needs exactly one table and no top-level fields",
        ));
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
                "live journal requests cannot supply {key}; the writer assigns its observed clock time"
            )));
        }
    }
    let (lower, upper) = record.time_bounds(header)?;
    let row = Row(row);
    match row.0.table.as_str() {
        "forecast" => {
            if upper >= row.time("outcome_due")? {
                return Err(late(
                    "registration's latest possible clock time must precede the outcome deadline",
                ));
            }
            state.forecast_at(row, Some(record.observed_ns))?;
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
            state.cohort_at(row, Some(record.observed_ns))?;
        }
        "outcome" => {
            if row.time("available_at")? > lower {
                return Err(late(
                    "outcome availability exceeds the writer's earliest possible clock time",
                ));
            }
            state.outcome_at(row, Some(record.observed_ns))?;
        }
        "assessment" => {
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

fn presentation(
    journal: &Journal,
    state: &Replay,
    operation: &str,
    assessment: Option<&Assessment>,
    pinned: bool,
) -> Result<(String, String), Refusal> {
    let last = journal
        .records()
        .last()
        .ok_or_else(|| absent("journal genesis missing"))?;
    let (earliest, latest) = last.time_bounds(journal.header())?;
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
    let report = assessment
        .map(|a| {
            state.render_origin(
                a,
                journal.head(),
                "journal_head",
                if operation == "report" {
                    "persisted_assessment_replay"
                } else {
                    "durable_journal"
                },
                "operator_declared_bounded_host_clock",
            )
        })
        .transpose()?
        .unwrap_or_else(|| "null".into());
    let retrieval = if operation == "report" {
        let selected = assessment.ok_or_else(|| absent("persisted assessment report missing"))?;
        format!(
            ",\"report_origin\":\"persisted_assessment\",\"selected_assessment_digest\":{},\"result_digest_binding\":{},\"read_only\":true,\"record_time_basis\":\"observed_host_clock_with_declared_uncertainty\"",
            js(&selected.digest.to_hex()),
            js(if digest_is_bound(state, selected.digest) {
                "persisted_checkpoint"
            } else {
                "caller_requested_digest_only"
            }),
        )
    } else {
        String::new()
    };
    let payload = format!(
        "{{\"schema\":\"fmine.forecast-journal.v1\",\"operation_schema\":\"{OPERATION_SCHEMA}\",\"operation\":{},\"head\":{},\"expected_head_checked\":{pinned},\"cell_id\":{},\"clock_skew_ns\":{},\"sequence\":{},\"observed_ns\":{},\"observed_at\":{},\"clock_bounds\":{{\"earliest_ns\":{earliest},\"latest_ns\":{latest}}},\"n_operations\":{},\"n_forecasts\":{},\"n_cohorts\":{},\"n_outcomes\":{outcomes},\"n_assessments\":{},\"clock_basis\":\"operator_declared_bounded_host_clock\",\"external_time_proof\":\"not_verified\",\"source_citations\":\"supplied_not_verified\",\"forecasts\":[{forecasts}],\"cohorts\":[{cohorts}],\"assessments\":[{checkpoints}]{retrieval},\"report\":{report}}}",
        js(operation),
        js(&journal.head().to_hex()),
        js(&journal.header().cell_id),
        journal.header().clock_skew_ns,
        last.sequence,
        last.observed_ns,
        utc(last.observed_ns),
        journal.records().len() - 1,
        state.forecasts.len(),
        state.cohorts.len(),
        state.checkpoints.len(),
    );
    Ok((
        payload,
        markdown(journal, state, operation, assessment, outcomes),
    ))
}

fn markdown(
    journal: &Journal,
    state: &Replay,
    operation: &str,
    assessment: Option<&Assessment>,
    outcomes: usize,
) -> String {
    use crate::guidance::text;
    let mut out = format!(
        "# Forecast journal: {operation}\n\nCell: {}. Head: `{}`.\n\n{NO_CLAIM}\n\nClock uncertainty bound: {} ns. Forecasts: {}; frozen cohorts: {}; outcome events: {outcomes}; persisted assessments: {}.\n",
        text(&journal.header().cell_id),
        journal.head().to_hex(),
        journal.header().clock_skew_ns,
        state.forecasts.len(),
        state.cohorts.len(),
        state.checkpoints.len(),
    );
    if let Some(report) = assessment {
        if operation == "report" {
            let _ = writeln!(
                out,
                "\nRetrieved persisted assessment `{}` read-only. Its recording instants remain observed host-clock times under the declared uncertainty bound.",
                report.digest,
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
