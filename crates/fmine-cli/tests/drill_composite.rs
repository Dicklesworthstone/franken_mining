//! SYNTHETIC independent source-to-composite controls (BD-166, BD-188).
//! Determinism: DeterministicPerIsa
#![allow(clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, invoke};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const MANIFEST: &str = include_str!("../../../tests/drill/composite_synthetic.toml");
const ASSAYS: &str = include_str!("../../../tests/drill/assays.csv");
const SURVEYS: &str = include_str!("../../../tests/drill/surveys.csv");
const DIALECTS: &str = include_str!("../../../registries/io_dialects.toml");
static NEXT: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-drill-composite-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(root.join("registries")).expect("fixture");
    for (path, value) in [
        ("input.toml", MANIFEST),
        (
            "collars.csv",
            include_str!("../../../tests/drill/collars.csv"),
        ),
        ("surveys.csv", SURVEYS),
        ("assays.csv", ASSAYS),
        ("registries/io_dialects.toml", DIALECTS),
        (
            "registries/units.toml",
            include_str!("../../../registries/units.toml"),
        ),
        (
            "registries/crs.toml",
            include_str!("../../../registries/crs.toml"),
        ),
    ] {
        std::fs::write(root.join(path), value).expect("source");
    }
    std::fs::canonicalize(root).expect("root")
}

fn args() -> Vec<String> {
    [
        "drill",
        "composite",
        "--input",
        "input.toml",
        "--cas",
        "cas",
    ]
    .map(str::to_owned)
    .into()
}

fn execute(root: &Path) -> Envelope {
    invoke(&args(), root).0
}

fn success(root: &Path) -> Json {
    let result = execute(root);
    assert_eq!(result.exit_code, 0, "{}", result.to_json());
    parse(result.to_json().as_bytes()).expect("JSON")
}

fn field<'a>(value: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter().fold(value, |v, k| v.get(k).expect(k))
}

fn hole(value: &Json) -> &Json {
    &field(value, &["payload", "report", "compositing", "holes"])
        .as_array()
        .expect("holes")[0]
}

fn pieces(value: &Json) -> &[Json] {
    field(hole(value), &["composites"])
        .as_array()
        .expect("composites")
}

fn near(actual: &Json, expected: f64) {
    let value = actual.f64().expect("number");
    assert!((value - expected).abs() < 1e-9, "{value} != {expected}");
}

fn identity(value: &Json) -> (&str, &str) {
    (
        field(value, &["payload", "receipt_sha256"])
            .as_str()
            .expect("receipt"),
        field(&pieces(value)[0], &["claim", "id"])
            .as_str()
            .expect("claim"),
    )
}

