//! BD-332.S3: source-bound public figures and independent published-cell known answers.

#[cfg(test)]
mod tests {
    use fmine_evidence::{EvidenceClass, KnowledgeState};
    use fmine_io::xlsx::{
        MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx_styled,
    };
    use fmine_public::key::{
        self, NilReason, OwnershipBasis, PublicValue, ShareSource, StatusKind, SubjectStatus,
    };
    use fmine_public::mine_stats::{MineStatsCell, MineStatsValue, View};
    use fmine_public::sources::{MineStatsLabelAlias, PrintedShareCorrection};
    use fmine_public::synthetic::synthetic_profile;

    fn example(raw: &str, format: &str) -> (Workbook, MineStatsValue) {
        let bytes = build_minimal_xlsx_styled(
            &[MinimalSheet {
                name: "Minestats3".into(),
                hidden: false,
                cells: vec![(
                    "F19".into(),
                    MinimalCell::Styled {
                        raw: raw.into(),
                        xf: 1,
                    },
                )],
            }],
            Some(&MinimalStyles {
                num_fmts: vec![(164, format.into())],
                xfs: vec![0, 164],
            }),
        );
        let book = Workbook::from_bytes("synthetic_mine_stats.xlsx", &bytes).expect("workbook");
        let value = MineStatsValue {
            locator: "2026Q1:Minestats3!F19".into(),
            sheet: "Minestats3".into(),
            cell: "F19".into(),
            view: View::Minestats,
            months: 3,
            period: "2026Q1".into(),
            period_label: "For the three months ended March 31,".into(),
            site_raw: "Site Z (47.5%)2".into(),
            site: "Site Z".into(),
            printed_share: Some("47.5".into()),
            label: "Recovery rate (percent)".into(),
            label_as_printed: "Recovery rate (percent)2".into(),
            footnote: Some(2),
            section: None,
            value: MineStatsCell::Number {
                raw_text: raw.into(),
                value: raw.parse().expect("number"),
            },
        };
        (book, value)
    }

    #[test]
    fn disposed_zeros_are_owned_by_the_registry_and_nonzero_values_survive() {
        let mut profile = synthetic_profile();
        profile.subject_statuses = vec![SubjectStatus::from_date(
            "Site Z",
            StatusKind::Disposed,
            (2025, 12, 1),
            "synthetic",
        )];
        let (book, raw) = example("0", "0%");
        assert!(matches!(
            raw.value,
            MineStatsCell::Number { value: 0.0, .. }
        ));
        let disposed = key::from_mine_stats(&profile, &book, "2026Q1", &raw).expect("claim");
        assert_eq!(disposed.status_evidence.as_deref(), Some("synthetic"));
        assert!(
            disposed
                .to_json()
                .contains("\"status_evidence\":\"synthetic\"")
        );
        let mut recited = disposed.clone();
        recited.status_evidence = Some("amended evidence".into());
        assert_ne!(
            key::claims_digest([&disposed]),
            key::claims_digest([&recited])
        );
        assert_eq!(
            disposed.claim.value,
            PublicValue::Nil {
                token: "0".into(),
                reason: NilReason::Disposed
            }
        );
        profile.subject_statuses.clear();
        let active = key::from_mine_stats(&profile, &book, "2026Q1", &raw).expect("claim");
        assert!(active.status_evidence.is_none());
        assert!(matches!(active.claim.value, PublicValue::Number { .. }));
        assert_ne!(active.claim.claim_id, disposed.claim.claim_id);
        assert_ne!(
            key::claims_digest([&active]),
            key::claims_digest([&disposed])
        );
        profile.subject_statuses.push(SubjectStatus::from_date(
            "Site Z",
            StatusKind::Disposed,
            (2025, 12, 1),
            "synthetic",
        ));
        let (book, raw) = example("0.91", "0%");
        let nonzero = key::from_mine_stats(&profile, &book, "2026Q1", &raw).expect("claim");
        assert!(matches!(
            nonzero.claim.value,
            PublicValue::Number {
                under_status: Some(StatusKind::Disposed),
                ..
            }
        ));
        assert_eq!(nonzero.claim.evidence, EvidenceClass::Assumed);
        assert_eq!(nonzero.claim.state, KnowledgeState::Unknown);
        assert!(nonzero.to_json().contains("\"evidence\":\"synthetic\""));
    }

    #[test]
    fn selected_percent_section_keeps_the_stored_fraction_unit() {
        let profile = synthetic_profile();
        for format in ["0%", "0%;(0%);\"-\""] {
            for raw in ["0.91", "0"] {
                let (book, value) = example(raw, format);
                let cell =
                    key::from_mine_stats(&profile, &book, "2026Q1", &value).expect("fraction");
                assert_eq!(cell.key.unit, "mass_fraction", "{format} {raw}");
                if raw == "0.91" {
                    assert!(
                        matches!(cell.claim.value, PublicValue::Number { ref stored, displayed: Ok(ref d), .. }
                    if stored == "0.91" && d == "91%")
                    );
                }
            }
        }
        let (book, value) = example("91", "0");
        assert_eq!(
            key::from_mine_stats(&profile, &book, "2026Q1", &value)
                .expect("percent")
                .key
                .unit,
            "percent"
        );
    }

