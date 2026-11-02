//! Command-level reserve replacement acceptance using invented documents, definitions and readings.
//! Reader roles below are synthetic test inputs, not a claim of human or blind review.

#![cfg(test)]
#![allow(clippy::expect_used, clippy::panic)]

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke, run};
use fmine_core::hash::sha256;
use fmine_public::sources::{load_issuer_profile, registry_path, transcriptions_path};

const ISSUER: &str = "reserve_fixture";
const PDF: &[u8] =
    b"%PDF-1.7 Invented reserve statement; byte verification fixture, not parsed PDF text.";
const DOC: &str = "Reserve.pdf";
const BINDINGS: &str = "replacement.toml";
const GOLD_BASIS: &str = "attributable_gold_ex_ad";
const GEO_BASIS: &str = "attributable_geo_ex_ad";
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
struct Row {
    quantity: String,
    row: String,
    col: String,
    token: &'static str,
    unit: &'static str,
    basis: &'static str,
    period: String,
    reader: &'static str,
}

fn readings() -> Vec<Row> {
    let mut rows = Vec::new();
    for (metal, label, basis, net, total, headline) in [
        (
            "gold",
            "Gold",
            GOLD_BASIS,
            ["6.0", "8.0", "10.0"],
            "24.0",
            "220",
        ),
        (
            "geo",
            "GEO",
            GEO_BASIS,
            ["12.0", "18.0", "24.0"],
            "53.9",
            "450",
        ),
    ] {
        for reader in ["synthetic_reader_a", "synthetic_reader_b"] {
            for (index, year) in (2030..=2032).enumerate() {
                for (role, col, token) in [
                    ("net", "net change", net[index]),
                    ("depletion", "depletion", ["(3.0)", "(3.0)", "(6.0)"][index]),
                ] {
                    rows.push(Row {
                        quantity: format!("{metal}_{role}_{year}"),
                        row: year.to_string(),
                        col: format!("{label} {col}"),
                        token,
                        unit: "Moz",
                        basis,
                        period: year.to_string(),
                        reader,
                    });
                }
            }
            for (role, col, token, unit) in [
                ("net_total", "net change", total, "Moz"),
                ("depletion_total", "depletion", "(12.0)", "Moz"),
                ("headline", "headline", headline, "percent"),
            ] {
                rows.push(Row {
                    quantity: format!("{metal}_{role}"),
                    row: "2030-2032 Total".into(),
                    col: format!("{label} {col}"),
                    token,
                    unit,
                    basis,
                    period: "2030-2032".into(),
                    reader,
                });
            }
        }
    }
    rows
}

fn year_binding(metal: &str, year: u16) -> String {
    format!(
        "\n[[year]]\nseries = \"{metal}\"\nyear = {year}\ndoc = \"{DOC}\"\n\
         net_change = \"{metal}_net_{year}\"\ndepletion = \"{metal}_depletion_{year}\"\n"
    )
}

fn bindings() -> String {
    let mut text = format!("schema = \"fmine.reserve-replacement.v1\"\nissuer = \"{ISSUER}\"\n");
    for (metal, basis, kind) in [
        ("gold", GOLD_BASIS, "approximate"),
        ("geo", GEO_BASIS, "rounded"),
    ] {
        write!(
            text,
            "\n[[series]]\nid = \"{metal}\"\ndefinition = \"replacement.{metal}@fixture\"\n\
             start_year = 2030\nend_year = 2032\nunit = \"Moz\"\nbasis = \"{basis}\"\n\
             depletion_sign = \"negative\"\n"
        )
        .expect("series text");
        for year in 2030..=2032 {
            text.push_str(&year_binding(metal, year));
        }
        write!(
            text,
            "\n[[aggregate]]\nseries = \"{metal}\"\ndoc = \"{DOC}\"\n\
             net_change = \"{metal}_net_total\"\ndepletion = \"{metal}_depletion_total\"\n\
             period = \"2030-2032\"\n\n[[headline]]\nseries = \"{metal}\"\ndoc = \"{DOC}\"\n\
             quantity = \"{metal}_headline\"\nperiod = \"2030-2032\"\nkind = \"{kind}\"\n"
        )
        .expect("aggregate and headline text");
    }
    text
}

