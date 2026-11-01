//! DESIGN (BD-345.S4): source-to-command checks using invented registered workbook bytes.
//! The arithmetic expectations below are fixed independently of the candidate's derivation code.
//! The direct-publication controls were moved intact from the CLI to this cross-crate T layer.
#![cfg(test)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, invoke, run};
use fmine_core::hash::sha256;
use fmine_io::xlsx::{MinimalCell, MinimalSheet, MinimalStyles, build_minimal_xlsx_styled};
use fmine_numerics::exact_feas::Rat;
use fmine_public::sources::{load_issuer_profile, registry_path};

static WORKSPACE: AtomicU64 = AtomicU64::new(0);

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|s| (*s).to_string()).collect()
}

fn temporary_workspace(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-public-lint-{label}-{}-{}",
        std::process::id(),
        WORKSPACE.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(&path).expect("temporary workspace");
    path
}

fn no_authority(report: &Envelope) {
    for flag in [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert!(report.to_json().contains(&format!("\"{flag}\":false")));
    }
}

const PUBLIC_RECORD_ISSUER: &str = "publicationtest";

fn public_record_workspace(label: &str, published_at: Option<&str>) -> (PathBuf, PathBuf) {
    let workspace = temporary_workspace(label);
    let synthetic_registry =
        fmine_public::synthetic::write_issuer_workspace(&workspace).expect("invented workbooks");
    let mut registry = std::fs::read_to_string(synthetic_registry)
        .expect("synthetic registry")
        .replace(
            "issuer = \"synthetic\"",
            &format!("issuer = \"{PUBLIC_RECORD_ISSUER}\""),
        )
        .replace(
            "retrieved_utc = \"synthetic\"",
            "retrieved_utc = \"2030-06-01T00:00:00Z\"",
        );
    if let Some(date) = published_at {
        let file = "[[source]]\nfile = \"Synthetic_AISC.xlsx\"\n";
        assert_eq!(registry.matches(file).count(), 1);
        registry = registry.replace(
            file,
            &format!(
                "{file}published_at = \"{date}\"\n\
                 published_at_basis = \"FACT: invented release calendar for CLI regression\"\n"
            ),
        );
    }
    let path = registry_path(&workspace, PUBLIC_RECORD_ISSUER);
    std::fs::write(&path, registry).expect("registered imaginary issuer");
    (workspace, path)
}

fn public_record_component_digest<'a>(payload: &'a str, key: &str) -> &'a str {
    let key = format!("\"{key}\":");
    payload
        .split_once(key.as_str())
        .expect("report component")
        .1
        .split_once("\"report_digest\":\"")
        .expect("component digest")
        .1
        .split('"')
        .next()
        .expect("digest value")
}

#[test]
fn public_record_cut_checks_registered_publication_without_changing_cited_equality() {
    let (workspace, registry) = public_record_workspace("public-record-cited", Some("2030-05-01"));
    let profile = load_issuer_profile(&workspace, PUBLIC_RECORD_ISSUER)
        .expect("registered imaginary issuer")
        .through("2030Q1")
        .expect("selected release");
    let snapshot = fmine_public::run_public_assurance(&profile, &workspace).expect("snapshot");
    let cell = &snapshot.public_claims.cells["Synthetic_AISC.xlsx:Q1 2030!B16"];
    assert_eq!(cell.claim.evidence, fmine_evidence::EvidenceClass::ISSUER);
    let id = cell.claim.claim_id.to_string();
    let draft = format!(
        "Site A Q1 2030 ownership basis unprinted AISC per ounce sold is [$1,250/oz](fmine:{id}).\n\
         PRIVATE_PUBLICATION_DRAFT_PROSE_MUST_NOT_BE_ECHOED.\n"
    );
    let draft_path = workspace.join("publication.md");
    std::fs::write(&draft_path, &draft).expect("draft");
    let arguments = |cut| {
        args(&[
            "lint",
            "publication.md",
            "--issuer",
            PUBLIC_RECORD_ISSUER,
            "--through",
            "2030Q1",
            "--as-of",
            cut,
        ])
    };
    let before = invoke(&arguments("2030-04-30"), &workspace).0;
    let after_args = arguments("2030-05-03");
    let after = invoke(&after_args, &workspace).0;
    for report in [&before, &after] {
        assert_eq!(report.exit_code, 0, "{}", report.to_json());
        assert!(report.payload_json.contains("\"verdict\":\"verified\""));
        assert!(report.payload_json.contains(&id));
        no_authority(report);
        assert!(!report.to_json().contains("PRIVATE_PUBLICATION_DRAFT_PROSE"));
    }
    assert!(
        before
            .payload_json
            .contains("\"status\":\"not_in_public_record\"")
    );
    assert!(before.payload_json.contains("\"admission\":\"after_cut\""));
    assert!(after.payload_json.contains("\"status\":\"disclosed\""));
    assert!(
        after
            .payload_json
            .contains(&format!("\"first_registered_disclosure\":\"{id}\""))
    );
    assert_eq!(before.certificate, after.certificate);
    assert_eq!(
        public_record_component_digest(&before.payload_json, "lint"),
        public_record_component_digest(&after.payload_json, "lint"),
    );
    assert_ne!(
        public_record_component_digest(&before.payload_json, "public_record"),
        public_record_component_digest(&after.payload_json, "public_record"),
    );
    let original_registry = std::fs::read_to_string(&registry).expect("registry");
    assert_eq!(
        original_registry
            .matches("published_at = \"2030-05-01\"")
            .count(),
        1
    );
    std::fs::write(
        &registry,
        original_registry.replace(
            "published_at = \"2030-05-01\"",
            "published_at = \"2030-05-02\"",
        ),
    )
    .expect("correct registered date");
    let revised = invoke(&after_args, &workspace).0;
    assert_eq!(revised.exit_code, 0, "{}", revised.to_json());
    assert!(revised.payload_json.contains("\"status\":\"disclosed\""));
    assert!(revised.payload_json.contains(&id));
    assert_eq!(after.certificate, revised.certificate);
    assert_eq!(
        public_record_component_digest(&after.payload_json, "lint"),
        public_record_component_digest(&revised.payload_json, "lint"),
    );
    assert_ne!(
        public_record_component_digest(&after.payload_json, "public_record"),
        public_record_component_digest(&revised.payload_json, "public_record"),
    );
    assert_eq!(revised, invoke(&after_args, &workspace).0);
    let mut markdown = after_args;
    markdown.extend(args(&["--format", "markdown"]));
    let (text, exit) = run(&markdown, &workspace);
    assert_eq!(exit, 0);
    assert!(text.contains("Public record at 2030-05-03"));
    assert!(text.contains("Synthetic_AISC.xlsx:Q1 2030!B16"));
    assert!(text.contains("2030-05-02"));
    assert!(!text.contains("PRIVATE_PUBLICATION_DRAFT_PROSE"));
    assert_eq!(
        std::fs::read_to_string(draft_path).expect("draft retained"),
        draft
    );
}

