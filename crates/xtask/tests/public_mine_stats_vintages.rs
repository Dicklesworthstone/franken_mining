//! Cross-crate AISC and Mine Stats Claim comparisons across reporting vintages.

#[cfg(test)]
mod tests {
    use fmine_claim::{
        AdditivityClass, Claim, ConfidentialityTier, Representation, SpatialSupport,
    };
    use fmine_core::anchor::SourceAnchor;
    use fmine_core::hash::Digest32;
    use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
    use fmine_public::key::consistency::{
        VintageScope, VintageState, compare_vintages, compare_vintages_with_disclosures,
    };
    use fmine_public::key::disclosure::VerifiedDisclosureRegistry;
    use fmine_public::key::{
        KeyedCell, LineKey, NilReason, OwnershipBasis, Period, PeriodKind, PublicKey, PublicValue,
        Scope, ShareSource, Vintage, claims_digest,
    };
    use std::collections::{BTreeMap, BTreeSet};

    fn number(displayed: &str) -> PublicValue {
        PublicValue::Number {
            stored: displayed.into(),
            displayed: Ok(displayed.into()),
            under_status: None,
        }
    }

    fn observation(release: &str, cell: &str, value: PublicValue) -> KeyedCell {
        let key = PublicKey {
            issuer: "synthetic".into(),
            subject: "Site A".into(),
            valid_period: Period {
                year: 2030,
                kind: PeriodKind::Quarter(1),
            },
            reporting_vintage: Vintage {
                release: release.into(),
                file_sha256: "ab".repeat(32),
            },
            ownership_basis: OwnershipBasis::Consolidated100,
            scope: Scope::Site,
            unit: "usd_per_oz".into(),
            line: LineKey {
                label: "summary:aisc_usd_per_oz".into(),
                occurrence: 0,
            },
        };
        let file = format!("synthetic-{release}.xlsx");
        let locator = format!("{file}:QTD!{cell}");
        let claim = Claim::new(
            "aisc",
            value,
            &key.canonical_tag(&locator),
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::IntensiveWeighted,
            EvidenceClass::ISSUER,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            ConfidentialityTier::Public,
            vec![],
            vec![],
            vec![SourceAnchor::new(&file, [0xab; 32], format!("QTD!{cell}"))],
        );
        KeyedCell {
            key,
            claim,
            locator,
            printed_basis: OwnershipBasis::Consolidated100,
            basis_evidence: None,
            unit_evidence: None,
            status_evidence: None,
            label_as_printed: None,
        }
    }

    #[test]
    fn exact_values_match_after_columns_move() {
        let a = observation("2030Q1", "G23", number("1479"));
        let b = observation("2031Q1", "N22", number("1479.0"));
        let r = compare_vintages(&[a, b]);
        assert_eq!(r.series, 1);
        assert_eq!(r.totals.comparisons, 1);
        assert_eq!(r.totals.states[&VintageState::Unchanged], 1);
        assert!(r.items.is_empty());
    }

    #[test]
    fn redundant_cost_section_unit_preserves_canonical_keys() {
        let mut first = observation("2030Q1", "B33", number("87"));
        first.key.line.label = "site:costs per ounce:royalties".into();
        let mut later = observation("2031Q1", "M41", number("87"));
        later.key.line.label = "site:costs per ounce ($/oz):royalties".into();
        let r = compare_vintages(&[first.clone(), later.clone()]);
        assert_eq!(r.series, 2, "original keys remain distinct");
        assert_eq!(r.candidate_groups, 1);
        assert_eq!(r.totals.states[&VintageState::Unchanged], 1);
        later.key.unit = "usd_per_lb".into();
        assert_eq!(
            compare_vintages(&[first.clone(), later.clone()])
                .totals
                .comparisons,
            0
        );
        later.key.unit = "usd_per_oz".into();
        later.key.line.label = "site:costs per ounce (adjusted):royalties".into();
        assert_eq!(compare_vintages(&[first, later]).totals.comparisons, 0);
    }

    #[test]
    fn first_and_previous_show_reversion_separately() {
        let claims = vec![
            observation("2030Q1", "G23", number("1479")),
            observation("2031Q1", "N22", number("1474")),
            observation("2032Q1", "I24", number("1479")),
        ];
        let r = compare_vintages(&claims);
        assert_eq!(r.items.len(), 2);
        assert_eq!(r.items[0].against_first.reason, Some("value"));
        assert_eq!(r.items[1].against_first.state, VintageState::Unchanged);
        assert_eq!(
            r.items[1].against_previous.as_ref().expect("prior").state,
            VintageState::ChangedWithoutRegisteredExplanation
        );
        let json = r.to_json();
        assert!(json.contains("synthetic-2030Q1.xlsx:QTD!G23"));
        assert!(json.contains("synthetic-2031Q1.xlsx:QTD!N22"));
        assert!(json.contains(&"ab".repeat(32)));
        assert!(json.contains("\"disclosure_check\":\"not_performed\""));
        assert!(json.contains("\"rounding_only\":null"));
    }

