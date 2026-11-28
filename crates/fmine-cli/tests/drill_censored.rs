//! SYNTHETIC CLI controls for independently calculated MLE/ROS summaries and retained censoring.
//! Determinism: BitwisePortable.
#![allow(clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const ASSAYS: &str = include_str!("../../../tests/drill/censored_assays.toml");
const INPUT: &str = "censored_assays.toml";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn fixture(text: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-censored-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("directory");
    std::fs::write(root.join(INPUT), text).expect("source");
    std::fs::canonicalize(root).expect("root")
}
fn args() -> Vec<String> {
    ["drill", "censored", "--input", INPUT]
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
fn fit<'a>(report: &'a Json, method: &str) -> &'a Json {
    assert_eq!(field(report, &[method, "status"]).as_str(), Some("ok"));
    field(report, &[method, "fit"])
}
fn near(value: &Json, name: &str, expected: f64, tolerance: f64) {
    let actual = field(value, &[name]).f64().expect("number");
    assert!(
        (actual - expected).abs() <= tolerance,
        "{name}: {actual} != {expected}"
    );
}
fn readings(tokens: &[&str]) -> String {
    let mut result = ASSAYS
        .split("[[observation]]")
        .next()
        .expect("header")
        .to_owned();
    for (i, token) in tokens.iter().enumerate() {
        let _ = std::fmt::Write::write_fmt(
            &mut result,
            format_args!("[[observation]]\nsample_id = \"S{i}\"\nvalue = \"{token}\"\n"),
        );
    }
    result
}

#[test]
fn native_multi_limit_comparison_matches_independent_reference_values_without_replacing_assays() {
    let root = fixture(ASSAYS);
    let native = || {
        Command::new(env!("CARGO_BIN_EXE_fmine"))
            .current_dir(&root)
            .args(args())
            .output()
            .expect("native command")
    };
    let output = native();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        execute(&root).to_json()
    );
    assert_eq!(output.stdout, native().stdout);
    let value = successful(&root);
    let r = report(&value);
    assert_eq!(field(r, &["n"]).u64(), Some(5));
    assert_eq!(field(r, &["n_uncensored"]).u64(), Some(3));
    assert_eq!(field(r, &["n_censored"]).u64(), Some(2));
    assert_eq!(field(r, &["comparison_state"]).as_str(), Some("complete"));
    assert_eq!(field(r, &["raw_assays_replaced"]).as_bool(), Some(false));
    let mle = fit(r, "mle");
    // Independent reference: SciPy 1.17.0 normal logpdf/logcdf likelihood, optimized in
    // (mu, ln sigma) by Nelder-Mead, x/f tolerances 1e-12; no candidate results used.
    // u={1/8,1/4,1/2}, left limits={1/8,1/4}; includes the -sum(log u) Jacobian.
    near(mle, "mu_log", -1.7626867323457214, 2e-7);
    near(mle, "sigma_log", 0.7075473583043507, 2e-7);
    near(mle, "log_likelihood", -0.4144132854004583, 2e-7);
    near(mle, "mean", 0.22038591721599665, 2e-7);
    near(mle, "median", 0.17158324572232686, 2e-7);
    near(mle, "p90", 0.4248885806462309, 2e-7);
    let ros = fit(r, "ros");
    // Hand-derived Hirsch-Stedinger plotting positions: measured {.45,11/15,13/15},
    // censored {.15,.30}; ordinary least squares of log(u) on their normal quantiles.
    // Reference computed independently with scipy.stats.norm.ppf and numpy.polyfit.
    near(ros, "mu_log", -1.9784948862905187, 2e-10);
    near(ros, "sigma_log", 1.104826995947064, 2e-10);
    near(ros, "mean", 0.19929388798084885, 2e-10);
    near(ros, "sd", 0.18538626647630468, 2e-10);
    near(ros, "median", 0.125, 1e-14);
    near(ros, "p75", 0.25, 1e-14);
    near(ros, "p90", 0.4, 1e-14);
    assert_eq!(field(ros, &["modeled_above_dl_count"]).u64(), Some(0));
    for result in [mle, ros] {
        assert_eq!(field(result, &["confidence_interval"]), &Json::Null);
        assert_eq!(
            field(result, &["high_censoring_warning"]).as_bool(),
            Some(false)
        );
        assert!(
            field(result, &["fitted_lognormal_probability_above_one"])
                .f64()
                .expect("tail")
                > 0.0
        );
    }
    let rows = field(r, &["observations"])
        .as_array()
        .expect("original observations");
    assert_eq!(rows.len(), 5);
    for (index, token) in ["0.125", "<0.125", "0.25", "<0.25", "0.5"]
        .iter()
        .enumerate()
    {
        assert_eq!(field(&rows[index], &["token"]).as_str(), Some(*token));
        assert_eq!(field(&rows[index], &["index"]).u64(), Some(index as u64));
    }
    assert_eq!(field(&rows[1], &["substitution"]), &Json::Null);
    assert_eq!(field(&rows[3], &["substitution"]), &Json::Null);
    assert_eq!(
        field(r, &["source", "sha256"]).as_str(),
        Some(sha256(ASSAYS.as_bytes()).to_string().as_str())
    );
    for (_, flag) in field(&value, &["flags"]).as_object().expect("flags") {
        assert_eq!(flag.as_bool(), Some(false));
    }
    assert_eq!(
        field(&value, &["payload", "claim", "evidence"]).as_str(),
        Some("Assumed")
    );
    assert_eq!(
        field(&value, &["payload", "claim", "confidentiality"]).as_str(),
        Some("Internal")
    );
    assert_eq!(
        std::fs::read(root.join(INPUT)).expect("unchanged source"),
        ASSAYS.as_bytes()
    );
    assert_eq!(std::fs::read_dir(&root).expect("read-only").count(), 1);
}

