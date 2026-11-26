//! Synthetic acceptance for explicit investigation of a retained monitor alarm.
//! Invented clocks, roles and evidence do not authenticate a human or establish a causal diagnosis.

#![cfg(test)]
#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_calib::record::{parse_hex32, parse_utc};
use fmine_claim::forecast_journal::{Entry, Record, validate_chain};
use fmine_cli::Envelope;
use fmine_cli::forecast_journal::{Command, execute_with_clock};
use fmine_core::hash::Digest32;
use fmine_io::forecast::Journal as DurableJournal;
use fmine_runtime::{ClockError, WallClock};

const SECOND: i64 = 1_000_000_000;
const CUT: &str = "2031-01-10T00:00:00Z";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Clock(i64);
impl WallClock for Clock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        Ok(self.0)
    }
}

struct NoClock;
impl WallClock for NoClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        panic!("historical inspection must not sample a clock")
    }
}

fn ns(text: &str) -> i64 {
    parse_utc(text).expect("synthetic UTC") * SECOND
}

fn digest_field(json: &str, field: &str) -> Digest32 {
    let value = json
        .split_once(&format!("\"{field}\":\""))
        .unwrap_or_else(|| panic!("missing {field}: {json}"))
        .1
        .split('"')
        .next()
        .expect("string field");
    Digest32(parse_hex32(value).expect("canonical digest"))
}

fn assessment(json: &str) -> &str {
    json.split_once("\"assessment\":")
        .expect("assessment")
        .1
        .split_once(",\"checkpoints\":")
        .expect("checkpoints")
        .0
}

struct Journal {
    root: PathBuf,
    path: PathBuf,
    head: Digest32,
    requests: usize,
}

impl Journal {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "fmine-investigations-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("fresh test directory");
        let path = root.join("forecasts.journal");
        let mut journal = Self {
            root,
            path,
            head: Digest32::default(),
            requests: 0,
        };
        journal.ok(
            Command::Init {
                journal: journal.path.clone(),
                cell_id: "synthetic-investigation-cell".into(),
                clock_skew_ns: 0,
            },
            "2030-01-01T00:00:00Z",
        );
        journal
    }

    fn ok(&mut self, command: Command, at: &str) -> String {
        let result = execute_with_clock(&command, &self.root, &Clock(ns(at)));
        assert_eq!(result.exit_code, 0, "{result:?}");
        self.head = digest_field(&result.payload_json, "head");
        result.payload_json
    }

    fn request(&mut self, text: &str) -> Command {
        let request = self.root.join(format!("request-{}.toml", self.requests));
        self.requests += 1;
        fs::write(&request, text).expect("synthetic request");
        let journal = self.path.clone();
        let head = self.head;
        match text.lines().next().expect("table") {
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
            "[[investigation]]" => Command::Investigate {
                journal,
                head,
                request,
            },
            "[[investigation_resolution]]" => Command::ResolveInvestigation {
                journal,
                head,
                request,
            },
            "[[investigation_rearm]]" => Command::RearmInvestigation {
                journal,
                head,
                request,
            },
            table => panic!("unexpected table {table}"),
        }
    }

    fn append(&mut self, text: &str, at: &str) -> String {
        let request = self.request(text);
        self.ok(request, at)
    }

    fn refuse(&self, command: &Command, at: &str) -> Envelope {
        let before = fs::read(&self.path).expect("before refusal");
        let result = execute_with_clock(command, &self.root, &Clock(ns(at)));
        assert_ne!(result.exit_code, 0, "{result:?}");
        assert!(result.refusal.is_some(), "{result:?}");
        assert_eq!(before, fs::read(&self.path).expect("after refusal"));
        result
    }

    fn report(&self, digest: Digest32) -> Envelope {
        let result = execute_with_clock(
            &Command::Report {
                journal: self.path.clone(),
                head: Some(self.head),
                assessment: digest,
                provenance: None,
                timestamp: None,
            },
            &self.root,
            &NoClock,
        );
        assert_eq!(result.exit_code, 0, "{result:?}");
        result
    }

    fn assess(&mut self, at: &str) -> String {
        self.ok(
            Command::Assess {
                journal: self.path.clone(),
                head: self.head,
                cohort: "original".into(),
                as_of: at.into(),
                selector: "first-published".into(),
                expected_digest: None,
            },
            at,
        )
    }
}