    #[test]
    fn token_classes_and_glyphs_stay_distinct() {
        let nil = |token: &str| PublicValue::Nil {
            token: token.into(),
            reason: NilReason::Unknown,
        };
        let r = compare_vintages(&[
            observation("2030Q1", "A1", nil("—")),
            observation("2031Q1", "A2", nil("- ")),
            observation(
                "2032Q1",
                "A3",
                PublicValue::NotApplicable {
                    token: "n/a".into(),
                },
            ),
            observation("2033Q1", "A4", PublicValue::Missing),
        ]);
        assert_eq!(r.items.len(), 3);
        assert_eq!(r.items[0].against_first.state, VintageState::Unchanged);
        assert!(r.items[0].against_first.token_glyph_changed);
        assert_eq!(r.items[1].against_first.reason, Some("token"));
        assert_eq!(
            r.items[2].against_previous.as_ref().expect("prior").reason,
            Some("token")
        );
    }

    #[test]
    fn duplicate_release_is_conflict_and_keeps_every_assertion() {
        let mut a = observation("2030Q1", "A1", number("100"));
        let mut b = observation("2030Q1", "B1", number("101"));
        a.key.reporting_vintage.file_sha256 = "aa".repeat(32);
        b.key.reporting_vintage.file_sha256 = "bb".repeat(32);
        let r = compare_vintages(&[a, b, observation("2031Q1", "C1", number("100"))]);
        assert_eq!(r.totals.states[&VintageState::Conflict], 2);
        assert_eq!(r.items[0].current.len(), 2);
        assert_eq!(r.items[1].first.len(), 2);
        assert_eq!(
            r.items[1].against_first.reason,
            Some("ambiguous_same_release")
        );
    }

    #[test]
    fn basis_change_and_region_map_are_not_numeric_restatements() {
        let a = observation("2030Q1", "A1", number("100"));
        let mut b = observation("2031Q1", "B1", number("50"));
        b.key.ownership_basis = OwnershipBasis::Attributable {
            num: 1,
            den: 2,
            source: ShareSource::Printed,
        };
        let r = compare_vintages(&[a.clone(), b]);
        assert_eq!(r.series, 2);
        assert_eq!(r.items[0].against_first.reason, Some("ownership_basis"));
        let mut region0 = a;
        region0.key.scope = Scope::Region {
            map_version: "before".into(),
        };
        let mut region1 = observation("2031Q1", "B1", number("100"));
        region1.key.scope = Scope::Region {
            map_version: "after".into(),
        };
        let r = compare_vintages(&[region0, region1]);
        assert_eq!(r.items[0].against_first.state, VintageState::NotComparable);
        assert_eq!(r.items[0].against_first.reason, Some("region_map"));
    }

    #[test]
    fn registered_label_conflict_and_display_refusal_are_visible_even_once() {
        let mut c = observation("2030Q1", "A1", number("100"));
        c.basis_evidence = Some("synthetic-label-conflict".into());
        c.printed_basis = OwnershipBasis::Attributable {
            num: 19,
            den: 40,
            source: ShareSource::Printed,
        };
        c.key.ownership_basis = OwnershipBasis::Attributable {
            num: 49,
            den: 200,
            source: ShareSource::Registry,
        };
        let r = compare_vintages(&[c]);
        assert_eq!(
            r.items[0].against_first.reason,
            Some("registered_basis_correction")
        );
        let r = compare_vintages(&[observation(
            "2030Q1",
            "B1",
            PublicValue::Number {
                stored: "1.1".into(),
                displayed: Err("general_non_integer"),
                under_status: None,
            },
        )]);
        assert_eq!(r.items[0].against_first.reason, Some("display_refused"));
        assert_eq!(r.totals.appearances, 1);
    }

    #[test]
    fn aisc_registered_basis_evidence_does_not_invent_a_printed_conflict() {
        let mut claims = [
            observation("2030Q1", "J18", number("70")),
            observation("2031Q1", "I17", number("70")),
        ];
        for cell in &mut claims {
            cell.printed_basis = OwnershipBasis::Unprinted;
            cell.key.ownership_basis = OwnershipBasis::Attributable {
                num: 1,
                den: 2,
                source: ShareSource::Registry,
            };
            cell.basis_evidence = Some("synthetic-share; attributable basis label".into());
        }
        let report = compare_vintages_with_disclosures(&claims, VintageScope::Aisc, None);
        assert_eq!(report.totals.states[&VintageState::Unchanged], 1);
        assert!(report.items.is_empty());
        assert_eq!(report.input_claims_digest, claims_digest(&claims));
        assert!(report.to_json().contains("\"scope\":\"aisc\""));
    }

    #[test]
    fn file_order_does_not_change_any_finding() {
        let mut claims = vec![
            observation("2032Q1", "B1", number("8")),
            observation("2030Q1", "D2", number("3")),
            observation("2031Q1", "G4", number("9")),
            observation("2030Q1", "D2", number("4")),
        ];
        let expected = compare_vintages(&claims).to_json();
        claims.reverse();
        assert_eq!(compare_vintages(&claims).to_json(), expected);
        claims.rotate_left(1);
        assert_eq!(compare_vintages(&claims).to_json(), expected);
    }

