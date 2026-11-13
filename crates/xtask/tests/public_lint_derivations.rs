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
fn source_retrieval_override_reaches_assurance_trace_and_public_record() {
    let (workspace, registry) = public_record_workspace("public-record-source-retrieval", None);
    let original = std::fs::read_to_string(&registry).expect("registry");
    let marker = "[[source]]\nfile = \"Synthetic_AISC.xlsx\"\n";
    assert_eq!(original.matches(marker).count(), 1);
    let override_time = "2030-06-02T23:59:59Z/2030-06-03T00:00:01Z";
    std::fs::write(
        &registry,
        original.replace(
            marker,
            &format!("{marker}retrieved_utc = \"{override_time}\"\n"),
        ),
    )
    .expect("source-specific retrieval");
    let profile = load_issuer_profile(&workspace, PUBLIC_RECORD_ISSUER)
        .expect("registry with override")
        .through("2030Q1")
        .expect("selected quarter");
    let snapshot = fmine_public::run_public_assurance(&profile, &workspace).expect("snapshot");
    assert_eq!(snapshot.retrieved_utc, "2030-06-01T00:00:00Z");
    assert_eq!(
        snapshot.source_retrievals["Synthetic_AISC.xlsx"],
        override_time
    );
    assert!(
        snapshot
            .to_json()
            .contains(&format!("\"retrieved_utc\":\"{override_time}\""))
    );
    let cell = &snapshot.public_claims.cells["Synthetic_AISC.xlsx:Q1 2030!B16"];
    let trace = fmine_public::key::inspect::trace_json(cell, &profile);
    assert!(trace.contains(&format!("\"retrieved_utc\":\"{override_time}\"")));
    std::fs::write(
        workspace.join("retrieval.md"),
        "Site A Q1 2030 ownership basis unprinted AISC per ounce sold is $1,250/oz.\n",
    )
    .expect("draft");
    let arguments = args(&[
        "lint",
        "retrieval.md",
        "--issuer",
        PUBLIC_RECORD_ISSUER,
        "--through",
        "2030Q1",
        "--as-of",
        "2030-06-10",
        "--discover",
    ]);
    let result = invoke(&arguments, &workspace).0;
    assert_eq!(result.exit_code, 0, "{}", result.to_json());
    assert!(
        result
            .payload_json
            .contains("\"known_available_by_retrieval\":\"2030-06-03\"")
    );
    assert!(
        result
            .payload_json
            .contains("\"status\":\"publication_date_unknown\"")
    );
    assert!(!result.payload_json.contains("\"status\":\"disclosed\""));
    std::fs::write(
        &registry,
        original.replace(marker, &format!("{marker}retrieved_utc = \"2030-06-05\"\n")),
    )
    .expect("revised source retrieval evidence");
    let revised = invoke(&arguments, &workspace).0;
    assert_eq!(revised.exit_code, 0, "{}", revised.to_json());
    assert!(
        revised
            .payload_json
            .contains("\"known_available_by_retrieval\":\"2030-06-05\"")
    );
    assert_eq!(result.certificate, revised.certificate);
    assert_eq!(
        public_record_component_digest(&result.payload_json, "lint"),
        public_record_component_digest(&revised.payload_json, "lint"),
    );
    assert_ne!(
        public_record_component_digest(&result.payload_json, "public_record"),
        public_record_component_digest(&revised.payload_json, "public_record"),
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
        (3, "9M 2030", "600", (11997, 20), (12003, 20)),
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
fn nine_month_published_equality_obeys_the_exact_source_and_publication_cut() {
    let root = derivation_workspace();
    let hash = source(
        &root,
        "NineMonths.xlsx",
        "2030Q3",
        Some("2030-11-01"),
        "0.0",
        &[summary(
            "9M",
            "Example Mine (60%)",
            PRODUCTION,
            number("600.04"),
        )],
    );
    for period in ["9M 2030", "20309M", "nine months of 2030"] {
        let draft = format!("Example Mine {period} 100% basis gold production is 600 koz.");
        let before = assess(&root, &draft, "2030-10-31");
        let after = assess(&root, &draft, "2030-11-01");
        for report in [&before, &after] {
            assert!(report.payload_json.contains("\"numbers\":\"1\""));
            assert!(report.payload_json.contains("\"verdict\":\"verified\""));
            assert!(report.payload_json.contains(&hash));
            assert!(report.payload_json.contains("20309M"));
        }
        assert!(
            before
                .payload_json
                .contains("\"status\":\"not_in_public_record\"")
        );
        assert!(before.payload_json.contains("\"admission\":\"after_cut\""));
        assert!(after.payload_json.contains("\"status\":\"disclosed\""));
        assert!(after.payload_json.contains("2030-11-01"));
    }
    let too_precise = assess(
        &root,
        "Example Mine 9M 2030 100% basis gold production is 600.00 koz.",
        "2030-11-01",
    );
    assert!(
        too_precise
            .payload_json
            .contains("\"verdict\":\"rounding_beyond_tolerance\"")
    );
    assert!(
        !too_precise
            .payload_json
            .contains("\"status\":\"disclosed\"")
    );
}

#[test]
fn nine_month_sum_requires_q3_and_preserves_competing_or_different_period_sources() {
    let root = derivation_workspace();
    for q in 1..=2 {
        sum_source(
            &root,
            q,
            "Example Mine (60%)",
            PRODUCTION,
            number("100"),
            "0.0",
            Some("2030-08-01"),
        );
    }
    let draft = "Example Mine 9M 2030 100% basis gold production is 300 koz.";
    let missing = assess(&root, draft, "2030-10-31");
    assert_eq!(
        string_field(quarter_sum_report(&missing), "status"),
        "not_available"
    );
    sum_source(
        &root,
        3,
        "Example Mine (60%)",
        PRODUCTION,
        number("100"),
        "0.0",
        Some("2030-11-01"),
    );
    let future = assess(&root, draft, "2030-10-31");
    assert_eq!(
        string_field(quarter_sum_report(&future), "status"),
        "not_available"
    );
    assert!(quarter_sum_report(&future).contains("\"admission\":\"after_cut\""));
    let complete = assess(&root, draft, "2030-11-01");
    assert_eq!(
        string_field(quarter_sum_report(&complete), "status"),
        "derived"
    );
    let fy = assess(&root, &draft.replace("9M 2030", "FY2030"), "2030-11-01");
    assert_eq!(
        string_field(quarter_sum_report(&fy), "status"),
        "not_available"
    );
    source(
        &root,
        "LaterQ3.xlsx",
        "2031Q3",
        Some("2031-11-01"),
        "0.0",
        &[summary(
            "Q3",
            "Example Mine (60%)",
            PRODUCTION,
            number("200"),
        )],
    );
    // The registered summary layout carries a prior-year Q3 in a later Q3 release. A Q4
    // release's three-month column is Q4, so it would not be a competing Q3 observation.
    let profile = load_issuer_profile(&root, DERIVATION_ISSUER).unwrap();
    let snapshot = fmine_public::run_public_assurance(&profile, &root).unwrap();
    assert_eq!(
        snapshot.public_claims.cells["LaterQ3.xlsx:QTD!B3"]
            .key
            .valid_period
            .code(),
        "2030Q3"
    );
    for amount in ["300", "400"] {
        let report = assess(&root, &draft.replace("300", amount), "2031-11-01");
        assert_eq!(
            string_field(quarter_sum_report(&report), "status"),
            "ambiguous"
        );
        assert!(quarter_sum_report(&report).contains("Quarter3.xlsx"));
        assert!(quarter_sum_report(&report).contains("LaterQ3.xlsx"));
        assert!(!report.payload_json.contains("\"status\":\"derivable\""));
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

fn ownership_source(
    root: &Path,
    file: &str,
    release: &str,
    date: Option<&str>,
    label: &str,
    include_share: bool,
) {
    let mut sheet = summary("Q1", label, PRODUCTION, number("100"));
    if include_share {
        sheet
            .cells
            .push(("C2".into(), MinimalCell::Text(SALES.into())));
        sheet.cells.push(("C3".into(), number("50")));
    }
    // Distinct publications can repeat the same observations but must have distinct registered bytes.
    let note = MinimalSheet {
        name: "Publication note".into(),
        hidden: false,
        cells: vec![(
            "A1".into(),
            MinimalCell::Text(format!("Invented publication {file} ({release})")),
        )],
    };
    source(root, file, release, date, "0", &[sheet, note]);
}

fn ownership_report(report: &Envelope) -> &str {
    report
        .payload_json
        .split_once("\"ownership_scaling\":")
        .expect("ownership assessment")
        .1
}

const OWNERSHIP_DRAFT: &str = "Example Mine Q1 2030 attributable gold production is 60 koz.";

#[test]
fn ownership_scaling_uses_same_source_printed_share_and_exact_rounding() {
    let root = derivation_workspace();
    ownership_source(
        &root,
        "Ownership.xlsx",
        "2030Q1",
        Some("2030-05-01"),
        "Example Mine (60%)",
        true,
    );
    let report = assess(&root, OWNERSHIP_DRAFT, "2030-05-01");
    let ownership = ownership_report(&report);
    assert_eq!(
        string_field(ownership, "status"),
        "derived",
        "{}",
        report.to_json()
    );
    assert!(report.payload_json.contains("\"status\":\"derivable\""));
    assert!(!report.payload_json.contains("\"status\":\"disclosed\""));
    assert!(ownership.contains("\"fraction\":{\"num\":\"3\",\"den\":\"5\"}"));
    let derived = ownership.split_once("\"derived_claim\":").unwrap().1;
    // Independently: (100 +/- 0.5) * 0.6 = [59.7, 60.3].
    assert!(derived.contains("\"lo\":{\"num\":\"597\",\"den\":\"10\"}"));
    assert!(derived.contains("\"hi\":{\"num\":\"603\",\"den\":\"10\"}"));
    let profile = load_issuer_profile(&root, DERIVATION_ISSUER).unwrap();
    let snapshot = fmine_public::run_public_assurance(&profile, &root).unwrap();
    for at in ["Ownership.xlsx:QTD!B3", "Ownership.xlsx:QTD!C3"] {
        let c = &snapshot.public_claims.cells[at];
        assert!(ownership.contains(&format!("\"parents\":[\"{}\"]", c.claim.claim_id)));
        assert!(ownership.contains(&c.key.reporting_vintage.file_sha256));
    }
    assert_eq!(report, assess(&root, OWNERSHIP_DRAFT, "2030-05-01"));
    let precise = assess(
        &root,
        &OWNERSHIP_DRAFT.replace("60 koz", "60.0 koz"),
        "2030-05-01",
    );
    assert_eq!(
        string_field(ownership_report(&precise), "status"),
        "precision_not_established"
    );
    assert!(!precise.payload_json.contains("\"status\":\"derivable\""));
    let mismatch = assess(
        &root,
        &OWNERSHIP_DRAFT.replace("60 koz", "80 koz"),
        "2030-05-01",
    );
    assert_eq!(
        string_field(ownership_report(&mismatch), "status"),
        "contradicted"
    );
    let (markdown, code) = run(
        &args(&[
            "lint",
            "derived.md",
            "--issuer",
            DERIVATION_ISSUER,
            "--discover",
            "--as-of",
            "2030-05-01",
            "--format",
            "markdown",
        ]),
        &root,
    );
    assert_eq!(code, 0);
    for text in [
        "Ownership scaling",
        "Ownership Claim",
        "Ownership.xlsx:QTD!C3",
        "non-obvious derived figure",
    ] {
        assert!(markdown.contains(text), "{markdown}");
    }
}

#[test]
fn ownership_scaling_keeps_undated_future_and_missing_evidence_unavailable() {
    for (date, share, cut) in [
        (None, true, "2030-05-01"),
        (Some("2030-05-02"), true, "2030-05-01"),
        (Some("2030-05-01"), false, "2030-05-01"),
    ] {
        let root = derivation_workspace();
        ownership_source(
            &root,
            "Ownership.xlsx",
            "2030Q1",
            date,
            "Example Mine (60%)",
            share,
        );
        let report = assess(&root, OWNERSHIP_DRAFT, cut);
        assert_eq!(
            string_field(ownership_report(&report), "status"),
            "not_available",
            "{}",
            report.to_json()
        );
        assert!(!report.payload_json.contains("\"status\":\"derivable\""));
    }
    let root = derivation_workspace();
    ownership_source(
        &root,
        "Original.xlsx",
        "2030Q1",
        Some("2030-05-01"),
        "Example Mine (60%)",
        false,
    );
    source(
        &root,
        "Later.xlsx",
        "2031Q1",
        Some("2031-05-01"),
        "0",
        &[summary("Q1", "Example Mine (60%)", SALES, number("50"))],
    );
    let report = assess(&root, OWNERSHIP_DRAFT, "2031-05-01");
    assert_eq!(
        string_field(ownership_report(&report), "status"),
        "not_available"
    );
    assert!(ownership_report(&report).contains("other_source_ownership_not_transferred"));
}

#[test]
fn ownership_scaling_never_selects_repeated_or_conflicting_source_evidence() {
    let root = derivation_workspace();
    ownership_source(
        &root,
        "Original.xlsx",
        "2030Q1",
        Some("2030-05-01"),
        "Example Mine (60%)",
        true,
    );
    ownership_source(
        &root,
        "Repeated.xlsx",
        "2031Q1",
        Some("2031-05-01"),
        "Example Mine (60%)",
        true,
    );
    let before = assess(&root, OWNERSHIP_DRAFT, "2030-05-01");
    assert_eq!(string_field(ownership_report(&before), "status"), "derived");
    let after = assess(&root, OWNERSHIP_DRAFT, "2031-05-01");
    assert_eq!(
        string_field(ownership_report(&after), "status"),
        "ambiguous"
    );
    assert!(ownership_report(&after).contains("competing_full_mine_assertions"));
    for label in ["Example Mine (80%)", "Example Mine"] {
        let root = derivation_workspace();
        ownership_source(
            &root,
            "Original.xlsx",
            "2030Q1",
            Some("2030-05-01"),
            label,
            true,
        );
        let report = assess(&root, OWNERSHIP_DRAFT, "2030-05-01");
        assert_ne!(string_field(ownership_report(&report), "status"), "derived");
    }
    let root = derivation_workspace();
    ownership_source(
        &root,
        "Original.xlsx",
        "2030Q1",
        Some("2030-05-01"),
        "Example Mine (60%)",
        true,
    );
    source(
        &root,
        "Restated.xlsx",
        "2031Q1",
        Some("2031-05-01"),
        "0",
        &[summary("Q1", "Example Mine (80%)", SALES, number("80"))],
    );
    let report = assess(&root, OWNERSHIP_DRAFT, "2031-05-01");
    assert_eq!(
        string_field(ownership_report(&report), "status"),
        "ambiguous"
    );
    assert!(ownership_report(&report).contains("conflicting_printed_ownership_shares"));
}

#[test]
fn ownership_scaling_does_not_scale_intensive_costs_or_hide_direct_conflicts() {
    let root = derivation_workspace();
    let mut sheet = summary("Q1", "Example Mine (60%)", PRODUCTION, number("100"));
    sheet.cells.extend([
        ("C2".into(), MinimalCell::Text(ATTRIBUTABLE.into())),
        ("C3".into(), number("80")),
        (
            "D2".into(),
            MinimalCell::Text("All-in sustaining costs ($/oz)".into()),
        ),
        ("D3".into(), number("100")),
    ]);
    source(
        &root,
        "Ownership.xlsx",
        "2030Q1",
        Some("2030-05-01"),
        "0",
        &[sheet],
    );
    let report = assess(&root, OWNERSHIP_DRAFT, "2030-05-01");
    assert_eq!(string_field(ownership_report(&report), "status"), "derived");
    assert!(
        report
            .payload_json
            .contains("\"status\":\"contradicts_disclosed\"")
    );
    assert!(!report.payload_json.contains("\"status\":\"derivable\""));
    let cost = assess(
        &root,
        "Example Mine Q1 2030 attributable AISC per ounce sold is $60/oz.",
        "2030-05-01",
    );
    assert!(ownership_report(&cost).starts_with("null"));
    assert!(!cost.payload_json.contains("\"status\":\"derivable\""));
}

#[test]
fn exact_ownership_kernel_preserves_witnesses_and_refuses_mutation_invalid_shares_and_intensive_inputs()
 {
    use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
    use fmine_metrics::guidance::{Ownership, Period};
    use fmine_metrics::public_derivation::{self as kernel, Amount, AmountKind, OwnershipShare};
    use fmine_numerics::exact_feas::Interval;
    let period = Period {
        year: 2030,
        first_month: 1,
        last_month: 3,
    };
    let amount = Amount {
        issuer: "invented".into(),
        subject: "mine".into(),
        scope: "site".into(),
        kind: AmountKind::GoldProductionKoz,
        ownership: Ownership::Full,
        period,
        interval: Interval {
            lo: Rat::new(199, 2).unwrap(),
            hi: Rat::new(201, 2).unwrap(),
            lo_strict: false,
            hi_strict: false,
        },
    };
    let input = Claim::new(
        kernel::INPUT_METRIC,
        amount.clone(),
        &amount.canonical_tag(),
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::ExtensiveAdditive,
        EvidenceClass::ISSUER,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            "invented.xlsx",
            sha256(b"independent fixture").0,
            "Q1!B3",
        )],
    );
    let share = |fraction, locator: &str| {
        let value = OwnershipShare {
            issuer: "invented".into(),
            subject: "mine".into(),
            scope: "site".into(),
            period,
            fraction,
        };
        let tag = value.canonical_tag();
        Claim::new(
            kernel::SHARE_METRIC,
            value,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::NonAdditive,
            EvidenceClass::ISSUER,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new(
                "invented.xlsx",
                sha256(b"independent fixture").0,
                locator,
            )],
        )
    };
    let a = share(Rat::new(3, 5).unwrap(), "Q1!A3");
    let b = share(Rat::new(3, 5).unwrap(), "Q1!A4");
    for change in 0..4 {
        let mut divergent = a.clone();
        match change {
            0 => divergent.uu_terms.push(fmine_claim::UuTerm {
                scenario_id: "unshared-tail".into(),
                description: "Independent unquantified exposure".into(),
                downside_exposure_usd_m: 10.0,
                trigger_indicator: "Review the cited ownership terms".into(),
            }),
            1 => divergent.supersedes = Some(input.claim_id),
            2 => {
                divergent.representation = Representation::Gaussian {
                    mean: 0.6,
                    std_dev: 0.1,
                }
            }
            _ => {
                divergent = divergent.with_regime(
                    fmine_evidence::color::ValidityDomain::unconstrained(),
                    "independent-ownership-assessment",
                )
            }
        }
        divergent.check_id().unwrap();
        assert_eq!(divergent.claim_id, a.claim_id);
        for witnesses in [[a.clone(), divergent.clone()], [divergent, a.clone()]] {
            assert_eq!(
                kernel::ownership_scale(&input, &witnesses)
                    .unwrap_err()
                    .code,
                if change == 2 {
                    "FMINE-REF-208"
                } else {
                    "FMINE-REF-105"
                }
            );
        }
    }
    let result = kernel::ownership_scale(&input, &[a.clone(), b.clone()]).unwrap();
    assert_eq!(result.value.interval.lo, Rat::new(597, 10).unwrap());
    assert_eq!(result.value.interval.hi, Rat::new(603, 10).unwrap());
    assert_eq!(result.parents.len(), 3);
    assert!(result.parents.contains(&input.claim_id));
    assert!(result.parents.contains(&a.claim_id));
    assert!(result.parents.contains(&b.claim_id));
    assert_eq!(
        result,
        kernel::ownership_scale(&input, &[b, a.clone()]).unwrap()
    );
    for fraction in [Rat::from_integer(0), Rat::new(3, 2).unwrap()] {
        assert_eq!(
            kernel::ownership_scale(&input, &[share(fraction, "Q1!A3")])
                .unwrap_err()
                .code,
            "FMINE-REF-107"
        );
    }
    let mut mutated = a.clone();
    mutated.value.fraction = Rat::new(1, 2).unwrap();
    assert_eq!(
        kernel::ownership_scale(&input, &[mutated])
            .unwrap_err()
            .code,
        "FMINE-REF-507"
    );
    assert!(kernel::ownership_scale(&input, &[]).is_err());
    assert!(
        kernel::ownership_scale(
            &input,
            &[a.clone(), share(Rat::new(4, 5).unwrap(), "Q1!A4")]
        )
        .is_err()
    );
    let old = input;
    let intensive = Claim::new(
        kernel::INPUT_METRIC,
        old.value.clone(),
        old.value_tag(),
        old.representation.clone(),
        old.support,
        AdditivityClass::IntensiveWeighted,
        old.evidence,
        old.epistemic,
        old.state,
        old.confidentiality.clone(),
        vec![],
        vec![],
        old.anchors.clone(),
    );
    assert_eq!(
        kernel::ownership_scale(&intensive, &[a]).unwrap_err().code,
        "FMINE-REF-103"
    );
}

const AISC_RATE_DRAFT: &str = "Example Mine Q1 2030 attributable AISC per ounce sold is $1.5k/oz.";

fn cost_sheet(numerator: &str, sold: MinimalCell, printed_rate: Option<&str>) -> MinimalSheet {
    let mut cells = vec![
        (
            "A1".into(),
            MinimalCell::Text(
                "Reconciliation of cost of sales to total cash costs and all-in sustaining costs"
                    .into(),
            ),
        ),
        (
            "A3".into(),
            MinimalCell::Text("($ millions, except per ounce information)".into()),
        ),
        ("B4".into(), MinimalCell::Text("Example Mine".into())),
    ];
    for (row, label, value) in [
        (
            5,
            "Cost of sales applicable to gold production",
            number("100"),
        ),
        (6, "Depreciation", number("0")),
        (7, "Non-controlling interests", number("0")),
        (8, "Total cash costs", number("100")),
        (9, "General & administrative costs", number("0")),
        (10, "Minesite sustaining capital expenditures", number("50")),
        (11, "All-in sustaining costs", number(numerator)),
        (13, "Ounces sold - attributable basis (000s ounces)", sold),
        (14, "Cost of sales per ounce", number("1000")),
        (
            15,
            "Total cash costs per ounce",
            MinimalCell::Text("—".into()),
        ),
        (
            16,
            "All-in sustaining costs per ounce",
            printed_rate.map_or_else(|| MinimalCell::Text("—".into()), number),
        ),
    ] {
        cells.push((format!("A{row}"), MinimalCell::Text(label.into())));
        cells.push((format!("B{row}"), value));
    }
    MinimalSheet {
        name: "Q1 2030".into(),
        hidden: false,
        cells,
    }
}

fn cost_source(
    root: &Path,
    file: &str,
    date: Option<&str>,
    precision: &str,
    sheet: MinimalSheet,
) -> String {
    let note = MinimalSheet {
        name: "Publication note".into(),
        hidden: false,
        cells: vec![(
            "A1".into(),
            MinimalCell::Text(format!("Invented cost filing {file}")),
        )],
    };
    let hash = source(root, file, "2030Q1", date, precision, &[sheet, note]);
    let path = registry_path(root, DERIVATION_ISSUER);
    let mut registry = std::fs::read_to_string(&path).unwrap();
    let before = format!("file = \"{file}\"\nkind = \"mine_stats\"");
    assert_eq!(registry.matches(&before).count(), 1);
    registry = registry.replace(
        &before,
        &format!("file = \"{file}\"\nkind = \"aisc_reconciliation\""),
    );
    if !registry.contains("[[share]]") {
        registry.push_str("\n[[share]]\nsubject = \"Example Mine\"\nnum = 3\nden = 5\nevidence = \"FACT: invented registry basis, not a printed numeric share\"\n");
    }
    std::fs::write(path, registry).unwrap();
    hash
}

fn cost_rate_report(report: &Envelope) -> &str {
    report
        .payload_json
        .split_once("\"cost_rate\":")
        .expect("cost-rate assessment")
        .1
}

#[test]
fn coproduct_aisc_is_disclosed_from_its_own_dated_source_rate() {
    let root = derivation_workspace();
    let mut sheet = cost_sheet("150", number("100"), Some("1500"));
    sheet.cells.extend([
        (
            "A17".into(),
            MinimalCell::Text("All-in sustaining costs per ounce (on a co-product basis)".into()),
        ),
        ("B17".into(), number("1600.4")),
    ]);
    let hash = cost_source(&root, "Costs.xlsx", Some("2030-05-01"), "0", sheet);
    let draft = "Example Mine Q1 2030 ownership basis unprinted co-product AISC per ounce sold is $1,600/oz.";
    let report = assess(&root, draft, "2030-05-01");
    assert!(
        report.payload_json.contains("\"verdict\":\"verified\""),
        "{}",
        report.to_json()
    );
    assert!(report.payload_json.contains("\"status\":\"disclosed\""));
    assert!(report.payload_json.contains("Costs.xlsx:Q1 2030!B17"));
    assert!(
        report
            .payload_json
            .contains("\"stored\":\"1600.4\",\"displayed\":\"1600\"")
    );
    assert!(report.payload_json.contains(&hash));
    assert!(!report.payload_json.contains("\"status\":\"derivable\""));
    assert_eq!(report, assess(&root, draft, "2030-05-01"));
    let before = assess(&root, draft, "2030-04-30");
    assert!(!before.payload_json.contains("\"status\":\"disclosed\""));
    let contradicted = assess(
        &root,
        &draft.replace("$1,600/oz", "$1,500/oz"),
        "2030-05-01",
    );
    assert!(
        contradicted
            .payload_json
            .contains("\"status\":\"contradicts_disclosed\""),
        "{}",
        contradicted.to_json()
    );
    assert!(contradicted.payload_json.contains("Costs.xlsx:Q1 2030!B17"));
}

#[test]
fn cost_rates_derive_gold_aisc_and_byproduct_tcc_from_exact_printed_source_enclosures() {
    let root = derivation_workspace();
    let hash = cost_source(
        &root,
        "Costs.xlsx",
        Some("2030-05-01"),
        "0",
        cost_sheet("150.4", number("100"), None),
    );
    for (draft, family, lo, hi) in [
        (AISC_RATE_DRAFT, "aisc_per_oz", "299000", "301000"),
        (
            "Example Mine Q1 2030 attributable by-product TCC per ounce sold is $1.0k/oz.",
            "tcc_per_oz",
            "199000",
            "201000",
        ),
    ] {
        let report = assess(&root, draft, "2030-05-01");
        let rate = cost_rate_report(&report);
        assert_eq!(
            string_field(rate, "status"),
            "derived",
            "{}",
            report.to_json()
        );
        assert!(report.payload_json.contains("\"status\":\"derivable\""));
        assert_eq!(string_field(rate, "family"), family);
        assert!(rate.contains(&format!("\"lo\":{{\"num\":\"{lo}\",\"den\":\"201\"}}")));
        assert!(rate.contains(&format!("\"hi\":{{\"num\":\"{hi}\",\"den\":\"199\"}}")));
        let derived = rate
            .split_once("\"derived_claim\":")
            .unwrap()
            .1
            .split_once(",\"evidence\":[")
            .unwrap()
            .0;
        assert!(derived.contains("\"evidence\":\"Assertion { source: Issuer }\""));
        assert!(!derived.contains("\"evidence\":\"Derived\""));
        assert!(derived.contains("\"additivity\":\"IntensiveWeighted\""));
        let parents = derived
            .split_once("\"parents\":[")
            .unwrap()
            .1
            .split(']')
            .next()
            .unwrap();
        assert_eq!(parents.matches("clm_").count(), 2);
        assert!(derived.contains("\"locator\":\"Q1 2030!B13\""));
        assert!(derived.contains(&hash));
        assert!(rate.contains("\"formula\":\"1000*N/Q\""));
        assert!(rate.contains("\"unit\":\"USD/oz\""));
        assert!(
            rate.contains("\"kind\":\"unprinted\""),
            "original numerator basis survives"
        );
        assert!(
            rate.contains("\"source\":\"registry\""),
            "the share is not relabelled printed"
        );
        assert!(rate.contains(&hash));
        assert!(rate.contains("Costs.xlsx:Q1 2030!B13"));
        assert!(rate.contains("not make the derived figure an issuer disclosure"));
        assert_eq!(report, assess(&root, draft, "2030-05-01"));
    }
    let aisc = assess(&root, AISC_RATE_DRAFT, "2030-05-01");
    assert!(cost_rate_report(&aisc).contains("\"stored\":\"150.4\",\"displayed\":\"150\""));
    let (markdown, exit) = run(
        &args(&[
            "lint",
            "derived.md",
            "--issuer",
            DERIVATION_ISSUER,
            "--discover",
            "--as-of",
            "2030-05-01",
            "--format",
            "markdown",
        ]),
        &root,
    );
    assert_eq!(exit, 0);
    assert!(markdown.contains("299000/201"));
    assert!(markdown.contains("301000/199"));
    assert!(markdown.contains("Costs.xlsx:Q1 2030!B11"));
    assert!(markdown.contains("1000*N/Q"));
}

#[test]
fn cost_rate_rounding_and_definition_controls_never_invent_precision_or_a_denominator_basis() {
    let root = derivation_workspace();
    cost_source(
        &root,
        "Costs.xlsx",
        Some("2030-05-01"),
        "0",
        cost_sheet("150", number("100"), None),
    );
    for (value, status) in [
        ("$1,500/oz", "precision_not_established"),
        ("$2.0k/oz", "contradicted"),
    ] {
        let report = assess(
            &root,
            &AISC_RATE_DRAFT.replace("$1.5k/oz", value),
            "2030-05-01",
        );
        assert_eq!(string_field(cost_rate_report(&report), "status"), status);
        assert!(!report.payload_json.contains("\"status\":\"derivable\""));
    }
    for draft in [
        "Example Mine Q1 2030 attributable AISC per ounce produced is $1.5k/oz.",
        "Example Mine Q1 2030 100% basis AISC per ounce sold is $1.5k/oz.",
        "Example Mine Q1 2030 attributable co-product TCC per ounce sold is $1.0k/oz.",
        "Example Mine Q1 2030 attributable TCC per ounce sold is $1.0k/oz.",
        "Example Mine Q1 2030 attributable C1 per pound sold is $1.5/lb.",
        "Example Mine Q1 2030 attributable cost of sales per ounce sold is $1.0k/oz.",
    ] {
        let report = assess(&root, draft, "2030-05-01");
        assert!(
            cost_rate_report(&report).starts_with("null"),
            "{draft}: {}",
            report.to_json()
        );
        assert!(!report.payload_json.contains("\"status\":\"derivable\""));
    }
}

#[test]
fn cost_rates_keep_publication_cuts_unknowns_and_nonpositive_denominators_explicit() {
    for (date, precision, sold, status, refusal) in [
        (None, "0", number("100"), "not_available", None),
        (
            Some("2030-05-02"),
            "0",
            number("100"),
            "not_available",
            None,
        ),
        (
            Some("2030-05-01"),
            "0",
            number("0"),
            "refused",
            Some("FMINE-REF-104"),
        ),
        (
            Some("2030-05-01"),
            "0",
            number("-1"),
            "refused",
            Some("FMINE-REF-104"),
        ),
        (
            Some("2030-05-01"),
            "0",
            MinimalCell::Text("—".into()),
            "not_available",
            None,
        ),
    ] {
        let root = derivation_workspace();
        cost_source(
            &root,
            "Costs.xlsx",
            date,
            precision,
            cost_sheet("150", sold, None),
        );
        let report = assess(&root, AISC_RATE_DRAFT, "2030-05-01");
        let rate = cost_rate_report(&report);
        assert_eq!(string_field(rate, "status"), status, "{}", report.to_json());
        assert!(rate.contains("\"derived_claim\":null"));
        if let Some(code) = refusal {
            assert!(rate.contains(code));
        }
        assert!(!report.payload_json.contains("\"status\":\"derivable\""));
    }
    let root = derivation_workspace();
    cost_source(
        &root,
        "Reversal.xlsx",
        Some("2030-05-01"),
        "0",
        cost_sheet("-150", number("100"), None),
    );
    let report = assess(
        &root,
        &AISC_RATE_DRAFT.replace("$1.5k", "-$1.5k"),
        "2030-05-01",
    );
    let rate = cost_rate_report(&report);
    assert_eq!(string_field(rate, "status"), "derived");
    assert!(rate.contains("\"lo\":{\"num\":\"-301000\",\"den\":\"199\"}"));
    assert!(rate.contains("\"hi\":{\"num\":\"-299000\",\"den\":\"201\"}"));
}

#[test]
fn cost_rates_retain_competing_vintages_without_selecting_a_matching_number() {
    for (other_date, other_value) in [
        (Some("2030-05-02"), "150"),
        (Some("2030-05-02"), "200"),
        (None, "150"),
    ] {
        let root = derivation_workspace();
        let first = cost_source(
            &root,
            "Original.xlsx",
            Some("2030-05-01"),
            "0",
            cost_sheet("150", number("100"), None),
        );
        let second = cost_source(
            &root,
            "Restated.xlsx",
            other_date,
            "0",
            cost_sheet(other_value, number("100"), None),
        );
        let report = assess(&root, AISC_RATE_DRAFT, "2030-05-02");
        let rate = cost_rate_report(&report);
        assert_eq!(string_field(rate, "status"), "ambiguous");
        assert!(rate.contains(&first) && rate.contains(&second));
        assert!(rate.contains("\"derived_claim\":null"));
        assert!(!report.payload_json.contains("\"status\":\"derivable\""));
        if other_date.is_some() {
            let before = assess(&root, AISC_RATE_DRAFT, "2030-05-01");
            assert_eq!(string_field(cost_rate_report(&before), "status"), "derived");
            assert!(cost_rate_report(&before).contains("\"admission\":\"after_cut\""));
        }
    }
}

#[test]
fn cost_rate_arithmetic_never_overrides_a_same_basis_direct_disclosure_or_conflict() {
    for (printed, expected) in [("1500", "disclosed"), ("2000", "contradicts_disclosed")] {
        let root = derivation_workspace();
        cost_source(
            &root,
            "Costs.xlsx",
            Some("2030-05-01"),
            "0",
            cost_sheet("150", number("100"), None),
        );
        source(
            &root,
            "PublishedRate.xlsx",
            "2030Q1",
            Some("2030-05-01"),
            "0",
            &[summary(
                "Q1",
                "Example Mine (60%)",
                "All-in sustaining costs ($/oz)",
                number(printed),
            )],
        );
        let report = assess(&root, AISC_RATE_DRAFT, "2030-05-01");
        assert_eq!(string_field(cost_rate_report(&report), "status"), "derived");
        assert!(
            report
                .payload_json
                .contains(&format!("\"status\":\"{expected}\"")),
            "{}",
            report.to_json()
        );
        assert!(!report.payload_json.contains("\"status\":\"derivable\""));
    }
}

#[test]
fn decimal_gold_cost_rates_keep_each_displayed_rounding_step() {
    for (numerator, amount, lo, hi) in [
        ("150.004", "$1,500/oz", "29999000/20001", "30001000/19999"),
        (
            "-150.004",
            "-$1,500/oz",
            "-30001000/19999",
            "-29999000/20001",
        ),
    ] {
        let root = derivation_workspace();
        let hash = cost_source(
            &root,
            "DecimalCosts.xlsx",
            Some("2030-05-01"),
            "0.00",
            cost_sheet(numerator, number("100.004"), None),
        );
        let draft = AISC_RATE_DRAFT.replace("$1.5k/oz", amount);
        let report = assess(&root, &draft, "2030-05-01");
        let rate = cost_rate_report(&report);
        assert_eq!(
            string_field(rate, "status"),
            "derived",
            "{}",
            report.to_json()
        );
        for (key, value) in [("lo", lo), ("hi", hi)] {
            let (n, d) = value.split_once('/').unwrap();
            assert!(rate.contains(&format!("\"{key}\":{{\"num\":\"{n}\",\"den\":\"{d}\"}}")));
        }
        assert!(rate.contains("\"stored\":\"100.004\",\"displayed\":\"100.00\""));
        assert!(rate.contains(&hash));
        assert!(rate.contains("half its own displayed step"));
        assert!(report.payload_json.contains("\"status\":\"derivable\""));
        let precise = assess(
            &root,
            &draft.replace("1,500/oz", "1,500.00/oz"),
            "2030-05-01",
        );
        assert_eq!(
            string_field(cost_rate_report(&precise), "status"),
            "precision_not_established"
        );
        assert!(!precise.payload_json.contains("\"status\":\"derivable\""));
        assert_eq!(report, assess(&root, &draft, "2030-05-01"));
    }
}

fn copper_cost_sheet(subject: &str, sold: &str, c1: &str, aisc: &str) -> MinimalSheet {
    let text = |s: &str| MinimalCell::Text(s.into());
    MinimalSheet {
        name: "Minestats1".into(),
        hidden: false,
        cells: vec![
            ("B1".into(), text(subject)),
            ("A2".into(), text("For the three months ended March 31,")),
            ("B2".into(), MinimalCell::Number(2030.0)),
            ("A3".into(), text("Sales (millions of pounds)")),
            ("B3".into(), number(sold)),
            ("A4".into(), text("Costs ($ millions)")),
            ("A5".into(), text("C1 cash costs")),
            ("B5".into(), number(c1)),
            ("A6".into(), text("All-in sustaining costs")),
            ("B6".into(), number(aisc)),
        ],
    }
}

fn copper_cost_source(root: &Path, file: &str, date: Option<&str>, sheet: MinimalSheet) -> String {
    let hash = source(root, file, "2030Q1", date, "0.00", &[sheet]);
    let path = registry_path(root, DERIVATION_ISSUER);
    let mut registry = std::fs::read_to_string(&path).unwrap();
    if !registry.contains("[[mine_stats_label]]") {
        registry.push_str(
            "\n[[mine_stats_label]]\nlabel = \"Sales (millions of pounds)\"\n\
             [[mine_stats_label]]\nsection = \"Costs ($ millions)\"\nlabel = \"C1 cash costs\"\n\
             [[mine_stats_label]]\nsection = \"Costs ($ millions)\"\nlabel = \"All-in sustaining costs\"\n\
             [[mine_stats_label]]\nsection = \"Costs ($ millions)\"\nlabel = \"Cost of sales\"\n",
        );
        std::fs::write(path, registry).unwrap();
    }
    hash
}

fn copper_sales_cost_sheet(subject: &str, sold: &str, cost: &str) -> MinimalSheet {
    let mut sheet = copper_cost_sheet(subject, sold, "200", "225");
    sheet.cells.extend([
        ("A7".into(), MinimalCell::Text("Cost of sales".into())),
        ("B7".into(), number(cost)),
    ]);
    sheet
}

#[test]
fn copper_cost_of_sales_derives_cited_exact_bounds_and_reacts_to_both_operands() {
    for (subject, ownership) in [
        ("Example Mine (60%)", "attributable"),
        ("Example Mine (100%)", "100% basis"),
    ] {
        let root = derivation_workspace();
        let hash = copper_cost_source(
            &root,
            "CopperCosts.xlsx",
            Some("2030-05-01"),
            copper_sales_cost_sheet(subject, "100.004", "300.004"),
        );
        let draft =
            format!("Example Mine Q1 2030 {ownership} cost of sales per pound sold is $3.00/lb.");
        let report = assess(&root, &draft, "2030-05-01");
        let rate = cost_rate_report(&report);
        assert_eq!(
            string_field(rate, "status"),
            "derived",
            "{}",
            report.to_json()
        );
        assert_eq!(string_field(rate, "family"), "cos_per_lb");
        // The displayed 300.00 and 100.00 carry their own half-cent boxes, not the stored decimals.
        assert!(rate.contains("\"lo\":{\"num\":\"59999\",\"den\":\"20001\"}"));
        assert!(rate.contains("\"hi\":{\"num\":\"60001\",\"den\":\"19999\"}"));
        assert!(rate.contains("\"formula\":\"N/Q\""));
        assert!(rate.contains("\"unit\":\"USD/lb\""));
        assert!(rate.contains("CopperCosts.xlsx:Minestats1!B7"));
        assert!(rate.contains("CopperCosts.xlsx:Minestats1!B3"));
        assert!(rate.contains(&hash));
        assert!(rate.contains("\"stored\":\"300.004\",\"displayed\":\"300.00\""));
        assert!(rate.contains("\"evidence\":\"Assertion { source: Issuer }\""));
        assert!(report.payload_json.contains("\"status\":\"derivable\""));
        assert!(!report.payload_json.contains("\"status\":\"disclosed\""));
        assert_eq!(report, assess(&root, &draft, "2030-05-01"));
        no_authority(&report);

        let precise = assess(&root, &draft.replace("3.00/lb", "3.0000/lb"), "2030-05-01");
        assert_eq!(
            string_field(cost_rate_report(&precise), "status"),
            "precision_not_established"
        );
        let before = assess(&root, &draft, "2030-04-30");
        assert_eq!(
            string_field(cost_rate_report(&before), "status"),
            "not_available"
        );

        for (sold, cost) in [("100.004", "310.004"), ("120.004", "300.004")] {
            let changed = derivation_workspace();
            copper_cost_source(
                &changed,
                "CopperCosts.xlsx",
                Some("2030-05-01"),
                copper_sales_cost_sheet(subject, sold, cost),
            );
            let result = assess(&changed, &draft, "2030-05-01");
            assert_eq!(
                string_field(cost_rate_report(&result), "status"),
                "contradicted"
            );
            assert_ne!(report.payload_json, result.payload_json);
        }
        assess(&root, &draft, "2030-05-01");
        let (markdown, exit) = run(
            &args(&[
                "lint",
                "derived.md",
                "--issuer",
                DERIVATION_ISSUER,
                "--discover",
                "--as-of",
                "2030-05-01",
                "--format",
                "markdown",
            ]),
            &root,
        );
        assert_eq!(exit, 0);
        assert!(markdown.contains("cos_per_lb"));
        assert!(markdown.contains("59999/20001") && markdown.contains("60001/19999"));
    }
}

#[test]
fn copper_cost_of_sales_cannot_mix_source_tables_or_infer_missing_bases() {
    let draft = "Example Mine Q1 2030 attributable cost of sales per pound sold is $3.00/lb.";
    let root = derivation_workspace();
    let mut costs = copper_sales_cost_sheet("Example Mine (60%)", "100", "300");
    costs.cells.retain(|(at, _)| at != "A3" && at != "B3");
    copper_cost_source(&root, "Costs.xlsx", Some("2030-05-01"), costs);
    copper_cost_source(
        &root,
        "Sales.xlsx",
        Some("2030-05-01"),
        copper_cost_sheet("Example Mine (60%)", "100", "200", "225"),
    );
    let split = assess(&root, draft, "2030-05-01");
    let rate = cost_rate_report(&split);
    assert_eq!(string_field(rate, "status"), "refused");
    assert!(rate.contains("FMINE-REF-105"));
    assert!(rate.contains("\"derived_claim\":null"));

    let root = derivation_workspace();
    copper_cost_source(
        &root,
        "Copper.xlsx",
        Some("2030-05-01"),
        copper_sales_cost_sheet("Example Mine (60%)", "100", "300"),
    );
    for unsupported in [
        draft.replace("sold", "produced"),
        draft.replace("pound", "ounce").replace("/lb", "/oz"),
        draft.replace("attributable", "100% basis"),
        draft.replace("Q1", "Q2"),
    ] {
        let report = assess(&root, &unsupported, "2030-05-01");
        assert!(
            !report.payload_json.contains("\"status\":\"derivable\""),
            "{}",
            report.to_json()
        );
        assert!(!report.payload_json.contains("\"status\":\"derived\""));
    }
    let unknown = derivation_workspace();
    copper_cost_source(
        &unknown,
        "Copper.xlsx",
        Some("2030-05-01"),
        copper_sales_cost_sheet("Example Mine", "100", "300"),
    );
    assert!(cost_rate_report(&assess(&unknown, draft, "2030-05-01")).starts_with("null"));
}

#[test]
fn copper_cost_rates_derive_aisc_and_c1_from_registered_decimal_workbook_rows() {
    for (subject, ownership) in [
        ("Example Mine (60%)", "attributable"),
        ("Example Mine (100%)", "100% basis"),
    ] {
        let root = derivation_workspace();
        let hash = copper_cost_source(
            &root,
            "CopperCosts.xlsx",
            Some("2030-05-01"),
            copper_cost_sheet(subject, "100.004", "200.004", "225.004"),
        );
        // Printed C1 lower endpoint: 199.995 / 100.005 = 39999/20001 = 13333/6667.
        for (metric, amount, family, lo, hi) in [
            ("AISC", "2.25", "aisc_per_lb", "44999/20001", "45001/19999"),
            ("C1", "2.00", "c1_per_lb", "13333/6667", "40001/19999"),
        ] {
            let draft = format!(
                "Example Mine Q1 2030 {ownership} {metric} per pound sold is ${amount}/lb."
            );
            let report = assess(&root, &draft, "2030-05-01");
            let rate = cost_rate_report(&report);
            assert_eq!(
                string_field(rate, "status"),
                "derived",
                "{}",
                report.to_json()
            );
            assert_eq!(string_field(rate, "family"), family);
            assert!(rate.contains("\"formula\":\"N/Q\""));
            assert!(rate.contains("\"unit\":\"USD/lb\""));
            assert!(rate.contains("public-disclosure.copper-cost-rate/1"));
            for (key, value) in [("lo", lo), ("hi", hi)] {
                let (n, d) = value.split_once('/').unwrap();
                assert!(rate.contains(&format!("\"{key}\":{{\"num\":\"{n}\",\"den\":\"{d}\"}}")));
            }
            assert!(rate.contains("\"stored\":\"100.004\",\"displayed\":\"100.00\""));
            assert!(rate.contains("CopperCosts.xlsx:Minestats1!B3"));
            assert!(rate.contains(&hash));
            assert!(rate.contains("\"evidence\":\"Assertion { source: Issuer }\""));
            assert!(rate.contains("\"additivity\":\"IntensiveWeighted\""));
            assert!(report.payload_json.contains("\"status\":\"derivable\""));
            assert!(!report.payload_json.contains("\"status\":\"disclosed\""));
            assert_eq!(report, assess(&root, &draft, "2030-05-01"));
            let precise = assess(
                &root,
                &draft.replace(&format!("${amount}/lb"), &format!("${amount}00/lb")),
                "2030-05-01",
            );
            assert_eq!(
                string_field(cost_rate_report(&precise), "status"),
                "precision_not_established"
            );
            let contradicted = assess(
                &root,
                &draft.replace(&format!("${amount}/lb"), "$9.00/lb"),
                "2030-05-01",
            );
            assert_eq!(
                string_field(cost_rate_report(&contradicted), "status"),
                "contradicted"
            );
            assess(&root, &draft, "2030-05-01");
            let (markdown, exit) = run(
                &args(&[
                    "lint",
                    "derived.md",
                    "--issuer",
                    DERIVATION_ISSUER,
                    "--discover",
                    "--as-of",
                    "2030-05-01",
                    "--format",
                    "markdown",
                ]),
                &root,
            );
            assert_eq!(exit, 0);
            assert!(markdown.contains(lo) && markdown.contains(hi));
            assert!(markdown.contains("N/Q in USD/lb"));
        }
    }
}

#[test]
fn copper_cost_rates_keep_publication_cuts_competing_vintages_and_corruption_explicit() {
    let draft = "Example Mine Q1 2030 attributable C1 per pound sold is $2.00/lb.";
    for later_date in [Some("2030-05-02"), None] {
        let root = derivation_workspace();
        let first = copper_cost_source(
            &root,
            "Copper.xlsx",
            Some("2030-05-01"),
            copper_cost_sheet("Example Mine (60%)", "100", "200", "225"),
        );
        let before = assess(&root, draft, "2030-04-30");
        assert_eq!(
            string_field(cost_rate_report(&before), "status"),
            "not_available"
        );
        assert!(!before.payload_json.contains("\"status\":\"derivable\""));
        let second = copper_cost_source(
            &root,
            "RevisedCopper.xlsx",
            later_date,
            copper_cost_sheet("Example Mine (60%)", "100", "210", "235"),
        );
        let report = assess(&root, draft, "2030-05-02");
        let rate = cost_rate_report(&report);
        assert_eq!(string_field(rate, "status"), "ambiguous");
        assert!(rate.contains(&first) && rate.contains(&second));
        assert!(rate.contains("\"derived_claim\":null"));
        if later_date.is_some() {
            let earlier = assess(&root, draft, "2030-05-01");
            assert_eq!(
                string_field(cost_rate_report(&earlier), "status"),
                "derived"
            );
        }
        std::fs::write(root.join("corpus/Copper.xlsx"), b"changed workbook bytes").unwrap();
        let corrupted = invoke(
            &args(&[
                "lint",
                "derived.md",
                "--issuer",
                DERIVATION_ISSUER,
                "--as-of",
                "2030-05-02",
            ]),
            &root,
        )
        .0;
        assert_ne!(corrupted.exit_code, 0);
        assert!(corrupted.to_json().contains("FMINE-REF-505"));
    }
}

#[test]
fn copper_cost_rates_refuse_unknown_ownership_production_units_and_unsafe_denominators() {
    let draft = "Example Mine Q1 2030 attributable C1 per pound sold is $2.00/lb.";
    for (subject, sales, status) in [
        ("Example Mine", "100.00", "none"),
        ("Example Mine (60%)", "0.00", "refused"),
        ("Example Mine (60%)", "-0.01", "refused"),
    ] {
        let root = derivation_workspace();
        copper_cost_source(
            &root,
            "Copper.xlsx",
            Some("2030-05-01"),
            copper_cost_sheet(subject, sales, "200.00", "225.00"),
        );
        let report = assess(&root, draft, "2030-05-01");
        if status == "none" {
            assert!(cost_rate_report(&report).starts_with("null"));
        } else {
            assert_eq!(string_field(cost_rate_report(&report), "status"), status);
            assert!(cost_rate_report(&report).contains("FMINE-REF-104"));
        }
        assert!(!report.payload_json.contains("\"status\":\"derivable\""));
    }
    let root = derivation_workspace();
    copper_cost_source(
        &root,
        "Copper.xlsx",
        Some("2030-05-01"),
        copper_cost_sheet("Example Mine (60%)", "100.00", "200.00", "225.00"),
    );
    for invalid in [
        draft.replace("sold", "produced"),
        draft.replace("pound", "ounce").replace("/lb", "/oz"),
        draft.replace("attributable", "100% basis"),
        draft.replace("C1", "co-product C1"),
        draft.replace("C1", "by-product TCC"),
    ] {
        let report = assess(&root, &invalid, "2030-05-01");
        assert!(
            cost_rate_report(&report).starts_with("null"),
            "{invalid}: {}",
            report.to_json()
        );
        assert!(!report.payload_json.contains("\"status\":\"derivable\""));
    }
    // The real registry has stale Mlb labels whose printed values are kt. Honour that correction;
    // the source adapter must never turn an alias into a silent tonne-to-pound conversion here.
    let path = registry_path(&root, DERIVATION_ISSUER);
    let mut registry = std::fs::read_to_string(&path).unwrap();
    registry.push_str(
        "\n[[mine_stats_label]]\nlabel = \"Sales (Thousands of tonnes)\"\n\
         [[mine_stats_label_alias]]\nlabel = \"Sales (millions of pounds)\"\n\
         reads_as = \"Sales (Thousands of tonnes)\"\nfirst = \"2030Q1\"\nlast = \"2030Q1\"\n\
         evidence = \"FACT: invented source label correction\"\n",
    );
    std::fs::write(path, registry).unwrap();
    let corrected = assess(&root, draft, "2030-05-01");
    assert!(cost_rate_report(&corrected).starts_with("null"));
    assert!(!corrected.payload_json.contains("\"status\":\"derivable\""));
}

#[cfg(test)]
mod derived_amount_claims {
    use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
    use fmine_metrics::guidance::{Ownership, Period};
    use fmine_metrics::public_derivation::{Amount, AmountKind, INPUT_METRIC, Rule, derive};
    use fmine_numerics::exact_feas::{Interval, Rat};

    fn amount(first: u8, last: u8, printed: i128) -> Claim<Amount> {
        let value = Amount {
            issuer: "invented".into(),
            subject: "Site A".into(),
            scope: "site".into(),
            kind: AmountKind::GoldProductionKoz,
            ownership: Ownership::Full,
            period: Period {
                year: 2030,
                first_month: first,
                last_month: last,
            },
            interval: Interval {
                lo: Rat::new(2 * printed - 1, 2).unwrap(),
                hi: Rat::new(2 * printed + 1, 2).unwrap(),
                lo_strict: false,
                hi_strict: false,
            },
        };
        let tag = value.canonical_tag();
        Claim::new(
            INPUT_METRIC,
            value,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::ISSUER,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new(
                "invented.xlsx",
                [first + last; 32],
                format!("Q{last}!B1"),
            )],
        )
    }

    fn reseal(source: Claim<Amount>) -> Claim<Amount> {
        let mut result = Claim::new(
            &source.metric_key,
            source.value.clone(),
            &source.value.canonical_tag(),
            source.representation,
            source.support,
            source.additivity,
            source.evidence,
            source.epistemic,
            source.state,
            source.confidentiality,
            source.uu_terms,
            source.parents,
            source.anchors,
        );
        result.stale = source.stale;
        result.calibration_failed = source.calibration_failed;
        result
    }

    #[test]
    fn exact_sum_overflow_refuses_in_both_directions_without_partial_output() {
        for (limit, increment) in [(i128::MAX, 1), (i128::MIN, -1)] {
            let mut q1 = amount(1, 3, 0);
            q1.value.interval.lo = Rat::from_integer(limit);
            q1.value.interval.hi = Rat::from_integer(limit);
            let mut q2 = amount(4, 6, 0);
            q2.value.interval.lo = Rat::from_integer(increment);
            q2.value.interval.hi = Rat::from_integer(increment);
            let sources = [reseal(q1), reseal(q2)];
            assert!(sources.iter().all(|source| source.check_id().is_ok()));
            assert_eq!(
                derive(Rule::QuarterSum, &sources).unwrap_err().code,
                "FMINE-REF-207"
            );
        }
    }

    #[test]
    fn canonical_but_unusable_and_placeholder_inputs_never_become_summed_amounts() {
        for state in [
            KnowledgeState::Unknown,
            KnowledgeState::NotApplicable,
            KnowledgeState::NotObservable,
            KnowledgeState::Conflicted,
            KnowledgeState::Indeterminate,
            KnowledgeState::Redacted,
            KnowledgeState::Stale,
        ] {
            let mut bad = amount(1, 3, 100);
            bad.state = state;
            let bad = reseal(bad);
            assert!(bad.check_id().is_ok());
            assert!(derive(Rule::QuarterSum, &[bad, amount(4, 6, 100)]).is_err());
        }
        for change in 0..9 {
            let mut bad = amount(1, 3, 100);
            match change {
                0 => {
                    bad.representation = Representation::Placeholder {
                        token: "—".into()
                    }
                }
                1 => bad.anchors.clear(),
                2 => bad.anchors[0].artifact_sha256 = [0; 32],
                3 => bad.anchors[0].locator.clear(),
                4 => bad.anchors[0].artifact_path.clear(),
                5 => bad.confidentiality = Confidentiality::internal(),
                6 => bad.calibration_failed = true,
                7 => bad.support = SpatialSupport::PointCore,
                _ => bad.value.interval.lo_strict = true,
            }
            let bad = reseal(bad);
            assert!(bad.check_id().is_ok());
            assert!(
                derive(Rule::QuarterSum, &[bad, amount(4, 6, 100)]).is_err(),
                "admission control {change}"
            );
        }
    }

    #[test]
    fn summation_preserves_source_identity_weaker_metadata_and_unknown_terms() {
        let mut q1 = amount(1, 3, 100);
        q1.evidence = EvidenceClass::Measured;
        let q1 = reseal(q1);
        let mut q2 = amount(4, 6, 100);
        q2.state = KnowledgeState::Estimated;
        q2.epistemic = EpistemicStatus::KnownUnknown(fmine_evidence::KuKind::Parametric);
        q2.uu_terms.push(fmine_claim::UuTerm {
            scenario_id: "invented-period-reclassification".into(),
            description: "Synthetic unknown accounting allocation".into(),
            downside_exposure_usd_m: 1.0,
            trigger_indicator: "later issuer correction".into(),
        });
        let q2 = reseal(q2);
        let output = derive(Rule::QuarterSum, &[q2.clone(), q1.clone()]).unwrap();
        assert!(output.check_id().is_ok());
        assert_eq!(output.evidence, EvidenceClass::ISSUER);
        assert!(output.evidence.no_stronger_than(q1.evidence));
        assert!(output.evidence.no_stronger_than(q2.evidence));
        assert_eq!(output.state, q2.state);
        assert_eq!(output.epistemic, q2.epistemic);
        assert_eq!(output.uu_terms, q2.uu_terms);
        let mut parents = vec![q1.claim_id, q2.claim_id];
        parents.sort();
        assert_eq!(output.parents, parents);
        let mut anchors = [q1.anchors.clone(), q2.anchors.clone()].concat();
        anchors.sort();
        assert_eq!(output.anchors, anchors);
        assert_eq!(output.value_tag(), output.value.canonical_tag());
        assert_eq!(
            output.claim_id,
            derive(Rule::QuarterSum, &[q1, q2]).unwrap().claim_id
        );
    }

    #[test]
    fn addition_propagates_rounding_and_keeps_zero_as_a_real_observation() {
        let q1 = amount(1, 3, 100);
        let q2 = amount(4, 6, 150);
        let result = derive(Rule::QuarterSum, &[q2.clone(), q1.clone()]).unwrap();
        assert_eq!(result.value.interval.lo, Rat::from_integer(249));
        assert_eq!(result.value.interval.hi, Rat::from_integer(251));
        assert_eq!(
            result.value.period,
            Period {
                year: 2030,
                first_month: 1,
                last_month: 6
            }
        );
        assert_eq!(result.parents.len(), 2);
        assert_ne!(result.evidence, EvidenceClass::Measured);
        assert_eq!(
            result.claim_id,
            derive(Rule::QuarterSum, &[q1, q2]).unwrap().claim_id
        );
        let zero = derive(Rule::QuarterSum, &[amount(1, 3, 0), amount(4, 6, 0)]).unwrap();
        assert_eq!(zero.value.interval.lo, Rat::from_integer(-1));
        assert_eq!(zero.value.interval.hi, Rat::from_integer(1));
    }

    #[test]
    fn quarter_sums_require_complete_disjoint_half_or_full_year() {
        let qs: Vec<_> = (0..4).map(|q| amount(q * 3 + 1, q * 3 + 3, 100)).collect();
        assert_eq!(
            derive(Rule::QuarterSum, &qs[..2])
                .unwrap()
                .value
                .interval
                .lo,
            Rat::from_integer(199)
        );
        let fy = derive(Rule::QuarterSum, &qs).unwrap();
        assert_eq!(
            fy.value.interval,
            Interval {
                lo: Rat::from_integer(398),
                hi: Rat::from_integer(402),
                lo_strict: false,
                hi_strict: false
            }
        );
        let mut reverse = qs.clone();
        reverse.reverse();
        assert_eq!(
            fy.claim_id,
            derive(Rule::QuarterSum, &reverse).unwrap().claim_id
        );
        let nine = derive(Rule::QuarterSum, &qs[..3]).unwrap();
        assert_eq!(nine.value.period.first_month, 1);
        assert_eq!(nine.value.period.last_month, 9);
        assert_eq!(nine.value.interval.lo, Rat::new(597, 2).unwrap());
        assert_eq!(nine.value.interval.hi, Rat::new(603, 2).unwrap());
        assert_eq!(nine.parents.len(), 3);
        assert_ne!(nine.claim_id, fy.claim_id);
        assert_eq!(
            nine.claim_id,
            derive(Rule::QuarterSum, &reverse[1..]).unwrap().claim_id
        );
        for invalid in [&qs[..1], &qs[1..], &qs[1..3]] {
            assert!(derive(Rule::QuarterSum, invalid).is_err());
        }
        assert!(derive(Rule::QuarterSum, &[qs[0].clone(), qs[0].clone()]).is_err());
        for invalid in [
            vec![qs[0].clone(), qs[0].clone(), qs[2].clone()],
            vec![qs[0].clone(), qs[1].clone(), qs[3].clone()],
        ] {
            assert!(derive(Rule::QuarterSum, &invalid).is_err());
        }
    }

    #[test]
    fn altered_source_and_nonadditive_or_unknown_evidence_refuse() {
        let q1 = amount(1, 3, 100);
        let q2 = amount(4, 6, 250);
        let mut altered = q1.clone();
        altered.value.subject = "Another site".into();
        assert_eq!(
            derive(Rule::QuarterSum, &[altered, q2.clone()])
                .unwrap_err()
                .code,
            "FMINE-REF-507"
        );
        for change in [0, 1, 2] {
            let mut bad = q1.clone();
            match change {
                0 => bad.additivity = AdditivityClass::IntensiveWeighted,
                1 => bad.state = KnowledgeState::Unknown,
                _ => bad.stale = true,
            }
            // Rebuild identity: refusal must check admission, not merely detect an old Claim id.
            bad = Claim::new(
                INPUT_METRIC,
                bad.value.clone(),
                &bad.value.canonical_tag(),
                bad.representation,
                bad.support,
                bad.additivity,
                bad.evidence,
                bad.epistemic,
                bad.state,
                bad.confidentiality,
                bad.uu_terms,
                bad.parents,
                bad.anchors,
            );
            if change == 2 {
                bad.stale = true;
            }
            assert!(derive(Rule::QuarterSum, &[bad, q2.clone()]).is_err());
        }
    }

    #[test]
    fn different_basis_unit_scope_and_year_do_not_mix() {
        let q1 = amount(1, 3, 100);
        for change in 0..5 {
            let mut h1 = amount(4, 6, 250);
            match change {
                0 => {
                    h1.value.ownership = Ownership::Attributable {
                        share: Rat::new(4, 5).unwrap(),
                    }
                }
                1 => h1.value.kind = AmountKind::CopperProductionKt,
                2 => h1.value.scope = "region-version-2".into(),
                3 => h1.value.period.year = 2031,
                _ => h1.value.subject = "Site B".into(),
            }
            h1 = Claim::new(
                INPUT_METRIC,
                h1.value.clone(),
                &h1.value.canonical_tag(),
                h1.representation,
                h1.support,
                h1.additivity,
                h1.evidence,
                h1.epistemic,
                h1.state,
                h1.confidentiality,
                h1.uu_terms,
                h1.parents,
                h1.anchors,
            );
            assert!(derive(Rule::QuarterSum, &[q1.clone(), h1]).is_err());
        }
    }
}