fn rules() -> String {
    let mut text = String::from("schema = \"fmine.metrics.v1\"\n");
    for (id, family, unit) in [
        ("replacement.gold@fixture", "reserve_replacement", "percent"),
        ("replacement.geo@fixture", "reserve_replacement", "percent"),
        ("geo.deck@fixture", "geo", "oz"),
    ] {
        write!(
            text,
            "\n[[definition]]\nid = \"{id}\"\nfamily = \"{family}\"\nunit = \"{unit}\"\n\
             formula = \"Invented fixture calculation definition\"\n\
             source = \"Synthetic test definition, not an issuer document\"\nevidence = \"E0\"\n"
        )
        .expect("definition text");
    }
    for (id, metal, basis) in [
        ("gold", "gold", GOLD_BASIS),
        ("geo", "gold_equivalent", GEO_BASIS),
    ] {
        write!(
            text,
            "\n[[reserve_replacement]]\ndefinition = \"replacement.{id}@fixture\"\n\
             metal = \"{metal}\"\nwindow_years = 3\namount_unit = \"Moz\"\nbasis = \"{basis}\"\n\
             excludes_acquisitions_divestments = \"true\"\n"
        )
        .expect("rule text");
        if id == "geo" {
            text.push_str("geo_definition = \"geo.deck@fixture\"\n");
        }
    }
    for (year, gold, copper) in [
        (2030, "2000", "4.00"),
        (2031, "2100", "4.20"),
        (2032, "2200", "4.40"),
    ] {
        write!(
            text,
            "\n[[geo_deck]]\ndefinition = \"geo.deck@fixture\"\nyear = {year}\n\
             gold_usd_per_oz = \"{gold}\"\ncopper_usd_per_lb = \"{copper}\"\n"
        )
        .expect("deck text");
    }
    text
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(tag: &str, document_present: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "fmine-replacement-{tag}-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("new isolated fixture directory");
        for dir in ["docs", "registries/sources", "registries/transcriptions"] {
            fs::create_dir_all(root.join(dir)).expect("fixture directory");
        }
        if document_present {
            fs::write(root.join("docs").join(DOC), PDF).expect("invented document");
        }
        fs::write(registry_path(&root, ISSUER), format!(
            "schema = \"fmine.sources.v1\"\nissuer = \"{ISSUER}\"\ncorpus_dir = \"docs\"\n\
             retrieved_utc = \"2033-02-01T00:00:00Z\"\nurl_prefix = \"https://example.invalid/\"\n\
             [[subject]]\nname = \"Invented Reserve Issuer\"\n\
             [[document]]\nfile = \"{DOC}\"\nkind = \"mda\"\nurl = \"https://example.invalid/{DOC}\"\n\
             url_basis = \"W1 table\"\npublished_at = \"2033-02-01\"\n\
             published_at_basis = \"FACT: invented test publication only\"\n\
             pages = 4\nbytes = {}\nsha256 = \"{}\"\n", PDF.len(), sha256(PDF).to_hex()
        )).expect("document-only source profile");
        fs::write(root.join(BINDINGS), bindings()).expect("source selections");
        fs::write(root.join("registries/metrics.toml"), rules()).expect("invented metric rules");
        let fixture = Self { root };
        fixture.write_readings(&readings());
        fixture
    }

    fn write_readings(&self, rows: &[Row]) {
        let mut text = format!("schema = \"fmine.transcriptions.v1\"\nissuer = \"{ISSUER}\"\n");
        for row in rows {
            write!(
                text,
                "\n[[entry]]\ndoc = \"{DOC}\"\ndoc_sha256 = \"{}\"\npage = 2\n\
                 table = \"Invented reserve replacement\"\nrow = \"{}\"\ncol = \"{}\"\n\
                 token = \"{}\"\nquantity = \"{}\"\nunit = \"{}\"\nbasis = \"{}\"\n\
                 period = \"{}\"\nreader = \"{}\"\n",
                sha256(PDF).to_hex(),
                row.row,
                row.col,
                row.token,
                row.quantity,
                row.unit,
                row.basis,
                row.period,
                row.reader
            )
            .expect("synthetic reader entry");
        }
        fs::write(transcriptions_path(&self.root, ISSUER), text).expect("synthetic reader streams");
    }

    fn args(extra: &[&str]) -> Vec<String> {
        [
            "reserve",
            "replacement",
            "--issuer",
            ISSUER,
            "--bindings",
            BINDINGS,
        ]
        .into_iter()
        .chain(extra.iter().copied())
        .map(str::to_string)
        .collect()
    }

    fn invoke(&self, extra: &[&str]) -> Envelope {
        invoke(&Self::args(extra), &self.root).0
    }
}

