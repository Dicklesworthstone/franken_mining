use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_core::hash::sha256;

const INPUT: &str = include_str!("../../../tests/forecasts/spatial_synthetic.toml");
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Workspace(PathBuf);

impl Workspace {
    fn new(text: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "fmine-spatial-command-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("front.toml"), text).unwrap();
        Self(path)
    }

    fn run(&self, options: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_fmine"))
            .args([
                "forecast",
                "spatial-monitor",
                "--input",
                "front.toml",
                "--workspace",
            ])
            .arg(&self.0)
            .args(options)
            .output()
            .expect("run fmine")
    }
}

fn body(output: &Output) -> &str {
    std::str::from_utf8(&output.stdout).unwrap()
}

#[test]
fn spatial_monitor_binary_retains_sources_and_replays_identically() {
    let workspace = Workspace::new(INPUT);
    let first = workspace.run(&["--json"]);
    assert_eq!(first.status.code(), Some(0), "{}", body(&first));
    assert_eq!(first.stdout, workspace.run(&["--json"]).stdout);
    let json = body(&first);
    assert!(json.contains("fmine.spatial-monitor-command.v1"));
    assert!(json.contains(&sha256(INPUT.as_bytes()).to_hex()));
    for expected in [
        "synthetic-front-model",
        "polygon-1",
        "polygon-2",
        "polygon-3",
        "synthetic survey:polygon-3",
        "\"execution_authorized\":false",
        "\"reserve_disclosure_claim\":false",
        "\"geotechnical_signoff\":false",
        "\"regulatory_compliance_claim\":false",
    ] {
        assert!(json.contains(expected), "missing {expected}: {json}");
    }

    let markdown = workspace.run(&["--format", "markdown"]);
    assert_eq!(markdown.status.code(), Some(0), "{}", body(&markdown));
    assert!(body(&markdown).contains("polygon-3"));
    assert!(body(&markdown).contains("synthetic-front-model"));
    assert_eq!(
        markdown.stdout,
        workspace.run(&["--format", "markdown"]).stdout
    );

    let changed = INPUT.replace("observed_value = \"5.2\"", "observed_value = \"6.2\"");
    std::fs::write(workspace.0.join("front.toml"), &changed).unwrap();
    let rerun = workspace.run(&["--json"]);
    assert_eq!(rerun.status.code(), Some(0), "{}", body(&rerun));
    assert_ne!(rerun.stdout, first.stdout);
    assert!(body(&rerun).contains(&sha256(changed.as_bytes()).to_hex()));
}

#[test]
fn spatial_monitor_binary_replays_only_observations_available_at_the_cutoff() {
    let workspace = Workspace::new(INPUT);
    let command = ["--as-of-ns", "50", "--json"];
    let output = workspace.run(&command);
    assert_eq!(output.status.code(), Some(0), "{}", body(&output));
    assert_eq!(output.stdout, workspace.run(&command).stdout);
    let json = body(&output);
    assert!(json.contains("fmine.spatial-assessment.as-of.v1"));
    assert!(json.contains("\"as_of_ns\":\"50\""));
    assert!(json.contains("\"observations\":2"));
    assert!(json.contains("synthetic survey:polygon-2"));
    assert!(!json.contains("polygon-3"));
    assert!(json.contains(&sha256(INPUT.as_bytes()).to_hex()));
    for flag in [
        "execution_authorized",
        "reserve_disclosure_claim",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert!(json.contains(&format!("\"{flag}\":false")));
    }

    let markdown = workspace.run(&["--format", "markdown", "--as-of-ns", "50"]);
    assert_eq!(markdown.status.code(), Some(0), "{}", body(&markdown));
    assert!(body(&markdown).contains("Declared availability cutoff: 50 ns"));
    assert!(!body(&markdown).contains("polygon-3"));

    let earlier = workspace.run(&["--as-of-ns", "10", "--json"]);
    assert_eq!(earlier.status.code(), Some(0), "{}", body(&earlier));
    assert!(body(&earlier).contains("\"observations\":0"));
    assert!(!body(&earlier).contains("polygon-1"));
    let before_policy = workspace.run(&["--as-of-ns", "9", "--json"]);
    assert!(body(&before_policy).contains("FMINE-REF-206"));
    assert!(!body(&before_policy).contains("\"assessment\":{"));
    for invalid in [
        &["--as-of-ns", "-1"][..],
        &["--as-of-ns", "50.5"][..],
        &["--as-of-ns", "50", "--as-of-ns", "70"][..],
    ] {
        let error = workspace.run(invalid);
        assert_eq!(error.status.code(), Some(2), "{}", body(&error));
    }
    assert_eq!(
        std::fs::read_to_string(workspace.0.join("front.toml")).unwrap(),
        INPUT
    );
}

#[test]
fn spatial_monitor_binary_refuses_time_scope_and_numeric_substitutions() {
    let workspace = Workspace::new(INPUT);
    for changed in [
        INPUT.replace("available_ns = 50", "available_ns = 30"),
        INPUT.replace(
            "prediction_registered_ns = 40",
            "prediction_registered_ns = 50",
        ),
        INPUT.replacen("target_unit = \"g/t\"", "target_unit = \"percent\"", 1),
        INPUT.replacen(
            "support = \"mining_polygon\"",
            "support = \"point_core\"",
            1,
        ),
        INPUT.replace("observed_value = \"5.2\"", "observed_value = \"NaN\""),
    ] {
        std::fs::write(workspace.0.join("front.toml"), &changed).unwrap();
        let refused = workspace.run(&["--json"]);
        assert_ne!(refused.status.code(), Some(0), "{}", body(&refused));
        assert!(body(&refused).contains("FMINE-REF-"));
        assert!(!body(&refused).contains("\"assessment\":{"));
        assert_eq!(
            std::fs::read_to_string(workspace.0.join("front.toml")).unwrap(),
            changed
        );
    }

    let invalid = workspace.run(&["--unknown", "value"]);
    assert_eq!(invalid.status.code(), Some(2), "{}", body(&invalid));
}

#[test]
fn spatial_monitor_is_registered_and_documented_for_robots() {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(["robot-docs", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", body(&output));
    assert!(body(&output).contains("\"forecast_spatial_monitor\":"));
    assert!(body(&output).contains("forecast spatial-monitor --input"));
    assert!(body(&output).contains("--as-of-ns"));

    let schema = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(["robot", "schema", "verbs", "--json"])
        .output()
        .unwrap();
    assert_eq!(schema.status.code(), Some(0), "{}", body(&schema));
    assert!(body(&schema).contains("forecast spatial-monitor"));
}