fn gold_sales_cost_sheet(subject: &str, sold: &str, cost: &str) -> MinimalSheet {
    let text = |s: &str| MinimalCell::Text(s.into());
    MinimalSheet {
        name: "Minestats1".into(),
        hidden: false,
        cells: vec![
            ("B1".into(), text(subject)),
            ("A2".into(), text("For the three months ended March 31,")),
            ("B2".into(), MinimalCell::Number(2030.0)),
            ("A3".into(), text("Sales (thousands of ounces)")),
            ("B3".into(), number(sold)),
            ("A4".into(), text("Costs ($ millions)")),
            ("A5".into(), text("Cost of sales")),
            ("B5".into(), number(cost)),
        ],
    }
}

fn gold_cost_source(root: &Path, file: &str, date: Option<&str>, sheet: MinimalSheet) -> String {
    let hash = source(root, file, "2030Q1", date, "0.00", &[sheet]);
    let path = registry_path(root, DERIVATION_ISSUER);
    let mut registry = std::fs::read_to_string(&path).unwrap();
    if !registry.contains("[[mine_stats_label]]") {
        registry.push_str(
            "\n[[mine_stats_label]]\nlabel = \"Sales (thousands of ounces)\"\n\
             [[mine_stats_label]]\nsection = \"Costs ($ millions)\"\nlabel = \"Cost of sales\"\n\
             [[mine_stats_label]]\nsection = \"Costs per ounce\"\nlabel = \"Cost of sales\"\n",
        );
        std::fs::write(path, registry).unwrap();
    }
    hash
}

