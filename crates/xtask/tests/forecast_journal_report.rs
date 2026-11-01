//! Synthetic acceptance for retrieving persisted forecast assessments without extending their journal.
//! Invented clocks, forecasts and citations are not external time proof or independently verified data.

#![cfg(test)]
#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use fmine_calib::record::{format_utc, parse_hex32, parse_utc};
use fmine_claim::forecast_journal::Entry;
use fmine_cli::forecast_journal::{Command, TimestampProof, execute_with_clock};
use fmine_cli::{CliCommand, Envelope, OutputFormat, execute_command, invoke, parse_args};
use fmine_core::hash::{Digest32, sha256};
use fmine_io::forecast::Journal as DurableJournal;
use fmine_io::forecast::prefix::{PrefixCommitment, commitment};
use fmine_io::forecast::timestamp::verify_rfc3161;
use fmine_registry::toml_subset::{TomlValue, parse_subset};
use fmine_runtime::{ClockError, WallClock};

const SECOND: i64 = 1_000_000_000;
const EARLY_CUT: &str = "2031-01-10T00:00:00Z";
const LATE_CUT: &str = "2031-01-30T00:00:00Z";
const JOURNAL: &str = "forecasts.journal";
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
// OpenSSL process creation can inherit another test's flock until exec, briefly extending its lifetime.
// Serialize this binary's fixtures so independent drop/reopen checks cannot race that inheritance.
static JOURNAL_FIXTURES: Mutex<()> = Mutex::new(());

fn isolated_fixture() -> MutexGuard<'static, ()> {
    JOURNAL_FIXTURES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct FixedClock(i64);

impl WallClock for FixedClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        Ok(self.0)
    }
}

struct NeverClock;

impl WallClock for NeverClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        panic!("retrieving a persisted assessment must not sample a clock")
    }
}

fn ns(utc: &str) -> i64 {
    parse_utc(utc)
        .expect("synthetic UTC")
        .checked_mul(SECOND)
        .expect("nanoseconds")
}

// Extract only named scalars and the assessment subsection of these controlled JSON fixtures.
fn string_field<'a>(json: &'a str, field: &str) -> &'a str {
    json.split_once(&format!("\"{field}\":\""))
        .unwrap_or_else(|| panic!("missing {field}: {json}"))
        .1
        .split('"')
        .next()
        .expect("string field")
}

fn digest_field(json: &str, field: &str) -> Digest32 {
    Digest32(parse_hex32(string_field(json, field)).expect("canonical digest"))
}

fn assessment(json: &str) -> &str {
    json.split_once("\"assessment\":")
        .expect("assessment")
        .1
        .split_once(",\"checkpoints\":")
        .expect("checkpoints")
        .0
}

