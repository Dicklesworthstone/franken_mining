//! DESIGN (BD-047/BD-048): empirical distributions survive public forecast parsing and durable replay.
//! Every clock, outcome and citation below is synthetic; none is source authentication or time proof.

#![cfg(test)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_calib::record::{parse_hex32, parse_utc};
use fmine_calib::scoring::{MAX_EMPIRICAL_SAMPLES, empirical_crps, empirical_interval};
use fmine_cli::forecast_journal::{Command, execute_with_clock};
use fmine_cli::{Envelope, invoke};
use fmine_core::Digest32;
use fmine_runtime::{ClockError, WallClock};

const JOURNAL: &str = "empirical.journal";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct FixedClock(i64);

impl WallClock for FixedClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        Ok(self.0)
    }
}

struct NeverClock;

impl WallClock for NeverClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        panic!("saved empirical assessments must not sample a clock")
    }
}

fn ns(utc: &str) -> i64 {
    parse_utc(utc).unwrap().checked_mul(1_000_000_000).unwrap()
}

fn field<'a>(json: &'a str, name: &str) -> &'a str {
    json.split_once(&format!("\"{name}\":\""))
        .unwrap_or_else(|| panic!("missing {name}: {json}"))
        .1
        .split('"')
        .next()
        .unwrap()
}

fn digest(json: &str, name: &str) -> Digest32 {
    Digest32(parse_hex32(field(json, name)).unwrap())
}

fn report(json: &str) -> &str {
    json.split_once("\"report\":")
        .unwrap()
        .1
        .strip_suffix('}')
        .unwrap()
}

struct Fixture {
    root: PathBuf,
    head: Digest32,
    requests: usize,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "fmine-empirical-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let mut fixture = Self {
            root,
            head: Digest32::default(),
            requests: 0,
        };
        fixture.ok(
            Command::Init {
                journal: JOURNAL.into(),
                cell_id: "synthetic-empirical".into(),
                clock_skew_ns: 0,
            },
            "2030-01-01T00:00:00Z",
        );
        fixture
    }

    fn ok(&mut self, command: Command, at: &str) -> Envelope {
        let output = execute_with_clock(&command, &self.root, &FixedClock(ns(at)));
        assert_eq!(output.exit_code, 0, "{}", output.to_json());
        self.head = digest(&output.payload_json, "head");
        output
    }

    fn request(&mut self, text: &str) -> Command {
        let request = PathBuf::from(format!("operation-{}.toml", self.requests));
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
            other => panic!("unexpected fixture operation {other}"),
        }
    }

    fn append(&mut self, text: &str, at: &str) -> Envelope {
        let command = self.request(text);
        self.ok(command, at)
    }

    fn assess(&mut self, cut: &str, selector: &str, at: &str) -> Envelope {
        self.ok(
            Command::Assess {
                journal: JOURNAL.into(),
                head: self.head,
                cohort: "ensemble".into(),
                as_of: cut.into(),
                selector: selector.into(),
                expected_digest: None,
            },
            at,
        )
    }
}

fn forecast(alias: &str, samples: &str) -> String {
    format!(
        "[[forecast]]\nid = \"{alias}\"\nmodel_key = \"synthetic/ensemble\"\noutcome_due = \"2030-12-31T00:00:00Z\"\nkind = \"empirical\"\nsamples = \"{samples}\"\n"
    )
}

fn cohort(members: &str) -> String {
    format!(
        "[[cohort]]\nname = \"ensemble\"\nmembers = \"{members}\"\nresolution_rule = \"Synthetic annual production in fixed units\"\neligible_sources = \"synthetic:issuer\"\nalpha = \"0.2\"\nmin_calibration_n = 35\nrho_low = \"0\"\nrho_high = \"0.5\"\n"
    )
}

fn outcome(value: i64, restated: bool) -> String {
    let (kind, date, revision) = if restated {
        ("restated", "2031-01-10T00:00:00Z", "supersedes = 1\n")
    } else {
        ("published", "2031-01-01T00:00:00Z", "")
    };
    format!(
        "[[outcome]]\nforecast = \"mixture\"\nkind = \"{kind}\"\navailable_at = \"{date}\"\nvalue = \"{value}\"\nsource = \"synthetic:issuer\"\nvintage = \"synthetic-{kind}\"\nlocator = \"Invented!A1\"\nsha256 = \"{}\"\n{revision}",
        "11".repeat(32)
    )
}

