//! SYNTHETIC source-to-monitor controls; declared order is not authenticated chronology.
//! Determinism: DeterministicPerIsa.
#![allow(clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const STREAM: &str = include_str!("../../../tests/drill/crm_monitor.toml");
const INPUT: &str = "crm_monitor.toml";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn fixture(text: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-monitor-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("directory");
    std::fs::write(root.join(INPUT), text).expect("source");
    std::fs::canonicalize(root).expect("root")
}
fn args() -> Vec<String> {
    ["drill", "monitor", "--input", INPUT]
        .map(str::to_owned)
        .into()
}
fn execute(root: &Path) -> Envelope {
    let (envelope, format) = invoke(&args(), root);
    assert_eq!(format, OutputFormat::Json);
    envelope
}
fn successful(root: &Path) -> Json {
    let envelope = execute(root);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    parse(envelope.to_json().as_bytes()).expect("valid JSON")
}
fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |v, k| v.get(k).expect(k))
}
fn report(value: &Json) -> &Json {
    field(value, &["payload", "report"])
}
fn alarms(report: &Json) -> &[Json] {
    field(report, &["alarms"]).as_array().expect("alarms")
}

#[test]
fn native_monitor_links_pattern_drift_and_integrity_alarms_to_ordered_sources() {
    let root = fixture(STREAM);
    let envelope = execute(&root);
    let native = || {
        Command::new(env!("CARGO_BIN_EXE_fmine"))
            .current_dir(&root)
            .args(args())
            .output()
            .expect("native command")
    };
    let first = native();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&first.stdout).trim_end(),
        envelope.to_json()
    );
    assert_eq!(first.stdout, native().stdout);
    let value = successful(&root);
    let r = report(&value);
    assert_eq!(field(r, &["n"]).u64(), Some(18));
    assert_eq!(
        field(r, &["reference", "sd_basis"]).as_str(),
        Some("sigma_c")
    );
    assert_eq!(
        field(r, &["source", "sha256"]).as_str(),
        Some(sha256(STREAM.as_bytes()).to_string().as_str())
    );
    for (monitor, step, scope) in [
        ("two_consecutive_beyond_2sd", 2, "calibration_or_bias"),
        ("four_consecutive_beyond_1sd", 4, "calibration_or_bias"),
        ("sign_imbalance", 6, "calibration_or_bias"),
        ("dispersion_collapse", 18, "integrity"),
    ] {
        let alarm = alarms(r)
            .iter()
            .find(|a| field(a, &["monitor"]).as_str() == Some(monitor))
            .expect("expected alarm");
        assert_eq!(field(alarm, &["step"]).u64(), Some(step));
        assert_eq!(field(alarm, &["index"]).u64(), Some(step - 1));
        assert_eq!(field(alarm, &["scope"]).as_str(), Some(scope));
        assert_eq!(
            field(alarm, &["sample_id"]).as_str(),
            Some(format!("CRM-{step:02}").as_str())
        );
    }
    let alarm_step = field(r, &["eproc_alarm_step"]).u64().expect("drift alarm");
    assert_eq!(
        alarm_step, 7,
        "first crossing of log(20) in the declared shifted stream"
    );
    let prefixes = field(r, &["prefixes"]).as_array().expect("prefixes");
    assert_eq!(prefixes.len(), 18);
    let source_lines: Vec<usize> = STREAM
        .lines()
        .enumerate()
        .filter_map(|(i, line)| (line == "[[observation]]").then_some(i + 1))
        .collect();
    for (i, prefix) in prefixes.iter().enumerate() {
        assert_eq!(field(prefix, &["index"]).u64(), Some(i as u64));
        assert_eq!(
            field(prefix, &["source_line"]).u64(),
            Some(source_lines[i] as u64)
        );
    }
    assert_eq!(
        field(&prefixes[0], &["eproc_log_wealth"]).f64(),
        Some(0.0),
        "first bet reads only empty history"
    );
    assert_eq!(field(r, &["latest_dispersion_ratio"]).f64(), Some(0.0));
    for (_, flag) in field(&value, &["flags"]).as_object().expect("flags") {
        assert_eq!(flag.as_bool(), Some(false));
    }
    assert_eq!(
        field(&value, &["payload", "claim", "evidence"]).as_str(),
        Some("Assumed")
    );
    assert_eq!(
        field(&value, &["payload", "claim", "calibration"]).as_str(),
        Some("uncalibrated")
    );
    assert_eq!(
        field(&value, &["payload", "claim", "confidentiality"]).as_str(),
        Some("Internal")
    );
    assert_eq!(
        field(r, &["policy", "preregistration"]).as_str(),
        Some("not_verified")
    );
    assert_eq!(
        std::fs::read(root.join(INPUT)).expect("unchanged source"),
        STREAM.as_bytes()
    );
    assert_eq!(std::fs::read_dir(&root).expect("read-only").count(), 1);
}

