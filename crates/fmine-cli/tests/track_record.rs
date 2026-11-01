#![cfg(test)]

//! BD-341: source-to-score CLI checks on invented workbooks and two synthetic reader records.
//! The PDF stubs are hash-bound bytes, never evidence that a real document or human reading was checked.

#![allow(clippy::expect_used, clippy::unwrap_used)]

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use fmine_cli::{Envelope, OutputFormat};
    use fmine_core::hash::sha256;
    use fmine_io::xlsx::{MinimalCell, MinimalSheet, build_minimal_xlsx};
    use fmine_public::sources::{load_issuer_profile, registry_path, transcriptions_path};

    const DOCUMENT: &[u8] = b"%PDF-1.7 explicitly synthetic 2023 guidance, never parsed";
    const DOCUMENT_2024: &[u8] = b"%PDF-1.7 explicitly synthetic 2024 guidance, never parsed";
    const TECHNICAL: &[u8] = b"%PDF-1.7 invented optional technical inputs, never parsed";
    const TRACK_MODES: [&[&str]; 2] = [&[], &["--as-of", "2026-02-02"]];
    const ORIGINAL: &str = "Synthetic_2024Q4.xlsx";
    const LATER: &str = "Synthetic_2025Q4.xlsx";
    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn text(cell: &str, value: &str) -> (String, MinimalCell) {
        (cell.into(), MinimalCell::Text(value.into()))
    }

    fn number(cell: &str, value: f64) -> (String, MinimalCell) {
        (cell.into(), MinimalCell::Number(value))
    }

    fn workbook(year: u16, current: f64, prior: f64, attribute: bool) -> Vec<u8> {
        build_minimal_xlsx(&[summary_sheet(year, current, prior, attribute)])
    }

    fn summary_sheet(year: u16, current: f64, prior: f64, attribute: bool) -> MinimalSheet {
        let mut cells = vec![
            text("A1", "Summary of Operations"),
            text("A3", "For the twelve months ended December 31,"),
            number("B3", f64::from(year)),
            number("I3", f64::from(year - 1)),
            text("B4", "Gold produced (000s oz 100%)"),
            text("I4", "Gold produced (000s oz 100%)"),
            text("D4", "All-in sustaining costs ($/oz)"),
            text("K4", "All-in sustaining costs ($/oz)"),
            text("A5", "Site A (75%)"),
            number("B5", 200.0),
            number("I5", 180.0),
            number("D5", 1000.0),
            number("K5", 950.0),
        ];
        if attribute {
            cells.extend([
                text("C4", "Gold produced (000s oz attributable)"),
                text("J4", "Gold produced (000s oz attributable)"),
                number("C5", current),
                number("J5", prior),
            ]);
        }
        MinimalSheet {
            name: "YTD".into(),
            hidden: false,
            cells,
        }
    }

    #[cfg(test)]
    fn install_sources(root: &Path, original: &[u8], later: &[u8]) {
        let mut registry = String::from(
            "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"corpus\"\n\
         retrieved_utc = \"synthetic\"\nurl_prefix = \"https://example.invalid/\"\n",
        );
        for (file, quarter, bytes) in [(ORIGINAL, "2024Q4", original), (LATER, "2025Q4", later)] {
            std::fs::write(root.join("corpus").join(file), bytes).unwrap();
            write!(
                registry,
                "\n[[source]]\nfile = \"{file}\"\nkind = \"mine_stats\"\nquarter = \"{quarter}\"\n\
             path = \"{file}\"\nurl_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{}\"\n",
                bytes.len(),
                sha256(bytes).to_hex(),
            )
            .unwrap();
        }
        registry.push_str("\n[[subject]]\nname = \"Site A\"\n");
        for (year, bytes) in [(2023, DOCUMENT), (2024, DOCUMENT_2024)] {
            write!(
            registry,
            "\n[[document]]\nfile = \"Guidance{year}.pdf\"\n\
             dir = \"documents\"\nkind = \"mda\"\nurl = \"https://example.invalid/Guidance{year}.pdf\"\n\
             url_basis = \"W1 table\"\npublished_at = \"{year}-02-01\"\n\
             published_at_basis = \"FACT: synthetic fixture date\"\npages = 2\nbytes = {}\nsha256 = \"{}\"\n",
            bytes.len(), sha256(bytes).to_hex(),
        ).unwrap();
        }
        std::fs::write(registry_path(root, "synthetic"), registry).unwrap();
    }

    #[cfg(test)]
    fn install_readings(root: &Path, production_basis: &str, disagree: bool, one_reader: bool) {
        let mut readings =
            String::from("schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n");
        for reader in ["reader_a", "reader_b"] {
            if one_reader && reader == "reader_b" {
                continue;
            }
            for (year, metric, unit, expected) in [
                (2023, "production_koz", "koz", "80 - 100"),
                (2024, "production_koz", "koz", "90 - 110"),
                (2024, "aisc_usd_per_oz", "usd_per_oz", "900 - 1100"),
            ] {
                let token = if disagree && reader == "reader_b" && year == 2023 {
                    "81 - 101"
                } else {
                    expected
                };
                let basis = if metric == "production_koz" {
                    production_basis
                } else {
                    "attributable:3/4;aisc=issuer-reported-per-sold-oz"
                };
                write!(
                    readings,
                    "\n[[entry]]\ndoc = \"Guidance{year}.pdf\"\ndoc_sha256 = \"{}\"\npage = 1\n\
                 table = \"Invented guidance\"\nrow = \"Site A {metric}\"\ncol = \"{year}\"\n\
                 token = \"{token}\"\nquantity = \"guidance_site_a_{metric}\"\nunit = \"{unit}\"\n\
                 basis = \"{basis}\"\nperiod = \"{year}FY\"\nreader = \"{reader}\"\n",
                    sha256(if year == 2023 {
                        DOCUMENT
                    } else {
                        DOCUMENT_2024
                    })
                    .to_hex(),
                )
                .unwrap();
            }
        }
        std::fs::write(transcriptions_path(root, "synthetic"), readings).unwrap();
    }

    #[cfg(test)]
    fn workspace(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "fmine-track-record-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        for dir in [
            "corpus",
            "documents",
            "registries/sources",
            "registries/transcriptions",
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("documents/Guidance2023.pdf"), DOCUMENT).unwrap();
        std::fs::write(root.join("documents/Guidance2024.pdf"), DOCUMENT_2024).unwrap();
        install_sources(
            &root,
            &workbook(2024, 100.0, 90.0, true),
            &workbook(2025, 120.0, 150.0, true),
        );
        install_readings(&root, "attributable:3/4", false, false);
        root
    }

    fn invoke(root: &Path, extra: &[&str]) -> Envelope {
        let args: Vec<String> = ["track-record", "--issuer", "synthetic"]
            .into_iter()
            .chain(extra.iter().copied())
            .map(str::to_owned)
            .collect();
        let (envelope, format) = fmine_cli::invoke(&args, root);
        assert_eq!(format, OutputFormat::Json);
        envelope
    }

    fn successful(root: &Path, extra: &[&str]) -> Envelope {
        let result = invoke(root, extra);
        assert_eq!(result.exit_code, 0, "{}", result.to_json());
        result
    }

    #[cfg(test)]
    fn row<'a>(payload: &'a str, metric: &str, year: u16) -> &'a str {
        let key = format!("\"site\":\"Site A\",\"metric\":\"{metric}\",\"year\":{year},");
        let key_at = payload.find(&key).expect("requested row exists");
        let start = payload[..key_at].rfind("{\"id\":").unwrap();
        let (mut depth, mut quoted, mut escaped) = (0_u32, false, false);
        for (offset, byte) in payload[start..].bytes().enumerate() {
            if escaped {
                escaped = false;
            } else if quoted && byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = !quoted;
            } else if !quoted {
                match byte {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            return &payload[start..=start + offset];
                        }
                    }
                    _ => {}
                }
            }
        }
        panic!("requested row has no closing object");
    }

    #[cfg(test)]
    fn selected(row: &str) -> &str {
        row.split_once("\"outcome\":")
            .unwrap()
            .1
            .split_once(",\"later_outcomes\":")
            .unwrap()
            .0
    }

    #[cfg(test)]
    fn assert_refused(root: &Path, code: &str, exit: u8) {
        let result = invoke(root, &[]);
        assert_eq!(result.exit_code, exit, "{}", result.to_json());
        assert_eq!(result.refusal.as_ref().unwrap().code, code);
        assert!(!result.payload_json.contains("\"score\":"));
    }

    #[test]
    fn synthetic_sources_become_canonical_claims_and_exact_scores() {
        let root = workspace("positive");
        let result = successful(&root, &[]);
        let payload = &result.payload_json;
        assert!(payload.contains("\"lane\":\"synthetic\""));
        assert!(payload.contains("\"mode\":\"single_site_descriptive_cards\""));
        let annual = row(payload, "production_koz", 2024);
        assert!(annual.contains("\"state\":\"resolved_first_published\""));
        assert!(selected(annual).contains("\"displayed\":\"100\""));
        assert!(annual.contains("\"side\":\"inside\""));
        assert!(annual.contains("\"z\":{\"num\":\"0\",\"den\":\"1\"}"));
        assert!(annual.contains("\"definitely_inside\":true"));
        assert!(annual.contains("Synthetic_2024Q4.xlsx:YTD!C5"));
        assert!(
            !selected(annual).contains("Synthetic_2025Q4.xlsx"),
            "later restatement must not replace original"
        );
        assert!(
            annual.contains("Synthetic_2025Q4.xlsx:YTD!J5"),
            "later appearances stay visible separately"
        );
        assert!(selected(row(payload, "aisc_usd_per_oz", 2024)).contains("\"displayed\":\"1000\""));
        let comparative = row(payload, "production_koz", 2023);
        assert!(comparative.contains("\"state\":\"resolved_on_comparative_vintage\""));
        assert!(comparative.contains("Synthetic_2024Q4.xlsx:YTD!J5"));
        assert!(payload.contains("\"outcome_selector\":\"comparative_only\""));
        assert!(payload.contains("\"outcome_selector\":\"first_published\""));
        assert!(payload.contains("\"effective_n\":null"));
        let profile = load_issuer_profile(&root, "synthetic").unwrap();
        let report = fmine_public::run_public_assurance(&profile, &root).unwrap();
        let claim = report
            .public_claims
            .cells
            .values()
            .find(|c| c.locator == "Synthetic_2024Q4.xlsx:YTD!C5")
            .unwrap();
        assert!(annual.contains(&claim.claim.claim_id.to_string()));
        assert!(
            annual.contains(&format!("\"parents\":[\"{}\"]", claim.claim.claim_id)),
            "the interpreted outcome must retain its exact source Claim parent"
        );
        assert!(annual.contains("\"source_claim\":{\"claim_id\":\"clm_"));
        assert!(annual.contains("\"interpreted_outcome_claim\":{\"claim_id\":\"clm_"));
        let scored = annual.split_once("\"score\":").unwrap().1;
        assert!(scored.contains("\"claim\":{\"claim_id\":\"clm_"));
        assert!(scored.contains("\"value_tag\":\""));
        assert!(
            scored.contains("\"knowledge\":\"Unknown\""),
            "synthetic source knowledge must never be upgraded by scoring"
        );
        assert!(scored.contains("\"evidence\":\"Assumed\""));
        assert!(annual.contains(&claim.key.reporting_vintage.file_sha256));
        assert!(payload.contains("\"scope\":{\"kind\":\"site_history\""));
        assert!(payload.contains("\"iid_reference_interval_90_exact\":{\"lo\":{\"num\":"));
        assert!(payload.contains("\"source_digest\":\""));
        assert!(payload.contains(&sha256(DOCUMENT).to_hex()));
        for flag in [
            "reserve_disclosure_claim",
            "execution_authorized",
            "geotechnical_signoff",
            "regulatory_compliance_claim",
        ] {
            assert!(result.to_json().contains(&format!("\"{flag}\":false")));
        }
        assert_eq!(result, successful(&root, &[]), "reruns are deterministic");
    }

    #[test]
    fn ownership_selection_never_substitutes_full_project_production() {
        let root = workspace("ownership");
        install_readings(&root, "100%", false, false);
        let full = successful(&root, &["--site", "site-a"]);
        let annual = row(&full.payload_json, "production_koz", 2024);
        assert!(selected(annual).contains("\"displayed\":\"200\""));
        assert!(annual.contains("Synthetic_2024Q4.xlsx:YTD!B5"));
        assert!(annual.contains("\"side\":\"above\""));
        install_readings(&root, "attributable:3/4", false, false);
        install_sources(
            &root,
            &workbook(2024, 100.0, 90.0, false),
            &workbook(2025, 120.0, 150.0, true),
        );
        let missing = successful(&root, &[]);
        let annual = row(&missing.payload_json, "production_koz", 2024);
        assert!(annual.contains("\"state\":\"unresolved\""));
        assert!(
            selected(annual).contains("no annual summary cell in the original registered release")
        );
        assert!(
            annual
                .split_once("\"score\":")
                .unwrap()
                .1
                .contains("\"value\":null")
        );
        assert!(!selected(annual).contains("\"displayed\":\"200\""));
        install_sources(
            &root,
            &workbook(2024, 100.0, 90.0, true),
            &workbook(2025, 120.0, 150.0, true),
        );
        install_readings(&root, "attributable:1/2", false, false);
        let mismatch = successful(&root, &[]);
        let annual = row(&mismatch.payload_json, "production_koz", 2024);
        assert!(selected(annual).contains("\"state\":\"unresolved\""));
        assert!(selected(annual).contains("\"code\":\"FMINE-REF-105\""));
        assert!(
            annual
                .split_once("\"score\":")
                .unwrap()
                .1
                .contains("\"value\":null")
        );
    }

    fn dated_workspace(tag: &str) -> PathBuf {
        let root = workspace(tag);
        let path = registry_path(&root, "synthetic");
        let registry = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            path,
            registry.replace(
                "retrieved_utc = \"synthetic\"",
                "retrieved_utc = \"2026-02-01T00:00:00Z\"",
            ),
        )
        .unwrap();
        root
    }

    fn technical_entry(reader: &str, token: &str) -> String {
        format!(
            "\n[[entry]]\ndoc = \"Technical.pdf\"\ndoc_sha256 = \"{}\"\npage = 1\n\
         table = \"Invented technical inputs\"\nrow = \"Site A\"\ncol = \"Dry feed\"\n\
         token = \"{token}\"\nquantity = \"technical_site_a_remaining_dry_feed_mt\"\n\
         unit = \"dry_mt\"\nbasis = \"100%;scenario=plan\"\nperiod = \"2024H2\"\nreader = \"{reader}\"\n",
            sha256(TECHNICAL).to_hex(),
        )
    }

    #[test]
    fn cutoff_and_source_reload_preserve_snapshot_identity() {
        let root = workspace("reload");
        let original = successful(&root, &["--through", "2024Q4"]);
        let full = successful(&root, &[]);
        assert!(original.payload_json.contains("\"through\":\"2024Q4\""));
        assert_eq!(
            selected(row(&original.payload_json, "production_koz", 2024)),
            selected(row(&full.payload_json, "production_koz", 2024))
        );
        install_sources(
            &root,
            &workbook(2024, 105.0, 90.0, true),
            &workbook(2025, 120.0, 150.0, true),
        );
        let changed = successful(&root, &["--through", "2024Q4"]);
        assert_ne!(original.payload_json, changed.payload_json);
        let annual = row(&changed.payload_json, "production_koz", 2024);
        assert!(selected(annual).contains("\"displayed\":\"105\""));
        assert!(annual.contains("\"z\":{\"num\":\"1\",\"den\":\"2\"}"));
        assert_eq!(changed, successful(&root, &["--through", "2024Q4"]));
    }

    #[test]
    fn changed_or_missing_workbooks_and_documents_refuse() {
        let root = workspace("integrity");
        let document = root.join("documents/Guidance2023.pdf");
        let mut altered = DOCUMENT.to_vec();
        altered[0] ^= 1;
        std::fs::write(&document, altered).unwrap();
        assert_refused(&root, "FMINE-REF-505", 8);
        std::fs::write(&document, DOCUMENT).unwrap();
        let held_document = root.join("documents/Guidance2023.pdf.held");
        std::fs::rename(&document, &held_document).unwrap();
        assert_refused(&root, "FMINE-REF-506", 5);
        std::fs::rename(&held_document, &document).unwrap();
        let workbook = root.join("corpus").join(ORIGINAL);
        let bytes = std::fs::read(&workbook).unwrap();
        let mut altered = bytes.clone();
        altered[0] ^= 1;
        std::fs::write(&workbook, altered).unwrap();
        assert_refused(&root, "FMINE-REF-505", 8);
        std::fs::write(&workbook, bytes).unwrap();
        let held_book = root.join("corpus/original.held");
        std::fs::rename(&workbook, &held_book).unwrap();
        assert_refused(&root, "FMINE-REF-506", 5);
        std::fs::rename(&held_book, &workbook).unwrap();
        successful(&root, &[]);
    }

    #[test]
    fn disagreement_and_incomplete_readings_never_produce_scores() {
        let root = workspace("readers");
        install_readings(&root, "attributable:3/4", true, false);
        assert_refused(&root, "FMINE-REF-202", 3);
        install_readings(&root, "attributable:3/4", false, true);
        assert_refused(&root, "FMINE-REF-202", 3);
        install_readings(&root, "attributable:3/4", false, false);
        successful(&root, &[]);
    }

    #[test]
    fn binary_formats_and_selectors_expose_the_same_cited_report() {
        let root = workspace("binary");
        let args = [
            "track-record",
            "--issuer",
            "synthetic",
            "--site",
            "Site A",
            "--through",
            "2024Q4",
        ];
        let expected = successful(&root, &["--site", "Site A", "--through", "2024Q4"]);
        for (format, expected_text) in [
            ("json", expected.to_json()),
            ("markdown", expected.rendered_markdown.clone().unwrap()),
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
                .args(args)
                .args(["--format", format, "--workspace"])
                .arg(&root)
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                format!("{expected_text}\n")
            );
        }
        let markdown = expected.rendered_markdown.unwrap();
        assert!(markdown.contains("# Guidance track record"));
        assert!(markdown.contains("Synthetic\\_2024Q4.xlsx:YTD!C5"));
        assert!(markdown.contains(&sha256(DOCUMENT).to_hex()));
        assert!(markdown.contains("No forecast registration, calibration verdict"));
        let unknown = invoke(&root, &["--site", "absent-site"]);
        assert_eq!(unknown.exit_code, 3);
        assert_eq!(unknown.refusal.as_ref().unwrap().code, "FMINE-REF-201");
    }

    fn royalty_sheet(year: u16, months: &str, royalty: f64) -> MinimalSheet {
        MinimalSheet {
            name: "Minestats3".into(),
            hidden: false,
            cells: vec![
                text("A1", "Mine Statistics (Gold)"),
                text("B2", "Site A (75%)"),
                text("A3", &format!("For the {months} months ended,")),
                number("B3", f64::from(year)),
                text("A4", "Costs per ounce ($/oz)"),
                text("A5", "Royalties"),
                number("B5", royalty),
            ],
        }
    }

    /// A complete explicitly bounded two-quarter price history; every byte and number is invented.
    fn price_workspace(tag: &str, price_subject: &str, omit_second_quarter_price: bool) -> PathBuf {
        const PRICES: &[u8] =
            b"%PDF-1.7 synthetic annual and quarterly realized prices, never parsed";
        let root = workspace(tag);
        let mut registry = String::from(
            "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"corpus\"\n\
         retrieved_utc = \"synthetic\"\nurl_prefix = \"https://example.invalid/\"\n",
        );
        let annual = build_minimal_xlsx(&[
            summary_sheet(2023, 100.0, 90.0, true),
            royalty_sheet(2023, "twelve", 100.0),
        ]);
        for (quarter, bytes) in [
            ("2023Q4", annual),
            (
                "2024Q1",
                build_minimal_xlsx(&[royalty_sheet(2024, "three", 50.0)]),
            ),
            (
                "2024Q2",
                build_minimal_xlsx(&[royalty_sheet(2024, "three", 100.0)]),
            ),
        ] {
            let file = format!("Synthetic_Price_{quarter}.xlsx");
            std::fs::write(root.join("corpus").join(&file), &bytes).unwrap();
            write!(
                registry,
                "\n[[source]]\nfile = \"{file}\"\nkind = \"mine_stats\"\nquarter = \"{quarter}\"\n\
             path = \"{file}\"\nurl_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{}\"\n",
                bytes.len(),
                sha256(&bytes).to_hex()
            )
            .unwrap();
        }
        registry.push_str("\n[[subject]]\nname = \"Site A\"\n\n[[subject]]\nname = \"Company\"\n");
        for (file, bytes, date) in [
            ("Guidance2023.pdf", DOCUMENT, "2023-02-01"),
            ("Prices.pdf", PRICES, "2024-07-01"),
        ] {
            std::fs::write(root.join("documents").join(file), bytes).unwrap();
            write!(
                registry,
                "\n[[document]]\nfile = \"{file}\"\ndir = \"documents\"\nkind = \"mda\"\n\
             url = \"https://example.invalid/{file}\"\nurl_basis = \"W1 table\"\n\
             published_at = \"{date}\"\npublished_at_basis = \"FACT: synthetic fixture date\"\n\
             pages = 2\nbytes = {}\nsha256 = \"{}\"\n",
                bytes.len(),
                sha256(bytes).to_hex()
            )
            .unwrap();
        }
        std::fs::write(registry_path(&root, "synthetic"), registry).unwrap();
        let mut readings =
            String::from("schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n");
        let entries = [
            (
                "Guidance2023.pdf",
                DOCUMENT,
                "2023FY",
                "guidance_site_a_aisc_usd_per_oz".into(),
                "900 - 1100",
                "attributable:6/8;aisc=issuer-reported-per-sold-oz",
            ),
            (
                "Guidance2023.pdf",
                DOCUMENT,
                "2023FY",
                "aisc_price_assumption_site_a_2023fy_usd_per_oz".into(),
                "1500",
                "realized-sales-price",
            ),
            (
                "Prices.pdf",
                PRICES,
                "2023FY",
                format!("realized_price_{price_subject}_2023fy_usd_per_oz"),
                "2000",
                "realized-sales-price",
            ),
            (
                "Prices.pdf",
                PRICES,
                "2024Q1",
                format!("realized_price_{price_subject}_2024q1_usd_per_oz"),
                "1000",
                "realized-sales-price",
            ),
            (
                "Prices.pdf",
                PRICES,
                "2024Q2",
                format!("realized_price_{price_subject}_2024q2_usd_per_oz"),
                "2000",
                "realized-sales-price",
            ),
        ];
        for reader in ["reader_a", "reader_b"] {
            for (file, bytes, period, quantity, token, basis) in &entries {
                if omit_second_quarter_price && *period == "2024Q2" {
                    continue;
                }
                write!(
                    readings,
                    "\n[[entry]]\ndoc = \"{file}\"\ndoc_sha256 = \"{}\"\npage = 1\n\
                 table = \"Invented price sensitivity\"\nrow = \"{quantity}\"\ncol = \"{period}\"\n\
                 token = \"{token}\"\nquantity = \"{quantity}\"\nunit = \"usd_per_oz\"\n\
                 basis = \"{basis}\"\nperiod = \"{period}\"\nreader = \"{reader}\"\n",
                    sha256(bytes).to_hex()
                )
                .unwrap();
            }
        }
        std::fs::write(transcriptions_path(&root, "synthetic"), readings).unwrap();
        root
    }

    #[test]
    fn price_sensitivity_binds_registered_royalties_and_retains_both_exact_scores() {
        let root = price_workspace("price-positive", "site_a", false);
        let result = successful(&root, &["--through", "2024Q2"]);
        let annual = row(&result.payload_json, "aisc_usd_per_oz", 2023);
        assert!(annual.contains("\"basis\":\"attributable:6/8;aisc=issuer-reported-per-sold-oz\""));
        let price = annual.split_once("\"price_sensitivity\":").unwrap().1;
        assert!(
            price.contains("\"price_contribution\":{\"num\":\"25\",\"den\":\"1\"}"),
            "{price}"
        );
        assert!(price.contains("\"unconditional\":"));
        assert!(price.contains("\"z\":{\"num\":\"0\",\"den\":\"1\"}"));
        assert!(price.contains("\"conditional\":"));
        assert!(price.contains("\"z\":{\"num\":\"-1\",\"den\":\"4\"}"));
        assert!(price.contains("\"price_proxy\":false"));
        let profile = load_issuer_profile(&root, "synthetic")
            .unwrap()
            .through("2024Q2")
            .unwrap();
        let claims = fmine_public::key::mine_stats::load_claims(&profile, &root).unwrap();
        let royalties: Vec<_> = claims
            .claims
            .iter()
            .filter(|c| c.key.line.label.ends_with(":royalties"))
            .collect();
        assert_eq!(royalties.len(), 3);
        for claim in royalties {
            assert!(price.contains(&claim.claim.claim_id.to_string()), "{price}");
            assert!(price.contains(&claim.locator), "{price}");
            assert!(
                price.contains(&claim.key.reporting_vintage.file_sha256),
                "{price}"
            );
        }
        assert!(price.contains("Guidance2023.pdf:p1:Invented price sensitivity"));
        assert!(price.contains("Prices.pdf:p1:Invented price sensitivity"));
        assert_eq!(result, successful(&root, &["--through", "2024Q2"]));
        // Prefix scoping keeps both price families while excluding an unrelated pending/conflicted
        // technical locator on the very same guidance document.
        let path = transcriptions_path(&root, "synthetic");
        let readings = std::fs::read_to_string(&path).unwrap();
        let optional = |reader, token| {
            technical_entry(reader, token)
                .replace("Technical.pdf", "Guidance2023.pdf")
                .replace(&sha256(TECHNICAL).to_hex(), &sha256(DOCUMENT).to_hex())
        };
        for extra in [
            optional("optional-a", "10"),
            format!(
                "{}{}",
                optional("optional-a", "10"),
                optional("optional-b", "11")
            ),
        ] {
            std::fs::write(&path, format!("{readings}{extra}")).unwrap();
            assert_eq!(result, successful(&root, &["--through", "2024Q2"]));
        }
        let unbounded = successful(&root, &[]);
        let annual = row(&unbounded.payload_json, "aisc_usd_per_oz", 2023);
        let price = annual.split_once("\"price_sensitivity\":").unwrap().1;
        assert!(
            price.contains("2026Q2"),
            "the full declared population remains required: {price}"
        );
        assert!(!price.contains("\"price_contribution\":"), "{price}");
    }

    #[test]
    fn price_sensitivity_requires_complete_history_and_an_explicit_company_proxy() {
        let missing = price_workspace("price-missing", "site_a", true);
        let result = successful(&missing, &["--through", "2024Q2"]);
        let annual = row(&result.payload_json, "aisc_usd_per_oz", 2023);
        assert!(annual.contains("\"z\":{\"num\":\"0\",\"den\":\"1\"}"));
        let price = annual.split_once("\"price_sensitivity\":").unwrap().1;
        assert!(!price.contains("\"price_contribution\":"), "{price}");
        assert!(
            price.contains("2024Q2"),
            "missing quarter must remain visible: {price}"
        );

        let root = price_workspace("price-proxy", "company", false);
        let default = successful(&root, &["--through", "2024Q2"]);
        let unsplit = row(&default.payload_json, "aisc_usd_per_oz", 2023);
        assert!(!unsplit.contains("\"price_contribution\":"), "{unsplit}");
        let proxy = successful(
            &root,
            &["--through", "2024Q2", "--price-subject", "Company"],
        );
        let annual = row(&proxy.payload_json, "aisc_usd_per_oz", 2023);
        assert!(annual.contains("\"price_proxy\":true"), "{annual}");
        assert!(annual.contains("\"price_subject\":\"Company\""));
        assert!(annual.contains("\"price_contribution\":{\"num\":\"25\",\"den\":\"1\"}"));
    }

    #[test]
    fn shared_price_assumption_retains_its_source_and_refuses_competing_site_assumptions() {
        const SITE: &str = "aisc_price_assumption_site_a_2023fy_usd_per_oz";
        const ALL: &str = "aisc_price_assumption_all_sites_2023fy_usd_per_oz";
        let root = price_workspace("price-shared", "site_a", false);
        let before = successful(&root, &["--through", "2024Q2"]);
        let before_row = row(&before.payload_json, "aisc_usd_per_oz", 2023);
        let before_price = before_row.split_once("\"price_sensitivity\":").unwrap().1;
        let path = transcriptions_path(&root, "synthetic");
        let readings = std::fs::read_to_string(&path).unwrap();
        let site_quantity = format!("quantity = \"{SITE}\"");
        assert_eq!(readings.matches(&site_quantity).count(), 2);
        // The printed locator stays unchanged when its explicitly declared scope changes.
        let mut shared_readings =
            readings.replace(&site_quantity, &format!("quantity = \"{ALL}\""));
        std::fs::write(&path, &shared_readings).unwrap();
        let shared = successful(&root, &["--through", "2024Q2"]);
        let shared_row = row(&shared.payload_json, "aisc_usd_per_oz", 2023);
        let shared_price = shared_row.split_once("\"price_sensitivity\":").unwrap().1;
        assert_eq!(selected(before_row), selected(shared_row));
        assert_eq!(
            shared_price,
            before_price.replace(
                &format!("\"quantity\":\"{SITE}\""),
                &format!("\"quantity\":\"{ALL}\"")
            )
        );
        assert!(shared_price.contains("\"price_contribution\":{\"num\":\"25\",\"den\":\"1\"}"));

        for reader in ["reader_a", "reader_b"] {
            write!(
                shared_readings,
                "\n[[entry]]\ndoc = \"Guidance2023.pdf\"\ndoc_sha256 = \"{}\"\npage = 1\n\
             table = \"Invented price sensitivity\"\nrow = \"Additional site assumption\"\n\
             col = \"2023FY\"\ntoken = \"1600\"\nquantity = \"{SITE}\"\n\
             unit = \"usd_per_oz\"\nbasis = \"realized-sales-price\"\n\
             period = \"2023FY\"\nreader = \"{reader}\"\n",
                sha256(DOCUMENT).to_hex()
            )
            .unwrap();
        }
        std::fs::write(&path, shared_readings).unwrap();
        let ambiguous = successful(&root, &["--through", "2024Q2"]);
        let annual = row(&ambiguous.payload_json, "aisc_usd_per_oz", 2023);
        assert_eq!(selected(annual), selected(before_row));
        assert!(annual.contains("\"z\":{\"num\":\"0\",\"den\":\"1\"}"));
        let price = annual.split_once("\"price_sensitivity\":").unwrap().1;
        assert!(!price.contains("\"price_contribution\":"), "{price}");
        assert!(
            price.contains("\"unsplit_reason\":\"unusable_price_assumption\""),
            "{price}"
        );
        assert!(
            price.contains("found 2 in guidance document Guidance2023.pdf"),
            "{price}"
        );
        for quantity in [SITE, ALL] {
            assert!(
                price.contains(&format!("\"quantity\":\"{quantity}\"")),
                "{price}"
            );
        }
        for source_row in [SITE, "Additional site assumption"] {
            assert!(
                price.contains(&format!(
                    "Guidance2023.pdf:p1:Invented price sensitivity:{source_row}/2023FY"
                )),
                "{price}"
            );
        }
    }

    #[test]
    fn optional_technical_failures_do_not_block_either_track_record_path() {
        let root = dated_workspace("optional-technical");
        let before: Vec<_> = TRACK_MODES
            .iter()
            .map(|extra| successful(&root, extra))
            .collect();
        let source_path = registry_path(&root, "synthetic");
        let mut registry = std::fs::read_to_string(&source_path).unwrap();
        write!(registry,
        "\n[[document]]\nfile = \"Technical.pdf\"\ndir = \"documents\"\nkind = \"technical_report\"\n\
         url = \"https://example.invalid/Technical.pdf\"\nurl_basis = \"W1 table\"\n\
         published_at = \"2024-01-01\"\npublished_at_basis = \"FACT: invented technical fixture\"\n\
         pages = 1\nbytes = {}\nsha256 = \"{}\"\n",TECHNICAL.len(),sha256(TECHNICAL).to_hex()).unwrap();
        std::fs::write(source_path, registry).unwrap();
        let path = transcriptions_path(&root, "synthetic");
        let base = std::fs::read_to_string(&path).unwrap();
        let agreed = format!(
            "{base}{}{}",
            technical_entry("reader_a", "10"),
            technical_entry("reader_b", "10")
        );
        std::fs::write(&path, &agreed).unwrap();
        let unchanged = || {
            for (extra, expected) in TRACK_MODES.iter().zip(&before) {
                assert_eq!(&successful(&root, extra), expected, "mode {extra:?}");
            }
        };
        unchanged(); // Missing optional bytes are outside both track-record populations.
        std::fs::write(
            root.join("documents/Technical.pdf"),
            b"altered optional bytes",
        )
        .unwrap();
        unchanged();
        std::fs::write(root.join("documents/Technical.pdf"), TECHNICAL).unwrap();
        std::fs::write(
            &path,
            format!("{base}{}", technical_entry("reader_a", "10")),
        )
        .unwrap();
        unchanged(); // Pending optional reader.
        std::fs::write(
            &path,
            format!(
                "{base}{}{}",
                technical_entry("reader_a", "10"),
                technical_entry("reader_b", "11")
            ),
        )
        .unwrap();
        unchanged(); // Disagreeing optional readers.
        // Selection never bypasses structural registry/document-identity validation.
        std::fs::write(
            &path,
            agreed.replace(&sha256(TECHNICAL).to_hex(), &"0".repeat(64)),
        )
        .unwrap();
        for extra in TRACK_MODES {
            let result = invoke(&root, extra);
            assert_eq!(result.exit_code, 5);
            assert_eq!(result.refusal.as_ref().unwrap().code, "FMINE-REF-506");
        }
    }

    #[test]
    fn prefix_selection_retains_conflicting_guidance_readers_and_unsupported_keys() {
        let root = dated_workspace("guidance-prefix");
        let path = transcriptions_path(&root, "synthetic");
        let base = std::fs::read_to_string(&path).unwrap();
        let entry = |reader: &str, row: &str, quantity: &str| {
            format!(
                "\n[[entry]]\ndoc = \"Guidance2024.pdf\"\ndoc_sha256 = \"{}\"\npage = 1\n\
         table = \"Invented guidance\"\nrow = \"{row}\"\ncol = \"2024\"\n\
         token = \"90 - 110\"\nquantity = \"{quantity}\"\nunit = \"koz\"\n\
         basis = \"attributable:3/4\"\nperiod = \"2024FY\"\nreader = \"{reader}\"\n",
                sha256(DOCUMENT_2024).to_hex(),
            )
        };
        // Two readers select this locator; a third's non-guidance key must not be filtered away.
        std::fs::write(
            &path,
            format!(
                "{base}{}",
                entry(
                    "reader_c",
                    "Site A production_koz",
                    "technical_site_a_another_interpretation"
                )
            ),
        )
        .unwrap();
        for extra in TRACK_MODES {
            let result = invoke(&root, extra);
            assert_eq!(result.refusal.as_ref().unwrap().code, "FMINE-REF-202");
        }
        let unsupported = "guidance_site_a_unsupported_output";
        std::fs::write(
            &path,
            format!(
                "{base}{}{}",
                entry("reader_a", "Site A unsupported output", unsupported),
                entry("reader_b", "Site A unsupported output", unsupported),
            ),
        )
        .unwrap();
        let public = successful(&root, TRACK_MODES[1]);
        assert!(public.payload_json.contains("\"unsupported_guidance\":[{\"at\":\"Guidance2024.pdf:p1:Invented guidance:Site A unsupported output/2024\""));
        assert!(
            public
                .payload_json
                .contains(&format!("\"quantity\":\"{unsupported}\""))
        );
        // Legacy mode's existing explicit unsupported-metric refusal is preserved, not silently omitted.
        assert_refused(&root, "FMINE-REF-201", 3);
        std::fs::write(
            &path,
            format!(
                "{base}{}",
                entry("reader_a", "Site A unsupported output", unsupported)
            ),
        )
        .unwrap();
        for extra in TRACK_MODES {
            assert_eq!(
                invoke(&root, extra).refusal.as_ref().unwrap().code,
                "FMINE-REF-202"
            );
        }
    }

    #[test]
    fn realized_price_document_cannot_precede_its_quarter_end() {
        let root = price_workspace("price-publication", "site_a", false);
        let before = successful(&root, &["--through", "2024Q2"]);
        let path = registry_path(&root, "synthetic");
        let registry = std::fs::read_to_string(&path).unwrap();
        assert_eq!(registry.matches("published_at = \"2024-07-01\"").count(), 1);
        std::fs::write(
            &path,
            registry.replace(
                "published_at = \"2024-07-01\"",
                "published_at = \"2024-01-01\"",
            ),
        )
        .unwrap();
        let early = successful(&root, &["--through", "2024Q2"]);
        let annual = row(&early.payload_json, "aisc_usd_per_oz", 2023);
        assert_eq!(
            selected(annual),
            selected(row(&before.payload_json, "aisc_usd_per_oz", 2023))
        );
        assert!(annual.contains("\"z\":{\"num\":\"0\",\"den\":\"1\"}"));
        let price = annual.split_once("\"price_sensitivity\":").unwrap().1;
        assert!(!price.contains("\"price_contribution\":"), "{price}");
        assert!(price.contains("precedes period end 2024-06-30"), "{price}");
        assert!(price.contains("Prices.pdf:p1:Invented price sensitivity:realized_price_site_a_2024q2_usd_per_oz/2024Q2"));
        assert!(price.contains("\"published_at\":\"2024-01-01\""));
    }

    fn revise_price_reader(
        root: &Path,
        quantity: &str,
        reader: &str,
        replacement: Option<(&str, &str)>,
    ) {
        let path = transcriptions_path(root, "synthetic");
        let readings = std::fs::read_to_string(&path).unwrap();
        let quantity_line = format!("quantity = \"{quantity}\"");
        let reader_line = format!("reader = \"{reader}\"");
        let mut changed = 0;
        let mut revised = String::new();
        for (index, entry) in readings.split("\n[[entry]]").enumerate() {
            if index == 0 {
                revised.push_str(entry);
                continue;
            }
            let targeted = entry.contains(&quantity_line) && entry.contains(&reader_line);
            if targeted {
                changed += 1;
                if let Some((from, to)) = replacement {
                    assert_eq!(entry.matches(from).count(), 1);
                    revised.push_str("\n[[entry]]");
                    revised.push_str(&entry.replace(from, to));
                }
            } else {
                revised.push_str("\n[[entry]]");
                revised.push_str(entry);
            }
        }
        assert_eq!(changed, 1, "mutation must target one reader at one locator");
        std::fs::write(path, revised).unwrap();
    }

    #[test]
    fn incomplete_optional_price_evidence_keeps_the_unconditional_track_record() {
        for disagree in [false, true] {
            let root = price_workspace(
                if disagree {
                    "price-conflicting-optional"
                } else {
                    "price-pending-optional"
                },
                "site_a",
                false,
            );
            let before = successful(&root, &["--through", "2024Q2"]);
            revise_price_reader(
                &root,
                "realized_price_site_a_2024q2_usd_per_oz",
                "reader_b",
                disagree.then_some(("token = \"2000\"", "token = \"2100\"")),
            );
            let after = successful(&root, &["--through", "2024Q2"]);
            let annual = row(&after.payload_json, "aisc_usd_per_oz", 2023);
            assert_eq!(
                selected(annual),
                selected(row(&before.payload_json, "aisc_usd_per_oz", 2023)),
            );
            assert!(annual.contains("\"z\":{\"num\":\"0\",\"den\":\"1\"}"));
            let price = annual.split_once("\"price_sensitivity\":").unwrap().1;
            assert!(
                price.contains("\"unsplit_reason\":\"incomplete_transcription_agreement\""),
                "{price}"
            );
            assert!(!price.contains("\"price_contribution\":"), "{price}");
            assert!(price.contains("Prices.pdf:p1:Invented price sensitivity:realized_price_site_a_2024q2_usd_per_oz/2024Q2"), "{price}");
            assert!(price.contains("reader_a"), "{price}");
            assert_eq!(after, successful(&root, &["--through", "2024Q2"]));

            // The general assurance loader retains its stricter all-quantity disagreement gate.
            let profile = load_issuer_profile(&root, "synthetic").unwrap();
            let general = fmine_public::transcribe::load_transcription_report(&profile, &root);
            assert_eq!(general.is_err(), disagree);

            // Optional evidence never waives a byte-integrity failure.
            std::fs::write(
                root.join("documents/Prices.pdf"),
                b"altered optional price bytes",
            )
            .unwrap();
            let result = invoke(&root, &["--through", "2024Q2"]);
            assert_eq!(result.exit_code, 8);
            assert!(result.to_json().contains("FMINE-REF-505"));
        }
    }

    #[test]
    fn incomplete_or_ambiguously_named_guidance_still_refuses_the_population() {
        let missing = price_workspace("guidance-reader-pending", "site_a", false);
        revise_price_reader(
            &missing,
            "guidance_site_a_aisc_usd_per_oz",
            "reader_b",
            None,
        );
        let mixed = price_workspace("price-guidance-name-conflict", "site_a", false);
        revise_price_reader(
            &mixed,
            "realized_price_site_a_2024q2_usd_per_oz",
            "reader_b",
            Some((
                "quantity = \"realized_price_site_a_2024q2_usd_per_oz\"",
                "quantity = \"guidance_site_a_aisc_usd_per_oz\"",
            )),
        );
        for root in [missing, mixed] {
            let result = invoke(&root, &["--through", "2024Q2"]);
            assert_eq!(result.exit_code, 3, "{}", result.to_json());
            assert!(result.to_json().contains("FMINE-REF-202"));
        }
    }

    /// Invented filings for an imaginary issuer exercise the normal Assertion path.
    /// Synthetic-lane Unknown Claims remain inadmissible to the public as-of scorer.
    fn as_of_price_workspace(tag: &str, subject: &str) -> PathBuf {
        let root = price_workspace(tag, subject, false);
        let source = std::fs::read_to_string(registry_path(&root, "synthetic"))
            .unwrap()
            .replace("issuer = \"synthetic\"", "issuer = \"pricecut\"")
            .replace(
                "retrieved_utc = \"synthetic\"",
                "retrieved_utc = \"2024-06-30T00:00:00Z\"",
            );
        std::fs::write(registry_path(&root, "pricecut"), source).unwrap();
        let entries = std::fs::read_to_string(transcriptions_path(&root, "synthetic"))
            .unwrap()
            .replace("issuer = \"synthetic\"", "issuer = \"pricecut\"");
        std::fs::write(transcriptions_path(&root, "pricecut"), entries).unwrap();
        root
    }

    fn price_at(root: &Path, cut: &str, subject: Option<&str>) -> Envelope {
        let mut args: Vec<String> = ["track-record", "--issuer", "pricecut", "--as-of", cut]
            .into_iter()
            .map(str::to_owned)
            .collect();
        if let Some(subject) = subject {
            args.extend(["--price-subject".into(), subject.into()]);
        }
        let (result, format) = fmine_cli::invoke(&args, root);
        assert_eq!(format, OutputFormat::Json);
        result
    }

    fn checked_price_at(root: &Path, cut: &str, subject: Option<&str>) -> Envelope {
        let result = price_at(root, cut, subject);
        assert_eq!(result.exit_code, 0, "{}", result.to_json());
        result
    }

    #[test]
    fn as_of_prices_preserve_base_population_and_bind_both_selections() {
        let root = as_of_price_workspace("asof-paired", "site_a");
        let result = checked_price_at(&root, "2024-07-01", None);
        let base = fmine_public::guidance::track_record::run(&root, "pricecut", None, "2024-07-01")
            .unwrap();
        let base_json = base.to_json();
        let prefix = format!("{{\"digest\":\"{}\",", base.digest);
        let body = base_json
            .strip_prefix(&prefix)
            .unwrap()
            .strip_suffix('}')
            .unwrap();
        assert!(
            result.payload_json.contains(body),
            "base rows and denominators changed"
        );
        assert!(
            result
                .payload_json
                .contains(&format!("\"comparison_digest\":\"{}\"", base.digest))
        );
        assert!(
            !result.payload_json.starts_with(&prefix),
            "combined digest must bind price evidence"
        );
        assert!(
            result
                .payload_json
                .contains("\"digest_scope\":\"base_comparison_and_both_price_selections\"")
        );
        let prices = result
            .payload_json
            .split_once("\"price_sensitivity\":")
            .unwrap()
            .1;
        assert_eq!(
            prices
                .matches("\"price_contribution\":{\"num\":\"25\",\"den\":\"1\"}")
                .count(),
            2
        );
        assert!(prices.contains("\"site\":\"site-a\""), "{prices}");
        assert!(prices.contains("\"price_subject\":\"Site A\""), "{prices}");
        assert!(prices.contains("\"required_quarters\":[\"2024Q1\",\"2024Q2\"]"));
        assert!(!prices.contains("explicitly requested --through"));
        assert_eq!(result, checked_price_at(&root, "2024-07-01", None));
        let markdown = result.rendered_markdown.as_ref().unwrap();
        assert!(markdown.contains("#### First published"));
        assert!(markdown.contains("#### Latest registered"));
        assert!(markdown.contains("Combined comparison and price digest"));
        assert!(!markdown.contains("gold-price adjustments are not implemented"));
    }

    #[test]
    fn as_of_price_documents_after_the_cut_cannot_change_earlier_output() {
        let root = as_of_price_workspace("asof-price-future", "company");
        let before = checked_price_at(&root, "2024-06-30", Some("Company"));
        assert!(!before.payload_json.contains("\"price_contribution\":"));
        let after = checked_price_at(&root, "2024-07-01", Some("Company"));
        assert!(
            after
                .payload_json
                .contains("\"price_contribution\":{\"num\":\"25\",\"den\":\"1\"}")
        );
        assert!(after.payload_json.contains("\"explicit_proxy\":true"));
        let readings_path = transcriptions_path(&root, "pricecut");
        let readings = std::fs::read_to_string(&readings_path).unwrap();
        let mut changed_readers = 0;
        let conflicted = readings
            .split("\n[[entry]]")
            .map(|entry| {
                if entry.contains("quantity = \"realized_price_company_2024q2_usd_per_oz\"")
                    && entry.contains("reader = \"reader_b\"")
                {
                    changed_readers += 1;
                    assert_eq!(entry.matches("token = \"2000\"").count(), 1);
                    entry.replace("token = \"2000\"", "token = \"2100\"")
                } else {
                    entry.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n[[entry]]");
        assert_eq!(changed_readers, 1);
        std::fs::write(&readings_path, conflicted).unwrap();
        assert_eq!(
            before,
            checked_price_at(&root, "2024-06-30", Some("Company"))
        );
        let conflicted_report = checked_price_at(&root, "2024-07-01", Some("Company"));
        assert!(
            conflicted_report
                .payload_json
                .contains("incomplete_transcription_agreement")
        );
        assert!(
            !conflicted_report
                .payload_json
                .contains("\"price_contribution\":")
        );
        std::fs::write(readings_path, readings).unwrap();
        let path = root.join("documents/Prices.pdf");
        let original = std::fs::read(&path).unwrap();
        let mut changed = original.clone();
        changed[0] ^= 1;
        std::fs::write(&path, changed).unwrap();
        assert_eq!(
            before,
            checked_price_at(&root, "2024-06-30", Some("Company"))
        );
        let refused = price_at(&root, "2024-07-01", Some("Company"));
        assert_eq!(refused.refusal.as_ref().unwrap().code, "FMINE-REF-505");
        std::fs::write(path, original).unwrap();
    }

    #[test]
    fn as_of_price_population_keeps_completed_but_unregistered_quarters() {
        let root = as_of_price_workspace("asof-price-population", "site_a");
        let before = checked_price_at(&root, "2024-09-29", None);
        assert!(before.payload_json.contains("\"price_contribution\":"));
        let completed = checked_price_at(&root, "2024-09-30", None);
        let prices = completed
            .payload_json
            .split_once("\"price_sensitivity\":")
            .unwrap()
            .1;
        assert!(prices.contains("\"required_quarters\":[\"2024Q1\",\"2024Q2\",\"2024Q3\"]"));
        assert!(prices.contains("required original Mine Stats release is not registered"));
        assert!(!prices.contains("\"price_contribution\":"));
        assert!(completed.payload_json.contains("\"n_scored\":1"));
    }

    #[test]
    fn as_of_price_selections_use_their_own_outcome_and_royalty_vintage() {
        let root = as_of_price_workspace("asof-price-vintages", "site_a");
        let mut comparative_royalty = royalty_sheet(2023, "twelve", 100.0);
        comparative_royalty.name = "Minestats1".into();
        let bytes = build_minimal_xlsx(&[
            summary_sheet(2024, 100.0, 90.0, true),
            royalty_sheet(2024, "three", 100.0),
            comparative_royalty,
        ]);
        let path = root.join("corpus/Synthetic_Price_2024Q2.xlsx");
        let original = std::fs::read(&path).unwrap();
        let old = format!(
            "bytes = {}\nsha256 = \"{}\"",
            original.len(),
            sha256(&original)
        );
        let new = format!("bytes = {}\nsha256 = \"{}\"", bytes.len(), sha256(&bytes));
        let registry = registry_path(&root, "pricecut");
        let source = std::fs::read_to_string(&registry).unwrap();
        assert_eq!(source.matches(&old).count(), 1);
        std::fs::write(&registry, source.replace(&old, &new)).unwrap();
        std::fs::write(&path, bytes).unwrap();
        let result = checked_price_at(&root, "2024-07-01", None);
        let prices = result
            .payload_json
            .split_once("\"price_sensitivity\":")
            .unwrap()
            .1;
        let (first, latest) = prices.split_once("\"latest_registered\":").unwrap();
        assert!(first.contains("Synthetic_Price_2023Q4.xlsx:YTD!D5"));
        assert!(first.contains("\"z\":{\"num\":\"-1\",\"den\":\"4\"}"));
        assert!(latest.contains("Synthetic_Price_2024Q2.xlsx:YTD!K5"));
        assert!(latest.contains("Synthetic_Price_2024Q2.xlsx:Minestats1!B5"));
        assert!(latest.contains("\"z\":{\"num\":\"-3\",\"den\":\"4\"}"));
        assert!(result.payload_json.contains("\"later_comparative\""));
    }
}