    #[test]
    fn displayed_equality_never_substitutes_stored_precision() {
        let a = observation(
            "2030Q1",
            "A1",
            PublicValue::Number {
                stored: "100.49".into(),
                displayed: Ok("100".into()),
                under_status: None,
            },
        );
        let b = observation("2031Q1", "B1", number("100"));
        assert_eq!(
            compare_vintages(&[a, b]).totals.states[&VintageState::Unchanged],
            1
        );
        let r = compare_vintages(&[
            observation("2030Q1", "A1", number("9007199254740992")),
            observation("2031Q1", "B1", number("9007199254740993")),
        ]);
        assert_eq!(r.items[0].against_first.reason, Some("value"));
        let empty = compare_vintages(&[]);
        assert_eq!(empty.totals.appearances, 0);
        assert!(empty.items.is_empty());
    }

    #[test]
    fn percentages_and_formatted_zero_compare_in_the_declared_unit() {
        let percent = |release: &str, stored: &str, displayed: &str| {
            let mut c = observation(
                release,
                "A1",
                PublicValue::Number {
                    stored: stored.into(),
                    displayed: Ok(displayed.into()),
                    under_status: None,
                },
            );
            c.key.unit = "mass_fraction".into();
            c
        };
        let a = percent("2030Q1", "0.8004", "80.0%");
        let r = compare_vintages(&[a.clone(), percent("2031Q1", "0.8", "0.8")]);
        assert_eq!(r.totals.states[&VintageState::Unchanged], 1);
        let r = compare_vintages(&[a.clone(), percent("2031Q1", "80", "80")]);
        assert_eq!(r.items[0].against_first.reason, Some("value"));
        let mut ambiguous = a;
        ambiguous.key.unit = "unknown".into();
        let r = compare_vintages(&[ambiguous]);
        assert_eq!(
            r.items[0].against_first.reason,
            Some("display_unit_ambiguous")
        );
        let zero = observation(
            "2030Q1",
            "A1",
            PublicValue::Number {
                stored: "0".into(),
                displayed: Ok("-".into()),
                under_status: None,
            },
        );
        let r = compare_vintages(&[zero, observation("2031Q1", "A2", number("0.00"))]);
        assert_eq!(r.totals.states[&VintageState::Unchanged], 1);
        let not_zero = observation(
            "2030Q1",
            "A1",
            PublicValue::Number {
                stored: "5".into(),
                displayed: Ok("-".into()),
                under_status: None,
            },
        );
        let r = compare_vintages(&[not_zero]);
        assert_eq!(r.items[0].against_first.reason, Some("display_not_numeric"));
    }