#[test]
fn public_record_discovery_never_uses_retrieval_as_a_publication_date() {
    let (workspace, _) = public_record_workspace("public-record-undated", None);
    let draft = "Site A Q1 2030 ownership basis unprinted AISC per ounce sold is $1,250/oz.\n\
                 PRIVATE_UNDATED_DRAFT_PROSE_MUST_NOT_BE_ECHOED.\n";
    let path = workspace.join("undated.md");
    std::fs::write(&path, draft).expect("uncited draft");
    let arguments = args(&[
        "lint",
        "undated.md",
        "--issuer",
        PUBLIC_RECORD_ISSUER,
        "--through",
        "2030Q1",
        "--as-of",
        "2030-06-02",
        "--discover",
    ]);
    let result = invoke(&arguments, &workspace).0;
    assert_eq!(result.exit_code, 0, "{}", result.to_json());
    assert!(result.payload_json.contains("\"verdict\":\"verified\""));
    assert!(
        result
            .payload_json
            .contains("\"status\":\"publication_date_unknown\"")
    );
    assert!(
        result
            .payload_json
            .contains("\"known_available_by_retrieval\":\"2030-06-01\"")
    );
    assert!(result.payload_json.contains("\"published_at\":null"));
    assert!(
        result
            .payload_json
            .contains("\"first_registered_disclosure\":null")
    );
    assert!(!result.payload_json.contains("\"status\":\"disclosed\""));
    assert!(!result.to_json().contains("PRIVATE_UNDATED_DRAFT_PROSE"));
    assert_eq!(result, invoke(&arguments, &workspace).0);
    assert_eq!(
        std::fs::read_to_string(path).expect("unchanged draft"),
        draft
    );
}

#[test]
fn public_record_discovery_rechecks_source_bytes_before_reusing_a_disclosure_result() {
    let (workspace, _) = public_record_workspace("public-record-tamper", Some("2030-05-01"));
    let draft = "Site A Q1 2030 ownership basis unprinted AISC per ounce sold is $1,250/oz.\n";
    let draft_path = workspace.join("discovery.md");
    std::fs::write(&draft_path, draft).expect("draft");
    let arguments = args(&[
        "lint",
        "discovery.md",
        "--issuer",
        PUBLIC_RECORD_ISSUER,
        "--as-of",
        "2030-06-02",
        "--discover",
    ]);
    let admitted = invoke(&arguments, &workspace).0;
    assert_eq!(admitted.exit_code, 0, "{}", admitted.to_json());
    assert!(admitted.payload_json.contains("\"status\":\"disclosed\""));
    assert!(
        admitted
            .payload_json
            .contains("Synthetic_AISC.xlsx:Q1 2030!B16")
    );
    let source = workspace.join("corpus/Synthetic_AISC.xlsx");
    let mut bytes = std::fs::read(&source).expect("registered workbook");
    bytes.push(0);
    std::fs::write(&source, bytes).expect("alter fixture bytes");
    let refused = invoke(&arguments, &workspace).0;
    assert_eq!(refused.status, "refused");
    assert_eq!(
        refused.refusal.as_ref().expect("integrity refusal").code,
        "FMINE-REF-505"
    );
    assert!(!refused.to_json().contains("\"status\":\"disclosed\""));
    assert_eq!(
        std::fs::read_to_string(draft_path).expect("unchanged draft"),
        draft
    );
}