#[test]
fn uncensored_closed_form_and_partial_fit_failures_are_reported_distinctly() {
    let root = fixture(&readings(&["0.0625", "0.125", "0.25"]));
    let value = successful(&root);
    let r = report(&value);
    let mle = fit(r, "mle");
    let log_two = std::f64::consts::LN_2;
    near(mle, "mu_log", -3.0 * log_two, 1e-12);
    near(mle, "sigma_log", log_two * (2.0_f64 / 3.0).sqrt(), 1e-12);
    let ros = fit(r, "ros");
    // Uncensored ROS summary uses the actual measurements, not fitted replacements.
    near(ros, "mean", 7.0 / 48.0, 1e-14);
    near(ros, "sd", (7.0_f64 / 768.0).sqrt(), 1e-14);
    near(ros, "median", 0.125, 1e-14);
    near(ros, "sigma_log", 1.027661547649078, 1e-12);
    std::fs::write(
        root.join(INPUT),
        readings(&["<0.0625", "<0.0625", "<0.0625", "0.125", "0.125"]),
    )
    .expect("tied measured data");
    let partial = successful(&root);
    let r = report(&partial);
    assert_eq!(field(r, &["comparison_state"]).as_str(), Some("partial"));
    assert_eq!(field(r, &["mle", "status"]).as_str(), Some("ok"));
    assert_eq!(field(r, &["ros", "status"]).as_str(), Some("refused"));
    assert_eq!(field(r, &["ros", "fit"]), &Json::Null);
    assert_eq!(
        field(r, &["ros", "refusal", "code"]).as_str(),
        Some("FMINE-REF-207")
    );
    std::fs::write(root.join(INPUT), readings(&["<0.25", "0.125", "0.125"]))
        .expect("no identified fit");
    let result = execute(&root);
    assert_eq!(result.status, "refused");
    assert_eq!(result.payload_json, "null");
    assert!(
        result
            .refusal
            .expect("both fit failures")
            .what
            .contains("both methods refused")
    );
}