// Independent O(n²) definition; the candidate integrates sorted ECDF gaps instead.
fn quadratic_crps(samples: &[f64], outcome: f64) -> f64 {
    let n = samples.len() as f64;
    let error: f64 = samples.iter().map(|x| (x - outcome).abs()).sum();
    let pairwise: f64 = samples
        .iter()
        .flat_map(|x| samples.iter().map(move |z| (x - z).abs()))
        .sum();
    error / n - pairwise / (2.0 * n * n)
}

#[test]
fn empirical_scores_match_an_independent_definition_and_inverse_cdf_boundaries() {
    for samples in [
        vec![7.0],
        vec![7.0; 4],
        vec![0.0, 0.0, 10.0, 10.0],
        vec![0.0, 0.0, 0.0, 8.0],
        vec![1.0, 2.0, 4.0, 8.0],
    ] {
        for y in [-3.0, 0.0, 2.0, 5.0, 10.0, 20.0] {
            let expected = quadratic_crps(&samples, y);
            let actual = empirical_crps(&samples, y).unwrap();
            assert!((actual - expected).abs() <= 1e-12, "{samples:?}, y={y}");
            let mut reverse = samples.clone();
            reverse.reverse();
            assert_eq!(actual, empirical_crps(&reverse, y).unwrap());
            let repeated = [samples.clone(), samples.clone()].concat();
            assert_eq!(actual, empirical_crps(&repeated, y).unwrap());
        }
    }
    let base = 2_f64.powi(50);
    let translated = [base + 1.0, base + 2.0, base + 4.0, base + 8.0];
    assert_eq!(empirical_crps(&translated, base + 3.0).unwrap(), 0.8125);
    let hundred: Vec<f64> = (0..100).map(f64::from).collect();
    // 0.07 * 100 rounds above seven; ceil(p*n) would incorrectly choose the eighth sample.
    assert_eq!(empirical_interval(&hundred, 0.14).unwrap(), (6.0, 92.0));
    assert_eq!(empirical_interval(&[7.0; 4], 0.2).unwrap(), (7.0, 7.0));
}

#[test]
fn transcript_keeps_empirical_shape_and_the_entire_mixed_cohort_denominator() {
    let fixture = Fixture::new();
    let register = |alias: &str, samples: &str| {
        forecast(alias, samples).replace(
            "model_key =",
            "registered_at = \"2030-01-02T00:00:00Z\"\nmodel_key =",
        )
    };
    let unresolved = "[[forecast]]\nid = \"pending\"\nmodel_key = \"synthetic/normal\"\nregistered_at = \"2030-01-02T00:00:00Z\"\noutcome_due = \"2030-12-31T00:00:00Z\"\nkind = \"gaussian\"\nmean = \"0\"\nstd_dev = \"1\"\n";
    let frozen = cohort("mixture, skewed, pending, pending-empirical")
        .replace("name =", "frozen_at = \"2030-01-03T00:00:00Z\"\nname =");
    let first = outcome(5, false).replace(
        "available_at =",
        "recorded_at = \"2031-01-02T00:00:00Z\"\navailable_at =",
    );
    let second = first
        .replace("\"mixture\"", "\"skewed\"")
        .replace("value = \"5\"", "value = \"2\"");
    let text = format!(
        "schema = \"fmine.forecast-ledger.v1\"\nlane = \"synthetic\"\n{}{}{}{}{}{}{}",
        register("mixture", "0,0,10,10"),
        register("skewed", "0,0,0,8"),
        register("pending-empirical", "10,0,10,0"),
        unresolved,
        frozen,
        first,
        second
    );
    fs::write(fixture.root.join("ledger.toml"), text).unwrap();
    let args = [
        "forecast",
        "assess",
        "--ledger",
        "ledger.toml",
        "--cohort",
        "ensemble",
        "--as-of",
        "2031-01-03T00:00:00Z",
    ]
    .map(str::to_string);
    let (output, _) = invoke(&args, &fixture.root);
    assert_eq!(output.exit_code, 0, "{}", output.to_json());
    let json = &output.payload_json;
    assert!(json.contains("\"samples\":[0,0,10,10]"), "{json}");
    assert!(json.contains("\"weight_kind\":\"uniform\""));
    assert!(json.contains("\"quantile_rule\":\"inverse_ecdf\""));
    assert!(json.contains("\"n_members\":4,\"n_resolved\":2,\"n_unresolved\":2"));
    // Observed losses 2.5 + 1.5, unresolved empirical minimum 2.5, Gaussian lower bound zero.
    assert!(json.contains("\"crps\":{\"n_scored\":2,\"observed_mean\":2,\"whole_cohort\":{\"lower\":1.625,\"upper\":null,\"upper_unbounded\":true}}"), "{json}");
    assert!(json.contains("\"mean_standardized_bias\":null"));
    assert!(json.contains("\"calibration_flag\":\"descriptive\""));
    assert_eq!(output.to_json(), invoke(&args, &fixture.root).0.to_json());
}

