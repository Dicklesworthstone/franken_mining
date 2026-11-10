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
        object(payload, start)
    }

    fn object(payload: &str, start: usize) -> &str {
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

    fn capex_sheet(year: u16, metal: &str, current: [f64; 3], prior: [f64; 3]) -> MinimalSheet {
        let mut cells = vec![
            text("A1", &format!("Mine Statistics ({metal})")),
            text("B2", "Site A (75%)"),
            text("A3", "For the twelve months ended December 31,"),
            number("B3", f64::from(year)),
            number("C3", f64::from(year - 1)),
            text("A4", "Capital expenditures ($ millions)"),
            text("A5", "Minesite sustaining"),
            text("A6", "Projects"),
            text("A7", "Total capital expenditures ($ millions)"),
        ];
        for (index, (current, prior)) in current.into_iter().zip(prior).enumerate() {
            cells.push(number(&format!("B{}", index + 5), current));
            cells.push(number(&format!("C{}", index + 5), prior));
        }
        MinimalSheet {
            name: "Minestats1".into(),
            hidden: false,
            cells,
        }
    }

    /// Invented registered gold/copper site capital lines, with a later comparative that must not replace
    /// the original annual outcome. The reported scopes make no assertion about cash/accrual or leases.
    fn capex_workspace(tag: &str, metal: &str, original_total: Option<f64>) -> PathBuf {
        let root = workspace(tag);
        let mut first = capex_sheet(2024, metal, [40.0, 60.0, 100.0], [30.0, 50.0, 80.0]);
        if let Some(total) = original_total {
            first.cells.iter_mut().find(|(at, _)| at == "B7").unwrap().1 =
                MinimalCell::Number(total);
        } else {
            first.cells.retain(|(at, _)| at != "B7");
        }
        let original = build_minimal_xlsx(&[summary_sheet(2024, 100.0, 90.0, true), first]);
        let later = build_minimal_xlsx(&[
            summary_sheet(2025, 120.0, 150.0, true),
            capex_sheet(2025, metal, [60.0, 100.0, 160.0], [50.0, 100.0, 150.0]),
        ]);
        install_sources(&root, &original, &later);
        let registry = std::fs::read_to_string(registry_path(&root, "synthetic"))
            .unwrap()
            .replace("issuer = \"synthetic\"", "issuer = \"capexcut\"")
            .replace("retrieved_utc = \"synthetic\"", "retrieved_utc = \"2026-02-01T00:00:00Z\"")
            .replace("quarter = \"2024Q4\"", "quarter = \"2024Q4\"\npublished_at = \"2025-02-01\"\npublished_at_basis = \"FACT: invented fixture publication date\"")
            .replace("quarter = \"2025Q4\"", "quarter = \"2025Q4\"\npublished_at = \"2026-02-01\"\npublished_at_basis = \"FACT: invented fixture publication date\"");
        std::fs::write(registry_path(&root, "capexcut"), registry).unwrap();
        let mut readings = std::fs::read_to_string(transcriptions_path(&root, "synthetic"))
            .unwrap()
            .replace("issuer = \"synthetic\"", "issuer = \"capexcut\"");
        for reader in ["reader_a", "reader_b"] {
            for (metric, scope, token) in [
                ("capex_usd_m", "total", "90 - 110"),
                ("capex_sustaining_usd_m", "minesite-sustaining", "35 - 45"),
                ("capex_projects_usd_m", "projects", "50 - 70"),
            ] {
                writeln!(readings,
                    "\n[[entry]]\ndoc = \"Guidance2024.pdf\"\ndoc_sha256 = \"{}\"\npage = 1\n\
                     table = \"Invented guidance\"\nrow = \"Site A {metric}\"\ncol = \"2024\"\n\
                     token = \"{token}\"\nquantity = \"guidance_site_a_{metric}\"\nunit = \"usd_m\"\n\
                     basis = \"attributable:3/4;capex=issuer-reported-{scope}\"\nperiod = \"2024FY\"\nreader = \"{reader}\"",
                    sha256(DOCUMENT_2024).to_hex(),
                ).unwrap();
            }
        }
        std::fs::write(transcriptions_path(&root, "capexcut"), readings).unwrap();
        root
    }

    fn invoke_capex(root: &Path, extra: &[&str]) -> Envelope {
        let args: Vec<String> = ["track-record", "--issuer", "capexcut"]
            .into_iter()
            .chain(extra.iter().copied())
            .map(str::to_owned)
            .collect();
        let (envelope, format) = fmine_cli::invoke(&args, root);
        assert_eq!(format, OutputFormat::Json);
        envelope
    }

    #[test]
    fn capex_reported_scopes_reach_all_track_record_modes_with_exact_source_claims() {
        for metal in ["Gold", "Copper"] {
            let root = capex_workspace(&format!("capex-{metal}"), metal, Some(100.0));
            let result = invoke_capex(&root, &[]);
            assert_eq!(result.exit_code, 0, "{}", result.to_json());
            for (metric, token, cell) in [
                ("capex_usd_m", "100", "B7"),
                ("capex_sustaining_usd_m", "40", "B5"),
                ("capex_projects_usd_m", "60", "B6"),
            ] {
                let annual = row(&result.payload_json, metric, 2024);
                assert!(
                    annual.contains("\"state\":\"resolved_first_published\""),
                    "{annual}"
                );
                assert!(selected(annual).contains(&format!("\"displayed\":\"{token}\"")));
                assert!(annual.contains("\"side\":\"inside\""));
                assert!(annual.contains("\"z\":{\"num\":\"0\",\"den\":\"1\"}"));
                assert!(annual.contains("\"unit\":\"usd_m\""));
                assert!(annual.contains("\"interpreted_outcome_claim\":{\"claim_id\":\"clm_"));
                assert!(annual.contains(&format!("{ORIGINAL}:Minestats1!{cell}")));
                assert!(!selected(annual).contains(LATER));
                assert!(
                    annual.contains(LATER),
                    "the later restatement stays visible"
                );
            }
            let profile = load_issuer_profile(&root, "capexcut").unwrap();
            let inventory = fmine_public::key::mine_stats::load_claims(&profile, &root).unwrap();
            let total = inventory
                .claims
                .iter()
                .find(|c| c.locator == format!("{ORIGINAL}:Minestats1!B7"))
                .unwrap();
            let annual = row(&result.payload_json, "capex_usd_m", 2024);
            assert!(annual.contains(&format!("\"parents\":[\"{}\"]", total.claim.claim_id)));
            assert!(annual.contains(&total.key.reporting_vintage.file_sha256));
            assert_eq!(result, invoke_capex(&root, &[]));

            for selector in ["--as-of", "--cohort-as-of"] {
                let dated = invoke_capex(&root, &[selector, "2025-03-01"]);
                assert_eq!(dated.exit_code, 0, "{}", dated.to_json());
                let site = if selector == "--as-of" {
                    "site-a"
                } else {
                    "Site A"
                };
                for (metric, cell) in [
                    ("capex_usd_m", "B7"),
                    ("capex_sustaining_usd_m", "B5"),
                    ("capex_projects_usd_m", "B6"),
                ] {
                    let start = dated
                        .payload_json
                        .find(&format!(
                            "{{\"site\":\"{site}\",\"year\":2024,\"metric\":\"{metric}\""
                        ))
                        .unwrap();
                    let annual = object(&dated.payload_json, start);
                    assert!(
                        annual.contains(&format!("{ORIGINAL}:Minestats1!{cell}")),
                        "{annual}"
                    );
                    assert!(annual.contains("\"score\":{\""), "{annual}");
                    assert!(
                        annual.contains("\"side\":\"inside\"")
                            || annual.contains("\"side\":\"Inside\""),
                        "{annual}"
                    );
                    assert!(
                        !annual.contains(LATER),
                        "future comparative cannot enter a dated row: {annual}"
                    );
                }
                let markdown = dated.rendered_markdown.unwrap();
                assert!(markdown.contains("capex\\_usd\\_m") || markdown.contains("capex_usd_m"));
                assert!(markdown.contains("issuer-reported-total"));
            }
            for (format, expected) in [
                ("json", result.to_json()),
                ("markdown", result.rendered_markdown.unwrap()),
            ] {
                let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
                    .args([
                        "track-record",
                        "--issuer",
                        "capexcut",
                        "--format",
                        format,
                        "--workspace",
                    ])
                    .arg(&root)
                    .output()
                    .unwrap();
                assert_eq!(output.status.code(), Some(0));
                assert_eq!(
                    String::from_utf8(output.stdout).unwrap(),
                    format!("{expected}\n")
                );
            }
        }
    }

    #[test]
    fn capex_never_replaces_missing_originals_or_converts_ownership_units_and_scopes() {
        let missing = capex_workspace("capex-missing", "Gold", None);
        let result = invoke_capex(&missing, &[]);
        assert_eq!(result.exit_code, 0, "{}", result.to_json());
        let annual = row(&result.payload_json, "capex_usd_m", 2024);
        assert!(annual.contains("\"state\":\"unresolved\""));
        let score = object(annual.split_once("\"score\":").unwrap().1, 0);
        assert!(score.contains("\"value\":null,\"claim\":null"));
        assert!(annual.contains("\"interpreted_outcome_claim\":null"));
        assert!(annual.contains(LATER));
        assert!(
            row(&result.payload_json, "capex_projects_usd_m", 2024).contains("\"side\":\"inside\"")
        );

        for (tag, from, to, code) in [
            (
                "ownership",
                "attributable:3/4;capex=issuer-reported-total",
                "100%;capex=issuer-reported-total",
                "FMINE-REF-105",
            ),
            (
                "currency",
                "unit = \"usd_m\"",
                "unit = \"cad_m\"",
                "FMINE-REF-105",
            ),
            (
                "scope",
                ";capex=issuer-reported-total",
                ";capex=issuer-reported-projects",
                "FMINE-REF-105",
            ),
            (
                "unstated",
                ";capex=issuer-reported-total",
                "",
                "FMINE-REF-105",
            ),
        ] {
            let root = capex_workspace(&format!("capex-{tag}"), "Copper", Some(100.0));
            let path = transcriptions_path(&root, "capexcut");
            let source = std::fs::read_to_string(&path).unwrap();
            std::fs::write(&path, source.replace(from, to)).unwrap();
            let result = invoke_capex(&root, &[]);
            assert!(
                result.to_json().contains(code),
                "{tag}: {}",
                result.to_json()
            );
            if result.exit_code == 0 {
                let annual = row(&result.payload_json, "capex_usd_m", 2024);
                let score = object(annual.split_once("\"score\":").unwrap().1, 0);
                assert!(
                    score.contains("\"value\":null,\"claim\":null"),
                    "{tag}: {score}"
                );
                assert!(annual.contains("\"interpreted_outcome_claim\":null"));
            }
        }
    }

    #[test]
    fn as_of_ignores_future_workbook_bytes_but_verifies_included_sources() {
        let root = capex_workspace("asof-workbook-cut", "Gold", Some(100.0));
        let cut = ["--as-of", "2025-03-01"];
        let baseline = invoke_capex(&root, &cut);
        assert_eq!(baseline.exit_code, 0, "{}", baseline.to_json());
        assert!(
            baseline
                .payload_json
                .contains("\"workbooks_available_by\":\"2025-02-01\"")
        );
        assert!(
            baseline
                .payload_json
                .contains("\"digest_scope\":\"base_comparison_and_both_price_selections\"")
        );
        let future_start = baseline
            .payload_json
            .find(&format!("{{\"file\":\"{LATER}\""))
            .unwrap();
        let future = object(&baseline.payload_json, future_start);
        assert!(future.contains("\"available_by\":\"2026-02-01\""));
        assert!(future.contains("\"basis_kind\":\"registered_publication\""));
        assert!(future.contains("\"eligible\":false"));
        let annual_start = baseline
            .payload_json
            .find("{\"site\":\"site-a\",\"year\":2024,\"metric\":\"capex_usd_m\"")
            .unwrap();
        let annual = object(&baseline.payload_json, annual_start);
        assert!(annual.contains(&format!("{ORIGINAL}:Minestats1!B7")));
        assert!(annual.contains("\"score\":{\""));
        assert!(!annual.contains(LATER));
        let cohort_cut = ["--cohort-as-of", "2025-03-01"];
        let cohort_baseline = invoke_capex(&root, &cohort_cut);
        assert_eq!(
            cohort_baseline.exit_code,
            0,
            "{}",
            cohort_baseline.to_json()
        );

        let future_path = root.join("corpus").join(LATER);
        std::fs::write(&future_path, b"altered future workbook").unwrap();
        assert_eq!(
            invoke_capex(&root, &cut),
            baseline,
            "future bytes must not affect either report digest or presentation"
        );
        assert_eq!(invoke_capex(&root, &cohort_cut), cohort_baseline);
        std::fs::rename(&future_path, root.join("corpus/FutureHeldAside.xlsx")).unwrap();
        assert_eq!(
            invoke_capex(&root, &cut),
            baseline,
            "an unavailable future workbook must not affect a historical report"
        );
        assert_eq!(invoke_capex(&root, &cohort_cut), cohort_baseline);

        // Publication, rather than local file availability, decides which bytes must verify.
        let advanced = invoke_capex(&root, &["--as-of", "2026-03-01"]);
        let refusal = advanced.refusal.as_ref().expect("included file is missing");
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(refusal.what.contains(LATER));
        std::fs::write(
            root.join("corpus").join(ORIGINAL),
            b"altered included workbook",
        )
        .unwrap();
        let altered = invoke_capex(&root, &cut);
        let refusal = altered
            .refusal
            .as_ref()
            .expect("included source tampering must fail");
        assert_eq!(refusal.code, "FMINE-REF-505");
        assert!(refusal.what.contains(ORIGINAL));
        assert!(!refusal.what.contains(LATER));
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

    #[test]
    fn one_sided_guidance_reaches_the_binary_without_invented_range_statistics() {
        let root = dated_workspace("one-sided");
        // An imaginary registered issuer exercises admission of these exact fixture bytes. The
        // reserved synthetic cohort lane uses built-in evidence instead of workspace readings.
        let source = std::fs::read_to_string(registry_path(&root, "synthetic"))
            .unwrap()
            .replace("issuer = \"synthetic\"", "issuer = \"boundcut\"");
        std::fs::write(registry_path(&root, "boundcut"), source).unwrap();
        let path = transcriptions_path(&root, "boundcut");
        let readings = std::fs::read_to_string(transcriptions_path(&root, "synthetic"))
            .unwrap()
            .replace("issuer = \"synthetic\"", "issuer = \"boundcut\"")
            .replace("token = \"80 - 100\"", "token = \"at least 90\"")
            .replace("token = \"90 - 110\"", "token = \">= 100\"")
            .replace("token = \"900 - 1100\"", "token = \"≤1000\"");
        std::fs::write(&path, &readings).unwrap();
        let invoke_bound = |extra: &[&str]| {
            let args: Vec<String> = ["track-record", "--issuer", "boundcut"]
                .into_iter()
                .chain(extra.iter().copied())
                .map(str::to_owned)
                .collect();
            let (envelope, format) = fmine_cli::invoke(&args, &root);
            assert_eq!(format, OutputFormat::Json);
            envelope
        };
        let expected = invoke_bound(&[]);
        assert_eq!(expected.exit_code, 0, "{}", expected.to_json());
        let annual = row(&expected.payload_json, "production_koz", 2024);
        assert!(annual.contains("\"kind\":\"at_least\""));
        assert!(annual.contains("\"side\":\"inside\""));
        assert!(annual.contains("\"boundary\":true"));
        assert!(annual.contains("\"z\":null"));
        assert!(annual.contains("\"midpoint\":null"));
        assert!(annual.contains("Synthetic_2024Q4.xlsx:YTD!C5"));
        let cost = row(&expected.payload_json, "aisc_usd_per_oz", 2024);
        assert!(cost.contains("\"kind\":\"at_most\""));
        assert!(cost.contains("\"token\":\"≤1000\""));
        assert!(cost.contains("\"side\":\"inside\""));
        assert!(cost.contains("\"z\":null"));
        for (format, expected_text) in [
            ("json", expected.to_json()),
            ("markdown", expected.rendered_markdown.clone().unwrap()),
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
                .args([
                    "track-record",
                    "--issuer",
                    "boundcut",
                    "--format",
                    format,
                    "--workspace",
                ])
                .arg(&root)
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                format!("{expected_text}\n")
            );
        }
        let cut = invoke_bound(&["--cohort-as-of", "2026-02-02"]);
        assert_eq!(cut.exit_code, 0, "{}", cut.to_json());
        assert!(cut.payload_json.contains("\"token\":\">= 100\""));
        assert!(cut.payload_json.contains("\"z\":null"));
        assert!(
            cut.payload_json
                .contains("\"side\":\"Inside\",\"hit\":\"Boundary\",\"boundary\":true")
        );
        assert!(
            cut.payload_json
                .contains("Guidance2024.pdf:p1:Invented guidance:Site A production_koz/2024")
        );
        assert!(cut.payload_json.contains("Synthetic_2024Q4.xlsx:YTD!C5"));
        assert!(cut.payload_json.contains(&sha256(DOCUMENT_2024).to_hex()));
        assert!(
            expected
                .rendered_markdown
                .unwrap()
                .contains("Point and one-sided guidance have no z")
        );

        std::fs::write(
            &path,
            readings.replacen("token = \">= 100\"", "token = \"<= 100\"", 1),
        )
        .unwrap();
        let disagreement = invoke_bound(&[]);
        assert_ne!(disagreement.exit_code, 0);
        assert_eq!(disagreement.refusal.unwrap().code, "FMINE-REF-202");
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