// These helpers select stable named fields in the command's controlled fixture output, not arbitrary JSON.
fn field<'a>(json: &'a str, name: &str) -> &'a str {
    json.split_once(&format!("\"{name}\":\""))
        .expect("named string field")
        .1
        .split('"')
        .next()
        .expect("string value")
}

fn series<'a>(json: &'a str, id: &str) -> &'a str {
    let at = json
        .find(&format!("\"selection\":{{\"id\":\"{id}\""))
        .expect("retained series");
    let tail = &json[at..];
    tail.split_once("},{\"state\":")
        .map_or(tail, |(selected, _)| selected)
}

fn ratio(point: (i128, i128), lo: (i128, i128), hi: (i128, i128)) -> String {
    let rat = |(num, den)| format!("{{\"num\":\"{num}\",\"den\":\"{den}\"}}");
    format!(
        "\"percent\":{{\"printed\":{},\"rounding\":{{\"lo\":{},\"hi\":{},\"closed\":true}}}}",
        rat(point),
        rat(lo),
        rat(hi)
    )
}

fn ok(envelope: &Envelope) {
    assert_eq!(
        (envelope.status, envelope.exit_code),
        ("ok", 0),
        "{}",
        envelope.to_json()
    );
    assert!(envelope.refusal.is_none());
    assert!(envelope.no_claim.is_some());
}

fn refused(envelope: &Envelope, code: &str, exit: u8) {
    assert_eq!(
        (envelope.status, envelope.exit_code),
        ("refused", exit),
        "{}",
        envelope.to_json()
    );
    assert_eq!(envelope.refusal.as_ref().expect("typed refusal").code, code);
}

#[test]
fn document_only_command_computes_both_bases_and_preserves_separate_printed_totals() {
    let fixture = Fixture::new("positive", true);
    assert!(
        load_issuer_profile(&fixture.root, ISSUER)
            .expect("profile")
            .sources
            .is_empty()
    );
    let envelope = fixture.invoke(&["--as-of", "2033-02-01"]);
    ok(&envelope);
    assert_eq!(envelope.verb, "reserve replacement");
    let json = &envelope.payload_json;
    for fragment in [
        "\"schema\":\"fmine.reserve-replacement.report.v1\"",
        "\"lane\":\"public\"",
        "\"gate_eligible\":false",
        "\"as_of\":\"2033-02-01\"",
        "\"pending_readings\":[]",
    ] {
        assert!(json.contains(fragment), "{fragment}: {json}");
    }
    assert_eq!(json.matches("\"state\":\"computed\"").count(), 2);
    assert!(!json.contains("\"state\":\"refused\""));
    let gold = series(json, "gold");
    let geo = series(json, "geo");
    // Independently calculated: N_gold=24, N_geo=54; D=12, annual half widths=3/20.
    let gold_annual = gold
        .split_once("\"annual\":")
        .expect("annual")
        .1
        .split_once(",\"aggregate\":")
        .expect("aggregate")
        .0;
    let geo_annual = geo
        .split_once("\"annual\":")
        .expect("annual")
        .1
        .split_once(",\"aggregate\":")
        .expect("aggregate")
        .0;
    assert!(
        gold_annual.contains(&ratio((200, 1), (5300, 27), (16100, 79))),
        "{gold_annual}"
    );
    assert!(
        geo_annual.contains(&ratio((450, 1), (35900, 81), (36100, 79))),
        "{geo_annual}"
    );
    assert!(
        gold.contains(&ratio((200, 1), (47900, 241), (48100, 239))),
        "{gold}"
    );
    assert!(
        geo.contains(&ratio((2695, 6), (107700, 241), (107900, 239))),
        "{geo}"
    );
    assert!(geo.contains("\"token\":\"53.9\""));
    assert!(geo.contains("\"printed\":{\"num\":\"54\",\"den\":\"1\"}"));
    assert!(gold.contains("\"state\":\"approximate_unassessed\",\"comparison_interval\":null"));
    assert!(gold.contains("\"token\":\"220\""));
    assert!(geo.contains("\"state\":\"rounded_interval_compatible\""));
    for (year, gold_price) in [(2030, "2000"), (2031, "2100"), (2032, "2200")] {
        assert!(geo.contains(&format!(
            "\"year\":{year},\"gold_usd_per_oz\":\"{gold_price}\""
        )));
    }
    assert!(gold.contains("\"geo_definition\":null"));
    assert!(json.contains("\"token\":\"(3.0)\""));
    assert!(json.contains("\"readers\":[\"synthetic_reader_a\",\"synthetic_reader_b\"]"));
}