const DERIVATION_ISSUER: &str = "derivationtest";
const PRODUCTION: &str = "Gold produced (000s oz 100%)";
const ATTRIBUTABLE: &str = "Gold produced (000s oz attributable)";
const SALES: &str = "Gold sold (000s oz attributable)";

fn derivation_workspace() -> PathBuf {
    let root = temporary_workspace("derivations");
    for directory in ["corpus", "registries/sources"] {
        std::fs::create_dir_all(root.join(directory)).expect("fixture directory");
    }
    std::fs::write(
        registry_path(&root, DERIVATION_ISSUER),
        format!(
            "schema = \"fmine.sources.v1\"\nissuer = \"{DERIVATION_ISSUER}\"\n\
             corpus_dir = \"corpus\"\nretrieved_utc = \"2031-03-01T00:00:00Z\"\n\
             url_prefix = \"https://example.invalid/\"\n\
             [[subject]]\nname = \"Example Mine\"\n[[subject]]\nname = \"Other Mine\"\n"
        ),
    )
    .expect("invented source registry");
    root
}

fn number(raw: &str) -> MinimalCell {
    MinimalCell::Styled {
        raw: raw.into(),
        xf: 1,
    }
}

fn summary(period: &str, subject: &str, metric: &str, value: MinimalCell) -> MinimalSheet {
    let (name, months, end) = match period {
        "Q1" => ("QTD", "three", "March 31"),
        "Q2" => ("QTD", "three", "June 30"),
        "Q3" => ("QTD", "three", "September 30"),
        "Q4" => ("QTD", "three", "December 31"),
        "H1" => ("YTD", "six", "June 30"),
        "9M" => ("YTD", "nine", "September 30"),
        "FY" => ("YTD", "twelve", "December 31"),
        other => panic!("unsupported synthetic period {other}"),
    };
    let text = |s: &str| MinimalCell::Text(s.into());
    MinimalSheet {
        name: name.into(),
        hidden: false,
        cells: vec![
            (
                "A1".into(),
                text(&format!("For the {months} months ended {end},")),
            ),
            ("B1".into(), MinimalCell::Number(2030.0)),
            ("B2".into(), text(metric)),
            ("A3".into(), text(subject)),
            ("B3".into(), value),
        ],
    }
}

fn source(
    root: &Path,
    file: &str,
    release: &str,
    date: Option<&str>,
    precision: &str,
    sheets: &[MinimalSheet],
) -> String {
    let bytes = build_minimal_xlsx_styled(
        sheets,
        Some(&MinimalStyles {
            num_fmts: vec![(164, precision.into())],
            xfs: vec![0, 164],
        }),
    );
    let hash = sha256(&bytes).to_hex();
    std::fs::write(root.join("corpus").join(file), &bytes).expect("invented source bytes");
    let path = registry_path(root, DERIVATION_ISSUER);
    let mut registry = std::fs::read_to_string(&path).expect("fixture registry");
    write!(
        registry,
        "\n[[source]]\nfile = \"{file}\"\nkind = \"mine_stats\"\nquarter = \"{release}\"\n\
         path = \"{file}\"\nurl_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{hash}\"\n",
        bytes.len(),
    )
    .expect("source row");
    if let Some(date) = date {
        writeln!(
            registry,
            "published_at = \"{date}\"\npublished_at_basis = \"FACT: invented test publication\""
        )
        .expect("publication metadata");
    }
    std::fs::write(path, registry).expect("registered invented source");
    hash
}

fn pair(root: &Path, prior: &str, cumulative: &str, precision: &str) -> [String; 2] {
    [
        source(
            root,
            "Prior.xlsx",
            "2030Q1",
            Some("2030-05-01"),
            precision,
            &[summary(
                "Q1",
                "Example Mine (60%)",
                PRODUCTION,
                number(prior),
            )],
        ),
        source(
            root,
            "Cumulative.xlsx",
            "2030Q2",
            Some("2030-08-01"),
            precision,
            &[summary(
                "H1",
                "Example Mine (60%)",
                PRODUCTION,
                number(cumulative),
            )],
        ),
    ]
}

fn assess(root: &Path, draft: &str, cut: &str) -> Envelope {
    std::fs::write(root.join("derived.md"), draft).expect("draft");
    let result = invoke(
        &args(&[
            "lint",
            "derived.md",
            "--issuer",
            DERIVATION_ISSUER,
            "--discover",
            "--as-of",
            cut,
        ]),
        root,
    )
    .0;
    assert_eq!(result.exit_code, 0, "{}", result.to_json());
    no_authority(&result);
    result
}

fn production(quarter: u8, amount: &str) -> String {
    format!("Example Mine Q{quarter} 2030 100% basis gold production is {amount} koz.")
}

fn derived(report: &Envelope) -> &str {
    let body = report
        .payload_json
        .split_once("\"derivation\":")
        .expect("derivation field")
        .1;
    assert!(body.starts_with('{'), "{}", report.to_json());
    body
}

fn string_field<'a>(json: &'a str, name: &str) -> &'a str {
    json.split_once(&format!("\"{name}\":\""))
        .expect("named field")
        .1
        .split('"')
        .next()
        .expect("string field")
}