#[test]
fn gold_cost_of_sales_derives_exact_source_bound_enclosures_on_both_printed_ownership_bases() {
    for (subject, ownership) in [
        ("Example Mine (60%)", "attributable"),
        ("Example Mine (100%)", "100% basis"),
    ] {
        let root = derivation_workspace();
        let hash = gold_cost_source(
            &root,
            "GoldCosts.xlsx",
            Some("2030-05-01"),
            gold_sales_cost_sheet(subject, "100.004", "150.004"),
        );
        let draft = format!(
            "Example Mine Q1 2030 {ownership} cost of sales per ounce sold is $1.5k/oz."
        );
        let report = assess(&root, &draft, "2030-05-01");
        let rate = cost_rate_report(&report);
        assert!(rate.starts_with('{'), "{}", report.to_json());
        assert_eq!(string_field(rate, "status"), "derived");
        assert_eq!(string_field(rate, "family"), "cos_per_oz");
        // 1000 * [149.995, 150.005] / [99.995, 100.005], independently computed.
        assert!(rate.contains("\"lo\":{\"num\":\"29999000\",\"den\":\"20001\"}"));
        assert!(rate.contains("\"hi\":{\"num\":\"30001000\",\"den\":\"19999\"}"));
        for expected in [
            "1000*N/Q",
            "USD/oz",
            "GoldCosts.xlsx:Minestats1!B5",
            "GoldCosts.xlsx:Minestats1!B3",
            &hash,
            "\"stored\":\"150.004\",\"displayed\":\"150.00\"",
            "Assertion { source: Issuer }",
            "not make the derived figure an issuer disclosure",
        ] {
            assert!(rate.contains(expected), "missing {expected}: {rate}");
        }
        assert!(report.payload_json.contains("\"status\":\"derivable\""));
        assert!(!report.payload_json.contains("\"status\":\"disclosed\""));
        assert_eq!(report, assess(&root, &draft, "2030-05-01"));
        for (amount, expected) in [
            ("$1500.00/oz", "precision_not_established"),
            ("$2.0k/oz", "contradicted"),
        ] {
            let changed = assess(&root, &draft.replace("$1.5k/oz", amount), "2030-05-01");
            assert_eq!(string_field(cost_rate_report(&changed), "status"), expected);
        }
        let before = assess(&root, &draft, "2030-04-30");
        assert_eq!(
            string_field(cost_rate_report(&before), "status"),
            "not_available"
        );
        for (sold, cost) in [("100.004", "160.004"), ("120.004", "150.004")] {
            let changed = derivation_workspace();
            gold_cost_source(
                &changed,
                "GoldCosts.xlsx",
                Some("2030-05-01"),
                gold_sales_cost_sheet(subject, sold, cost),
            );
            let changed = assess(&changed, &draft, "2030-05-01");
            assert_eq!(string_field(cost_rate_report(&changed), "status"), "contradicted");
            assert_ne!(report.payload_json, changed.payload_json);
        }
    }
}