fn forecast(alias: &str, due: &str) -> String {
    format!(
        "[[forecast]]\nid = \"{alias}\"\nmodel_key = \"synthetic:gaussian\"\noutcome_due = \"{due}\"\nkind = \"gaussian\"\nmean = \"0\"\nstd_dev = \"1\"\n"
    )
}

fn cohort(name: &str, member: &str) -> String {
    format!(
        "[[cohort]]\nname = \"{name}\"\nmembers = \"{member}\"\nresolution_rule = \"Synthetic annual output\"\neligible_sources = \"synthetic:issuer\"\nalpha = \"0.2\"\nmin_calibration_n = 35\nrho_low = \"0\"\nrho_high = \"0.5\"\noutcome_lo = \"-10\"\noutcome_hi = \"10\"\ncusum_reference_value = \"0.5\"\ncusum_threshold = \"2\"\ncusum_support = \"Synthetic annual output\"\ncusum_reference_assumption = \"Declared Gaussian residual reference\"\ncusum_tuning_basis = \"Illustrative only; no ARL validation\"\ncusum_order = \"{member}\"\n"
    )
}

fn seeded(value: i32) -> (Journal, String, Digest32) {
    let mut journal = Journal::new();
    journal.append(
        &forecast("first", "2030-12-31T00:00:00Z"),
        "2030-01-02T00:00:00Z",
    );
    journal.append(&cohort("original", "first"), "2030-01-03T00:00:00Z");
    journal.append(
        &format!(
            "[[outcome]]\nforecast = \"first\"\nkind = \"published\"\navailable_at = \"2031-01-02T00:00:00Z\"\nvalue = \"{value}\"\nsource = \"synthetic:issuer\"\nvintage = \"original\"\nlocator = \"Synthetic!A1\"\nsha256 = \"{}\"\n",
            "11".repeat(32)
        ),
        "2031-01-03T00:00:00Z",
    );
    let saved = journal.assess(CUT);
    let digest = digest_field(assessment(&saved), "digest");
    (journal, saved, digest)
}

fn open(id: &str, digest: Digest32) -> String {
    format!(
        "[[investigation]]\nid = \"{id}\"\nassessment = \"{}\"\nmonitor = \"cusum\"\nowner_role = \"model-reviewer\"\ndeadline = \"2031-01-20T00:00:00Z\"\nhypotheses = \"HYP-1: Input measurement bias|HYP-2: Model location drift\"\nprobes = \"Check source measurements|Compare an independent sample\"\n",
        digest.to_hex()
    )
}

fn resolve(id: &str) -> String {
    format!(
        "[[investigation_resolution]]\nid = \"{id}\"\nkind = \"false_alarm_logged\"\nhuman_review = \"human\"\nhuman_role = \"review-committee\"\nrationale = \"Synthetic review records a retained false alarm\"\nevidence = \"synthetic:review-note-1\"\n"
    )
}

fn rearm(id: &str, cohort: &str) -> String {
    format!("[[investigation_rearm]]\nid = \"{id}\"\ncohort = \"{cohort}\"\n")
}