fn rational(text: &str) -> Rat {
    if let Some((num, den)) = text.split_once('/') {
        Rat::new(
            num.parse().expect("numerator"),
            den.parse().expect("denominator"),
        )
        .expect("exact rational")
    } else {
        Rat::from_decimal_str(text).expect("exact decimal")
    }
}

fn pair_interval(report: &Envelope) -> (Rat, Rat) {
    let interval = derived(report)
        .split_once("\"pairs\":[")
        .expect("source pairs")
        .1
        .split_once("\"interval\":")
        .expect("interval")
        .1;
    (
        rational(string_field(interval, "lower")),
        rational(string_field(interval, "upper")),
    )
}

#[test]
fn additive_quarters_use_displayed_enclosures_and_retain_both_dated_sources() {
    for (quarter, prior_period, current_period, prior, cumulative, expected, rule) in [
        (2, "Q1", "H1", "100.01", "200.04", 100, "H1-Q1"),
        (3, "H1", "9M", "200.01", "325.04", 125, "9M-H1"),
        (4, "9M", "FY", "325.01", "450.04", 125, "FY-9M"),
    ] {
        let root = derivation_workspace();
        let prior_file = source(
            &root,
            "Prior.xlsx",
            &format!("2030Q{}", quarter - 1),
            Some("2031-02-01"),
            "0.0",
            &[summary(
                prior_period,
                "Example Mine (60%)",
                PRODUCTION,
                number(prior),
            )],
        );
        let cumulative_file = source(
            &root,
            "Cumulative.xlsx",
            &format!("2030Q{quarter}"),
            Some("2031-02-02"),
            "0.0",
            &[summary(
                current_period,
                "Example Mine (60%)",
                PRODUCTION,
                number(cumulative),
            )],
        );
        let draft = production(quarter, &expected.to_string());
        let report = assess(&root, &draft, "2031-02-02");
        assert!(
            report.payload_json.contains("\"status\":\"derivable\""),
            "{}",
            report.to_json()
        );
        assert_eq!(string_field(derived(&report), "status"), "derived");
        assert_eq!(string_field(derived(&report), "rule"), rule);
        assert_eq!(
            pair_interval(&report),
            (
                Rat::new(10 * expected - 1, 10).unwrap(),
                Rat::new(10 * expected + 1, 10).unwrap()
            ),
        );
        for expected in [
            &prior_file,
            &cumulative_file,
            "Prior.xlsx",
            "Cumulative.xlsx",
            "2031-02-01",
            "2031-02-02",
            "FACT: invented test publication",
        ] {
            assert!(
                derived(&report).contains(expected),
                "missing {expected}: {}",
                report.to_json()
            );
        }
        assert!(report.payload_json.contains("\"verdict\":\"uncited\""));
        assert!(!report.payload_json.contains("\"status\":\"disclosed\""));
        assert!(derived(&report).contains("new information"));
        assert_eq!(report, assess(&root, &draft, "2031-02-02"));
        let (markdown, exit) = run(
            &args(&[
                "lint",
                "derived.md",
                "--issuer",
                DERIVATION_ISSUER,
                "--discover",
                "--as-of",
                "2031-02-02",
                "--format",
                "markdown",
            ]),
            &root,
        );
        assert_eq!(exit, 0);
        for expected in [rule, "Prior.xlsx", "Cumulative.xlsx", "derivable"] {
            assert!(markdown.contains(expected), "{markdown}");
        }
    }
}

#[test]
fn derivation_obeys_both_publication_dates_and_never_uses_retrieval_as_publication() {
    let root = derivation_workspace();
    pair(&root, "100.01", "200.04", "0.0");
    let draft = production(2, "100");
    let before = assess(&root, &draft, "2030-07-31");
    assert!(!before.payload_json.contains("\"status\":\"derivable\""));
    assert!(derived(&before).contains("\"admission\":\"after_cut\""));
    let after = assess(&root, &draft, "2030-08-01");
    assert!(after.payload_json.contains("\"status\":\"derivable\""));
    let path = registry_path(&root, DERIVATION_ISSUER);
    let registry = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        registry.replace(
            "published_at = \"2030-08-01\"",
            "published_at = \"2030-08-02\"",
        ),
    )
    .unwrap();
    let revised = assess(&root, &draft, "2030-08-01");
    assert!(!revised.payload_json.contains("\"status\":\"derivable\""));
    assert_eq!(after.certificate, revised.certificate);
    assert_eq!(
        public_record_component_digest(&after.payload_json, "lint"),
        public_record_component_digest(&revised.payload_json, "lint"),
    );
    assert_ne!(
        public_record_component_digest(&after.payload_json, "public_record"),
        public_record_component_digest(&revised.payload_json, "public_record"),
    );
    let unknown = derivation_workspace();
    source(
        &unknown,
        "Prior.xlsx",
        "2030Q1",
        None,
        "0.0",
        &[summary(
            "Q1",
            "Example Mine (60%)",
            PRODUCTION,
            number("100"),
        )],
    );
    source(
        &unknown,
        "Cumulative.xlsx",
        "2030Q2",
        Some("2030-08-01"),
        "0.0",
        &[summary(
            "H1",
            "Example Mine (60%)",
            PRODUCTION,
            number("200"),
        )],
    );
    let report = assess(&unknown, &draft, "2031-03-02");
    assert!(!report.payload_json.contains("\"status\":\"derivable\""));
    assert!(derived(&report).contains("\"admission\":\"date_unknown\""));
}

