#![cfg(test)]

//! BD-351.S1: independent exact endpoint checks and CLI admission/rendering parity.
//! All worksheets contain invented assumptions, not measured recovery or plant calibration.

#![allow(clippy::expect_used)]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const MODEL: &str = include_str!("../../../tests/decisions/untested_ore.toml");
static NEXT: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn binary() -> PathBuf {
    if let Some(path) = std::env::var_os("FMINE_TEST_BINARY") {
        return PathBuf::from(path);
    }
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "deps" || name == "build"))
        .and_then(Path::parent)
        .expect("Cargo profile directory; otherwise set FMINE_TEST_BINARY");
    profile.join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
}

fn invoke(path: &Path, workspace: &Path) -> fmine_cli::Envelope {
    let args = [
        "decide".to_string(),
        "recovery-bounds".to_string(),
        path.to_string_lossy().into_owned(),
    ];
    let (result, format) = fmine_cli::invoke(&args, workspace);
    assert_eq!(format, fmine_cli::OutputFormat::Json);
    result
}

fn declared(text: &str) -> fmine_cli::Envelope {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "fmine-recovery-bounds-{}-{stamp}-{serial}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("create synthetic worksheet directory");
    let path = directory.join("model.toml");
    std::fs::write(&path, text).expect("write synthetic worksheet");
    invoke(&path, &root())
}