#[test]
fn json_and_markdown_are_deterministic_and_cite_the_verified_source() {
    let fixture = Fixture::new("rendering", true);
    let args = Fixture::args(&[]);
    let (envelope, format) = invoke(&args, &fixture.root);
    ok(&envelope);
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(run(&args, &fixture.root), (envelope.to_json(), 0));
    assert_eq!(fixture.invoke(&[]), envelope);
    let sha = sha256(PDF).to_hex();
    let locator = "Reserve.pdf:p2:Invented reserve replacement:2030/Gold net change";
    assert!(envelope.payload_json.contains(locator));
    assert!(envelope.payload_json.contains(&sha));
    let markdown_args = Fixture::args(&["--format", "markdown"]);
    let (markdown_envelope, format) = invoke(&markdown_args, &fixture.root);
    assert_eq!(format, OutputFormat::Markdown);
    assert_eq!(markdown_envelope.payload_json, envelope.payload_json);
    let (markdown, exit) = run(&markdown_args, &fixture.root);
    assert_eq!(exit, 0);
    assert_eq!(run(&markdown_args, &fixture.root), (markdown.clone(), 0));
    for citation in [locator, sha.as_str()] {
        assert!(
            markdown.contains(citation),
            "missing {citation}: {markdown}"
        );
    }
    for flag in [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert!(envelope.to_json().contains(&format!("\"{flag}\":false")));
    }
    assert!(envelope.certificate.is_none() && envelope.anchor_id.is_none());
    let mut rows = readings();
    rows.reverse();
    fixture.write_readings(&rows);
    assert_eq!(fixture.invoke(&[]).payload_json, envelope.payload_json);
}

#[test]
fn exact_binding_and_metric_rule_bytes_are_part_of_the_report_identity() {
    let fixture = Fixture::new("manifest", true);
    let initial = fixture.invoke(&[]);
    ok(&initial);
    assert_eq!(
        field(&initial.payload_json, "bindings_sha256"),
        sha256(bindings().as_bytes()).to_hex()
    );
    assert_eq!(
        field(&initial.payload_json, "rules_sha256"),
        sha256(rules().as_bytes()).to_hex()
    );
    fs::write(
        fixture.root.join(BINDINGS),
        format!("{}\n# explicit source-selection review\n", bindings()),
    )
    .expect("binding byte change");
    let changed_binding = fixture.invoke(&[]);
    ok(&changed_binding);
    assert_ne!(
        field(&initial.payload_json, "digest"),
        field(&changed_binding.payload_json, "digest")
    );
    assert_ne!(
        field(&initial.payload_json, "bindings_sha256"),
        field(&changed_binding.payload_json, "bindings_sha256")
    );
    fs::write(
        fixture.root.join("registries/metrics.toml"),
        format!("{}\n# fixture rule revision\n", rules()),
    )
    .expect("rule byte change");
    let changed_rule = fixture.invoke(&[]);
    ok(&changed_rule);
    assert_ne!(
        field(&changed_binding.payload_json, "digest"),
        field(&changed_rule.payload_json, "digest")
    );
    assert_ne!(
        field(&changed_binding.payload_json, "rules_sha256"),
        field(&changed_rule.payload_json, "rules_sha256")
    );
}