#[test]
fn gold_cost_of_sales_preserves_signed_costs_and_direct_disclosure_precedence() {
    for (cost, amount, lo, hi) in [
        ("-150.004", "-$1.5k/oz", "-30001000", "-29999000"),
        ("0", "$0.0k/oz", "-1000", "1000"),
    ] {
        let root = derivation_workspace();
        gold_cost_source(
            &root,
            "Signed.xlsx",
            Some("2030-05-01"),
            gold_sales_cost_sheet("Example Mine (60%)", "100.004", cost),
        );
        let draft =
            format!("Example Mine Q1 2030 attributable cost of sales per ounce sold is {amount}.");
        let report = assess(&root, &draft, "2030-05-01");
        let rate = cost_rate_report(&report);
        assert_eq!(string_field(rate, "status"), "derived");
        assert!(rate.contains(&format!("\"lo\":{{\"num\":\"{lo}\",\"den\":\"19999\"}}")));
        let upper_den = if cost == "0" { "19999" } else { "20001" };
        assert!(rate.contains(&format!("\"hi\":{{\"num\":\"{hi}\",\"den\":\"{upper_den}\"}}")));
    }
    for (printed, status) in [("1500", "disclosed"), ("2000", "contradicts_disclosed")] {
        let root = derivation_workspace();
        let mut sheet = gold_sales_cost_sheet("Example Mine (60%)", "100", "150");
        sheet.cells.extend([
            ("A6".into(), MinimalCell::Text("Costs per ounce".into())),
            ("A7".into(), MinimalCell::Text("Cost of sales".into())),
            ("B7".into(), number(printed)),
        ]);
        gold_cost_source(&root, "Direct.xlsx", Some("2030-05-01"), sheet);
        let report = assess(
            &root,
            "Example Mine Q1 2030 attributable cost of sales per ounce sold is $1.5k/oz.",
            "2030-05-01",
        );
        assert_eq!(string_field(cost_rate_report(&report), "status"), "derived");
        assert!(report.payload_json.contains(&format!("\"status\":\"{status}\"")));
        assert!(!report.payload_json.contains("\"status\":\"derivable\""));
    }
}