    /// FACT: scripts/oracle/vintage_census.py independently reads the visible OOXML table cells and
    /// reports 5,819 keys, 2,980 multi-vintage series, 3,722 later appearances and zero stored changes.
    #[test]
    #[ignore = "needs the registered Barrick AISC workbooks under corpus_of_input_docs/"]
    fn real_aisc_vintages_match_independent_census() {
        use fmine_public::aisc::{key_aisc_operands, ledger_for_table, locate_table};
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let profile =
            fmine_public::sources::load_issuer_profile(&root, "barrick").expect("profile");
        let sources =
            fmine_public::load_registered_sources(&profile, &root).expect("verified sources");
        let mut instances = Vec::new();
        // The visible table ledger supplies canonical source operands. No one-cell search or missing-fact
        // explanation is needed to compare those operands; this test does not score identity transitions.
        for source in sources
            .iter()
            .filter(|s| s.kind == fmine_public::sources::SourceKind::AiscReconciliation)
        {
            for sheet in &source.workbook.sheets {
                if let Some(table) = locate_table(sheet, &profile.subjects).expect("table lookup") {
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
            }
        }
        let index = key_aisc_operands(&profile, &sources, &instances);
        assert!(index.refusals.is_empty(), "{:?}", index.refusals);
        let claims: Vec<_> = index.cells.values().cloned().collect();
        let report = compare_vintages_with_disclosures(&claims, VintageScope::Aisc, None);
        println!(
            "AISC canonical vintage census: series={} multi_vintage={} totals={:?}",
            report.series, report.multi_vintage, report.totals
        );
        assert_eq!(report.series, 5_819);
        assert_eq!(report.multi_vintage, 2_980);
        assert_eq!(claims.len(), 9_541);
        assert_eq!(report.totals.comparisons, 3_722);
        assert_eq!(report.totals.states[&VintageState::Unchanged], 3_722);
        assert_eq!(report.totals.appearances, claims.len());
        assert_eq!(report.totals.states.values().sum::<usize>(), claims.len());
        for changed in [
            VintageState::ChangedWithoutRegisteredExplanation,
            VintageState::RestatedDisclosed,
            VintageState::RestatedSilent,
            VintageState::Conflict,
        ] {
            assert_eq!(
                report.totals.states.get(&changed).copied().unwrap_or(0),
                0,
                "{changed:?}"
            );
        }
        for (first, later) in [
            (
                "Barrick_Q2_2025_AISC.xlsx:YTD Q2 2025!J18",
                "Barrick_Q2_2026_AISC.xlsx:YTD Q2 2025!I17",
            ),
            (
                "Barrick_Q3_2024_AISC.xlsx:Q2 2024!O11",
                "Barrick_Q2_2025_AISC.xlsx:Q2 2024!O11",
            ),
        ] {
            let pair = compare_vintages_with_disclosures(
                &[index.cells[first].clone(), index.cells[later].clone()],
                VintageScope::Aisc,
                None,
            );
            assert_eq!(pair.totals.states[&VintageState::Unchanged], 1);
        }
        // FACT: the independent visible-OOXML census includes 359 co-product rate cells absent from the
        // identity-operand population. They contribute 249 series and 110 later appearances, with no
        // by-product alias. Their source values were compared independently of this canonical adapter.
        let co_product: Vec<_> = claims
            .iter()
            .filter(|c| c.key.line.label == "total cash costs per ounce (on a co-product basis)")
            .cloned()
            .collect();
        assert_eq!(co_product.len(), 359);
        assert!(co_product.iter().all(|c| c.key.unit == "usd_per_oz"));
        assert_eq!(
            co_product
                .iter()
                .filter(|c| matches!(
                    c.claim.value,
                    PublicValue::Number {
                        displayed: Ok(_),
                        ..
                    }
                ))
                .count(),
            352
        );
        let subset = compare_vintages_with_disclosures(&co_product, VintageScope::Aisc, None);
        assert_eq!(
            (
                subset.series,
                subset.multi_vintage,
                subset.totals.comparisons
            ),
            (249, 96, 110)
        );
        assert_eq!(subset.totals.states[&VintageState::Unchanged], 110);
        assert!(subset.items.is_empty());
        let withheld: BTreeSet<_> = co_product
            .iter()
            .filter(|c| {
                matches!(
                    c.claim.value,
                    PublicValue::Nil {
                        reason: NilReason::Withheld,
                        ..
                    }
                )
            })
            .map(|c| c.locator.as_str())
            .collect();
        assert_eq!(
            withheld,
            BTreeSet::from([
                "Barrick_Q1_2025_AISC.xlsx:Q1 2025!N22",
                "Barrick_Q2_2025_AISC.xlsx:Q1 2025!N22",
                "Barrick_Q2_2025_AISC.xlsx:Q2 2025!N22",
                "Barrick_Q2_2025_AISC.xlsx:YTD Q2 2025!N22",
                "Barrick_Q3_2025_AISC.xlsx:Q2 2025!L22",
                "Barrick_Q3_2025_AISC.xlsx:Q3 2025!L22",
                "Barrick_Q3_2025_AISC.xlsx:YTD Q3 2025!L22",
            ])
        );
        // FACT: Q1 2025!C22 stores this long decimal under a whole-number format (numFmtId 164).
        assert!(
            matches!(&index.cells["Barrick_Q1_2025_AISC.xlsx:Q1 2025!C22"].claim.value,
            PublicValue::Number { stored, displayed: Ok(displayed), .. }
                if stored == "1466.1763085877856" && displayed == "1466")
        );
        assert!(
            matches!(&index.cells["Barrick_Q1_2025_AISC.xlsx:Q1 2024!L22"].claim.value,
            PublicValue::Number { stored, .. } if stored == "0")
        );
    }

    /// FACT: E474, W1-DR-09 and W1's T16 population predate this comparator. W1 lists passes/failures:
    /// 6,391 + 9 = 6,400 comparisons, excluding 30 region-map changes. Our additional display/basis states
    /// remain diagnostics; this test does not claim that GATE-BULL-01 part 3 passed.
    #[test]
    #[ignore = "needs the registered Barrick Mine Stats workbooks under corpus_of_input_docs/"]
    fn real_mine_stats_vintages_keep_e474_and_basis_conflicts() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace")
            .to_path_buf();
        let profile =
            fmine_public::sources::load_issuer_profile(&root, "barrick").expect("profile");
        let sources =
            fmine_public::load_registered_sources(&profile, &root).expect("verified sources");
        let vocabulary = profile.mine_stats_labels.iter().cloned().collect();
        let mut claims = Vec::new();
        for source in sources
            .iter()
            .filter(|s| s.kind == fmine_public::sources::SourceKind::MineStats)
        {
            let read = fmine_public::mine_stats::read_mine_stats(
                &source.workbook,
                &source.quarter,
                Some(&vocabulary),
            )
            .expect("read");
            assert!(read.refusals.is_empty(), "{:?}", read.refusals);
            for value in &read.values {
                claims.push(
                    fmine_public::key::from_mine_stats(
                        &profile,
                        &source.workbook,
                        &source.quarter,
                        value,
                    )
                    .expect("canonical source-backed Claim"),
                );
            }
        }
        let r = compare_vintages(&claims);
        let registry = VerifiedDisclosureRegistry::verify(&profile, &sources, &claims);
        assert!(registry.is_complete(), "{}", registry.to_json());
        let assessed =
            compare_vintages_with_disclosures(&claims, VintageScope::MineStats, Some(&registry));
        assert_eq!(assessed.totals.states[&VintageState::RestatedSilent], 9);
        assert!(
            !assessed
                .totals
                .states
                .contains_key(&VintageState::ChangedWithoutRegisteredExplanation)
        );
        let region_items: Vec<_> = assessed
            .items
            .iter()
            .filter(|item| item.against_first.reason == Some("region_map"))
            .collect();
        assert_eq!(region_items.len(), 30);
        for item in region_items {
            assert_eq!(item.against_first.state, VintageState::NotComparable);
            assert!(
                item.against_first
                    .disclosures
                    .iter()
                    .any(|note| note.evidence == "E461")
            );
            assert!(
                item.against_previous
                    .as_ref()
                    .expect("region predecessor")
                    .disclosures
                    .iter()
                    .any(|note| note.evidence == "E461")
            );
        }
        let e474_assessed = assessed
            .items
            .iter()
            .find(|item| {
                item.current
                    .iter()
                    .any(|cell| cell.locator == "Barrick_Q1_2025_Mine_Stats.xlsx:QTD!N22")
            })
            .expect("E474 with checked registry");
        assert_eq!(
            e474_assessed.against_first.state,
            VintageState::RestatedSilent
        );
        assert!(e474_assessed.against_first.disclosures.is_empty());
        assert_eq!(assessed.input_claims_digest, r.input_claims_digest);
        println!(
            "Mine Stats verified disclosure census: {:?}",
            assessed.totals
        );
        // FACT: these 20 Pueblo Viejo pairs have the same metric/unit/value in the independent W1 read.
        // Only the section's redundant ($/oz) changes; never rewrite the canonical keys to make them match.
        let by_locator: BTreeMap<_, _> = claims.iter().map(|c| (c.locator.as_str(), c)).collect();
        let section_pairs = [
            (
                "Barrick_Q4_2024_Mine_Stats.xlsx",
                "Barrick_Q4_2025_Mine_Stats.xlsx",
                &[
                    ("B33", "M41"),
                    ("B34", "M42"),
                    ("B35", "M43"),
                    ("B36", "M44"),
                    ("B82", "M102"),
                    ("B83", "M103"),
                    ("B84", "M104"),
                    ("B85", "M105"),
                ][..],
            ),
            (
                "Barrick_Q1_2025_Mine_Stats.xlsx",
                "Barrick_Q1_2026_Mine_Stats.xlsx",
                &[
                    ("B33", "M41"),
                    ("B34", "M42"),
                    ("B35", "M43"),
                    ("B36", "M44"),
                ][..],
            ),
            (
                "Barrick_Q2_2025_Mine_Stats.xlsx",
                "Barrick_Q2_2026_Mine_Stats.xlsx",
                &[
                    ("B33", "M43"),
                    ("B34", "M44"),
                    ("B35", "M45"),
                    ("B36", "M46"),
                    ("B82", "M106"),
                    ("B83", "M107"),
                    ("B84", "M108"),
                    ("B85", "M109"),
                ][..],
            ),
        ];
        for (from_file, to_file, cells) in section_pairs {
            for (from_cell, to_cell) in cells {
                let first = by_locator[format!("{from_file}:Minestats2!{from_cell}").as_str()];
                let later = by_locator[format!("{to_file}:Minestats1!{to_cell}").as_str()];
                assert_eq!(first.key.subject, "Pueblo Viejo");
                assert_ne!(first.key.series_key(), later.key.series_key());
                let pair = compare_vintages(&[first.clone(), later.clone()]);
                assert_eq!(pair.candidate_groups, 1);
                assert_eq!(pair.totals.states[&VintageState::Unchanged], 1);
            }
        }
        let e474 = r
            .items
            .iter()
            .find(|i| {
                i.current
                    .iter()
                    .any(|c| c.locator == "Barrick_Q1_2025_Mine_Stats.xlsx:QTD!N22")
            })
            .expect("E474 later cell");
        assert!(
            e474.first
                .iter()
                .any(|c| c.locator == "Barrick_Q1_2024_Mine_Stats.xlsx:QTD!G23")
        );
        assert_eq!(
            e474.against_first.state,
            VintageState::ChangedWithoutRegisteredExplanation
        );
        assert_eq!(e474.against_first.reason, Some("value"));
        let changed: BTreeSet<_> = r
            .items
            .iter()
            .filter(|i| i.against_first.state == VintageState::ChangedWithoutRegisteredExplanation)
            .map(|i| i.first[0].locator.as_str())
            .collect();
        // FACT: the original locators of E474 and W1-DR-09's eight changes were captured before this code.
        assert_eq!(
            changed,
            BTreeSet::from([
                "Barrick_Q1_2024_Mine_Stats.xlsx:QTD!G23",
                "Barrick_Q3_2024_Mine_Stats.xlsx:Minestats3!F57",
                "Barrick_Q2_2025_Mine_Stats.xlsx:Minestats1!F107",
                "Barrick_Q2_2025_Mine_Stats.xlsx:Minestats1!F108",
                "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F14",
                "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F27",
                "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F63",
                "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F76",
                "Barrick_Q2_2025_Mine_Stats.xlsx:Minestats3!B8",
            ])
        );
        let mut region_changes = BTreeMap::<&str, usize>::new();
        for item in r
            .items
            .iter()
            .filter(|i| !i.previous.is_empty() && i.against_first.reason == Some("region_map"))
        {
            *region_changes
                .entry(&item.current[0].key.reporting_vintage.release)
                .or_default() += 1;
        }
        // W1 §5.1 lists passes/failures, not total/failures; §13 explicitly states 6,400 cells.
        for (release, passed, changed, regions) in [
            ("2025Q1", 645, 1, 0),
            ("2025Q2", 1_288, 4, 0),
            ("2025Q3", 1_291, 1, 0),
            ("2025Q4", 1_268, 0, 12),
            ("2026Q1", 634, 0, 6),
            ("2026Q2", 1_265, 3, 12),
        ] {
            let counts = &r.by_vintage[release];
            assert_eq!(
                region_changes.get(release).copied().unwrap_or(0),
                regions,
                "{release} region boundary"
            );
            assert_eq!(
                counts.comparisons,
                passed + changed + regions,
                "{release} T16 population plus explicit region changes"
            );
            assert_eq!(
                counts
                    .states
                    .get(&VintageState::ChangedWithoutRegisteredExplanation)
                    .copied()
                    .unwrap_or(0),
                changed,
                "{release} independently recorded changes"
            );
        }
        assert_eq!(region_changes.values().sum::<usize>(), 30);
        assert_eq!(r.totals.comparisons - 30, 6_400);
        assert!(r.items.iter().any(|i| {
            i.against_first.reason == Some("registered_basis_correction")
                && i.current.iter().any(|c| {
                    c.key.subject == "Porgera" && c.basis_evidence.as_deref() == Some("W1-DR-01")
                })
        }));
        assert!(
            r.items
                .iter()
                .any(|i| i.against_first.reason == Some("region_map"))
        );
        assert_eq!(
            r.totals.appearances,
            r.totals.states.values().sum::<usize>()
        );
        let mut reversed = claims;
        reversed.reverse();
        assert_eq!(r.to_json(), compare_vintages(&reversed).to_json());
        println!(
            "mine_stats_vintages: series={} candidate_groups={} multi_vintage={} totals={:?} independent_census=false",
            r.series, r.candidate_groups, r.multi_vintage, r.totals
        );
        let mut reasons = BTreeMap::<&str, usize>::new();
        for i in &r.items {
            if let Some(reason) = i.against_first.reason {
                *reasons.entry(reason).or_default() += 1;
            }
        }
        println!("finding reasons: {reasons:?}");
        for i in &r.items {
            if i.against_first.state == VintageState::ChangedWithoutRegisteredExplanation {
                println!("{i:?}");
            }
        }
    }

