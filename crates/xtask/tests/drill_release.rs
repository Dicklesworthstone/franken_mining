//! Native CLI control for BD-350. All readings and document locators are synthetic.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use fmine_core::hash::sha256;
use fmine_gen::junior::{JuniorCampaignConfig, JuniorScoringHarness};
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
fn quoted_true_width_clears_only_its_own_interval() {
    let (header, _) = EXAMPLE.split_once("[[phrase]]").unwrap();
    let host = "DR-001: 12.0 m @ 5.00 g/t Au";
    let cases = [
        (
            "quoted",
            format!("{host} (estimated true width ≈ 9.6 m)"),
            None,
            false,
        ),
        (
            "year only",
            format!("{host} (true width estimated for 2026)"),
            None,
            true,
        ),
        (
            "unknown",
            format!("{host} (true width unknown)"),
            None,
            true,
        ),
        (
            "negated width",
            format!("{host} (not true width: 9.6 m)"),
            None,
            true,
        ),
        (
            "unitless width",
            format!("{host} (true width 9.6)"),
            None,
            true,
        ),
        ("manual", host.to_owned(), Some(true), false),
        (
            "explicit false",
            format!("{host} (true width 9.6 m)"),
            Some(false),
            true,
        ),
        (
            "unknown inclusion",
            format!(
                "{host} (true width 9.6 m) including 2.0 m @ 12.00 g/t Au (true width unknown)"
            ),
            Some(true),
            true,
        ),
        (
            "both intervals",
            format!("{host} (true width 9.6 m) including 2.0 m @ 12.00 g/t Au (true width 1.6m)"),
            None,
            false,
        ),
        (
            "width on child only",
            "DR-001: 2.0 m @ 12.00 g/t Au (true width 1.6 m) within 12.0 m @ 5.00 g/t Au".into(),
            None,
            true,
        ),
    ];
    let directory = new_pack();
    let mut direct = Vec::new();
    for (index, (name, phrase, declared, want_flag)) in cases.into_iter().enumerate() {
        let declaration = declared.map_or_else(String::new, |width| {
            format!("true_width_stated = \"{width}\"\n")
        });
        let input = format!(
            "{header}[[phrase]]\nlocator = \"p. 2, {name}\"\ntext = \"{phrase}\"\n{declaration}"
        );
        let payload = ok(&input);
        let interval = &payload.get("intervals").unwrap().as_array().unwrap()[0];
        let flags = interval.get("flags").unwrap().as_array().unwrap();
        assert_eq!(
            flags
                .iter()
                .any(|flag| flag.get("kind").unwrap().as_str() == Some("no_true_width")),
            want_flag,
            "{name}"
        );
        std::fs::write(directory.join(format!("{index:02}.toml")), &input).unwrap();
        direct.push(payload);
    }
    let out = screen(&directory, "json");
    assert!(out.status.success());
    let pack = parse(&out.stdout).unwrap();
    let rows = pack
        .get("payload")
        .unwrap()
        .get("documents")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(rows.len(), direct.len());
    for (row, expected) in rows.iter().zip(direct) {
        let actual = row.get("result").unwrap().get("payload").unwrap();
        for field in ["arithmetic_digest", "intervals", "release_flags", "summary"] {
            assert_eq!(actual.get(field), expected.get(field), "{field}");
        }
        assert_eq!(
            actual.get("worksheet").unwrap().get("sha256"),
            expected.get("worksheet").unwrap().get("sha256")
        );
    }
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

fn new_pack() -> PathBuf {
    let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "fmine-target-pack-{}-{}-{}",
        std::process::id(),
        time.as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&directory).unwrap();
    directory
}

fn screen(directory: &Path, format: &str) -> Output {
    Command::new(binary())
        .args(["screen", "target", "--docs"])
        .arg(directory)
        .args(["--format", format])
        .output()
        .unwrap()
}

