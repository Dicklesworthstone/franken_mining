use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_calib::record::{parse_hex32, parse_utc};
use fmine_claim::forecast_journal::Entry;
use fmine_cli::forecast_journal::{Command, execute_with_clock};
use fmine_cli::{Envelope, invoke};
use fmine_core::Digest32;
use fmine_io::forecast::Journal;
use fmine_io::omf::json::{Json, parse};
use fmine_runtime::{ClockError, WallClock};

const JOURNAL: &str = "weighted.journal";
const DUE: &str = "2030-12-31T00:00:00Z";
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
        panic!("read-only replay must not sample a clock")
    }
}

fn ns(utc: &str) -> i64 {
    parse_utc(utc).unwrap().checked_mul(1_000_000_000).unwrap()
}

fn field<'a>(value: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter().fold(value, |value, key| {
        value
            .get(key)
            .unwrap_or_else(|| panic!("missing {key}: {value:?}"))
    })
}

fn digest(value: &Json, keys: &[&str]) -> Digest32 {
    Digest32(parse_hex32(field(value, keys).as_str().unwrap()).unwrap())
}

fn near(value: &Json, expected: f64) {
    let actual = value.f64().expect("finite score");
    assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
}

fn workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-weighted-forecast-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&root).unwrap();
    root
}

struct Fixture {
    root: PathBuf,
    head: Digest32,
    requests: usize,
}

impl Fixture {
    fn new() -> Self {
        let mut fixture = Self {
            root: workspace(),
            head: Digest32::default(),
            requests: 0,
        };
        fixture.ok(
            Command::Init {
                journal: JOURNAL.into(),
                cell_id: "synthetic-weighted".into(),
                clock_skew_ns: 0,
            },
            "2030-01-01T00:00:00Z",
        );
        fixture
    }

    fn ok(&mut self, command: Command, at: &str) -> Json {
        let envelope = execute_with_clock(&command, &self.root, &Clock(ns(at)));
        let payload = checked(envelope);
        self.head = digest(&payload, &["head"]);
        payload
    }

    fn request(&mut self, text: &str) -> Command {
        let request = PathBuf::from(format!("request-{}.toml", self.requests));
        self.requests += 1;
        fs::write(self.root.join(&request), text).unwrap();
        let journal = JOURNAL.into();
        let head = self.head;
        match text.lines().next().unwrap() {
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
            other => panic!("unexpected test operation {other}"),
        }
    }

    fn append(&mut self, text: &str, at: &str) -> Json {
        let command = self.request(text);
        self.ok(command, at)
    }

    fn assess(&mut self, cut: &str, selector: &str, at: &str) -> Json {
        self.ok(
            Command::Assess {
                journal: JOURNAL.into(),
                head: self.head,
                cohort: "joint".into(),
                as_of: cut.into(),
                selector: selector.into(),
                expected_digest: None,
            },
            at,
        )
    }

    fn bytes(&self) -> Vec<u8> {
        fs::read(self.root.join(JOURNAL)).unwrap()
    }
}

fn checked(envelope: Envelope) -> Json {
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    parse(envelope.payload_json.as_bytes()).unwrap()
}

fn forecast(id: &str, samples: &str, weights: &str) -> String {
    format!(
        "[[forecast]]\nid = \"{id}\"\nmodel_key = \"synthetic/weighted/{id}\"\noutcome_due = \"{DUE}\"\nkind = \"weighted_empirical\"\nsamples = \"{samples}\"\nweights = \"{weights}\"\n"
    )
}

fn cohort(members: &str) -> String {
    format!(
        "[[cohort]]\nname = \"joint\"\nmembers = \"{members}\"\nresolution_rule = \"Synthetic complete period in declared units\"\neligible_sources = \"synthetic:issuer\"\nalpha = \"0.2\"\noutcome_lo = \"0\"\noutcome_hi = \"2\"\n"
    )
}

