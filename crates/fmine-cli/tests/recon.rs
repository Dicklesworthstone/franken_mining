use std::path::Path;

use fmine_core::hash::sha256;

const EXAMPLE: &str = include_str!("../../../tests/recon/matched_cohorts.toml");

#[test]
fn matched_cohort_command_reports_hidden_grade_loss_and_boundary_control() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut args = [
        "recon",
        "run",
        "--input",
        "tests/recon/matched_cohorts.toml",
    ]
    .map(String::from)
    .to_vec();
    let (json, status) = fmine_cli::run(&args, &root);
    assert_eq!(status, 0, "{json}");
    assert!(json.contains(&format!("\"sha256\":\"{}\"", sha256(EXAMPLE.as_bytes()))));
    assert!(json.contains("\"review_count\":2"));
    assert!(json.contains("\"dry_tonnes_delta\":\"200/1\",\"contained_metal_kg_delta\":\"2500/1\",\"tonnes_ratio\":null,\"grade_ratio\":null,\"metal_ratio\":null,\"finding\":\"unplanned_movement\",\"review\":true"));
    assert!(json.contains("\"tonnes_ratio\":\"6/5\",\"grade_ratio\":\"5/6\",\"metal_ratio\":\"1/1\",\"finding\":\"masked_within_metal_tolerance\""));
    assert!(json.contains("\"tonnes_ratio\":\"19/20\",\"grade_ratio\":\"1/1\",\"metal_ratio\":\"19/20\",\"finding\":\"within_tolerance\""));
    for split in [
        r#""metal_split":{"status":"computed","from_dry_tonnes_kg":"5500/3","from_grade_kg":"-5500/3"}"#,
        r#""metal_split":{"status":"computed","from_dry_tonnes_kg":"-500/1","from_grade_kg":"0/1"}"#,
        r#""metal_split":{"status":"undefined_grade","from_dry_tonnes_kg":null,"from_grade_kg":null}"#,
    ] {
        assert!(json.contains(split), "{json}");
    }
    args.extend(["--format".into(), "markdown".into()]);
    let (report, status) = fmine_cli::run(&args, &root);
    assert_eq!(status, 0, "{report}");
    assert!(report.contains("120.00% | 83.33% | 100.00% | masked within metal tolerance"));
    assert!(report.contains("2 of 3 cohorts need review"));
    assert!(
        report.contains("unplanned movement | 200/1 | 2500/1 | undefined grade | undefined grade")
    );
    assert!(report.contains("masked within metal tolerance | 200/1 | 0/1 | 5500/3 | -5500/3"));
    assert!(report.contains(&sha256(EXAMPLE.as_bytes()).to_string()));
}
