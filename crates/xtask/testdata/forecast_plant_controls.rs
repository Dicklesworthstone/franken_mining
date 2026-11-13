//! FACT: synthetic source worksheets exercise real admission, durable replay and interval scoring.
//! The declared host-clock bound and supplied outcome are not external evidence or plant validation.

use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_calib::ledger::ForecastPayload;
use fmine_claim::forecast_journal::Entry;
use fmine_io::forecast::Journal;
use fmine_runtime::{ClockError, WallClock};

use super::super::live::{Command, execute_with_clock};
use super::super::{Replay, Row, parse_subset};

const COUPLING: &str = include_str!("../../../tests/plant/production_joint.toml");
const GRINDING: &str = include_str!("../../../tests/plant/grinding_joint.toml");
const RECOVERY: &str = include_str!("../../../tests/plant/production_recovery.toml");
const HOUR: i64 = 3_600_000_000_000;

struct Clock(i64);
impl WallClock for Clock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        Ok(self.0)
    }
}
struct NoClock;
impl WallClock for NoClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        panic!("read-only or rejected admission must not sample the clock")
    }
}

fn fixture(start: i64) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "fmine-plant-forecast-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&root).expect("private synthetic workspace");
    for (path, text) in [
        ("production.toml", COUPLING),
        ("grinding.toml", GRINDING),
        ("recovery.toml", RECOVERY),
    ] {
        let text = text
            .replace("start_ns = \"0\"", &format!("start_ns = \"{start}\""))
            .replace(
                "end_ns = \"3600000000000\"",
                &format!("end_ns = \"{}\"", start + HOUR),
            );
        std::fs::write(root.join(path), text).expect("synthetic worksheet");
    }
    root
}

fn registration(target: Target) -> Registration<'static> {
    Registration {
        input: Path::new("production.toml"),
        grinding: Path::new("grinding.toml"),
        recovery: Path::new("recovery.toml"),
        id: "production-1",
        target,
        author_role: "synthetic-model-role",
        time_basis: "utc-unix-ns",
    }
}

fn registered(root: &Path, target: Target, registered_ns: i64) -> (Prepared, Replay) {
    let prepared = prepare(&registration(target), root).expect("source-bound forecast request");
    let doc = parse_subset(&prepared.request).expect("existing forecast grammar");
    assert!(doc.top.is_empty());
    assert_eq!(doc.rows.len(), 1);
    registration_start(&Row(&doc.rows[0]), &prepared.request).expect("typed plant projection");
    let mut forecast = doc.rows[0].clone();
    forecast.table = "forecast".into();
    forecast
        .fields
        .retain(|field| field.key != "registration_before");
    let mut state = Replay::default();
    state
        .forecast_at(Row(&forecast), Some(registered_ns))
        .expect("unchanged forecast owner");
    (prepared, state)
}

fn values(state: &Replay) -> &[f64] {
    let id = state.forecasts["production-1"];
    let row = state.ledger.forecast(id).expect("registered forecast");
    let ForecastPayload::Quantiles { levels, values } = &row.payload else {
        panic!("weighted populations must not become uniform empirical draws")
    };
    assert_eq!(levels, &[0.1, 0.5, 0.9]);
    values
}

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
}

