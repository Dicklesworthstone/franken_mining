//! Cross-crate public report and Claim contracts, retained unchanged in the T layer.
#[cfg(test)]
mod tests {
    use fmine_claim::{AdditivityClass, Claim, Representation, SpatialSupport};
    use fmine_core::hash::Digest32;
    use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal};
    use fmine_io::xlsx::{CellCensus, MinimalCell, QuarantineItem, Workbook};
    use fmine_numerics::exact_feas::discover::UnitFinding;
    use fmine_public::aisc::{
        self, Calc, CitedCell, ConsistentBasis, ExplanationBasis, IdentityFamily, IdentityInstance,
        IdentityState, NotPublishedReason, OneCellCheck, OneCellOutcome, RateMiss,
        ReconstructionCheck, RefusedReason, StatedShare, Undecided, footing_discovery,
    };
    use fmine_public::sources::ExpectedRoot;
    use fmine_public::{
        Lane, PublicAssuranceReport, assess, cell_to_public_claim, discovery_json,
        load_issuer_profile, run_public_assurance, run_synthetic_assurance, sources, synthetic,
    };
    use std::path::PathBuf;
    const STATE_CODES: [&str; 6] = ["i", "ii", "iii", "iv", "v", "np"];
    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }
    fn hex(bytes: &[u8; 32]) -> String {
        Digest32(*bytes).to_hex()
    }
    fn state_of<'a>(
        r: &'a PublicAssuranceReport,
        cell: &str,
        family: IdentityFamily,
    ) -> &'a IdentityState {
        let found = r
            .instances
            .iter()
            .find(|i| i.cell == cell && i.family == family)
            .map(|i| &i.state);
        match found {
            Some(s) => s,
            None => panic!("no instance at {cell} {family:?}"),
        }
    }
    fn calc_of(r: &PublicAssuranceReport, cell: &str, family: IdentityFamily) -> Calc {
        match r
            .instances
            .iter()
            .find(|i| i.cell == cell && i.family == family)
        {
            Some(i) => i.calc,
            None => panic!("no instance at {cell} {family:?}"),
        }
    }
    fn one_cell<'a>(r: &'a PublicAssuranceReport, column: &str) -> &'a OneCellCheck {
        let found: Vec<_> = r.one_cell.iter().filter(|c| c.column == column).collect();
        assert_eq!(found.len(), 1, "one check for column {column}");
        found[0]
    }
    /// The synthetic AISC workbook's sha256, from its bytes.
    fn synthetic_aisc_sha() -> String {
        synthetic::synthetic_workbooks(&[])
            .into_iter()
            .find(|w| w.0 == synthetic::SYNTHETIC_AISC_FILE)
            .map(|w| fmine_core::hash::sha256(&w.3).to_hex())
            .expect("synthetic AISC workbook")
    }
    fn kit_readme() -> String {
        std::fs::read_to_string(
            workspace_root()
                .join("repro")
                .join("barrick")
                .join("README.md"),
        )
        .expect("kit README")
    }

    /// BD-339.S4 golden: the kit README's generated section equals the generator's output from the code's row
    /// anchors and the registry. On a mismatch it prints the section to paste; tests never write files.
    #[test]
    fn test_kit_readme_selectors_match_code() {
        let readme = kit_readme();
        let profile = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        let want = sources::kit_selectors_markdown(&profile);
        let got = sources::kit_readme_section(&readme).expect("both markers");
        assert!(
            got == want.trim_matches('\n'),
            "repro/barrick/README.md's generated section differs; paste this between the markers:\n{}\n{}{}",
            sources::KIT_SELECTORS_BEGIN,
            want,
            sources::KIT_SELECTORS_END
        );
    }

    /// BD-339.S4: the golden comparison sees a one-subject drift in either direction.
    #[test]
    fn test_kit_readme_golden_detects_drift() {
        let readme = kit_readme();
        let section = sources::kit_readme_section(&readme).expect("both markers");
        let mut profile = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        let first = profile.subjects[0].clone();
        assert_eq!(
            section,
            sources::kit_selectors_markdown(&profile).trim_matches('\n')
        );
        let renamed = readme.replacen(&format!("\n- {first}\n"), "\n- Renamed subject\n", 1);
        assert_ne!(renamed, readme, "the subject line is in the README");
        assert_ne!(
            sources::kit_readme_section(&renamed).expect("both markers"),
            sources::kit_selectors_markdown(&profile).trim_matches('\n')
        );
        profile.subjects.remove(0);
        assert_ne!(
            section,
            sources::kit_selectors_markdown(&profile).trim_matches('\n')
        );
    }

    /// BD-339.S4: the kit README carries the no-affiliation sentence verbatim and the RISK-31 instruction.
    #[test]
    fn test_kit_readme_states_no_affiliation_and_risk_31() {
        let readme = kit_readme();
        assert!(readme.contains(
            "FrankenMining is not affiliated with Barrick; these are Barrick's public files, fetched from \
             Barrick's host"
        ));
        assert!(
            readme.contains("RISK-31: do not redistribute the files; fetch them from the issuer.")
        );
    }
    /// BD-339.S1 negative control: one flipped hex digit in any compared field is reported, naming the field.
    #[test]
    fn test_expected_root_comparison_rejects_flipped_digit() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let good = ExpectedRoot {
            commit: "0000000".to_string(),
            registry_epoch: r.registry_epoch(),
            state_root: r.ledger_anchor.state_root.to_hex(),
            anchor_id: r.ledger_anchor.anchor_id().to_string(),
            totals: STATE_CODES.map(|c| r.count_state(c) as u64),
            census_digest_of_digests: r.census_digest_of_digests(),
            reason: "synthetic".to_string(),
            evidence: "synthetic".to_string(),
            line: 1,
        };
        assert_eq!(r.check_expected(&good), Ok(()));
        let flip = |s: &str| {
            let mut b = s.as_bytes().to_vec();
            let last = b.len() - 1;
            b[last] = if b[last] == b'0' { b'1' } else { b'0' };
            String::from_utf8(b).expect("ascii")
        };
        let cases: [(&str, ExpectedRoot); 5] = [
            (
                "state_root",
                ExpectedRoot {
                    state_root: flip(&good.state_root),
                    ..good.clone()
                },
            ),
            (
                "anchor_id",
                ExpectedRoot {
                    anchor_id: flip(&good.anchor_id),
                    ..good.clone()
                },
            ),
            (
                "registry_epoch",
                ExpectedRoot {
                    registry_epoch: flip(&good.registry_epoch),
                    ..good.clone()
                },
            ),
            (
                "census_digest_of_digests",
                ExpectedRoot {
                    census_digest_of_digests: flip(&good.census_digest_of_digests),
                    ..good.clone()
                },
            ),
            (
                "totals",
                ExpectedRoot {
                    totals: [good.totals[0] + 1, 0, 0, 0, 0, 0],
                    ..good.clone()
                },
            ),
        ];
        for (field, bad) in cases {
            let why = r.check_expected(&bad).expect_err(field);
            assert!(why.contains(&format!("{field}: run ")), "{field}: {why}");
        }
    }

    #[test]
    fn test_next_actions_text_has_no_hard_coded_count() {
        let actions = run_synthetic_assurance(&[])
            .expect("synthetic")
            .next_actions();
        let text: String = actions.iter().map(|(c, w)| format!("{c} {w} ")).collect();
        assert!(text.contains("registries/sources/barrick.toml"), "{text}");
        assert!(!text.contains(" 18 "), "{text}");
    }
    /// `cell_to_public_claim` on a cell of the synthetic issuer table.
    fn synthetic_claim(row: u32, col: u32) -> Result<Claim<Option<f64>>, Refusal> {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let f = synthetic::SYNTHETIC_AISC_FILE;
        let aisc = r.sources.iter().find(|s| s.file_name == f).expect("aisc");
        let t = synthetic::SYNTHETIC_TABLE_SHEET;
        let sheet = aisc.workbook.sheet(t).expect("sheet");
        let cell = sheet.cell(row, col).expect("cell");
        let additive = AdditivityClass::ExtensiveAdditive;
        cell_to_public_claim(&aisc.workbook, &sheet.name, "m", cell, additive)
    }

    #[test]
    fn test_issuer_cell_is_assertion_issuer_known() {
        let claim = synthetic_claim(5, 2).expect("B5");
        assert_eq!(claim.value, Some(500.0));
        assert_eq!(claim.anchors[0].locator, "Q1 2030!B5");
        assert_eq!(claim.evidence, EvidenceClass::ISSUER);
        assert_eq!(claim.state, KnowledgeState::Known);
        assert_eq!(claim.epistemic, EpistemicStatus::KnownKnown);
    }

    #[test]
    fn test_placeholder_cell_is_assumed_unknown() {
        let claim = synthetic_claim(6, 6).expect("F6");
        assert_eq!(claim.value, None);
        assert_eq!(claim.evidence, EvidenceClass::Assumed);
        assert_eq!(claim.state, KnowledgeState::Unknown);
    }

    /// "Reproduced" means the issuer's figures agree with each other, so the result stays an assertion (NL-2).
    #[test]
    fn test_reproduction_result_class_is_assertion() {
        let cell = synthetic_claim(5, 2).expect("B5");
        let footing = fmine_claim::compose_binary(
            "component_sum",
            &cell,
            &cell,
            Some(1000.0),
            "1000",
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            fmine_evidence::color::IntervalOp::Add,
        );
        assert_eq!(footing.evidence, EvidenceClass::ISSUER);
        assert!(footing.color().is_err());
    }

    /// A formula-error cell is a source cell unusable (208, exit 3), never 401 (unknown reactive ground, exit 7).
    #[test]
    fn test_formula_error_cell_refuses_208_exit_3() {
        let refusal = synthetic_claim(20, 2).expect_err("formula error");
        assert_eq!((refusal.code, refusal.exit_code), ("FMINE-REF-208", 3));
        assert!(
            refusal.what.starts_with("Source cell unusable")
                && refusal.what.contains("quarantined formula error")
                && refusal.what.ends_with("at Q1 2030!B20"),
            "{}",
            refusal.what
        );
    }

    /// bd-jg43.1 parity: reading styles changes no census count and no numeric digest. The values were printed
    /// by this test at commit 345e2ba, before the styles reader existed, and pasted here. bd-jg43.3 styled the
    /// AISC workbook and added column N (11 numbers and the header "Site G"): its census must equal that of the
    /// same cells written as plain numbers with no styles part.
    #[test]
    fn test_census_unchanged_by_style_reading() {
        let line = |name: &str, bytes: &[u8]| {
            let c = Workbook::from_bytes(name, bytes)
                .expect("synthetic workbook")
                .cell_census();
            format!(
                "{name} {} {} {} {} {} {} {}",
                c.visible_sheets,
                c.hidden_sheets,
                c.numeric_cells,
                c.placeholder_cells,
                c.formula_error_cells,
                c.text_cells,
                hex(&c.numeric_digest)
            )
        };
        let got: Vec<String> = synthetic::synthetic_workbooks(&[])
            .into_iter()
            .map(|(name, _, _, bytes)| line(name, &bytes))
            .collect();
        assert_eq!(
            got[0],
            "Synthetic_Mine_Stats.xlsx 2 0 2 0 0 9 ed492958121280b991212c61d01c9c2a257a551dfb07af145f1f6c117e646c21"
        );
        let plain: Vec<(String, MinimalCell)> = synthetic::synthetic_aisc_cells()
            .into_iter()
            .map(|(r, c)| match c {
                MinimalCell::Styled { raw, .. } => {
                    (r, MinimalCell::Number(raw.parse().expect("decimal")))
                }
                other => (r, other),
            })
            .collect();
        let unstyled = fmine_io::xlsx::build_minimal_xlsx(&[
            fmine_io::xlsx::MinimalSheet {
                name: synthetic::SYNTHETIC_TABLE_SHEET.to_string(),
                hidden: false,
                cells: plain,
            },
            fmine_io::xlsx::MinimalSheet {
                name: "Scratch".to_string(),
                hidden: true,
                cells: vec![("A1".to_string(), MinimalCell::Number(1.0))],
            },
            fmine_io::xlsx::MinimalSheet {
                name: synthetic::SYNTHETIC_HIDDEN_SHEET.to_string(),
                hidden: true,
                cells: synthetic::synthetic_hidden_cells(),
            },
        ]);
        assert_eq!(got[1], line(synthetic::SYNTHETIC_AISC_FILE, &unstyled));
        assert!(
            got[1].starts_with("Synthetic_AISC.xlsx 1 2 118 4 1 29 "),
            "{}",
            got[1]
        );
    }

    /// Site F (column G) stores 100.4 and 80.4 under `#,##0` (bd-jg43.3): the ledger takes the displayed 100 and
    /// 80, so both footings foot and the three rates fall in their intervals (worked in `synthetic.rs`'s header).
    #[test]
    fn test_synthetic_g_column_scored_on_displayed_values() {
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        let footing = |printed, component_sum, k_alt| Calc::Footing {
            printed,
            component_sum,
            k_alt,
        };
        let rate = |n, k, q, r| Calc::Rate(RateMiss { n, k, q, r });
        for (cell, family, calc) in [
            ("G8", IdentityFamily::TccFooting, footing(80, 80, 3)),
            ("G11", IdentityFamily::AiscFooting, footing(100, 100, 5)),
            ("G14", IdentityFamily::CosPerOunce, rate(100, 1, 50, 2008)),
            ("G15", IdentityFamily::TccPerOunce, rate(80, 3, 50, 1608)),
            ("G16", IdentityFamily::AiscPerOunce, rate(100, 5, 50, 2008)),
        ] {
            assert_eq!(state_of(&r, cell, family).code(), "i", "{cell}");
            assert_eq!(calc_of(&r, cell, family), calc, "{cell}");
        }
        // The operands carry the displayed whole numbers, never the stored 100.4.
        let g14 = r
            .instances
            .iter()
            .find(|i| i.cell == "G14" && i.family == IdentityFamily::CosPerOunce)
            .expect("G14");
        assert!(
            g14.operands
                .iter()
                .any(|o| o.cell == "G5" && o.value == aisc::OperandValue::Int(100))
        );
    }

    /// Site G (column N) holds a General non-integer at N6: the TCC footing is refused naming the cell and the
    /// format code, while the AISC footing, which does not read N6, foots.
    #[test]
    fn test_synthetic_site_g_general_non_integer_refused() {
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        let n8 = state_of(&r, "N8", IdentityFamily::TccFooting);
        assert_eq!(
            n8,
            &IdentityState::RefusedAt {
                reason: RefusedReason::GeneralNonInteger,
                at: "N6 (General)".to_string(),
            }
        );
        assert_eq!(n8.reason_code(), Some("general_format_non_integer"));
        assert_eq!(
            n8.detail(),
            "general format with non-integer stored value: N6 (General)"
        );
        assert_eq!(state_of(&r, "N11", IdentityFamily::AiscFooting).code(), "i");
        assert_eq!(
            calc_of(&r, "N11", IdentityFamily::AiscFooting),
            Calc::Footing {
                printed: 300,
                component_sum: 300,
                k_alt: 5
            }
        );
    }

    /// Site G's ounces (N13) are under a date format: all three rates are refused naming that cell and its code.
    #[test]
    fn test_synthetic_site_g_date_format_refused() {
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        for (cell, family) in [
            ("N14", IdentityFamily::CosPerOunce),
            ("N15", IdentityFamily::TccPerOunce),
            ("N16", IdentityFamily::AiscPerOunce),
        ] {
            let s = state_of(&r, cell, family);
            assert_eq!(
                s,
                &IdentityState::RefusedAt {
                    reason: RefusedReason::UnsupportedFormat,
                    at: "N13 (mm-dd-yy)".to_string(),
                },
                "{cell}"
            );
            assert_eq!(s.reason_code(), Some("unsupported_number_format"));
            assert_eq!(calc_of(&r, cell, family), Calc::None);
        }
    }

    #[test]
    fn synthetic_lane_reproduces_every_seeded_answer() {
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        assert_eq!(r.lane, Lane::Synthetic);
        assert!(!r.gate_eligible());
        assert_eq!(r.ledger_anchor.cell_id, "CELL-SYNTHETIC");
        assert_eq!((r.aisc_tables, r.instances.len()), (1, 55));
        let totals: Vec<usize> = STATE_CODES.iter().map(|c| r.count_state(c)).collect();
        // Columns K and L (BD-331.S3) add nine (i) and one (iv) to the 35 instances of columns B–H; column M
        // (BD-331.S2) adds four (i) and one stated (ii). On displayed values (bd-jg43.3) G's five (v) are (i), and
        // column N "Site G" adds four (v) and one (i).
        assert_eq!(totals, vec![40, 2, 1, 2, 5, 5]);

        match state_of(&r, "C8", IdentityFamily::TccFooting) {
            IdentityState::IssuerDiscrepancy(c) => {
                assert_eq!((c.printed, c.component_sum, c.residual), (350, 250, 100));
                assert_eq!(c.sign_flip_witness.as_deref(), Some("C6"));
                assert_eq!(c.line_cells, vec!["C5", "C6", "C7"]);
                assert!(!c.alternate_consistent);
            }
            other => panic!("C8 TCC footing: {other:?}"),
        }
        match state_of(&r, "D16", IdentityFamily::AiscPerOunce) {
            s @ IdentityState::Undecidable {
                why,
                missing_fact,
                closing_document,
            } => {
                // Structured (bd-sjrh.1): r 2400, N = D11 200 over k 5 lines, Q = D13 100.
                assert_eq!(
                    why,
                    &Undecided::RateOutside(RateMiss {
                        n: 200,
                        k: 5,
                        q: 100,
                        r: 2400
                    })
                );
                // 405000/199 = 2035.1758…: the upper bound rounds up (it printed 2035.175 before 2026-10-29).
                assert!(
                    s.detail().contains("[1965.174, 2035.176]"),
                    "{}",
                    s.detail()
                );
                // Site C's registered missing fact replaces the family default (BD-331.S2).
                assert_eq!(
                    missing_fact,
                    "the sustaining capital the synthetic issuer used for this rate (synthetic)"
                );
                assert_eq!(closing_document, &None);
            }
            other => panic!("D16: {other:?}"),
        }
        assert_eq!(
            state_of(&r, "E14", IdentityFamily::CosPerOunce),
            &IdentityState::Refused {
                reason: RefusedReason::NoRegisteredShare
            }
        );
        assert_eq!(state_of(&r, "E15", IdentityFamily::TccPerOunce).code(), "i");
        // Stated share 3/4: K prints 1000·¾·400/150 = 2000 (i); L prints 2200 (iv).
        let k14 = state_of(&r, "K14", IdentityFamily::CosPerOunce);
        assert_eq!(
            k14,
            &IdentityState::Consistent {
                alternate_consistent: true,
                basis: Some(ConsistentBasis::Share(StatedShare {
                    num: 3,
                    den: 4,
                    evidence: "synthetic".to_string(),
                    footnote: "Q1 2030!A30".to_string(),
                    label_cell: "Synthetic_Mine_Stats.xlsx:QTD!A5".to_string(),
                })),
            }
        );
        assert_eq!(
            k14.detail(),
            "alternate_construction_consistent=true; stated share 3/4 (synthetic); footnote Q1 2030!A30; label \
             Synthetic_Mine_Stats.xlsx:QTD!A5"
        );
        match state_of(&r, "L14", IdentityFamily::CosPerOunce) {
            s @ IdentityState::Undecidable {
                why, missing_fact, ..
            } => {
                assert!(
                    matches!(why, Undecided::ShareOutside { miss, share, s_interval }
                        if miss.r == 2200 && share.num == 3 && share.den == 4
                            && s_interval == "[0.8210, 0.8290]"),
                    "{why:?}"
                );
                assert!(
                    s.detail().contains("allow s in [0.8210, 0.8290]"),
                    "{}",
                    s.detail()
                );
                assert_eq!(missing_fact, aisc::SHARE_MISSING_FACT);
                assert!(
                    s.detail().ends_with(
                        "; missing fact: the attributable cost of sales or ounces the issuer used"
                    ),
                    "{}",
                    s.detail()
                );
            }
            other => panic!("L14: {other:?}"),
        }
        // Columns G and N: test_synthetic_g_column_scored_on_displayed_values and the two Site G tests.
        assert!(
            r.instances
                .iter()
                .filter(|i| i.column == "F")
                .all(|i| i.state.code() == "np")
        );
        match state_of(&r, "H16", IdentityFamily::AiscPerOunce) {
            IdentityState::Explained { component, .. } => {
                assert_eq!(
                    component,
                    &CitedCell {
                        file: "Synthetic_Mine_Stats.xlsx".to_string(),
                        sheet: "Minestats3".to_string(),
                        cell: "B32".to_string(),
                        value: 13,
                    }
                );
                assert_eq!(
                    component.text(),
                    "Synthetic_Mine_Stats.xlsx:Minestats3!B32=13"
                );
            }
            other => panic!("H16: {other:?}"),
        }
        assert_eq!(
            state_of(&r, "M16", IdentityFamily::AiscPerOunce).label(),
            "explained_stated"
        );
        assert_eq!(r.unregistered_headers.len(), 1);
        assert_eq!(r.unregistered_headers[0].cell, "J2");
        assert!(r.instances.iter().all(|i| i.column != "I"));
        let aisc = r
            .sources
            .iter()
            .find(|s| s.file_name == synthetic::SYNTHETIC_AISC_FILE)
            .expect("aisc");
        assert!(
            aisc.workbook
                .quarantined
                .iter()
                .any(|q| matches!(q, QuarantineItem::HiddenSheet { .. }))
        );
        assert!(
            aisc.workbook
                .quarantined
                .iter()
                .any(|q| matches!(q, QuarantineItem::FormulaErrorCell { .. }))
        );

        let json = r.to_json();
        assert!(json.starts_with(
            "{\"issuer\":\"synthetic\",\"lane\":\"synthetic\",\"gate_eligible\":false"
        ));
        assert!(json.contains("\"sign_flip_witness\":\"C6\""));
        assert!(
            // Site J (BD-331.S2) adds four (i) and one stated (ii); the hidden table is not counted. Columns G and
            // N on displayed values (bd-jg43.3): five (v) become (i), and N adds one (i) and four (v).
            json.contains("\"totals\":{\"i\":40,\"ii\":2,\"iii\":1,\"iv\":2,\"v\":5,\"np\":5}")
        );
        assert!(json.contains(
            "\"hidden_scope\":{\"scope\":\"sheet not displayed to readers\",\"counted\":false,\
             \"aisc_tables\":1,\"instances\":5,\"by_state\":{\"i\":4,\"ii\":0,\"iii\":1,\"iv\":0,\"v\":0,\"np\":0}"
        ));
        // The flags, the no-claim line and the next actions belong to the CLI envelope, not the payload.
        for field in ["\"flags\"", "\"no_claim\"", "\"next_actions\""] {
            assert!(!json.contains(field), "{field} in the payload");
        }
        assert!(json.ends_with(&format!(
            "\"anchor_id\":\"{}\"}}",
            r.ledger_anchor.anchor_id()
        )));
    }

    /// A cell changed in an input moves the state root and the affected instance's state.
    #[test]
    fn perturbing_an_input_cell_moves_the_root_and_the_state() {
        let base = run_synthetic_assurance(&[]).expect("base");
        assert_eq!(
            state_of(&base, "B8", IdentityFamily::TccFooting).code(),
            "i"
        );
        let edited = run_synthetic_assurance(&[("B8".to_string(), MinimalCell::Number(401.0))])
            .expect("edited");
        assert_ne!(
            base.ledger_anchor.state_root,
            edited.ledger_anchor.state_root
        );
        match state_of(&edited, "B8", IdentityFamily::TccFooting) {
            IdentityState::IssuerDiscrepancy(c) => assert_eq!(c.residual, 1),
            other => panic!("B8 after edit: {other:?}"),
        }
        // A change outside every table still moves the root (it is in the numeric digest).
        let far =
            run_synthetic_assurance(&[("Z99".to_string(), MinimalCell::Number(1.0))]).expect("far");
        assert_ne!(base.ledger_anchor.state_root, far.ledger_anchor.state_root);
        // And the run is deterministic.
        let again = run_synthetic_assurance(&[]).expect("again");
        assert_eq!(
            base.ledger_anchor.state_root,
            again.ledger_anchor.state_root
        );
    }

    /// The registry is read from the workspace; the corpus is absent. Every registered file is named, and the
    /// count comes from the registry, not from a literal.
    #[test]
    fn missing_corpus_refuses_naming_every_registered_file() {
        let profile = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        let n = profile.sources.len();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("no-such-workspace");
        let refusal = run_public_assurance(&profile, &root).expect_err("must refuse");
        // Missing files are foundation I/O (506, exit 5), never a digest code (§30.1).
        assert_eq!((refusal.code, refusal.exit_code), ("FMINE-REF-506", 5));
        assert!(
            refusal
                .what
                .contains(&format!("{n} of {n} registered barrick workbooks unusable")),
            "{}",
            refusal.what
        );
        for src in &profile.sources {
            assert!(
                refusal.what.contains(&src.file_name),
                "{} not named",
                src.file_name
            );
        }
    }

    /// A row added to a copy of the registry is picked up with no code change: the refusal count follows the
    /// registry, and the added file is named.
    #[test]
    fn test_refusal_count_follows_registry_rows() {
        let ws = std::env::temp_dir().join(format!("fmine-public-rows-{}", std::process::id()));
        let reg_dir = ws.join("registries").join("sources");
        std::fs::create_dir_all(&reg_dir).expect("temp registry dir");
        let text = std::fs::read_to_string(sources::registry_path(&workspace_root(), "barrick"))
            .expect("read");
        let base = load_issuer_profile(&workspace_root(), "barrick")
            .expect("registry")
            .sources
            .len();
        let added = "\n[[source]]\nfile = \"Barrick_Q3_2026_Mine_Stats.xlsx\"\nkind = \"mine_stats\"\n\
                     quarter = \"2026Q3\"\npath = \"2026/q3/Barrick_Q3_2026_Mine_Stats.xlsx\"\n\
                     url_basis = \"W1 naming rule\"\nbytes = 1\nsha256 = \"0000000000000000000000000000000000000000000000000000000000000001\"\n";
        std::fs::write(reg_dir.join("barrick.toml"), format!("{text}{added}")).expect("write");
        let profile = load_issuer_profile(&ws, "barrick").expect("extended registry");
        assert_eq!(profile.sources.len(), base + 1);
        let refusal = run_public_assurance(&profile, &ws).expect_err("no corpus");
        let n = base + 1;
        assert!(
            refusal
                .what
                .contains(&format!("{n} of {n} registered barrick workbooks unusable"))
        );
        assert!(refusal.what.contains("Barrick_Q3_2026_Mine_Stats.xlsx"));
        // `--through 2026Q2` drops the added row again.
        assert_eq!(
            profile.through("2026Q2").expect("bound").sources.len(),
            base
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// A component whose labels do not match keeps the instance undecidable and says why.
    #[test]
    fn reconstruction_with_a_wrong_label_does_not_apply() {
        let sources = run_synthetic_assurance(&[]).expect("base").sources;
        let mut profile = synthetic::synthetic_profile();
        profile.reconstructions[0].label_checks = vec![
            ("A32".to_string(), "Minesite sustaining".to_string()),
            ("B3".to_string(), "2031".to_string()),
        ];
        let r = assess(&profile, Lane::Synthetic, sources);
        match state_of(&r, "H16", IdentityFamily::AiscPerOunce) {
            s @ IdentityState::Undecidable {
                why: Undecided::ReconstructionNotApplied { check, .. },
                ..
            } => {
                assert!(
                    matches!(check, ReconstructionCheck::LabelMismatch { at, expected_prefix, .. }
                        if at == "Minestats3!B3" && expected_prefix == "2031"),
                    "{check:?}"
                );
                assert!(s.detail().contains("B3"), "{}", s.detail());
            }
            other => panic!("H16: {other:?}"),
        }
    }

    /// bd-4wvl.4: the construction discovery reads exactly the decided footings, per family and scope, and the
    /// payload carries it after the hidden scope. A handful of synthetic footings identifies no construction.
    #[test]
    fn test_construction_discovery_on_synthetic_ledger() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let found = r.construction_discovery().expect("discovery");
        let order: Vec<(&str, &str)> = found.iter().map(|(s, d)| (*s, d.family.as_str())).collect();
        assert_eq!(
            order,
            [
                ("visible", "tcc_footing"),
                ("visible", "aisc_footing"),
                ("hidden", "tcc_footing"),
                ("hidden", "aisc_footing")
            ]
        );
        let json = r.to_json();
        let (_, section) = json
            .split_once("\"construction_discovery\":")
            .expect("section");
        assert!(section.starts_with("{\"report_only\":"), "{section}");
        assert!(json.contains("\"table_refusals\":0},\"construction_discovery\":{"));
        // The unit discovery follows; the synthetic Mine Stats print no row labelled in pounds, so nothing is
        // paired and nothing is found. The in-memory synthetic run has no transcription registry (bd-plx1).
        let u = r.copper_unit_discovery().expect("unit discovery");
        assert_eq!((u.pairs, &u.finding), (0, &UnitFinding::NoFit));
        assert!(json.contains(
            "]},\"unit_discovery\":{\"report_only\":\"which unit the copper rows labelled (millions of pounds) are \
             in, from their site-sheet pairs; never a state, not in the state root\",\"pairs\":0,\
             \"fits\":{\"kt\":0,\"Mlb\":0},\"finding\":\"no_fit\",\"reason_code\":null},\"transcriptions\":null,\
             \"quarantine\":["
        ));
        for (scope, d) in &found {
            let instances = if *scope == "visible" {
                &r.instances
            } else {
                &r.hidden_scope.instances
            };
            let decided = |code: &str| {
                instances
                    .iter()
                    .filter(|i| i.family == d.family && i.state.code() == code)
                    .count()
            };
            assert!(d.totals > 0, "{scope} {:?}", d.family);
            assert_eq!(
                d.totals,
                decided("i") + decided("iii"),
                "{scope} {:?}",
                d.family
            );
            assert_eq!(d.foot_exact, decided("i"), "{scope} {:?}", d.family);
            assert_eq!(
                d.report.discovered,
                fmine_numerics::exact_feas::discover::Discovered::Ambiguous,
                "{d:?}"
            );
            assert!(section.contains(&discovery_json(scope, d)));
        }
    }

    /// bd-4wvl.4: a footing printed twice with the same subject and numbers (a comparative column) is one draw;
    /// the same numbers under another subject are another.
    #[test]
    fn test_construction_discovery_counts_a_reprinted_footing_once() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let base = footing_discovery(&r.instances).expect("discovery");
        let doubled: Vec<IdentityInstance> =
            r.instances.iter().chain(&r.instances).cloned().collect();
        let twice = footing_discovery(&doubled).expect("discovery");
        let renamed: Vec<IdentityInstance> = r
            .instances
            .iter()
            .cloned()
            .chain(r.instances.iter().cloned().map(|mut i| {
                i.subject.push_str(" (other)");
                i
            }))
            .collect();
        let other = footing_discovery(&renamed).expect("discovery");
        for ((b, t), o) in base.iter().zip(&twice).zip(&other) {
            assert_eq!((t.totals, t.foot_exact), (2 * b.totals, 2 * b.foot_exact));
            assert_eq!(t.report, b.report, "the reprint adds no draw");
            assert_eq!(
                o.report.n,
                2 * b.report.n,
                "another subject is another draw"
            );
        }
    }

    /// bd-sjrh.1 part 1: every refusal and not-published reason has a unique snake_case code and a reason text
    /// with no digits, and so does each next action's text.
    #[test]
    fn test_reason_codes_and_why_have_no_digits() {
        let pairs: Vec<(&str, &str)> = RefusedReason::ALL
            .iter()
            .map(|r| (r.code(), r.why()))
            .chain(NotPublishedReason::ALL.iter().map(|r| (r.code(), r.why())))
            .collect();
        // 16 since BD-331.S6 added `exact_kernel_indeterminate`; 18 since bd-jg43.3 split the unrounded refusal
        // into three by the operand's format; 21 since BD-331.S4 added the three regional refusals.
        assert_eq!(pairs.len(), 21);
        for (code, why) in &pairs {
            assert!(
                !code.is_empty() && code.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
                "{code}"
            );
            assert!(!why.bytes().any(|b| b.is_ascii_digit()), "{why}");
        }
        let codes: std::collections::BTreeSet<&str> = pairs.iter().map(|p| p.0).collect();
        assert_eq!(codes.len(), pairs.len());
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        for (_, why) in r.next_actions() {
            assert!(!why.bytes().any(|b| b.is_ascii_digit()), "{why}");
        }
        // Each state's code reaches the payload: E14's refusal, N8's, N14's and D16's.
        let json = r.to_json();
        for code in [
            "\"reason_code\":\"no_registered_share\"",
            "\"reason_code\":\"general_format_non_integer\"",
            "\"reason_code\":\"unsupported_number_format\"",
            "\"reason_code\":\"rate_outside_interval\"",
            "\"reason_code\":\"all_zero_column\"",
        ] {
            assert!(json.contains(code), "{code}");
        }
    }

    /// A broken cost-of-sales label on the release's own quarter sheet refuses the table by name instead of
    /// dropping it (BD-338.S3 finding).
    #[test]
    fn test_own_quarter_table_label_broken_refuses() {
        let r = run_synthetic_assurance(&[(
            "A5".to_string(),
            MinimalCell::Text("Cost of sales applicable to gold productionx".to_string()),
        )])
        .expect("synthetic run");
        let refusals: Vec<(&str, &str)> = r
            .table_refusals
            .iter()
            .map(|t| (t.sheet.as_str(), t.reason.as_str()))
            .collect();
        assert_eq!(
            refusals,
            [(
                synthetic::SYNTHETIC_TABLE_SHEET,
                "no cost-of-sales row on the release's own quarter sheet"
            )]
        );
        assert!(r.instances.is_empty());
    }

    /// BD-331.S2: a (ii) says whether the issuer states its rule. Region North's is reconstructed; Site J's is
    /// stated at A31. Barrick's E816 rows are registered as reconstructed.
    #[test]
    fn test_explained_basis_labels() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let h16 = state_of(&r, "H16", IdentityFamily::AiscPerOunce);
        assert_eq!(h16.label(), "explained_reconstructed");
        assert_eq!(
            h16.detail(),
            "SYNTHETIC: Synthetic_Mine_Stats.xlsx:Minestats3!B32=13"
        );
        let m16 = state_of(&r, "M16", IdentityFamily::AiscPerOunce);
        assert_eq!(m16.label(), "explained_stated");
        assert_eq!(
            m16.detail(),
            "SYNTHETIC-STATED: Synthetic_Mine_Stats.xlsx:Minestats3!B32=13; stated at \
             Synthetic_AISC.xlsx:Q1 2030!A31"
        );
        let json = r.to_json();
        assert!(json.contains("\"label\":\"explained_stated\""), "{json}");
        let barrick = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        let e816: Vec<&ExplanationBasis> = barrick
            .reconstructions
            .iter()
            .filter(|r| r.evidence_id == "E816")
            .map(|r| &r.basis)
            .collect();
        assert_eq!(e816, [&ExplanationBasis::Reconstructed; 3]);
    }

    /// bd-prmn.8: K.4's one-cell check covers every registered-subject column once, at the column's basis, and
    /// agrees with the ledger's cost-of-sales-per-ounce states; it is reported, not counted.
    #[test]
    fn test_one_cell_check_on_synthetic_columns() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let states: Vec<(&str, &str)> = r
            .one_cell
            .iter()
            .map(|c| (c.column.as_str(), c.state()))
            .collect();
        // B–H and K–N are registered subjects; I ("Footnote") and J ("Mystery Mine") are not. G passes on its
        // displayed values (bd-jg43.3); N's ounces are under a date format.
        assert_eq!(
            states,
            [
                ("B", "pass"),
                ("C", "pass"),
                ("D", "pass"),
                ("E", "refused"),
                ("F", "refused"),
                ("G", "pass"),
                ("H", "pass"),
                ("K", "pass"),
                ("L", "fail"),
                ("M", "pass"),
                ("N", "refused"),
            ]
        );
        // Site D's operands are valid, so its 100% check is still reported: 1000·1000/400 = 2500, printed 2500.
        for (column, reason, at_full) in [
            ("E", "no registered share", Some(true)),
            ("F", "all-zero column", None),
            ("N", "unsupported number format", None),
        ] {
            assert_eq!(
                one_cell(&r, column).outcome,
                OneCellOutcome::Refused {
                    reason,
                    at_full,
                    at_share: None
                },
                "{column}"
            );
        }
        assert!(one_cell(&r, "H").region);
        // Site H: 1000·¾·400/150 = 2000 at its stated share; at 100% the rate would be 2666.67.
        assert_eq!(
            one_cell(&r, "K").outcome,
            OneCellOutcome::Checked {
                basis: (3, 4),
                pass: true,
                at_full: false,
                at_share: Some(true),
                // [1000·3·799/(4·4001), 1000·3·801/(4·3999)] = [149.7750…, 150.2250…].
                implied_oz: ("149.775".to_string(), "150.226".to_string()),
            }
        );
        // Site I prints 2200: it fails at both bases; ounces in [1000·3·799/(4·4401), 1000·3·801/(4·4399)]
        // = [136.1622…, 136.5651…] would close it, against the printed 150.
        assert_eq!(
            one_cell(&r, "L").outcome,
            OneCellOutcome::Checked {
                basis: (3, 4),
                pass: false,
                at_full: false,
                at_share: Some(false),
                implied_oz: ("136.162".to_string(), "136.566".to_string()),
            }
        );
        // Every failing column is a (iv) cost-of-sales-per-ounce instance of the ledger, and the reverse.
        let fails: Vec<String> = r
            .one_cell
            .iter()
            .filter(|c| c.state() == "fail")
            .map(|c| format!("{}!{}", c.sheet, c.column))
            .collect();
        let undecidable: Vec<String> = r
            .instances
            .iter()
            .filter(|i| i.family == IdentityFamily::CosPerOunce && i.state.code() == "iv")
            .map(|i| format!("{}!{}", i.sheet, i.column))
            .collect();
        assert_eq!(fails, undecidable);
        let json = r.to_json();
        assert!(
            json.contains(
                "\"columns\":11,\"pass\":7,\"fail\":1,\"refused\":3,\"failing\":[{\"file\":\"Synthetic_AISC.xlsx\",\
                 \"sheet\":\"Q1 2030\",\"column\":\"L\",\"subject\":\"Site I\",\"basis\":\"3/4\",\"at_full\":false,\
                 \"at_share\":false,\"implied_oz\":[\"136.162\",\"136.566\"]}]},\"hidden_scope\":"
            ),
            "{json}"
        );
    }

    /// bd-sjrh.1 part 4, frame v4: the root is deterministic, and moves when one cell does (B8 400 → 401).
    #[test]
    fn test_frame_v4_root_deterministic_and_moves_on_cell_edit() {
        let root = |edits: &[(String, MinimalCell)]| {
            run_synthetic_assurance(edits)
                .expect("synthetic run")
                .ledger_anchor
                .state_root
                .to_hex()
        };
        let base = root(&[]);
        assert_eq!(base, root(&[]));
        assert_ne!(
            base,
            root(&[("B8".to_string(), MinimalCell::Number(401.0))])
        );
    }

    /// bd-sjrh.1 part 3: C8 (hand-read from `synthetic.rs`: C5 300, C6 −50, C7 0, C8 350) renders as three cited
    /// numbers: 350 as read, the sum 250 of C5:C7 and the residual 100, in its certificate and its `cited` record.
    #[test]
    fn test_c8_footing_renders_three_cited_numbers() {
        let json = run_synthetic_assurance(&[])
            .expect("synthetic run")
            .to_json();
        let sha = synthetic_aisc_sha();
        let at = |c: &str| format!("\"Synthetic_AISC.xlsx:Q1 2030!{c}\"");
        let x = |v: i64, c: &str| format!("\"v\":{v},\"at\":{},\"sha256\":\"{sha}\"}}", at(c));
        let rule = |r: &str| format!("\"rule\":\"{r}\",\"rules_version\":\"m0-ledger/1\"");
        let lines = format!("{},{},{}", at("C5"), at("C6"), at("C7"));
        let sum = format!(
            "\"v\":250,{},\"in\":[{lines}]}}",
            rule("aisc.footing.component_sum")
        );
        let residual = format!(
            "\"v\":100,{},\"in\":[{},{lines}]}}",
            rule("aisc.footing.residual"),
            at("C8")
        );
        let certificate = format!(
            "\"certificate\":{{\"printed\":{{{},\"component_sum\":{{{sum},\"residual\":{{{residual},\
             \"total_cell\":\"C8\",\"line_cells\":[\"C5\",\"C6\",\"C7\"],\"sign_flip_witness\":\"C6\"",
            x(350, "C8")
        );
        assert!(json.contains(&certificate), "{certificate}");
        let cited = format!(
            "\"cited\":{{\"operands\":[{{\"role\":\"total\",{},{{\"role\":\"line\",{},{{\"role\":\"line\",{},\
             {{\"role\":\"line\",{}],\"derived\":[{{\"name\":\"component_sum\",{sum},{{\"name\":\"residual\",\
             {residual}]}}",
            x(350, "C8"),
            x(300, "C5"),
            x(-50, "C6"),
            x(0, "C7")
        );
        assert!(json.contains(&cited), "{cited}");
    }

    /// bd-sjrh.1 part 3: D16 prints 2400 against N = D11 200 over k = 5 numeric lines (D5, D6, D7, D9, D10) and
    /// Q = D13 100. Hand-derived: 1000·197.5/100.5 = 1965.1741… floors to 1965.174, and 1000·202.5/99.5 =
    /// 2035.1758… ceils to 2035.176.
    #[test]
    fn test_d16_rate_range_cited() {
        let json = run_synthetic_assurance(&[])
            .expect("synthetic run")
            .to_json();
        let at = |c: &str| format!("\"Synthetic_AISC.xlsx:Q1 2030!{c}\"");
        let rule = |r: &str| format!("\"rule\":\"{r}\",\"rules_version\":\"m0-ledger/1\"");
        let counted: Vec<String> = ["D5", "D6", "D7", "D9", "D10"].map(at).to_vec();
        let range_in = format!("[{},{},{}]", at("D13"), at("D11"), counted.join(","));
        let want = format!(
            "\"derived\":[{{\"name\":\"k\",\"v\":5,{},\"in\":[{}]}},\
             {{\"name\":\"range_lo\",\"v\":1965.174,{},\"in\":{range_in}}},\
             {{\"name\":\"range_hi\",\"v\":2035.176,{},\"in\":{range_in}}}]",
            rule("ledger.count"),
            counted.join(","),
            rule("aisc.rate_interval"),
            rule("aisc.rate_interval"),
        );
        assert!(json.contains(&want), "{want}");
        // One record per counted instance, each with operands.
        let (_, records) = json.split_once("\"instances\":[").expect("instances array");
        let records = records.split_once("],\"one_cell_cos\"").expect("end").0;
        assert_eq!(records.matches("\"cited\":{\"operands\":[{").count(), 55);
        assert_eq!(records.matches("{\"at\":").count(), 55);
    }

    /// BD-331.S2: hidden sheets are parsed into `hidden_sheets` and leave the census and its digest unchanged.
    #[test]
    fn test_hidden_sheet_parsed_not_censused() {
        let files = synthetic::synthetic_workbooks(&[]);
        let aisc_bytes = files
            .iter()
            .find(|f| f.0 == synthetic::SYNTHETIC_AISC_FILE)
            .map(|f| f.3.clone())
            .expect("aisc");
        let with_hidden = Workbook::from_bytes("a.xlsx", &aisc_bytes).expect("parse");
        let visible_only = Workbook::from_bytes(
            "a.xlsx",
            &fmine_io::xlsx::build_minimal_xlsx(&[fmine_io::xlsx::MinimalSheet {
                name: synthetic::SYNTHETIC_TABLE_SHEET.to_string(),
                hidden: false,
                cells: synthetic::synthetic_aisc_cells(),
            }]),
        )
        .expect("parse");
        let names: Vec<&str> = with_hidden
            .hidden_sheets
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(names, ["Scratch", synthetic::SYNTHETIC_HIDDEN_SHEET]);
        assert_eq!(
            with_hidden.hidden_sheets[0]
                .cell(1, 1)
                .and_then(|c| c.value.as_number()),
            Some(1.0)
        );
        assert!(with_hidden.sheet("Scratch").is_none());
        let (a, b) = (with_hidden.cell_census(), visible_only.cell_census());
        assert_eq!((a.hidden_sheets, b.hidden_sheets), (2, 0));
        assert_eq!(
            CellCensus {
                hidden_sheets: 0,
                ..a
            },
            b
        );
    }

    /// BD-331.S2: the hidden table's instances are reported in their own section, labelled, and never counted.
    #[test]
    fn test_hidden_scope_never_counted() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let hidden = &r.hidden_scope;
        assert_eq!(hidden.aisc_tables, 1);
        assert_eq!(r.aisc_tables, 1);
        let states: Vec<(&str, &str)> = hidden
            .instances
            .iter()
            .map(|i| (i.cell.as_str(), i.state.code()))
            .collect();
        assert_eq!(
            states,
            [
                ("B8", "iii"),
                ("B11", "i"),
                ("B14", "i"),
                ("B15", "i"),
                ("B16", "i")
            ]
        );
        assert!(
            hidden
                .instances
                .iter()
                .all(|i| i.sheet == synthetic::SYNTHETIC_HIDDEN_SHEET)
        );
        assert!(
            r.instances
                .iter()
                .all(|i| i.sheet == synthetic::SYNTHETIC_TABLE_SHEET)
        );
        assert_eq!(r.count_state("iii"), 1, "only C8 on the visible sheet");
        let json = r.to_json();
        let (ledger, rest) = json.split_once("\"hidden_scope\":").expect("section");
        assert!(!ledger.contains(synthetic::SYNTHETIC_HIDDEN_SHEET));
        assert!(rest.contains(
            "\"sheet\":\"Q4 2029\",\"cell\":\"B8\",\"subject\":\"Site A\",\"family\":\"tcc_footing\",\
             \"detail\":\"printed 401 vs component sum 400 (residual 1)\",\
             \"scope\":\"sheet not displayed to readers\""
        ));
    }

    /// A complete run through a TOML registry in default CI (the real-corpus runs are ignored tests): the two
    /// synthetic workbooks plus a third, later Mine Stats file are written to a scratch corpus, registered by
    /// digest in a generated TOML, and loaded with no code-side profile. The ledger equals the in-code synthetic
    /// profile's, and `--through 2030Q1` drops the third file and leaves the ledger unchanged.
    #[test]
    fn test_synthetic_three_file_toml_profile_runs_end_to_end() {
        let ws = std::env::temp_dir().join(format!("fmine-public-toml-e2e-{}", std::process::id()));
        let reg = synthetic::write_issuer_workspace(&ws).expect("write workspace");
        assert_eq!(reg, sources::registry_path(&ws, "synthetic"));
        let code_profile = synthetic::synthetic_profile();

        let profile = load_issuer_profile(&ws, "synthetic").expect("generated registry loads");
        assert_eq!(profile.sources.len(), 3);
        assert_eq!(profile.reconstructions, code_profile.reconstructions);
        assert_eq!(profile.missing_facts, code_profile.missing_facts);
        let r = run_public_assurance(&profile, &ws).expect("all three files verify by digest");
        let totals: Vec<usize> = STATE_CODES.iter().map(|c| r.count_state(c)).collect();
        assert_eq!(
            totals,
            vec![40, 2, 1, 2, 5, 5],
            "same ledger as the in-code synthetic profile"
        );
        assert_eq!(r.hidden_scope.instances.len(), 5);
        assert_eq!(
            (&profile.regions, &profile.shares),
            (&code_profile.regions, &code_profile.shares)
        );
        assert_eq!(r.sources.len(), 3);
        assert_eq!(r.ledger_anchor.commit_sequence, 3);
        // The reserved issuer name keeps invented numbers out of the public lane; the same registry under
        // another name would be gate-eligible, so the name is what decides.
        assert_eq!((r.lane, r.gate_eligible()), (Lane::Synthetic, false));
        let mut renamed = profile.clone();
        renamed.issuer = "renamed".to_string();
        let public = run_public_assurance(&renamed, &ws).expect("same files");
        assert_eq!((public.lane, public.gate_eligible()), (Lane::Public, true));

        let bounded =
            run_public_assurance(&profile.through("2030Q1").expect("bound"), &ws).expect("run");
        assert_eq!(bounded.sources.len(), 2);
        assert_eq!(bounded.through.as_deref(), Some("2030Q1"));
        let totals: Vec<usize> = STATE_CODES.iter().map(|c| bounded.count_state(c)).collect();
        assert_eq!(totals, vec![40, 2, 1, 2, 5, 5]);
        assert_ne!(
            bounded.ledger_anchor.state_root, r.ledger_anchor.state_root,
            "the source set is in the root"
        );
        let _ = std::fs::remove_dir_all(&ws);
    }
}
