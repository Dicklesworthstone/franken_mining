//! Registered two-reader technical scenarios through the public guidance entry point (BD-342).
//! Every workbook, PDF byte and transcription below is invented test evidence.

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::path::{Path, PathBuf};

    use fmine_core::hash::sha256;
    use fmine_evidence::{EvidenceClass, KnowledgeState, Refusal};
    use fmine_io::xlsx::{MinimalCell, MinimalSheet, build_minimal_xlsx};
    use fmine_metrics::capacity::CapacityVerdict;
    use fmine_metrics::guidance::{
        CostMetric, EnvelopeTier, HistoryVerdict, Ownership, Period, QuantityKind,
        SpendingRequirement, quarters,
    };
    use fmine_numerics::exact_feas::Rat;
    use fmine_public::guidance::capacity::{
        TechnicalAssessment, TechnicalComparison, TechnicalScenarioKind,
    };
    use fmine_public::guidance::{self, GuidanceCompletionReport};
    use fmine_public::sources::{load_issuer_profile, registry_path, transcriptions_path};
    use fmine_public::transcribe::load_transcription_quantities_as_of;

    const ISSUER: &str = "technical_fixture";
    const BASIS: &str = "100%;scenario=declared_upper_bound";

    #[derive(Clone)]
    struct Row {
        doc: &'static str,
        col: &'static str,
        token: &'static str,
        quantity: &'static str,
        unit: &'static str,
        basis: &'static str,
        period: &'static str,
    }

    fn document_bytes(file: &str) -> Vec<u8> {
        format!("%PDF-1.7 Invented {file} bytes; never a real technical report.").into_bytes()
    }

    fn entry(row: &Row, reader: &str) -> String {
        format!(
            "\n[[entry]]\ndoc = \"{}\"\ndoc_sha256 = \"{}\"\npage = 1\n\
             table = \"Invented explicit scenario\"\nrow = \"Example Mine\"\ncol = \"{}\"\n\
             token = \"{}\"\nquantity = \"{}\"\nunit = \"{}\"\nbasis = \"{}\"\n\
             period = \"{}\"\nreader = \"{reader}\"\n",
            row.doc,
            sha256(&document_bytes(row.doc)).to_hex(),
            row.col,
            row.token,
            row.quantity,
            row.unit,
            row.basis,
            row.period,
        )
    }

    fn guidance_rows() -> Vec<Row> {
        vec![
            Row {
                doc: "Guidance.pdf",
                col: "Production",
                token: "200 - 230",
                quantity: "guidance_example_mine_production_koz",
                unit: "koz",
                basis: "attributable:4/5",
                period: "2030FY",
            },
            Row {
                doc: "Guidance.pdf",
                col: "AISC",
                token: "1520 - 1680",
                quantity: "guidance_example_mine_aisc_usd_per_oz",
                unit: "usd_per_oz",
                basis: "attributable:4/5;aisc=issuer-reported-per-sold-oz",
                period: "2030FY",
            },
        ]
    }

    fn technical_rows() -> Vec<Row> {
        [
            ("Dry feed", "10", "remaining_dry_feed_mt", "dry_mt"),
            ("Head grade", "2", "head_grade_g_per_t", "g_per_dry_t"),
            ("Recovery", "90", "recovery_percent", "percent"),
        ]
        .into_iter()
        .map(|(col, token, role, unit)| Row {
            doc: "Technical.pdf",
            col,
            token,
            quantity: match role {
                "remaining_dry_feed_mt" => "technical_example_mine_remaining_dry_feed_mt",
                "head_grade_g_per_t" => "technical_example_mine_head_grade_g_per_t",
                _ => "technical_example_mine_recovery_percent",
            },
            unit,
            basis: BASIS,
            period: "2030H2",
        })
        .collect()
    }

    fn write_rows(root: &Path, rows: &[Row], extra: &str) {
        let mut text = format!("schema = \"fmine.transcriptions.v1\"\nissuer = \"{ISSUER}\"\n");
        for row in rows {
            for reader in ["independent-reader-a", "independent-reader-b"] {
                text.push_str(&entry(row, reader));
            }
        }
        text.push_str(extra);
        std::fs::write(transcriptions_path(root, ISSUER), text).unwrap();
    }

    fn register_document(root: &Path, file: &str, kind: &str, date: &str, present: bool) {
        let bytes = document_bytes(file);
        if present {
            std::fs::write(root.join("corpus").join(file), &bytes).unwrap();
        }
        let path = registry_path(root, ISSUER);
        let mut text = std::fs::read_to_string(&path).unwrap();
        write!(
            text,
            "\n[[document]]\nfile = \"{file}\"\nkind = \"{kind}\"\n\
             url = \"https://example.invalid/{file}\"\nurl_basis = \"W1 table\"\n\
             published_at = \"{date}\"\npublished_at_basis = \"FACT: invented test publication\"\n\
             pages = 1\nbytes = {}\nsha256 = \"{}\"\n",
            bytes.len(),
            sha256(&bytes).to_hex(),
        )
        .unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn sheet(name: &str, year: &str, months: &str, end: &str, values: [&str; 3]) -> MinimalSheet {
        let text = |s: &str| MinimalCell::Text(s.into());
        let number = |s: &str| MinimalCell::Styled {
            raw: s.into(),
            xf: 0,
        };
        let mut cells = vec![
            (
                "A3".into(),
                text(&format!("For the {months} months ended {end},")),
            ),
            ("B3".into(), number(year)),
            ("A5".into(), text("Example Mine (80%)")),
        ];
        for ((col, header), value) in [
            ("B", "Gold produced (000s oz attributable)"),
            ("C", "Gold sold (000s oz attributable)"),
            ("D", "All-in sustaining costs ($/oz)"),
        ]
        .into_iter()
        .zip(values)
        {
            cells.push((format!("{col}4"), text(header)));
            cells.push((format!("{col}5"), number(value)));
        }
        MinimalSheet {
            name: name.into(),
            hidden: false,
            cells,
        }
    }

    fn workspace(tag: &str) -> PathBuf {
        workspace_with_tcc(tag, None, true)
    }

    fn tcc_row(signed: bool) -> Row {
        Row {
            doc: "Guidance.pdf",
            col: "Total cash costs",
            token: if signed { "-900 - -800" } else { "800 - 900" },
            quantity: "guidance_example_mine_tcc_usd_per_oz",
            unit: "usd_per_oz",
            basis: "attributable:4/5;tcc=issuer-reported-per-sold-oz",
            period: "2030FY",
        }
    }

    fn workspace_with_tcc(tag: &str, signed_tcc: Option<bool>, historical_tcc: bool) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("fmine-technical-{tag}-{}", std::process::id()));
        for dir in ["corpus", "registries/sources", "registries/transcriptions"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        let mut registry = format!(
            "schema = \"fmine.sources.v1\"\nissuer = \"{ISSUER}\"\ncorpus_dir = \"corpus\"\n\
             retrieved_utc = \"2030-07-31T00:00:00Z\"\nurl_prefix = \"https://example.invalid/\"\n\
             [[subject]]\nname = \"Example Mine\"\n"
        );
        for (release, mut sheets) in [
            (
                "2029Q2",
                vec![sheet("YTD", "2029", "six", "June 30", ["70", "70", "1400"])],
            ),
            (
                "2029Q3",
                vec![sheet(
                    "QTD",
                    "2029",
                    "three",
                    "September 30",
                    ["70", "70", "1300"],
                )],
            ),
            (
                "2029Q4",
                vec![
                    sheet("QTD", "2029", "three", "December 31", ["70", "70", "1350"]),
                    sheet(
                        "YTD",
                        "2029",
                        "twelve",
                        "December 31",
                        ["210", "210", "1400"],
                    ),
                ],
            ),
            (
                "2030Q2",
                vec![sheet("YTD", "2030", "six", "June 30", ["68", "60", "2283"])],
            ),
        ] {
            if let Some(signed) = signed_tcc
                && (historical_tcc || release == "2030Q2")
            {
                let token = match (release == "2030Q2", signed) {
                    (true, false) => "1200",
                    (true, true) => "-1200",
                    (false, false) => "700",
                    (false, true) => "-700",
                };
                for sheet in &mut sheets {
                    sheet.cells.push((
                        "E4".into(),
                        MinimalCell::Text("Total cash costs ($/oz)".into()),
                    ));
                    sheet.cells.push((
                        "E5".into(),
                        MinimalCell::Styled {
                            raw: token.into(),
                            xf: 0,
                        },
                    ));
                }
            }
            let file = format!("{release}.xlsx");
            let bytes = build_minimal_xlsx(&sheets);
            std::fs::write(root.join("corpus").join(&file), &bytes).unwrap();
            write!(
                registry,
                "\n[[source]]\nfile = \"{file}\"\nkind = \"mine_stats\"\nquarter = \"{release}\"\n\
                 path = \"{file}\"\nurl_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{}\"\n",
                bytes.len(),
                sha256(&bytes).to_hex(),
            )
            .unwrap();
        }
        std::fs::write(registry_path(&root, ISSUER), registry).unwrap();
        register_document(&root, "Guidance.pdf", "mda", "2030-01-15", true);
        register_document(
            &root,
            "Technical.pdf",
            "technical_report",
            "2030-06-30",
            true,
        );
        let mut rows = guidance_rows();
        if let Some(signed) = signed_tcc {
            rows.push(tcc_row(signed));
        }
        rows.extend(technical_rows());
        write_rows(&root, &rows, "");
        root
    }

    fn run(root: &Path, as_of: &str) -> Result<GuidanceCompletionReport, Refusal> {
        guidance::run_public(root, ISSUER, "example-mine", "2030Q2", as_of)
    }

    fn run_tcc(root: &Path) -> Result<GuidanceCompletionReport, Refusal> {
        guidance::run_public_with_metric(
            root,
            ISSUER,
            "example-mine",
            "2030Q2",
            "2030-07-31",
            CostMetric::Tcc,
        )
    }

    #[test]
    fn gold_tcc_registered_sources_keep_signed_exact_requirements_and_selected_metric_in_cli() {
        for signed in [false, true] {
            let root = workspace_with_tcc(
                if signed { "tcc-signed" } else { "tcc-positive" },
                Some(signed),
                true,
            );
            let report = run_tcc(&root).unwrap();
            let aisc = run(&root, "2030-07-31").unwrap();
            let interval = |lo, hi| fmine_numerics::exact_feas::Interval {
                lo,
                hi,
                lo_strict: false,
                hi_strict: false,
            };
            let q = |n, d| Rat::new(n, d).unwrap();
            let sign_bounds = |lo, hi| {
                if signed {
                    interval(
                        Rat::from_integer(0).checked_sub(hi).unwrap(),
                        Rat::from_integer(0).checked_sub(lo).unwrap(),
                    )
                } else {
                    interval(lo, hi)
                }
            };
            // Independent half-step arithmetic: ytd = [59.5,60.5] koz * [1199.5,1200.5] USD/oz.
            // Remaining sales are 210±0.5 minus 70±0.5 = [139,141] koz. For each annual
            // target A, future spend is (A*R + (A-r)*S)/1000, preserving the same S in both terms.
            assert_eq!(
                report.inputs.ytd_aisc.value.kind,
                QuantityKind::TccUsdPerSoldOz
            );
            assert_eq!(report.inputs.ytd_aisc.metric_key, "summary:tcc_usd_per_oz");
            assert_eq!(
                report.inputs.ytd_aisc.value.interval,
                sign_bounds(q(2399, 2), q(2401, 2))
            );
            assert_eq!(
                report.completion.ytd_aisc_spend.value.interval,
                sign_bounds(q(285481, 4000), q(290521, 4000))
            );
            assert_eq!(
                report.completion.remaining_aisc_spend.value.interval,
                sign_bounds(q(347879, 4000), q(436319, 4000))
            );
            assert_eq!(
                report.completion.remaining_aisc_rate.value.interval,
                sign_bounds(q(347879, 556), q(436319, 564))
            );
            assert_eq!(
                report.completion.remaining_aisc_spend.value.kind,
                QuantityKind::TccSpendUsdMillion
            );
            assert_eq!(
                report.completion.spending_requirement,
                if signed {
                    SpendingRequirement::EntirelyNegative
                } else {
                    SpendingRequirement::Nonnegative
                }
            );
            assert_eq!(
                report.completion.remaining_production,
                aisc.completion.remaining_production
            );
            assert_ne!(report.digest, aisc.digest);
            assert_eq!(
                (report.history.tier, report.history.verdict),
                (EnvelopeTier::T1, HistoryVerdict::WithinEnvelope)
            );
            let source = &report.history_sources[0];
            let h = quarters::QuarterlyHistory {
                id: source.id.clone(),
                quarters: source.quarters.clone(),
            };
            let witness = &report.history.checks[0].witness.as_ref().unwrap().value;
            assert!(quarters::check_quarter_witness(&report.inputs, &h, witness).unwrap());
            for quarter in &source.quarters {
                assert_eq!(quarter.aisc.value.kind, QuantityKind::TccUsdPerSoldOz);
                assert_eq!(quarter.aisc.metric_key, "summary:tcc_usd_per_oz");
            }
            let json = report.to_json();
            for expected in [
                "\"schema\":\"fmine.guidance.completion.v3\"",
                "\"cost_metric\":\"tcc\"",
                "\"tcc_guidance\"",
                "\"ytd_tcc\"",
                "\"ytd_tcc_spend\"",
                "\"remaining_tcc_rate\"",
                "\"remaining_tcc_spend\"",
                "\"annual_tcc\"",
                "\"tcc_usd_per_sold_oz\"",
                "\"tcc_spend_usd_m\"",
                "2030Q2.xlsx:YTD!E5",
                "2029Q3.xlsx:QTD!E5",
            ] {
                assert!(json.contains(expected), "missing {expected}: {json}");
            }
            for wrong in [
                "\"aisc_guidance\"",
                "\"ytd_aisc\"",
                "\"remaining_aisc_rate\"",
                "2030Q2.xlsx:YTD!D5",
                "2029Q3.xlsx:QTD!D5",
            ] {
                assert!(!json.contains(wrong), "TCC borrowed AISC {wrong}: {json}");
            }
            assert_eq!(json, run_tcc(&root).unwrap().to_json());
            let mut args: Vec<_> = [
                "guidance",
                "completion",
                "--issuer",
                ISSUER,
                "--site",
                "example-mine",
                "--through",
                "2030Q2",
                "--as-of",
                "2030-07-31",
                "--metric",
                "tcc",
            ]
            .into_iter()
            .map(str::to_string)
            .collect();
            let (envelope, _) = fmine_cli::invoke(&args, &root);
            assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
            assert_eq!(envelope.payload_json, json);
            args.extend(["--format".into(), "markdown".into()]);
            let (markdown, exit) = fmine_cli::run(&args, &root);
            assert_eq!(exit, 0, "{markdown}");
            for label in [
                "Annual TCC guidance",
                "YTD TCC spending",
                "Required remaining TCC",
                "USD per sold oz",
                "joint production/TCC witness",
                "separate TCC frontier",
                "Checked witness",
            ] {
                assert!(markdown.contains(label), "missing {label}: {markdown}");
            }
            assert!(!markdown.contains("AISC"), "{markdown}");
        }
    }

    #[test]
    fn tcc_never_borrows_aisc_for_missing_guidance_actuals_or_history() {
        let root = workspace("tcc-absent");
        assert_eq!(run_tcc(&root).unwrap_err().code, "FMINE-REF-202");
        let mut rows = guidance_rows();
        rows.push(tcc_row(false));
        rows.extend(technical_rows());
        write_rows(&root, &rows, "");
        assert_eq!(run_tcc(&root).unwrap_err().code, "FMINE-REF-201");
        assert!(run(&root, "2030-07-31").is_ok());

        let root = workspace_with_tcc("tcc-no-history", Some(false), false);
        let report = run_tcc(&root).unwrap();
        assert_eq!(
            (report.history.tier, report.history.verdict),
            (EnvelopeTier::T3, HistoryVerdict::Indeterminate)
        );
        assert!(report.history_sources.is_empty());
        assert_eq!(report.history_exclusions.len(), 1);
        assert!(
            report.history_exclusions[0]
                .reason
                .contains("summary:tcc_usd_per_oz")
        );
        assert_eq!(
            run(&root, "2030-07-31").unwrap().history.verdict,
            HistoryVerdict::WithinEnvelope
        );

        let root = workspace_with_tcc("tcc-declarations", Some(false), true);
        for (basis, unit, code) in [
            (
                "attributable:4/5;aisc=issuer-reported-per-sold-oz",
                "usd_per_oz",
                "FMINE-REF-107",
            ),
            (
                "attributable:4/5;tcc=issuer-reported-per-produced-oz",
                "usd_per_oz",
                "FMINE-REF-107",
            ),
            (
                "attributable:4/5;tcc=issuer-reported-per-sold-oz",
                "usd_per_lb",
                "FMINE-REF-105",
            ),
            (
                "attributable:3/5;tcc=issuer-reported-per-sold-oz",
                "usd_per_oz",
                "FMINE-REF-105",
            ),
        ] {
            let mut row = tcc_row(false);
            row.basis = basis;
            row.unit = unit;
            let mut rows = guidance_rows();
            rows.push(row);
            rows.extend(technical_rows());
            write_rows(&root, &rows, "");
            assert_eq!(run_tcc(&root).unwrap_err().code, code, "{basis} {unit}");
            assert!(run(&root, "2030-07-31").is_ok());
        }
    }

    fn comparison(report: &GuidanceCompletionReport) -> &TechnicalComparison {
        let TechnicalAssessment::Compared(c) = &report.technical else {
            panic!("expected T2 comparison: {:?}", report.technical)
        };
        c
    }

    fn reason(report: &GuidanceCompletionReport) -> &Refusal {
        let TechnicalAssessment::Indeterminate { reason } = &report.technical else {
            panic!("expected T3 indeterminate: {:?}", report.technical)
        };
        reason
    }

    fn unchanged_core(before: &GuidanceCompletionReport, after: &GuidanceCompletionReport) {
        assert_eq!(before.inputs, after.inputs);
        assert_eq!(before.completion, after.completion);
        assert_eq!(before.history, after.history);
    }

    #[test]
    fn registered_technical_report_binds_exact_period_units_share_and_source_claims() {
        let root = workspace("exact");
        let report = run(&root, "2030-07-31").unwrap();
        let c = comparison(&report);
        assert_eq!(report.technical.tier(), "T2");
        assert_eq!(c.kind, TechnicalScenarioKind::DeclaredUpperBound);
        assert_eq!(c.document.kind, "technical_report");
        assert_eq!(c.result.verdict, CapacityVerdict::NotExcluded);
        // 10 million dry t × 2 g/t × 90% / 31.1034768 g/oz / 1000 oz/koz × 4/5.
        let maximum = Rat::new(3_000_000_000, 6_479_891).unwrap();
        assert_eq!(c.result.maximum_production.value.interval.lo, maximum);
        assert_eq!(c.result.maximum_production.value.interval.hi, maximum);
        assert_eq!(
            c.limits.recovery.value.interval.lo,
            Rat::new(9, 10).unwrap()
        );
        assert_eq!(c.limits.feed.value.interval.lo, Rat::from_integer(10));
        assert_eq!(c.limits.grade.value.interval.lo, Rat::from_integer(2));
        let required = &c.result.optimistic_required_grade.value;
        assert_eq!(
            required.interval.lo,
            Rat::new(263 * 311_034_768, 2 * 10_000_000 * 7_200).unwrap()
        );
        assert_eq!(
            required.interval.hi,
            Rat::new(325 * 311_034_768, 2 * 10_000_000 * 7_200).unwrap()
        );
        assert_eq!(c.sources.len(), 3);
        for limit in [&c.limits.feed, &c.limits.grade, &c.limits.recovery] {
            assert_eq!(limit.value.ownership, Ownership::Full);
            assert_eq!(
                limit.value.period,
                Period {
                    year: 2030,
                    first_month: 7,
                    last_month: 12
                }
            );
            assert_eq!(limit.evidence, EvidenceClass::ISSUER);
            assert_eq!(limit.state, KnowledgeState::Known);
            assert_eq!(
                limit.anchors[0].artifact_sha256,
                sha256(&document_bytes("Technical.pdf")).0
            );
            assert!(limit.anchors[0].locator.starts_with("Technical.pdf:p1:"));
            assert!(
                c.result
                    .maximum_production
                    .parents
                    .contains(&limit.claim_id)
            );
        }
        assert!(
            c.result
                .maximum_production
                .parents
                .contains(&report.completion.remaining_production.claim_id)
        );
        assert_eq!(
            (report.history.tier, report.history.verdict),
            (EnvelopeTier::T1, HistoryVerdict::WithinEnvelope)
        );
        let json = report.to_json();
        assert!(json.contains("\"physical_impossibility_claim\":false"));
        assert!(json.contains("\"joint_cost_feasibility_claim\":false"));
        assert!(json.contains("\"gate_eligible\":false"));
        assert_eq!(json, run(&root, "2030-07-31").unwrap().to_json());
    }

    #[test]
    fn plan_and_nameplate_remain_rounded_conditional_scenarios() {
        let root = workspace("scenario-kind");
        let original = run(&root, "2030-07-31").unwrap();
        for (basis, kind) in [
            ("100%;scenario=plan", TechnicalScenarioKind::Plan),
            ("100%;scenario=nameplate", TechnicalScenarioKind::Nameplate),
        ] {
            let mut technical = technical_rows();
            for row in &mut technical {
                row.basis = basis;
            }
            let mut rows = guidance_rows();
            rows.extend(technical);
            write_rows(&root, &rows, "");
            let report = run(&root, "2030-07-31").unwrap();
            let c = comparison(&report);
            unchanged_core(&original, &report);
            assert_eq!(c.kind, kind);
            assert_eq!(c.limits.feed.value.interval.lo, Rat::new(19, 2).unwrap());
            assert_eq!(c.limits.feed.value.interval.hi, Rat::new(21, 2).unwrap());
            assert_eq!(
                c.limits.recovery.value.interval.hi,
                Rat::new(181, 200).unwrap()
            );
            assert_eq!(c.limits.feed.state, KnowledgeState::Estimated);
            assert!(
                kind.interpretation()
                    .contains("conditional scenario parameters")
            );
            assert_ne!(report.digest, original.digest);
            assert!(report.to_json().contains("not a physical maximum"));
        }
        let mut technical = technical_rows();
        technical[0].token = "1";
        let mut rows = guidance_rows();
        rows.extend(technical);
        write_rows(&root, &rows, "");
        let excluded = run(&root, "2030-07-31").unwrap();
        assert_eq!(
            comparison(&excluded).result.verdict,
            CapacityVerdict::ExceedsStatedProductionCeiling
        );
        assert!(
            excluded
                .to_json()
                .contains("exceeds_stated_declared_upper_bound_scenario")
        );
        unchanged_core(&original, &excluded);
    }

    #[test]
    fn missing_and_invalid_technical_inputs_preserve_arithmetic_and_history() {
        let root = workspace("invalid-inputs");
        let original = run(&root, "2030-07-31").unwrap();
        write_rows(&root, &guidance_rows(), "");
        let missing = run(&root, "2030-07-31").unwrap();
        assert_eq!(reason(&missing).code, "FMINE-REF-201");
        assert_eq!(missing.technical.tier(), "T3");
        unchanged_core(&original, &missing);
        for (field, value, expected) in [
            ("period", "2030FY", "FMINE-REF-102"),
            ("unit", "mtpa", "FMINE-REF-105"),
            ("unit", "wet_mt", "FMINE-REF-105"),
            (
                "basis",
                "attributable:4/5;scenario=declared_upper_bound",
                "FMINE-REF-105",
            ),
            ("basis", "100%", "FMINE-REF-105"),
            ("feed", "0", "FMINE-REF-109"),
            ("feed", "-1", "FMINE-REF-109"),
            ("recovery", "101", "FMINE-REF-109"),
        ] {
            let mut technical = technical_rows();
            match field {
                "period" => technical[0].period = value,
                "unit" => technical[0].unit = value,
                "basis" => technical[0].basis = value,
                "feed" => technical[0].token = value,
                _ => technical[2].token = value,
            }
            let mut rows = guidance_rows();
            rows.extend(technical);
            write_rows(&root, &rows, "");
            let report = run(&root, "2030-07-31").unwrap();
            assert_eq!(reason(&report).code, expected, "{field}={value}");
            unchanged_core(&original, &report);
        }
    }

    #[test]
    fn missing_altered_and_disagreeing_optional_document_bytes_stay_typed_t3() {
        let root = workspace("source-failures");
        let original = run(&root, "2030-07-31").unwrap();
        register_document(
            &root,
            "Missing.pdf",
            "technical_report",
            "2030-07-01",
            false,
        );
        let mut technical = technical_rows();
        for row in &mut technical {
            row.doc = "Missing.pdf";
        }
        let mut rows = guidance_rows();
        rows.extend(technical.clone());
        write_rows(&root, &rows, "");
        let missing = run(&root, "2030-07-31").unwrap();
        assert_eq!(reason(&missing).code, "FMINE-REF-506");
        unchanged_core(&original, &missing);
        let mut altered = document_bytes("Missing.pdf");
        altered[0] ^= 1;
        std::fs::write(root.join("corpus/Missing.pdf"), altered).unwrap();
        let altered = run(&root, "2030-07-31").unwrap();
        assert_eq!(reason(&altered).code, "FMINE-REF-505");
        unchanged_core(&original, &altered);
        std::fs::write(
            root.join("corpus/Missing.pdf"),
            document_bytes("Missing.pdf"),
        )
        .unwrap();
        let mut disagreement = technical[0].clone();
        disagreement.token = "11";
        write_rows(&root, &rows, &entry(&disagreement, "independent-reader-c"));
        let disagreement = run(&root, "2030-07-31").unwrap();
        assert_eq!(reason(&disagreement).code, "FMINE-REF-202");
        unchanged_core(&original, &disagreement);
        // Required guidance errors are never swallowed by the optional technical branch.
        std::fs::write(root.join("corpus/Guidance.pdf"), b"altered guidance bytes").unwrap();
        assert_eq!(run(&root, "2030-07-31").unwrap_err().code, "FMINE-REF-505");
    }

    #[test]
    fn latest_technical_vintage_never_falls_back_over_pending_or_inapplicable_rows() {
        let root = workspace("latest");
        let original = run(&root, "2030-07-31").unwrap();
        register_document(&root, "Updated.pdf", "technical_report", "2030-07-10", true);
        let mut rows = guidance_rows();
        rows.extend(technical_rows());
        let mut latest = technical_rows();
        for row in &mut latest {
            row.doc = "Updated.pdf";
        }
        write_rows(&root, &rows, &entry(&latest[0], "independent-reader-a"));
        let pending = run(&root, "2030-07-31").unwrap();
        assert_eq!(reason(&pending).code, "FMINE-REF-202");
        unchanged_core(&original, &pending);
        let mut partial = rows.clone();
        partial.push(latest[0].clone());
        write_rows(&root, &partial, "");
        assert_eq!(
            reason(&run(&root, "2030-07-31").unwrap()).code,
            "FMINE-REF-202"
        );
        latest[0].period = "2030FY";
        rows.extend(latest);
        write_rows(&root, &rows, "");
        assert_eq!(
            reason(&run(&root, "2030-07-31").unwrap()).code,
            "FMINE-REF-102"
        );
        let path = registry_path(&root, ISSUER);
        let registry = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            path,
            registry.replace(
                "file = \"Updated.pdf\"\nkind = \"technical_report\"",
                "file = \"Updated.pdf\"\nkind = \"investor_deck\"",
            ),
        )
        .unwrap();
        assert_eq!(
            reason(&run(&root, "2030-07-31").unwrap()).code,
            "FMINE-REF-105"
        );
    }

    #[test]
    fn future_optional_failures_are_absent_before_publication_and_typed_afterward() {
        let root = workspace("future");
        let original = run(&root, "2030-07-31").unwrap();
        register_document(&root, "Future.pdf", "technical_report", "2030-08-01", false);
        let mut rows = guidance_rows();
        rows.extend(technical_rows());
        let mut future = technical_rows();
        for row in &mut future {
            row.doc = "Future.pdf";
        }
        rows.extend(future.clone());
        let mut disagree = future[0].clone();
        disagree.token = "11";
        write_rows(&root, &rows, &entry(&disagree, "independent-reader-c"));
        assert_eq!(
            original.to_json(),
            run(&root, "2030-07-31").unwrap().to_json()
        );
        assert_eq!(
            reason(&run(&root, "2030-08-01").unwrap()).code,
            "FMINE-REF-506"
        );
        std::fs::write(root.join("corpus/Future.pdf"), b"altered future bytes").unwrap();
        assert_eq!(
            original.to_json(),
            run(&root, "2030-07-31").unwrap().to_json()
        );
        assert_eq!(
            reason(&run(&root, "2030-08-01").unwrap()).code,
            "FMINE-REF-505"
        );
        std::fs::write(root.join("corpus/Future.pdf"), document_bytes("Future.pdf")).unwrap();
        assert_eq!(
            original.to_json(),
            run(&root, "2030-07-31").unwrap().to_json()
        );
        assert_eq!(
            reason(&run(&root, "2030-08-01").unwrap()).code,
            "FMINE-REF-202"
        );
    }

    #[test]
    fn quantity_scoping_retains_every_reader_at_the_selected_locator() {
        let root = workspace("reader-key");
        let mut rows = guidance_rows();
        rows.extend(technical_rows());
        let mut third = technical_rows()[0].clone();
        third.quantity = "a_different_interpretation_of_the_same_cell";
        write_rows(&root, &rows, &entry(&third, "independent-reader-c"));
        let profile = load_issuer_profile(&root, ISSUER).unwrap();
        assert_eq!(
            load_transcription_quantities_as_of(
                &profile,
                &root,
                "2030-07-31",
                &[technical_rows()[0].quantity.into()],
                None,
            )
            .unwrap_err()
            .code,
            "FMINE-REF-202",
        );
        assert_eq!(
            reason(&run(&root, "2030-07-31").unwrap()).code,
            "FMINE-REF-202"
        );
        let mut third = guidance_rows()[0].clone();
        third.quantity = "another_interpretation_of_the_same_guidance_cell";
        write_rows(&root, &rows, &entry(&third, "independent-reader-c"));
        assert_eq!(run(&root, "2030-07-31").unwrap_err().code, "FMINE-REF-202");
    }

    #[test]
    fn explicit_synthetic_run_compares_technical_limits_without_promoting_evidence() {
        let report = guidance::run_synthetic().unwrap();
        let c = comparison(&report);
        assert_eq!(c.result.verdict, CapacityVerdict::NotExcluded);
        assert_eq!(c.limits.feed.evidence, EvidenceClass::Assumed);
        assert_eq!(c.limits.feed.state, KnowledgeState::Estimated);
        assert_eq!(c.result.maximum_production.evidence, EvidenceClass::Assumed);
        assert_eq!(
            (report.history.tier, report.history.verdict),
            (EnvelopeTier::T1, HistoryVerdict::WithinEnvelope)
        );
        assert!(report.to_json().contains("\"lane\":\"synthetic\""));
        assert!(report.to_json().contains("\"gate_eligible\":false"));
    }
}