#[test]
fn incompatible_rounded_headline_is_a_cited_finding_without_rewriting_values() {
    let fixture = Fixture::new("headline", true);
    let mut rows = readings();
    for row in &mut rows {
        if row.quantity == "geo_headline" {
            row.token = "900";
        }
    }
    fixture.write_readings(&rows);
    let envelope = fixture.invoke(&[]);
    ok(&envelope);
    let geo = series(&envelope.payload_json, "geo");
    assert!(geo.contains("\"state\":\"incompatible\""));
    assert!(geo.contains("\"token\":\"900\""));
    assert!(geo.contains(&ratio((450, 1), (35900, 81), (36100, 79))));
    assert!(geo.contains(&sha256(PDF).to_hex()));
}

#[test]
fn altered_document_bytes_refuse_before_any_numeric_report() {
    let fixture = Fixture::new("altered", true);
    let mut changed = PDF.to_vec();
    changed[PDF.len() - 1] ^= 1;
    fs::write(fixture.root.join("docs").join(DOC), changed).expect("same-size altered fixture");
    let envelope = fixture.invoke(&[]);
    refused(&envelope, "FMINE-REF-505", 8);
    assert_eq!(envelope.payload_json, "null");
    assert!(
        envelope
            .refusal
            .as_ref()
            .expect("refusal")
            .what
            .contains(DOC)
    );
}

#[test]
fn a_source_registry_cannot_route_another_issuers_readings_under_the_requested_name() {
    let fixture = Fixture::new("cross-issuer", true);
    ok(&fixture.invoke(&[]));
    let other = "other_fixture";
    let original_identity = format!("issuer = \"{ISSUER}\"");
    let other_identity = format!("issuer = \"{other}\"");
    let source_path = registry_path(&fixture.root, ISSUER);
    let sources = fs::read_to_string(&source_path).expect("source registry");
    fs::write(
        &source_path,
        sources.replacen(&original_identity, &other_identity, 1),
    )
    .expect("mismatched internal source issuer");
    // Supply the alternate issuer's complete, agreeing readings. An unchecked identity would
    // successfully compute using these inputs, rather than fail merely because a file is missing.
    let readings = fs::read_to_string(transcriptions_path(&fixture.root, ISSUER))
        .expect("complete original reader streams");
    fs::write(
        transcriptions_path(&fixture.root, other),
        readings.replacen(&original_identity, &other_identity, 1),
    )
    .expect("valid alternate issuer reader streams");
    let envelope = fixture.invoke(&[]);
    refused(&envelope, "FMINE-REF-506", 5);
    assert_eq!(envelope.payload_json, "null");
    assert!(
        envelope
            .refusal
            .as_ref()
            .expect("issuer identity refusal")
            .what
            .contains(&format!(
                "issuer \"{other}\" does not match the file name \"{ISSUER}\""
            ))
    );
}

#[test]
fn disagreement_cannot_be_filtered_out_by_changing_a_readers_quantity_key() {
    for alter_key in [false, true] {
        let fixture = Fixture::new("disagreement", true);
        let mut rows = readings();
        let row = rows
            .iter_mut()
            .find(|row| row.quantity == "gold_net_2030" && row.reader == "synthetic_reader_b")
            .expect("second reader");
        if alter_key {
            row.quantity = "unselected_quantity".into();
        } else {
            row.token = "7.0";
        }
        fixture.write_readings(&rows);
        let envelope = fixture.invoke(&[]);
        refused(&envelope, "FMINE-REF-202", 3);
        assert_eq!(envelope.payload_json, "null");
        let detail = &envelope.refusal.as_ref().expect("refusal").what;
        for expected in [
            "readers disagree",
            "Gold net change",
            "synthetic_reader_a",
            "synthetic_reader_b",
        ] {
            assert!(detail.contains(expected), "{detail}");
        }
    }
}