#[test]
fn native_command_preserves_domain_contacts_censor_bounds_support_and_spatial_location() {
    let root = fixture();
    let output = || {
        Command::new(env!("CARGO_BIN_EXE_fmine"))
            .current_dir(&root)
            .args(args())
            .output()
            .expect("native composite")
    };
    let first = output();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stdout)
    );
    assert_eq!(
        first.stdout,
        output().stdout,
        "repeat invocation is byte-identical"
    );
    let report = parse(&first.stdout).expect("JSON");
    assert_eq!(field(&report, &["verb"]).as_str(), Some("drill composite"));
    assert_eq!(
        field(&report, &["payload", "report", "schema"]).as_str(),
        Some("fmine.drill-composite.v1"),
    );
    for (_, flag) in field(&report, &["flags"]).as_object().expect("flags") {
        assert_eq!(flag.as_bool(), Some(false));
    }
    let h = hole(&report);
    assert_eq!(field(h, &["sampled_mm"]).u64(), Some(100_000));
    assert_eq!(field(h, &["unsampled_mm"]).u64(), Some(100_000));
    let expected = [
        (0, 60_000, 0, None),
        (60_000, 120_000, 20_000, Some((0.0, 0.005))),
        (120_000, 130_000, 10_000, Some((0.0, 0.005))),
        (130_000, 190_000, 60_000, Some((50.0 / 60.0, 50.1 / 60.0))),
        (190_000, 200_000, 10_000, Some((1.25, 1.25))),
    ];
    assert_eq!(pieces(&report).len(), expected.len());
    for (entry, (from, to, sampled, grade)) in pieces(&report).iter().zip(expected) {
        let result = field(entry, &["result"]);
        assert_eq!(field(result, &["from_mm"]).u64(), Some(from));
        assert_eq!(field(result, &["to_mm"]).u64(), Some(to));
        assert_eq!(field(result, &["sampled_mm"]).u64(), Some(sampled));
        assert_eq!(
            field(result, &["unsampled_mm"]).u64(),
            Some(to - from - sampled)
        );
        assert_eq!(
            field(entry, &["claim", "support", "length_mm"]).u64(),
            Some(to - from)
        );
        assert_eq!(
            field(entry, &["claim", "evidence"]).as_str(),
            Some("Assumed")
        );
        match grade {
            None => assert_eq!(field(result, &["grade_bounds"]), &Json::Null),
            Some((lo, hi)) => {
                near(field(result, &["grade_bounds", "lower"]), lo);
                near(field(result, &["grade_bounds", "upper"]), hi);
            }
        }
        for (key, depth) in [("start", from), ("end", to)] {
            let point = field(result, &[key, "point_m"]).as_array().expect("point");
            near(&point[0], 500_000.0);
            near(&point[1], 4_500_000.0 + depth as f64 / 1000.0);
            near(&point[2], 1500.0);
            let covariance = field(result, &[key, "collar_covariance_m2"])
                .as_array()
                .expect("covariance");
            near(&covariance[0].as_array().expect("row")[0], 0.04);
            assert_eq!(
                field(result, &[key, "covariance_terms_m2"])
                    .as_array()
                    .expect("terms")
                    .len(),
                2,
            );
        }
    }
    let totals = field(h, &["conservation"]);
    for key in ["input_total_bounds", "output_total_bounds"] {
        near(field(totals, &[key, "lower"]), 62_500.0);
        near(field(totals, &[key, "upper"]), 62_750.0);
    }
    near(field(totals, &["relative_error"]), 0.0);
    let inputs = field(h, &["inputs"]).as_array().expect("source inputs");
    assert_eq!(inputs.len(), 2);
    assert_eq!(
        field(&inputs[0], &["reading", "limit"]).as_str(),
        Some("1/200")
    );
    assert_eq!(
        field(&inputs[0], &["reading", "raw"]).as_str(),
        Some("<0.005")
    );
    assert_eq!(field(&inputs[0], &["method_assumed"]).as_bool(), Some(true));
    let contributions = field(&pieces(&report)[3], &["result", "contributors"])
        .as_array()
        .expect("contributors");
    assert_eq!(
        field(&contributions[0], &["overlap_mm"]).u64(),
        Some(20_000)
    );
    assert_eq!(
        field(&contributions[1], &["overlap_mm"]).u64(),
        Some(40_000)
    );
    for source in field(&report, &["payload", "report", "sources"])
        .as_array()
        .expect("anchors")
    {
        let path = field(source, &["path"]).as_str().expect("source path");
        assert_eq!(
            field(source, &["sha256"]).as_str(),
            Some(
                sha256(&std::fs::read(path).expect("source bytes"))
                    .to_string()
                    .as_str()
            ),
        );
    }
}

#[test]
fn residuals_and_placeholders_remain_explicit_and_markdown_is_usable() {
    for (policy, kept, dropped) in [
        ("keep", 5, 0),
        ("merge_below", 3, 0),
        ("drop_below", 3, 2),
        ("distribute", 3, 0),
    ] {
        let root = fixture();
        let replacement = if matches!(policy, "keep" | "distribute") {
            format!("residual = \"{policy}\"")
        } else {
            format!("residual = \"{policy}\"\nminimum_m = \"20\"")
        };
        std::fs::write(
            root.join("input.toml"),
            MANIFEST.replace("residual = \"keep\"", &replacement),
        )
        .expect("policy");
        let report = success(&root);
        assert_eq!(pieces(&report).len(), kept);
        assert_eq!(
            field(hole(&report), &["dropped"])
                .as_array()
                .expect("dropped")
                .len(),
            dropped,
        );
        near(
            field(
                hole(&report),
                &["conservation", "output_total_bounds", "upper"],
            ),
            62_750.0,
        );
        for piece in field(hole(&report), &["dropped"])
            .as_array()
            .expect("dropped")
        {
            assert_eq!(field(piece, &["result", "dropped"]).as_bool(), Some(true));
        }
    }
    let root = fixture();
    std::fs::write(
        root.join("input.toml"),
        MANIFEST
            .replace("AU_GPT", "CU_PCT")
            .replace("g_per_t", "wt_pct"),
    )
    .expect("copper");
    let report = success(&root);
    assert_eq!(field(hole(&report), &["sampled_mm"]).u64(), Some(50_000));
    assert_eq!(field(hole(&report), &["unsampled_mm"]).u64(), Some(150_000));
    let missing = field(hole(&report), &["missing"])
        .as_array()
        .expect("missing");
    assert_eq!(missing.len(), 1);
    assert_eq!(
        field(&missing[0], &["reading", "raw"]).as_str(),
        Some("NSS")
    );
    assert_eq!(
        field(&pieces(&report)[1], &["result", "grade_bounds"]),
        &Json::Null
    );
    let mut arguments = args();
    arguments.splice(1..1, ["--format".into(), "md".into()]);
    let (envelope, format) = invoke(&arguments, &root);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert_eq!(format, fmine_cli::OutputFormat::Markdown);
    let text = envelope.rendered_markdown.expect("Markdown report");
    for expected in [
        "Drill composite",
        "SYN-01",
        "Sampled / unsampled",
        "not statistical confidence intervals",
        "assay-unit × millimetres",
    ] {
        assert!(text.contains(expected), "{text}");
    }
}