#[test]
fn rounded_cumulative_inputs_do_not_support_a_more_precise_quarter() {
    let root = derivation_workspace();
    pair(&root, "100", "200", "0");
    let report = assess(&root, &production(2, "100"), "2030-08-01");
    assert_eq!(
        string_field(derived(&report), "status"),
        "precision_not_established"
    );
    assert_eq!(
        pair_interval(&report),
        (Rat::from_integer(99), Rat::from_integer(101))
    );
    assert!(!report.payload_json.contains("\"status\":\"derivable\""));
    let wrong = assess(&root, &production(2, "90"), "2030-08-01");
    assert_eq!(string_field(derived(&wrong), "status"), "contradicted");
    assert!(!wrong.payload_json.contains("\"status\":\"derivable\""));
}

#[test]
fn competing_public_vintages_are_retained_instead_of_selected_by_the_draft_value() {
    let root = derivation_workspace();
    pair(&root, "100", "200", "0.0");
    source(
        &root,
        "Comparative.xlsx",
        "2030Q3",
        Some("2030-11-01"),
        "0.0",
        &[summary(
            "H1",
            "Example Mine (60%)",
            PRODUCTION,
            number("220"),
        )],
    );
    let draft = production(2, "100");
    let before = assess(&root, &draft, "2030-08-01");
    assert!(before.payload_json.contains("\"status\":\"derivable\""));
    let after = assess(&root, &draft, "2030-11-01");
    assert_eq!(
        string_field(derived(&after), "status"),
        "conflicting_derivations"
    );
    assert!(!after.payload_json.contains("\"status\":\"derivable\""));
    for file in ["Prior.xlsx", "Cumulative.xlsx", "Comparative.xlsx"] {
        assert!(derived(&after).contains(file));
    }
    assert_eq!(derived(&after).matches("\"cumulative_claim\":").count(), 2);
}

#[test]
fn printed_zero_is_an_operand_and_a_dash_cannot_be_used_as_zero() {
    for (prior, precision, available) in [
        (number("0"), "0.0", true),
        (MinimalCell::Text("—".into()), "0.0", false),
        (number("0"), "0.0;-0.0;\"—\"", false),
    ] {
        let root = derivation_workspace();
        source(
            &root,
            "Prior.xlsx",
            "2030Q1",
            Some("2030-05-01"),
            precision,
            &[summary("Q1", "Example Mine (60%)", PRODUCTION, prior)],
        );
        source(
            &root,
            "Cumulative.xlsx",
            "2030Q2",
            Some("2030-08-01"),
            "0.0",
            &[summary(
                "H1",
                "Example Mine (60%)",
                PRODUCTION,
                number("100"),
            )],
        );
        let report = assess(&root, &production(2, "100"), "2030-08-01");
        assert_eq!(
            report.payload_json.contains("\"status\":\"derivable\""),
            available,
            "{}",
            report.to_json()
        );
    }
}

#[test]
fn only_compatible_additive_metrics_and_explicit_shares_are_combined() {
    for (prior_metric, current_metric, prior_subject, current_subject, sentence, available) in [
        (
            SALES,
            SALES,
            "Example Mine (60%)",
            "Example Mine (60%)",
            "Example Mine Q2 2030 attributable gold sold is 100 koz.",
            true,
        ),
        (
            "Copper produced (thousands of tonnes 100%)",
            "Copper produced (thousands of tonnes 100%)",
            "Example Mine (60%)",
            "Example Mine (60%)",
            "Example Mine Q2 2030 100% basis copper production is 100 kt.",
            true,
        ),
        (
            PRODUCTION,
            SALES,
            "Example Mine (60%)",
            "Example Mine (60%)",
            "Example Mine Q2 2030 100% basis gold production is 100 koz.",
            false,
        ),
        (
            ATTRIBUTABLE,
            ATTRIBUTABLE,
            "Example Mine (60%)",
            "Example Mine (80%)",
            "Example Mine Q2 2030 attributable gold production is 100 koz.",
            false,
        ),
        (
            ATTRIBUTABLE,
            ATTRIBUTABLE,
            "Example Mine",
            "Example Mine (60%)",
            "Example Mine Q2 2030 attributable gold production is 100 koz.",
            false,
        ),
        (
            PRODUCTION,
            PRODUCTION,
            "Example Mine (60%)",
            "Example Mine (60%)",
            "Example Mine Q2 2030 100% basis gold production is 100 kt.",
            false,
        ),
        (
            PRODUCTION,
            PRODUCTION,
            "Example Mine (60%)",
            "Example Mine (60%)",
            "Example Mine Q2 2030 gold production is 100 koz.",
            false,
        ),
        (
            PRODUCTION,
            "Copper produced (thousands of tonnes 100%)",
            "Example Mine (60%)",
            "Example Mine (60%)",
            "Example Mine Q2 2030 100% basis gold production is 100 koz.",
            false,
        ),
        (
            PRODUCTION,
            PRODUCTION,
            "Other Mine (60%)",
            "Example Mine (60%)",
            "Example Mine Q2 2030 100% basis gold production is 100 koz.",
            false,
        ),
        (
            "All-in sustaining costs ($/oz)",
            "All-in sustaining costs ($/oz)",
            "Example Mine (60%)",
            "Example Mine (60%)",
            "Example Mine Q2 2030 attributable AISC per ounce sold is $100/oz.",
            false,
        ),
    ] {
        let root = derivation_workspace();
        source(
            &root,
            "Prior.xlsx",
            "2030Q1",
            Some("2030-05-01"),
            "0.0",
            &[summary("Q1", prior_subject, prior_metric, number("100"))],
        );
        source(
            &root,
            "Cumulative.xlsx",
            "2030Q2",
            Some("2030-08-01"),
            "0.0",
            &[summary(
                "H1",
                current_subject,
                current_metric,
                number("200"),
            )],
        );
        let report = assess(&root, sentence, "2030-08-01");
        assert_eq!(
            report.payload_json.contains("\"status\":\"derivable\""),
            available,
            "{sentence}: {}",
            report.to_json()
        );
    }
}