#[test]
fn source_quantiles_preserve_physical_target_element_units_and_entire_receipt() {
    let start = time("2030-01-02T00:00:00Z").expect("UTC");
    let root = fixture(start);
    let loaded = crate::plant::production::load(
        Path::new("production.toml"),
        Path::new("grinding.toml"),
        Path::new("recovery.toml"),
        &root,
    )
    .expect("joint source model");
    let cases = [
        (
            Target::Recovery,
            [0.6, 0.6, 0.9],
            "fraction",
            "model_recovery_fraction:Au",
        ),
        (
            Target::Throughput,
            [80.0, 80.0, 320.0],
            "dry_metric_tonnes_per_hour",
            "dry_feed_throughput",
        ),
        (
            Target::DryTonnes,
            [80.0, 80.0, 320.0],
            "dry_metric_tonnes",
            "dry_feed_mass",
        ),
        (
            Target::ContainedMetal,
            [0.32, 0.32, 0.32],
            "kg",
            "contained_feed_metal:Au",
        ),
        (
            Target::RecoveredMetal,
            [0.192, 0.192, 0.288],
            "kg",
            "model_recovered_metal:Au",
        ),
    ];
    for (target, expected, unit, quantity) in cases {
        let (prepared, state) = registered(&root, target, start - 10);
        for (actual, expected) in values(&state).iter().zip(expected) {
            near(*actual, expected);
        }
        let row = state
            .ledger
            .forecast(state.forecasts["production-1"])
            .unwrap();
        assert_eq!(row.freeze_time_ns, start + HOUR);
        let metadata = row.metadata.as_ref().expect("prospective metadata");
        assert_eq!(metadata.unit, unit);
        assert_eq!(metadata.quantity, quantity);
        assert_eq!(metadata.site, "synthetic-plant/v1");
        assert_eq!(
            metadata.horizon,
            "UTC [2030-01-02T00:00:00Z,2030-01-02T01:00:00Z)"
        );
        assert!(row.model_key.contains(loaded.receipt().digest()));
        assert!(prepared.evidence_json.contains(&loaded.to_json()));
        assert!(
            prepared
                .request
                .starts_with(&format!("{EVIDENCE_PREFIX}{}\n", prepared.evidence_json))
        );
        assert_eq!(
            prepared.request,
            prepare(&registration(target), &root).unwrap().request
        );
    }
    let prepared = prepare(&registration(Target::RecoveredMetal), &root).unwrap();
    let mismatched = prepared.request.replacen(
        "\"target\":\"recovered-metal\"",
        "\"target\":\"contained-metal\"",
        1,
    );
    let doc = parse_subset(&mismatched).unwrap();
    assert_eq!(
        registration_start(&Row(&doc.rows[0]), &mismatched)
            .unwrap_err()
            .code,
        "FMINE-REF-107",
    );
}

#[test]
fn arbitrary_exact_probability_mass_is_not_replaced_by_uniform_member_counts() {
    let start = time("2030-01-02T00:00:00Z").unwrap();
    let root = fixture(start);
    for path in ["production.toml", "grinding.toml", "recovery.toml"] {
        let text = std::fs::read_to_string(root.join(path)).unwrap();
        let text = text
            .replace(
                "id = \"soft\"\nweight = \"1/2\"",
                "id = \"soft\"\nweight = \"1/20\"",
            )
            .replace(
                "id = \"hard\"\nweight = \"1/2\"",
                "id = \"hard\"\nweight = \"19/20\"",
            );
        std::fs::write(root.join(path), text).unwrap();
    }
    let (prepared, state) = registered(&root, Target::RecoveredMetal, start - 10);
    for actual in values(&state) {
        near(*actual, 0.288);
    }
    assert!(
        prepared.evidence_json.contains("19/20")
            || prepared.evidence_json.contains("\"numerator\":\"19\"")
    );
    assert!(
        prepared
            .evidence_json
            .contains("not CRPS or full-distribution scoring")
    );
}

fn head(root: &Path) -> fmine_core::Digest32 {
    Journal::open_read(&root.join("history"), None)
        .unwrap()
        .head()
}

fn command(root: &Path, target: Target) -> Command {
    Command::RegisterPlant {
        journal: "history".into(),
        head: head(root),
        input: "production.toml".into(),
        grinding: "grinding.toml".into(),
        recovery: "recovery.toml".into(),
        id: "production-1".into(),
        target,
        author_role: "synthetic-model-role".into(),
        time_basis: "utc-unix-ns".into(),
    }
}

fn init(root: &Path, now: i64) {
    let envelope = execute_with_clock(
        &Command::Init {
            journal: "history".into(),
            cell_id: "synthetic-plant".into(),
            clock_skew_ns: 7,
        },
        root,
        &Clock(now),
    );
    assert_eq!(envelope.exit_code, 0, "{:?}", envelope.refusal);
}

