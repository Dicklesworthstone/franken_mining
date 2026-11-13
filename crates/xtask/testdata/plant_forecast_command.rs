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
fn binary_registers_source_bound_plant_quantiles_and_reopens_both_time_boundaries() {
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
        Some("quantiles")
    );
    for (key, expected) in [
        ("levels", [0.1, 0.5, 0.9]),
        ("values", [0.192, 0.192, 0.288]),
    ] {
        let values = field(forecast, &["payload", key])
            .as_array()
            .expect("quantile coordinates");
        assert_eq!(values.len(), 3);
        for (actual, expected) in values.iter().zip(expected) {
            assert!((actual.f64().expect("number") - expected).abs() < 1e-12);
        }
    }
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
        field(&inspected, &["payload", "operation_version_counts", "v9"]).u64(),
        Some(1)
    );
    assert_eq!(
        std::fs::read(root.join("journal")).expect("read-only bytes"),
        committed
    );
    let journal = Journal::open_read(&root.join("journal"), None).expect("reopened records");
    assert_eq!(journal.records().len(), 2);
    let Entry::Operation(operation) = &journal.records()[1].entry else {
        panic!("plant operation")
    };
    assert!(operation.starts_with("schema = \"fmine.forecast-operation.v9\"\n"));
    let retained = operation
        .lines()
        .find_map(|line| line.strip_prefix("# fmine.plant-forecast-admission.v1 "))
        .expect("retained admission");
    assert_eq!(
        &parse(retained.as_bytes()).expect("retained JSON"),
        admission
    );
}