#[test]
fn native_target_screen_matches_generated_release_arithmetic_and_public_npv() {
    let directory = new_pack();
    let cfg = JuniorCampaignConfig {
        seed_root: 0xBD35_0505,
        replicates_per_class: 10,
        holes_per_release: 6,
    };
    let harness = JuniorScoringHarness::new(cfg.seed_root);
    let campaign = harness.generate_campaign(&cfg).unwrap();
    assert_eq!(campaign.len(), 60);
    let mut expected = Vec::new();
    // Reverse the insertion order so the reported name order is exercised independently.
    for (index, (release, _)) in campaign.iter().enumerate().rev() {
        let name = format!("{index:02}.toml");
        let worksheet = release.render_drill_release_worksheet_toml();
        let result = JuniorScoringHarness::audit_packet(release).unwrap();
        std::fs::write(directory.join(&name), &worksheet).unwrap();
        expected.push((name, sha256(worksheet.as_bytes()), result.audit));
    }
    expected.reverse();
    let npv = include_str!("../../../tests/exploration/taguas_2021_pre_tax.toml")
        .replace("end_of_first_period", "auto");
    std::fs::write(directory.join("valuation.toml"), npv).unwrap();
    let out = screen(&directory, "json");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(out.stdout, screen(&directory, "json").stdout);
    let json = parse(&out.stdout).unwrap();
    let payload = json.get("payload").unwrap();
    assert_eq!(payload.get("screened").unwrap().u64(), Some(61));
    assert_eq!(payload.get("refused").unwrap().u64(), Some(0));
    let rows = payload.get("documents").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), 61);
    for (row, (name, digest, audit)) in rows.iter().zip(&expected) {
        assert_eq!(row.get("file").unwrap().as_str(), Some(name.as_str()));
        let result = row.get("result").unwrap();
        let result = result.get("payload").unwrap();
        assert_eq!(result.get("evidence").unwrap().as_str(), Some("synthetic"));
        assert_eq!(
            row.get("source").unwrap().get("sha256").unwrap().as_str(),
            Some(digest.to_hex().as_str())
        );
        assert_eq!(
            result.get("arithmetic_digest").unwrap().as_str(),
            Some(audit.digest.to_string().as_str()),
            "{name}"
        );
        let intervals = result.get("intervals").unwrap().as_array().unwrap();
        assert_eq!(intervals.len(), audit.intervals.len());
        assert!(intervals.iter().all(|interval| {
            !interval
                .get("sources")
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        }));
    }
    let valuation = rows
        .last()
        .unwrap()
        .get("result")
        .unwrap()
        .get("payload")
        .unwrap();
    assert_eq!(
        valuation.get("year_zero_timing").unwrap().as_str(),
        Some("end_of_first_period")
    );
    assert_eq!(
        valuation
            .get("comparisons")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        6
    );
}

#[test]
fn native_target_screen_keeps_good_results_beside_refusals() {
    let directory = new_pack();
    let absent = screen(&directory.join("missing"), "json");
    assert_eq!(absent.status.code(), Some(5));
    assert_eq!(screen(&directory, "json").status.code(), Some(5));
    std::fs::write(directory.join("b-good.toml"), EXAMPLE).unwrap();
    let wrong_schema = "schema = \"unknown\"\n";
    std::fs::write(directory.join("a-bad.toml"), wrong_schema).unwrap();
    std::fs::write(directory.join("c-too-large.toml"), vec![b'x'; 65_537]).unwrap();
    let no_fit = include_str!("../../../tests/exploration/developer_npv.toml")
        .replace("\"25.1\"", "\"80\"")
        .replacen("[[flow]]", "year_zero_timing = \"auto\"\n\n[[flow]]", 1);
    std::fs::write(directory.join("d-no-fit.toml"), no_fit).unwrap();
    let out = screen(&directory, "json");
    assert!(!out.status.success());
    let json = parse(&out.stdout).unwrap();
    let payload = json.get("payload").unwrap();
    assert_eq!(payload.get("screened").unwrap().u64(), Some(1));
    assert_eq!(payload.get("refused").unwrap().u64(), Some(3));
    let files = payload.get("documents").unwrap().as_array().unwrap();
    assert_eq!(
        files[0]
            .get("source")
            .unwrap()
            .get("sha256")
            .unwrap()
            .as_str(),
        Some(sha256(wrong_schema.as_bytes()).to_hex().as_str())
    );
    assert_eq!(
        files[0]
            .get("result")
            .unwrap()
            .get("refusal")
            .unwrap()
            .get("code")
            .unwrap()
            .as_str(),
        Some("FMINE-REF-105")
    );
    assert_eq!(
        files[1]
            .get("result")
            .unwrap()
            .get("status")
            .unwrap()
            .as_str(),
        Some("ok")
    );
    assert_eq!(files[2].get("source"), Some(&Json::Null));
    let valuation = files[3].get("result").unwrap();
    assert_eq!(valuation.get("status").unwrap().as_str(), Some("refused"));
    let comparisons = valuation
        .get("payload")
        .unwrap()
        .get("comparisons")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(comparisons.len(), 6);
    assert!(
        comparisons
            .iter()
            .all(|row| row.get("overlaps_printed") == Some(&Json::Bool(false)))
    );
    let md = screen(&directory, "markdown");
    assert!(!md.status.success());
    let markdown = String::from_utf8(md.stdout).unwrap();
    assert!(markdown.contains("1 of 4 worksheets screened. 3 refused."));
    assert!(markdown.contains("p. 4, DR-003"));
    assert_eq!(markdown.matches("Refused `FMINE-REF-104`").count(), 1);
    for i in 0..61 {
        std::fs::write(directory.join(format!("extra-{i}.toml")), EXAMPLE).unwrap();
    }
    let oversized = screen(&directory, "json");
    assert_eq!(oversized.status.code(), Some(5));
    let bad_usage = Command::new(binary())
        .args(["screen", "target", "--docs"])
        .output()
        .unwrap();
    assert_eq!(bad_usage.status.code(), Some(2));
}
