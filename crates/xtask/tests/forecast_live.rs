//! Synthetic end-to-end acceptance for the durable forecast writer, its clock boundary and replay.
//! Every command opens the journal afresh. These invented clocks/citations are not external time proof.

#![cfg(test)]
#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_calib::record::{parse_hex32, parse_utc};
use fmine_calib::{
    AssessmentPolicy, CohortSpec, ForecastLedger, ForecastPayload, OutcomeCitation, OutcomeKind,
    OutcomeSelector,
};
use fmine_claim::forecast_journal::{Entry, Record, validate_chain};
use fmine_cli::Envelope;
use fmine_cli::forecast_journal::{Command, execute_with_clock};
use fmine_core::hash::Digest32;
use fmine_io::forecast::Journal as DurableJournal;
use fmine_runtime::{ClockError, WallClock};

const SECOND: i64 = 1_000_000_000;
const DUE: &str = "2030-12-31T00:00:00Z";
const CUT: &str = "2031-01-10T00:00:00Z";
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct FixedClock(i64);

impl WallClock for FixedClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        Ok(self.0)
    }
}

fn ns(utc: &str) -> i64 {
    parse_utc(utc)
        .expect("synthetic UTC")
        .checked_mul(SECOND)
        .expect("nanoseconds")
}

// Read only the named scalar in these controlled JSON fixtures; no new JSON parser/dependency is needed.
fn string_field<'a>(json: &'a str, field: &str) -> &'a str {
    json.split_once(&format!("\"{field}\":\""))
        .unwrap_or_else(|| panic!("missing {field}: {json}"))
        .1
        .split('"')
        .next()
        .expect("string field")
}

fn head(json: &str) -> Digest32 {
    Digest32(parse_hex32(string_field(json, "head")).expect("canonical journal head"))
}

fn assessment(json: &str) -> &str {
    json.split_once("\"assessment\":")
        .expect("assessment")
        .1
        .split_once(",\"checkpoints\":")
        .expect("checkpoints")
        .0
}

fn assessment_digest(json: &str) -> Digest32 {
    Digest32(parse_hex32(string_field(assessment(json), "digest")).expect("assessment digest"))
}

fn assert_refused(envelope: &Envelope, code: &str) {
    assert_ne!(envelope.exit_code, 0, "{envelope:?}");
    assert_eq!(
        envelope.refusal.as_ref().expect("typed refusal").code,
        code,
        "{envelope:?}"
    );
}

struct Journal {
    root: PathBuf,
    path: PathBuf,
    head: Digest32,
    next_request: usize,
}

