//! Cross-crate Claim comparisons across Mine Stats reporting vintages.

#[cfg(test)]
mod tests {
    use fmine_claim::{
        AdditivityClass, Claim, ConfidentialityTier, Representation, SpatialSupport,
    };
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
    use fmine_public::key::consistency::{VintageState, compare_vintages};
    use fmine_public::key::{
        KeyedCell, LineKey, NilReason, OwnershipBasis, Period, PeriodKind, PublicKey, PublicValue,
        Scope, ShareSource, Vintage,
    };

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
            r.items[1].against_previous.expect("prior").state,
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
            r.items[2].against_previous.expect("prior").reason,
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
}
