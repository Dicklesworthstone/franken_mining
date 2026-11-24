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

    let schema = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(["robot", "schema", "verbs", "--json"])
        .output()
        .unwrap();
    assert_eq!(schema.status.code(), Some(0), "{}", body(&schema));
    assert!(body(&schema).contains("forecast spatial-monitor"));
}
