//! Native CLI control for BD-350. All readings and document locators are synthetic.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const EXAMPLE: &str = include_str!("../../../tests/exploration/drill_release.toml");
static NEXT: AtomicU64 = AtomicU64::new(0);
const CITED: &str = r#"schema = "fmine.drill-release-check.v1"
document = "Synthetic cited-only release"
evidence = "synthetic"
grade_basis = "Au"

[[cited]]
id = "host"
locator = "p. 2, host row"
hole = "H-42"
length = "12.0"
length_unit = "m"
grade = "5.00"
grade_unit = "g/t"
from = "0.0"
to = "12.0"

[[cited]]
id = "zone"
parent = "host"
locator = "p. 2, zone row"
hole = "H-42"
length = "4.0"
length_unit = "m"
grade = "8.00"
grade_unit = "g/t"
from = "1.0"
to = "5.0"

[[cited]]
id = "core"
parent = "zone"
locator = "p. 3, core row"
hole = "H-42"
length = "1.0"
length_unit = "m"
grade = "14.00"
grade_unit = "g/t"
from = "2.0"
to = "3.0"
"#;

fn binary() -> PathBuf {
    if let Some(path) = std::env::var_os("FMINE_TEST_BINARY") {
        return path.into();
    }
    let exe = std::env::current_exe().expect("test executable");
    let target = exe
        .ancestors()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "build" || name == "deps")
        })
        .and_then(Path::parent)
        .expect("profile directory");
    target.join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
}