#[test]
fn source_edits_change_receipts_and_markdown_explains_physical_model_limits() {
    let root = fixture(ASSAYS);
    let first = successful(&root);
    let mut a = args();
    a.extend(["--format".into(), "markdown".into()]);
    let (envelope, format) = invoke(&a, &root);
    assert_eq!(format, OutputFormat::Markdown);
    let markdown = envelope.rendered_markdown.expect("human report");
    for phrase in [
        "Left-censored assay comparison",
        "Lognormal MLE",
        "Regression on Order Statistics",
        "5 observations: 3 measured, 2 left-censored",
        "not confidence intervals",
        "support above a physical mass fraction of one",
        "SHA-256:",
    ] {
        assert!(markdown.contains(phrase), "missing {phrase}");
    }
    std::fs::write(
        root.join(INPUT),
        format!("{ASSAYS}\n# extra source declaration\n"),
    )
    .expect("source edit");
    let second = successful(&root);
    assert_ne!(
        field(&first, &["payload", "claim", "id"]),
        field(&second, &["payload", "claim", "id"])
    );
    assert_ne!(
        field(&first, &["payload", "receipt_sha256"]),
        field(&second, &["payload", "receipt_sha256"])
    );
    assert_eq!(
        field(report(&first), &["mle"]),
        field(report(&second), &["mle"])
    );
    assert_eq!(
        field(report(&first), &["ros"]),
        field(report(&second), &["ros"])
    );
}

#[test]
fn invalid_sources_refuse_without_dropping_or_substituting_observations() {
    let root = fixture(ASSAYS);
    for source in [
        ASSAYS.replace("unit = \"mass_fraction\"", "unit = \"ppm\""),
        ASSAYS.replace("basis = \"dry\"\n", ""),
        ASSAYS.replace("value = \"<0.125\"", "value = \"DL/2\""),
        ASSAYS.replace("value = \"<0.125\"", "value = \">0.125\""),
        ASSAYS.replace("value = \"<0.125\"", "value = \"<0\""),
        ASSAYS.replace("value = \"<0.125\"", "value = \"<1e-400\""),
        ASSAYS.replace("value = \"0.125\"", "value = \"0\""),
        ASSAYS.replace("value = \"0.125\"", "value = \"1.01\""),
        ASSAYS.replace("value = \"0.125\"", "value = \"NaN\""),
        ASSAYS.replace("sample_id = \"SYN-02\"", "sample_id = \"SYN-01\""),
        ASSAYS.replace("[[observation]]", "[[wrong]]"),
        readings(&["0.125", "<0.125", "<0.25"]),
    ] {
        std::fs::write(root.join(INPUT), source).expect("invalid input");
        let result = execute(&root);
        assert_eq!(result.status, "refused", "{}", result.to_json());
        assert_eq!(result.payload_json, "null");
    }
}

#[test]
fn lost_positive_ros_imputation_never_escapes_as_a_successful_comparison() {
    let root = fixture(&readings(&["1e-300", "1", "<1e-310"]));
    let envelope = execute(&root);
    assert_eq!(envelope.status, "refused", "{}", envelope.to_json());
    assert_eq!(envelope.payload_json, "null");
    let error = envelope
        .refusal
        .expect("both methods exceed representable moments");
    assert_eq!(error.code, "FMINE-REF-207");
    assert!(error.what.contains("ROS") && error.what.contains("underflowed"));

    std::fs::write(root.join(INPUT), readings(&["1e-200", "2e-200", "4e-200"]))
        .expect("representable tiny assay range");
    let value = successful(&root);
    let ros = fit(report(&value), "ros");
    let sd = field(ros, &["sd"]).f64().expect("positive SD");
    assert!((sd / 1e-200 - (7.0_f64 / 3.0).sqrt()).abs() < 1e-14);
}