#[test]
fn cusum_alarm_opens_resolves_and_rearms_without_erasing_saved_evidence() {
    let (mut journal, original, digest) = seeded(3);
    // Independent recurrence: max(0, 0 + 3 - .5) = 2.5 >= threshold 2.
    assert!(original.contains("\"upper\":2.5,\"lower\":0,"));
    assert!(original.contains("\"first_crossing\":1,\"alarmed\":true"));
    let prefix = fs::read(&journal.path).expect("original prefix");
    let opened = journal.append(&open("case-1", digest), "2031-01-11T00:00:00Z");
    assert!(opened.contains("\"state\":\"open\""), "{opened}");
    assert!(opened.contains(&digest.to_hex()), "{opened}");
    assert!(opened.contains("HYP-1: Input measurement bias"));
    assert!(opened.contains("Check source measurements"));
    let resolved = journal.append(&resolve("case-1"), "2031-01-12T00:00:00Z");
    assert!(resolved.contains("\"state\":\"resolved\""), "{resolved}");
    assert!(resolved.contains("false_alarm_logged"));
    journal.append(
        &forecast("next", "2031-12-31T00:00:00Z"),
        "2031-01-13T00:00:00Z",
    );
    journal.append(&cohort("future", "next"), "2031-01-14T00:00:00Z");
    let rearmed = journal.append(&rearm("case-1", "future"), "2031-01-15T00:00:00Z");
    assert!(rearmed.contains("\"state\":\"rearmed\""), "{rearmed}");
    assert!(rearmed.contains("\"v18\":3"), "{rearmed}");
    assert!(
        fs::read(&journal.path)
            .expect("extended journal")
            .starts_with(&prefix)
    );
    let report = journal.report(digest);
    assert_eq!(assessment(&report.payload_json), assessment(&original));
    assert_eq!(report.to_json(), journal.report(digest).to_json());
    let before = fs::read(&journal.path).expect("before inspect");
    let inspected = execute_with_clock(
        &Command::Inspect {
            journal: journal.path.clone(),
            head: Some(journal.head),
        },
        &journal.root,
        &NoClock,
    );
    assert_eq!(inspected.exit_code, 0, "{inspected:?}");
    assert!(inspected.payload_json.contains("\"state\":\"rearmed\""));
    assert_eq!(before, fs::read(&journal.path).expect("after inspect"));
}

#[test]
fn opening_requires_a_saved_alarm_exact_monitor_and_future_deadline() {
    let (mut calm, _, no_alarm) = seeded(0);
    let command = calm.request(&open("quiet", no_alarm));
    calm.refuse(&command, "2031-01-11T00:00:00Z");
    let (mut journal, _, digest) = seeded(3);
    let valid = open("case-1", digest);
    for bad in [
        open("case-1", Digest32([9; 32])),
        valid.replace("monitor = \"cusum\"", "monitor = \"coverage\""),
        valid.replace("2031-01-20T00:00:00Z", "2031-01-11T00:00:00Z"),
        valid.replace("owner_role = \"model-reviewer\"", "owner_role = \"\""),
        format!("{valid}opened_at = \"2030-01-01T00:00:00Z\"\n"),
    ] {
        let command = journal.request(&bad);
        journal.refuse(&command, "2031-01-11T00:00:00Z");
    }
    let opened = journal.append(&valid, "2031-01-11T00:00:00Z");
    assert!(opened.contains("\"v18\":1"));
}

#[test]
fn another_assessment_cannot_reopen_the_same_original_crossing_and_stale_heads_are_atomic() {
    let (mut journal, _, digest) = seeded(3);
    let stale = journal.request(&open("stale", digest));
    journal.append(&open("case-1", digest), "2031-01-11T00:00:00Z");
    let result = journal.refuse(&stale, "2031-01-12T00:00:00Z");
    assert_eq!(result.refusal.expect("stale refusal").code, "FMINE-REF-502");
    let later = journal.assess("2031-01-12T00:00:00Z");
    let new_digest = digest_field(assessment(&later), "digest");
    assert_ne!(
        digest, new_digest,
        "the cut changed, the original crossing did not"
    );
    let duplicate = journal.request(&open("case-2", new_digest));
    journal.refuse(&duplicate, "2031-01-13T00:00:00Z");
}