fn outcome(id: &str, value: &str, date: &str, restated: bool) -> String {
    let (kind, predecessor) = if restated {
        ("restated", "supersedes = 1\n")
    } else {
        ("published", "")
    };
    format!(
        "[[outcome]]\nforecast = \"{id}\"\nkind = \"{kind}\"\navailable_at = \"{date}\"\nvalue = \"{value}\"\nsource = \"synthetic:issuer\"\nvintage = \"invented-{kind}\"\nlocator = \"Synthetic!A1\"\nsha256 = \"{}\"\n{predecessor}",
        "ab".repeat(32),
    )
}

fn offline(root: &Path, text: &str) -> Json {
    fs::write(root.join("transcript.toml"), text).unwrap();
    let args = [
        "forecast",
        "assess",
        "--ledger",
        "transcript.toml",
        "--cohort",
        "joint",
        "--as-of",
        "2031-01-10T00:00:00Z",
    ]
    .map(str::to_string);
    checked(invoke(&args, root).0)
}

#[test]
fn offline_weighted_scores_preserve_mixed_cohort_denominators_and_decimal_quantile_boundaries() {
    let root = workspace();
    let register = |text: String| {
        text.replace(
            "model_key =",
            "registered_at = \"2030-01-02T00:00:00Z\"\nmodel_key =",
        )
    };
    let frozen = |members| {
        cohort(members).replace(
            "name =",
            "frozen_at = \"2030-01-03T00:00:00Z\"\nname =",
        )
    };
    let observed = |id, value| {
        outcome(id, value, "2031-01-01T00:00:00Z", false).replace(
            "available_at =",
            "recorded_at = \"2031-01-02T00:00:00Z\"\navailable_at =",
        )
    };
    let uniform = format!(
        "[[forecast]]\nid = \"uniform\"\nmodel_key = \"synthetic/uniform\"\noutcome_due = \"{DUE}\"\nkind = \"empirical\"\nsamples = \"0,2\"\n"
    );
    let gaussian = format!(
        "[[forecast]]\nid = \"normal\"\nmodel_key = \"synthetic/normal\"\noutcome_due = \"{DUE}\"\nkind = \"gaussian\"\nmean = \"0\"\nstd_dev = \"1\"\n"
    );
    let text = format!(
        "schema = \"fmine.forecast-ledger.v1\"\nlane = \"synthetic\"\n{}{}{}{}{}{}{}",
        register(forecast("weighted", "0,2", "1/4,3/4")),
        register(forecast("pending", "0,2", "1/4,3/4")),
        register(uniform),
        register(gaussian),
        frozen("weighted,pending,uniform,normal"),
        observed("weighted", "1"),
        observed("uniform", "1"),
    );
    let report = offline(&root, &text);
    let summary = field(&report, &["assessment", "summary"]);
    assert_eq!(field(summary, &["n_members"]).u64(), Some(4));
    assert_eq!(field(summary, &["n_resolved"]).u64(), Some(2));
    // Independent CRPS definition E|X-y| - E|X-X'|/2: 5/8 and 1/2.
    // Unresolved weighted minimum is 1/8; legacy Gaussian lower bound remains zero.
    near(field(summary, &["crps", "observed_mean"]), 9.0 / 16.0);
    near(field(summary, &["crps", "whole_cohort", "lower"]), 5.0 / 16.0);
    assert_eq!(
        field(summary, &["crps", "whole_cohort", "upper"]),
        &Json::Null
    );
    assert_eq!(
        field(summary, &["calibration_flag"]).as_str(),
        Some("descriptive")
    );
    assert_eq!(report, offline(&root, &text));

    let boundary = format!(
        "schema = \"fmine.forecast-ledger.v1\"\nlane = \"synthetic\"\n{}{}{}",
        register(forecast("boundary", "0,1,2", "1/10,4/5,1/10")),
        frozen("boundary"),
        observed("boundary", "2"),
    );
    let report = offline(&root, &boundary);
    // Exact decimal 10% and 90% CDF jumps select 0 and 1: width 1, plus penalty 10.
    near(
        field(
            &report,
            &["assessment", "summary", "interval_score", "observed_mean"],
        ),
        11.0,
    );
}

