//! SYNTHETIC future-period registration; the declared host clock is not external timestamp proof.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fmine_calib::record::format_utc;
use fmine_claim::forecast_journal::Entry;
use fmine_core::hash::sha256;
use fmine_io::forecast::Journal;
use fmine_io::omf::json::{Json, parse};

const NS: i64 = 1_000_000_000;
const HOUR: i64 = 3_600 * NS;

fn fixture() -> (PathBuf, i64, Vec<(&'static str, String)>) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("host UTC");
    let start = (i64::try_from(now.as_secs()).expect("signed UTC") + 7 * 86_400) * NS;
    let root = std::env::temp_dir().join(format!(
        "fmine-plant-forecast-binary-{}-{}",
        std::process::id(),
        now.as_nanos(),
    ));
    std::fs::create_dir(&root).expect("synthetic workspace");
    let sources = [
        (
            "production",
            include_str!("../../../tests/plant/production_joint.toml"),
        ),
        (
            "grinding",
            include_str!("../../../tests/plant/grinding_joint.toml"),
        ),
        (
            "recovery",
            include_str!("../../../tests/plant/production_recovery.toml"),
        ),
    ]
    .into_iter()
    .map(|(name, text)| {
        let text = text
            .replace("start_ns = \"0\"", &format!("start_ns = \"{start}\""))
            .replace(
                "end_ns = \"3600000000000\"",
                &format!("end_ns = \"{}\"", start + HOUR),
            );
        std::fs::write(root.join(format!("{name}.toml")), &text).expect("synthetic source");
        (name, text)
    })
    .collect();
    (root, start, sources)
}

fn run(root: &Path, args: &[&str], expected: i32) -> Json {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .current_dir(root)
        .args(["forecast", "journal"])
        .args(args)
        .output()
        .expect("actual fmine binary");
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{args:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope = parse(&output.stdout).expect("JSON envelope");
    assert_eq!(
        field(&envelope, &["exit_code"]).i64(),
        Some(i64::from(expected))
    );
    envelope
}

fn field<'a>(value: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter()
        .fold(value, |value, key| value.get(key).expect(key))
}

fn head(root: &Path) -> String {
    Journal::open_read(&root.join("journal"), None)
        .expect("stored journal")
        .head()
        .to_hex()
}

#[test]
fn binary_registers_complete_weighted_plant_population_and_reopens_both_time_boundaries() {
    let (root, start, sources) = fixture();
    run(
        &root,
        &[
            "init",
            "--journal",
            "journal",
            "--cell",
            "synthetic-plant",
            "--max-clock-skew-ms",
            "1",
            "--trust-host-clock",
        ],
        0,
    );
    let initial_head = head(&root);
    let before = std::fs::read(root.join("journal")).expect("initial journal bytes");
    let args = vec![
        "register-plant",
        "--journal",
        "journal",
        "--head",
        &initial_head,
        "--input",
        "production.toml",
        "--grinding",
        "grinding.toml",
        "--recovery",
        "recovery.toml",
        "--id",
        "joint-1",
        "--target",
        "recovered-metal",
        "--author-role",
        "synthetic-model-role",
        "--time-basis",
        "utc-unix-ns",
        "--trust-host-clock",
    ];
    for missing in ["--grinding", "--time-basis", "--trust-host-clock"] {
        let mut invalid = args.clone();
        let index = invalid
            .iter()
            .position(|arg| *arg == missing)
            .expect("flag");
        let count = if missing == "--trust-host-clock" {
            1
        } else {
            2
        };
        drop(invalid.drain(index..index + count));
        run(&root, &invalid, 2);
        assert_eq!(head(&root), initial_head);
        assert_eq!(
            std::fs::read(root.join("journal")).expect("unchanged bytes"),
            before
        );
    }
    for (old, wrong, exit) in [
        ("recovered-metal", "wet-tonnes", 2),
        ("utc-unix-ns", "relative-time", 3),
    ] {
        let invalid = args
            .iter()
            .map(|arg| if *arg == old { wrong } else { *arg })
            .collect::<Vec<_>>();
        run(&root, &invalid, exit);
        assert_eq!(head(&root), initial_head);
        assert_eq!(
            std::fs::read(root.join("journal")).expect("unchanged bytes"),
            before
        );
    }

    let registered = run(&root, &args, 0);
    let payload = field(&registered, &["payload"]);
    let admission = field(payload, &["plant_admission"]);
    assert_eq!(
        field(admission, &["schema"]).as_str(),
        Some("fmine.plant-forecast-admission.v2")
    );
    assert_eq!(
        field(admission, &["target"]).as_str(),
        Some("recovered-metal")
    );
    assert_eq!(field(admission, &["physical_start_ns"]).i64(), Some(start));
    assert_eq!(
        field(admission, &["physical_end_ns"]).i64(),
        Some(start + HOUR)
    );
    let receipt = field(admission, &["source_receipt", "production"]);
    let digest = field(receipt, &["receipt_digest"])
        .as_str()
        .expect("receipt digest");
    assert_eq!(field(admission, &["receipt_digest"]).as_str(), Some(digest));
    assert_eq!(digest.len(), 64);
    for (name, text) in &sources {
        let source = if *name == "production" {
            receipt
        } else {
            field(admission, &["source_receipt", "sources", name])
        };
        assert_eq!(
            field(source, &["submitted_worksheet"]).as_str(),
            Some(text.as_str())
        );
        assert_eq!(
            field(source, &["source", "sha256"]).as_str(),
            Some(sha256(text.as_bytes()).to_hex().as_str())
        );
    }
    let forecasts = field(payload, &["forecasts"])
        .as_array()
        .expect("forecasts");
    assert_eq!(forecasts.len(), 1);
    let forecast = &forecasts[0];
    assert_eq!(field(forecast, &["alias"]).as_str(), Some("joint-1"));
    assert_eq!(
        field(forecast, &["model_key"]).as_str(),
        Some(format!("plant-production:recovered-metal:{digest}").as_str())
    );
    assert_eq!(
        field(forecast, &["registration_before_ns"]).i64(),
        Some(start)
    );
    assert_eq!(
        field(forecast, &["outcome_due_ns"]).i64(),
        Some(start + HOUR)
    );
    assert_eq!(
        field(forecast, &["registration_before"]).as_str(),
        Some(format_utc(start / NS).as_str())
    );
    assert_eq!(
        field(forecast, &["outcome_due"]).as_str(),
        Some(format_utc((start + HOUR) / NS).as_str())
    );
    assert!(
        field(forecast, &["registered_ns"])
            .i64()
            .expect("registered time")
            < start
    );
    assert_eq!(field(forecast, &["metadata", "unit"]).as_str(), Some("kg"));
    assert_eq!(
        field(forecast, &["metadata", "quantity"]).as_str(),
        Some("model_recovered_metal:Au")
    );
    assert_eq!(
        field(forecast, &["payload", "kind"]).as_str(),
        Some("weighted_empirical")
    );
    let samples = field(forecast, &["payload", "samples"])
        .as_array()
        .expect("complete scenario values");
    assert_eq!(samples.len(), 2);
    for (actual, expected) in samples.iter().zip([0.288, 0.192]) {
        assert!((actual.f64().expect("number") - expected).abs() < 1e-12);
    }
    assert_eq!(
        field(forecast, &["payload", "weights"]),
        &parse(br#"["1/2","1/2"]"#).unwrap()
    );
    assert_eq!(
        field(forecast, &["payload", "weight_kind"]).as_str(),
        Some("exact_probability")
    );
    assert_eq!(
        field(forecast, &["metadata", "family"]).as_str(),
        Some("conditional-plant-production-weighted/2")
    );
    assert_eq!(field(payload, &["n_cohorts"]).u64(), Some(0));
    assert_eq!(field(payload, &["n_outcomes"]).u64(), Some(0));

    let registered_head = head(&root);
    let committed = std::fs::read(root.join("journal")).expect("committed journal bytes");
    let inspected = run(
        &root,
        &[
            "inspect",
            "--journal",
            "journal",
            "--head",
            &registered_head,
        ],
        0,
    );
    assert_eq!(
        field(&inspected, &["payload", "head"]).as_str(),
        Some(registered_head.as_str())
    );
    assert_eq!(
        field(&inspected, &["payload", "forecasts"]),
        field(payload, &["forecasts"])
    );
    assert_eq!(
        field(&inspected, &["payload", "operation_version_counts", "v14"]).u64(),
        Some(1)
    );
    assert_eq!(
        std::fs::read(root.join("journal")).expect("read-only bytes"),
        committed
    );
    for name in ["production", "grinding", "recovery"] {
        std::fs::rename(
            root.join(format!("{name}.toml")),
            root.join(format!("{name}.saved")),
        )
        .unwrap();
    }
    let verification_args = [
        "verify-plant",
        "--journal",
        "journal",
        "--forecast",
        "joint-1",
        "--head",
        &registered_head,
    ];
    let verified = run(&root, &verification_args, 0);
    let verification = field(&verified, &["payload", "verification"]);
    assert_eq!(field(verification, &["status"]).as_str(), Some("verified"));
    assert_eq!(
        field(verification, &["source_files_opened"]).as_bool(),
        Some(false)
    );
    assert_eq!(field(verification, &["admission"]), admission);
    let mut forbidden_clock = verification_args.to_vec();
    forbidden_clock.push("--trust-host-clock");
    run(&root, &forbidden_clock, 2);
    let markdown = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .current_dir(&root)
        .args(["forecast", "journal"])
        .args(verification_args)
        .args(["--format", "markdown"])
        .output()
        .unwrap();
    assert!(
        markdown.status.success(),
        "{}",
        String::from_utf8_lossy(&markdown.stderr)
    );
    assert!(
        String::from_utf8(markdown.stdout)
            .unwrap()
            .contains("Plant forecast numerical verification")
    );
    assert_eq!(std::fs::read(root.join("journal")).unwrap(), committed);
    let journal = Journal::open_read(&root.join("journal"), None).expect("reopened records");
    assert_eq!(journal.records().len(), 2);
    let Entry::Operation(operation) = &journal.records()[1].entry else {
        panic!("plant operation")
    };
    assert!(operation.starts_with("schema = \"fmine.forecast-operation.v14\"\n"));
    let retained = operation
        .lines()
        .find_map(|line| line.strip_prefix("# fmine.plant-forecast-admission.v2 "))
        .expect("retained admission");
    assert_eq!(
        &parse(retained.as_bytes()).expect("retained JSON"),
        admission
    );
}

#[test]
fn supply_limited_forecasts_replay_retained_sources_and_keep_operating_rate_semantics() {
    let (root, start, _) = fixture();
    let supply = include_str!("../../../tests/plant/production_supply_limited.toml")
        .replace("start_ns = \"0\"", &format!("start_ns = \"{start}\""))
        .replace(
            "end_ns = \"3600000000000\"",
            &format!("end_ns = \"{}\"", start + HOUR),
        );
    std::fs::write(root.join("production.toml"), &supply).expect("supply-limited source");
    run(
        &root,
        &[
            "init",
            "--journal",
            "journal",
            "--cell",
            "synthetic-plant",
            "--max-clock-skew-ms",
            "1",
            "--trust-host-clock",
        ],
        0,
    );
    let targets = [
        ("limited-metal", "recovered-metal", [0.288, 0.06]),
        ("limited-tonnes", "dry-tonnes", [80.0, 100.0]),
        ("operating-rate", "throughput", [80.0, 320.0]),
    ];
    for (id, target, expected) in targets {
        let previous = head(&root);
        let registered = run(
            &root,
            &[
                "register-plant",
                "--journal",
                "journal",
                "--head",
                &previous,
                "--input",
                "production.toml",
                "--grinding",
                "grinding.toml",
                "--recovery",
                "recovery.toml",
                "--id",
                id,
                "--target",
                target,
                "--author-role",
                "synthetic-model-role",
                "--time-basis",
                "utc-unix-ns",
                "--trust-host-clock",
            ],
            0,
        );
        let receipt = field(
            &registered,
            &["payload", "plant_admission", "source_receipt", "production"],
        );
        assert_eq!(
            field(receipt, &["schema"]).as_str(),
            Some("fmine.plant-production-result.v2")
        );
        assert_eq!(
            field(receipt, &["submitted_worksheet"]).as_str(),
            Some(supply.as_str())
        );
        let forecast = field(&registered, &["payload", "forecasts"])
            .as_array()
            .expect("forecasts")
            .iter()
            .find(|row| field(row, &["alias"]).as_str() == Some(id))
            .expect("registered target");
        let samples = field(forecast, &["payload", "samples"])
            .as_array()
            .expect("all weighted states");
        assert_eq!(samples.len(), 2);
        for (sample, expected) in samples.iter().zip(expected) {
            assert!((sample.f64().expect("number") - expected).abs() < 1e-12);
        }
        assert_eq!(
            field(forecast, &["payload", "weights"]),
            &parse(br#"["1/2","1/2"]"#).unwrap()
        );
    }
    let retained_head = head(&root);
    let before = std::fs::read(root.join("journal")).expect("retained journal");
    for name in ["production", "grinding", "recovery"] {
        std::fs::rename(
            root.join(format!("{name}.toml")),
            root.join(format!("{name}.saved")),
        )
        .unwrap();
    }
    for (id, _, _) in targets {
        let verified = run(
            &root,
            &[
                "verify-plant",
                "--journal",
                "journal",
                "--forecast",
                id,
                "--head",
                &retained_head,
            ],
            0,
        );
        let verification = field(&verified, &["payload", "verification"]);
        assert_eq!(field(verification, &["status"]).as_str(), Some("verified"));
        assert_eq!(
            field(verification, &["source_files_opened"]).as_bool(),
            Some(false)
        );
        let receipt = field(verification, &["admission", "source_receipt", "production"]);
        assert_eq!(
            field(receipt, &["submitted_worksheet"]).as_str(),
            Some(supply.as_str())
        );
        assert_eq!(
            field(receipt, &["schema"]).as_str(),
            Some("fmine.plant-production-result.v2")
        );
    }
    assert_eq!(head(&root), retained_head);
    assert_eq!(std::fs::read(root.join("journal")).unwrap(), before);
}

#[test]
fn scenario_corrections_feed_weighted_forecasts_and_verify_without_source_files() {
    let (root, _, _) = fixture();
    let original = std::fs::read_to_string(root.join("recovery.toml")).unwrap();
    let soft = r#"
[[correction]]
scenario = "soft"
id = "soft-adjustment"
revision = "synthetic-v1"
additive_fraction = "-0.1"
"#;
    let hard = r#"
[[correction]]
scenario = "hard"
id = "hard-adjustment"
revision = "synthetic-v1"
additive_fraction = "0.2"
upper_cap_fraction = "0.95"
"#;
    let corrected = format!("{original}{soft}{hard}");
    std::fs::write(root.join("recovery.toml"), &corrected).unwrap();
    run(
        &root,
        &[
            "init",
            "--journal",
            "journal",
            "--cell",
            "synthetic-plant",
            "--max-clock-skew-ms",
            "1",
            "--trust-host-clock",
        ],
        0,
    );
    let targets = [
        ("corrected-recovery", "recovery", [0.95, 0.5]),
        ("corrected-metal", "recovered-metal", [0.304, 0.160]),
    ];
    for (id, target, expected) in targets {
        let previous = head(&root);
        let registered = run(
            &root,
            &[
                "register-plant",
                "--journal",
                "journal",
                "--head",
                &previous,
                "--input",
                "production.toml",
                "--grinding",
                "grinding.toml",
                "--recovery",
                "recovery.toml",
                "--id",
                id,
                "--target",
                target,
                "--author-role",
                "synthetic-model-role",
                "--time-basis",
                "utc-unix-ns",
                "--trust-host-clock",
            ],
            0,
        );
        let forecast = field(&registered, &["payload", "forecasts"])
            .as_array()
            .unwrap()
            .iter()
            .find(|row| field(row, &["alias"]).as_str() == Some(id))
            .unwrap();
        let samples = field(forecast, &["payload", "samples"]).as_array().unwrap();
        assert_eq!(samples.len(), 2);
        // Independent answers: hard min(.9+.2,.95), soft .6-.1.
        // Both states contain .32 kg; corrected recovered metal is .304 and .160.
        for (sample, expected) in samples.iter().zip(expected) {
            assert!((sample.f64().unwrap() - expected).abs() < 1e-12);
        }
        assert_eq!(
            field(forecast, &["payload", "weights"]),
            &parse(br#"["1/2","1/2"]"#).unwrap(),
        );
        assert_eq!(
            field(
                &registered,
                &[
                    "payload",
                    "plant_admission",
                    "source_receipt",
                    "sources",
                    "recovery",
                    "submitted_worksheet"
                ],
            )
            .as_str(),
            Some(corrected.as_str()),
        );
    }
    let saved_head = head(&root);
    let saved_bytes = std::fs::read(root.join("journal")).unwrap();
    std::fs::write(root.join("recovery.toml"), format!("{original}{soft}")).unwrap();
    run(
        &root,
        &[
            "register-plant",
            "--journal",
            "journal",
            "--head",
            &saved_head,
            "--input",
            "production.toml",
            "--grinding",
            "grinding.toml",
            "--recovery",
            "recovery.toml",
            "--id",
            "incomplete",
            "--target",
            "recovery",
            "--author-role",
            "synthetic-model-role",
            "--time-basis",
            "utc-unix-ns",
            "--trust-host-clock",
        ],
        3,
    );
    assert_eq!(head(&root), saved_head);
    assert_eq!(std::fs::read(root.join("journal")).unwrap(), saved_bytes);
    for name in ["production", "grinding", "recovery"] {
        std::fs::rename(
            root.join(format!("{name}.toml")),
            root.join(format!("{name}.saved")),
        )
        .unwrap();
    }
    for (id, _, _) in targets {
        let verified = run(
            &root,
            &[
                "verify-plant",
                "--journal",
                "journal",
                "--forecast",
                id,
                "--head",
                &saved_head,
            ],
            0,
        );
        let verification = field(&verified, &["payload", "verification"]);
        assert_eq!(field(verification, &["status"]).as_str(), Some("verified"));
        assert_eq!(
            field(verification, &["source_files_opened"]).as_bool(),
            Some(false)
        );
        assert_eq!(
            field(
                verification,
                &[
                    "admission",
                    "source_receipt",
                    "sources",
                    "recovery",
                    "submitted_worksheet"
                ],
            )
            .as_str(),
            Some(corrected.as_str()),
        );
    }
    assert_eq!(head(&root), saved_head);
    assert_eq!(std::fs::read(root.join("journal")).unwrap(), saved_bytes);
}

#[test]
fn fitted_recovery_updates_joint_forecasts_and_replays_retained_pairs_without_source_files() {
    let (root, start, _) = fixture();
    // The forecast is seven days ahead; these paired outcomes and their cut are in the past.
    let first = start - 8 * 24 * HOUR;
    let first_end = first + HOUR;
    let second = first + 2 * HOUR;
    let second_end = first + 3 * HOUR;
    let cut = first + 4 * HOUR;
    let original = std::fs::read_to_string(root.join("recovery.toml"))
        .unwrap()
        .replace(
            &format!("valid_start_ns = \"{start}\""),
            &format!("valid_start_ns = \"{first}\""),
        )
        .replace("additive_fraction = \"0\"", "additive_fraction = \"0.1\"");
    let fitted = format!(
        r#"{original}
[[correction_fit]]
id = "invented-paired-fit"
revision = "v1"
method = "unweighted_additive_residual_mean"
baseline = "post_operational_pre_correction"
alignment = "same_processing_support"
evidence_cut_ns = "{cut}"

[[correction_pair]]
id = "earlier-shift"
start_ns = "{first}"
end_ns = "{first_end}"
available_ns = "{first_end}"
baseline_fraction = "0.6"
observed_fraction = "0.62"
uncensored = "true"

[[correction_pair]]
id = "later-shift"
start_ns = "{second}"
end_ns = "{second_end}"
available_ns = "{second_end}"
baseline_fraction = "0.8"
observed_fraction = "0.86"
uncensored = "true"
"#,
    );
    std::fs::write(root.join("recovery.toml"), &fitted).unwrap();
    run(
        &root,
        &[
            "init",
            "--journal",
            "journal",
            "--cell",
            "synthetic-plant",
            "--max-clock-skew-ms",
            "1",
            "--trust-host-clock",
        ],
        0,
    );
    let register = |id: &str, target: &str, expected| {
        let previous = head(&root);
        run(
            &root,
            &[
                "register-plant",
                "--journal",
                "journal",
                "--head",
                &previous,
                "--input",
                "production.toml",
                "--grinding",
                "grinding.toml",
                "--recovery",
                "recovery.toml",
                "--id",
                id,
                "--target",
                target,
                "--author-role",
                "synthetic-model-role",
                "--time-basis",
                "utc-unix-ns",
                "--trust-host-clock",
            ],
            expected,
        )
    };
    let near = |actual: &Json, expected: f64| {
        assert!((actual.f64().unwrap() - expected).abs() < 1e-12);
    };
    let revised = fitted
        .replace(
            "observed_fraction = \"0.62\"",
            "observed_fraction = \"0.63\"",
        )
        .replace(
            "observed_fraction = \"0.86\"",
            "observed_fraction = \"0.85\"",
        );
    let evaluated = format!(
        r#"{}
[[correction_input]]
pair = "earlier-shift"
name = "Au"
measurand = "total-Au/dry"
unit = "g_per_dry_t"
value = "1"

[[correction_input]]
pair = "earlier-shift"
name = "P80"
measurand = "grinding-product-p80"
unit = "micrometres"
value = "100"

[[correction_input]]
pair = "later-shift"
name = "Au"
measurand = "total-Au/dry"
unit = "g_per_dry_t"
value = "3"

[[correction_input]]
pair = "later-shift"
name = "P80"
measurand = "grinding-product-p80"
unit = "micrometres"
value = "100"
"#,
        fitted
            .replace(
                "post_operational_pre_correction",
                "evaluated_post_operational_pre_correction",
            )
            .replace("baseline_fraction = \"0.6\"\n", "")
            .replace("baseline_fraction = \"0.8\"\n", ""),
    );
    let mut admissions = Vec::new();
    let mut digests = Vec::new();
    for (id, target, expected, source) in [
        ("fitted-recovery", "recovery", [0.94, 0.64], &fitted),
        ("fitted-metal", "recovered-metal", [0.3008, 0.2048], &fitted),
        (
            "revised-pairs",
            "recovered-metal",
            [0.3008, 0.2048],
            &revised,
        ),
        (
            "evaluated-history",
            "recovered-metal",
            [0.3008, 0.2048],
            &evaluated,
        ),
    ] {
        std::fs::write(root.join("recovery.toml"), source).unwrap();
        let registered = register(id, target, 0);
        let admission = field(&registered, &["payload", "plant_admission"]);
        assert_eq!(field(admission, &["physical_start_ns"]).i64(), Some(start));
        assert_eq!(
            field(admission, &["physical_end_ns"]).i64(),
            Some(start + HOUR)
        );
        let snapshot = field(admission, &["source_receipt", "sources", "recovery"]);
        assert_eq!(
            field(snapshot, &["submitted_worksheet"]).as_str(),
            Some(source.as_str())
        );
        assert_eq!(
            field(snapshot, &["source", "sha256"]).as_str(),
            Some(sha256(source.as_bytes()).to_hex().as_str())
        );
        let forecast = field(&registered, &["payload", "forecasts"])
            .as_array()
            .unwrap()
            .iter()
            .find(|row| field(row, &["alias"]).as_str() == Some(id))
            .unwrap();
        assert!(field(forecast, &["registered_ns"]).i64().unwrap() >= cut);
        let samples = field(forecast, &["payload", "samples"]).as_array().unwrap();
        assert_eq!(samples.len(), 2);
        // Mean residual (.02 + .06)/2 = .04 replaces the authored +.10 correction.
        // Paired states retain .32 kg contained: .32*.94=.3008, .32*.64=.2048.
        // Revised residuals .03/.05 have the same mean but a different descriptive sample variance;
        // neither .0008 nor .0002 is silently turned into forecast noise or scenario weights.
        // Evaluated history computes (60+10*1-100/10)/100 and (60+10*3-100/10)/100
        // through the original card, representing those binary64 baselines exactly before fitting.
        for (sample, expected) in samples.iter().zip(expected) {
            near(sample, expected);
        }
        assert_eq!(
            field(forecast, &["payload", "weights"]),
            &parse(br#"["1/2","1/2"]"#).unwrap()
        );
        let production = field(admission, &["source_receipt", "production"]);
        let states = field(production, &["scenarios"]).as_array().unwrap();
        assert_eq!(states.len(), 2);
        for (state, (id, recovery, metal)) in states
            .iter()
            .zip([("hard", 0.94, 0.3008), ("soft", 0.64, 0.2048)])
        {
            assert_eq!(field(state, &["id"]).as_str(), Some(id));
            near(field(state, &["recovery_fraction"]), recovery);
            near(field(state, &["contained_metal_kg"]), 0.32);
            near(field(state, &["recovered_metal_kg"]), metal);
        }
        near(
            field(production, &["summary", "recovered_metal_kg", "mean"]),
            0.2528,
        );
        digests.push(
            field(admission, &["receipt_digest"])
                .as_str()
                .unwrap()
                .to_string(),
        );
        admissions.push((id, admission.clone()));
    }
    assert_eq!(
        digests[0], digests[1],
        "target selection preserves the physical receipt"
    );
    assert_ne!(
        digests[1], digests[2],
        "changed paired evidence changes identity despite equal means"
    );
    assert_ne!(
        digests[1], digests[3],
        "computed historical baselines retain their own derivation evidence"
    );
    let saved_head = head(&root);
    let saved_bytes = std::fs::read(root.join("journal")).unwrap();
    for invalid in [
        fitted.replacen("uncensored = \"true\"", "uncensored = \"false\"", 1),
        fitted.replace(
            &format!("available_ns = \"{second_end}\""),
            &format!("available_ns = \"{}\"", cut + 1),
        ),
        // Admissible hypothetical worksheet, but unavailable at the actual registration clock.
        fitted.replace(
            &format!("evidence_cut_ns = \"{cut}\""),
            &format!("evidence_cut_ns = \"{}\"", start - HOUR),
        ),
        evaluated.replace(
            "unit = \"g_per_dry_t\"\nvalue = \"3\"",
            "unit = \"ppm_dry_mass\"\nvalue = \"3\"",
        ),
    ] {
        std::fs::write(root.join("recovery.toml"), invalid).unwrap();
        register("inadmissible-fit", "recovered-metal", 3);
        assert_eq!(head(&root), saved_head);
        assert_eq!(std::fs::read(root.join("journal")).unwrap(), saved_bytes);
    }
    for name in ["production", "grinding", "recovery"] {
        std::fs::rename(
            root.join(format!("{name}.toml")),
            root.join(format!("{name}.saved")),
        )
        .unwrap();
    }
    for (id, admission) in &admissions {
        let verified = run(
            &root,
            &[
                "verify-plant",
                "--journal",
                "journal",
                "--forecast",
                id,
                "--head",
                &saved_head,
            ],
            0,
        );
        let verification = field(&verified, &["payload", "verification"]);
        assert_eq!(field(verification, &["status"]).as_str(), Some("verified"));
        assert_eq!(
            field(verification, &["source_files_opened"]).as_bool(),
            Some(false)
        );
        assert_eq!(field(verification, &["admission"]), admission);
    }
    let reopened = Journal::open_read(&root.join("journal"), None).unwrap();
    assert_eq!(reopened.records().len(), 1 + admissions.len());
    for record in reopened.records().iter().skip(1) {
        let Entry::Operation(operation) = &record.entry else {
            panic!("plant operation")
        };
        let retained = operation
            .lines()
            .find_map(|line| line.strip_prefix("# fmine.plant-forecast-admission.v2 "))
            .unwrap();
        let retained = parse(retained.as_bytes()).unwrap();
        assert!(admissions.iter().any(|(_, expected)| *expected == retained));
    }
    // A retained calculation cannot claim fitting evidence before availability. Construct local
    // clock histories to test verification's saved bound without sampling the current clock.
    struct AtCut(i64);
    impl fmine_runtime::WallClock for AtCut {
        fn now_ns(&self) -> Result<i64, fmine_runtime::ClockError> {
            Ok(self.0)
        }
    }
    let Entry::Operation(operation) = &reopened.records()[1].entry else {
        panic!("retained fitted registration")
    };
    for (skew, expected) in [(0, 0), (1, 3)] {
        let file = format!("fit-clock-{skew}");
        let mut crafted =
            Journal::create(&root.join(&file), "synthetic-fit-clock", skew, &AtCut(cut)).unwrap();
        crafted
            .append(operation, &AtCut(cut), |_, _| Ok(()))
            .unwrap();
        let pinned = crafted.head().to_hex();
        drop(crafted);
        let verified = run(
            &root,
            &[
                "verify-plant",
                "--journal",
                &file,
                "--forecast",
                "fitted-recovery",
                "--head",
                &pinned,
            ],
            expected,
        );
        if expected == 3 {
            assert_eq!(
                field(&verified, &["refusal", "code"]).as_str(),
                Some("FMINE-REF-206")
            );
        } else {
            assert_eq!(
                field(&verified, &["payload", "verification", "status"]).as_str(),
                Some("verified")
            );
        }
    }
    assert_eq!(head(&root), saved_head);
    assert_eq!(std::fs::read(root.join("journal")).unwrap(), saved_bytes);
}
