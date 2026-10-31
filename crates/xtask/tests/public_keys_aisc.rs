//! Cross-crate public Claim binding and series identity (BD-332.S2).

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use fmine_public::LoadedSource;
    use fmine_public::aisc::{
        AiscClaimIndex, IdentityInstance, OperandRole, OperandValue, key_aisc_operands,
        ledger_for_table, locate_table,
    };
    use fmine_public::key::{
        KeyedCell, NilReason, OwnershipBasis, PublicValue, ShareSource, StatusKind, SubjectStatus,
    };
    use fmine_public::sources::{IssuerProfile, SourceKind};
    use fmine_public::synthetic;

    fn keyed_synthetic(profile: &IssuerProfile) -> AiscClaimIndex {
        let report = fmine_public::run_synthetic_assurance(&[]).expect("synthetic assurance");
        key_aisc_operands(profile, &report.sources, &report.instances)
    }

    fn synthetic_at(cell: &str) -> String {
        format!(
            "{}:{}!{cell}",
            synthetic::SYNTHETIC_AISC_FILE,
            synthetic::SYNTHETIC_TABLE_SHEET
        )
    }

    #[test]
    fn test_operand_keys_synthetic_complete() {
        let report = fmine_public::run_synthetic_assurance(&[]).expect("synthetic assurance");
        let index = key_aisc_operands(
            &synthetic::synthetic_profile(),
            &report.sources,
            &report.instances,
        );
        assert!(index.refusals.is_empty(), "{:?}", index.refusals);
        for operand in report.instances.iter().flat_map(|i| &i.operands) {
            if operand.file == synthetic::SYNTHETIC_AISC_FILE
                && operand.role != OperandRole::LabelCheck
            {
                let at = format!("{}:{}!{}", operand.file, operand.sheet, operand.cell);
                let cell = index.cells.get(&at).expect("every numeric operand keyed");
                assert_eq!(cell.key.reporting_vintage.release, "2030Q1");
                assert_eq!(cell.claim.anchors.len(), 1);
            }
        }
        assert_eq!(index.cells[&synthetic_at("C8")].key.subject, "Site B");
        assert_eq!(index.cells[&synthetic_at("B8")].key.unit, "usd_m");
        assert_eq!(index.cells[&synthetic_at("B13")].key.unit, "koz");
        assert_eq!(index.cells[&synthetic_at("B14")].key.unit, "usd_per_oz");
        assert!(matches!(index.cells[&synthetic_at("G5")].claim.value,
        PublicValue::Number { ref stored, displayed: Ok(ref displayed), .. }
            if stored == "100.4" && displayed == "100"));
        assert!(matches!(
            index.cells[&synthetic_at("N6")].claim.value,
            PublicValue::Number {
                displayed: Err(_),
                ..
            }
        ));
    }

    #[test]
    fn test_aisc_withheld_zero_uses_only_registered_status() {
        let mut profile = synthetic::synthetic_profile();
        profile.subject_statuses.push(SubjectStatus {
            subject: "Site E".to_string(),
            kind: StatusKind::Withheld,
            first: (2030, 1),
            last: Some((2030, 3)),
            evidence: "SYNTHETIC-WITHHELD".to_string(),
        });
        let registered = keyed_synthetic(&profile);
        assert_eq!(
            registered.cells[&synthetic_at("F5")]
                .status_evidence
                .as_deref(),
            Some("SYNTHETIC-WITHHELD")
        );
        assert_eq!(
            registered.cells[&synthetic_at("F5")].claim.value,
            PublicValue::Nil {
                token: "0".to_string(),
                reason: NilReason::Withheld
            }
        );
        assert!(registered.unregistered_all_zero.is_empty());
        profile.subject_statuses.clear();
        let unregistered = keyed_synthetic(&profile);
        assert!(matches!(
            unregistered.cells[&synthetic_at("F5")].claim.value,
            PublicValue::Number { .. }
        ));
        assert_eq!(unregistered.unregistered_all_zero, [synthetic_at("F")]);
        assert_ne!(
            registered.cells[&synthetic_at("F5")].claim.claim_id,
            unregistered.cells[&synthetic_at("F5")].claim.claim_id
        );
    }

    #[test]
    fn test_row_without_registered_basis_is_unprinted() {
        let mut profile = synthetic::synthetic_profile();
        let index = keyed_synthetic(&profile);
        assert_eq!(
            index.cells[&synthetic_at("K5")].key.ownership_basis,
            OwnershipBasis::Unprinted
        );
        let ounces = &index.cells[&synthetic_at("K13")];
        assert_eq!(
            ounces.key.ownership_basis,
            OwnershipBasis::Attributable {
                num: 3,
                den: 4,
                source: ShareSource::Registry
            }
        );
        assert_eq!(ounces.printed_basis, OwnershipBasis::Unprinted);
        assert!(
            ounces
                .basis_evidence
                .as_ref()
                .expect("source")
                .contains("A13")
        );
        profile.shares.clear();
        let missing = keyed_synthetic(&profile);
        assert_eq!(
            missing.cells[&synthetic_at("K13")].key.ownership_basis,
            OwnershipBasis::Unprinted
        );
        assert!(missing.cells[&synthetic_at("K13")].basis_evidence.is_none());
    }

    fn keying_fixture(file: &str, release: &str, col: &str, shift: u32) -> LoadedSource {
        use fmine_io::xlsx::{MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx};
        let rows = [
            (5, "Cost of sales applicable to gold production", 100.0),
            (6, "Depreciation", -20.0),
            (7, "Non-controlling interests", 0.0),
            (8, "Total cash costs", 80.0),
            (9, "General & administrative costs", 20.0),
            (10, "Non-controlling interests", 0.0),
            (11, "All-in sustaining costs", 100.0),
            (13, "Ounces sold - attributable basis (000s ounces)", 50.0),
            (14, "Cost of sales per ounce", 2000.0),
            (15, "Total cash costs per ounce", 1600.0),
            (16, "All-in sustaining costs per ounce", 2000.0),
        ];
        let mut cells = vec![(
            format!("{col}{}", 2 + shift),
            MinimalCell::Text("Site A".to_string()),
        )];
        for (r, label, value) in rows {
            cells.push((
                format!("A{}", r + shift),
                MinimalCell::Text(label.to_string()),
            ));
            cells.push((format!("{col}{}", r + shift), MinimalCell::Number(value)));
        }
        let bytes = build_minimal_xlsx(&[MinimalSheet {
            name: "Q1 2030".to_string(),
            hidden: false,
            cells,
        }]);
        let workbook = Workbook::from_bytes(file, &bytes).expect("fixture");
        LoadedSource {
            file_name: file.to_string(),
            kind: SourceKind::AiscReconciliation,
            quarter: release.to_string(),
            url: String::new(),
            url_basis: "synthetic".to_string(),
            bytes: bytes.len() as u64,
            census: workbook.cell_census(),
            workbook,
        }
    }

    fn fixture_instances(sources: &[LoadedSource]) -> Vec<IdentityInstance> {
        let mut instances = Vec::new();
        for source in sources {
            let sheet = &source.workbook.sheets[0];
            let table = locate_table(sheet, &["Site A"])
                .expect("table")
                .expect("table");
            ledger_for_table(
                &source.file_name,
                sheet,
                &table,
                &["Site A"],
                &BTreeMap::new(),
                &mut |_, _, _| None,
                &mut instances,
                &mut Vec::new(),
            );
        }
        instances
    }

    #[test]
    fn test_aisc_series_survives_row_and_column_shift() {
        let profile = synthetic::synthetic_profile();
        let mut sources = vec![
            keying_fixture("first.xlsx", "2030Q1", "B", 0),
            keying_fixture("comparative.xlsx", "2031Q1", "D", 3),
        ];
        let instances = fixture_instances(&sources);
        let index = key_aisc_operands(&profile, &sources, &instances);
        assert!(index.refusals.is_empty(), "{:?}", index.refusals);
        let first = &index.cells["first.xlsx:Q1 2030!B11"];
        let comparative = &index.cells["comparative.xlsx:Q1 2030!D14"];
        assert_eq!(first.key.series_key(), comparative.key.series_key());
        assert_ne!(first.claim.claim_id, comparative.claim.claim_id);
        assert_eq!(index.cells["first.xlsx:Q1 2030!B7"].key.line.occurrence, 0);
        assert_eq!(index.cells["first.xlsx:Q1 2030!B10"].key.line.occurrence, 1);
        let original = index.cells;
        sources.reverse();
        let mut reordered = instances;
        reordered.reverse();
        let index = key_aisc_operands(&profile, &sources, &reordered);
        let ids = |cells: &BTreeMap<String, KeyedCell>| {
            cells
                .iter()
                .map(|(at, c)| (at.clone(), c.claim.claim_id))
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&original), ids(&index.cells));
    }

    #[test]
    fn test_aisc_key_refusals_name_missing_source_and_bad_period() {
        let source = keying_fixture("fixture.xlsx", "2030Q1", "B", 0);
        let mut instances = fixture_instances(std::slice::from_ref(&source));
        let profile = synthetic::synthetic_profile();
        let missing = key_aisc_operands(&profile, &[], &instances);
        assert_eq!(missing.refusals.len(), 11);
        assert!(
            missing
                .refusals
                .iter()
                .all(|r| r.locator.starts_with("fixture.xlsx:Q1 2030!B"))
        );
        let mut source = source;
        source.workbook.sheets[0].name = "Unknown period".to_string();
        for instance in &mut instances {
            instance.sheet = "Unknown period".to_string();
            for operand in &mut instance.operands {
                operand.sheet = "Unknown period".to_string();
            }
        }
        let bad = key_aisc_operands(&profile, &[source], &instances);
        assert_eq!(bad.refusals.len(), 11);
        assert!(bad.cells.is_empty());
        assert!(
            bad.refusals
                .iter()
                .all(|r| r.reason.contains("period form"))
        );
    }

    #[test]
    fn test_aisc_key_refuses_rebound_operand() {
        let source = keying_fixture("fixture.xlsx", "2030Q1", "B", 0);
        let mut instances = fixture_instances(std::slice::from_ref(&source));
        for operand in instances.iter_mut().flat_map(|i| &mut i.operands) {
            if operand.cell == "B11" {
                operand.value = OperandValue::Int(101);
            }
        }
        let index = key_aisc_operands(&synthetic::synthetic_profile(), &[source], &instances);
        assert_eq!(index.refusals.len(), 1);
        assert_eq!(index.refusals[0].locator, "fixture.xlsx:Q1 2030!B11");
        assert!(index.refusals[0].reason.contains("no longer matches"));
        assert!(!index.cells.contains_key("fixture.xlsx:Q1 2030!B11"));
    }

    /// FACT: the paired cells and withheld zeros are stated in BD-332.S2; expectations are independent of this
    /// adapter's census and refer to the published locators.
    #[test]
    #[ignore = "needs the registered Barrick workbooks through 2026Q2"]
    fn barrick_aisc_claims_known_answers() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let profile = fmine_public::load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let report =
            fmine_public::run_public_assurance(&profile, &root).expect("registered corpus");
        let index = key_aisc_operands(&profile, &report.sources, &report.instances);
        assert!(index.refusals.is_empty(), "{:?}", index.refusals);
        for (a, b) in [
            (
                "Barrick_Q2_2025_AISC.xlsx:YTD Q2 2025!J18",
                "Barrick_Q2_2026_AISC.xlsx:YTD Q2 2025!I17",
            ),
            (
                "Barrick_Q3_2024_AISC.xlsx:Q2 2024!O11",
                "Barrick_Q2_2025_AISC.xlsx:Q2 2024!O11",
            ),
        ] {
            assert_eq!(
                index.cells[a].key.series_key(),
                index.cells[b].key.series_key()
            );
            assert_ne!(index.cells[a].claim.claim_id, index.cells[b].claim.claim_id);
        }
        for cell in ["N5", "N18", "N19", "N23"] {
            let at = format!("Barrick_Q1_2025_AISC.xlsx:Q1 2025!{cell}");
            assert!(
                matches!(
                    index.cells[&at].claim.value,
                    PublicValue::Nil {
                        reason: NilReason::Withheld,
                        ..
                    }
                ),
                "{at}"
            );
        }
    }
}
