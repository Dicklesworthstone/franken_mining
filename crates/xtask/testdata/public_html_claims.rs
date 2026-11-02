//! Invented, source-bound filings. These tests establish extraction behavior, not issuer corpus coverage.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_core::hash::sha256;
use fmine_public::html::{HtmlClaimsReport, run};
use fmine_public::key::{NilReason, OwnershipBasis, PublicValue, ShareSource, StatusKind};
use fmine_registry::sources::parse_sources_registry;

const HTML: &str = "<html><body><table>\n\
<tr><th colspan=\"4\">Site A</th></tr>\n\
<tr><th colspan=\"4\">Attributable (60%)</th></tr>\n\
<tr><th>Metric</th><th>2025Q1</th><th>2025Q2</th><th>Unit</th></tr>\n\
<tr><th>AISC</th><td>10.00</td><td>9,007,199,254,740,993.25</td><th>US$/oz sold</th></tr>\n\
<tr><th>TCC</th><td>&mdash;</td><td>n/a</td><th>US$/oz sold</th></tr>\n\
<tr><th>Gold-equivalent production</th><td>321.00</td><td>1,234.50</td><th>koz</th></tr>\n\
<tr><th>Head grade</th><td>1.20</td><td>4.50</td><th>g/t</th></tr>\n\
<tr><th>Recovery</th><td></td><td>0</td><th>%</th></tr>\n\
</table></body></html>";

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "fmine-html-core-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("registries/sources")).unwrap();
        fs::create_dir_all(root.join("corpus")).unwrap();
        let fixture = Self { root };
        fixture.write_source(HTML, "");
        fs::write(fixture.root.join("mappings.toml"), mapping_rows()).unwrap();
        fixture
    }

    fn write_source(&self, source: &str, extra_registry: &str) {
        fs::write(self.root.join("corpus/filing.html"), source).unwrap();
        fs::write(
            self.root.join("registries/sources/html_example.toml"),
            registry(source, extra_registry),
        )
        .unwrap();
    }

    fn run(&self) -> HtmlClaimsReport {
        run(
            &self.root,
            "html_example",
            std::path::Path::new("mappings.toml"),
            None,
        )
        .unwrap()
    }
}

fn registry(source: &str, extra: &str) -> String {
    format!(
        "schema = \"fmine.sources.v1\"\nissuer = \"html_example\"\ncorpus_dir = \"corpus\"\n\
        retrieved_utc = \"2025-08-01T00:00:00Z\"\nurl_prefix = \"https://example.org/\"\n\
        [[subject]]\nname = \"Site A\"\n\
        [[html]]\nfile = \"filing.html\"\nrelease = \"2025Q2\"\nurl = \"https://example.org/filing.html\"\n\
        url_basis = \"fetched and verified 2025-08-01\"\npublished_at = \"2025-07-31\"\n\
        published_at_basis = \"FACT: invented fixture publication label\"\nbytes = {}\nsha256 = \"{}\"\n{extra}",
        source.len(),
        sha256(source.as_bytes()).to_hex()
    )
}

fn mapping_rows() -> String {
    let mut text = "schema = \"fmine.html-claims.v1\"\nissuer = \"html_example\"\n".to_string();
    for (row, metric, unit) in [
        (4, "AISC", "US$/oz sold"),
        (5, "TCC", "US$/oz sold"),
        (6, "Gold-equivalent production", "koz"),
        (7, "Head grade", "g/t"),
        (8, "Recovery", "%"),
    ] {
        for (column, period) in [("B", "2025Q1"), ("C", "2025Q2")] {
            text.push_str(&format!("\n[[claim]]\nid = \"r{row}_{column}\"\nfile = \"filing.html\"\ntable = 1\ncell = \"{column}{row}\"\n\
                subject_cell = \"B1\"\nsubject_label = \"Site A\"\nperiod_cell = \"{column}3\"\nperiod_label = \"{period}\"\n\
                metric_cell = \"A{row}\"\nmetric_label = \"{metric}\"\nunit_cell = \"D{row}\"\nunit_label = \"{unit}\"\n\
                ownership_cell = \"D2\"\nownership_label = \"Attributable (60%)\"\n"));
        }
    }
    text
}