    /// Synthetic notes still pass through the production byte, quote, chronology and scope verifier.
    fn note_fixture(
        release: &str,
        key: &PublicKey,
    ) -> (
        fmine_public::sources::IssuerProfile,
        Vec<fmine_public::LoadedSource>,
    ) {
        use fmine_io::xlsx::{MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx};
        use fmine_public::sources::{
            RegisteredSource, RestatementNoteRow, RestatementScopeRow, SourceKind,
        };
        let file = "synthetic-disclosure.xlsx";
        let quote = "Synthetic comparative information has been restated.";
        let bytes = build_minimal_xlsx(&[MinimalSheet {
            name: "Notes".into(),
            hidden: false,
            cells: vec![("A1".into(), MinimalCell::Text(quote.into()))],
        }]);
        let workbook = Workbook::from_bytes(file, &bytes).expect("synthetic note workbook");
        let digest = Digest32(workbook.sha256).to_hex();
        let source = fmine_public::LoadedSource {
            file_name: file.into(),
            kind: SourceKind::MineStats,
            quarter: release.into(),
            url: "https://example.invalid/synthetic-disclosure.xlsx".into(),
            url_basis: "synthetic".into(),
            bytes: bytes.len() as u64,
            census: workbook.cell_census(),
            workbook,
        };
        let mut profile = fmine_public::synthetic::synthetic_profile();
        if !profile.subjects.contains(&key.subject) {
            profile.subjects.push(key.subject.clone());
        }
        profile.sources = vec![RegisteredSource {
            file_name: file.into(),
            kind: source.kind,
            quarter: release.into(),
            url: source.url.clone(),
            url_basis: source.url_basis.clone(),
            bytes: source.bytes,
            sha256_hex: digest.clone(),
        }];
        profile.restatement_notes = vec![RestatementNoteRow {
            id: "synthetic-note".into(),
            file_name: file.into(),
            locator: "Notes!A1".into(),
            quoted_text: quote.into(),
            from_quarter: release.into(),
            evidence: "SYNTHETIC-DISCLOSURE".into(),
            sha256: digest,
        }];
        profile.restatement_scopes = vec![RestatementScopeRow {
            note: "synthetic-note".into(),
            subject: key.subject.clone(),
            period_from: format!("{}Q1", key.valid_period.year),
            period_to: format!("{}Q4", key.valid_period.year),
            line: key.line.label.clone(),
            occurrence: key.line.occurrence,
        }];
        (profile, vec![source])
    }