#[test]
fn source_manifest_and_interpretation_changes_bind_every_claim() {
    let root = fixture();
    let baseline = success(&root);
    for (path, contents) in [
        ("assays.csv", ASSAYS.replace("1.25", "1.50")),
        (
            "input.toml",
            MANIFEST.replace("fire_assay_aas", "declared_alternate"),
        ),
        (
            "registries/io_dialects.toml",
            format!("{DIALECTS}\n# explicitly changed interpretation snapshot\n"),
        ),
    ] {
        let prior = std::fs::read(root.join(path)).expect("prior");
        std::fs::write(root.join(path), contents).expect("mutation");
        let changed = success(&root);
        assert_ne!(identity(&baseline).0, identity(&changed).0, "{path}");
        assert_ne!(identity(&baseline).1, identity(&changed).1, "{path}");
        std::fs::write(root.join(path), prior).expect("restore fixture");
    }
}

#[test]
fn refuses_unit_method_support_domain_and_resource_errors_without_partial_composites() {
    let root = fixture();
    for (from, to) in [
        ("basis = \"dry\"", "basis = \"wet\""),
        ("unit = \"g_per_t\"", "unit = \"wt_pct\""),
        ("column = \"AU_GPT\"", "column = \"UNKNOWN\""),
        ("length_m = \"60\"", "length_m = \"0.001\""),
        ("length_m = \"60\"", "length_m = \"0\""),
        (
            "sigma_azimuth_deg = \"1\"",
            "sigma_azimuth_deg = \"1e-400\"",
        ),
        ("from_m = \"130\"", "from_m = \"129\""),
        ("from_m = \"130\"", "from_m = \"131\""),
        ("from_m = \"130\"", "from_m = \"130.0001\""),
        ("to_m = \"200\"", "to_m = \"201\""),
        ("residual = \"keep\"", "residual = \"drop_below\""),
    ] {
        std::fs::write(root.join("input.toml"), MANIFEST.replace(from, to)).expect("declaration");
        let result = execute(&root);
        assert_ne!(result.exit_code, 0, "{from} -> {to}: {}", result.to_json());
        assert!(
            !result.payload_json.contains("\"composites\":["),
            "no partial result"
        );
    }
    std::fs::write(root.join("input.toml"), MANIFEST).expect("valid manifest");
    let old = "column = \"AU_GPT\"\nunit = \"g_per_t\"\nbasis = \"dry\"\nsupport = \"point_core\"";
    let changed = DIALECTS.replace(old, &old.replace("point_core", "plant_lot"));
    assert_ne!(changed, DIALECTS);
    std::fs::write(root.join("registries/io_dialects.toml"), changed).expect("changed support");
    let refused = execute(&root);
    assert_ne!(refused.exit_code, 0, "{}", refused.to_json());
    assert!(refused.to_json().contains("PointCore"));
    std::fs::write(root.join("registries/io_dialects.toml"), DIALECTS).expect("original dialect");
    let changed = ASSAYS
        .replace("CU_PCT\n", "CU_PCT,METHOD\n")
        .replace("<0.005,NSS\n", "<0.005,NSS,different\n")
        .replace("1.25,0.3\n", "1.25,0.3,fire_assay_aas\n");
    assert_ne!(changed, ASSAYS);
    std::fs::write(root.join("assays.csv"), changed).expect("different methods");
    let refused = execute(&root);
    assert_ne!(refused.exit_code, 0, "{}", refused.to_json());
    assert!(refused.payload_json.contains("\"compositing\":null"));
    assert!(refused.to_json().contains("method"));
    std::fs::write(root.join("assays.csv"), ASSAYS).expect("valid assays");
    std::fs::write(
        root.join("surveys.csv"),
        SURVEYS.replace("SYN-01,100,0,0,GYRO", "SYN-01,100,0,100,GYRO"),
    )
    .expect("quarantined survey");
    let refused = execute(&root);
    assert_ne!(refused.exit_code, 0, "{}", refused.to_json());
    assert!(refused.payload_json.contains("\"rejected_rows\":[{"));
}