#[test]
fn all_five_metrics_become_exact_cited_claims_with_distinct_placeholders() {
    let fixture = Fixture::new();
    let report = fixture.run();
    assert_eq!(report.claims.len(), 10);
    assert_eq!(report.claims_digest, fixture.run().claims_digest);
    let claim = |id: &str| {
        report
            .claims
            .iter()
            .find(|claim| claim.definition_id == id)
            .unwrap()
    };
    let large = claim("r4_C");
    let exact = large.exact_number.as_ref().unwrap();
    assert_eq!(
        (exact.value.mantissa, exact.value.scale),
        (900719925474099325, 2)
    );
    assert_eq!((exact.step.mantissa, exact.step.scale), (1, 2));
    assert_eq!(large.value_source.text, "9,007,199,254,740,993.25");
    let (start, end) = large.value_source.byte_range;
    assert_eq!(&HTML[start..end], "9,007,199,254,740,993.25");
    assert_eq!(large.cell.key.valid_period.code(), "2025Q2");
    assert_eq!(
        large.cell.key.ownership_basis,
        OwnershipBasis::Attributable {
            num: 3,
            den: 5,
            source: ShareSource::Printed
        }
    );
    assert_eq!(
        large.interpretations[0].source.requested_locator,
        "Table1!B1"
    );
    assert_eq!(large.interpretations[0].source.anchor.locator, "Table1!A1");
    assert_eq!(large.interpretations[4].source.anchor.locator, "Table1!A2");
    assert!(large.cell.claim.anchors.contains(&large.mapping_source));
    assert_eq!(
        large.mapping_source.artifact_sha256,
        sha256(mapping_rows().as_bytes()).0
    );
    assert!(
        large
            .cell
            .claim
            .anchors
            .iter()
            .any(|anchor| anchor.artifact_sha256 == sha256(registry(HTML, "").as_bytes()).0)
    );
    assert!(matches!(
        claim("r5_B").cell.claim.value,
        PublicValue::Nil {
            reason: NilReason::Unknown,
            ..
        }
    ));
    assert!(matches!(
        claim("r5_C").cell.claim.value,
        PublicValue::NotApplicable { .. }
    ));
    assert!(matches!(
        claim("r8_B").cell.claim.value,
        PublicValue::Missing
    ));
    assert!(matches!(
        claim("r8_C").cell.claim.value,
        PublicValue::Number { .. }
    ));
    assert_eq!(
        claim("r6_C").cell.key.line.label,
        "html:gold_equivalent_production_koz"
    );
    assert_eq!(claim("r7_B").cell.key.unit, "g_per_t");
}

#[test]
fn registered_company_totals_do_not_become_site_claims() {
    let fixture = Fixture::new();
    for total in ["Total Gold", "Total Copper"] {
        let source = HTML.replace("Site A", total);
        fs::write(fixture.root.join("corpus/filing.html"), &source).unwrap();
        fs::write(
            fixture.root.join("registries/sources/html_example.toml"),
            registry(&source, "").replace("Site A", total),
        )
        .unwrap();
        fs::write(
            fixture.root.join("mappings.toml"),
            mapping_rows().replace("Site A", total),
        )
        .unwrap();
        let refusal = run(
            &fixture.root,
            "html_example",
            std::path::Path::new("mappings.toml"),
            None,
        )
        .unwrap_err();
        assert!(
            refusal.what.contains("not a region or company total"),
            "{total}"
        );
    }
}

#[test]
fn registered_status_changes_zero_to_nil_without_losing_the_printed_number() {
    let fixture = Fixture::new();
    fixture.write_source(HTML, "\n[[subject_status]]\nsubject = \"Site A\"\nstatus = \"withheld\"\nfirst = \"2025Q2\"\nevidence = \"invented source notice\"\n");
    let report = fixture.run();
    let zero = report
        .claims
        .iter()
        .find(|claim| claim.definition_id == "r8_C")
        .unwrap();
    assert!(matches!(
        zero.cell.claim.value,
        PublicValue::Nil {
            reason: NilReason::Withheld,
            ..
        }
    ));
    assert_eq!(zero.exact_number.as_ref().unwrap().value.mantissa, 0);
    assert_eq!(
        zero.cell.status_evidence.as_deref(),
        Some("invented source notice")
    );
    let positive = report
        .claims
        .iter()
        .find(|claim| claim.definition_id == "r4_C")
        .unwrap();
    assert!(matches!(
        positive.cell.claim.value,
        PublicValue::Number {
            under_status: Some(StatusKind::Withheld),
            ..
        }
    ));
}