    #[test]
    fn disclosures_respect_chronology_and_bind_both_comparisons() {
        let claims = [
            observation("2030Q1", "B1", number("100")),
            observation("2031Q1", "G2", number("110")),
            observation("2032Q1", "M3", number("120")),
        ];
        let (mut profile, sources) = note_fixture("2032Q1", &claims[2].key);
        let registry = VerifiedDisclosureRegistry::verify(&profile, &sources, &claims);
        assert!(registry.is_complete(), "{}", registry.to_json());
        let report =
            compare_vintages_with_disclosures(&claims, VintageScope::Aisc, Some(&registry));
        assert_eq!(
            report.items[0].against_first.state,
            VintageState::RestatedSilent
        );
        assert!(
            report.items[0].against_first.disclosures.is_empty(),
            "no future note"
        );
        assert_eq!(
            report.items[1].against_first.state,
            VintageState::RestatedDisclosed
        );
        assert_eq!(report.items[1].against_first.disclosures.len(), 1);
        let prior = report.items[1]
            .against_previous
            .as_ref()
            .expect("previous comparison");
        assert_eq!(prior.state, VintageState::RestatedDisclosed);
        assert_eq!(prior.disclosures, report.items[1].against_first.disclosures);
        assert_eq!(report.input_claims_digest, claims_digest(&claims));
        assert_eq!(report.disclosure_registry_digest, Some(registry.digest()));
        let json = report.to_json();
        assert!(
            json.contains("Value differs between vintages; no note found in registered documents.")
        );
        assert!(json.contains("SYNTHETIC-DISCLOSURE"));
        assert!(json.contains("synthetic-disclosure.xlsx"));
        assert!(json.contains("\"rounding_only\":null"));
        let mut reordered = claims.clone();
        reordered.reverse();
        assert_eq!(
            json,
            compare_vintages_with_disclosures(&reordered, VintageScope::Aisc, Some(&registry))
                .to_json()
        );
        profile.restatement_notes.clear();
        profile.restatement_scopes.clear();
        let empty = VerifiedDisclosureRegistry::verify(&profile, &sources, &claims);
        assert!(empty.is_complete());
        let silent = compare_vintages_with_disclosures(&claims, VintageScope::Aisc, Some(&empty));
        assert_eq!(silent.totals.states[&VintageState::RestatedSilent], 2);
        assert_ne!(
            silent.disclosure_registry_digest,
            report.disclosure_registry_digest
        );
        assert_eq!(silent.input_claims_digest, report.input_claims_digest);
    }