// The nested report is the final payload field; retain its full object, removing only the outer brace.
fn nested_report(json: &str) -> &str {
    json.split_once("\"report\":")
        .expect("nested report")
        .1
        .strip_suffix('}')
        .expect("payload end")
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

struct Journal {
    root: PathBuf,
    head: Digest32,
    requests: usize,
}

impl Journal {
    fn new() -> Self {
        Self::new_at(ns("2030-01-01T00:00:00Z"))
    }

    fn new_at(at: i64) -> Self {
        let root = std::env::temp_dir().join(format!(
            "fmine-saved-forecast-report-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("fresh synthetic test directory");
        let mut journal = Self {
            root,
            head: Digest32::default(),
            requests: 0,
        };
        journal.write(
            Command::Init {
                journal: JOURNAL.into(),
                cell_id: "synthetic-report-cell".into(),
                clock_skew_ns: SECOND as u64,
            },
            at,
        );
        journal
    }

    fn write(&mut self, command: Command, at: i64) -> String {
        let envelope = execute_with_clock(&command, &self.root, &FixedClock(at));
        assert_eq!(envelope.exit_code, 0, "{envelope:?}");
        self.head = digest_field(&envelope.payload_json, "head");
        envelope.payload_json
    }

    fn append(&mut self, text: &str, at: &str) {
        self.append_at(text, ns(at));
    }

    fn append_at(&mut self, text: &str, at: i64) {
        let request = PathBuf::from(format!("request-{}.toml", self.requests));
        self.requests += 1;
        fs::write(self.root.join(&request), text).expect("synthetic operation");
        let journal = JOURNAL.into();
        let head = self.head;
        let command = match text.lines().next().expect("operation table") {
            "[[forecast]]" => Command::Register {
                journal,
                head,
                request,
            },
            "[[cohort]]" => Command::Freeze {
                journal,
                head,
                request,
            },
            "[[outcome]]" => Command::Outcome {
                journal,
                head,
                request,
            },
            other => panic!("unexpected synthetic operation {other}"),
        };
        self.write(command, at);
    }

    fn assess(&mut self, cut: &str, selector: &str) -> String {
        self.write(
            Command::Assess {
                journal: JOURNAL.into(),
                head: self.head,
                cohort: "two-sites".into(),
                as_of: cut.into(),
                selector: selector.into(),
                expected_digest: None,
            },
            ns(cut) + SECOND,
        )
    }

    fn report(&self, assessment: Digest32, head: Option<Digest32>) -> Envelope {
        self.report_with_provenance(assessment, head, None)
    }

    fn report_with_provenance(
        &self,
        assessment: Digest32,
        head: Option<Digest32>,
        provenance: Option<PathBuf>,
    ) -> Envelope {
        execute_with_clock(
            &Command::Report {
                journal: JOURNAL.into(),
                head,
                assessment,
                provenance,
                timestamp: None,
            },
            &self.root,
            &NeverClock,
        )
    }

    fn report_with_timestamp(&self, assessment: Digest32, proof: TimestampProof) -> Envelope {
        execute_with_clock(
            &Command::Report {
                journal: JOURNAL.into(),
                head: Some(self.head),
                assessment,
                provenance: None,
                timestamp: Some(proof),
            },
            &self.root,
            &NeverClock,
        )
    }

    fn bytes(&self) -> Vec<u8> {
        fs::read(self.root.join(JOURNAL)).expect("journal bytes")
    }
}

fn saved_reports() -> (Journal, String, String) {
    saved_reports_with_fraction(0)
}

fn saved_reports_with_fraction(fraction_ns: i64) -> (Journal, String, String) {
    let mut journal = Journal::new();
    for (alias, at) in [
        ("f1", "2030-01-02T00:00:00Z"),
        ("f2", "2030-01-03T00:00:00Z"),
    ] {
        journal.append_at(
            &format!(
                "[[forecast]]\nid = \"{alias}\"\nmodel_key = \"synthetic:{alias}\"\noutcome_due = \"2030-12-31T00:00:00Z\"\nkind = \"interval\"\nlo = \"40\"\nhi = \"50\"\nnominal = \"0.8\"\n"
            ),
            ns(at) + fraction_ns,
        );
    }
    journal.append(
        "[[cohort]]\nname = \"two-sites\"\nmembers = \"f1,f2\"\nresolution_rule = \"Synthetic annual output on a fixed basis\"\neligible_sources = \"synthetic:issuer\"\nalpha = \"0.2\"\nmin_calibration_n = 35\nrho_low = \"0\"\nrho_high = \"0.5\"\noutcome_lo = \"0\"\noutcome_hi = \"100\"\n",
        "2030-01-04T00:00:00Z",
    );
    let outcome = |kind: &str, value: i64, available: &str, predecessor: &str| {
        format!(
            "[[outcome]]\nforecast = \"f1\"\nkind = \"{kind}\"\navailable_at = \"{available}\"\nvalue = \"{value}\"\nsource = \"synthetic:issuer\"\nvintage = \"synthetic-{kind}\"\nlocator = \"Invented!A1\"\nsha256 = \"{}\"\n{predecessor}",
            "11".repeat(32)
        )
    };
    journal.append(
        &outcome("published", 45, "2031-01-02T00:00:00Z", ""),
        "2031-01-03T00:00:00Z",
    );
    journal.append(
        "[[outcome]]\nforecast = \"f2\"\nkind = \"missing\"\navailable_at = \"2031-01-02T00:00:00Z\"\nreason = \"not_published\"\n",
        "2031-01-04T00:00:00Z",
    );
    let early = journal.assess(EARLY_CUT, "first-published");
    journal.append(
        &outcome("restated", 55, "2031-01-20T00:00:00Z", "supersedes = 1\n"),
        "2031-01-21T00:00:00Z",
    );
    let latest = journal.assess(LATE_CUT, "latest-as-of");
    (journal, early, latest)
}

#[test]
fn saved_reports_reopen_without_clock_or_mutation() {
    let _fixture = isolated_fixture();
    let (journal, early, latest) = saved_reports();
    let before = journal.bytes();
    let early_digest = digest_field(assessment(&early), "digest");
    let latest_digest = digest_field(assessment(&latest), "digest");
    assert_ne!(early_digest, latest_digest);

    let persisted = DurableJournal::open_read(&journal.root.join(JOURNAL), Some(journal.head))
        .expect("reopen the hash-verified journal");
    let mut checkpoints = Vec::new();
    for record in persisted.records() {
        if let Entry::Operation(request) = &record.entry {
            checkpoints.extend(
                parse_subset(request)
                    .expect("persisted operation grammar")
                    .rows
                    .into_iter()
                    .filter(|row| row.table == "assessment"),
            );
        }
    }
    assert_eq!(checkpoints.len(), 2);
    for (row, (cut, selector, digest)) in checkpoints.iter().zip([
        (EARLY_CUT, "first-published", early_digest),
        (LATE_CUT, "latest-as-of", latest_digest),
    ]) {
        // These are the actual hash-chained operation bytes, not fields copied from a report response.
        for (key, expected) in [
            ("cohort", "two-sites".to_string()),
            ("as_of", cut.to_string()),
            ("selector", selector.to_string()),
            ("expected_digest", digest.to_hex()),
        ] {
            assert_eq!(
                row.get(key).map(|field| &field.value),
                Some(&TomlValue::Str(expected)),
                "persisted assessment {key}"
            );
        }
    }
    drop(persisted);

    // Independent interval-score answers: width 10 inside; 10 + (2 / .2) * 5 = 60 outside.
    for (saved, digest, score, kind, value, coverage) in [
        (&early, early_digest, 10, "published", 45, "[0.5,1]"),
        (&latest, latest_digest, 60, "restated", 55, "[0,0.5]"),
    ] {
        let expected = assessment(saved);
        for fragment in [
            "\"n_members\":2,\"n_resolved\":1,\"n_unresolved\":1,".to_string(),
            "\"unresolved_by_reason\":{\"not_published\":1}".to_string(),
            format!("\"interval_score\":{{\"n_scored\":1,\"observed_mean\":{score},"),
            format!("\"kind\":\"{kind}\""),
            format!("\"status\":\"resolved\",\"value\":{value},"),
            format!("\"whole_cohort\":{coverage}"),
            "\"status\":\"unresolved\",\"reason\":\"not_published\"".to_string(),
        ] {
            assert!(
                expected.contains(&fragment),
                "missing {fragment}: {expected}"
            );
        }
        for pin in [Some(journal.head), None] {
            let envelope = journal.report(digest, pin);
            assert_eq!(envelope.exit_code, 0, "{envelope:?}");
            assert_eq!(envelope.verb, "forecast journal report");
            assert_eq!(envelope.anchor_id, None);
            assert_eq!(envelope.certificate, None);
            let payload = &envelope.payload_json;
            assert_eq!(assessment(payload), expected);
            let original_report = nested_report(saved);
            let replayed_report = nested_report(payload);
            assert_eq!(replayed_report, original_report);
            assert_eq!(
                digest_field(replayed_report, "journal_head"),
                digest_field(saved, "head")
            );
            assert_eq!(string_field(replayed_report, "mode"), "durable_journal");
            if digest == early_digest {
                assert!(!replayed_report.contains(&latest_digest.to_hex()));
            }
            assert_eq!(digest_field(payload, "head"), journal.head);
            assert_eq!(digest_field(payload, "selected_assessment_digest"), digest);
            let retrieval = payload
                .split_once("\"selected_assessment_digest\":")
                .expect("selected assessment receipt")
                .1;
            assert_eq!(
                string_field(retrieval, "result_digest_binding"),
                "persisted_checkpoint"
            );
            for fragment in [
                "\"report_origin\":\"persisted_assessment\"".to_string(),
                "\"record_time_basis\":\"clock_upper_bound_with_declared_uncertainty\"".to_string(),
                "\"read_only\":true".to_string(),
                "\"n_assessments\":2".to_string(),
                format!("\"expected_head_checked\":{}", pin.is_some()),
            ] {
                assert!(payload.contains(&fragment), "missing {fragment}: {payload}");
            }
            let json = envelope.to_json();
            for flag in [
                "reserve_disclosure_claim",
                "execution_authorized",
                "geotechnical_signoff",
                "regulatory_compliance_claim",
            ] {
                assert!(json.contains(&format!("\"{flag}\":false")), "{json}");
            }
            let markdown = envelope.rendered_markdown.expect("readable saved report");
            assert!(markdown.contains(&digest.to_hex()));
            assert!(markdown.contains("unresolved: not_published"));
            assert_eq!(journal.bytes(), before);
        }
    }
    for (requested, pin, code) in [
        (Digest32::default(), journal.head, "FMINE-REF-201"),
        (early_digest, digest_field(&early, "head"), "FMINE-REF-502"),
    ] {
        let refused = journal.report(requested, Some(pin));
        assert_ne!(refused.exit_code, 0, "{refused:?}");
        assert_eq!(refused.refusal.expect("typed refusal").code, code);
        assert_eq!(journal.bytes(), before);
    }
}

#[test]
fn legacy_unbound_assessments_remain_readable_without_claiming_a_persisted_result() {
    let _fixture = isolated_fixture();
    let (mut journal, early, latest) = saved_reports();
    // Reproduce the older v1 writer's valid optional-field grammar. The CLI validates all semantics
    // on reopening; no existing operation is rewritten to manufacture this historical fixture.
    let mut writer = DurableJournal::open_write(&journal.root.join(JOURNAL), journal.head)
        .expect("exclusive legacy fixture append");
    writer
        .append(
            &format!(
                "[[assessment]]\ncohort = \"two-sites\"\nas_of = \"{LATE_CUT}\"\nselector = \"first-published\"\n"
            ),
            &FixedClock(ns("2031-01-31T00:00:00Z")),
            |_, _| Ok(()),
        )
        .expect("old assessment row without a result binding");
    journal.head = writer.head();
    drop(writer);
    let before = journal.bytes();
    let inspected = execute_with_clock(
        &Command::Inspect {
            journal: JOURNAL.into(),
            head: Some(journal.head),
        },
        &journal.root,
        &NeverClock,
    );
    assert_eq!(inspected.exit_code, 0, "{inspected:?}");
    assert_eq!(journal.bytes(), before);
    let checkpoints = inspected
        .payload_json
        .split_once("\"assessments\":[")
        .expect("persisted assessment inventory")
        .1
        .split_once(']')
        .expect("inventory end")
        .0;
    let legacy = checkpoints
        .rsplit_once("},{")
        .expect("new and legacy checkpoints")
        .1;
    assert!(legacy.contains("\"result_digest_binding\":\"legacy_unbound\""));
    let digest = digest_field(legacy, "digest");
    assert_ne!(digest, digest_field(assessment(&early), "digest"));
    assert_ne!(digest, digest_field(assessment(&latest), "digest"));
    let envelope = journal.report(digest, Some(journal.head));
    assert_eq!(envelope.exit_code, 0, "{envelope:?}");
    let payload = &envelope.payload_json;
    assert!(payload.contains("\"result_digest_binding\":\"caller_requested_digest_only\""));
    assert!(payload.contains("\"n_assessments\":3"));
    assert_eq!(digest_field(payload, "head"), journal.head);
    assert_eq!(digest_field(assessment(payload), "digest"), digest);
    let result = assessment(payload);
    assert!(result.contains("\"selector\":\"first-published\""));
    assert!(result.contains("\"n_members\":2,\"n_resolved\":1,\"n_unresolved\":1,"));
    assert!(result.contains("\"interval_score\":{\"n_scored\":1,\"observed_mean\":10,"));
    assert!(result.contains("\"kind\":\"published\""));
    assert!(result.contains("\"status\":\"resolved\",\"value\":45,"));
    assert!(result.contains("\"status\":\"unresolved\",\"reason\":\"not_published\""));
    assert_eq!(journal.bytes(), before);

    // A later writer can pin the same numerical result, but cannot retroactively pin its earlier prefix.
    let original_head = journal.head;
    journal.write(
        Command::Assess {
            journal: JOURNAL.into(),
            head: journal.head,
            cohort: "two-sites".into(),
            as_of: LATE_CUT.into(),
            selector: "first-published".into(),
            expected_digest: Some(digest),
        },
        ns("2031-02-01T00:00:00Z"),
    );
    assert_ne!(journal.head, original_head);
    let after = journal.bytes();
    let reopened = journal.report(digest, Some(journal.head));
    assert_eq!(reopened.exit_code, 0, "{reopened:?}");
    let current = &reopened.payload_json;
    assert_eq!(nested_report(current), nested_report(payload));
    assert_eq!(
        digest_field(nested_report(current), "journal_head"),
        original_head
    );
    assert_eq!(digest_field(current, "head"), journal.head);
    assert!(current.contains("\"n_assessments\":4"));
    let retrieval = current
        .split_once("\"selected_assessment_digest\":")
        .expect("historical retrieval receipt")
        .1;
    assert_eq!(
        string_field(retrieval, "result_digest_binding"),
        "caller_requested_digest_only"
    );
    let checkpoints = current
        .split_once("\"assessments\":[")
        .expect("replayed checkpoint inventory")
        .1
        .split_once(']')
        .expect("inventory end")
        .0;
    let pinned_duplicate = checkpoints.rsplit_once("},{").expect("later duplicate").1;
    assert_eq!(digest_field(pinned_duplicate, "digest"), digest);
    assert_eq!(
        string_field(pinned_duplicate, "result_digest_binding"),
        "persisted_checkpoint"
    );
    assert_eq!(journal.bytes(), after);
}

#[test]
fn forecast_journal_commands_require_explicit_admission_policy_and_exact_heads() {
    let _fixture = isolated_fixture();
    let root = workspace_root();
    let head = "ab".repeat(32);
    let (command, format, selected_root) = parse_args(
        &args(&[
            "--format",
            "markdown",
            "forecast",
            "--workspace",
            "runs",
            "journal",
            "init",
            "--journal",
            "forecasts.journal",
            "--cell",
            "forecast-demo",
            "--trust-host-clock",
            "--max-clock-skew-ms",
            "2",
        ]),
        &root,
    )
    .expect("explicit clock policy");
    assert_eq!(format, OutputFormat::Markdown);
    assert_eq!(selected_root, Path::new("runs"));
    assert!(
        matches!(command, CliCommand::ForecastJournal(Command::Init {
        cell_id, clock_skew_ns: 2_000_000, ..
    }) if cell_id == "forecast-demo")
    );
    for operation in ["register", "freeze", "outcome"] {
        let (command, _, _) = parse_args(
            &args(&[
                "forecast",
                "journal",
                operation,
                "--journal",
                "forecasts.journal",
                "--head",
                &head,
                "--request",
                "request.toml",
                "--trust-host-clock",
            ]),
            &root,
        )
        .expect("typed live operation");
        assert!(matches!(command, CliCommand::ForecastJournal(_)));
    }
    let (command, _, _) = parse_args(
        &args(&[
            "forecast",
            "journal",
            "assess",
            "--journal",
            "forecasts.journal",
            "--head",
            &head,
            "--cohort",
            "demo",
            "--as-of",
            "2026-11-01T00:00:00Z",
            "--trust-host-clock",
        ]),
        &root,
    )
    .expect("persisted assessment");
    assert!(
        matches!(command, CliCommand::ForecastJournal(Command::Assess {
        selector, expected_digest: None, ..
    }) if selector == "first-published")
    );
    let (command, _, _) = parse_args(
        &args(&[
            "forecast",
            "journal",
            "inspect",
            "--journal",
            "forecasts.journal",
        ]),
        &root,
    )
    .expect("unpinned historical inspection");
    assert!(matches!(
        command,
        CliCommand::ForecastJournal(Command::Inspect { head: None, .. })
    ));
    let docs = execute_command(&CliCommand::RobotDocs, &root);
    for operation in ["init", "register", "freeze", "outcome", "assess", "inspect"] {
        assert!(
            docs.payload_json
                .contains(&format!("forecast journal {operation}"))
        );
    }
}

#[test]
fn forecast_journal_usage_refuses_backdating_flags_ignored_options_and_clock_ambiguity() {
    let _fixture = isolated_fixture();
    let root = workspace_root();
    let head = "ab".repeat(32);
    let base = args(&[
        "forecast",
        "journal",
        "register",
        "--journal",
        "forecasts.journal",
        "--head",
        &head,
        "--request",
        "request.toml",
        "--trust-host-clock",
    ]);
    let mut no_clock = base.clone();
    no_clock.pop();
    assert!(parse_args(&no_clock, &root).is_err());
    for extras in [
        vec!["--registered-at", "2025-01-01T00:00:00Z"],
        vec!["--head", &head],
        vec!["--max-clock-skew-ms", "0"],
        vec!["--issuer", "barrick"],
        vec!["--json", "--format", "markdown"],
        vec!["--provenance", "unverified.toml"],
    ] {
        let mut bad = base.clone();
        bad.extend(args(&extras));
        let (envelope, _) = invoke(&bad, &root);
        assert_eq!(
            (envelope.verb.as_str(), envelope.exit_code),
            ("forecast journal", 2)
        );
    }
    for bad in [
        vec![
            "forecast",
            "journal",
            "init",
            "--journal",
            "x",
            "--cell",
            "demo",
            "--trust-host-clock",
            "--max-clock-skew-ms",
            "18446744073709551615",
        ],
        vec![
            "forecast",
            "journal",
            "inspect",
            "--journal",
            "x",
            "--trust-host-clock",
        ],
        vec![
            "forecast",
            "journal",
            "inspect",
            "--journal",
            "x",
            "--head",
            "not-a-digest",
        ],
        vec!["forecast", "journal", "inspect", "--journal", ""],
    ] {
        assert!(parse_args(&args(&bad), &root).is_err(), "{bad:?}");
    }
    for mut other in [
        args(&["forecast", "journal", "inspect", "--journal", "x"]),
        args(&[
            "forecast",
            "journal",
            "assess",
            "--journal",
            "x",
            "--head",
            &head,
            "--cohort",
            "demo",
            "--as-of",
            "2026-11-01T00:00:00Z",
            "--trust-host-clock",
        ]),
    ] {
        other.extend(args(&[
            "--timestamp-head",
            &head,
            "--timestamp-receipt",
            "proof.tsr",
            "--timestamp-roots",
            "roots.pem",
        ]));
        assert!(
            parse_args(&other, &root).is_err(),
            "report-only timestamp options: {other:?}"
        );
    }
}

#[test]
fn saved_report_cli_routes_without_accepting_mutation_or_cut_flags() {
    let _fixture = isolated_fixture();
    let (journal, early, _) = saved_reports();
    let before = journal.bytes();
    let selected = digest_field(assessment(&early), "digest");
    let digest = selected.to_hex();
    let root = journal.root.to_str().expect("synthetic UTF-8 path");
    let base = args(&["forecast", "journal", "report", "--journal", JOURNAL]);
    let mut selected_args = base.clone();
    selected_args.extend(args(&["--assessment", &digest]));

    for (format_name, expected_format, before_verb) in [
        ("json", OutputFormat::Json, true),
        ("markdown", OutputFormat::Markdown, false),
    ] {
        let globals = args(&["--workspace", root, "--format", format_name]);
        let mut invocation = if before_verb {
            [globals, selected_args.clone()].concat()
        } else {
            [selected_args.clone(), globals].concat()
        };
        let expected_head = if before_verb {
            None
        } else {
            Some(journal.head)
        };
        if let Some(pin) = expected_head {
            invocation.extend(args(&["--head", &pin.to_hex()]));
        }
        let (command, format, selected_root) =
            parse_args(&invocation, Path::new(".")).expect("read-only report route");
        assert_eq!(format, expected_format);
        assert_eq!(selected_root, journal.root);
        assert!(
            matches!(command, CliCommand::ForecastJournal(Command::Report {
            journal: path, head: actual_head, assessment: actual,
            provenance: None,
            timestamp: None,
        }) if path == Path::new(JOURNAL) && actual_head == expected_head && actual == selected)
        );
        let (envelope, format) = invoke(&invocation, Path::new("."));
        assert_eq!(envelope.exit_code, 0, "{envelope:?}");
        assert_eq!(format, expected_format);
        assert_eq!(assessment(&envelope.payload_json), assessment(&early));
        assert_eq!(digest_field(&envelope.payload_json, "head"), journal.head);
        assert_eq!(journal.bytes(), before);
    }

    for extras in [
        vec!["--trust-host-clock"],
        vec!["--as-of", EARLY_CUT],
        vec!["--selector", "first-published"],
        vec!["--request", "request.toml"],
        vec!["--max-clock-skew-ms", "0"],
        vec!["--recorded-at", EARLY_CUT],
        vec!["--assessment", &digest],
        vec!["--unknown", "value"],
        vec!["--timestamp-head", &digest],
        vec!["--timestamp-receipt", "proof.tsr"],
        vec!["--timestamp-roots", "roots.pem"],
        vec!["--timestamp-intermediates", "chain.pem"],
        vec![
            "--timestamp-head",
            &digest,
            "--timestamp-receipt",
            "proof.tsr",
        ],
        vec![
            "--timestamp-head",
            &digest,
            "--timestamp-roots",
            "roots.pem",
        ],
        vec![
            "--timestamp-receipt",
            "proof.tsr",
            "--timestamp-roots",
            "roots.pem",
        ],
        vec![
            "--timestamp-head",
            "not-a-digest",
            "--timestamp-receipt",
            "proof.tsr",
            "--timestamp-roots",
            "roots.pem",
        ],
        vec![
            "--timestamp-head",
            &digest,
            "--timestamp-receipt",
            "",
            "--timestamp-roots",
            "roots.pem",
        ],
    ] {
        let mut bad = selected_args.clone();
        bad.extend(args(&extras));
        let (envelope, _) = invoke(&bad, &journal.root);
        assert_eq!(envelope.exit_code, 2, "{extras:?}: {envelope:?}");
        assert_eq!(journal.bytes(), before);
    }
    for selection in [
        vec![],
        vec!["--assessment"],
        vec!["--assessment", ""],
        vec!["--assessment", "not-a-digest"],
    ] {
        let mut bad = base.clone();
        bad.extend(args(&selection));
        let (envelope, _) = invoke(&bad, &journal.root);
        assert_eq!(envelope.exit_code, 2, "{selection:?}: {envelope:?}");
        assert_eq!(journal.bytes(), before);
    }

    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (docs, _) = invoke(&args(&["robot-docs"]), &workspace);
    assert_eq!(docs.exit_code, 0, "{docs:?}");
    assert!(
        docs.payload_json
            .contains("forecast journal report --journal")
    );
    let registry = include_str!("../../../registries/verbs.toml");
    let entry = registry
        .split("[[verb]]")
        .find(|entry| entry.contains("path = \"forecast journal report\""))
        .expect("registered saved-report verb");
    assert!(entry.contains("mutates_authority = false"), "{entry}");
    assert_eq!(journal.bytes(), before);
}

// Reuse the real local TSA setup from forecast_record_score; the authority is explicitly synthetic.
// There is one arbitrary-digest signer, with no calendar window or rules-before-record sleep.
fn openssl(tsa: &Path, args: &str) {
    let output = ProcessCommand::new("openssl")
        .current_dir(tsa)
        .args(args.split_ascii_whitespace())
        .env("LC_ALL", "C")
        .output()
        .expect("OpenSSL is required by the offline timestamp verifier");
    assert!(
        output.status.success(),
        "openssl {args}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn timestamp_authority(root: &Path) -> PathBuf {
    let tsa = root.join("tsa");
    fs::create_dir(&tsa).expect("new synthetic authority");
    fs::write(
        tsa.join("tsa.cnf"),
        r#"[ tsa_config ]
serial = ./serial
crypto_device = builtin
signer_digest = sha256
default_policy = 1.2.3.4.1
digests = sha256
accuracy = secs:0
ordering = no
tsa_name = no
ess_cert_id_chain = no
ess_cert_id_alg = sha256
[ req ]
distinguished_name = dn
prompt = no
[ dn ]
CN = synthetic_journal_tsa
[ tsa_ext ]
basicConstraints = CA:FALSE
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, timeStamping
[ ca_ext ]
basicConstraints = critical, CA:TRUE
keyUsage = critical, keyCertSign, cRLSign
"#,
    )
    .expect("synthetic TSA configuration");
    fs::write(tsa.join("serial"), "01\n").expect("fresh TSA serial");
    for command in [
        "req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.pem -days 2 -subj /CN=synthetic_journal_root -config tsa.cnf -extensions ca_ext",
        "req -newkey rsa:2048 -nodes -keyout tsa.key -out tsa.csr -subj /CN=synthetic_journal_tsa -config tsa.cnf",
        "x509 -req -in tsa.csr -CA ca.pem -CAkey ca.key -CAcreateserial -out tsa.pem -days 2 -extfile tsa.cnf -extensions tsa_ext",
        "req -x509 -newkey rsa:2048 -nodes -keyout wrong.key -out wrong.pem -days 2 -subj /CN=unrelated_journal_root -config tsa.cnf -extensions ca_ext",
    ] {
        openssl(&tsa, command);
    }
    tsa
}

fn sign_timestamp(tsa: &Path, name: &str, digest: Digest32) -> PathBuf {
    openssl(
        tsa,
        &format!(
            "ts -query -digest {} -sha256 -cert -out {name}.tsq",
            digest.to_hex()
        ),
    );
    openssl(
        tsa,
        &format!(
            "ts -reply -config tsa.cnf -section tsa_config -queryfile {name}.tsq -inkey tsa.key -signer tsa.pem -out {name}.tsr"
        ),
    );
    tsa.join(format!("{name}.tsr"))
}

fn prefix_imprint(head: Digest32, sequence: u64) -> Digest32 {
    // Independent protocol framing: this is not the SHA-256 of the journal, or of the raw BLAKE3 head.
    sha256(
        format!(
            "fmine.forecast-journal-prefix.v1\nhead={}\nsequence={sequence}\n",
            head.to_hex()
        )
        .as_bytes(),
    )
}

const DAY: i64 = 86_400;

fn timestamp_cohort(journal: &mut Journal, epoch: i64, missing_due: i64) -> Digest32 {
    for (alias, due, at) in [
        ("f1", epoch + DAY, epoch - 3 * DAY),
        ("f2", missing_due, epoch - 3 * DAY + 10),
    ] {
        journal.append_at(&format!("[[forecast]]\nid = \"{alias}\"\nmodel_key = \"synthetic:{alias}\"\noutcome_due = \"{}\"\nkind = \"interval\"\nlo = \"40\"\nhi = \"50\"\nnominal = \"0.8\"\n", format_utc(due)), at * SECOND);
    }
    let before_freeze = journal.head;
    journal.append_at("[[cohort]]\nname = \"two-sites\"\nmembers = \"f1,f2\"\nresolution_rule = \"Invented observations\"\neligible_sources = \"synthetic:issuer\"\nalpha = \"0.2\"\noutcome_lo = \"0\"\noutcome_hi = \"100\"\n", (epoch - 2 * DAY) * SECOND);
    before_freeze
}

fn timestamp_outcomes(journal: &mut Journal, epoch: i64, missing_due: i64) -> (String, Digest32) {
    journal.append_at(&format!("[[outcome]]\nforecast = \"f2\"\nkind = \"missing\"\navailable_at = \"{}\"\nreason = \"not_published\"\n", format_utc(missing_due + 10)), (epoch + DAY + 20) * SECOND);
    let missing_prefix = journal.head;
    journal.append_at(&format!("[[outcome]]\nforecast = \"f1\"\nkind = \"published\"\navailable_at = \"{}\"\nvalue = \"45\"\nsource = \"synthetic:issuer\"\nvintage = \"invented\"\nlocator = \"Invented!A1\"\nsha256 = \"{}\"\n", format_utc(epoch + DAY + 10), "11".repeat(32)), (epoch + DAY + 30) * SECOND);
    (
        journal.assess(&format_utc(epoch + DAY + 60), "first-published"),
        missing_prefix,
    )
}

#[test]
fn signed_prefix_proofs_survive_appends_and_refuse_hindsight_without_mutating_history() {
    let _fixture = isolated_fixture();
    let mut journal = Journal::new_at(0);
    let tsa = timestamp_authority(&journal.root);
    let roots = tsa.join("ca.pem");
    let probe_digest = sha256(b"synthetic timestamp fixture epoch, not a journal proof");
    let probe = sign_timestamp(&tsa, "epoch", probe_digest);
    let epoch = verify_rfc3161(&probe, &roots, probe_digest)
        .expect("authenticated fixture epoch")
        .times
        .upper_utc;
    let missing_due = epoch + DAY / 2;
    let before_freeze = timestamp_cohort(&mut journal, epoch, missing_due);
    let frozen_head = journal.head;
    let imprint = prefix_imprint(frozen_head, 3);
    let reader = DurableJournal::open_read(&journal.root.join(JOURNAL), Some(frozen_head))
        .expect("frozen prefix");
    let frozen = commitment(&reader, frozen_head).expect("known prefix");
    assert_eq!(
        frozen,
        PrefixCommitment::for_record(&reader.records()[3]).expect("canonical record")
    );
    assert_eq!((frozen.sequence, frozen.imprint_sha256), (3, imprint));
    assert_ne!(imprint, sha256(&journal.bytes()));
    drop(reader);
    let receipt = sign_timestamp(&tsa, "frozen", imprint);
    let proof_time = verify_rfc3161(&receipt, &roots, imprint).expect("real signed prefix");
    assert!(proof_time.times.upper_utc < missing_due);
    let receipt_bytes = fs::read(&receipt).expect("signed response");
    let root_bytes = fs::read(&roots).expect("selected synthetic trust root");
    let proof = |prefix_head, receipt: PathBuf| TimestampProof {
        prefix_head,
        receipt,
        trust_roots: roots.clone(),
        intermediates: None,
    };

    // The proof exists before any invented outcome is appended; later reports retain its exact old prefix.
    let (saved, missing_prefix) = timestamp_outcomes(&mut journal, epoch, missing_due);
    let digest = digest_field(assessment(&saved), "digest");
    assert!(
        assessment(&saved).contains("\"interval_score\":{\"n_scored\":1,\"observed_mean\":10,")
    );
    assert!(assessment(&saved).contains("\"n_members\":2,\"n_resolved\":1,\"n_unresolved\":1,"));
    let before = journal.bytes();
    let checked = journal.report_with_timestamp(digest, proof(frozen_head, receipt.clone()));
    assert_eq!(checked.exit_code, 0, "{checked:?}");
    assert_eq!(nested_report(&checked.payload_json), nested_report(&saved));
    assert_eq!(
        string_field(&checked.payload_json, "external_time_proof"),
        "verified_caller_selected_roots"
    );
    let verification = checked
        .payload_json
        .split_once("\"timestamp_verification\":")
        .expect("supplemental verification")
        .1
        .split_once(",\"source_citations\":")
        .expect("verification end")
        .0;
    assert_eq!(digest_field(verification, "head"), frozen_head);
    assert_eq!(digest_field(verification, "imprint_sha256"), imprint);
    assert_eq!(
        digest_field(verification, "receipt_sha256"),
        sha256(&receipt_bytes)
    );
    assert_eq!(
        digest_field(verification, "trust_roots_sha256"),
        sha256(&root_bytes)
    );
    for fragment in [
        "\"status\":\"verified\"".to_string(),
        "\"trust_basis\":\"caller_selected_roots\"".to_string(),
        "\"sequence\":3".to_string(),
        "\"n_members\":2".to_string(),
        "\"publication_scope\":\"current_journal_declarations\"".to_string(),
        "\"host_clock\":\"not_verified\"".to_string(),
        "\"source_truth\":\"not_verified\"".to_string(),
        format!("\"earliest_deadline_ns\":{}", missing_due * SECOND),
        format!(
            "\"earliest_recorded_publication_ns\":{}",
            (epoch + DAY + 10) * SECOND
        ),
        format!("\"lower_seconds\":{}", proof_time.times.lower_utc),
        format!("\"upper_seconds\":{}", proof_time.times.upper_utc),
    ] {
        assert!(
            verification.contains(&fragment),
            "missing {fragment}: {verification}"
        );
    }
    assert!(checked.anchor_id.is_none() && checked.certificate.is_none());
    for flag in [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert!(checked.to_json().contains(&format!("\"{flag}\":false")));
    }
    assert!(
        checked
            .rendered_markdown
            .as_deref()
            .expect("proof explanation")
            .contains("caller-selected trust roots")
    );
    assert_eq!(journal.bytes(), before);

    let query = args(&[
        "forecast",
        "journal",
        "report",
        "--journal",
        JOURNAL,
        "--assessment",
        &digest.to_hex(),
        "--head",
        &journal.head.to_hex(),
        "--timestamp-head",
        &frozen_head.to_hex(),
        "--timestamp-receipt",
        receipt.to_str().expect("receipt path"),
        "--timestamp-roots",
        roots.to_str().expect("root path"),
    ]);
    let routed = invoke(&query, &journal.root).0;
    assert_eq!(routed.to_json(), checked.to_json());
    assert_eq!(journal.bytes(), before);

    journal.append_at(&format!("[[outcome]]\nforecast = \"f1\"\nkind = \"restated\"\navailable_at = \"{}\"\nvalue = \"55\"\nsource = \"synthetic:issuer\"\nvintage = \"invented-later\"\nlocator = \"Invented!A2\"\nsha256 = \"{}\"\nsupersedes = 1\n", format_utc(epoch + DAY + 100), "22".repeat(32)), (epoch + DAY + 120) * SECOND);
    let later = journal.assess(&format_utc(epoch + DAY + 180), "latest-as-of");
    assert!(
        assessment(&later).contains("\"interval_score\":{\"n_scored\":1,\"observed_mean\":60,")
    );
    let before = journal.bytes();
    for selected in [digest, digest_field(assessment(&later), "digest")] {
        let replayed = journal.report_with_timestamp(selected, proof(frozen_head, receipt.clone()));
        assert_eq!(replayed.exit_code, 0, "{replayed:?}");
        let original = if selected == digest { &saved } else { &later };
        assert_eq!(
            nested_report(&replayed.payload_json),
            nested_report(original)
        );
        assert_eq!(digest_field(&replayed.payload_json, "head"), journal.head);
        let current = replayed
            .payload_json
            .split_once("\"timestamp_commitment\":")
            .expect("current commitment")
            .1
            .split_once(",\"timestamp_verification\":")
            .expect("commitment end")
            .0;
        assert_eq!(digest_field(current, "head"), journal.head);
        assert_eq!(
            digest_field(current, "imprint_sha256"),
            prefix_imprint(journal.head, 8)
        );
        assert_eq!(journal.bytes(), before);
    }

    let wrong_imprint = sign_timestamp(&tsa, "wrong-imprint", sha256(b"another signed message"));
    let before_cohort = sign_timestamp(&tsa, "before-cohort", prefix_imprint(before_freeze, 2));
    let after_missing = sign_timestamp(&tsa, "after-missing", prefix_imprint(missing_prefix, 4));
    let corrupt_path = tsa.join("corrupt.tsr");
    let mut corrupt = receipt_bytes.clone();
    corrupt[0] ^= 1;
    fs::write(&corrupt_path, corrupt).expect("altered response fixture");
    for (name, request, code) in [
        (
            "wrong roots",
            TimestampProof {
                trust_roots: tsa.join("wrong.pem"),
                ..proof(frozen_head, receipt.clone())
            },
            "FMINE-REF-505",
        ),
        (
            "untrusted intermediates cannot add roots",
            TimestampProof {
                trust_roots: tsa.join("wrong.pem"),
                intermediates: Some(roots.clone()),
                ..proof(frozen_head, receipt.clone())
            },
            "FMINE-REF-505",
        ),
        (
            "wrong imprint",
            proof(frozen_head, wrong_imprint),
            "FMINE-REF-505",
        ),
        (
            "altered response",
            proof(frozen_head, corrupt_path),
            "FMINE-REF-505",
        ),
        (
            "unknown prefix",
            proof(Digest32([0x5a; 32]), receipt.clone()),
            "FMINE-REF-502",
        ),
        (
            "cohort not yet frozen",
            proof(before_freeze, before_cohort),
            "FMINE-REF-206",
        ),
        (
            "unresolved outcome already recorded",
            proof(missing_prefix, after_missing),
            "FMINE-REF-206",
        ),
    ] {
        let refused = journal.report_with_timestamp(digest, request);
        assert_eq!(
            refused.refusal.expect("typed timestamp refusal").code,
            code,
            "{name}"
        );
        assert_eq!(journal.bytes(), before, "{name}");
    }

    // Backdated local admissions cannot overcome the unresolved member's earlier deadline.
    let mut late = Journal::new_at(0);
    timestamp_cohort(&mut late, epoch, epoch - DAY);
    let late_head = late.head;
    let late_receipt = sign_timestamp(&tsa, "late", prefix_imprint(late_head, 3));
    let (late_saved, _) = timestamp_outcomes(&mut late, epoch, epoch - DAY);
    assert!(
        assessment(&late_saved).contains("\"n_members\":2,\"n_resolved\":1,\"n_unresolved\":1,")
    );
    let before = late.bytes();
    let refused = late.report_with_timestamp(
        digest_field(assessment(&late_saved), "digest"),
        proof(late_head, late_receipt),
    );
    let refusal = refused.refusal.expect("authenticated late registration");
    assert_eq!(refusal.code, "FMINE-REF-206");
    assert!(refusal.what.contains("deadline"), "{}", refusal.what);
    assert_eq!(late.bytes(), before);
    assert_eq!(
        fs::read(receipt).expect("original receipt retained"),
        receipt_bytes
    );
    assert_eq!(fs::read(roots).expect("caller roots retained"), root_bytes);
}

const OBSERVATION: &str = "evd_11111111111111111111111111111111";
const FITTED: &str = "mod_22222222222222222222222222222222";
const EXCLUSION: &str = "evd_33333333333333333333333333333333";
const DECK: &str = "evd_44444444444444444444444444444444";
const BRIDGE: &str = "clm_55555555555555555555555555555555";

fn registration<'a>(report: &'a str, alias: &str) -> (&'a str, &'a str) {
    let forecast = report
        .split_once(&format!("\"alias\":\"{alias}\""))
        .expect("registered forecast")
        .1;
    (
        string_field(forecast, "forecast_id"),
        string_field(forecast, "registered_at"),
    )
}

fn provenance_bundle(report: &str, changed_time: Option<(&str, &str)>) -> String {
    let mut bundle =
        String::from("schema = \"fmine.forecast-provenance.v1\"\nlane = \"supplied\"\n");
    let bridge_parents = format!("{FITTED}, {EXCLUSION}, {DECK}");
    for (index, (id, kind, field, at, parents)) in [
        (
            OBSERVATION,
            "observation",
            "available_at",
            "2029-12-31T23:59:59Z",
            "",
        ),
        (
            FITTED,
            "fitted_parameters",
            "performed_at",
            "2030-01-01T00:00:00Z",
            OBSERVATION,
        ),
        (
            EXCLUSION,
            "exclusion_act",
            "performed_at",
            "2030-01-01T00:00:00Z",
            OBSERVATION,
        ),
        (
            DECK,
            "price_deck",
            "available_at",
            "2030-01-01T00:00:00Z",
            "",
        ),
        (
            BRIDGE,
            "claim",
            "performed_at",
            "2030-01-01T00:00:00Z",
            bridge_parents.as_str(),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let at = changed_time
            .filter(|(node, _)| *node == id)
            .map_or(at, |(_, at)| at);
        let origin = if field == "available_at" {
            "source"
        } else {
            "derived"
        };
        bundle.push_str(&format!(
            "\n[[node]]\nid = \"{id}\"\nkind = \"{kind}\"\norigin = \"{origin}\"\n{field} = \"{at}\"\nrecord_cell = \"inputs\"\nseq = {}\nrecorded_at = \"2030-01-01T00:00:00Z\"\n", index + 1,
        ));
        if !parents.is_empty() {
            bundle.push_str(&format!("parents = \"{parents}\"\n"));
        }
    }
    for (seq, alias) in [(1, "f1"), (2, "f2")] {
        let (id, at) = registration(report, alias);
        bundle.push_str(&format!(
            "\n[[anchor]]\nid = \"{alias}\"\ncell = \"inputs\"\nseq = 10\n\n[[anchor]]\nid = \"{alias}\"\ncell = \"synthetic-report-cell\"\nseq = {seq}\n\n[[node]]\nid = \"{id}\"\nkind = \"forecast\"\norigin = \"derived\"\nperformed_at = \"{at}\"\nparents = \"{FITTED}, {BRIDGE}\"\nrecord_cell = \"synthetic-report-cell\"\nseq = {seq}\nrecorded_at = \"{at}\"\n\n[[binding]]\nforecast = \"{alias}\"\nforecast_id = \"{id}\"\nanchor = \"{alias}\"\n",
        ));
    }
    bundle
}

fn checked_report(journal: &Journal, saved: &str, bundle: &str) -> Envelope {
    let path = journal.root.join("provenance.toml");
    fs::write(&path, bundle).expect("supplied synthetic ancestry");
    let before = journal.bytes();
    let envelope = journal.report_with_provenance(
        digest_field(assessment(saved), "digest"),
        Some(journal.head),
        Some("provenance.toml".into()),
    );
    assert_eq!(
        journal.bytes(),
        before,
        "provenance checks never extend history"
    );
    assert_eq!(
        fs::read(path).expect("retained provenance"),
        bundle.as_bytes()
    );
    envelope
}

#[test]
fn saved_report_checks_fractional_registration_provenance_without_changing_results() {
    let _fixture = isolated_fixture();
    let (journal, early, latest) = saved_reports_with_fraction(123_456_789);
    let bundle = provenance_bundle(&early, None);
    for saved in [&early, &latest] {
        let envelope = checked_report(&journal, saved, &bundle);
        assert_eq!(envelope.exit_code, 0, "{envelope:?}");
        assert_eq!(assessment(&envelope.payload_json), assessment(saved));
        assert_eq!(digest_field(&envelope.payload_json, "head"), journal.head);
        assert_eq!(
            digest_field(&envelope.payload_json, "provenance_sha256"),
            sha256(bundle.as_bytes())
        );
        for fragment in [
            "\"status\":\"checked\"",
            "\"scope\":\"declared_ancestry_at_registration\"",
            "\"time_evidence\":\"supplied_not_verified\"",
            "\"generator_replay\":\"not_performed\"",
            "\"external_time_proof\":\"not_verified\"",
            "\"forecast\":\"f1\"",
            "\"forecast\":\"f2\"",
            "\"decision_cut\":\"2030-01-02T00:00:01.123456789Z\"",
            "\"decision_cut\":\"2030-01-03T00:00:01.123456789Z\"",
        ] {
            assert!(
                envelope.payload_json.contains(fragment),
                "missing {fragment}: {envelope:?}"
            );
        }
        assert_eq!(envelope.certificate, None);
        assert_eq!(envelope.anchor_id, None);
        for flag in [
            "reserve_disclosure_claim",
            "execution_authorized",
            "geotechnical_signoff",
            "regulatory_compliance_claim",
        ] {
            assert!(envelope.to_json().contains(&format!("\"{flag}\":false")));
        }
        assert!(
            envelope
                .rendered_markdown
                .expect("readable report")
                .contains("Declared ancestry was checked")
        );
    }
    // The public parser carries the supplied bundle through to this same read-only path.
    let invocation = args(&[
        "forecast",
        "journal",
        "report",
        "--journal",
        JOURNAL,
        "--assessment",
        &digest_field(assessment(&early), "digest").to_hex(),
        "--provenance",
        "provenance.toml",
        "--head",
        &journal.head.to_hex(),
    ]);
    let (parsed, _, root) =
        parse_args(&invocation, &journal.root).expect("provenance report route");
    let CliCommand::ForecastJournal(command) = parsed else {
        panic!("journal report route")
    };
    let routed = execute_with_clock(&command, &root, &NeverClock);
    assert_eq!(routed.exit_code, 0, "{routed:?}");
    assert_eq!(assessment(&routed.payload_json), assessment(&early));
    for operation in ["inspect", "assess", "register"] {
        assert!(
            parse_args(
                &args(&[
                    "forecast",
                    "journal",
                    operation,
                    "--journal",
                    JOURNAL,
                    "--provenance",
                    "provenance.toml"
                ]),
                &journal.root
            )
            .is_err()
        );
    }
}

#[test]
fn saved_report_provenance_retains_minimal_frontiers_and_checks_unresolved_members() {
    let _fixture = isolated_fixture();
    let (journal, early, _) = saved_reports_with_fraction(123_456_789);
    let (root, _) = registration(&early, "f1");
    // V2 registration is the observed instant plus this journal's one-second uncertainty bound.
    let future = "2030-01-02T00:00:01.123456790Z";
    for (node, suffix) in [
        (OBSERVATION, vec![FITTED, OBSERVATION]),
        (FITTED, vec![FITTED]),
        (EXCLUSION, vec![BRIDGE, EXCLUSION]),
        (DECK, vec![BRIDGE, DECK]),
    ] {
        let bundle = provenance_bundle(&early, Some((node, future)));
        let refused = checked_report(&journal, &early, &bundle);
        assert_eq!(
            refused
                .refusal
                .as_ref()
                .expect("typed temporal refusal")
                .code,
            "FMINE-REF-110"
        );
        let path = std::iter::once(root)
            .chain(suffix)
            .map(|id| format!("\"{id}\""))
            .collect::<Vec<_>>()
            .join(",");
        for fragment in [
            "\"status\":\"refused\"".to_string(),
            "\"forecast\":\"f1\"".to_string(),
            format!("\"node\":\"{node}\""),
            format!("\"available_at\":\"{future}\""),
            "\"after_cut\":true".to_string(),
            format!("\"path\":[{path}]"),
        ] {
            assert!(
                refused.payload_json.contains(&fragment),
                "missing {fragment}: {refused:?}"
            );
        }
    }
    let bundle = provenance_bundle(&early, None);
    let (unresolved, _) = registration(&early, "f2");
    let missing_binding = bundle.replace(
        &format!(
            "[[binding]]\nforecast = \"f2\"\nforecast_id = \"{unresolved}\"\nanchor = \"f2\"\n"
        ),
        "",
    );
    let missing = checked_report(&journal, &early, &missing_binding);
    let refusal = missing
        .refusal
        .expect("unresolved member still needs ancestry");
    assert_eq!(refusal.code, "FMINE-REF-110");
    assert!(refusal.what.contains("f2"));
    for (bad, code) in [
        (
            bundle.replace("available_at = \"2029-12-31T23:59:59Z\"", ""),
            "FMINE-REF-110",
        ),
        (
            bundle.replace(
                &format!("forecast_id = \"{root}\""),
                "forecast_id = \"for_00000000000000000000000000000000\"",
            ),
            "FMINE-REF-110",
        ),
        (
            bundle.replace("lane = \"supplied\"", "lane = \"synthetic\""),
            "FMINE-REF-107",
        ),
        (
            bundle.replace(
                "performed_at = \"2030-01-02T00:00:01.123456789Z\"",
                "performed_at = \"2030-01-02T00:00:01.123456788Z\"",
            ),
            "FMINE-REF-110",
        ),
    ] {
        let refused = checked_report(&journal, &early, &bad);
        assert_eq!(
            refused.refusal.expect("invalid ancestry refuses").code,
            code
        );
    }
}

#[test]
fn provenance_fractional_times_preserve_nanoseconds_and_refuse_ambiguous_or_overflowing_dates() {
    let _fixture = isolated_fixture();
    let (journal, early, _) = saved_reports_with_fraction(123_456_789);
    let mut cones = std::collections::BTreeSet::new();
    for digits in 1..=9 {
        let at = format!("2030-01-02T00:00:01.{}Z", &"123456789"[..digits]);
        let bundle = provenance_bundle(&early, Some((OBSERVATION, &at)));
        let envelope = checked_report(&journal, &early, &bundle);
        assert_eq!(envelope.exit_code, 0, "{envelope:?}");
        cones.insert(string_field(&envelope.payload_json, "cone_digest").to_string());
    }
    assert_eq!(
        cones.len(),
        9,
        "fractional values must never be truncated to seconds"
    );
    let short = provenance_bundle(&early, Some((OBSERVATION, "2030-01-02T00:00:01.1Z")));
    let padded = provenance_bundle(
        &early,
        Some((OBSERVATION, "2030-01-02T00:00:01.100000000Z")),
    );
    let a = checked_report(&journal, &early, &short);
    let b = checked_report(&journal, &early, &padded);
    assert_eq!(
        string_field(&a.payload_json, "cone_digest"),
        string_field(&b.payload_json, "cone_digest")
    );
    assert_ne!(
        string_field(&a.payload_json, "provenance_sha256"),
        string_field(&b.payload_json, "provenance_sha256")
    );
    let original = checked_report(&journal, &early, &provenance_bundle(&early, None));
    for edge in [
        "1677-09-21T00:12:43.145224192Z",
        "2262-04-11T23:47:16.854775807Z",
    ] {
        let bundle = format!(
            "{}\n[[node]]\nid = \"evd_66666666666666666666666666666666\"\nkind = \"registry_row\"\norigin = \"source\"\navailable_at = \"{edge}\"\n",
            provenance_bundle(&early, None)
        );
        let envelope = checked_report(&journal, &early, &bundle);
        assert_eq!(envelope.exit_code, 0, "{envelope:?}");
        assert_eq!(
            string_field(&envelope.payload_json, "cone_digest"),
            string_field(&original.payload_json, "cone_digest")
        );
    }
    for invalid in [
        "2030-01-02T00:00:00.Z",
        "2030-01-02T00:00:00.1234567890Z",
        "2030-01-02T00:00:00.1+00:00",
        "2030-01-02T00:00:00.1",
        "2030-01-02T00:00:00.1e2Z",
        "2030-01-02T00:00:00.1.2Z",
        "2030-01-02T00:00:00.１２Z",
        "2030-01-02T00:00:60.1Z",
        "1677-09-21T00:12:43.145224191Z",
        "2262-04-11T23:47:16.854775808Z",
    ] {
        let envelope = checked_report(
            &journal,
            &early,
            &provenance_bundle(&early, Some((OBSERVATION, invalid))),
        );
        assert_eq!(
            envelope.refusal.expect("invalid fractional UTC").code,
            "FMINE-REF-107",
            "{invalid}"
        );
    }
    // Fractional input support is confined to provenance, not a reinterpretation of FJR1 operation grammar.
    fs::write(journal.root.join("fractional-due.toml"), "[[forecast]]\nid = \"new\"\nmodel_key = \"synthetic\"\noutcome_due = \"2035-01-01T00:00:00.1Z\"\nkind = \"bernoulli\"\nprobability = \"0.5\"\n").expect("incompatible operation fixture");
    let before = journal.bytes();
    let refused = execute_with_clock(
        &Command::Register {
            journal: JOURNAL.into(),
            head: journal.head,
            request: "fractional-due.toml".into(),
        },
        &journal.root,
        &FixedClock(ns(LATE_CUT) + 2 * SECOND),
    );
    assert_eq!(
        refused.refusal.expect("original journal grammar").code,
        "FMINE-REF-107"
    );
    assert_eq!(journal.bytes(), before);
}