#[test]
fn missing_and_single_reader_operands_keep_the_whole_selected_series() {
    for single_reader in [false, true] {
        let fixture = Fixture::new("incomplete", true);
        let mut rows = readings();
        rows.retain(|row| {
            row.quantity != "gold_net_2031" || (single_reader && row.reader == "synthetic_reader_a")
        });
        fixture.write_readings(&rows);
        let envelope = fixture.invoke(&[]);
        refused(&envelope, "FMINE-REF-202", 3);
        let json = &envelope.payload_json;
        assert!(
            json.contains("\"state\":\"refused\",\"selection\":{\"id\":\"gold\""),
            "{json}"
        );
        assert!(
            json.contains("\"state\":\"computed\",\"selection\":{\"id\":\"geo\""),
            "{json}"
        );
        assert!(series(json, "gold").contains("no agreed reading for Reserve.pdf/gold_net_2031"));
        for year in [2030, 2031, 2032] {
            assert!(series(json, "gold").contains(&format!("\"year\":{year}")));
        }
        if single_reader {
            assert!(json.contains("\"pending_readings\":[{\"locator\":\"Reserve.pdf:p2:Invented reserve replacement:2031/Gold net change\",\"reader\":\"synthetic_reader_a\"}]"));
            let args = Fixture::args(&["--format", "markdown"]);
            let (markdown_envelope, format) = invoke(&args, &fixture.root);
            assert_eq!(format, OutputFormat::Markdown);
            refused(&markdown_envelope, "FMINE-REF-202", 3);
            assert_eq!(&markdown_envelope.payload_json, json);
            let (markdown, exit) = run(&args, &fixture.root);
            assert_eq!(exit, 3);
            for expected in [
                "Reserve.pdf:p2:Invented reserve replacement:2031/Gold net change",
                "synthetic\\_reader\\_a",
            ] {
                assert!(
                    markdown.contains(expected),
                    "missing {expected}: {markdown}"
                );
            }
        } else {
            assert!(json.contains("\"pending_readings\":[]"));
        }
    }
}

#[test]
fn an_agreed_quantity_with_a_pending_competing_locator_refuses_even_when_values_match() {
    for token in ["6.0", "7.0"] {
        let fixture = Fixture::new("pending-competitor", true);
        let mut rows = readings();
        let mut pending = rows
            .iter()
            .find(|row| row.quantity == "gold_net_2030")
            .expect("agreed annual quantity")
            .clone();
        pending.reader = "synthetic_reader_c";
        pending.row = "Alternate 2030 location".into();
        pending.token = token;
        rows.push(pending);
        fixture.write_readings(&rows);
        let envelope = fixture.invoke(&[]);
        refused(&envelope, "FMINE-REF-202", 3);
        assert_eq!(envelope.payload_json, "null");
    }
}

#[test]
fn pending_quantities_outside_the_selected_document_and_quantity_do_not_block() {
    for other_document in [false, true] {
        let fixture = Fixture::new("unrelated-pending", true);
        let baseline = fixture.invoke(&[]);
        ok(&baseline);
        if other_document {
            // The same quantity in another registered document is a separate selection. Its bytes
            // are deliberately absent: the current binding never asks to inspect that document.
            let other_bytes =
                b"%PDF-1.7 Distinct invented reserve statement in an unselected document.";
            let other_sha256 = sha256(other_bytes).to_hex();
            let source_path = registry_path(&fixture.root, ISSUER);
            let mut sources = fs::read_to_string(&source_path).expect("source registry");
            write!(sources,
                "\n[[document]]\nfile = \"Other.pdf\"\nkind = \"mda\"\n\
                 url = \"https://example.invalid/Other.pdf\"\nurl_basis = \"W1 table\"\n\
                 published_at = \"2033-02-01\"\npublished_at_basis = \"FACT: invented publication\"\n\
                 pages = 4\nbytes = {}\nsha256 = \"{}\"\n",
                other_bytes.len(), other_sha256
            ).expect("unselected document registration");
            fs::write(source_path, sources).expect("extended source registry");
            let path = transcriptions_path(&fixture.root, ISSUER);
            let mut entries = fs::read_to_string(&path).expect("reader registry");
            write!(entries,
                "\n[[entry]]\ndoc = \"Other.pdf\"\ndoc_sha256 = \"{}\"\npage = 2\n\
                 table = \"Invented reserve replacement\"\nrow = \"2030\"\ncol = \"Gold net change\"\n\
                 token = \"7.0\"\nquantity = \"gold_net_2030\"\nunit = \"Moz\"\n\
                 basis = \"{GOLD_BASIS}\"\nperiod = \"2030\"\nreader = \"synthetic_reader_c\"\n",
                other_sha256
            ).expect("unselected document reading");
            fs::write(path, entries).expect("extended reader registry");
        } else {
            let mut rows = readings();
            let mut pending = rows[0].clone();
            pending.reader = "synthetic_reader_c";
            pending.row = "Unrelated 2030 quantity".into();
            pending.quantity = "unselected_quantity".into();
            rows.push(pending);
            fixture.write_readings(&rows);
        }
        let envelope = fixture.invoke(&[]);
        ok(&envelope);
        assert_eq!(envelope.payload_json, baseline.payload_json);
    }
}