#[test]
fn mappings_commit_interpretation_bytes_and_cannot_supply_a_replacement_amount() {
    let fixture = Fixture::new();
    let original = fixture.run();
    fs::write(
        fixture.root.join("mappings.toml"),
        format!("# a new reviewed mapping\n{}", mapping_rows()),
    )
    .unwrap();
    let changed = fixture.run();
    assert_ne!(original.definitions_sha256, changed.definitions_sha256);
    assert_ne!(
        original.claims[0].cell.claim.claim_id,
        changed.claims[0].cell.claim.claim_id
    );
    assert_eq!(
        original.claims[0].exact_number,
        changed.claims[0].exact_number
    );
    fs::write(
        fixture.root.join("mappings.toml"),
        mapping_rows().replacen("cell = \"B4\"", "cell = \"B4\"\nvalue = \"999\"", 1),
    )
    .unwrap();
    let refusal = run(
        &fixture.root,
        "html_example",
        std::path::Path::new("mappings.toml"),
        None,
    )
    .unwrap_err();
    assert!(refusal.what.contains("unknown field value"));
}

#[test]
fn labels_must_both_match_the_bytes_and_support_the_declared_semantics() {
    let fixture = Fixture::new();
    for (old, new) in [
        ("AISC", "potatoes"),
        ("US$/oz sold", "US$/oz produced"),
        ("Attributable (60%)", "60% probably"),
        ("2025Q1", "next quarter"),
    ] {
        fixture.write_source(&HTML.replace(old, new), "");
        fs::write(
            fixture.root.join("mappings.toml"),
            mapping_rows().replace(old, new),
        )
        .unwrap();
        assert!(
            run(
                &fixture.root,
                "html_example",
                std::path::Path::new("mappings.toml"),
                None
            )
            .is_err(),
            "{new}"
        );
    }
    fixture.write_source(HTML, "");
    fs::write(
        fixture.root.join("mappings.toml"),
        mapping_rows().replacen(
            "subject_label = \"Site A\"",
            "subject_label = \"Site B\"",
            1,
        ),
    )
    .unwrap();
    assert!(
        run(
            &fixture.root,
            "html_example",
            std::path::Path::new("mappings.toml"),
            None
        )
        .unwrap_err()
        .what
        .contains("subject label")
    );
}

#[test]
fn a_merged_data_origin_cannot_be_imported_as_two_independent_observations() {
    let fixture = Fixture::new();
    fixture.write_source(
        &HTML.replace(
            "<td>321.00</td><td>1,234.50</td>",
            "<td colspan=\"2\">321.00</td>",
        ),
        "",
    );
    let refusal = run(
        &fixture.root,
        "html_example",
        std::path::Path::new("mappings.toml"),
        None,
    )
    .unwrap_err();
    assert!(
        refusal
            .what
            .contains("one source-cell origin more than once")
    );
}

#[test]
fn html_registry_has_real_publication_metadata_and_no_fake_pdf_pages() {
    let valid = registry(HTML, "");
    let parsed = parse_sources_registry(&valid).unwrap();
    assert!(parsed.sources.is_empty() && parsed.documents.is_empty());
    assert_eq!(parsed.html_sources[0].dir, "corpus");
    assert_eq!(parsed.html_sources[0].release, "2025Q2");
    for changed in [
        valid.replace("release = \"2025Q2\"", "release = \"2025FY\""),
        valid.replace("2025-07-31", "2025-02-30"),
        valid.replace("FACT: invented fixture publication label", "FACT: "),
        format!("{valid}pages = 1\n"),
        format!("{valid}unknown = \"x\"\n"),
    ] {
        assert!(parse_sources_registry(&changed).is_err());
    }
    let duplicate = format!(
        "{valid}\n[[document]]\nfile = \"other.pdf\"\nkind = \"mda\"\nurl = \"https://example.org/other.pdf\"\nurl_basis = \"W1 table\"\npublished_at = \"2025-08-01\"\npublished_at_basis = \"FACT: fixture\"\npages = 1\nbytes = 1\nsha256 = \"{}\"\n",
        sha256(HTML.as_bytes()).to_hex()
    );
    assert!(
        parse_sources_registry(&duplicate)
            .unwrap_err()
            .reason
            .contains("duplicate sha256")
    );
}