    #[test]
    fn incomplete_registry_never_turns_missing_coverage_into_silence() {
        let claims = [
            observation("2030Q1", "A1", number("1")),
            observation("2031Q1", "A2", number("2")),
        ];
        let (mut profile, sources) = note_fixture("2031Q1", &claims[1].key);
        profile.restatement_notes[0].quoted_text = "This text is not in the source.".into();
        let invalid = VerifiedDisclosureRegistry::verify(&profile, &sources, &claims);
        assert!(!invalid.is_complete());
        let report =
            compare_vintages_with_disclosures(&claims, VintageScope::MineStats, Some(&invalid));
        assert_eq!(
            report.items[0].against_first.state,
            VintageState::ChangedWithoutRegisteredExplanation
        );
        assert!(report.items[0].against_first.disclosures.is_empty());
        assert!(
            report
                .to_json()
                .contains("\"disclosure_check\":\"incomplete_registry\"")
        );
        let (mut profile, sources) = note_fixture("2031Q1", &claims[1].key);
        let mut bad = profile.restatement_notes[0].clone();
        bad.id = "unverified-note".into();
        bad.locator = "Notes!A2".into();
        profile.restatement_notes.push(bad);
        let mut scope = profile.restatement_scopes[0].clone();
        scope.note = "unverified-note".into();
        profile.restatement_scopes.push(scope);
        let partial = VerifiedDisclosureRegistry::verify(&profile, &sources, &claims);
        assert!(!partial.is_complete());
        let disclosed =
            compare_vintages_with_disclosures(&claims, VintageScope::MineStats, Some(&partial));
        assert_eq!(
            disclosed.items[0].against_first.state,
            VintageState::RestatedDisclosed
        );
        assert_eq!(disclosed.items[0].against_first.disclosures.len(), 1);
    }