#[test]
fn empirical_journal_preserves_saved_assessments_across_restatement() {
    let mut fixture = Fixture::new();
    fixture.append(&forecast("mixture", "0,0,10,10"), "2030-01-02T00:00:00Z");
    fixture.append(&cohort("mixture"), "2030-01-03T00:00:00Z");
    fixture.append(&outcome(5, false), "2031-01-02T00:00:00Z");
    let early = fixture.assess(
        "2031-01-03T00:00:00Z",
        "first-published",
        "2031-01-04T00:00:00Z",
    );
    let frozen_report = report(&early.payload_json).to_string();
    let id = digest(
        frozen_report.split_once("\"assessment\":{").unwrap().1,
        "digest",
    );
    assert!(frozen_report.contains("\"crps\":{\"n_scored\":1,\"observed_mean\":2.5"));
    fixture.append(&outcome(20, true), "2031-01-11T00:00:00Z");
    let latest = fixture.assess(
        "2031-01-12T00:00:00Z",
        "latest-as-of",
        "2031-01-13T00:00:00Z",
    );
    assert!(
        latest
            .payload_json
            .contains("\"crps\":{\"n_scored\":1,\"observed_mean\":12.5")
    );
    let before = fs::read(fixture.root.join(JOURNAL)).unwrap();
    let saved = execute_with_clock(
        &Command::Report {
            journal: JOURNAL.into(),
            head: Some(fixture.head),
            assessment: id,
            provenance: None,
            timestamp: None,
        },
        &fixture.root,
        &NeverClock,
    );
    assert_eq!(saved.exit_code, 0, "{}", saved.to_json());
    assert_eq!(report(&saved.payload_json), frozen_report);
    assert_eq!(fs::read(fixture.root.join(JOURNAL)).unwrap(), before);
}

#[test]
fn malformed_empirical_registrations_are_atomic_and_declared_order_is_retained() {
    let mut fixture = Fixture::new();
    let too_many = vec!["1"; MAX_EMPIRICAL_SAMPLES + 1].join(",");
    let mut requests = ["", "NaN", "0,inf", "0,-inf", "0,1,"]
        .map(|samples| forecast("invalid", samples))
        .to_vec();
    requests.push(forecast("invalid", &too_many));
    requests.push(format!(
        "{}weights = \"0.5,0.5\"\n",
        forecast("invalid", "0,1")
    ));
    for request in requests {
        let command = fixture.request(&request);
        let before = fs::read(fixture.root.join(JOURNAL)).unwrap();
        let output = execute_with_clock(
            &command,
            &fixture.root,
            &FixedClock(ns("2030-01-10T00:00:00Z")),
        );
        assert_ne!(
            output.exit_code, 0,
            "request unexpectedly accepted: {request}"
        );
        assert!(output.refusal.is_some());
        assert_eq!(fs::read(fixture.root.join(JOURNAL)).unwrap(), before);
    }
    // Failed requests did not advance the journal clock; this earlier valid registration remains legal.
    let original = fixture.append(&forecast("original", "0,0,10,10"), "2030-01-02T00:00:00Z");
    let original_id = field(&original.payload_json, "forecast_id").to_string();
    let permuted = fixture.append(&forecast("permuted", "10,0,10,0"), "2030-01-02T00:00:00Z");
    let row = permuted
        .payload_json
        .split_once("\"alias\":\"permuted\"")
        .unwrap()
        .1;
    assert_ne!(field(row, "forecast_id"), original_id);
    assert!(row.contains("\"samples\":[10,0,10,0]"));
    let point = fixture.append(&forecast("point", "7"), "2030-01-02T00:00:00Z");
    assert!(point.payload_json.contains("\"samples\":[7]"));
}
