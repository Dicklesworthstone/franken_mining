//! Cross-crate source verification and canonical Mine Stats inventory checks (BD-332.S3).

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use fmine_core::hash::Digest32;
    use fmine_evidence::EvidenceClass;
    use fmine_io::omf::json::{Json, parse};
    use fmine_io::xlsx::{MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx};
    use fmine_public::LoadedSource;
    use fmine_public::key::mine_stats::{KeyedClaims, build_claims, load_claims, value_kind};
    use fmine_public::key::{
        KeyedCell, NilReason, OwnershipBasis, PublicValue, ShareSource, StatusKind, SubjectStatus,
        claims_digest, from_mine_stats, raw_token,
    };
    use fmine_public::mine_stats::{CellRefusal, MineStatsCell, MineStatsRead, read_mine_stats};
    use fmine_public::sources::{
        IssuerProfile, PrintedShareCorrection, RegisteredSource, SourceKind, load_issuer_profile,
    };

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn fixture_bytes(raw: MinimalCell) -> Vec<u8> {
        let text = |s: &str| MinimalCell::Text(s.into());
        build_minimal_xlsx(&[MinimalSheet {
            name: "Minestats1".into(),
            hidden: false,
            cells: vec![
                ("A1".into(), text("Mine Statistics (Gold)")),
                ("B2".into(), text("Site A (61.5%)1")),
                ("A3".into(), text("For the three months ended March 31,")),
                ("B3".into(), MinimalCell::Number(2030.0)),
                ("A4".into(), text("Recovery rate (percent)")),
                ("B4".into(), raw),
            ],
        }])
    }

    fn fixture(raw: MinimalCell) -> (LoadedSource, MineStatsRead, IssuerProfile) {
        let bytes = fixture_bytes(raw);
        let workbook = Workbook::from_bytes("synthetic.xlsx", &bytes).expect("workbook");
        let read = read_mine_stats(&workbook, "2030Q1", None).expect("read");
        assert!(read.refusals.is_empty(), "{:?}", read.refusals);
        assert_eq!(read.values.len(), 1);
        let mut profile = fmine_public::synthetic::synthetic_profile();
        profile.sources.push(RegisteredSource {
            retrieved_utc: None,
            published_at: None,
            published_at_basis: None,
            file_name: "synthetic.xlsx".into(),
            kind: SourceKind::MineStats,
            quarter: "2030Q1".into(),
            url: "https://example.invalid/synthetic.xlsx".into(),
            url_basis: "synthetic".into(),
            bytes: bytes.len() as u64,
            sha256_hex: Digest32(workbook.sha256).to_hex(),
        });
        profile.subject_statuses = vec![SubjectStatus::from_date(
            "Site A",
            StatusKind::Disposed,
            (2029, 12, 1),
            "synthetic-status",
        )];
        let source = LoadedSource {
            file_name: "synthetic.xlsx".into(),
            kind: SourceKind::MineStats,
            quarter: "2030Q1".into(),
            url: "https://example.invalid/synthetic.xlsx".into(),
            url_basis: "synthetic".into(),
            bytes: bytes.len() as u64,
            census: workbook.cell_census(),
            workbook,
        };
        (source, read, profile)
    }

    fn inventory(p: &IssuerProfile, s: &LoadedSource, read: &MineStatsRead) -> KeyedClaims {
        build_claims(
            p,
            std::slice::from_ref(s),
            &[(s.file_name.clone(), read.clone())],
        )
    }

    #[test]
    fn empty_mine_stats_coverage_is_a_named_workbook_refusal() {
        let text = |s: &str| MinimalCell::Text(s.into());
        let title = vec![("A1".into(), text("Mine Statistics (Gold)"))];
        let block = vec![
            ("A1".into(), text("Mine Statistics (Gold)")),
            ("B2".into(), text("Site A (61.5%)1")),
            ("A3".into(), text("For the three months ended March 31,")),
            ("B3".into(), MinimalCell::Number(2030.0)),
        ];
        let mut populated = block.clone();
        populated.extend([
            ("A4".into(), text("Recovery rate (percent)")),
            ("B4".into(), MinimalCell::Number(91.0)),
        ]);
        for (name, hidden, cells, blocks, reason) in [
            (
                "RenamedStatistics",
                false,
                populated.clone(),
                0,
                "no recognized visible QTD, YTD or Minestats worksheet",
            ),
            (
                "Minestats1",
                true,
                populated,
                0,
                "no recognized visible QTD, YTD or Minestats worksheet",
            ),
            (
                "Minestats1",
                false,
                title,
                0,
                "recognized Mine Stats worksheets contain no reporting blocks",
            ),
            (
                "Minestats1",
                false,
                block,
                1,
                "recognized Mine Stats reporting blocks contain no typed observations",
            ),
        ] {
            let bytes = build_minimal_xlsx(&[MinimalSheet {
                name: name.into(),
                hidden,
                cells,
            }]);
            let workbook = Workbook::from_bytes("coverage.xlsx", &bytes).expect("workbook");
            let read = read_mine_stats(&workbook, "2030Q1", None).expect("read");
            assert_eq!(read.blocks, blocks, "{name}, hidden={hidden}");
            assert!(read.values.is_empty(), "{name}, hidden={hidden}");
            assert_eq!(
                read.refusals,
                vec![CellRefusal {
                    locator: "coverage.xlsx".into(),
                    reason: reason.into(),
                }]
            );
            assert_eq!(read.hidden_sheets.len(), usize::from(hidden));
        }
    }

    #[test]
    fn unreadable_minestats_block_is_named_beside_valid_blocks() {
        let text = |s: &str| MinimalCell::Text(s.into());
        for (year, site) in [
            (Some(text("2030")), Some("Site B (100%)")),
            (None, Some("Site B (100%)")),
            (Some(MinimalCell::Number(2030.5)), Some("Site B (100%)")),
            (Some(MinimalCell::Number(2030.0)), None),
        ] {
            let mut cells = vec![("A1".into(), text("Mine Statistics (Gold)"))];
            for row in [3, 8, 13] {
                cells.extend([
                    (
                        format!("A{row}"),
                        text("For the three months ended March 31,"),
                    ),
                    (format!("A{}", row + 1), text("Recovery rate (percent)")),
                    (format!("B{}", row + 1), MinimalCell::Number(91.0)),
                ]);
                if row == 8 {
                    if let Some(year) = year.clone() {
                        cells.push(("B8".into(), year));
                    }
                    if let Some(site) = site {
                        cells.push(("B7".into(), text(site)));
                    }
                } else {
                    cells.extend([
                        (format!("B{row}"), MinimalCell::Number(2030.0)),
                        (format!("B{}", row - 1), text("Site A (61.5%)1")),
                    ]);
                }
            }
            let bytes = build_minimal_xlsx(&[MinimalSheet {
                name: "Minestats1".into(),
                hidden: false,
                cells,
            }]);
            let workbook = Workbook::from_bytes("partial.xlsx", &bytes).expect("workbook");
            let read = read_mine_stats(&workbook, "2030Q1", None).expect("read");
            assert_eq!((read.blocks, read.values.len()), (3, 2));
            assert_eq!(
                read.values
                    .iter()
                    .map(|v| (v.locator.as_str(), v.period.as_str(), v.site.as_str()))
                    .collect::<Vec<_>>(),
                [
                    ("2030Q1:Minestats1!B4", "2030Q1", "Site A"),
                    ("2030Q1:Minestats1!B14", "2030Q1", "Site A"),
                ]
            );
            assert!(read.values.iter().all(|v| v.value
                == MineStatsCell::Number {
                    raw_text: "91".into(),
                    value: 91.0,
                }));
            let mut expected = Vec::new();
            if matches!(&year, Some(MinimalCell::Text(_))) {
                expected.push(CellRefusal {
                    locator: "2030Q1:Minestats1!B8".into(),
                    reason: "Minestats year is stored as text; a numeric integer year is required"
                        .into(),
                });
            }
            if site.is_none() {
                expected.push(CellRefusal {
                    locator: "2030Q1:Minestats1!B7".into(),
                    reason: "no site name at or left of this cell".into(),
                });
            }
            expected.push(CellRefusal {
                locator: "2030Q1:Minestats1!A8".into(),
                reason: "Minestats block has no readable numeric-year columns with a site name"
                    .into(),
            });
            assert_eq!(read.refusals, expected);
            let source = LoadedSource {
                file_name: "partial.xlsx".into(),
                kind: SourceKind::MineStats,
                quarter: "2030Q1".into(),
                url: "https://example.invalid/partial.xlsx".into(),
                url_basis: "synthetic".into(),
                bytes: bytes.len() as u64,
                census: workbook.cell_census(),
                workbook,
            };
            let all = inventory(
                &fmine_public::synthetic::synthetic_profile(),
                &source,
                &read,
            );
            expected.sort_by(|a, b| a.locator.cmp(&b.locator));
            assert_eq!(all.refusals, expected);
            assert_eq!(all.claims.len(), 2);
        }
    }

    #[test]
    fn text_year_column_is_named_without_dropping_numeric_neighbors() {
        let text = |s: &str| MinimalCell::Text(s.into());
        let bytes = build_minimal_xlsx(&[MinimalSheet {
            name: "Minestats1".into(),
            hidden: false,
            cells: vec![
                ("A1".into(), text("Mine Statistics (Gold)")),
                ("B2".into(), text("Site A (61.5%)")),
                ("C2".into(), text("Site B (100%)")),
                ("D2".into(), text("Site C (100%)")),
                ("A3".into(), text("For the three months ended March 31,")),
                ("B3".into(), MinimalCell::Number(2030.0)),
                ("C3".into(), text("2030")),
                ("D3".into(), MinimalCell::Number(2029.0)),
                ("E3".into(), text("")),
                ("F3".into(), text("Notes")),
                ("G3".into(), text(" \t")),
                ("A4".into(), text("Recovery rate (percent)")),
                ("B4".into(), MinimalCell::Number(91.0)),
                ("C4".into(), MinimalCell::Number(92.0)),
                ("D4".into(), text("n/a")),
            ],
        }]);
        let workbook = Workbook::from_bytes("mixed-columns.xlsx", &bytes).expect("workbook");
        let read = read_mine_stats(&workbook, "2030Q1", None).expect("read");
        assert_eq!((read.blocks, read.values.len()), (1, 2));
        assert_eq!(
            read.values
                .iter()
                .map(|v| (v.locator.as_str(), v.period.as_str()))
                .collect::<Vec<_>>(),
            [
                ("2030Q1:Minestats1!B4", "2030Q1"),
                ("2030Q1:Minestats1!D4", "2029Q1"),
            ]
        );
        assert_eq!(read.count("number"), 1);
        assert_eq!(read.count("na"), 1);
        assert_eq!(
            read.refusals,
            vec![CellRefusal {
                locator: "2030Q1:Minestats1!C3".into(),
                reason: "Minestats year is stored as text; a numeric integer year is required"
                    .into(),
            }]
        );
    }

    #[test]
    fn unreadable_summary_block_is_named_beside_valid_blocks() {
        let text = |s: &str| MinimalCell::Text(s.into());
        for (sheet, months, period) in [("QTD", "three", "2030Q2"), ("YTD", "six", "2030H1")] {
            let mut cells = Vec::new();
            for row in [3, 8, 13] {
                cells.extend([
                    (
                        format!("A{row}"),
                        text(&format!("For the {months} months ended June 30,")),
                    ),
                    (format!("B{row}"), MinimalCell::Number(2030.0)),
                    (format!("A{}", row + 2), text("Site A (61.5%)")),
                    (format!("B{}", row + 2), MinimalCell::Number(0.0)),
                ]);
                if row != 8 {
                    cells.push((
                        format!("B{}", row + 1),
                        text("Gold produced (000s oz 100%)"),
                    ));
                }
            }
            let bytes = build_minimal_xlsx(&[MinimalSheet {
                name: sheet.into(),
                hidden: false,
                cells,
            }]);
            let workbook = Workbook::from_bytes("partial-summary.xlsx", &bytes).expect("workbook");
            let read = read_mine_stats(&workbook, "2030Q2", None).expect("read");
            assert_eq!(
                (read.blocks, read.values.len(), read.numeric_zeros()),
                (3, 2, 2)
            );
            assert_eq!(
                read.values
                    .iter()
                    .map(|v| v.locator.clone())
                    .collect::<Vec<_>>(),
                [format!("2030Q2:{sheet}!B5"), format!("2030Q2:{sheet}!B15")]
            );
            assert!(read.values.iter().all(|v| v.period == period));
            assert_eq!(
                read.refusals,
                vec![CellRefusal {
                    locator: format!("2030Q2:{sheet}!A8"),
                    reason: "summary block has no readable metric columns with an unambiguous four-digit year"
                        .into(),
                }]
            );
        }
    }

    #[test]
    fn empty_context_blocks_and_hidden_layouts_do_not_refuse_valid_observations() {
        let text = |s: &str| MinimalCell::Text(s.into());
        let period = "For the three months ended March 31,";
        let bytes = build_minimal_xlsx(&[
            MinimalSheet {
                name: "QTD".into(),
                hidden: false,
                cells: vec![
                    ("A3".into(), text(period)),
                    ("B3".into(), MinimalCell::Number(2030.0)),
                    ("B4".into(), text("Gold produced (000s oz 100%)")),
                ],
            },
            MinimalSheet {
                name: "Minestats1".into(),
                hidden: false,
                cells: vec![
                    ("B2".into(), text("Site A (61.5%)")),
                    ("A3".into(), text(period)),
                    ("B3".into(), MinimalCell::Number(2030.0)),
                    ("B7".into(), text("Site B (100%)")),
                    ("A8".into(), text(period)),
                    ("B8".into(), MinimalCell::Number(2030.0)),
                    ("A9".into(), text("Recovery rate (percent)")),
                    ("B9".into(), text("— ")),
                ],
            },
            MinimalSheet {
                name: "MinestatsHidden".into(),
                hidden: true,
                cells: vec![
                    ("A3".into(), text(period)),
                    ("B3".into(), text("2030")),
                    ("A4".into(), text("Recovery rate (percent)")),
                    ("B4".into(), MinimalCell::Number(99.0)),
                ],
            },
        ]);
        let workbook = Workbook::from_bytes("context.xlsx", &bytes).expect("workbook");
        let read = read_mine_stats(&workbook, "2030Q1", None).expect("read");
        assert_eq!((read.blocks, read.values.len()), (3, 1));
        assert!(read.refusals.is_empty(), "{:?}", read.refusals);
        assert_eq!(read.hidden_sheets, ["MinestatsHidden"]);
        assert_eq!(read.values[0].locator, "2030Q1:Minestats1!B9");
        assert_eq!(
            read.values[0].value,
            MineStatsCell::Nil {
                token: "— ".into()
            }
        );
    }

    #[test]
    fn empty_source_refusal_survives_verified_assurance_and_claim_inventory() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let ws = std::env::temp_dir().join(format!(
            "fmine-empty-coverage-{}-{nonce}",
            std::process::id()
        ));
        fmine_public::synthetic::write_issuer_workspace(&ws).expect("synthetic workspace");
        let profile = load_issuer_profile(&ws, "synthetic").expect("registered sources");
        let report =
            fmine_public::run_public_assurance(&profile, &ws).expect("verified source bytes");
        let file = fmine_public::synthetic::SYNTHETIC_LATER_STATS_FILE;
        let source = report
            .sources
            .iter()
            .find(|source| source.file_name == file)
            .expect("verified empty source retained");
        let registered = profile
            .sources
            .iter()
            .find(|source| source.file_name == file)
            .expect("empty source registered");
        let registered_locator = format!("{}/{}", profile.corpus_dir, registered.file_name);
        assert_eq!(source.workbook.artifact_path, registered_locator);
        assert_eq!(source.bytes, registered.bytes);
        assert_eq!(
            Digest32(source.workbook.sha256).to_hex(),
            registered.sha256_hex
        );
        let expected = CellRefusal {
            locator: source.workbook.artifact_path.clone(),
            reason: "recognized Mine Stats worksheets contain no reporting blocks".into(),
        };
        let (_, empty) = report
            .mine_stats
            .iter()
            .find(|(name, _)| name == file)
            .expect("empty source retained");
        assert_eq!(empty.blocks, 0);
        assert!(empty.values.is_empty());
        assert_eq!(empty.refusals, vec![expected.clone()]);
        assert!(report.mine_stats_claims.refusals.contains(&expected));
        assert!(
            report
                .public_claims
                .refusals
                .iter()
                .any(|r| r.locator == expected.locator && r.reason == expected.reason)
        );
        assert!(!report.public_claims.cells.is_empty());
        // The reconstruction fixture prints its Minestats year as text; generic year anchors are numeric.
        let (_, unobserved) = report
            .mine_stats
            .iter()
            .find(|(name, _)| name == fmine_public::synthetic::SYNTHETIC_STATS_FILE)
            .expect("original Mine Stats source retained");
        assert!(unobserved.blocks > 0, "{unobserved:?}");
        assert!(unobserved.values.is_empty(), "{unobserved:?}");
        let no_observations = CellRefusal {
            locator: format!(
                "{}/{}",
                profile.corpus_dir,
                fmine_public::synthetic::SYNTHETIC_STATS_FILE
            ),
            reason: "recognized Mine Stats reporting blocks contain no typed observations".into(),
        };
        let layout_refusals = vec![
            CellRefusal {
                locator: "2030Q1:QTD!A3".into(),
                reason: "summary block has no readable metric columns with an unambiguous four-digit year"
                    .into(),
            },
            CellRefusal {
                locator: "2030Q1:Minestats3!B3".into(),
                reason: "Minestats year is stored as text; a numeric integer year is required".into(),
            },
            CellRefusal {
                locator: "2030Q1:Minestats3!A3".into(),
                reason: "Minestats block has no readable numeric-year columns with a site name".into(),
            },
        ];
        let mut unobserved_refusals = layout_refusals.clone();
        unobserved_refusals.push(no_observations.clone());
        assert_eq!(unobserved.refusals, unobserved_refusals);
        for refusal in layout_refusals {
            assert!(report.mine_stats_claims.refusals.contains(&refusal));
            assert!(
                report
                    .public_claims
                    .refusals
                    .iter()
                    .any(|r| { r.locator == refusal.locator && r.reason == refusal.reason })
            );
        }
        assert!(report.mine_stats_claims.refusals.contains(&no_observations));
        assert!(!report.instances.is_empty());
        assert_eq!(report.sources.len(), profile.sources.len());
        let json = report.to_json();
        assert!(json.contains(&format!("\"locator\":\"{}\"", expected.locator)));
        assert!(json.contains(&expected.reason));
        let body = parse(json.as_bytes()).expect("assurance JSON");
        let coverage = body
            .get("mine_stats_claims")
            .and_then(|inventory| inventory.get("by_file"))
            .and_then(|files| files.get(file))
            .expect("verified workbook with no readable claims stays visible");
        assert_eq!(
            coverage.get("sha256").and_then(Json::as_str),
            Some(registered.sha256_hex.as_str())
        );
        assert_eq!(coverage.get("claims"), Some(&Json::Number("0".into())));
        assert!(
            coverage
                .get("by_kind")
                .and_then(Json::as_object)
                .expect("empty kind counts")
                .is_empty()
        );
        assert_eq!(coverage.get("refusals"), Some(&Json::Number("1".into())));
        let loaded = load_claims(&profile, &ws).expect("verified direct inventory");
        // Both entry points use load_registered_sources and its root-independent registry path.
        assert!(loaded.refusals.contains(&CellRefusal {
            locator: registered_locator,
            reason: expected.reason.clone(),
        }));
        assert_eq!(loaded.digest, report.mine_stats_claims.digest);
        let again = fmine_public::run_public_assurance(&profile, &ws).expect("rerun");
        assert_eq!(json, again.to_json());
    }

    #[test]
    fn zero_nil_and_missing_values_remain_observations() {
        for (cell, kind) in [
            (MinimalCell::Number(0.0), "number"),
            (MinimalCell::Text("—".into()), "nil"),
            (MinimalCell::Text("n/a".into()), "na"),
        ] {
            let (source, read, profile) = fixture(cell);
            assert_eq!(
                (read.blocks, read.values.len(), read.count(kind)),
                (1, 1, 1)
            );
            assert!(inventory(&profile, &source, &read).refusals.is_empty());
        }
        let text = |s: &str| MinimalCell::Text(s.into());
        let bytes = build_minimal_xlsx(&[MinimalSheet {
            name: "QTD".into(),
            hidden: false,
            cells: vec![
                ("A3".into(), text("For the three months ended March 31,")),
                ("B3".into(), MinimalCell::Number(2030.0)),
                ("B4".into(), text("Gold produced (000s oz 100%)")),
                ("A5".into(), text("Site A (61.5%)")),
            ],
        }]);
        let workbook = Workbook::from_bytes("missing-value.xlsx", &bytes).expect("workbook");
        let read = read_mine_stats(&workbook, "2030Q1", None).expect("read");
        assert!(read.refusals.is_empty(), "{:?}", read.refusals);
        assert_eq!(
            (read.blocks, read.values.len(), read.count("missing")),
            (1, 1, 1)
        );
        assert_eq!(read.values[0].locator, "2030Q1:QTD!B5");
    }

    #[test]
    fn inventory_keeps_canonical_ids_values_anchors_and_digest() {
        let (source, read, p) = fixture(MinimalCell::Number(91.0));
        let all = inventory(&p, &source, &read);
        assert!(all.refusals.is_empty());
        assert_eq!(all.claims.len(), 1);
        let canonical = from_mine_stats(&p, &source.workbook, &source.quarter, &read.values[0])
            .expect("canonical claim");
        assert_eq!(all.claims[0].to_json(), canonical.to_json());
        assert_eq!(all.digest, claims_digest([&canonical]));
        assert_eq!(all.claims[0].key.unit, "percent");
        assert_eq!(all.claims[0].locator, "synthetic.xlsx:Minestats1!B4");
        assert_eq!(
            all.claims[0].claim.anchors,
            vec![source.workbook.make_anchor("Minestats1", "B4")]
        );
        assert!(
            all.claims[0]
                .to_json()
                .contains("\"evidence\":\"synthetic\"")
        );
        assert!(all.summary_json().contains("\"number\":1"));
    }

    #[test]
    fn inventory_preserves_status_and_registered_basis_interpretation() {
        let (source, read, mut p) = fixture(MinimalCell::Number(0.0));
        let v = &read.values[0];
        assert!(matches!(v.value, MineStatsCell::Number { value: 0.0, .. }));
        let all = inventory(&p, &source, &read);
        let claim = &all.claims[0];
        assert_eq!(
            claim.claim.value,
            PublicValue::Nil {
                token: "0".into(),
                reason: NilReason::Disposed
            }
        );
        assert_eq!(claim.key.unit, "percent");
        assert_eq!(
            claim.key.ownership_basis,
            OwnershipBasis::Attributable {
                num: 123,
                den: 200,
                source: ShareSource::Printed
            }
        );
        assert_eq!(claim.claim.evidence, EvidenceClass::Assumed);
        assert_eq!(claim.status_evidence.as_deref(), Some("synthetic-status"));
        p.subject_statuses.clear();
        let without = inventory(&p, &source, &read);
        assert!(matches!(
            without.claims[0].claim.value,
            PublicValue::Number { .. }
        ));
        assert_ne!(claim.claim.claim_id, without.claims[0].claim.claim_id);
        assert_ne!(all.digest, without.digest);
        p.printed_share_corrections.push(PrintedShareCorrection {
            subject: "Site A".into(),
            printed: "61.5".into(),
            num: 1,
            den: 2,
            evidence: "synthetic-correction".into(),
        });
        let corrected_inventory = inventory(&p, &source, &read);
        let corrected = &corrected_inventory.claims[0];
        assert_eq!(corrected.printed_basis, claim.key.ownership_basis);
        assert_eq!(
            corrected.key.ownership_basis,
            OwnershipBasis::Attributable {
                num: 1,
                den: 2,
                source: ShareSource::Registry
            }
        );
        assert_ne!(corrected.claim.claim_id, without.claims[0].claim.claim_id);
        assert_eq!(
            corrected.basis_evidence.as_deref(),
            Some("synthetic-correction")
        );
    }

    #[test]
    fn source_binding_failures_and_reader_refusals_stay_in_the_inventory() {
        let (source, read, p) = fixture(MinimalCell::Number(91.0));
        let reads = [(source.file_name.clone(), read.clone())];
        let missing = build_claims(&p, &[], &reads);
        assert!(missing.claims.is_empty());
        assert_eq!(missing.refusals.len(), 1);
        assert_eq!(missing.refusals[0].locator, "synthetic.xlsx:Minestats1!B4");
        assert!(missing.refusals[0].reason.contains("no verified source"));

        let mut wrong_read = read.clone();
        wrong_read.values[0].value = MineStatsCell::Number {
            raw_text: "92".into(),
            value: 92.0,
        };
        let mismatched = inventory(&p, &source, &wrong_read);
        assert!(mismatched.claims.is_empty());
        assert_eq!(mismatched.refusals.len(), 1);
        assert!(
            mismatched.refusals[0]
                .reason
                .contains("read does not match")
        );

        let mut wrong_registry = p.clone();
        wrong_registry.sources[0].sha256_hex = "00".repeat(32);
        let mismatched = inventory(&wrong_registry, &source, &read);
        assert!(mismatched.claims.is_empty());
        assert_eq!(mismatched.refusals.len(), 1);
        assert!(
            mismatched.refusals[0]
                .reason
                .contains("do not match the registry")
        );

        let mut refused_read = read;
        refused_read.values[0].section = Some("Unregistered measurement".into());
        refused_read.refusals.push(CellRefusal {
            locator: "2030Q1:Minestats1!A9".into(),
            reason: "synthetic reader refusal".into(),
        });
        let refused = inventory(&p, &source, &refused_read);
        assert!(refused.claims.is_empty());
        assert_eq!(refused.refusals.len(), 2);
        assert!(refused.refusals.contains(&refused_read.refusals[0]));
        assert_eq!(
            refused.digest,
            claims_digest(std::iter::empty::<&KeyedCell>())
        );
    }

    #[test]
    fn loaded_inventory_matches_direct_keys_and_refuses_missing_or_altered_sources() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let ws = std::env::temp_dir().join(format!(
            "fmine-claim-inventory-{}-{nonce}",
            std::process::id()
        ));
        let (_, _, mut p) = fixture(MinimalCell::Number(91.0));
        p.corpus_dir = "corpus".into();
        let corpus = ws.join(&p.corpus_dir);
        std::fs::create_dir_all(&corpus).expect("synthetic workspace");
        std::fs::write(
            corpus.join("synthetic.xlsx"),
            fixture_bytes(MinimalCell::Number(91.0)),
        )
        .expect("synthetic workbook");
        let loaded = load_claims(&p, &ws).expect("loaded claims");
        assert!(!loaded.claims.is_empty());
        let sources = fmine_public::load_registered_sources(&p, &ws).expect("verified files");
        let mut canonical = Vec::new();
        for source in sources.iter().filter(|s| s.kind == SourceKind::MineStats) {
            let read = read_mine_stats(&source.workbook, &source.quarter, None).expect("read");
            canonical.extend(read.values.iter().map(|v| {
                from_mine_stats(&p, &source.workbook, &source.quarter, v).expect("canonical key")
            }));
        }
        assert_eq!(loaded.claims.len(), canonical.len());
        assert_eq!(loaded.digest, claims_digest(&canonical));
        let mut altered = p.clone();
        altered.sources[0].sha256_hex = "00".repeat(32);
        assert_eq!(
            load_claims(&altered, &ws).expect_err("digest").code,
            "FMINE-REF-505"
        );
        let mut missing = p;
        missing.sources[0].file_name = "absent.xlsx".into();
        assert_eq!(
            load_claims(&missing, &ws).expect_err("missing").code,
            "FMINE-REF-506"
        );
    }

    #[test]
    #[ignore = "needs the registered Barrick workbooks"]
    fn barrick_mine_stats_claims_known_answers() {
        let p = load_issuer_profile(&root(), "barrick")
            .expect("profile")
            .through("2026Q2")
            .expect("bound");
        let sources = fmine_public::load_registered_sources(&p, &root()).expect("verified files");
        let vocab = p.mine_stats_labels.iter().cloned().collect();
        let reads: Vec<_> = sources
            .iter()
            .filter(|s| s.kind == SourceKind::MineStats)
            .map(|s| {
                (
                    s.file_name.clone(),
                    read_mine_stats(&s.workbook, &s.quarter, Some(&vocab)).expect("read"),
                )
            })
            .collect();
        let all = build_claims(&p, &sources, &reads);
        assert!(all.refusals.is_empty(), "{:?}", all.refusals);
        assert_eq!(
            all.claims.len(),
            reads.iter().map(|(_, r)| r.values.len()).sum::<usize>()
        );
        // FACT: independent openpyxl read of the registered files and status rows (BD-332.S3), captured
        // before these assertions. These counts are value classifications, not calibration or gate results.
        let mut counts = BTreeMap::new();
        for c in &all.claims {
            *counts.entry(value_kind(&c.claim.value)).or_insert(0_usize) += 1;
        }
        assert_eq!(
            counts,
            BTreeMap::from([
                ("number", 18_768),
                ("nil_unknown", 1_173),
                ("nil_disposed", 7),
                ("not_applicable", 2_778),
                ("missing", 6),
            ])
        );
        // FACT: independent pre-Rust reader scripts/oracle/mine_stats_extract3.py, with E119 and
        // the subject-status dates. Columns: number, unknown, disposed, withheld, n/a, missing,
        // and the subset of numbers carrying the after_disposal flag.
        let expected = [
            ("2024Q1", [1082, 108, 0, 0, 190, 0, 0]),
            ("2024Q2", [2204, 183, 0, 0, 373, 0, 0]),
            ("2024Q3", [2210, 177, 0, 0, 373, 0, 0]),
            ("2024Q4", [2213, 173, 0, 0, 372, 2, 0]),
            ("2025Q1", [1095, 52, 0, 0, 145, 0, 0]),
            ("2025Q2", [2213, 82, 0, 0, 288, 1, 0]),
            ("2025Q3", [2220, 82, 0, 0, 281, 1, 0]),
            ("2025Q4", [2266, 44, 0, 0, 306, 0, 0]),
            ("2026Q1", [1063, 90, 3, 0, 152, 0, 0]),
            ("2026Q2", [2202, 182, 4, 0, 298, 2, 1]),
        ];
        let kind_names = [
            "number",
            "nil_unknown",
            "nil_disposed",
            "nil_withheld",
            "not_applicable",
            "missing",
        ];
        let summary = all.summary_json();
        let summary_body = parse(summary.as_bytes()).expect("inventory JSON");
        let files = summary_body
            .get("by_file")
            .expect("verified workbook coverage");
        assert_eq!(
            files.as_object().expect("coverage map").len(),
            expected.len()
        );
        for (release, counts) in expected {
            let source = p
                .sources
                .iter()
                .find(|s| s.kind == SourceKind::MineStats && s.quarter == release)
                .expect("registered Mine Stats release");
            let coverage = files.get(&source.file_name).expect("source coverage");
            assert_eq!(
                coverage.get("sha256").and_then(Json::as_str),
                Some(source.sha256_hex.as_str()),
                "{release}"
            );
            let wanted: BTreeMap<_, _> = kind_names
                .into_iter()
                .zip(counts)
                .filter(|(_, n)| *n > 0)
                .map(|(kind, n)| (kind, Json::Number(n.to_string())))
                .collect();
            let actual: BTreeMap<_, _> = coverage
                .get("by_kind")
                .and_then(Json::as_object)
                .expect("source kind counts")
                .iter()
                .map(|(kind, n)| (kind.as_str(), n.clone()))
                .collect();
            assert_eq!(actual, wanted, "{release}");
            assert_eq!(
                coverage.get("claims"),
                Some(&Json::Number(counts[..6].iter().sum::<usize>().to_string())),
                "{release}"
            );
            assert_eq!(
                coverage.get("after_disposal"),
                Some(&Json::Number(counts[6].to_string())),
                "{release}"
            );
            assert_eq!(
                coverage.get("refusals"),
                Some(&Json::Number("0".into())),
                "{release}"
            );
        }

        let at = |locator: &str| {
            let (release, cell) = locator.split_once(':').expect("oracle locator");
            let file = p
                .sources
                .iter()
                .find(|s| s.kind == SourceKind::MineStats && s.quarter == release)
                .expect("registered Mine Stats release");
            let canonical = format!("{}:{cell}", file.file_name);
            all.claims
                .iter()
                .find(|c| c.locator == canonical)
                .expect(locator)
        };
        let tongon = [
            "2026Q1:Minestats3!F19",
            "2026Q2:Minestats3!F19",
            "2026Q2:Minestats3!F64",
        ];
        let hemlo = [
            "2026Q1:Minestats1!N27",
            "2026Q1:Minestats1!N28",
            "2026Q2:Minestats1!N27",
            "2026Q2:Minestats1!N28",
        ];
        for locator in tongon.into_iter().chain(hemlo) {
            assert!(
                reads.iter().any(|(_, read)| read
                    .values
                    .iter()
                    .any(|v| v.locator == locator && v.is_numeric_zero())),
                "raw reader retains numeric zero at {locator}"
            );
            assert_eq!(
                at(locator).claim.value,
                PublicValue::Nil {
                    token: "0".into(),
                    reason: NilReason::Disposed
                },
                "{locator}"
            );
        }
        let mut without_tongon = p.clone();
        without_tongon
            .subject_statuses
            .retain(|status| !(status.subject == "Tongon" && status.kind == StatusKind::Disposed));
        assert_eq!(
            without_tongon.subject_statuses.len() + 1,
            p.subject_statuses.len()
        );
        let counterfactual = build_claims(&without_tongon, &sources, &reads);
        assert!(counterfactual.refusals.is_empty());
        assert_eq!(counterfactual.claims.len(), all.claims.len());
        assert_ne!(counterfactual.digest, all.digest);
        let expected_changes: std::collections::BTreeSet<_> = tongon
            .into_iter()
            .map(|loc| at(loc).locator.as_str())
            .collect();
        let original_kinds: BTreeMap<_, _> = all
            .claims
            .iter()
            .map(|c| (c.locator.as_str(), value_kind(&c.claim.value)))
            .collect();
        let mut changed = std::collections::BTreeSet::new();
        for c in &counterfactual.claims {
            if original_kinds[c.locator.as_str()] != value_kind(&c.claim.value) {
                assert_eq!(original_kinds[c.locator.as_str()], "nil_disposed");
                assert!(
                    matches!(&c.claim.value, PublicValue::Number { stored, under_status: None, .. } if stored == "0")
                );
                changed.insert(c.locator.as_str());
            }
        }
        assert_eq!(changed, expected_changes);
        let first = at("2024Q1:QTD!G23");
        let later = at("2025Q1:QTD!N22");
        assert_eq!(raw_token(&first.claim.value), "1479");
        assert_eq!(raw_token(&later.claim.value), "1474");
        assert_eq!(first.key.series_key(), later.key.series_key());
        assert_ne!(first.claim.claim_id, later.claim.claim_id);
        let stale_label = at("2024Q2:QTD!C15");
        assert_eq!(
            stale_label.printed_basis,
            OwnershipBasis::Attributable {
                num: 19,
                den: 40,
                source: ShareSource::Printed
            }
        );
        assert_eq!(
            stale_label.key.ownership_basis,
            OwnershipBasis::Attributable {
                num: 49,
                den: 200,
                source: ShareSource::Registry
            }
        );
        assert_eq!(stale_label.basis_evidence.as_deref(), Some("W1-DR-01"));
        let after = at("2026Q2:YTD!E11");
        assert_eq!(raw_token(&after.claim.value), "1691");
        assert!(matches!(
            after.claim.value,
            PublicValue::Number {
                under_status: Some(StatusKind::Disposed),
                ..
            }
        ));
        let mut reversed = reads.clone();
        reversed.reverse();
        let reordered = build_claims(&p, &sources, &reversed);
        assert_eq!(reordered.digest, all.digest);
        assert_eq!(reordered.summary_json(), summary);
        println!("{summary}");
    }
}
