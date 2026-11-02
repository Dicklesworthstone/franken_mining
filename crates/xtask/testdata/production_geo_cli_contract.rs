//! BD-062: declared production-GEO CLI, source binding, typed failures and actual binary parity.
//! The GB-27 fixture is invented arithmetic, not a real issuer production report or price declaration.

#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::hash::sha256;

const EXAMPLE: &str = include_str!("production_geo.toml");
static NEXT: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-production-geo-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}

fn run(root: &Path, text: &str) -> Envelope {
    fs::write(root.join("worksheet.toml"), text).unwrap();
    let (output, format) = invoke(
        &args(&["metrics", "geo", "--input", "worksheet.toml"]),
        root,
    );
    assert_eq!(format, OutputFormat::Json);
    output
}

fn string_field<'a>(json: &'a str, name: &str) -> &'a str {
    json.split_once(&format!("\"{name}\":\""))
        .unwrap_or_else(|| panic!("missing {name}: {json}"))
        .1
        .split('"')
        .next()
        .unwrap()
}

fn binary() -> PathBuf {
    if let Some(path) = std::env::var_os("FMINE_TEST_BINARY") {
        return PathBuf::from(path);
    }
    let executable = std::env::current_exe().unwrap();
    let profile = executable
        .ancestors()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "deps" || name == "build")
        })
        .and_then(Path::parent)
        .expect("Cargo profile directory; set FMINE_TEST_BINARY for another layout");
    profile.join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
}

#[test]
fn geo_cli_reproduces_the_declared_price_effect_with_bound_sources_and_no_authority() {
    let root = root();
    let output = run(&root, EXAMPLE);
    assert_eq!(output.exit_code, 0, "{}", output.to_json());
    assert_eq!(output.verb, "metrics geo");
    let json = output.to_json();
    for expected in [
        "\"total_geo_oz\":\"3860000\"",
        "\"total_geo_oz\":\"3640000\"",
        "\"difference_from_reference_oz\":\"-220000\"",
        "\"geo_oz\":\"660000\"",
        "\"geo_oz\":\"440000\"",
        "\"metals_outside_declared_population\":[\"silver\"]",
        "\"calibration\":\"not_established\"",
        "population.metals",
        "quantity.unit",
        "deck.gold_price_per_oz",
        "\"reserve_disclosure_claim\":false",
        "\"execution_authorized\":false",
        "\"geotechnical_signoff\":false",
        "\"regulatory_compliance_claim\":false",
    ] {
        assert!(json.contains(expected), "missing {expected}: {json}");
    }
    assert!(json.contains(&sha256(EXAMPLE.as_bytes()).to_hex()));
    assert!(output.anchor_id.is_none() && output.certificate.is_none());
    assert!(output.no_claim.unwrap().contains("price-ratio basis only"));
    assert_eq!(
        fs::read(root.join("worksheet.toml")).unwrap(),
        EXAMPLE.as_bytes()
    );
    assert_eq!(json, run(&root, EXAMPLE).to_json());
    let call = args(&["metrics", "geo", "--input", "worksheet.toml"]);
    for (format, expected) in [
        ("json", json),
        ("markdown", output.rendered_markdown.unwrap()),
    ] {
        let result = Command::new(binary())
            .args(&call)
            .args(["--format", format, "--workspace"])
            .arg(&root)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(0), "{:?}", result.stderr);
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            format!("{expected}\n")
        );
    }
}

#[test]
fn geo_cli_rejects_ambiguous_options_and_unreadable_or_oversized_inputs() {
    let root = root();
    for options in [
        vec!["metrics", "geo"],
        vec!["metrics", "geo", "--input"],
        vec!["metrics", "geo", "--input", "--format", "json"],
        vec!["metrics", "geo", "--input", "a", "--input", "b"],
        vec!["metrics", "geo", "--input", "a", "--definition", "anything"],
        vec!["metrics", "geo", "a"],
    ] {
        let output = invoke(&args(&options), &root).0;
        assert_eq!(output.exit_code, 2, "{options:?}: {}", output.to_json());
    }
    fs::write(root.join("invalid.toml"), [0xff]).unwrap();
    fs::write(root.join("large.toml"), vec![b' '; 1_048_577]).unwrap();
    for path in ["missing.toml", ".", "invalid.toml", "large.toml"] {
        let output = invoke(&args(&["metrics", "geo", "--input", path]), &root).0;
        assert_ne!(output.exit_code, 0);
        let refusal = output.refusal.unwrap();
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(refusal.what.contains("GEO worksheet input"));
        assert!(!output.payload_json.contains("total_geo_oz"));
    }
}

#[test]
fn geo_cli_preserves_refusals_and_rebinds_every_source_mutation() {
    let root = root();
    let original = run(&root, EXAMPLE);
    assert_eq!(original.exit_code, 0, "{}", original.to_json());
    let changed_comment = run(
        &root,
        &(EXAMPLE.to_string() + "\n# New declared-file vintage\n"),
    );
    assert_eq!(changed_comment.exit_code, 0);
    assert_ne!(
        string_field(&original.payload_json, "digest"),
        string_field(&changed_comment.payload_json, "digest")
    );
    assert_ne!(
        string_field(&original.payload_json, "sha256"),
        string_field(&changed_comment.payload_json, "sha256")
    );
    assert!(
        changed_comment
            .payload_json
            .contains("\"total_geo_oz\":\"3640000\"")
    );
    let changed_price = run(
        &root,
        &EXAMPLE.replace(
            "gold_price_per_oz = \"4500\"",
            "gold_price_per_oz = \"9000\"",
        ),
    );
    assert_eq!(changed_price.exit_code, 0, "{}", changed_price.to_json());
    assert!(
        changed_price
            .payload_json
            .contains("\"total_geo_oz\":\"3420000\"")
    );
    assert!(
        changed_price
            .payload_json
            .contains("\"difference_from_reference_oz\":\"-440000\"")
    );
    let silver_without_price = EXAMPLE.replace(
        "metals = \"gold,copper\"",
        "metals = \"gold,copper,silver\"",
    ) + "\n[[quantity]]\nmetal = \"silver\"\nunit = \"oz\"\namount = \"100\"\n";
    for (text, code) in [
        (
            EXAMPLE.replace("unit = \"lb\"", "unit = \"kt\""),
            "FMINE-REF-107",
        ),
        (
            EXAMPLE.replace("gold_price_per_oz = \"3000\"", "gold_price_per_oz = \"0\""),
            "FMINE-REF-109",
        ),
        (silver_without_price, "FMINE-REF-201"),
    ] {
        let output = run(&root, &text);
        assert_ne!(output.exit_code, 0, "{text}");
        assert_eq!(output.refusal.unwrap().code, code);
        assert!(!output.payload_json.contains("total_geo_oz"));
    }
}