#[test]
fn direct_contradiction_is_not_overridden_by_compatible_derived_arithmetic() {
    let root = derivation_workspace();
    pair(&root, "100", "200", "0.0");
    source(
        &root,
        "Direct.xlsx",
        "2030Q2",
        Some("2030-08-01"),
        "0.0",
        &[summary(
            "Q2",
            "Example Mine (60%)",
            PRODUCTION,
            number("90"),
        )],
    );
    let dated = assess(&root, &production(2, "100"), "2030-08-01");
    source(
        &root,
        "UndatedDirect.xlsx",
        "2030Q2",
        None,
        "0.0",
        &[summary(
            "Q2",
            "Example Mine (60%)",
            PRODUCTION,
            number("100"),
        )],
    );
    let mixed = assess(&root, &production(2, "100"), "2030-08-01");
    assert!(mixed.payload_json.contains("UndatedDirect.xlsx"));
    assert!(
        mixed
            .payload_json
            .contains("\"admission\":\"date_unknown\"")
    );
    for report in [dated, mixed] {
        assert!(
            report
                .payload_json
                .contains("\"status\":\"contradicts_disclosed\""),
            "{}",
            report.to_json()
        );
        assert!(!report.payload_json.contains("\"status\":\"derivable\""));
        assert_eq!(string_field(derived(&report), "status"), "derived");
    }
}

fn quarter_sum_report(report: &Envelope) -> &str {
    let body = report
        .payload_json
        .split_once("\"quarter_sum\":")
        .expect("quarter-sum field")
        .1;
    assert!(body.starts_with('{'), "{}", report.to_json());
    body
}

fn sum_source(
    root: &Path,
    quarter: u8,
    subject: &str,
    metric: &str,
    value: MinimalCell,
    precision: &str,
    date: Option<&str>,
) -> String {
    source(
        root,
        &format!("Quarter{quarter}.xlsx"),
        &format!("2030Q{quarter}"),
        date,
        precision,
        &[summary(&format!("Q{quarter}"), subject, metric, value)],
    )
}

#[test]
fn complete_half_and_year_sums_keep_exact_enclosures_and_canonical_parent_chains() {
    for (n, period, amount, lower, upper) in [
        (2, "H1 2030", "300", (2999, 10), (3001, 10)),
        (4, "FY2030", "1000", (4999, 5), (5001, 5)),
    ] {
        let root = derivation_workspace();
        let hashes: Vec<_> = (1..=n)
            .map(|q| {
                sum_source(
                    &root,
                    q,
                    "Example Mine (60%)",
                    PRODUCTION,
                    number(&format!("{}.01", 100 * u16::from(q))),
                    "0.0",
                    Some("2031-02-01"),
                )
            })
            .collect();
        let draft = format!("Example Mine {period} 100% basis gold production is {amount} koz.");
        let report = assess(&root, &draft, "2031-02-01");
        let sum = quarter_sum_report(&report);
        assert_eq!(
            string_field(sum, "status"),
            "derived",
            "{}",
            report.to_json()
        );
        assert!(report.payload_json.contains("\"status\":\"derivable\""));
        assert_eq!(string_field(sum, "rule"), "public-disclosure.quarter-sum/1");
        let derived = sum.split_once("\"derived_claim\":").unwrap().1;
        assert!(derived.contains(&format!(
            "\"lo\":{{\"num\":\"{}\",\"den\":\"{}\"}}",
            lower.0, lower.1
        )));
        assert!(derived.contains(&format!(
            "\"hi\":{{\"num\":\"{}\",\"den\":\"{}\"}}",
            upper.0, upper.1
        )));
        assert!(derived.contains(&format!(
            "\"evidence\":\"{:?}\"",
            fmine_evidence::EvidenceClass::ISSUER
        )));
        let encoded = string_field(derived, "canonical_body_hex");
        let bytes: Vec<_> = encoded
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(
            fmine_core::ObjectId::from_canonical_bytes(fmine_core::PrefixKind::Claim, &bytes)
                .to_string(),
            string_field(derived, "claim_id")
        );
        let profile = load_issuer_profile(&root, DERIVATION_ISSUER).unwrap();
        let snapshot = fmine_public::run_public_assurance(&profile, &root).unwrap();
        for c in snapshot
            .public_claims
            .cells
            .values()
            .filter(|c| c.key.line.label == "summary:au_prod_100_koz")
        {
            assert!(sum.contains(&format!("\"parents\":[\"{}\"]", c.claim.claim_id)));
        }
        for hash in hashes {
            assert!(sum.contains(&hash));
        }
        assert!(sum.contains("non-obvious derived figure"));
        assert_eq!(report, assess(&root, &draft, "2031-02-01"));
        let (markdown, code) = run(
            &args(&[
                "lint",
                "derived.md",
                "--issuer",
                DERIVATION_ISSUER,
                "--discover",
                "--as-of",
                "2031-02-01",
                "--format",
                "markdown",
            ]),
            &root,
        );
        assert_eq!(code, 0);
        for expected in [
            "Quarter sum",
            "Quarter1.xlsx",
            "Derived Claim",
            "public-disclosure.quarter-sum/1",
        ] {
            assert!(markdown.contains(expected), "{markdown}");
        }
    }
}

