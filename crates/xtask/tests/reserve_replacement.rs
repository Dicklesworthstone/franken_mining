//! Synthetic declared reserve ratios through the existing registry, admitted-reading and exact solver path.
//! These invented readings are not a real-PDF gate; CLI acceptance separately authenticates fixture bytes.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use fmine_metrics::roll_forward::{self, TableReport};
use fmine_numerics::exact_feas::{Outcome, Rat};
use fmine_registry::metrics::{METRICS_SCHEMA, MetricRegistry};
use fmine_registry::sources::DocumentRow;
use fmine_registry::table_identities::{TableIdentityRegistry, parse_table_identities};
use fmine_registry::transcriptions::{Accepted, Locator, parse_token};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const BASIS: &str = "attributable;contained;metal=gold;ex_ad;deck=none";
const WINDOW: &str = "2028-2030";
const DECLARATIONS: &str = r#"schema = "fmine.table-identities.v1"
issuer = "synthetic"
[[identity]]
id = "replacement"
doc = "Synthetic.pdf"
family = "reserve_replacement"
kind = "reserve_replacement"
total = "replacement_rate"
total_period = "2028-2030"
unit = "percent"
basis = "attributable;contained;metal=gold;ex_ad;deck=none"
construction = "rounded_once"
convention = "closed"
definition = "reserve_replacement.gold_oz@synthetic"
numerator_unit = "moz"
denominator_unit = "moz"
factor = 100
price_deck = "none"
[[term]]
identity = "replacement"
quantity = "net_change"
period = "2028-2030"
sign = 1
role = "numerator"
[[term]]
identity = "replacement"
quantity = "depletion"
period = "2028-2030"
sign = -1
role = "denominator"
"#;

const SUM: &str = r#"
[[identity]]
id = "cumulative_change"
doc = "Synthetic.pdf"
family = "reserve_sum"
kind = "sum"
total = "net_change"
total_period = "2028-2030"
unit = "moz"
basis = "attributable;contained;metal=gold;ex_ad;deck=none"
construction = "rounded_once"
convention = "closed"
[[term]]
identity = "cumulative_change"
quantity = "component_a"
period = "2028-2030"
sign = 1
role = "member"
[[term]]
identity = "cumulative_change"
quantity = "component_b"
period = "2028-2030"
sign = 1
role = "member"
"#;

fn document() -> DocumentRow {
    DocumentRow {
        file: "Synthetic.pdf".into(),
        dir: "synthetic".into(),
        kind: "mda".into(),
        url: "https://example.invalid/synthetic.pdf".into(),
        url_basis: "synthetic fixture".into(),
        published_at: "2031-02-01".into(),
        published_at_basis: "FACT: synthetic fixture".into(),
        pages: 2,
        bytes: 100,
        sha256: "11".repeat(32),
    }
}