#[test]
fn agreed_but_wrong_unit_basis_or_period_remains_a_series_refusal() {
    for mismatch in ["unit", "basis", "period"] {
        let fixture = Fixture::new(mismatch, true);
        let mut rows = readings();
        for row in rows
            .iter_mut()
            .filter(|row| row.quantity == "gold_net_2031")
        {
            match mismatch {
                "unit" => row.unit = "koz",
                "basis" => row.basis = "100_percent_gold",
                _ => row.period = "2030".into(),
            }
        }
        fixture.write_readings(&rows);
        let envelope = fixture.invoke(&[]);
        refused(&envelope, "FMINE-REF-202", 3);
        let gold = series(&envelope.payload_json, "gold");
        assert!(
            gold.contains("unit, metal/ownership basis or period differs"),
            "{gold}"
        );
        assert!(!gold.contains("\"annual\":"));
        assert!(
            envelope
                .payload_json
                .contains("\"state\":\"computed\",\"selection\":{\"id\":\"geo\"")
        );
    }
}

#[test]
fn a_missing_year_binding_cannot_shrink_the_declared_denominator() {
    let fixture = Fixture::new("missing-year", true);
    fs::write(
        fixture.root.join(BINDINGS),
        bindings().replace(&year_binding("gold", 2031), ""),
    )
    .expect("incomplete declaration");
    let envelope = fixture.invoke(&[]);
    refused(&envelope, "FMINE-REF-202", 3);
    assert!(
        envelope
            .refusal
            .as_ref()
            .expect("refusal")
            .what
            .contains("every year")
    );
    assert!(!envelope.payload_json.contains("\"state\":\"computed\""));
}

#[test]
fn prepublication_cut_refuses_before_opening_an_unavailable_document() {
    let fixture = Fixture::new("future-document", false);
    let early = fixture.invoke(&["--as-of", "2033-01-31"]);
    refused(&early, "FMINE-REF-110", 3);
    assert!(
        early
            .refusal
            .as_ref()
            .expect("cut refusal")
            .what
            .contains("2033-02-01")
    );
    assert_eq!(early.payload_json, "null");
    let current = fixture.invoke(&["--as-of", "2033-02-01"]);
    refused(&current, "FMINE-REF-506", 5);
}

#[test]
fn command_rejects_ambiguous_or_unrelated_selectors_before_reading_sources() {
    let fixture = Fixture::new("grammar", false);
    for extras in [
        vec!["--synthetic"],
        vec!["--through", "2032Q4"],
        vec!["--bindings", "other.toml"],
        vec!["--as-of", "2033-02-30"],
        vec!["--issuer", "second_issuer"],
    ] {
        let envelope = fixture.invoke(&extras);
        assert_eq!(
            (envelope.status, envelope.exit_code),
            ("usage_error", 2),
            "{}",
            envelope.to_json()
        );
    }
    for args in [
        vec!["reserve", "replacement", "--issuer", ISSUER],
        vec!["reserve", "replacement", "--bindings", BINDINGS],
    ] {
        let args: Vec<_> = args.into_iter().map(str::to_string).collect();
        let envelope = invoke(&args, &fixture.root).0;
        assert_eq!(
            (envelope.status, envelope.exit_code),
            ("usage_error", 2),
            "{}",
            envelope.to_json()
        );
    }
}