    #[test]
    fn disclosure_scope_uses_exact_keys_and_whole_periods() {
        let bound = observation("2030Q1", "Z1", number("10"));
        for dimension in ["issuer", "subject", "period", "line", "occurrence"] {
            let mut claims = vec![
                observation("2030Q1", "A1", number("1")),
                observation("2031Q1", "A2", number("2")),
            ];
            for cell in &mut claims {
                match dimension {
                    "issuer" => cell.key.issuer = "another_issuer".into(),
                    "subject" => cell.key.subject = "Another site".into(),
                    "period" => cell.key.valid_period.year = 2031,
                    "line" => cell.key.line.label = "another metric".into(),
                    "occurrence" => cell.key.line.occurrence = 1,
                    _ => unreachable!(),
                }
            }
            claims.push(bound.clone());
            let (profile, sources) = note_fixture("2031Q1", &bound.key);
            let registry = VerifiedDisclosureRegistry::verify(&profile, &sources, &claims);
            assert!(
                registry.is_complete(),
                "{dimension}: {}",
                registry.to_json()
            );
            let report = compare_vintages_with_disclosures(
                &claims,
                VintageScope::MineStats,
                Some(&registry),
            );
            assert_eq!(
                report.items[0].against_first.state,
                VintageState::RestatedSilent,
                "{dimension}"
            );
            assert!(
                report.items[0].against_first.disclosures.is_empty(),
                "{dimension}"
            );
        }
        let mut claims = [
            observation("2030Q2", "A1", number("1")),
            observation("2031Q2", "A2", number("2")),
            bound,
        ];
        claims[0].key.valid_period.kind = PeriodKind::H1;
        claims[1].key.valid_period.kind = PeriodKind::H1;
        claims[2].key.valid_period.kind = PeriodKind::Quarter(2);
        let (mut profile, sources) = note_fixture("2031Q1", &claims[2].key);
        profile.restatement_scopes[0].period_from = "2030Q2".into();
        profile.restatement_scopes[0].period_to = "2030Q2".into();
        let registry = VerifiedDisclosureRegistry::verify(&profile, &sources, &claims);
        assert!(registry.is_complete(), "{}", registry.to_json());
        let report =
            compare_vintages_with_disclosures(&claims, VintageScope::MineStats, Some(&registry));
        assert_eq!(
            report.items[0].against_first.state,
            VintageState::RestatedSilent,
            "Q2 coverage cannot cover all of H1"
        );
    }

    #[test]
    fn disclosure_preserves_region_exclusions_glyphs_and_reversions() {
        let mut claims = [
            observation("2030Q1", "A1", number("100")),
            observation("2031Q1", "A2", number("110")),
        ];
        claims[0].key.scope = Scope::Region {
            map_version: "old".into(),
        };
        claims[1].key.scope = Scope::Region {
            map_version: "new".into(),
        };
        let (mut profile, sources) = note_fixture("2031Q1", &claims[1].key);
        let registry = VerifiedDisclosureRegistry::verify(&profile, &sources, &claims);
        assert!(registry.is_complete(), "{}", registry.to_json());
        let report =
            compare_vintages_with_disclosures(&claims, VintageScope::MineStats, Some(&registry));
        assert_eq!(
            report.items[0].against_first.state,
            VintageState::NotComparable
        );
        assert_eq!(report.items[0].against_first.reason, Some("region_map"));
        assert_eq!(report.items[0].against_first.disclosures.len(), 1);
        profile.restatement_notes.clear();
        profile.restatement_scopes.clear();
        let empty = VerifiedDisclosureRegistry::verify(&profile, &sources, &claims);
        let without_note =
            compare_vintages_with_disclosures(&claims, VintageScope::MineStats, Some(&empty));
        assert_eq!(
            without_note.items[0].against_first.state,
            VintageState::NotComparable
        );
        assert!(without_note.items[0].against_first.disclosures.is_empty());
        let nil = |token: &str| PublicValue::Nil {
            token: token.into(),
            reason: NilReason::Unknown,
        };
        let claims = [
            observation("2030Q1", "A1", nil("—")),
            observation("2031Q1", "A2", nil("-")),
            observation(
                "2032Q1",
                "A3",
                PublicValue::NotApplicable {
                    token: "n/a".into(),
                },
            ),
        ];
        let (profile, sources) = note_fixture("2031Q1", &claims[1].key);
        let registry = VerifiedDisclosureRegistry::verify(&profile, &sources, &claims);
        let report =
            compare_vintages_with_disclosures(&claims, VintageScope::MineStats, Some(&registry));
        assert_eq!(report.items[0].against_first.state, VintageState::Unchanged);
        assert!(report.items[0].against_first.token_glyph_changed);
        assert!(report.items[0].against_first.disclosures.is_empty());
        assert_eq!(
            report.items[1].against_first.state,
            VintageState::RestatedDisclosed
        );
        assert_eq!(report.items[1].against_first.reason, Some("token"));
        let claims = [
            observation("2030Q1", "A1", number("100")),
            observation("2031Q1", "A2", number("110")),
            observation("2032Q1", "A3", number("100")),
        ];
        let (profile, sources) = note_fixture("2032Q1", &claims[2].key);
        let registry = VerifiedDisclosureRegistry::verify(&profile, &sources, &claims);
        let report =
            compare_vintages_with_disclosures(&claims, VintageScope::MineStats, Some(&registry));
        assert_eq!(report.items[1].against_first.state, VintageState::Unchanged);
        assert!(report.items[1].against_first.disclosures.is_empty());
        assert_eq!(
            report.items[1]
                .against_previous
                .as_ref()
                .expect("previous")
                .state,
            VintageState::RestatedDisclosed
        );
    }
}