#[test]
fn weighted_journal_scores_completion_bounds_and_preserves_saved_results_after_restatement() {
    let mut fixture = Fixture::new();
    fixture.append(
        &forecast("weighted", "0,2", "1/4,3/4"),
        "2030-01-02T00:00:00Z",
    );
    fixture.append(&cohort("weighted"), "2030-01-03T00:00:00Z");
    let unresolved = fixture.assess(
        "2031-01-01T00:00:00Z",
        "first-published",
        "2031-01-02T00:00:00Z",
    );
    let score = field(&unresolved, &["report", "assessment", "summary", "crps"]);
    assert_eq!(field(score, &["n_scored"]).u64(), Some(0));
    assert_eq!(field(score, &["observed_mean"]), &Json::Null);
    near(field(score, &["whole_cohort", "lower"]), 1.0 / 8.0);
    near(field(score, &["whole_cohort", "upper"]), 9.0 / 8.0);
    fixture.append(
        &outcome("weighted", "1", "2031-01-03T00:00:00Z", false),
        "2031-01-04T00:00:00Z",
    );
    let first = fixture.assess(
        "2031-01-05T00:00:00Z",
        "first-published",
        "2031-01-06T00:00:00Z",
    );
    let retained = field(&first, &["report"]).clone();
    let id = digest(&retained, &["assessment", "digest"]);
    near(
        field(
            &retained,
            &["assessment", "summary", "crps", "observed_mean"],
        ),
        5.0 / 8.0,
    );
    assert_eq!(
        field(&retained, &["assessment", "summary", "calibration_flag"]).as_str(),
        Some("descriptive")
    );
    fixture.append(
        &outcome("weighted", "2", "2031-01-07T00:00:00Z", true),
        "2031-01-08T00:00:00Z",
    );
    let latest = fixture.assess(
        "2031-01-09T00:00:00Z",
        "latest-as-of",
        "2031-01-10T00:00:00Z",
    );
    near(
        field(
            &latest,
            &["report", "assessment", "summary", "crps", "observed_mean"],
        ),
        1.0 / 8.0,
    );
    let before = fixture.bytes();
    let saved = checked(execute_with_clock(
        &Command::Report {
            journal: JOURNAL.into(),
            head: Some(fixture.head),
            assessment: id,
            provenance: None,
            timestamp: None,
        },
        &fixture.root,
        &NoClock,
    ));
    assert_eq!(field(&saved, &["report"]), &retained);
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn invalid_weights_and_stale_heads_never_append_or_advance_authority_time() {
    let mut fixture = Fixture::new();
    for (samples, weights) in [
        ("0,2", ""),
        ("0,2", "1"),
        ("0,2", "0,1"),
        ("0,2", "-1/4,5/4"),
        ("0,2", "1/4,1/4"),
        ("0,2", "1/4,1"),
        ("0,2", "1/0,3/4"),
        ("0,2", "NaN,1"),
        ("0,2", "1.0e-2147483648,1"),
        ("0,2", "1/4,3/4,"),
        ("0,1,2", "0.1,0.2,0.6999999999999999"),
        ("0,inf", "1/4,3/4"),
    ] {
        let command = fixture.request(&forecast("invalid", samples, weights));
        let before = fixture.bytes();
        let refused =
            execute_with_clock(&command, &fixture.root, &Clock(ns("2030-01-10T00:00:00Z")));
        assert_ne!(refused.exit_code, 0, "accepted {samples:?} / {weights:?}");
        assert!(refused.refusal.is_some());
        assert_eq!(fixture.bytes(), before);
    }
    let stale = fixture.head;
    // Failed later attempts have not advanced the committed authority clock.
    fixture.append(
        &forecast("valid", "0,2", "0.25,3/4"),
        "2030-01-02T00:00:00Z",
    );
    let mut command = fixture.request(&forecast("other", "0,2", "1/4,3/4"));
    let Command::Register { head, .. } = &mut command else {
        panic!("register")
    };
    *head = stale;
    let before = fixture.bytes();
    let refused = execute_with_clock(&command, &fixture.root, &NoClock);
    assert_ne!(refused.exit_code, 0);
    assert_eq!(fixture.bytes(), before);
}

fn binary(root: &Path, args: &[&str]) -> Json {
    let output = Process::new(env!("CARGO_BIN_EXE_fmine"))
        .current_dir(root)
        .args(["forecast", "journal"])
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope = parse(&output.stdout).unwrap();
    field(&envelope, &["payload"]).clone()
}

#[test]
fn binary_v13_preserves_exact_weights_metadata_and_supplied_order_across_restart() {
    let root = workspace();
    let init = binary(
        &root,
        &[
            "init",
            "--journal",
            JOURNAL,
            "--cell",
            "synthetic-weighted-binary",
            "--max-clock-skew-ms",
            "1",
            "--trust-host-clock",
        ],
    );
    let initial = field(&init, &["head"]).as_str().unwrap();
    let request = format!("{}family = \"conditional-ensemble\"\nauthor_role = \"synthetic-model\"\ndomain = \"one-plant\"\nhorizon = \"one-period\"\nsite = \"synthetic\"\nquantity = \"recovered-metal\"\nunit = \"kg\"\n",
        forecast("weighted", "2,0,2", "1/4,0.25,1/2").replace(DUE, "2200-01-01T00:00:00Z"));
    fs::write(root.join("request.toml"), &request).unwrap();
    let registered = binary(
        &root,
        &[
            "register",
            "--journal",
            JOURNAL,
            "--head",
            initial,
            "--request",
            "request.toml",
            "--trust-host-clock",
        ],
    );
    let head = field(&registered, &["head"]).as_str().unwrap();
    let before = fs::read(root.join(JOURNAL)).unwrap();
    let inspected = binary(&root, &["inspect", "--journal", JOURNAL, "--head", head]);
    assert_eq!(
        field(&registered, &["forecasts"]),
        field(&inspected, &["forecasts"])
    );
    assert_eq!(
        field(&inspected, &["operation_version_counts", "v13"]).u64(),
        Some(1)
    );
    let row = &field(&inspected, &["forecasts"]).as_array().unwrap()[0];
    assert_eq!(
        field(row, &["metadata", "family"]).as_str(),
        Some("conditional-ensemble")
    );
    assert_eq!(
        field(row, &["payload", "kind"]).as_str(),
        Some("weighted_empirical")
    );
    assert_eq!(
        field(row, &["payload", "weight_kind"]).as_str(),
        Some("exact_probability")
    );
    assert_eq!(
        field(row, &["payload", "samples"]),
        &parse(b"[2,0,2]").unwrap()
    );
    assert_eq!(
        field(row, &["payload", "weights"]),
        &parse(br#"["1/4","1/4","1/2"]"#).unwrap()
    );
    assert_eq!(fs::read(root.join(JOURNAL)).unwrap(), before);
    let journal = Journal::open_read(&root.join(JOURNAL), None).unwrap();
    let Entry::Operation(operation) = &journal.records()[1].entry else {
        panic!("registered operation")
    };
    assert_eq!(
        operation,
        &format!("schema = \"fmine.forecast-operation.v13\"\n{request}")
    );
}

#[test]
fn old_operation_schemas_cannot_acquire_weighted_payload_semantics() {
    let root = workspace();
    for version in 1..=12 {
        let path = root.join(format!("version-{version}.journal"));
        // The byte store deliberately accepts opaque operations. Admission semantics belong to replay.
        let mut journal = Journal::create(
            &path,
            "synthetic-legacy",
            0,
            &Clock(ns("2030-01-01T00:00:00Z")),
        )
        .unwrap();
        let prefix = if version == 1 {
            String::new()
        } else {
            format!("schema = \"fmine.forecast-operation.v{version}\"\n")
        };
        journal
            .append(
                &format!("{prefix}{}", forecast("weighted", "0,2", "1/4,3/4")),
                &Clock(ns("2030-01-02T00:00:00Z")),
                |_, _| Ok(()),
            )
            .unwrap();
        let head = journal.head();
        drop(journal);
        let before = fs::read(&path).unwrap();
        let refused = execute_with_clock(
            &Command::Inspect {
                journal: path.clone(),
                head: Some(head),
            },
            &root,
            &NoClock,
        );
        assert_ne!(
            refused.exit_code, 0,
            "legacy v{version} acquired new semantics"
        );
        assert!(refused.refusal.unwrap().what.contains("weighted"));
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