#[test]
fn quarter_sums_require_every_dated_quarter_without_zero_or_future_fallbacks() {
    for missing in ["absent", "future", "undated", "nil"] {
        let root = derivation_workspace();
        for q in 1..=3 {
            sum_source(
                &root,
                q,
                "Example Mine (60%)",
                PRODUCTION,
                number("100"),
                "0.0",
                Some("2031-02-01"),
            );
        }
        if missing != "absent" {
            sum_source(
                &root,
                4,
                "Example Mine (60%)",
                PRODUCTION,
                if missing == "nil" {
                    MinimalCell::Text("—".into())
                } else {
                    number("100")
                },
                "0.0",
                match missing {
                    "future" => Some("2031-02-02"),
                    "undated" => None,
                    _ => Some("2031-02-01"),
                },
            );
        }
        let draft = "Example Mine FY2030 100% basis gold production is 400 koz.";
        let report = assess(&root, draft, "2031-02-01");
        assert_eq!(
            string_field(quarter_sum_report(&report), "status"),
            "not_available",
            "{missing}: {}",
            report.to_json()
        );
        assert!(!report.payload_json.contains("\"status\":\"derivable\""));
        if missing != "absent" {
            assert!(quarter_sum_report(&report).contains("Quarter4.xlsx"));
        }
        if missing == "future" {
            assert!(quarter_sum_report(&report).contains("\"admission\":\"after_cut\""));
            let later = assess(&root, draft, "2031-02-02");
            assert_eq!(
                string_field(quarter_sum_report(&later), "status"),
                "derived"
            );
        }
        if missing == "undated" {
            assert!(quarter_sum_report(&report).contains("\"admission\":\"date_unknown\""));
        }
    }
}

#[test]
fn quarter_sum_precision_and_competing_vintages_cannot_be_selected_by_the_amount() {
    let root = derivation_workspace();
    for q in 1..=2 {
        sum_source(
            &root,
            q,
            "Example Mine (60%)",
            PRODUCTION,
            number("100"),
            "0",
            Some("2030-08-01"),
        );
    }
    let draft = "Example Mine H1 2030 100% basis gold production is 200 koz.";
    let precise = assess(&root, draft, "2030-08-01");
    assert_eq!(
        string_field(quarter_sum_report(&precise), "status"),
        "precision_not_established"
    );
    assert!(!precise.payload_json.contains("\"status\":\"derivable\""));
    let wider = assess(&root, &draft.replace("200 koz", "0.20k koz"), "2030-08-01");
    assert_eq!(
        string_field(quarter_sum_report(&wider), "status"),
        "derived"
    );
    let wrong = assess(&root, &draft.replace("200 koz", "300 koz"), "2030-08-01");
    assert_eq!(
        string_field(quarter_sum_report(&wrong), "status"),
        "contradicted"
    );
    source(
        &root,
        "CompetingQuarter.xlsx",
        "2031Q2",
        Some("2031-08-01"),
        "0",
        &[summary(
            "Q2",
            "Example Mine (60%)",
            PRODUCTION,
            number("200"),
        )],
    );
    let prior = assess(&root, &draft.replace("200 koz", "0.20k koz"), "2031-07-31");
    assert_eq!(
        string_field(quarter_sum_report(&prior), "status"),
        "derived"
    );
    for amount in ["0.20k", "0.30k"] {
        let report = assess(
            &root,
            &draft.replace("200 koz", &format!("{amount} koz")),
            "2031-08-01",
        );
        assert_eq!(
            string_field(quarter_sum_report(&report), "status"),
            "ambiguous"
        );
        assert!(!report.payload_json.contains("\"status\":\"derivable\""));
        for source in ["Quarter1.xlsx", "Quarter2.xlsx", "CompetingQuarter.xlsx"] {
            assert!(quarter_sum_report(&report).contains(source));
        }
    }
}