fn metrics(definition: &str) -> MetricRegistry {
    // These existing references identify the definition, never the invented numerical inputs.
    let evidence = if definition.contains(".geo@") {
        "E604"
    } else {
        "E603"
    };
    MetricRegistry::parse(&format!(r#"schema = "{METRICS_SCHEMA}"
[[definition]]
id = "{definition}"
family = "reserve_replacement"
unit = "percent"
formula = "three-year cumulative attributable net change excluding A&D divided by cumulative depletion, in the stated metal and deck"
source = "Synthetic reserve table"
evidence = "{evidence}"
"#)).expect("synthetic metric definition")
}

fn registry(text: &str) -> TableIdentityRegistry {
    parse_table_identities(text, "synthetic", &[document()]).expect("explicit declarations")
}

fn reading(quantity: &str, token: &str, unit: &str) -> Accepted {
    Accepted {
        locator: Locator {
            doc: document().file,
            page: 1,
            table: "Invented reserve replacement".into(),
            row: quantity.into(),
            col: WINDOW.into(),
        },
        doc_sha256: document().sha256,
        token: token.into(),
        value: parse_token(token).expect("exact token"),
        quantity: quantity.into(),
        unit: unit.into(),
        basis: BASIS.into(),
        period: WINDOW.into(),
        readers: vec!["reader_a".into(), "reader_b".into()],
    }
}

fn readings() -> Vec<Accepted> {
    vec![
        reading("replacement_rate", "200.0", "percent"),
        reading("net_change", "8.00", "moz"),
        reading("depletion", "(4.00)", "moz"),
    ]
}

fn assess(registry: &TableIdentityRegistry, readings: &[Accepted]) -> TableReport {
    roll_forward::assess_with_metrics(
        registry,
        readings,
        &[document()],
        &metrics("reserve_replacement.gold_oz@synthetic"),
    )
}

#[test]
fn gold_and_geo_ratios_keep_named_definitions_signed_sources_and_exact_witnesses() {
    for geo in [false, true] {
        let mut declaration = DECLARATIONS.to_string();
        let mut data = readings();
        let name = if geo {
            "reserve_replacement.geo@synthetic"
        } else {
            "reserve_replacement.gold_oz@synthetic"
        };
        if geo {
            declaration = declaration
                .replace("gold_oz@", "geo@")
                .replace("metal=gold", "metal=geo")
                .replace("none", "synthetic_deck_1500_3");
            for reading in &mut data {
                reading.basis = reading
                    .basis
                    .replace("metal=gold", "metal=geo")
                    .replace("none", "synthetic_deck_1500_3");
            }
        }
        let registry = registry(&declaration);
        let report =
            roll_forward::assess_with_metrics(&registry, &data, &[document()], &metrics(name));
        assert!(report.refused.is_empty(), "{}", report.to_json());
        assert_eq!(report.components.len(), 1);
        let Outcome::Feasible { witness } = &report.components[0].component.outcome else {
            panic!("{}", report.to_json());
        };
        let amount = |quantity| {
            *witness
                .get(
                    &data
                        .iter()
                        .find(|a| a.quantity == quantity)
                        .unwrap()
                        .locator
                        .to_string(),
                )
                .unwrap()
        };
        let ratio = Rat::from_integer(-100)
            .checked_mul(amount("net_change"))
            .unwrap()
            .checked_div(amount("depletion"))
            .unwrap();
        assert_eq!(amount("replacement_rate"), ratio);
        assert_eq!(report.metric_definitions[0].id, name);
        assert!(
            report
                .cells
                .iter()
                .any(|c| c.source.token == "(4.00)" && c.source.unit == "moz")
        );
        assert!(report.to_json().contains("\"checked\":true"));
        assert_eq!(
            report.digest(),
            roll_forward::assess_with_metrics(
                &registry,
                &data.iter().cloned().rev().collect::<Vec<_>>(),
                &[document()],
                &metrics(name)
            )
            .digest()
        );
        let mut changed_definition = metrics(name);
        changed_definition.definitions[0]
            .source
            .push_str("; amended citation");
        assert_ne!(
            report.digest(),
            roll_forward::assess_with_metrics(&registry, &data, &[document()], &changed_definition)
                .digest()
        );
    }
}

#[test]
fn ratio_and_cumulative_sum_are_checked_jointly_without_reusing_a_rate() {
    let declared = registry(&format!("{DECLARATIONS}{SUM}"));
    let mut data = readings();
    data[1] = reading("net_change", "8", "moz");
    data.extend([
        reading("component_a", "4.20", "moz"),
        reading("component_b", "4.20", "moz"),
    ]);
    for identity in &declared.identities {
        let single = TableIdentityRegistry {
            issuer: declared.issuer.clone(),
            identities: vec![identity.clone()],
            reserve_rows: Vec::new(),
        };
        let report = assess(&single, &data);
        assert!(report.refused.is_empty());
        assert!(
            matches!(
                report.components[0].component.outcome,
                Outcome::Feasible { .. }
            ),
            "{}",
            report.to_json()
        );
    }
    let joint = assess(&declared, &data);
    assert!(joint.refused.is_empty());
    assert_eq!(joint.components.len(), 1);
    assert!(
        matches!(
            joint.components[0].component.outcome,
            Outcome::Infeasible { .. }
        ),
        "{}",
        joint.to_json()
    );
    assert!(joint.components[0].certificate_digest.is_some());
    assert!(joint.to_json().contains("\"checked\":true"));
    data[3] = reading("component_a", "4.00", "moz");
    data[4] = reading("component_b", "4.00", "moz");
    let compatible = assess(&declared, &data);
    assert!(matches!(
        compatible.components[0].component.outcome,
        Outcome::Feasible { .. }
    ));
    assert!(
        compatible.components[0]
            .implied
            .iter()
            .all(|(_, value)| value.is_ok())
    );
    let shared_rate = format!(
        "{DECLARATIONS}{}",
        SUM.replace(
            "quantity = \"component_a\"",
            "quantity = \"replacement_rate\""
        )
    );
    let rejected = parse_table_identities(&shared_rate, "synthetic", &[document()]).unwrap_err();
    assert!(rejected.reason.contains("quoted replacement rate"));
}

#[test]
fn unsupported_ratio_interpretations_and_untrusted_inputs_cannot_verify() {
    for (from, to) in [
        ("factor = 100", "factor = 1"),
        ("factor = 100", ""),
        ("gold_oz@synthetic", "geo@synthetic"),
        ("deck=none", "deck=unprinted"),
        ("attributable;contained", "100%;contained"),
        (";ex_ad;", ";incl_ad;"),
        ("2028-2030", "2029-2030"),
        ("denominator_unit = \"moz\"", "denominator_unit = \"koz\""),
        ("role = \"denominator\"", "role = \"numerator\""),
        (
            "construction = \"rounded_once\"",
            "construction = \"component_sum\"",
        ),
    ] {
        assert!(
            parse_table_identities(&DECLARATIONS.replace(from, to), "synthetic", &[document()])
                .is_err(),
            "{from} -> {to}"
        );
    }
    let declared = registry(DECLARATIONS);
    assert!(
        !roll_forward::assess(&declared, &readings(), &[document()])
            .refused
            .is_empty()
    );
    let mut cases = Vec::new();
    for token in ["0", "4.00"] {
        let mut bad = readings();
        bad[2] = reading("depletion", token, "moz");
        cases.push(bad);
    }
    let mut bad = readings();
    bad[2].basis = bad[2].basis.replace("metal=gold", "metal=geo");
    cases.push(bad);
    let mut bad = readings();
    bad[2].unit = "koz".into();
    cases.push(bad);
    let mut bad = readings();
    bad[2].period = "2029-2031".into();
    cases.push(bad);
    let mut bad = readings();
    bad[2].readers = vec!["reader_a".into()];
    cases.push(bad);
    let mut bad = readings();
    bad[2].doc_sha256 = "22".repeat(32);
    cases.push(bad);
    let mut bad = readings();
    bad[2].token = "(8.00)".into();
    cases.push(bad);
    let mut bad = readings();
    bad.pop();
    cases.push(bad);
    let mut bad = readings();
    bad.push(bad[2].clone());
    cases.push(bad);
    for bad in cases {
        let report = assess(&declared, &bad);
        assert!(!report.refused.is_empty(), "{}", report.to_json());
        assert!(report.components.is_empty());
    }
    let mut wrong = readings();
    wrong[0] = reading("replacement_rate", "250.0", "percent");
    let report = assess(&declared, &wrong);
    assert!(matches!(
        report.components[0].component.outcome,
        Outcome::Infeasible { .. }
    ));
    assert!(report.components[0].certificate_digest.is_some());
}

const PDF: &[u8] = b"%PDF-1.7 invented reserve replacement fixture; not an actual filing";
static WORKSPACE: AtomicU64 = AtomicU64::new(0);

fn fixture(geo: bool, rate: &str, include_pdf: bool, disagree: bool) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-replacement-{}-{}",
        std::process::id(),
        WORKSPACE.fetch_add(1, Ordering::Relaxed)
    ));
    for dir in [
        "documents",
        "registries/sources",
        "registries/transcriptions",
    ] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    if include_pdf {
        std::fs::write(root.join("documents/Synthetic.pdf"), PDF).unwrap();
    }
    let hash = fmine_core::hash::sha256(PDF).to_hex();
    let unused_hash = fmine_core::hash::sha256(b"x").to_hex();
    let source = format!(
        r#"schema = "fmine.sources.v1"
issuer = "reserve_test"
corpus_dir = "corpus"
retrieved_utc = "synthetic fixture"
url_prefix = "https://example.invalid/"
[[source]]
file = "Unused.xlsx"
kind = "mine_stats"
quarter = "2030Q4"
path = "Unused.xlsx"
url_basis = "W1 table"
bytes = 1
sha256 = "{unused_hash}"
[[subject]]
name = "Invented reserve company"
[[document]]
file = "Synthetic.pdf"
dir = "documents"
kind = "aif"
url = "https://example.invalid/Synthetic.pdf"
url_basis = "W1 table"
published_at = "2031-02-01"
published_at_basis = "FACT: synthetic fixture date"
pages = 1
bytes = {size}
sha256 = "{hash}"
"#,
        size = PDF.len()
    );
    std::fs::write(root.join("registries/sources/reserve_test.toml"), source).unwrap();
    // Existing versioned definitions only; no issuer numerical corpus is copied into this fixture.
    std::fs::write(
        root.join("registries/metrics.toml"),
        include_str!("../../../registries/metrics.toml"),
    )
    .unwrap();
    let mut declarations =
        DECLARATIONS.replace("issuer = \"synthetic\"", "issuer = \"reserve_test\"");
    let mut basis = BASIS.to_string();
    if geo {
        declarations = declarations
            .replace("gold_oz@synthetic", "geo@barrick-2026-circular")
            .replace("metal=gold", "metal=geo")
            .replace("none", "synthetic_printed_deck");
        basis = basis
            .replace("metal=gold", "metal=geo")
            .replace("none", "synthetic_printed_deck");
    } else {
        declarations = declarations.replace("gold_oz@synthetic", "gold_oz@barrick-fy2025");
    }
    std::fs::write(root.join("table-identities.toml"), declarations).unwrap();
    let mut entries =
        String::from("schema = \"fmine.transcriptions.v1\"\nissuer = \"reserve_test\"\n");
    for reader in ["reader_a", "reader_b"] {
        for (quantity, value, unit) in [
            ("replacement_rate", rate, "percent"),
            ("net_change", "8.00", "moz"),
            ("depletion", "(4.00)", "moz"),
        ] {
            let token = if disagree && reader == "reader_b" && quantity == "depletion" {
                "(5.00)"
            } else {
                value
            };
            entries.push_str(&format!(
                r#"
[[entry]]
doc = "Synthetic.pdf"
doc_sha256 = "{hash}"
page = 1
table = "Invented reserve replacement"
row = "{quantity}"
col = "{WINDOW}"
token = "{token}"
quantity = "{quantity}"
unit = "{unit}"
basis = "{basis}"
period = "{WINDOW}"
reader = "{reader}"
"#
            ));
        }
    }
    std::fs::write(
        root.join("registries/transcriptions/reserve_test.toml"),
        entries,
    )
    .unwrap();
    root
}

