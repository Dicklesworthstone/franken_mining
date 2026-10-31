//! Cross-crate public report and Claim contracts, retained unchanged in the T layer.
#[cfg(test)]
mod tests {
    use fmine_claim::{AdditivityClass, Claim, Representation, SpatialSupport};
    use fmine_core::hash::Digest32;
    use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal};
    use fmine_io::xlsx::{MinimalCell, Workbook};
    use fmine_public::sources::ExpectedRoot;
    use fmine_public::{
        cell_to_public_claim, load_issuer_profile, run_synthetic_assurance, sources, synthetic,
    };
    use std::path::PathBuf;
    const STATE_CODES: [&str; 6] = ["i", "ii", "iii", "iv", "v", "np"];
    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }
    fn hex(bytes: &[u8; 32]) -> String {
        Digest32(*bytes).to_hex()
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
}