impl Journal {
    fn new(at: i64, skew: u64) -> Self {
        let root = std::env::temp_dir().join(format!(
            "fmine-forecast-live-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("fresh synthetic test directory");
        let path = root.join("forecasts.journal");
        let mut journal = Self {
            root,
            path,
            head: Digest32::default(),
            next_request: 0,
        };
        let payload = journal.ok(
            Command::Init {
                journal: journal.path.clone(),
                cell_id: "synthetic-test-cell".into(),
                clock_skew_ns: skew,
            },
            at,
        );
        assert!(payload.contains("\"n_operations\":0,"), "{payload}");
        journal
    }

    fn run(&self, command: &Command, at: i64) -> Envelope {
        execute_with_clock(command, &self.root, &FixedClock(at))
    }

    fn ok(&mut self, command: Command, at: i64) -> String {
        let envelope = self.run(&command, at);
        assert_eq!(envelope.exit_code, 0, "{envelope:?}");
        assert_eq!(envelope.status, "ok");
        assert!(
            envelope
                .payload_json
                .contains("\"schema\":\"fmine.forecast-journal.v1\"")
        );
        self.head = head(&envelope.payload_json);
        envelope.payload_json
    }

    fn request(&mut self, text: &str) -> Command {
        let request = self
            .root
            .join(format!("request-{}.toml", self.next_request));
        self.next_request += 1;
        fs::write(&request, text).expect("synthetic request");
        let journal = self.path.clone();
        let head = self.head;
        match text.lines().next().expect("request table") {
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
            other => panic!("unsupported synthetic request {other}"),
        }
    }

    fn append(&mut self, text: &str, at: i64) -> String {
        let command = self.request(text);
        self.ok(command, at)
    }

    fn refuse(&self, command: &Command, at: i64, code: &str) {
        let before = fs::read(&self.path).expect("journal bytes");
        assert_refused(&self.run(command, at), code);
        assert_eq!(fs::read(&self.path).expect("journal after refusal"), before);
        let inspected = self.run(
            &Command::Inspect {
                journal: self.path.clone(),
                head: Some(self.head),
            },
            at,
        );
        assert_eq!(inspected.exit_code, 0, "{inspected:?}");
        assert_eq!(head(&inspected.payload_json), self.head);
    }

    fn assess(
        &mut self,
        cut: &str,
        selector: &str,
        expected_digest: Option<Digest32>,
        at: i64,
    ) -> String {
        self.ok(
            Command::Assess {
                journal: self.path.clone(),
                head: self.head,
                cohort: "three-sites".into(),
                as_of: cut.into(),
                selector: selector.into(),
                expected_digest,
            },
            at,
        )
    }
}

fn forecast(alias: &str, lo: i64, hi: i64, due: &str) -> String {
    format!(
        "[[forecast]]\nid = \"{alias}\"\nmodel_key = \"synthetic:{alias}\"\noutcome_due = \"{due}\"\nkind = \"interval\"\nlo = \"{lo}\"\nhi = \"{hi}\"\nnominal = \"0.8\"\n"
    )
}

fn gaussian_forecast(alias: &str, due: &str) -> String {
    format!(
        "[[forecast]]\nid = \"{alias}\"\nmodel_key = \"synthetic:{alias}\"\noutcome_due = \"{due}\"\nkind = \"gaussian\"\nmean = \"0\"\nstd_dev = \"1\"\n"
    )
}

fn gaussian_cohort(opt_in: bool) -> String {
    let mut request = cohort("f1, f2")
        .replace("outcome_lo = \"0\"", "outcome_lo = \"-1\"")
        .replace("outcome_hi = \"100\"", "outcome_hi = \"2\"");
    if opt_in {
        request.push_str("gaussian_crps_bounds = \"registered-domain\"\n");
    }
    request
}

fn cohort(members: &str) -> String {
    format!(
        "[[cohort]]\nname = \"three-sites\"\nmembers = \"{members}\"\nresolution_rule = \"Synthetic annual output on a fixed basis\"\neligible_sources = \"synthetic:issuer\"\nalpha = \"0.2\"\nmin_calibration_n = 35\nrho_low = \"0\"\nrho_high = \"0.5\"\noutcome_lo = \"0\"\noutcome_hi = \"100\"\n"
    )
}

fn outcome(alias: &str, value: i64, available: &str, restated: bool) -> String {
    let kind = if restated { "restated" } else { "published" };
    let supersedes = if restated { "supersedes = 1\n" } else { "" };
    format!(
        "[[outcome]]\nforecast = \"{alias}\"\nkind = \"{kind}\"\navailable_at = \"{available}\"\nvalue = \"{value}\"\nsource = \"synthetic:issuer\"\nvintage = \"synthetic-{kind}\"\nlocator = \"Invented!A1\"\nsha256 = \"{}\"\n{supersedes}",
        "11".repeat(32)
    )
}

fn one_member(skew: u64) -> Journal {
    let mut journal = Journal::new(ns("2030-01-01T00:00:00Z"), skew);
    journal.append(&forecast("f1", 40, 50, DUE), ns("2030-01-02T00:00:00Z"));
    journal.append(&cohort("f1"), ns("2030-01-03T00:00:00Z"));
    journal
}

fn metadata_forecast(alias: &str, author: &str, site: &str) -> String {
    let base = forecast(alias, 40, 50, DUE)
        .replace(&format!("synthetic:{alias}"), "synthetic:production-v1");
    format!(
        "{base}family = \"annual-production\"\nauthor_role = \"{author}\"\ndomain = \"mine-total\"\nhorizon = \"one-year\"\nsite = \"{site}\"\nquantity = \"gold-production-100pct\"\nunit = \"koz\"\n"
    )
}

#[test]
fn prospective_segments_keep_missing_members_and_replay_saved_scores() {
    let mut journal = Journal::new(ns("2030-01-01T00:00:00Z"), 0);
    for (index, (alias, author, site)) in [
        ("f1", "agent:planner-a@v1", "synthetic-north"),
        ("f2", "agent:planner-a@v1", "synthetic-north"),
        ("f3", "agent:planner-b@v1", "synthetic-south"),
    ]
    .into_iter()
    .enumerate()
    {
        journal.append(
            &metadata_forecast(alias, author, site),
            ns("2030-01-02T00:00:00Z") + index as i64 * SECOND,
        );
    }
    journal.append(&cohort("f3, f2, f1"), ns("2030-01-03T00:00:00Z"));
    for (alias, value) in [("f1", 45), ("f3", 60)] {
        journal.append(
            &outcome(alias, value, "2031-01-01T00:00:00Z", false),
            ns("2031-01-02T00:00:00Z"),
        );
    }
    let saved = journal.assess(CUT, "first-published", None, ns(CUT));
    let json = &saved;
    assert!(json.contains("\"v7\":3"), "{json}");
    assert!(
        json.contains("\"schema\":\"fmine.forecast-segments.v1\""),
        "{json}"
    );
    let segments = json.split_once("\"segmented_assessment\":").unwrap().1;
    assert!(segments.contains("\"n_members\":3,\"interpretation\":\"descriptive_only\""));
    assert!(segments.contains("\"n_members\":2,\"n_resolved\":1,\"n_unresolved\":1"));
    // Independent interval-score arithmetic: width 10; the unresolved member ranges from
    // 10 to 510 over frozen outcomes [0,100], so this two-member mean ranges from 10 to 260.
    assert!(segments.contains("\"interval_score\":{\"n_scored\":1,\"observed_mean\":10,\"whole_cohort\":{\"lower\":10,\"upper\":260,"));
    assert!(segments.contains("\"inside\":1,\"observed_fraction\":1,\"whole_cohort\":[0.5,1]"));
    assert!(segments.contains("\"observed_mean\":110"));
    assert!(!segments.contains("\"effective_n\""));
    assert!(!segments.contains("\"calibration_flag\""));
    let digest = assessment_digest(json);
    journal.append(
        &outcome("f1", 70, "2031-01-11T00:00:00Z", true),
        ns("2031-01-12T00:00:00Z"),
    );
    let replayed = journal.run(
        &Command::Report {
            journal: journal.path.clone(),
            head: Some(journal.head),
            assessment: digest,
            provenance: None,
            timestamp: None,
        },
        0,
    );
    assert_eq!(replayed.exit_code, 0, "{}", replayed.to_json());
    assert_eq!(assessment(&replayed.payload_json), assessment(json));
    let markdown = replayed.rendered_markdown.as_deref().unwrap();
    assert!(markdown.contains("## Forecast segments"));
    assert!(markdown.contains("synthetic-north"));
    assert!(markdown.contains("Completion upper"));
    assert!(markdown.contains("| Interval score | 1 | 10 | 10 | 260 | koz |"));
}

#[test]
fn empirical_diagnostics_reopen_tied_missing_and_mixed_assessments_without_reselecting_sources() {
    let mut journal = Journal::new(ns("2030-01-01T00:00:00Z"), 0);
    for alias in ["f1", "f2"] {
        journal.append(
            &format!(
                "[[forecast]]\nid = \"{alias}\"\nmodel_key = \"synthetic:{alias}\"\noutcome_due = \"{DUE}\"\nkind = \"empirical\"\nsamples = \"1,3,3,9\"\n"
            ),
            ns("2030-01-02T00:00:00Z"),
        );
    }
    journal.append(&gaussian_forecast("f3", DUE), ns("2030-01-02T00:00:00Z"));
    journal.append(&cohort("f3,f2,f1"), ns("2030-01-03T00:00:00Z"));
    for (alias, value) in [("f1", 3), ("f3", 0)] {
        journal.append(
            &outcome(alias, value, "2031-01-01T00:00:00Z", false),
            ns("2031-01-02T00:00:00Z"),
        );
    }
    let saved = journal.assess(CUT, "latest-as-of", None, ns(CUT));
    let old_digest = assessment_digest(&saved);
    let diagnostics = saved.split_once("\"diagnostics\":").unwrap().1;
    assert!(diagnostics.contains("\"schema\":\"fmine.forecast-diagnostics.v2\""));
    assert!(
        diagnostics
            .contains("\"gaussian_pit\":{\"n_members\":1,\"n_resolved\":1,\"n_unresolved\":0,")
    );
    assert!(
        diagnostics
            .contains("\"empirical_rank\":{\"n_members\":2,\"n_resolved\":1,\"n_unresolved\":1,")
    );
    assert!(diagnostics.contains(concat!(
        "\"cdf_bounds\":{\"lower\":{\"num\":1,\"den\":4},",
        "\"upper\":{\"num\":3,\"den\":4}}"
    )));
    assert!(diagnostics.contains(concat!(
        "\"n_samples\":4,\"below\":null,\"equal\":null,",
        "\"above\":null,\"cdf_bounds\":null"
    )));
    assert!(diagnostics.contains("Invented!A1"));
    assert!(diagnostics.contains(&"11".repeat(32)));
    let old_diagnostic_digest = string_field(diagnostics, "digest").to_string();
    let coverage = saved.split_once("\"interval_coverage\":").unwrap().1;
    assert!(coverage.contains("\"schema\":\"fmine.forecast-coverage.v1\""));
    for level in [50, 80, 95] {
        assert!(coverage.contains(&format!("\"percent\":{level},\"n_supported\":3,\"n_unsupported\":0,\"n_resolved\":2,\"n_unresolved\":1,\"inside\":2,\"below\":0,\"above\":0")));
    }
    assert!(coverage.contains("\"whole_supported_fraction\":{\"lower\":{\"num\":2,\"den\":3},\"upper\":{\"num\":3,\"den\":3}}"));
    assert!(coverage.contains("\"percent\":50,\"bounds\":[1,3],\"status\":\"unresolved\""));
    let old_coverage_digest = string_field(coverage, "digest").to_string();

    journal.append(
        &outcome("f1", 9, "2031-01-11T00:00:00Z", true),
        ns("2031-01-12T00:00:00Z"),
    );
    journal.append(
        &outcome("f2", 4, "2031-01-11T00:00:00Z", false),
        ns("2031-01-12T00:00:00Z"),
    );
    let latest = journal.assess(
        "2031-01-13T00:00:00Z",
        "latest-as-of",
        None,
        ns("2031-01-13T00:00:00Z"),
    );
    let latest_diagnostics = latest.split_once("\"diagnostics\":").unwrap().1;
    assert_ne!(
        string_field(latest_diagnostics, "digest"),
        old_diagnostic_digest
    );
    assert!(
        latest_diagnostics
            .contains("\"empirical_rank\":{\"n_members\":2,\"n_resolved\":2,\"n_unresolved\":0,")
    );
    assert!(latest_diagnostics.contains(concat!(
        "\"cdf_bounds\":{\"lower\":{\"num\":3,\"den\":4},",
        "\"upper\":{\"num\":4,\"den\":4}}"
    )));
    let latest_coverage = latest.split_once("\"interval_coverage\":").unwrap().1;
    assert_ne!(string_field(latest_coverage, "digest"), old_coverage_digest);
    assert!(latest_coverage.contains("\"percent\":50,\"n_supported\":3,\"n_unsupported\":0,\"n_resolved\":3,\"n_unresolved\":0,\"inside\":1,\"below\":0,\"above\":2"));

    let bytes = fs::read(&journal.path).expect("journal before read-only replay");
    let reopened = journal.run(
        &Command::Report {
            journal: journal.path.clone(),
            head: Some(journal.head),
            assessment: old_digest,
            provenance: None,
            timestamp: None,
        },
        0,
    );
    assert_eq!(reopened.exit_code, 0, "{}", reopened.to_json());
    assert_eq!(assessment(&reopened.payload_json), assessment(&saved));
    assert_eq!(
        fs::read(&journal.path).expect("journal after replay"),
        bytes
    );
    let markdown = reopened.rendered_markdown.as_deref().unwrap();
    assert!(markdown.contains("### Empirical CDF rank intervals"));
    assert!(markdown.contains("[1/4, 3/4] (below 1; equal 2; above 1)"));
    assert!(markdown.contains("unavailable (n=4)"));
    assert!(markdown.contains("not calibration verdicts"));
    assert!(markdown.contains("### Central interval coverage"));
    assert!(markdown.contains("| 50% | 3 / 3 | 2 | 2 | 0 | 0 | 1 | [2/3, 3/3] |"));
    assert!(markdown.contains(&old_coverage_digest));
}

#[test]
fn metadata_admission_refuses_partial_and_mixed_quantities_without_appending() {
    let mut journal = Journal::new(ns("2030-01-01T00:00:00Z"), 0);
    let valid = metadata_forecast("f1", "agent:planner@v1", "synthetic-north");
    let partial = valid.replace("unit = \"koz\"\n", "");
    let command = journal.request(&partial);
    journal.refuse(&command, ns("2030-01-02T00:00:00Z"), "FMINE-REF-107");
    journal.append(&valid, ns("2030-01-02T00:00:00Z"));
    journal.append(&forecast("legacy", 40, 50, DUE), ns("2030-01-02T00:00:00Z"));
    let command = journal.request(&cohort("f1, legacy"));
    journal.refuse(&command, ns("2030-01-03T00:00:00Z"), "FMINE-REF-109");
    let other_unit = metadata_forecast("f2", "agent:planner@v1", "synthetic-north")
        .replace("unit = \"koz\"", "unit = \"oz\"");
    journal.append(&other_unit, ns("2030-01-02T00:00:00Z"));
    let command = journal.request(&cohort("f1, f2"));
    journal.refuse(&command, ns("2030-01-03T00:00:00Z"), "FMINE-REF-109");
    // Failed freezes did not reserve the name or advance authority chronology.
    journal.append(&cohort("f1"), ns("2030-01-03T00:00:00Z"));
}

#[test]
fn metadata_schema_downgrade_removal_and_retagging_cannot_rewrite_a_checkpoint() {
    let mut journal = Journal::new(ns("2030-01-01T00:00:00Z"), 0);
    journal.append(
        &metadata_forecast("f1", "agent:planner@v1", "synthetic-north"),
        ns("2030-01-02T00:00:00Z"),
    );
    journal.append(&cohort("f1"), ns("2030-01-03T00:00:00Z"));
    journal.assess(CUT, "first-published", None, ns(CUT));
    let snapshot = DurableJournal::open_read(&journal.path, Some(journal.head)).unwrap();
    for (change, from, to, code) in [
        (
            "downgrade",
            "fmine.forecast-operation.v7",
            "fmine.forecast-operation.v2",
            "FMINE-REF-107",
        ),
        ("partial", "unit = \"koz\"\n", "", "FMINE-REF-107"),
        (
            "retag",
            "agent:planner@v1",
            "agent:different@v2",
            "FMINE-REF-505",
        ),
    ] {
        let mut records = snapshot.records().to_vec();
        for record in &mut records {
            if let Entry::Operation(text) = &mut record.entry
                && text.contains("schema = \"fmine.forecast-operation.v7\"")
            {
                *text = text.replace(from, to);
            }
        }
        let path = journal.root.join(format!("metadata-{change}.journal"));
        let pin = write_fixture(&path, &mut records);
        assert_refused(
            &journal.run(
                &Command::Inspect {
                    journal: path,
                    head: Some(pin),
                },
                0,
            ),
            code,
        );
    }
}

/// Encode explicitly constructed legacy or negative fixture records; the production replay still checks them.
fn write_fixture(path: &std::path::Path, records: &mut [Record]) -> Digest32 {
    let mut previous = Digest32::default();
    for record in records.iter_mut() {
        record.previous = previous;
        previous = record.chain_head().expect("canonical fixture head");
    }
    validate_chain(records).expect("structurally valid synthetic chain");
    let bytes: Vec<_> = records
        .iter()
        .flat_map(|record| record.to_line().expect("fixture line"))
        .collect();
    fs::write(path, bytes).expect("write synthetic fixture");
    previous
}

#[test]
fn gaussian_domain_bounds_replay_with_pending_members_and_require_the_frozen_domain() {
    let mut journal = Journal::new(ns("2030-01-01T00:00:00Z"), 0);
    for alias in ["f1", "f2"] {
        journal.append(&gaussian_forecast(alias, DUE), ns("2030-01-02T00:00:00Z"));
    }
    let omitted = gaussian_cohort(true)
        .replace("outcome_lo = \"-1\"\n", "")
        .replace("outcome_hi = \"2\"\n", "");
    let request = journal.request(&omitted);
    journal.refuse(&request, ns("2030-01-03T00:00:00Z"), "FMINE-REF-109");
    let wrong_mode =
        journal.request(&gaussian_cohort(true).replace("registered-domain", "estimated"));
    journal.refuse(&wrong_mode, ns("2030-01-03T00:00:00Z"), "FMINE-REF-107");
    let frozen = journal.append(&gaussian_cohort(true), ns("2030-01-03T00:00:00Z"));
    assert!(frozen.contains("\"gaussian_crps_bounds\":\"registered-domain\""));
    let outside = journal.request(&outcome("f1", 3, "2031-01-02T00:00:00Z", false));
    journal.refuse(&outside, ns("2031-01-03T00:00:00Z"), "FMINE-REF-109");
    journal.append(
        &outcome("f1", 0, "2031-01-02T00:00:00Z", false),
        ns("2031-01-03T00:00:00Z"),
    );
    let output = journal.assess(CUT, "first-published", None, ns(CUT));
    assert!(output.contains("\"n_members\":2,\"n_resolved\":1,\"n_unresolved\":1"));
    assert!(output.contains("\"operation_version_counts\":{\"legacy_v1\":0,\"v2\":4,\"v6\":1}"));
    let crps = output
        .split_once("\"crps\":{")
        .unwrap()
        .1
        .split_once(",\"interval_score\"")
        .unwrap()
        .0;
    let number = |field: &str| -> f64 {
        crps.split_once(&format!("\"{field}\":"))
            .unwrap()
            .1
            .split([',', '}'])
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    // Independent normal known answers; one observed outcome at the mean, one pending on [-1,2].
    let at_mean = 0.233_694_977_255_109_07;
    let at_two = 1.452_791_821_685_903;
    assert!((number("observed_mean") - at_mean).abs() < 2e-13);
    assert!((number("lower") - at_mean).abs() < 2e-13);
    assert!((number("upper") - (at_mean + at_two) / 2.0).abs() < 2e-13);
    assert!(crps.contains("\"upper_unbounded\":false"));
    let before = fs::read(&journal.path).unwrap();
    let replay = journal.run(
        &Command::Report {
            journal: journal.path.clone(),
            head: Some(journal.head),
            assessment: assessment_digest(&output),
            provenance: None,
            timestamp: None,
        },
        0,
    );
    assert_eq!(replay.exit_code, 0, "{}", replay.to_json());
    assert_eq!(assessment(&replay.payload_json), assessment(&output));
    assert_eq!(fs::read(&journal.path).unwrap(), before);

    let snapshot = DurableJournal::open_read(&journal.path, Some(journal.head)).unwrap();
    for (change, from, to) in [
        (
            "downgrade",
            "fmine.forecast-operation.v6",
            "fmine.forecast-operation.v5",
        ),
        (
            "removed",
            "gaussian_crps_bounds = \"registered-domain\"\n",
            "",
        ),
        ("changed", "registered-domain", "estimated"),
    ] {
        let mut records = snapshot.records().to_vec();
        for record in &mut records {
            if let Entry::Operation(text) = &mut record.entry
                && text.contains("schema = \"fmine.forecast-operation.v6\"")
            {
                *text = text.replace(from, to);
            }
        }
        let path = journal
            .root
            .join(format!("changed-version-{change}.journal"));
        let pin = write_fixture(&path, &mut records);
        assert_refused(
            &journal.run(
                &Command::Inspect {
                    journal: path,
                    head: Some(pin),
                },
                0,
            ),
            "FMINE-REF-107",
        );
    }
}

#[test]
fn legacy_bounded_gaussian_checkpoint_survives_a_new_v6_cohort() {
    // FACT: captured at c410307 before adding the finite-domain opt-in; these values include an
    // unresolved Gaussian under a registered finite domain with the old conservative [0,+infinity] loss.
    let old_digest = Digest32(
        parse_hex32("ca9c4b239c871c4f454dcdf010dceaec10daccbd719b50d4f420dedcd46ea2bd").unwrap(),
    );
    let old_cohort = "5cd6b2ea312f087dc2bfc1b46fbb67a5906395115294658a9fdb34cee68ac304";
    let mut journal = Journal::new(ns("2030-01-01T00:00:00Z"), 0);
    let snapshot = DurableJournal::open_read(&journal.path, Some(journal.head)).unwrap();
    let mut records = snapshot.records().to_vec();
    drop(snapshot);
    for (at, request) in [
        (
            "2030-01-02T00:00:00Z",
            gaussian_forecast("f1", "2031-01-01T00:00:00Z"),
        ),
        (
            "2030-01-02T00:00:00Z",
            gaussian_forecast("f2", "2031-01-01T00:00:00Z"),
        ),
        ("2030-01-03T00:00:00Z", gaussian_cohort(false)),
        (
            "2031-01-03T00:00:00Z",
            outcome("f1", 0, "2031-01-02T00:00:00Z", false),
        ),
        (
            CUT,
            format!(
                "[[assessment]]\ncohort = \"three-sites\"\nas_of = \"{CUT}\"\nselector = \"first-published\"\nexpected_digest = \"{old_digest}\"\n"
            ),
        ),
    ] {
        records.push(Record {
            sequence: records.len() as u64,
            previous: Digest32::default(),
            observed_ns: ns(at),
            entry: Entry::Operation(format!(
                "schema = \"fmine.forecast-operation.v2\"\n{request}"
            )),
        });
    }
    journal.head = write_fixture(&journal.path, &mut records);
    let report = |journal: &Journal| {
        journal.run(
            &Command::Report {
                journal: journal.path.clone(),
                head: Some(journal.head),
                assessment: old_digest,
                provenance: None,
                timestamp: None,
            },
            0,
        )
    };
    let before = report(&journal);
    assert_eq!(before.exit_code, 0, "{}", before.to_json());
    assert!(before.payload_json.contains(old_cohort));
    assert_eq!(assessment_digest(&before.payload_json), old_digest);
    assert!(
        before
            .payload_json
            .contains("\"lower\":0.11684748862755456,\"upper\":null,\"upper_unbounded\":true")
    );
    journal.append(
        &gaussian_forecast("next", "2032-01-01T00:00:00Z"),
        ns("2031-01-11T00:00:00Z"),
    );
    journal.append(
        &gaussian_cohort(true)
            .replace("three-sites", "new-domain")
            .replace("f1, f2", "next"),
        ns("2031-01-12T00:00:00Z"),
    );
    let after = report(&journal);
    assert_eq!(after.exit_code, 0, "{}", after.to_json());
    assert_eq!(
        assessment(&after.payload_json),
        assessment(&before.payload_json)
    );
    assert!(after.payload_json.contains("\"v6\":1"));
}

#[test]
fn restarted_journal_scores_original_and_restatement_without_losing_missing_members() {
    let mut journal = Journal::new(ns("2030-01-01T00:00:00Z"), 0);
    for (alias, lo, hi, day) in [("f1", 40, 50, 2), ("f2", 20, 24, 3), ("f3", 0, 10, 4)] {
        journal.append(
            &forecast(alias, lo, hi, DUE),
            ns(&format!("2030-01-{day:02}T00:00:00Z")),
        );
    }
    journal.append(&cohort("f1,f2,f3"), ns("2030-01-05T00:00:00Z"));
    journal.append(
        &outcome("f1", 52, "2031-01-02T00:00:00Z", false),
        ns("2031-01-03T00:00:00Z"),
    );
    journal.append(
        &outcome("f2", 22, "2031-01-02T00:00:00Z", false),
        ns("2031-01-04T00:00:00Z"),
    );
    journal.append("[[outcome]]\nforecast = \"f3\"\nkind = \"missing\"\navailable_at = \"2031-01-02T00:00:00Z\"\nreason = \"divested\"\n", ns("2031-01-05T00:00:00Z"));
    let early = journal.assess(CUT, "first-published", None, ns(CUT));
    let early_digest = assessment_digest(&early);
    let coverage = early.split_once("\"interval_coverage\":").unwrap().1;
    for percent in [50, 95] {
        assert!(coverage.contains(&format!(
            "\"percent\":{percent},\"n_supported\":0,\"n_unsupported\":3,"
        )));
    }
    assert!(coverage.contains(
        "\"observed_fraction\":null,\"whole_supported_fraction\":{\"lower\":null,\"upper\":null}"
    ));
    assert!(coverage.contains("\"percent\":80,\"n_supported\":3,\"n_unsupported\":0,\"n_resolved\":2,\"n_unresolved\":1,\"inside\":1,\"below\":0,\"above\":1"));
    assert!(coverage.contains("\"bounds\":null,\"status\":\"level_not_supplied\""));
    // Independent Winkler answers: f1 = 10 + (2 / .2) * 2 = 30; f2 = 4; mean = 17.
    for expected in [
        "\"n_members\":3,\"n_resolved\":2,\"n_unresolved\":1,",
        "\"unresolved_by_reason\":{\"divested\":1}",
        "\"interval_score\":{\"n_scored\":2,\"observed_mean\":17,",
        "\"coverage\":{\"inside\":1,\"observed_fraction\":0.5,\"whole_cohort\":[0.3333333333333333,0.6666666666666666]}",
        "\"bounds_kind\":\"whole_cohort_completion_bounds_not_confidence_intervals\"",
        "\"calibration_flag\":\"descriptive\"",
        "\"mode\":\"durable_journal\"",
        "\"authority_times\":\"operator_declared_bounded_host_clock\"",
        "\"external_time_proof\":\"not_verified\"",
    ] {
        assert!(early.contains(expected), "missing {expected}: {early}");
    }
    journal.append(
        &outcome("f1", 54, "2031-01-20T00:00:00Z", true),
        ns("2031-01-21T00:00:00Z"),
    );
    let latest = journal.assess(
        "2031-01-30T00:00:00Z",
        "latest-as-of",
        None,
        ns("2031-01-30T00:00:00Z"),
    );
    // The restatement changes f1's score to 50, hence mean 27, with the same frozen denominator.
    assert!(
        latest.contains("\"interval_score\":{\"n_scored\":2,\"observed_mean\":27,"),
        "{latest}"
    );
    assert!(latest.contains("\"n_members\":3,\"n_resolved\":2,\"n_unresolved\":1,"));
    assert!(latest.contains("\"kind\":\"restated\",\"supersedes\":1,"));
    let first = journal.assess(
        "2031-01-30T00:00:00Z",
        "first-published",
        None,
        ns("2031-01-30T00:00:00Z"),
    );
    assert!(first.contains("\"interval_score\":{\"n_scored\":2,\"observed_mean\":17,"));
    let replay = journal.assess(
        CUT,
        "first-published",
        Some(early_digest),
        ns("2031-01-31T00:00:00Z"),
    );
    assert_eq!(assessment(&replay), assessment(&early));
    assert!(replay.contains("\"expected_digest_matched\":true"));
    assert_ne!(
        head(&early),
        journal.head,
        "later operations extend the durable chain"
    );
}

#[test]
fn quiet_journals_cannot_admit_late_forecasts_freezes_or_caller_backdates() {
    let mut empty = Journal::new(ns("2030-01-01T00:00:00Z"), 0);
    let late = empty.request(&forecast("late", 40, 50, DUE));
    empty.refuse(&late, ns("2031-01-01T00:00:00Z"), "FMINE-REF-206");
    let backdated = empty.request(&format!(
        "{}registered_at = \"2030-01-02T00:00:00Z\"\n",
        forecast("backdated", 40, 50, "2032-01-01T00:00:00Z")
    ));
    empty.refuse(&backdated, ns("2031-01-01T00:00:00Z"), "FMINE-REF-107");

    let mut quiet = Journal::new(ns("2030-01-01T00:00:00Z"), 0);
    quiet.append(&forecast("f1", 40, 50, DUE), ns("2030-01-02T00:00:00Z"));
    let late_freeze = quiet.request(&cohort("f1"));
    quiet.refuse(&late_freeze, ns("2031-01-01T00:00:00Z"), "FMINE-REF-206");
    let forged_freeze = quiet.request(&format!(
        "{}frozen_at = \"2030-01-03T00:00:00Z\"\n",
        cohort("f1")
    ));
    quiet.refuse(&forged_freeze, ns("2030-01-04T00:00:00Z"), "FMINE-REF-107");

    let mut frozen = one_member(0);
    let forged_outcome = frozen.request(&format!(
        "{}recorded_at = \"2031-01-03T00:00:00Z\"\n",
        outcome("f1", 52, "2031-01-02T00:00:00Z", false)
    ));
    frozen.refuse(&forged_outcome, ns("2031-01-04T00:00:00Z"), "FMINE-REF-107");
}

#[test]
fn clock_uncertainty_is_checked_at_each_admission_boundary_including_equality() {
    let mut journal = Journal::new(ns("2030-01-01T00:00:00Z"), SECOND as u64);
    let registration = journal.request(&forecast("f1", 40, 50, DUE));
    // The latest possible registration is exactly the deadline: refuse despite the observed earlier time.
    journal.refuse(&registration, ns(DUE) - SECOND, "FMINE-REF-206");
    journal.ok(registration, ns(DUE) - 2 * SECOND);
    let freeze = journal.request(&cohort("f1"));
    journal.refuse(&freeze, ns(DUE) - SECOND, "FMINE-REF-206");
    // One nanosecond of strict separation suffices; refused clock samples did not advance the journal.
    journal.ok(freeze, ns(DUE) - SECOND - 1);

    let available = "2031-01-02T00:00:00Z";
    let publication = journal.request(&outcome("f1", 52, available, false));
    journal.refuse(&publication, ns(available), "FMINE-REF-206");
    journal.ok(publication, ns(available) + SECOND);
    let assess = Command::Assess {
        journal: journal.path.clone(),
        head: journal.head,
        cohort: "three-sites".into(),
        as_of: CUT.into(),
        selector: "first-published".into(),
        expected_digest: None,
    };
    journal.refuse(&assess, ns(CUT), "FMINE-REF-206");
    let scored = journal.ok(assess, ns(CUT) + SECOND);
    assert!(scored.contains("\"n_members\":1,\"n_resolved\":1,\"n_unresolved\":0,"));
    assert!(scored.contains("\"registered_at\":\"2030-12-30T23:59:59Z\""));
    assert!(scored.contains("\"frozen_at\":\"2030-12-30T23:59:59.999999999Z\""));
}

#[test]
fn v2_outcomes_cross_historical_cuts_only_at_their_upper_bound_and_keep_checked_results() {
    let mut journal = one_member(SECOND as u64);
    let base = ns("2031-01-02T00:00:00Z");
    journal.append(
        &outcome("f1", 52, "2031-01-02T00:00:00Z", false),
        base + SECOND + SECOND / 2,
    );
    journal.append(
        &outcome("f1", 54, "2031-01-02T00:00:01Z", true),
        base + 2 * SECOND,
    );
    // Observed at B+1.5s is not definitely recorded by B+2s: its upper bound is B+2.5s.
    let before = journal.assess(
        "2031-01-02T00:00:02Z",
        "latest-as-of",
        None,
        base + 5 * SECOND,
    );
    assert!(
        before.contains("\"n_members\":1,\"n_resolved\":0,\"n_unresolved\":1,"),
        "{before}"
    );
    assert!(
        before.contains("\"unresolved_by_reason\":{\"overdue\":1}"),
        "{before}"
    );
    let before_digest = assessment_digest(&before);
    // The restatement's upper bound is exactly B+3s; the inclusive as-of boundary must admit it.
    let exact = journal.assess(
        "2031-01-02T00:00:03Z",
        "latest-as-of",
        None,
        base + 6 * SECOND,
    );
    assert!(
        exact.contains("\"n_members\":1,\"n_resolved\":1,\"n_unresolved\":0,"),
        "{exact}"
    );
    assert!(
        exact.contains("\"recorded_at\":\"2031-01-02T00:00:03Z\""),
        "{exact}"
    );
    assert!(
        exact.contains("\"interval_score\":{\"n_scored\":1,\"observed_mean\":50,"),
        "{exact}"
    );
    let first = journal.assess(
        "2031-01-02T00:00:03Z",
        "first-published",
        None,
        base + 7 * SECOND,
    );
    assert!(
        first.contains("\"recorded_at\":\"2031-01-02T00:00:02.500000000Z\""),
        "{first}"
    );
    assert!(
        first.contains("\"interval_score\":{\"n_scored\":1,\"observed_mean\":30,"),
        "{first}"
    );

    let bytes = fs::read(&journal.path).expect("committed v2 history");
    for saved in [&before, &exact, &first] {
        let report = journal.run(
            &Command::Report {
                journal: journal.path.clone(),
                head: Some(journal.head),
                assessment: assessment_digest(saved),
                provenance: None,
                timestamp: None,
            },
            0,
        );
        assert_eq!(report.exit_code, 0, "{report:?}");
        assert_eq!(assessment(&report.payload_json), assessment(saved));
        assert_eq!(
            string_field(&report.payload_json, "record_time_policy"),
            "clock_upper_bound"
        );
        assert_eq!(fs::read(&journal.path).expect("read-only report"), bytes);
    }
    // Corrupt only the stored result binding, then rebuild the canonical chain. Semantic replay must refuse.
    let snapshot =
        DurableJournal::open_read(&journal.path, Some(journal.head)).expect("v2 fixture");
    let records = snapshot.records().to_vec();
    drop(snapshot);
    for (name, missing, code) in [
        ("missing-pin", true, "FMINE-REF-107"),
        ("wrong-pin", false, "FMINE-REF-505"),
    ] {
        let mut changed = records.clone();
        let mut found = false;
        for record in &mut changed {
            if let Entry::Operation(request) = &mut record.entry
                && request.contains(&before_digest.to_hex())
            {
                assert!(request.starts_with("schema = \"fmine.forecast-operation.v2\"\n"));
                *request = if missing {
                    request
                        .lines()
                        .filter(|line| !line.starts_with("expected_digest ="))
                        .collect::<Vec<_>>()
                        .join("\n")
                        + "\n"
                } else {
                    request.replace(&before_digest.to_hex(), &Digest32::default().to_hex())
                };
                found = true;
            }
        }
        assert!(
            found,
            "the negative control must change a persisted v2 checkpoint"
        );
        let path = journal.root.join(name);
        let pin = write_fixture(&path, &mut changed);
        let bad_bytes = fs::read(&path).expect("negative fixture bytes");
        let refused = journal.run(
            &Command::Report {
                journal: path.clone(),
                head: Some(pin),
                assessment: before_digest,
                provenance: None,
                timestamp: None,
            },
            0,
        );
        assert_refused(&refused, code);
        assert_eq!(
            fs::read(path).expect("negative evidence retained"),
            bad_bytes
        );
    }
}

#[test]
fn legacy_observed_ids_and_assessment_digest_survive_v2_appends_with_mixed_labels() {
    let mut journal = Journal::new(ns("2030-01-01T00:00:00Z"), SECOND as u64);
    let registered = ns("2030-01-02T00:00:00Z") + 123_456_789;
    let frozen = ns("2030-01-03T00:00:00Z");
    let published = ns("2031-01-03T00:00:00Z");
    // This independent kernel construction pins the legacy observed-time interpretation, not the v2 adapter.
    let mut expected = ForecastLedger::new();
    let forecast_id = expected
        .register_payload(
            "synthetic:f1",
            registered,
            ns(DUE),
            ForecastPayload::Interval {
                lo: 40.0,
                hi: 50.0,
                nominal: Some(0.8),
            },
        )
        .expect("legacy forecast");
    let cohort_id = expected
        .freeze_cohort(CohortSpec {
            name: "three-sites".into(),
            frozen_at_ns: frozen,
            members: vec![forecast_id],
            resolution_rule: "Synthetic annual output on a fixed basis".into(),
            eligible_sources: vec!["synthetic:issuer".into()],
            policy: AssessmentPolicy {
                outcome_bounds: Some((0.0, 100.0)),
                ..AssessmentPolicy::default()
            },
        })
        .expect("legacy cohort");
    expected
        .append_outcome(
            forecast_id,
            ns("2031-01-02T00:00:00Z"),
            published,
            OutcomeKind::Published {
                value: 52.0,
                citation: OutcomeCitation {
                    source: "synthetic:issuer".into(),
                    vintage: "synthetic-published".into(),
                    locator: "Invented!A1".into(),
                    sha256: [0x11; 32],
                },
            },
        )
        .expect("legacy publication");
    let digest = expected
        .assess(cohort_id, ns(CUT), OutcomeSelector::FirstPublished)
        .expect("legacy score")
        .digest;

    let snapshot = DurableJournal::open_read(&journal.path, Some(journal.head)).expect("genesis");
    let mut records = snapshot.records().to_vec();
    drop(snapshot);
    // Old writers stored no top-level schema. Construct those exact old rows explicitly, never through a v2 writer.
    for (observed_ns, request) in [
        (registered, forecast("f1", 40, 50, DUE)),
        (frozen, cohort("f1")),
        (published, outcome("f1", 52, "2031-01-02T00:00:00Z", false)),
        (
            ns(CUT) + SECOND,
            format!(
                "[[assessment]]\ncohort = \"three-sites\"\nas_of = \"{CUT}\"\nselector = \"first-published\"\nexpected_digest = \"{digest}\"\n"
            ),
        ),
    ] {
        records.push(Record {
            sequence: records.len() as u64,
            previous: Digest32::default(),
            observed_ns,
            entry: Entry::Operation(request),
        });
    }
    journal.head = write_fixture(&journal.path, &mut records);
    let legacy = journal.run(
        &Command::Report {
            journal: journal.path.clone(),
            head: Some(journal.head),
            assessment: digest,
            provenance: None,
            timestamp: None,
        },
        0,
    );
    assert_eq!(legacy.exit_code, 0, "{legacy:?}");
    assert!(
        legacy
            .payload_json
            .contains(&format!("\"forecast_id\":\"{forecast_id}\""))
    );
    assert!(
        legacy
            .payload_json
            .contains(&format!("\"registered_ns\":{registered}"))
    );
    assert_eq!(assessment_digest(&legacy.payload_json), digest);
    assert_eq!(
        string_field(&legacy.payload_json, "record_time_policy"),
        "legacy_observed"
    );
    assert!(
        legacy
            .payload_json
            .contains("\"operation_version_counts\":{\"legacy_v1\":4,\"v2\":0}")
    );

    let mixed = journal.append(
        &forecast("f2", 20, 24, "2032-01-01T00:00:00Z"),
        ns("2031-01-11T00:00:00Z"),
    );
    assert_eq!(string_field(&mixed, "record_time_policy"), "mixed");
    assert!(
        mixed.contains("\"operation_version_counts\":{\"legacy_v1\":4,\"v2\":1}"),
        "{mixed}"
    );
    assert!(
        mixed.contains(&format!(
            "\"registered_ns\":{}",
            ns("2031-01-11T00:00:00Z") + SECOND
        )),
        "{mixed}"
    );
    let before = fs::read(&journal.path).expect("mixed history");
    let old = journal.run(
        &Command::Report {
            journal: journal.path.clone(),
            head: Some(journal.head),
            assessment: digest,
            provenance: None,
            timestamp: None,
        },
        0,
    );
    assert_eq!(old.exit_code, 0, "{old:?}");
    assert_eq!(
        assessment(&old.payload_json),
        assessment(&legacy.payload_json)
    );
    assert_eq!(
        string_field(&old.payload_json, "record_time_policy"),
        "mixed"
    );
    let report = old
        .payload_json
        .split_once("\"report\":")
        .expect("historical report")
        .1;
    assert_eq!(
        string_field(report, "record_time_policy"),
        "legacy_observed"
    );
    assert!(report.contains("\"operation_version_counts\":{\"legacy_v1\":4,\"v2\":0}"));
    assert_eq!(
        fs::read(&journal.path).expect("report writes nothing"),
        before
    );
}

#[test]
fn assessment_seals_survive_reopen_and_late_ingestion_preserves_earlier_results() {
    let mut journal = one_member(0);
    let sealed = journal.assess(CUT, "first-published", None, ns(CUT));
    let digest = assessment_digest(&sealed);
    assert!(sealed.contains("\"n_members\":1,\"n_resolved\":0,\"n_unresolved\":1,"));
    let publication = journal.request(&outcome("f1", 52, "2031-01-02T00:00:00Z", false));
    // Every invocation reopens from disk, including the persisted assessment seal.
    journal.refuse(&publication, ns(CUT), "FMINE-REF-206");
    let new_forecast = journal.request(&forecast("f2", 20, 24, "2032-01-01T00:00:00Z"));
    journal.refuse(&new_forecast, ns(CUT), "FMINE-REF-206");
    journal.refuse(&publication, ns(CUT) - SECOND, "FMINE-REF-502");
    // Historical availability is allowed when actual ingestion is later; it must not rewrite the old cut.
    journal.ok(publication, ns("2031-01-11T00:00:00Z"));
    let earlier = journal.assess(
        CUT,
        "first-published",
        Some(digest),
        ns("2031-01-12T00:00:00Z"),
    );
    assert_eq!(assessment(&earlier), assessment(&sealed));
    let resolved = journal.assess(
        "2031-01-12T00:00:00Z",
        "first-published",
        None,
        ns("2031-01-12T00:00:00Z"),
    );
    assert!(resolved.contains("\"n_members\":1,\"n_resolved\":1,\"n_unresolved\":0,"));
    assert!(resolved.contains("\"interval_score\":{\"n_scored\":1,\"observed_mean\":30,"));
    assert!(resolved.contains(
        "\"available_at\":\"2031-01-02T00:00:00Z\",\"recorded_at\":\"2031-01-11T00:00:00Z\""
    ));
    let wrong_expected = Command::Assess {
        journal: journal.path.clone(),
        head: journal.head,
        cohort: "three-sites".into(),
        as_of: CUT.into(),
        selector: "first-published".into(),
        expected_digest: Some(Digest32::default()),
    };
    journal.refuse(&wrong_expected, ns("2031-01-13T00:00:00Z"), "FMINE-REF-505");
}

#[test]
fn pinned_heads_detect_stale_writers_tampering_and_rollback_without_rewriting_evidence() {
    let mut journal = Journal::new(ns("2030-01-01T00:00:00Z"), 0);
    let genesis = journal.head;
    journal.append(&forecast("f1", 40, 50, DUE), ns("2030-01-02T00:00:00Z"));
    let earlier_bytes = fs::read(&journal.path).expect("earlier valid history");
    let mut stale = journal.request(&cohort("f1"));
    if let Command::Freeze { head, .. } = &mut stale {
        *head = genesis;
    }
    journal.refuse(&stale, ns("2030-01-03T00:00:00Z"), "FMINE-REF-502");
    journal.append(&cohort("f1"), ns("2030-01-03T00:00:00Z"));
    let valid_bytes = fs::read(&journal.path).expect("current valid history");
    let inspect = Command::Inspect {
        journal: journal.path.clone(),
        head: Some(journal.head),
    };

    // Alter a nibble while leaving the hex framing intact: neither CRC nor the pinned chain may be ignored.
    let mut tampered = valid_bytes.clone();
    let index = tampered
        .iter()
        .position(|b| *b == b'\n')
        .expect("genesis terminator")
        / 2;
    tampered[index] = if tampered[index] == b'0' { b'1' } else { b'0' };
    fs::write(&journal.path, &tampered).expect("synthetic corruption");
    assert_refused(
        &journal.run(&inspect, ns("2030-01-04T00:00:00Z")),
        "FMINE-REF-502",
    );
    assert_eq!(
        fs::read(&journal.path).expect("corrupt evidence retained"),
        tampered
    );

    // Replacing a journal with a valid old prefix is detectable against the externally retained latest head.
    fs::write(&journal.path, &earlier_bytes).expect("synthetic rollback");
    assert_refused(
        &journal.run(&inspect, ns("2030-01-04T00:00:00Z")),
        "FMINE-REF-502",
    );
    assert_eq!(
        fs::read(&journal.path).expect("rolled-back evidence retained"),
        earlier_bytes
    );
    fs::write(&journal.path, valid_bytes).expect("restore valid synthetic fixture");
    let restored = journal.run(&inspect, ns("2030-01-04T00:00:00Z"));
    assert_eq!(restored.exit_code, 0, "{restored:?}");
    assert!(
        restored
            .payload_json
            .contains("\"n_forecasts\":1,\"n_cohorts\":1,")
    );
}

// Renderer acceptance goes through durable public commands and reopens the saved assessment.
mod diagnostic_rendering {
    use super::*;

    fn fixture(
        name: &str,
        rule: &str,
        locator: &str,
        forecasts: &[(&str, u8, &str)],
        outcomes: &[(&str, &str)],
    ) -> ((String, Envelope), Digest32) {
        let mut journal = Journal::new(ns("2025-12-31T00:00:00Z"), 0);
        for (id, due_day, payload) in forecasts {
            journal.append(
                &format!("[[forecast]]\nid = \"{id}\"\nmodel_key = \"synthetic-model\"\noutcome_due = \"2026-02-{due_day:02}T00:00:00Z\"\n{payload}\n"),
                ns("2026-01-01T00:00:00Z"),
            );
        }
        let members = forecasts
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>()
            .join(",");
        journal.append(
            &format!("[[cohort]]\nname = \"{name}\"\nmembers = \"{members}\"\nresolution_rule = \"{rule}\"\neligible_sources = \"synthetic\"\n"),
            ns("2026-01-02T00:00:00Z"),
        );
        for (forecast, value) in outcomes {
            journal.append(
                &format!("[[outcome]]\nforecast = \"{forecast}\"\navailable_at = \"2026-02-01T00:00:01Z\"\nkind = \"published\"\nvalue = \"{value}\"\nsource = \"synthetic\"\nvintage = \"first\"\nlocator = \"{locator}\"\nsha256 = \"0000000000000000000000000000000000000000000000000000000000000001\"\n"),
                ns("2026-02-01T00:00:01Z"),
            );
        }
        let saved = journal.ok(
            Command::Assess {
                journal: journal.path.clone(),
                head: journal.head,
                cohort: name.into(),
                as_of: "2026-02-01T00:00:02Z".into(),
                selector: "first-published".into(),
                expected_digest: None,
            },
            ns("2026-02-01T00:00:02Z"),
        );
        let digest = assessment_digest(&saved);
        let reopened = journal.run(
            &Command::Report {
                journal: journal.path.clone(),
                head: Some(journal.head),
                assessment: digest,
                provenance: None,
                timestamp: None,
            },
            0,
        );
        assert_eq!(reopened.exit_code, 0, "{}", reopened.to_json());
        assert_eq!(assessment(&saved), assessment(&reopened.payload_json));
        ((saved, reopened), digest)
    }

    fn render(state: &(String, Envelope), _: &Digest32) -> (String, String) {
        let diagnostic = assessment(&state.1.payload_json)
            .split_once(",\"diagnostics\":")
            .expect("saved diagnostics")
            .1
            .split_once(",\"interval_coverage\":")
            .expect("separate coverage projection")
            .0;
        let json = format!(",\"diagnostics\":{diagnostic}");
        let markdown = state
            .1
            .rendered_markdown
            .clone()
            .expect("saved report Markdown");
        assert!(json.contains("\"selected_event\":{\"seq\":1,"));
        assert!(!json.contains("\"calibration_flag\""));
        assert!(state.0.contains(&json));
        (json, markdown)
    }

    #[test]
    fn empirical_renderers_keep_exact_ties_missing_counts_and_separate_gaussian_population() {
        let (state, report) = fixture(
            "mixed",
            "synthetic continuous outcome",
            "synthetic:whole",
            &[
                ("tied", 1, "kind = \"empirical\"\nsamples = \"8,10,10,12\""),
                (
                    "missing",
                    1,
                    "kind = \"empirical\"\nsamples = \"10,10,10,10\"",
                ),
                (
                    "normal",
                    1,
                    "kind = \"gaussian\"\nmean = \"10\"\nstd_dev = \"2\"",
                ),
            ],
            &[("tied", "10"), ("normal", "10")],
        );
        let (json, markdown) = render(&state, &report);
        assert!(json.contains("\"schema\":\"fmine.forecast-diagnostics.v2\""));
        assert!(
            json.contains("\"gaussian_pit\":{\"n_members\":1,\"n_resolved\":1,\"n_unresolved\":0,")
        );
        assert!(
            json.contains(
                "\"empirical_rank\":{\"n_members\":2,\"n_resolved\":1,\"n_unresolved\":1,"
            )
        );
        assert!(json.contains("\"n_samples\":4,\"below\":1,\"equal\":2,\"above\":1,"));
        assert!(json.contains(concat!(
            "\"cdf_bounds\":{\"lower\":{\"num\":1,\"den\":4},",
            "\"upper\":{\"num\":3,\"den\":4}}"
        )));
        assert!(json.contains(concat!(
            "\"n_samples\":4,\"below\":null,\"equal\":null,",
            "\"above\":null,\"cdf_bounds\":null"
        )));
        assert!(json.contains(concat!(
            "\"whole_population_fraction\":{\"lower\":{\"num\":0,\"den\":2},",
            "\"upper\":{\"num\":2,\"den\":2}}"
        )));
        assert!(json.contains("synthetic:whole"));
        assert!(markdown.contains("### Empirical CDF rank intervals"));
        assert!(markdown.contains("Empirical population: 2; resolved: 1; unresolved: 1."));
        assert!(markdown.contains("[1/4, 3/4] (below 1; equal 2; above 1)"));
        assert!(markdown.contains("unavailable (n=4)"));
        assert!(markdown.contains("no randomized PIT, uniformity or calibration"));
    }

    #[test]
    fn renderers_retain_sources_missing_members_and_descriptive_boundaries() {
        let (state, report) = fixture(
            "events",
            "synthetic binary event",
            "synthetic:whole-event",
            &[
                ("hit", 1, "kind = \"bernoulli\"\nprobability = \"0.15\""),
                ("pending", 2, "kind = \"bernoulli\"\nprobability = \"0.19\""),
            ],
            &[("hit", "1")],
        );
        let (json, markdown) = render(&state, &report);
        assert!(json.contains("\"schema\":\"fmine.forecast-diagnostics.v1\""));
        assert!(json.contains("\"gaussian_pit\":null"));
        assert!(!json.contains("\"empirical_rank\""));
        assert!(json.contains("\"n_members\":2,\"n_resolved\":1,\"n_unresolved\":1"));
        assert!(json.contains("\"whole_bin_frequency\":[0.5,1]"));
        assert!(json.contains("synthetic:whole-event"));
        assert!(json.contains("\"reason\":\"pending\""));
        assert!(markdown.contains("Bernoulli reliability bins"));
        assert!(markdown.contains("not calibration verdicts"));
        assert!(markdown.contains("pending (`"));
        assert!(markdown.contains(&report.to_hex()));
    }
}