    #[test]
    fn printed_share_and_unit_corrections_are_explicit_and_never_rescale_values() {
        let mut profile = synthetic_profile();
        profile
            .printed_share_corrections
            .push(PrintedShareCorrection {
                subject: "Site Z".into(),
                printed: "47.5".into(),
                num: 49,
                den: 200,
                evidence: "synthetic-share".into(),
            });
        profile.mine_stats_label_aliases.push(MineStatsLabelAlias {
            label: "Production (millions of pounds)".into(),
            reads_as: "Production (Thousands of tonnes)".into(),
            first: "2024Q1".into(),
            last: "2026Q1".into(),
            evidence: "synthetic-unit".into(),
        });
        let (book, mut raw) = example("18", "0");
        raw.label = "Production (millions of pounds)".into();
        raw.label_as_printed = raw.label.clone();
        let cell = key::from_mine_stats(&profile, &book, "2026Q1", &raw).expect("corrected");
        assert_eq!(
            cell.printed_basis,
            OwnershipBasis::Attributable {
                num: 19,
                den: 40,
                source: ShareSource::Printed
            }
        );
        assert_eq!(
            cell.key.ownership_basis,
            OwnershipBasis::Attributable {
                num: 49,
                den: 200,
                source: ShareSource::Registry
            }
        );
        assert_eq!(cell.basis_evidence.as_deref(), Some("synthetic-share"));
        assert_eq!(cell.key.unit, "kt");
        assert_eq!(cell.unit_evidence.as_deref(), Some("synthetic-unit"));
        assert_eq!(key::raw_token(&cell.claim.value), "18");
        raw.view = View::Summary;
        raw.label = "cu_prod_100_kt".into();
        let full =
            key::from_mine_stats(&profile, &book, "2026Q1", &raw).expect("explicit full basis");
        assert_eq!(full.key.ownership_basis, OwnershipBasis::Consolidated100);
        assert_eq!(full.basis_evidence, None);
    }