#[test]
fn plant_registration_reopens_and_scores_with_full_admission_evidence_retained() {
    let start = time("2030-01-02T00:00:00Z").unwrap();
    let root = fixture(start);
    init(&root, start - 1_000);
    let prepared = prepare(&registration(Target::RecoveredMetal), &root).unwrap();
    let admitted = execute_with_clock(
        &command(&root, Target::RecoveredMetal),
        &root,
        &Clock(start - 100),
    );
    assert_eq!(admitted.exit_code, 0, "{:?}", admitted.refusal);
    assert!(admitted.payload_json.contains(&prepared.evidence_json));
    assert!(admitted.payload_json.contains("\"v9\":1"));
    assert!(
        admitted
            .payload_json
            .contains(&format!("\"registration_before_ns\":{start}"))
    );
    let saved = Journal::open_read(&root.join("history"), None).unwrap();
    let Entry::Operation(operation) = &saved.records()[1].entry else {
        panic!("forecast operation")
    };
    assert!(operation.starts_with("schema = \"fmine.forecast-operation.v9\"\n"));
    assert!(operation.contains(&prepared.evidence_json));
    let saved_head = saved.head();
    drop(saved);

    std::fs::write(root.join("grinding.toml"), "changed after registration").unwrap();
    let inspected = execute_with_clock(
        &Command::Inspect {
            journal: "history".into(),
            head: Some(saved_head),
        },
        &root,
        &NoClock,
    );
    assert_eq!(inspected.exit_code, 0);
    assert_eq!(head(&root), saved_head);
    assert!(inspected.payload_json.contains("\"kind\":\"quantiles\""));

    std::fs::write(root.join("cohort.toml"), "[[cohort]]\nname = \"synthetic-production\"\nmembers = \"production-1\"\nresolution_rule = \"Supplied elemental production for the frozen complete physical interval\"\neligible_sources = \"synthetic:mill-report\"\noutcome_lo = \"0\"\noutcome_hi = \"1\"\n").unwrap();
    let frozen = execute_with_clock(
        &Command::Freeze {
            journal: "history".into(),
            head: head(&root),
            request: "cohort.toml".into(),
        },
        &root,
        &Clock(start - 50),
    );
    assert_eq!(frozen.exit_code, 0, "{:?}", frozen.refusal);
    let frozen_head = head(&root);
    std::fs::write(root.join("early.toml"), format!("[[outcome]]\nforecast = \"production-1\"\navailable_at = \"2030-01-02T00:30:00Z\"\nkind = \"published\"\nvalue = \"0.3\"\nsource = \"synthetic:mill-report\"\nvintage = \"invented-partial-period\"\nlocator = \"Synthetic!A1\"\nsha256 = \"{}\"\n", "a".repeat(64))).unwrap();
    let early = execute_with_clock(
        &Command::Outcome {
            journal: "history".into(),
            head: frozen_head,
            request: "early.toml".into(),
        },
        &root,
        &Clock(start + HOUR / 2 + NS_PER_SECOND),
    );
    assert_eq!(early.refusal.unwrap().code, "FMINE-REF-206");
    assert_eq!(head(&root), frozen_head);
    std::fs::write(root.join("outcome.toml"), format!("[[outcome]]\nforecast = \"production-1\"\navailable_at = \"2030-01-02T01:00:01Z\"\nkind = \"published\"\nvalue = \"0.3\"\nsource = \"synthetic:mill-report\"\nvintage = \"invented-period\"\nlocator = \"Synthetic!A1\"\nsha256 = \"{}\"\n", "a".repeat(64))).unwrap();
    let observed = execute_with_clock(
        &Command::Outcome {
            journal: "history".into(),
            head: head(&root),
            request: "outcome.toml".into(),
        },
        &root,
        &Clock(start + HOUR + 2 * NS_PER_SECOND),
    );
    assert_eq!(observed.exit_code, 0, "{:?}", observed.refusal);
    let assessment = execute_with_clock(
        &Command::Assess {
            journal: "history".into(),
            head: head(&root),
            cohort: "synthetic-production".into(),
            as_of: "2030-01-02T01:00:03Z".into(),
            selector: "first-published".into(),
            expected_digest: None,
        },
        &root,
        &Clock(start + HOUR + 4 * NS_PER_SECOND),
    );
    assert_eq!(assessment.exit_code, 0, "{:?}", assessment.refusal);
    let marker = "\"interval_score\":{\"n_scored\":1,\"observed_mean\":";
    let mean = assessment
        .payload_json
        .split_once(marker)
        .expect("interval score")
        .1
        .split(',')
        .next()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    near(mean, 0.216);
    assert!(assessment.payload_json.contains("\"crps\":null"));
    assert!(
        assessment
            .payload_json
            .contains("\"source_citations\":\"supplied_not_verified\"")
    );
    let reopened = Journal::open_read(&root.join("history"), None).unwrap();
    let Entry::Operation(original) = &reopened.records()[1].entry else {
        panic!("operation")
    };
    assert_eq!(
        original,
        &format!(
            "schema = \"fmine.forecast-operation.v9\"\n{}",
            prepared.request
        )
    );
}

