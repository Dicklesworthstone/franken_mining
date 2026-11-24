//! FACT: synthetic worksheets exercise source admission, durable replay and weighted-distribution scoring.
//! The declared host-clock bound and supplied outcome are not external evidence or plant validation.

use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_calib::ledger::ForecastPayload;
use fmine_claim::forecast_journal::Entry;
use fmine_io::forecast::Journal;
use fmine_runtime::{ClockError, WallClock};
use fmine_units::ExactQty;

use super::super::live::{Command, execute_with_clock};
use super::super::{Replay, Row, parse_subset};

#[path = "forecast_plant_comparison.rs"]
mod supplied_comparison;

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

fn population(state: &Replay) -> (&[f64], &[ExactQty]) {
    let id = state.forecasts["production-1"];
    let row = state.ledger.forecast(id).expect("registered forecast");
    let ForecastPayload::WeightedEmpirical { samples, weights } = &row.payload else {
        panic!("complete weighted populations must not become three quantiles or uniform draws")
    };
    (samples, weights)
}

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
}

#[test]
fn source_populations_preserve_every_physical_target_element_units_and_entire_receipt() {
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
            [0.9, 0.6],
            "fraction",
            "model_recovery_fraction:Au",
        ),
        (
            Target::Throughput,
            [80.0, 320.0],
            "dry_metric_tonnes_per_hour",
            "dry_feed_throughput",
        ),
        (
            Target::DryTonnes,
            [80.0, 320.0],
            "dry_metric_tonnes",
            "dry_feed_mass",
        ),
        (
            Target::ContainedMetal,
            [0.32, 0.32],
            "kg",
            "contained_feed_metal:Au",
        ),
        (
            Target::RecoveredMetal,
            [0.288, 0.192],
            "kg",
            "model_recovered_metal:Au",
        ),
    ];
    assert_eq!(
        loaded
            .receipt()
            .points()
            .iter()
            .map(|point| point.scenario_id.as_str())
            .collect::<Vec<_>>(),
        ["hard", "soft"],
    );
    for (target, expected, unit, quantity) in cases {
        let (prepared, state) = registered(&root, target, start - 10);
        let (samples, weights) = population(&state);
        assert_eq!(samples.len(), expected.len());
        assert_eq!(weights, &[ExactQty::new(1, 2).unwrap(); 2]);
        for (actual, expected) in samples.iter().zip(expected) {
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
        assert_eq!(metadata.family, FAMILY);
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
    // Both altered populations are otherwise physically valid. The retained receipt must still bind
    // the registered projection, rather than lending provenance to a caller's replacement distribution.
    for (field, replacement) in [("samples", "0.1,0.2"), ("weights", "1/4,3/4")] {
        let old = prepared
            .request
            .lines()
            .find(|line| line.starts_with(&format!("{field} = ")))
            .unwrap();
        let changed = prepared
            .request
            .replacen(old, &format!("{field} = \"{replacement}\""), 1);
        let doc = parse_subset(&changed).unwrap();
        assert_eq!(
            registration_start(&Row(&doc.rows[0]), &changed)
                .unwrap_err()
                .code,
            "FMINE-REF-107"
        );
    }
}

fn weights(root: &Path, soft: &str, hard: &str) {
    for path in ["production.toml", "grinding.toml", "recovery.toml"] {
        let text = std::fs::read_to_string(root.join(path)).unwrap();
        let text = text
            .replace(
                "id = \"soft\"\nweight = \"1/2\"",
                &format!("id = \"soft\"\nweight = \"{soft}\""),
            )
            .replace(
                "id = \"hard\"\nweight = \"1/2\"",
                &format!("id = \"hard\"\nweight = \"{hard}\""),
            );
        std::fs::write(root.join(path), text).unwrap();
    }
}

#[test]
fn arbitrary_exact_probability_mass_is_not_replaced_by_uniform_member_counts() {
    let start = time("2030-01-02T00:00:00Z").unwrap();
    let root = fixture(start);
    weights(&root, "1/20", "19/20");
    let (prepared, state) = registered(&root, Target::RecoveredMetal, start - 10);
    let (samples, weights) = population(&state);
    assert_eq!(samples.len(), 2);
    near(samples[0], 0.288);
    near(samples[1], 0.192);
    assert_eq!(
        weights,
        &[
            ExactQty::new(19, 20).unwrap(),
            ExactQty::new(1, 20).unwrap()
        ]
    );
    let (lo, hi) =
        fmine_calib::scoring::weighted_empirical_interval(samples, weights, 0.2).unwrap();
    near(lo, 0.288);
    near(hi, 0.288);
    near(
        fmine_calib::scoring::weighted_empirical_crps(samples, weights, 0.3).unwrap(),
        0.01224,
    );
    assert!(
        prepared.evidence_json.contains("19/20")
            || prepared.evidence_json.contains("\"numerator\":\"19\"")
    );
    assert!(
        prepared
            .evidence_json
            .contains("fmine.plant-forecast-admission.v2")
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

fn verification(root: &Path) -> Command {
    Command::VerifyPlant {
        journal: "history".into(),
        head: Some(head(root)),
        forecast: "production-1".into(),
    }
}

#[test]
fn offline_verification_recomputes_all_targets_after_original_source_paths_disappear() {
    let start = time("2030-01-02T00:00:00Z").unwrap();
    for target in [
        Target::Recovery,
        Target::Throughput,
        Target::DryTonnes,
        Target::ContainedMetal,
        Target::RecoveredMetal,
    ] {
        let root = fixture(start);
        init(&root, start - 1_000);
        let admitted = execute_with_clock(&command(&root, target), &root, &Clock(start - 100));
        assert_eq!(admitted.exit_code, 0, "{:?}", admitted.refusal);
        for name in ["production", "grinding", "recovery"] {
            std::fs::rename(
                root.join(format!("{name}.toml")),
                root.join(format!("{name}.saved")),
            )
            .unwrap();
        }
        let before = std::fs::read(root.join("history")).unwrap();
        let checked = execute_with_clock(&verification(&root), &root, &NoClock);
        assert_eq!(checked.exit_code, 0, "{:?}", checked.refusal);
        let json = parse_json(checked.payload_json.as_bytes()).unwrap();
        let result = json.get("verification").unwrap();
        assert_eq!(result.get("status").unwrap().as_str(), Some("verified"));
        assert_eq!(
            result.get("target").unwrap().as_str(),
            Some(target.as_str())
        );
        assert_eq!(
            result.get("source_files_opened").unwrap().as_bool(),
            Some(false)
        );
        assert_eq!(result.get("scenario_count").unwrap().u64(), Some(2));
        let production = result
            .get("admission")
            .unwrap()
            .get("source_receipt")
            .unwrap()
            .get("production")
            .unwrap();
        near(
            production
                .get("summary")
                .unwrap()
                .get("recovered_metal_kg")
                .unwrap()
                .get("mean")
                .unwrap()
                .f64()
                .unwrap(),
            0.24,
        );
        assert_eq!(std::fs::read(root.join("history")).unwrap(), before);
        assert!(
            checked
                .rendered_markdown
                .unwrap()
                .contains("Verified all three retained source worksheets")
        );
    }
}

/// The byte store deliberately accepts a self-consistent chain. Ordinary semantic replay must also
/// accept it, demonstrating that explicit recomputation catches more than a broken journal hash.
fn forged_history(root: &Path, start: i64, request: &str) {
    init(root, start - 1_000);
    let mut journal = Journal::open_write(&root.join("history"), head(root)).unwrap();
    journal
        .append(
            &format!("schema = \"fmine.forecast-operation.v14\"\n{request}"),
            &Clock(start - 100),
            |_, _| Ok(()),
        )
        .unwrap();
    drop(journal);
    let inspected = execute_with_clock(
        &Command::Inspect {
            journal: "history".into(),
            head: Some(head(root)),
        },
        root,
        &NoClock,
    );
    assert_eq!(inspected.exit_code, 0, "{:?}", inspected.refusal);
}

#[test]
fn offline_verification_rejects_self_consistent_forged_populations_receipts_and_metadata() {
    let start = time("2030-01-02T00:00:00Z").unwrap();
    for alteration in [
        "samples",
        "weights",
        "truncation",
        "order",
        "scenario",
        "mean",
        "site",
        "element",
    ] {
        let root = fixture(start);
        let loaded = crate::plant::production::load(
            Path::new("production.toml"),
            Path::new("grinding.toml"),
            Path::new("recovery.toml"),
            &root,
        )
        .unwrap();
        let mut samples: Vec<_> = loaded
            .receipt()
            .points()
            .iter()
            .map(|p| p.recovered_metal.value())
            .collect();
        let mut weights: Vec<_> = loaded.receipt().points().iter().map(|p| p.weight).collect();
        let mut source = loaded.to_json();
        match alteration {
            "samples" => samples = vec![0.1, 0.2],
            "weights" => weights = vec![ExactQty::new(1, 4).unwrap(), ExactQty::new(3, 4).unwrap()],
            "truncation" => {
                samples.truncate(1);
                weights = vec![ExactQty::from_integer(1)];
            }
            "order" => samples.reverse(),
            "scenario" => {
                source = source.replacen("\"id\":\"hard\"", "\"id\":\"invented-hard\"", 1)
            }
            "mean" => {
                source = source.replace(
                    &format!(
                        "\"mean\":{}",
                        loaded.receipt().summary().recovered_metal.mean.value()
                    ),
                    "\"mean\":0.375",
                )
            }
            _ => {}
        }
        let prepared = from_projection(
            &registration(Target::RecoveredMetal),
            loaded.receipt(),
            &source,
            ForecastPayload::WeightedEmpirical { samples, weights },
        )
        .unwrap();
        let request = match alteration {
            "site" => prepared.request.replacen(
                "site = \"synthetic-plant/v1\"",
                "site = \"unrelated-plant/v1\"",
                1,
            ),
            "element" => prepared.request.replacen(
                "quantity = \"model_recovered_metal:Au\"",
                "quantity = \"model_recovered_metal:Ag\"",
                1,
            ),
            _ => prepared.request,
        };
        forged_history(&root, start, &request);
        let before = std::fs::read(root.join("history")).unwrap();
        let checked = execute_with_clock(&verification(&root), &root, &NoClock);
        assert_eq!(
            checked.refusal.unwrap().code,
            "FMINE-REF-207",
            "{alteration}"
        );
        assert_eq!(std::fs::read(root.join("history")).unwrap(), before);
    }
}

#[test]
fn offline_verification_checks_source_bytes_and_recomputes_even_after_sha_is_updated() {
    let start = time("2030-01-02T00:00:00Z").unwrap();
    for update_hash in [false, true] {
        let root = fixture(start);
        let loaded = crate::plant::production::load(
            Path::new("production.toml"),
            Path::new("grinding.toml"),
            Path::new("recovery.toml"),
            &root,
        )
        .unwrap();
        let original = std::fs::read_to_string(root.join("production.toml")).unwrap();
        let changed = original.replace("scheduled_hours = \"1\"", "scheduled_hours = \"1/2\"");
        assert_ne!(changed, original);
        let source = loaded.to_json().replace(
            &crate::escape_json("scheduled_hours = \"1\""),
            &crate::escape_json("scheduled_hours = \"1/2\""),
        );
        let parsed = parse_json(source.as_bytes()).unwrap();
        assert_eq!(
            parsed
                .get("production")
                .unwrap()
                .get("submitted_worksheet")
                .unwrap()
                .as_str(),
            Some(changed.as_str())
        );
        let source = if update_hash {
            source.replace(
                &fmine_core::hash::sha256(original.as_bytes()).to_hex(),
                &fmine_core::hash::sha256(changed.as_bytes()).to_hex(),
            )
        } else {
            source
        };
        let prepared = from_receipt(
            &registration(Target::RecoveredMetal),
            loaded.receipt(),
            &source,
        )
        .unwrap();
        forged_history(&root, start, &prepared.request);
        let before = std::fs::read(root.join("history")).unwrap();
        let checked = execute_with_clock(&verification(&root), &root, &NoClock);
        assert_eq!(
            checked.refusal.unwrap().code,
            if update_hash {
                "FMINE-REF-207"
            } else {
                "FMINE-REF-505"
            }
        );
        assert_eq!(std::fs::read(root.join("history")).unwrap(), before);
    }
}

#[test]
fn plant_registration_reopens_and_scores_with_full_admission_evidence_retained() {
    let start = time("2030-01-02T00:00:00Z").unwrap();
    for (soft, hard, interval_score, crps) in [
        ("1/2", "1/2", 0.216, 0.036),
        ("1/20", "19/20", 0.12, 0.01224),
    ] {
        let root = fixture(start);
        weights(&root, soft, hard);
        init(&root, start - 1_000);
        let prepared = prepare(&registration(Target::RecoveredMetal), &root).unwrap();
        let admitted = execute_with_clock(
            &command(&root, Target::RecoveredMetal),
            &root,
            &Clock(start - 100),
        );
        assert_eq!(admitted.exit_code, 0, "{:?}", admitted.refusal);
        assert!(admitted.payload_json.contains(&prepared.evidence_json));
        assert!(admitted.payload_json.contains("\"v14\":1"));
        assert!(
            admitted
                .payload_json
                .contains(&format!("\"registration_before_ns\":{start}"))
        );
        let saved = Journal::open_read(&root.join("history"), None).unwrap();
        let Entry::Operation(operation) = &saved.records()[1].entry else {
            panic!("forecast operation")
        };
        assert!(operation.starts_with("schema = \"fmine.forecast-operation.v14\"\n"));
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
        assert!(
            inspected
                .payload_json
                .contains("\"kind\":\"weighted_empirical\"")
        );

        score_registered(&root, start, interval_score, Some(crps));
        let reopened = Journal::open_read(&root.join("history"), None).unwrap();
        let Entry::Operation(original) = &reopened.records()[1].entry else {
            panic!("operation")
        };
        assert_eq!(
            original,
            &format!(
                "schema = \"fmine.forecast-operation.v14\"\n{}",
                prepared.request
            )
        );
    }
}

fn score_registered(root: &Path, start: i64, expected_interval: f64, expected_crps: Option<f64>) {
    std::fs::write(root.join("cohort.toml"), "[[cohort]]\nname = \"synthetic-production\"\nmembers = \"production-1\"\nresolution_rule = \"Supplied elemental production for the frozen complete physical interval\"\neligible_sources = \"synthetic:mill-report\"\noutcome_lo = \"0\"\noutcome_hi = \"1\"\n").unwrap();
    let frozen = execute_with_clock(
        &Command::Freeze {
            journal: "history".into(),
            head: head(root),
            request: "cohort.toml".into(),
        },
        root,
        &Clock(start - 50),
    );
    assert_eq!(frozen.exit_code, 0, "{:?}", frozen.refusal);
    let frozen_head = head(root);
    std::fs::write(root.join("early.toml"), format!("[[outcome]]\nforecast = \"production-1\"\navailable_at = \"2030-01-02T00:30:00Z\"\nkind = \"published\"\nvalue = \"0.3\"\nsource = \"synthetic:mill-report\"\nvintage = \"invented-partial-period\"\nlocator = \"Synthetic!A1\"\nsha256 = \"{}\"\n", "a".repeat(64))).unwrap();
    let early = execute_with_clock(
        &Command::Outcome {
            journal: "history".into(),
            head: frozen_head,
            request: "early.toml".into(),
        },
        root,
        &Clock(start + HOUR / 2 + NS_PER_SECOND),
    );
    assert_eq!(early.refusal.unwrap().code, "FMINE-REF-206");
    assert_eq!(head(root), frozen_head);
    std::fs::write(root.join("outcome.toml"), format!("[[outcome]]\nforecast = \"production-1\"\navailable_at = \"2030-01-02T01:00:01Z\"\nkind = \"published\"\nvalue = \"0.3\"\nsource = \"synthetic:mill-report\"\nvintage = \"invented-period\"\nlocator = \"Synthetic!A1\"\nsha256 = \"{}\"\n", "a".repeat(64))).unwrap();
    let observed = execute_with_clock(
        &Command::Outcome {
            journal: "history".into(),
            head: head(root),
            request: "outcome.toml".into(),
        },
        root,
        &Clock(start + HOUR + 2 * NS_PER_SECOND),
    );
    assert_eq!(observed.exit_code, 0, "{:?}", observed.refusal);
    let assessment = execute_with_clock(
        &Command::Assess {
            journal: "history".into(),
            head: head(root),
            cohort: "synthetic-production".into(),
            as_of: "2030-01-02T01:00:03Z".into(),
            selector: "first-published".into(),
            expected_digest: None,
        },
        root,
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
    near(mean, expected_interval);
    if let Some(expected) = expected_crps {
        let marker = "\"crps\":{\"n_scored\":1,\"observed_mean\":";
        let mean = assessment
            .payload_json
            .split_once(marker)
            .expect("full-distribution CRPS")
            .1
            .split(',')
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap();
        near(mean, expected);
        assert!(
            assessment
                .payload_json
                .contains("\"schema\":\"fmine.forecast-diagnostics.v3\"")
        );
        assert!(
            assessment
                .payload_json
                .contains("\"kind\":\"weighted_empirical_rank\"")
        );
        assert!(
            assessment
                .payload_json
                .contains("\"below_mass\":{\"num\":\"1\",\"den\":\"1\"}")
        );
        assert!(
            assessment
                .payload_json
                .contains("\"equal_mass\":{\"num\":\"0\",\"den\":\"1\"}")
        );
    } else {
        assert!(assessment.payload_json.contains("\"crps\":null"));
        assert!(
            !assessment
                .payload_json
                .contains("\"weighted_empirical_rank\"")
        );
    }
    assert!(
        assessment
            .payload_json
            .contains("\"calibration_flag\":\"descriptive\"")
    );
    assert!(
        assessment
            .payload_json
            .contains("\"source_citations\":\"supplied_not_verified\"")
    );
}

#[test]
fn legacy_v9_plant_records_replay_original_quantiles_without_acquiring_crps() {
    let start = time("2030-01-02T00:00:00Z").unwrap();
    let root = fixture(start);
    init(&root, start - 1_000);
    let loaded = crate::plant::production::load(
        Path::new("production.toml"),
        Path::new("grinding.toml"),
        Path::new("recovery.toml"),
        &root,
    )
    .unwrap();
    let prepared = legacy_from_receipt(
        &registration(Target::RecoveredMetal),
        loaded.receipt(),
        &loaded.to_json(),
    )
    .expect("preserved original v9 source-bound request");
    let error = legacy_from_receipt(
        &registration(Target::PeriodThroughput),
        loaded.receipt(),
        &loaded.to_json(),
    )
    .err()
    .expect("period throughput uses the complete weighted distribution only");
    assert!(error.what.contains("requires a weighted plant forecast"));
    assert!(
        prepared
            .request
            .starts_with("# fmine.plant-forecast-admission.v1 ")
    );
    assert!(
        prepared
            .request
            .contains("kind = \"quantiles\"\nlevels = \"0.1,0.5,0.9\"")
    );
    assert!(
        prepared
            .evidence_json
            .contains("Only p10, p50 and p90 are registered")
    );
    assert!(prepared.evidence_json.contains(&loaded.to_json()));
    let operation = format!(
        "schema = \"fmine.forecast-operation.v9\"\n{}",
        prepared.request
    );
    let doc = parse_subset(&operation).unwrap();
    assert_eq!(
        registration_start(&Row(&doc.rows[0]), &operation).unwrap(),
        start
    );
    let mut journal = Journal::open_write(&root.join("history"), head(&root)).unwrap();
    // The byte store retains the original operation; the fresh semantic replay below validates it.
    journal
        .append(&operation, &Clock(start - 100), |_, _| Ok(()))
        .unwrap();
    let saved_head = journal.head();
    drop(journal);
    let before = std::fs::read(root.join("history")).unwrap();
    std::fs::write(
        root.join("grinding.toml"),
        "changed after legacy registration",
    )
    .unwrap();
    let inspected = execute_with_clock(
        &Command::Inspect {
            journal: "history".into(),
            head: Some(saved_head),
        },
        &root,
        &NoClock,
    );
    assert_eq!(inspected.exit_code, 0, "{:?}", inspected.refusal);
    assert!(inspected.payload_json.contains("\"v9\":1"));
    assert!(inspected.payload_json.contains("\"kind\":\"quantiles\""));
    assert!(
        !inspected
            .payload_json
            .contains("\"kind\":\"weighted_empirical\"")
    );
    assert_eq!(std::fs::read(root.join("history")).unwrap(), before);
    let verified = execute_with_clock(&verification(&root), &root, &NoClock);
    assert_eq!(verified.exit_code, 0, "{:?}", verified.refusal);
    assert!(
        verified
            .payload_json
            .contains("fmine.plant-forecast-admission.v1")
    );
    assert_eq!(std::fs::read(root.join("history")).unwrap(), before);
    score_registered(&root, start, 0.216, None);
    let reopened = Journal::open_read(&root.join("history"), None).unwrap();
    let Entry::Operation(original) = &reopened.records()[1].entry else {
        panic!("legacy operation")
    };
    assert_eq!(original, &operation);
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