    #[test]
    fn corrected_and_printed_shares_share_a_series_without_losing_provenance() {
        let mut profile = synthetic_profile();
        profile
            .printed_share_corrections
            .push(PrintedShareCorrection {
                subject: "Site Z".into(),
                printed: "47.5".into(),
                num: 49,
                den: 200,
                evidence: "synthetic-share".into(),
            });
        let (book, mut raw) = example("18", "0");
        raw.view = View::Summary;
        raw.label = "au_prod_attr_koz".into();
        raw.label_as_printed = raw.label.clone();
        let first = key::from_mine_stats(&profile, &book, "2026Q1", &raw).expect("corrected");
        raw.locator = "2027Q1:Minestats3!F19".into();
        raw.site_raw = "Site Z (24.5%)".into();
        raw.printed_share = Some("24.5".into());
        let later = key::from_mine_stats(&profile, &book, "2027Q1", &raw).expect("printed");
        let printed = OwnershipBasis::Attributable {
            num: 49,
            den: 200,
            source: ShareSource::Printed,
        };
        assert_eq!(later.key.ownership_basis, printed);
        assert!(matches!(
            first.key.ownership_basis,
            OwnershipBasis::Attributable {
                num: 49,
                den: 200,
                source: ShareSource::Registry
            }
        ));
        assert_eq!(first.basis_evidence.as_deref(), Some("synthetic-share"));
        assert_eq!(later.basis_evidence, None);
        assert_eq!(first.key.series_key(), later.key.series_key());
        assert_ne!(first.claim.claim_id, later.claim.claim_id);
        let mut source_only = first.key.clone();
        source_only.ownership_basis = printed;
        assert_eq!(first.key.series_key(), source_only.series_key());
        assert_ne!(
            first.key.canonical_tag("18"),
            source_only.canonical_tag("18")
        );
        for basis in [
            OwnershipBasis::Consolidated100,
            OwnershipBasis::Unprinted,
            OwnershipBasis::Attributable {
                num: 19,
                den: 40,
                source: ShareSource::Printed,
            },
        ] {
            let mut changed = later.key.clone();
            changed.ownership_basis = basis;
            assert_ne!(first.key.series_key(), changed.series_key());
        }
        let mut changed = later.key.clone();
        changed.valid_period.year += 1;
        assert_ne!(first.key.series_key(), changed.series_key());
        let mut changed = later.key.clone();
        changed.unit = "oz".into();
        assert_ne!(first.key.series_key(), changed.series_key());
        let mut changed = later.key.clone();
        changed.scope = key::Scope::Company;
        assert_ne!(first.key.series_key(), changed.series_key());
        let cells = [first, later];
        let grouped = key::series(&cells);
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped.values().next().expect("series").len(), 2);
    }

    #[test]
    fn read_must_match_its_release_and_source_cell() {
        let profile = synthetic_profile();
        let (book, mut raw) = example("0.91", "0%");
        assert!(key::from_mine_stats(&profile, &book, "2026Q2", &raw).is_err());
        raw.value = MineStatsCell::Number {
            raw_text: "0.92".into(),
            value: 0.92,
        };
        assert!(key::from_mine_stats(&profile, &book, "2026Q1", &raw).is_err());
        assert_eq!(
            key::parse_period_code("20269M").expect("9M").code(),
            "20269M"
        );
        assert!(key::parse_period_code("2026Q5").is_none());
        assert!(key::parse_period_code("éééQ1").is_none());
    }

    #[test]
    #[ignore = "needs all registered Barrick workbooks"]
    fn barrick_mine_stats_claims_known_answers() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = fmine_public::run_barrick_m0_assurance(&root).expect("registered sources");
        let at = |file: &str, sheet: &str, cell: &str| {
            report
                .public_claims
                .cells
                .get(&format!("Barrick_{file}_Mine_Stats.xlsx:{sheet}!{cell}"))
                .expect("keyed cell")
        };
        for (file, sheet, cell) in [
            ("Q1_2026", "Minestats3", "F19"),
            ("Q2_2026", "Minestats3", "F19"),
            ("Q2_2026", "Minestats3", "F64"),
            ("Q1_2026", "Minestats1", "N27"),
            ("Q1_2026", "Minestats1", "N28"),
            ("Q2_2026", "Minestats1", "N27"),
            ("Q2_2026", "Minestats1", "N28"),
        ] {
            assert!(
                matches!(
                    at(file, sheet, cell).claim.value,
                    PublicValue::Nil {
                        reason: NilReason::Disposed,
                        ..
                    }
                ),
                "{file}:{sheet}!{cell}"
            );
        }
        let first = at("Q1_2024", "QTD", "G23");
        let later = at("Q1_2025", "QTD", "N22");
        assert_eq!(first.key.series_key(), later.key.series_key());
        assert_ne!(first.claim.claim_id, later.claim.claim_id);
        assert_eq!(key::raw_token(&first.claim.value), "1479");
        assert_eq!(key::raw_token(&later.claim.value), "1474");
        assert_eq!(first.key.reporting_vintage.release, "2024Q1");
        assert_eq!(later.key.reporting_vintage.release, "2025Q1");
        let porgera = at("Q2_2024", "QTD", "C15");
        assert_eq!(
            porgera.printed_basis,
            OwnershipBasis::Attributable {
                num: 19,
                den: 40,
                source: ShareSource::Printed
            }
        );
        assert_eq!(
            porgera.key.ownership_basis,
            OwnershipBasis::Attributable {
                num: 49,
                den: 200,
                source: ShareSource::Registry
            }
        );
        assert_eq!(porgera.basis_evidence.as_deref(), Some("W1-DR-01"));
        let comparative: Vec<_> = report
            .public_claims
            .cells
            .values()
            .filter(|cell| {
                cell.key.subject == "Porgera"
                    && cell.key.valid_period.code() == "2024Q2"
                    && cell.key.reporting_vintage.release == "2025Q2"
                    && cell.key.line.label == "summary:au_prod_attr_koz"
            })
            .collect();
        assert_eq!(comparative.len(), 1);
        assert_eq!(
            comparative[0].key.ownership_basis,
            OwnershipBasis::Attributable {
                num: 49,
                den: 200,
                source: ShareSource::Printed
            }
        );
        assert_eq!(porgera.key.series_key(), comparative[0].key.series_key());
        assert_ne!(porgera.claim.claim_id, comparative[0].claim.claim_id);
        assert_eq!(
            at("Q2_2024", "Minestats1", "H35").claim.value,
            PublicValue::NotApplicable {
                token: "n/a".into()
            }
        );
        assert_eq!(
            at("Q2_2024", "Minestats1", "H95").claim.value,
            PublicValue::Nil {
                token: "—".into(),
                reason: NilReason::Unknown
            }
        );
        assert!(matches!(at("Q2_2026", "YTD", "E11").claim.value,
        PublicValue::Number { ref stored, under_status: Some(StatusKind::Disposed), .. } if stored == "1691"));
        let mut reasons = std::collections::BTreeMap::new();
        for refusal in &report.public_claims.refusals {
            *reasons.entry(&refusal.reason).or_insert(0_usize) += 1;
        }
        println!(
            "claims={} refusals={:?}",
            report.public_claims.cells.len(),
            reasons
        );
    }
}