#[test]
fn registration_clock_upper_must_precede_start_and_refusals_leave_journal_unchanged() {
    let start = time("2030-01-02T00:00:00Z").unwrap();
    let root = fixture(start);
    init(&root, start - 1_000);
    let original = head(&root);
    let refused = execute_with_clock(&command(&root, Target::Recovery), &root, &Clock(start - 7));
    assert_eq!(refused.refusal.unwrap().code, "FMINE-REF-206");
    assert_eq!(head(&root), original);
    let mut invalid = command(&root, Target::Recovery);
    let Command::RegisterPlant { time_basis, .. } = &mut invalid else {
        panic!("command")
    };
    *time_basis = "relative-process-time".into();
    let refused = execute_with_clock(&invalid, &root, &NoClock);
    assert_eq!(refused.refusal.unwrap().code, "FMINE-REF-107");
    assert_eq!(head(&root), original);
    let prepared = prepare(&registration(Target::Recovery), &root).unwrap();
    std::fs::write(root.join("forged-request.toml"), prepared.request).unwrap();
    let refused = execute_with_clock(
        &Command::Register {
            journal: "history".into(),
            head: original,
            request: "forged-request.toml".into(),
        },
        &root,
        &NoClock,
    );
    assert_eq!(refused.refusal.unwrap().code, "FMINE-REF-107");
    assert_eq!(head(&root), original);
    let admitted = execute_with_clock(
        &command(&root, Target::Recovery),
        &root,
        &Clock(start - 100),
    );
    assert_eq!(admitted.exit_code, 0, "{:?}", admitted.refusal);
    let registered_head = head(&root);
    std::fs::write(root.join("late-cohort.toml"), "[[cohort]]\nname = \"late\"\nmembers = \"production-1\"\nresolution_rule = \"Synthetic complete interval recovery\"\neligible_sources = \"synthetic:plant\"\n").unwrap();
    let late_cohort = execute_with_clock(
        &Command::Freeze {
            journal: "history".into(),
            head: registered_head,
            request: "late-cohort.toml".into(),
        },
        &root,
        &Clock(start - 7),
    );
    assert_eq!(late_cohort.refusal.unwrap().code, "FMINE-REF-206");
    assert_eq!(head(&root), registered_head);
    std::fs::write(root.join("recovery.toml"), "unreadable-model = true").unwrap();
    let refused = execute_with_clock(&command(&root, Target::Recovery), &root, &NoClock);
    assert_ne!(refused.exit_code, 0);
    assert_eq!(head(&root), registered_head);
}

#[test]
fn fractional_second_support_and_oversized_evidence_refuse_without_rounding_or_truncation() {
    let start = time("2030-01-02T00:00:00Z").unwrap();
    let fractional = fixture(start + 1);
    assert_eq!(
        prepare(&registration(Target::Recovery), &fractional)
            .err()
            .unwrap()
            .code,
        "FMINE-REF-107"
    );
    let root = fixture(start);
    let loaded = crate::plant::production::load(
        Path::new("production.toml"),
        Path::new("grinding.toml"),
        Path::new("recovery.toml"),
        &root,
    )
    .unwrap();
    let oversized = format!("{{\"retained\":\"{}\"}}", "x".repeat(MAX_OPERATION_BYTES));
    let refusal = from_receipt(
        &registration(Target::Recovery),
        loaded.receipt(),
        &oversized,
    )
    .err()
    .unwrap();
    assert!(refusal.what.contains("never dropped or truncated"));
}