#[test]
fn resolution_needs_explicit_human_review_and_rearm_needs_a_new_prospective_cohort() {
    let (mut journal, _, digest) = seeded(3);
    journal.append(&open("case-1", digest), "2031-01-11T00:00:00Z");
    let premature = journal.request(&rearm("case-1", "original"));
    journal.refuse(&premature, "2031-01-12T00:00:00Z");
    for bad in [
        resolve("case-1").replace("human_review = \"human\"", "human_review = \"agent\""),
        resolve("case-1").replace("human_review = \"human\"\n", ""),
        resolve("case-1").replace("kind = \"false_alarm_logged\"", "kind = \"silent_retrain\""),
        resolve("case-1").replace("evidence = \"synthetic:review-note-1\"", "evidence = \"\""),
        resolve("missing"),
    ] {
        let command = journal.request(&bad);
        journal.refuse(&command, "2031-01-12T00:00:00Z");
    }
    journal.append(
        &forecast("pre", "2031-12-31T00:00:00Z"),
        "2031-01-12T00:00:00Z",
    );
    journal.append(&cohort("pre-resolution", "pre"), "2031-01-13T00:00:00Z");
    journal.append(&resolve("case-1"), "2031-01-14T00:00:00Z");
    for bad in [
        resolve("case-1"),
        rearm("case-1", "original"),
        rearm("case-1", "pre-resolution"),
    ] {
        let command = journal.request(&bad);
        journal.refuse(&command, "2031-01-15T00:00:00Z");
    }
    journal.append(
        &forecast("next", "2031-12-31T00:00:00Z"),
        "2031-01-16T00:00:00Z",
    );
    journal.append(&cohort("future", "next"), "2031-01-17T00:00:00Z");
    journal.append(&rearm("case-1", "future"), "2031-01-18T00:00:00Z");
    let repeated = journal.request(&rearm("case-1", "future"));
    journal.refuse(&repeated, "2031-01-19T00:00:00Z");
}

fn write_chain(path: &Path, records: &mut [Record]) -> Digest32 {
    let mut head = Digest32::default();
    for record in records.iter_mut() {
        record.previous = head;
        head = record.chain_head().expect("synthetic frame digest");
    }
    validate_chain(records).expect("valid byte chain before semantic replay");
    let bytes: Vec<_> = records
        .iter()
        .flat_map(|record| record.to_line().expect("frame"))
        .collect();
    fs::write(path, bytes).expect("synthetic mutation");
    head
}

#[test]
fn semantic_replay_rejects_schema_downgrades_and_unrelated_upgrades_after_rechaining() {
    let (mut journal, _, digest) = seeded(3);
    journal.append(&open("case-1", digest), "2031-01-11T00:00:00Z");
    let snapshot =
        DurableJournal::open_read(&journal.path, Some(journal.head)).expect("saved journal");
    for (name, from, to) in [
        (
            "downgrade",
            "fmine.forecast-operation.v18",
            "fmine.forecast-operation.v2",
        ),
        (
            "upgrade",
            "fmine.forecast-operation.v2",
            "fmine.forecast-operation.v18",
        ),
        (
            "unknown",
            "fmine.forecast-operation.v18",
            "fmine.forecast-operation.v999",
        ),
    ] {
        let mut records = snapshot.records().to_vec();
        for record in &mut records {
            if let Entry::Operation(request) = &mut record.entry {
                *request = request.replacen(from, to, 1);
            }
        }
        let path = journal.root.join(format!("{name}.journal"));
        let head = write_chain(&path, &mut records);
        let refused = execute_with_clock(
            &Command::Inspect {
                journal: path,
                head: Some(head),
            },
            &journal.root,
            &NoClock,
        );
        assert_ne!(refused.exit_code, 0, "{name}: {refused:?}");
        assert_eq!(
            refused.refusal.expect("semantic refusal").code,
            "FMINE-REF-107"
        );
    }
}

#[test]
fn investigation_verbs_are_registered_and_require_explicit_clock_and_request_flags() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let head = "ab".repeat(32);
    let docs = fmine_cli::invoke(&["robot-docs".into()], &root).0;
    assert_eq!(docs.exit_code, 0);
    for verb in [
        "investigate",
        "resolve-investigation",
        "rearm-investigation",
    ] {
        let args: Vec<String> = [
            "forecast",
            "journal",
            verb,
            "--journal",
            "forecast.journal",
            "--head",
            &head,
            "--request",
            "request.toml",
            "--trust-host-clock",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        assert!(fmine_cli::parse_args(&args, &root).is_ok(), "{verb}");
        assert!(
            docs.payload_json
                .contains(&format!("forecast journal {verb}")),
            "{verb}"
        );
        let mut without_clock = args.clone();
        without_clock.pop();
        assert!(fmine_cli::parse_args(&without_clock, &root).is_err());
        let mut extra = args;
        extra.extend(["--recorded-at".into(), "2025-01-01T00:00:00Z".into()]);
        assert!(fmine_cli::parse_args(&extra, &root).is_err());
    }
}
