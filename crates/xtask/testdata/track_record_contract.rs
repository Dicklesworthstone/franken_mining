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
        let price = annual.split_once("\"price_sensitivity\":").unwrap().1;
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
    fn general_transcription_loader_keeps_its_strict_agreement_gate() {
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
            revise_price_reader(
                &root,
                "realized_price_site_a_2024q2_usd_per_oz",
                "reader_b",
                disagree.then_some(("token = \"2000\"", "token = \"2100\"")),
            );
            // The general assurance loader retains its stricter all-quantity disagreement gate.
            let profile = load_issuer_profile(&root, "synthetic").unwrap();
            let general = fmine_public::transcribe::load_transcription_report(&profile, &root);
            assert_eq!(general.is_err(), disagree);
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
    fn executable_guidance_binding_matrix_preserves_source_and_price_contracts() {
        let normal = dated_workspace("python-source-matrix");
        for (name, bytes) in [
            ("attribute-absent.xlsx", workbook(2024, 100.0, 90.0, false)),
            ("changed-original.xlsx", workbook(2024, 105.0, 90.0, true)),
        ] {
            std::fs::write(normal.join(name), bytes).unwrap();
        }
        let prices = price_workspace("python-price-matrix", "site_a", false);
        let mut comparative = royalty_sheet(2023, "twelve", 100.0);
        comparative.name = "Minestats1".into();
        std::fs::write(
            prices.join("comparative-q2.xlsx"),
            build_minimal_xlsx(&[
                summary_sheet(2024, 100.0, 90.0, true),
                royalty_sheet(2024, "three", 100.0),
                comparative,
            ]),
        )
        .unwrap();
        let output = Command::new("python3")
            .args([
                "-I",
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../xtask/testdata/guidance_binding_oracle.py"
                ),
                "--binary",
                env!("CARGO_BIN_EXE_fmine"),
                "--normal",
            ])
            .arg(normal)
            .arg("--prices")
            .arg(prices)
            .output()
            .expect("run independent T-layer guidance source matrix");
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