fn python(json: &str, checks: &str) {
    let script = format!("{ORACLE}\n{checks}\n");
    let mut child = Command::new("python3")
        .args(["-I", "-c", &script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python standard-library exact oracle");
    child.stdin.take().expect("oracle stdin").write_all(json.as_bytes()).expect("write oracle input");
    let output = child.wait_with_output().expect("wait for exact oracle");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

const ORACLE: &str = r#"
import itertools, json, sys
from fractions import Fraction as F

bundle = json.load(sys.stdin)
env = bundle[0] if isinstance(bundle, list) else bundle
assert env["status"] == "ok" and env["exit_code"] == 0 and env["refusal"] is None, env
assert env["certificate"] is None
assert env["flags"] and all(value is False for value in env["flags"].values())
payload = env["payload"]
assert payload["mode"] == "declared_recovery_bounds"
card = payload["card"]
assert card["schema"] == "fmine.recovery-bounds-card.v1"
assert card["calibration"] == "not_established" and card["disclosure_eligible"] is False
assert len(card["digest"]) == 64 and int(card["digest"], 16) >= 0
assert card["no_claim"] and env["no_claim"]
inputs, output = card["inputs"], card["result"]
assert inputs["evidence"] == output["evidence"] == "assumed"
assert inputs["confidentiality"] == output["confidentiality"] == "Internal"
assert inputs["representation"] == output["representation"] == "deterministic"
assert inputs["support"] == output["support"] == "period_aggregate"
assert inputs["additivity"] == output["additivity"] == "non_additive"
assert inputs["state"] == output["state"] == "estimated"
assert len(inputs["uu_terms"]) == 1 and inputs["uu_terms"] == output["uu_terms"]
assert inputs["uu_terms"][0]["downside_exposure_usd_m"] > 0
assert inputs["id"] in output["parents"]
assert inputs["anchors"] == output["anchors"]
assert inputs["anchors"] and all(len(a["sha256"]) == 64 for a in inputs["anchors"])
assert len({a["sha256"] for a in inputs["anchors"]}) == 1
assert any("lower" in a["locator"] or "upper" in a["locator"] for a in inputs["anchors"])
model, result = inputs["value"], output["value"]
pair = lambda interval: (F(interval["lower"]), F(interval["upper"]))
weights = model["weights"]
if weights["basis"] == "contained_metal":
    w = pair(weights["untested_fraction"])
else:
    assert weights["basis"] == "dry_tonnes_and_grade"
    tt, tg = pair(weights["tested_dry_metric_tonnes"]), pair(weights["tested_grade_g_per_t"])
    ut, ug = pair(weights["untested_dry_metric_tonnes"]), pair(weights["untested_grade_g_per_t"])
    t, u = (tt[0]*tg[0], tt[1]*tg[1]), (ut[0]*ug[0], ut[1]*ug[1])
    assert t[0]+u[0] > 0
    w = (u[0]/(u[0]+t[1]), u[1]/(u[1]+t[0]))
tested, untested = pair(model["tested_recovery"]), pair(model["untested_recovery"])
expected = [(wi, ti, ui, (1-wi)*ti+wi*ui) for wi, ti, ui in itertools.product(w, tested, untested)]
actual = [
    tuple(F(vertex[key]) for key in
        ("untested_metal_fraction", "tested_recovery", "untested_recovery", "aggregate_recovery"))
    for vertex in result["vertices"]
]
assert len(actual) == 8 and sorted(actual) == sorted(expected)
bounds = min(v[3] for v in expected), max(v[3] for v in expected)
assert pair(result["untested_metal_fraction"]) == w
assert pair(result["recovery"]) == bounds
threshold = F(model["threshold"])
assert F(result["threshold"]) == threshold
stability = "stable_proceed" if bounds[0] >= threshold else "stable_stop" if bounds[1] < threshold else "unstable"
assert result["stability"] == stability
"#;

#[test]
fn declared_known_answers_match_independent_exact_endpoint_arithmetic() {
    let workspace = root();
    let uncertain = invoke(Path::new("tests/decisions/untested_ore.toml"), &workspace);
    python(&uncertain.to_json(), r#"
assert w == (F(3,20), F(1,4))
assert bounds == (F(19,25), F(917,1000))
assert result["stability"] == "unstable"
assert result["proceed_condition"] == {"kind":"untested_lower_at_least", "comparison":">=", "value":"19/25"}
assert result["stop_condition"] == {"kind":"untested_upper_below", "comparison":"<", "value":"34/75"}
"#);
    let tonnes = invoke(Path::new("tests/decisions/untested_ore_tonnes.toml"), &workspace);
    python(&tonnes.to_json(), r#"
assert w == (F(3,7), F(3,7))
assert bounds == (F(24,35), F(9,10))
assert result["stability"] == "unstable"
"#);
    let fixed = MODEL
        .replace("lower = \"0.15\"\nupper = \"0.25\"", "lower = \"0.20\"\nupper = \"0.20\"")
        .replace("lower = \"0.88\"\nupper = \"0.92\"", "lower = \"0.90\"\nupper = \"0.90\"");
    python(&declared(&fixed).to_json(), r#"
assert w == (F(1,5), F(1,5))
assert bounds == (F(4,5), F(9,10))
assert result["stability"] == "unstable"
"#);
}

#[test]
fn decisive_test_conditions_preserve_the_threshold_tie() {
    let proceed = MODEL.replace("lower = \"0.40\"", "lower = \"19/25\"");
    python(&declared(&proceed).to_json(), r#"
assert bounds[0] == F(17,20) and result["stability"] == "stable_proceed"
"#);
    let tie = MODEL.replace("upper = \"0.90\"", "upper = \"34/75\"");
    python(&declared(&tie).to_json(), r#"
assert bounds[1] == F(17,20) and result["stability"] == "unstable"
"#);
    let stop = MODEL.replace("upper = \"0.90\"", "upper = \"0.45\"");
    python(&declared(&stop).to_json(), r#"
assert bounds[1] == F(1699,2000) and result["stability"] == "stable_stop"
"#);
}

#[test]
fn interaction_missing_basis_and_unbudgeted_unknowns_never_produce_a_card() {
    let cases = [
        (format!("{MODEL}\n[[interaction]]\nkind = \"preg_robbing\"\n"), "FMINE-REF-103"),
        (MODEL.lines().filter(|line| !line.starts_with("additivity_basis")).collect::<Vec<_>>().join("\n"), "FMINE-REF-506"),
        (MODEL.replace("lower = \"0.40\"\n", ""), "FMINE-REF-506"),
        (MODEL.replace("lower = \"0.40\"", "lower = \"0.95\""), "FMINE-REF-109"),
        (MODEL.replace("upper = \"0.90\"", "upper = \"1.10\""), "FMINE-REF-109"),
        (MODEL.replace("lower = \"0.40\"", "lower = \"+2/5\""), "FMINE-REF-506"),
        (MODEL.replace("lower = \"0.40\"", "lower = \"2/+5\""), "FMINE-REF-506"),
        (MODEL.replace("lower = \"0.40\"", "lower = \"1.1e-2147483648\""), "FMINE-REF-506"),
        (MODEL.split("[[uu]]").next().expect("UU header").to_string(), "FMINE-REF-204"),
        (MODEL.replace("downside_usd_m = \"10\"", "downside_usd_m = \"0\""), "FMINE-REF-204"),
    ];
    for (text, code) in cases {
        let result = declared(&text);
        assert_eq!(result.refusal.as_ref().map(|r| r.code), Some(code), "{}", result.to_json());
        assert_ne!(result.exit_code, 0);
        assert!(result.certificate.is_none());
        assert!(result.rendered_markdown.is_none());
        assert!(!result.payload_json.contains("\"schema\":\"fmine.recovery-bounds-card.v1\""));
    }
}

#[test]
fn source_identity_and_uu_declarations_remain_visible_in_the_receipt() {
    let original = declared(MODEL);
    let reprinted = declared(&MODEL.replace("lower = \"0.40\"", "lower = \"0.4000\""));
    python(&format!("[{},{}]", original.to_json(), reprinted.to_json()), r#"
other = bundle[1]["payload"]["card"]
assert card["inputs"]["value"] == other["inputs"]["value"]
assert card["result"]["value"] == other["result"]["value"]
assert card["digest"] != other["digest"]
assert card["inputs"]["anchors"][0]["sha256"] != other["inputs"]["anchors"][0]["sha256"]
assert any("0.4000" in a["locator"] for a in other["inputs"]["anchors"])
"#);
    let changed = declared(&MODEL.replace("downside_usd_m = \"10\"", "downside_usd_m = \"25.0\""));
    python(&format!("[{},{}]", original.to_json(), changed.to_json()), r#"
other = bundle[1]["payload"]["card"]
assert card["result"]["value"] == other["result"]["value"]
assert card["digest"] != other["digest"]
assert other["inputs"]["uu_terms"][0]["downside_exposure_usd_m"] == 25
assert other["inputs"]["uu_terms"] == other["result"]["uu_terms"]
"#);
}

#[test]
fn real_binary_matches_library_json_and_readable_exact_cards() {
    let workspace = root();
    for fixture in ["tests/decisions/untested_ore.toml", "tests/decisions/untested_ore_tonnes.toml"] {
        let expected = invoke(Path::new(fixture), &workspace);
        assert_eq!(expected.exit_code, 0, "{}", expected.to_json());
        let markdown = expected.rendered_markdown.clone().expect("exact decision card Markdown");
        assert!(markdown.contains("Endpoint checks"));
        assert!(markdown.contains("SHA-256"));
        assert!(markdown.contains("Disclosure") || markdown.contains("disclosure"));
        for (format, text) in [("json", expected.to_json()), ("markdown", markdown)] {
            let output = Command::new(binary())
                .args(["decide", "recovery-bounds", fixture, "--format", format, "--workspace"])
                .arg(&workspace)
                .output()
                .expect("execute actual recovery-bounds command; build fmine or set FMINE_TEST_BINARY");
            assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
            assert_eq!(String::from_utf8(output.stdout).expect("UTF-8 rendering"), format!("{text}\n"));
        }
    }
}
