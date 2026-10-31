//! Published co-product rates remain distinct Claims outside the by-product identity arithmetic.

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use fmine_io::xlsx::{
        MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx_styled,
    };
    use fmine_public::LoadedSource;
    use fmine_public::aisc::{key_aisc_operands, ledger_for_table, locate_table};
    use fmine_public::key::consistency::{
        VintageScope, VintageState, compare_vintages_with_disclosures,
    };
    use fmine_public::key::{NilReason, OwnershipBasis, PublicValue, StatusKind, SubjectStatus};
    use fmine_public::sources::SourceKind;
    use fmine_public::synthetic::synthetic_profile;

    const LINE: &str = "Total cash costs per ounce (on a co-product basis)";

    fn fixture(
        release: &str,
        col: &str,
        shift: u32,
        rates: &[(u32, &str, Option<MinimalCell>)],
    ) -> LoadedSource {
        let mut cells = vec![
            (
                format!("{col}{}", 2 + shift),
                MinimalCell::Text("Site A".into()),
            ),
            (
                format!("X{}", 2 + shift),
                MinimalCell::Text("Unregistered".into()),
            ),
        ];
        for (row, label, value) in [
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
            (18, "All-in sustaining costs per ounce", 2000.0),
        ] {
            cells.push((format!("A{}", row + shift), MinimalCell::Text(label.into())));
            cells.push((format!("{col}{}", row + shift), MinimalCell::Number(value)));
        }
        for (row, label, value) in rates {
            cells.push((
                format!("A{}", row + shift),
                MinimalCell::Text((*label).into()),
            ));
            cells.push((format!("X{}", row + shift), MinimalCell::Number(999.0)));
            if let Some(value) = value {
                cells.push((format!("{col}{}", row + shift), value.clone()));
            }
        }
        let bytes = build_minimal_xlsx_styled(
            &[
                MinimalSheet {
                    name: "Q1 2030".into(),
                    hidden: false,
                    cells: cells.clone(),
                },
                MinimalSheet {
                    name: "Q4 2029".into(),
                    hidden: true,
                    cells,
                },
            ],
            Some(&MinimalStyles {
                num_fmts: vec![],
                xfs: vec![0, 3],
            }),
        );
        let file_name = format!("synthetic-coproduct-{release}.xlsx");
        let workbook = Workbook::from_bytes(&file_name, &bytes).expect("synthetic workbook");
        LoadedSource {
            file_name,
            kind: SourceKind::AiscReconciliation,
            quarter: release.into(),
            url: String::new(),
            url_basis: "synthetic".into(),
            bytes: bytes.len() as u64,
            census: workbook.cell_census(),
            workbook,
        }
    }

    #[test]
    fn co_product_rates_keep_units_identity_and_display_precision_when_rows_move() {
        let sources = [
            fixture(
                "2030Q1",
                "B",
                0,
                &[(
                    16,
                    LINE,
                    Some(MinimalCell::Styled {
                        raw: "1466.1763085877856".into(),
                        xf: 1,
                    }),
                )],
            ),
            fixture(
                "2031Q1",
                "G",
                5,
                &[(16, LINE, Some(MinimalCell::Number(1466.0)))],
            ),
        ];
        let profile = synthetic_profile();
        let mut instances = Vec::new();
        for source in &sources {
            let sheet = &source.workbook.sheets[0];
            let table = locate_table(sheet, &profile.subjects)
                .expect("lookup")
                .expect("table");
            ledger_for_table(
                &source.file_name,
                sheet,
                &table,
                &profile.subjects,
                &BTreeMap::new(),
                &mut |_, _, _| None,
                &mut instances,
                &mut Vec::new(),
            );
        }
        let index = key_aisc_operands(&profile, &sources, &instances);
        assert!(index.refusals.is_empty(), "{:?}", index.refusals);
        let claims: Vec<_> = index
            .cells
            .values()
            .filter(|c| c.key.line.label == LINE.to_lowercase())
            .cloned()
            .collect();
        assert_eq!(
            claims.len(),
            2,
            "hidden tables and unregistered columns stay excluded"
        );
        assert_eq!(claims[0].key.series_key(), claims[1].key.series_key());
        assert_ne!(claims[0].claim.claim_id, claims[1].claim.claim_id);
        assert!(claims[0].locator.ends_with("Q1 2030!B16"));
        assert!(claims[1].locator.ends_with("Q1 2030!G21"));
        assert_eq!(claims[0].key.unit, "usd_per_oz");
        assert_eq!(claims[0].key.ownership_basis, OwnershipBasis::Unprinted);
        assert!(matches!(&claims[0].claim.value,
            PublicValue::Number { stored, displayed: Ok(displayed), .. }
                if stored == "1466.1763085877856" && displayed == "1466"));
        let by_product = &index.cells["synthetic-coproduct-2030Q1.xlsx:Q1 2030!B15"];
        assert_ne!(claims[0].key.series_key(), by_product.key.series_key());
        assert_eq!(by_product.key.unit, "usd_per_oz");
        assert!(
            instances
                .iter()
                .flat_map(|i| &i.operands)
                .all(|o| !matches!(o.cell.as_str(), "B16" | "G21")),
            "no invented ledger operand"
        );
        let report = compare_vintages_with_disclosures(&claims, VintageScope::Aisc, None);
        assert_eq!(report.totals.states[&VintageState::Unchanged], 1);
        assert!(report.items.is_empty());
    }

    #[test]
    fn co_product_rows_are_optional_exact_and_occurrences_are_not_collapsed() {
        let profile = synthetic_profile();
        for rows in [
            vec![],
            vec![(
                16,
                "Total cash costs per pound (on a co-product basis)",
                None,
            )],
        ] {
            let index = key_aisc_operands(&profile, &[fixture("2030Q1", "B", 0, &rows)], &[]);
            assert!(index.cells.is_empty());
            assert!(index.refusals.is_empty());
        }
        let source = fixture(
            "2030Q1",
            "B",
            0,
            &[(16, LINE, None), (17, LINE, Some(MinimalCell::Number(0.0)))],
        );
        let index = key_aisc_operands(&profile, &[source], &[]);
        assert!(index.refusals.is_empty());
        let claims: Vec<_> = index.cells.values().collect();
        assert_eq!(claims.len(), 2);
        assert_eq!(claims[0].key.line.occurrence, 0);
        assert_eq!(claims[1].key.line.occurrence, 1);
        assert_ne!(claims[0].claim.claim_id, claims[1].claim.claim_id);
        assert_eq!(claims[0].claim.value, PublicValue::Missing);
        assert!(matches!(claims[1].claim.value, PublicValue::Number { .. }));
    }

    #[test]
    fn co_product_tokens_status_and_display_refusals_use_the_canonical_classifier() {
        let cases = [
            (
                MinimalCell::Text("—".into()),
                PublicValue::Nil {
                    token: "—".into(),
                    reason: NilReason::Unknown,
                },
            ),
            (
                MinimalCell::Text("n/a".into()),
                PublicValue::NotApplicable {
                    token: "n/a".into(),
                },
            ),
            (
                MinimalCell::Styled {
                    raw: "1.25".into(),
                    xf: 0,
                },
                PublicValue::Number {
                    stored: "1.25".into(),
                    displayed: Err("general_non_integer"),
                    under_status: None,
                },
            ),
        ];
        let mut profile = synthetic_profile();
        for (input, expected) in cases {
            let source = fixture("2030Q1", "B", 0, &[(16, LINE, Some(input))]);
            let index = key_aisc_operands(&profile, &[source], &[]);
            assert!(index.refusals.is_empty());
            assert_eq!(
                index.cells.values().next().expect("Claim").claim.value,
                expected
            );
        }
        profile.subject_statuses.push(SubjectStatus {
            subject: "Site A".into(),
            kind: StatusKind::Disposed,
            first: (2030, 1),
            last: None,
            evidence: "SYNTHETIC-DISPOSAL".into(),
        });
        let source = fixture(
            "2030Q1",
            "B",
            0,
            &[(16, LINE, Some(MinimalCell::Number(0.0)))],
        );
        let index = key_aisc_operands(&profile, &[source], &[]);
        let cell = index.cells.values().next().expect("disposed Claim");
        assert_eq!(
            cell.claim.value,
            PublicValue::Nil {
                token: "0".into(),
                reason: NilReason::Disposed
            }
        );
        assert_eq!(cell.status_evidence.as_deref(), Some("SYNTHETIC-DISPOSAL"));
        let source = fixture(
            "2030Q1",
            "B",
            0,
            &[(16, LINE, Some(MinimalCell::Error("#REF!".into())))],
        );
        let index = key_aisc_operands(&profile, &[source], &[]);
        assert!(index.cells.is_empty());
        assert_eq!(index.refusals.len(), 1);
        assert_eq!(
            index.refusals[0].locator,
            "synthetic-coproduct-2030Q1.xlsx:Q1 2030!B16"
        );
    }
}