#[test]
fn gold_cost_of_sales_never_infers_sources_ownership_or_sold_units() {
    let draft = "Example Mine Q1 2030 attributable cost of sales per ounce sold is $1.5k/oz.";
    let root = derivation_workspace();
    gold_cost_source(
        &root,
        "Gold.xlsx",
        Some("2030-05-01"),
        gold_sales_cost_sheet("Example Mine (60%)", "100", "150"),
    );
    for unsupported in [
        draft.replace("sold", "produced"),
        draft.replace("ounce", "pound").replace("/oz", "/lb"),
        draft.replace("attributable", "100% basis"),
        draft.replace("Q1", "Q2"),
        draft.replace("cost of sales", "co-product cost of sales"),
    ] {
        let report = assess(&root, &unsupported, "2030-05-01");
        assert!(
            !report.payload_json.contains("\"status\":\"derived\""),
            "{}",
            report.to_json()
        );
    }
    for (subject, sold, date, expected) in [
        ("Example Mine", "100", Some("2030-05-01"), "null"),
        ("Example Mine (60%)", "0", Some("2030-05-01"), "refused"),
        ("Example Mine (60%)", "-100", Some("2030-05-01"), "refused"),
        ("Example Mine (60%)", "100", None, "not_available"),
    ] {
        let root = derivation_workspace();
        gold_cost_source(&root, "Gold.xlsx", date, gold_sales_cost_sheet(subject, sold, "150"));
        let report = assess(&root, draft, "2030-05-01");
        let rate = cost_rate_report(&report);
        if expected == "null" {
            assert!(rate.starts_with("null"));
        } else {
            assert_eq!(string_field(rate, "status"), expected, "{rate}");
        }
        assert!(!report.payload_json.contains("\"status\":\"derivable\""));
    }
    let separate = derivation_workspace();
    let mut cost = gold_sales_cost_sheet("Example Mine (60%)", "100", "150");
    cost.cells.retain(|(at, _)| !["A3", "B3"].contains(&at.as_str()));
    gold_cost_source(&separate, "Costs.xlsx", Some("2030-05-01"), cost);
    let mut sales = gold_sales_cost_sheet("Example Mine (60%)", "100", "150");
    sales.cells.retain(|(at, _)| !["A4", "A5", "B5"].contains(&at.as_str()));
    gold_cost_source(&separate, "Sales.xlsx", Some("2030-05-01"), sales);
    let report = assess(&separate, draft, "2030-05-01");
    assert_eq!(string_field(cost_rate_report(&report), "status"), "refused");
    assert!(cost_rate_report(&report).contains("FMINE-REF-105"));
    let competing = derivation_workspace();
    for (file, cost) in [("One.xlsx", "150"), ("Two.xlsx", "200")] {
        gold_cost_source(
            &competing,
            file,
            Some("2030-05-01"),
            gold_sales_cost_sheet("Example Mine (60%)", "100", cost),
        );
    }
    let report = assess(&competing, draft, "2030-05-01");
    assert_eq!(string_field(cost_rate_report(&report), "status"), "ambiguous");
}