fn invoke(root: &Path, format: &str) -> (fmine_cli::Envelope, fmine_cli::OutputFormat) {
    let args = [
        "check",
        "tables",
        "--issuer",
        "reserve_test",
        "--definitions",
        "table-identities.toml",
        "--format",
        format,
    ]
    .map(str::to_owned);
    fmine_cli::invoke(&args, root)
}

#[test]
fn cli_reads_registered_gold_and_geo_ratios_and_preserves_json_markdown_evidence() {
    for geo in [false, true] {
        let root = fixture(geo, "200.0", true, false);
        let (envelope, format) = invoke(&root, "json");
        assert_eq!(format, fmine_cli::OutputFormat::Json);
        assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
        assert!(envelope.refusal.is_none());
        assert!(envelope.anchor_id.is_none() && envelope.certificate.is_none());
        assert_eq!(envelope.to_json(), invoke(&root, "json").0.to_json());
        let payload = &envelope.payload_json;
        for expected in [
            "\"verdict\":\"feasible\",\"checked\":true",
            "\"refused\":[]",
            "(4.00)",
            "2028-2030",
            "reader_a",
            "reader_b",
            "Synthetic.pdf:p1:Invented reserve replacement:depletion/2028-2030",
        ] {
            assert!(payload.contains(expected), "missing {expected}: {payload}");
        }
        assert!(payload.contains(&fmine_core::hash::sha256(PDF).to_hex()));
        assert!(payload.contains(if geo {
            "reserve_replacement.geo@barrick-2026-circular"
        } else {
            "reserve_replacement.gold_oz@barrick-fy2025"
        }));
        let (markdown, format) = invoke(&root, "markdown");
        assert_eq!(format, fmine_cli::OutputFormat::Markdown);
        assert_eq!(markdown.payload_json, *payload);
        let rendered = markdown.rendered_markdown.unwrap();
        for expected in [
            "Reserve replacement definitions",
            "2028-2030",
            "moz / moz",
            "(4.00)",
            "feasible",
            "Synthetic.pdf",
            "Source:",
        ] {
            assert!(
                rendered.contains(expected),
                "missing {expected}: {rendered}"
            );
        }
        assert!(rendered.contains(if geo {
            "synthetic\\_printed\\_deck"
        } else {
            "gold\\_oz@barrick-fy2025"
        }));
        assert_eq!(
            std::fs::read(root.join("documents/Synthetic.pdf")).unwrap(),
            PDF
        );
    }
    let conflict = fixture(false, "250.0", true, false);
    let (envelope, _) = invoke(&conflict, "json");
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert!(
        envelope
            .payload_json
            .contains("\"verdict\":\"infeasible\",\"checked\":true")
    );
    assert!(envelope.payload_json.contains("\"certificate_digest\":\""));
    assert!(
        invoke(&conflict, "markdown")
            .0
            .rendered_markdown
            .unwrap()
            .contains("infeasible")
    );
}

#[test]
fn cli_missing_altered_pdf_and_disagreeing_readers_refuse_before_ratio_checks() {
    let missing = fixture(false, "200.0", false, false);
    let altered = fixture(false, "200.0", true, false);
    let mut changed = PDF.to_vec();
    changed[0] ^= 1;
    std::fs::write(altered.join("documents/Synthetic.pdf"), changed).unwrap();
    let disagreement = fixture(false, "200.0", true, true);
    for (root, expected_code, expected_exit) in [
        (missing, "FMINE-REF-506", 5),
        (altered, "FMINE-REF-505", 8),
        (disagreement, "FMINE-REF-202", 3),
    ] {
        let (envelope, _) = invoke(&root, "json");
        assert_eq!(envelope.exit_code, expected_exit, "{}", envelope.to_json());
        assert_eq!(envelope.refusal.as_ref().unwrap().code, expected_code);
        assert!(envelope.to_json().contains("Synthetic.pdf"));
        assert!(!envelope.to_json().contains("\"checked\":true"));
    }
}