#[test]
fn quarter_sums_support_copper_and_sales_but_refuse_mixed_shares_metrics_and_units() {
    for (metric1, metric2, subject2, prose, available) in [
        (
            "Copper produced (thousands of tonnes 100%)",
            "Copper produced (thousands of tonnes 100%)",
            "Example Mine (60%)",
            "Example Mine H1 2030 100% basis copper production is 200 kt.",
            true,
        ),
        (
            SALES,
            SALES,
            "Example Mine (60%)",
            "Example Mine H1 2030 attributable gold sold is 200 koz.",
            true,
        ),
        (
            ATTRIBUTABLE,
            ATTRIBUTABLE,
            "Example Mine (80%)",
            "Example Mine H1 2030 attributable gold production is 200 koz.",
            false,
        ),
        (
            PRODUCTION,
            SALES,
            "Example Mine (60%)",
            "Example Mine H1 2030 100% basis gold production is 200 koz.",
            false,
        ),
        (
            PRODUCTION,
            PRODUCTION,
            "Other Mine (60%)",
            "Example Mine H1 2030 100% basis gold production is 200 koz.",
            false,
        ),
        (
            PRODUCTION,
            PRODUCTION,
            "Example Mine (60%)",
            "Example Mine H1 2030 100% basis gold production is 200 kt.",
            false,
        ),
        (
            "AISC ($/oz)",
            "AISC ($/oz)",
            "Example Mine (60%)",
            "Example Mine H1 2030 attributable AISC per ounce sold is $200/oz.",
            false,
        ),
    ] {
        let root = derivation_workspace();
        sum_source(
            &root,
            1,
            "Example Mine (60%)",
            metric1,
            number("100"),
            "0.0",
            Some("2030-08-01"),
        );
        sum_source(
            &root,
            2,
            subject2,
            metric2,
            number("100"),
            "0.0",
            Some("2030-08-01"),
        );
        let report = assess(&root, prose, "2030-08-01");
        assert_eq!(
            report.payload_json.contains("\"status\":\"derivable\""),
            available,
            "{prose}: {}",
            report.to_json()
        );
        if metric1 == ATTRIBUTABLE {
            assert!(quarter_sum_report(&report).contains("FMINE-REF-105"));
        }
    }
}

#[test]
fn direct_disclosures_and_contradictions_keep_priority_over_quarter_sum_findings() {
    for (quarter_value, cumulative, expected, sum_status) in [
        ("100", "300", "disclosed", "contradicted"),
        ("150", "500", "contradicts_disclosed", "derived"),
    ] {
        let root = derivation_workspace();
        for q in 1..=2 {
            sum_source(
                &root,
                q,
                "Example Mine (60%)",
                PRODUCTION,
                number(quarter_value),
                "0.0",
                Some("2030-08-01"),
            );
        }
        source(
            &root,
            "DisclosedHalf.xlsx",
            "2030Q3",
            Some("2030-09-01"),
            "0.0",
            &[summary(
                "H1",
                "Example Mine (60%)",
                PRODUCTION,
                number(cumulative),
            )],
        );
        let report = assess(
            &root,
            "Example Mine H1 2030 100% basis gold production is 300 koz.",
            "2030-09-01",
        );
        let public = report
            .payload_json
            .split_once("\"public_record\":")
            .unwrap()
            .1;
        assert_eq!(string_field(public, "status"), expected);
        assert_eq!(
            string_field(quarter_sum_report(&report), "status"),
            sum_status
        );
    }
}

#[test]
fn regional_subtraction_requires_the_same_registered_membership_at_both_releases() {
    for (membership, prior_release, available) in [
        ("stable", "2030Q1", true),
        ("changed", "2030Q1", false),
        ("unregistered", "2030Q2", false),
    ] {
        let root = derivation_workspace();
        let path = registry_path(&root, DERIVATION_ISSUER);
        let mut registry = std::fs::read_to_string(&path).unwrap();
        registry.push_str("\n[[subject]]\nname = \"Region North\"\nkind = \"region\"\n");
        if membership != "unregistered" {
            registry.push_str(
                "[[region_member]]\nregion = \"Region North\"\nmember = \"Example Mine\"\nfirst = \"2030Q1\"\n",
            );
        }
        if membership == "changed" {
            registry.push_str(
                "[[region_member]]\nregion = \"Region North\"\nmember = \"Other Mine\"\nfirst = \"2030Q2\"\n",
            );
        }
        std::fs::write(path, registry).unwrap();
        source(
            &root,
            "Prior.xlsx",
            prior_release,
            Some("2030-05-01"),
            "0.0",
            &[summary("Q1", "Region North", PRODUCTION, number("100"))],
        );
        source(
            &root,
            "Cumulative.xlsx",
            "2030Q2",
            Some("2030-08-01"),
            "0.0",
            &[summary("H1", "Region North", PRODUCTION, number("200"))],
        );
        let report = assess(
            &root,
            "Region North Q2 2030 100% basis gold production is 100 koz.",
            "2030-08-01",
        );
        assert_eq!(
            report.payload_json.contains("\"status\":\"derivable\""),
            available,
            "{membership}: {}",
            report.to_json()
        );
    }
}