fn run(input: &str, format: &str) -> Output {
    let path = std::env::temp_dir().join(format!(
        "fmine-drill-check-{}-{}.toml",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::write(&path, input).unwrap();
    Command::new(binary())
        .args(["check", "drill-release"])
        .arg(path)
        .args(["--format", format])
        .output()
        .unwrap()
}

fn ok(input: &str) -> Json {
    let output = run(input, "json");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let body = parse(&output.stdout).unwrap();
    for flag in [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert!(String::from_utf8_lossy(&output.stdout).contains(&format!("\"{flag}\":false")));
    }
    body.get("payload").unwrap().clone()
}

#[test]
fn binary_preserves_exact_remainder_and_citations_after_sorting() {
    let output = ok(EXAMPLE);
    assert_eq!(
        output
            .get("worksheet")
            .unwrap()
            .get("sha256")
            .unwrap()
            .as_str(),
        Some(sha256(EXAMPLE.as_bytes()).to_hex().as_str()),
    );
    let intervals = output.get("intervals").unwrap().as_array().unwrap();
    assert_eq!(intervals.len(), 2);
    assert_eq!(intervals[0].get("hole").unwrap().as_str(), Some("DR-001"));
    let sources = |i: usize| intervals[i].get("sources").unwrap().as_array().unwrap();
    assert_eq!(
        sources(0)[0].get("locator").unwrap().as_str(),
        Some("p. 3, DR-001")
    );
    assert_eq!(
        sources(1)[0].get("locator").unwrap().as_str(),
        Some("p. 4, DR-003")
    );
    let remainder = intervals[1].get("remainder").unwrap();
    // (10 × 5 − 2 × 15) / (10 − 2) = 5/2.
    assert_eq!(remainder.get("grade").unwrap().as_str(), Some("5/2"));
    assert_eq!(remainder.get("length_m").unwrap().as_str(), Some("8/1"));
    let lower = remainder.get("grade_bounds").unwrap().as_array().unwrap();
    // Positive and negative inclusion contributions give these two closed-box vertices.
    assert_eq!(lower[0].as_str(), Some("947/395"));
    assert_eq!(lower[1].as_str(), Some("13/5"));
    let flags = intervals[1].get("flags").unwrap().as_array().unwrap();
    assert!(
        flags
            .iter()
            .any(|flag| flag.get("kind").unwrap().as_str() == Some("remainder_below_cutoff"))
    );
    let release = output.get("release_flags").unwrap().as_array().unwrap();
    assert_eq!(
        release[0].get("kind").unwrap().as_str(),
        Some("hole_sequence_gap")
    );
    assert_eq!(
        release[0]
            .get("missing_numbers")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .u64(),
        Some(2)
    );

    let markdown = run(EXAMPLE, "markdown");
    assert!(markdown.status.success());
    let text = String::from_utf8(markdown.stdout).unwrap();
    assert!(text.contains("p. 4, DR-003"));
    assert!(text.contains("5/2 ["));
    assert!(text.contains("hole_sequence_gap"));
}

#[test]
fn cited_rows_resolve_an_explicit_tree_and_leave_missing_context_visible() {
    let body = ok(CITED);
    let row = &body.get("intervals").unwrap().as_array().unwrap()[0];
    let value = row.get("remainder").unwrap();
    assert_eq!(value.get("grade").unwrap().as_str(), Some("7/2"));
    let nested = &value.get("nested").unwrap().as_array().unwrap()[0];
    assert_eq!(nested.get("grade").unwrap().as_str(), Some("6/1"));
    assert_eq!(row.get("sources").unwrap().as_array().unwrap().len(), 3);
    let flags = body.get("release_flags").unwrap().as_array().unwrap();
    assert_eq!(flags[0].get("missing_cutoff"), Some(&Json::Bool(true)));
    let positioned = format!(
        "{CITED}\n[[cited]]\nid = \"sibling\"\nparent = \"host\"\nlocator = \"p. 3, second zone\"\nhole = \"H-42\"\nlength = \"2.0\"\nlength_unit = \"m\"\ngrade = \"6.00\"\ngrade_unit = \"g/t\"\nfrom = \"7.0\"\nto = \"9.0\"\n"
    );
    let result = ok(&positioned);
    let intervals = result.get("intervals").unwrap().as_array().unwrap();
    assert_eq!(
        intervals[0]
            .get("remainder")
            .unwrap()
            .get("grade")
            .unwrap()
            .as_str(),
        Some("8/3")
    );
    let overlapping = positioned.replace(
        "from = \"7.0\"\nto = \"9.0\"",
        "from = \"4.0\"\nto = \"6.0\"",
    );
    let refused = run(&overlapping, "json");
    assert!(!refused.status.success());
    assert!(
        String::from_utf8(refused.stdout)
            .unwrap()
            .contains("overlapping child inclusions")
    );
}

#[test]
fn ambiguous_rows_and_unsupported_subtractions_refuse_the_whole_batch() {
    for (input, needle) in [
        (
            EXAMPLE.replace("including 2.0 m @ 15.00 g/t Au", "plus a richer core"),
            "phrase needs cited mode",
        ),
        (
            EXAMPLE.replace("2.0 m @ 15.00 g/t Au", "2.0 m @ 15.00 g/t Ag"),
            "printed metal must match",
        ),
        (
            EXAMPLE.replace(
                "2.0 m @ 15.00 g/t Au",
                "2.0 m @ 15.00 g/t Au and 1.0 m @ 8.00 g/t Au",
            ),
            "sibling inclusions need explicit",
        ),
        (
            EXAMPLE.replace("DR-003", "DR-4294967295"),
            "sequence gaps exceed 4096",
        ),
        (
            CITED.replace("parent = \"zone\"", "parent = \"core\""),
            "parent must be an earlier cited row",
        ),
    ] {
        let result = run(&input, "json");
        assert!(!result.status.success());
        let json = parse(&result.stdout).unwrap();
        assert!(
            json.get("refusal")
                .unwrap()
                .get("what")
                .unwrap()
                .as_str()
                .unwrap()
                .contains(needle)
        );
        assert!(
            json.get("payload")
                .is_none_or(|payload| payload.get("intervals").is_none())
        );
    }
}

#[test]
fn tied_host_points_with_distinct_printing_keep_a_stable_arithmetic_digest() {
    let (header, _) = EXAMPLE.split_once("[[phrase]]").unwrap();
    let first = "[[phrase]]\nlocator = \"p. 2, first\"\ntext = \"A-001: 10.0 m @ 5.00 g/t Au\"\n";
    let second = "[[phrase]]\nlocator = \"p. 3, second\"\ntext = \"A-001: 10.00 m @ 5.0 g/t Au\"\n";
    let a = ok(&format!("{header}{first}{second}"));
    let b = ok(&format!("{header}{second}{first}"));
    assert_eq!(a.get("arithmetic_digest"), b.get("arithmetic_digest"));
    assert_ne!(a.get("worksheet"), b.get("worksheet"));
}