#[test]
fn prefix_replay_is_stable_but_source_and_order_change_identity() {
    let root = fixture(STREAM);
    let full = successful(&root);
    let cut = STREAM
        .find("[[observation]]\nsample_id = \"CRM-13\"")
        .expect("prefix boundary");
    std::fs::write(root.join(INPUT), &STREAM[..cut]).expect("prefix");
    let prefix = successful(&root);
    let full_steps = field(report(&full), &["prefixes"])
        .as_array()
        .expect("full prefixes");
    assert_eq!(
        field(report(&prefix), &["prefixes"])
            .as_array()
            .expect("prefixes"),
        &full_steps[..12]
    );
    let commented = format!("{STREAM}\n# retained source declaration\n");
    std::fs::write(root.join(INPUT), commented).expect("source edit");
    let changed = successful(&root);
    assert_ne!(
        field(&full, &["payload", "claim", "id"]),
        field(&changed, &["payload", "claim", "id"])
    );
    assert_eq!(
        field(report(&full), &["prefixes"]),
        field(report(&changed), &["prefixes"])
    );
    let rows: Vec<&str> = STREAM.split("[[observation]]").collect();
    let reordered = format!(
        "{}{}",
        rows[0],
        rows[1..]
            .iter()
            .rev()
            .map(|r| format!("[[observation]]{r}"))
            .collect::<String>()
    );
    std::fs::write(root.join(INPUT), reordered).expect("reordered source");
    let reversed = successful(&root);
    assert_eq!(
        field(report(&reversed), &["dispersion_collapse_step"]).u64(),
        Some(6)
    );
    assert_ne!(
        field(report(&full), &["prefixes"]),
        field(report(&reversed), &["prefixes"])
    );
    let mut a = args();
    a.extend(["--format".into(), "markdown".into()]);
    let (envelope, format) = invoke(&a, &root);
    assert_eq!(format, OutputFormat::Markdown);
    let markdown = envelope.rendered_markdown.expect("human report");
    for text in [
        "CRM sequential monitor",
        "First dispersion-collapse alarm",
        "integrity",
        "Only the e-process",
        "no combined or anytime false-alarm guarantee",
        "SHA-256:",
        "Preregistration: **not verified**",
    ] {
        assert!(markdown.contains(text), "missing {text}");
    }
}

#[test]
fn unusable_or_ambiguous_streams_refuse_the_entire_replay() {
    let root = fixture(STREAM);
    for source in [
        STREAM.replace("basis = \"dry\"\n", ""),
        STREAM.replace("unit = \"mass_fraction\"", "unit = \"ppm\""),
        STREAM.replace("sd_basis = \"sigma_c\"", "sd_basis = \"running_sw\""),
        STREAM.replace("policy_version = \"1\"\n", ""),
        STREAM.replace("method = \"fusion_xrf\"", "method = \"unknown\""),
        STREAM.replace(
            "evidence_lane = \"synthetic\"",
            "evidence_lane = \"public\"",
        ),
        STREAM.replace("reference_sd = \"0.0625\"", "reference_sd = \"0\""),
        STREAM.replace("reference_sd = \"0.0625\"", "reference_sd = \"1e-400\""),
        STREAM.replace("reference_sd = \"0.0625\"", "reference_sd = \"5e-324\""),
        STREAM.replace("value = \"0.640625\"", "value = \"<0.640625\""),
        STREAM.replace("value = \"0.640625\"", "value = \"1.1\""),
        STREAM.replace("value = \"0.640625\"", "value = \"NaN\""),
        STREAM.replace("sample_id = \"CRM-02\"", "sample_id = \"CRM-01\""),
        STREAM.replace("dispersion_window = \"6\"", "dispersion_window = \"4097\""),
        STREAM.replace("eproc_alpha = \"0.05\"", "eproc_alpha = \"1\""),
        STREAM.replace("[[observation]]", "[[wrong]]"),
    ] {
        std::fs::write(root.join(INPUT), source).expect("invalid input");
        let result = execute(&root);
        assert_eq!(result.status, "refused", "{}", result.to_json());
        assert!(matches!(
            result.refusal.expect("typed refusal").code,
            "FMINE-REF-208" | "FMINE-REF-109"
        ));
        assert_eq!(result.payload_json, "null", "no partial prefix escapes");
    }
    std::fs::write(root.join(INPUT), vec![b'x'; 1024 * 1024 + 1]).expect("oversize");
    assert_eq!(
        execute(&root).refusal.expect("size refusal").code,
        "FMINE-REF-506"
    );
}
