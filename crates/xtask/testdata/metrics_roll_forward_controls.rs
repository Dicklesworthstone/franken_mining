    use super::*;
    use fmine_registry::table_identities::Term;
    use fmine_registry::transcriptions::Locator;

    fn document() -> DocumentRow {
        DocumentRow {
            file: "Report.pdf".into(),
            dir: "documents".into(),
            kind: "annual_report".into(),
            url: "https://example.test/report.pdf".into(),
            url_basis: "synthetic".into(),
            published_at: "2031-02-01".into(),
            published_at_basis: "synthetic".into(),
            pages: 2,
            bytes: 10,
            sha256: "a".repeat(64),
        }
    }

    fn reading(quantity: &str, token: &str, period: &str) -> Accepted {
        Accepted {
            locator: Locator {
                doc: "Report.pdf".into(),
                page: 1,
                table: "Synthetic reserves".into(),
                row: quantity.into(),
                col: period.into(),
            },
            doc_sha256: document().sha256,
            token: token.into(),
            value: parse_token(token).unwrap(),
            quantity: quantity.into(),
            unit: "moz".into(),
            basis: "attributable;contained".into(),
            period: period.into(),
            readers: vec!["reader_a".into(), "reader_b".into()],
        }
    }

    fn sum(id: &str, total: &str, period: &str, terms: &[(&str, &str, i8)]) -> TableIdentity {
        TableIdentity {
            id: id.into(),
            doc: "Report.pdf".into(),
            family: "reserve_sum".into(),
            kind: IdentityKind::Sum,
            total: total.into(),
            total_period: period.into(),
            unit: "moz".into(),
            basis: "attributable;contained".into(),
            construction: "rounded_once".into(),
            convention: "closed".into(),
            replacement: None,
            terms: terms
                .iter()
                .map(|(quantity, period, sign)| Term {
                    quantity: (*quantity).into(),
                    period: (*period).into(),
                    sign: *sign,
                    role: TermRole::Member,
                })
                .collect(),
        }
    }

    fn registry(identities: Vec<TableIdentity>) -> TableIdentityRegistry {
        TableIdentityRegistry {
            issuer: "synthetic".into(),
            identities,
            reserve_rows: Vec::new(),
            geo_checks: Vec::new(),
            cutoffs: Vec::new(),
        }
    }

    fn roll_forward() -> (TableIdentityRegistry, Vec<Accepted>) {
        let mut definition = sum(
            "reserves",
            "close",
            "2030FY",
            &[
                ("open", "2029FY", 1),
                ("depletion", "2030FY", 1),
                ("transfer", "2030FY", -1),
            ],
        );
        definition.kind = IdentityKind::RollForward;
        for term in &mut definition.terms {
            term.role = if term.quantity == "open" {
                TermRole::Opening
            } else {
                TermRole::Move
            };
        }
        (
            registry(vec![definition]),
            vec![
                reading("open", "100", "2029FY"),
                reading("depletion", "(20)", "2030FY"),
                reading("transfer", "(5)", "2030FY"),
                reading("close", "85", "2030FY"),
            ],
        )
    }

    #[test]
    fn signed_rollforward_preserves_printed_signs_and_checks_its_witness() {
        let (registry, readings) = roll_forward();
        let report = assess(&registry, &readings, &[document()]);
        assert!(report.refused.is_empty());
        assert_eq!(report.components.len(), 1);
        assert!(matches!(
            report.components[0].component.outcome,
            xf::Outcome::Feasible { .. }
        ));
        assert!(
            report.components[0]
                .implied
                .iter()
                .all(|(_, range)| range.is_ok())
        );
        assert!(report.to_json().contains("\"checked\":true"));
        assert!(report.to_json().contains("\"token\":\"(20)\""));
        assert!(report.to_json().contains("\"gate_eligible\":false"));
    }

    #[test]
    fn sign_flip_produces_a_checked_conflict_and_all_minimum_correction_sets() {
        let (mut registry, readings) = roll_forward();
        registry.identities[0]
            .terms
            .iter_mut()
            .find(|t| t.quantity == "transfer")
            .unwrap()
            .sign = 1;
        let report = assess(&registry, &readings, &[document()]);
        let component = &report.components[0];
        assert!(matches!(
            component.component.outcome,
            xf::Outcome::Infeasible { .. }
        ));
        assert_eq!(component.certificate_digest.as_ref().unwrap().len(), 64);
        let Some(xf::BlameOutcome::Blamed(blame)) = &component.blame else {
            panic!("checked blame")
        };
        assert_eq!(blame.label, xf::CorrectionLabel::Minimum);
        assert!(
            blame.corrections.len() > 1,
            "one equation must not choose a unique corrected figure"
        );
        assert!(blame.corrections.iter().all(|set| set.cells.len() == 1));
    }

    #[test]
    fn explicit_multiyear_membership_gives_the_exact_copied_row_implied_interval() {
        // Invented source records with the E692-shaped arithmetic; this is not an authenticated issuer test.
        let definition = sum(
            "multi_year",
            "total",
            "2019-2024",
            &[
                ("a", "2019-2020", 1),
                ("b", "2021FY", 1),
                ("c", "2022FY", 1),
                ("d", "2023FY", 1),
                ("copied", "2024FY", 1),
            ],
        );
        let readings = vec![
            reading("total", "111", "2019-2024"),
            reading("a", "4.2", "2019-2020"),
            reading("b", "8.5", "2021FY"),
            reading("c", "13", "2022FY"),
            reading("d", "6.7", "2023FY"),
            reading("copied", "6.7", "2024FY"),
        ];
        let report = assess(&registry(vec![definition]), &readings, &[document()]);
        let Some(xf::BlameOutcome::Blamed(blame)) = &report.components[0].blame else {
            panic!("blame")
        };
        let target = readings.last().unwrap().locator.to_string();
        let correction = blame
            .corrections
            .iter()
            .find(|set| set.cells == [target.clone()])
            .unwrap();
        let range = correction.implied[0].1.as_ref().unwrap();
        assert_eq!(range.lo, Rat::new(1549, 20).unwrap());
        assert_eq!(range.hi, Rat::new(319, 4).unwrap());
        assert!(!range.lo_strict && !range.hi_strict);
        assert!(report.to_json().contains("\"period\":\"2019-2020\""));
    }

    #[test]
    fn shared_total_is_solved_jointly_when_each_sum_alone_is_feasible() {
        let a = sum(
            "left",
            "total",
            "2030FY",
            &[("a", "2030FY", 1), ("b", "2030FY", 1)],
        );
        let b = sum(
            "right",
            "total",
            "2030FY",
            &[("c", "2030FY", 1), ("d", "2030FY", 1)],
        );
        let readings = vec![
            reading("total", "1", "2030FY"),
            reading("a", "0.4", "2030FY"),
            reading("b", "0.1", "2030FY"),
            reading("c", "0.7", "2030FY"),
            reading("d", "0.7", "2030FY"),
        ];
        for definition in [&a, &b] {
            assert!(matches!(
                assess(
                    &registry(vec![definition.clone()]),
                    &readings,
                    &[document()]
                )
                .components[0]
                    .component
                    .outcome,
                xf::Outcome::Feasible { .. }
            ));
        }
        let report = assess(&registry(vec![a, b]), &readings, &[document()]);
        assert_eq!(report.components.len(), 1);
        assert_eq!(report.components[0].component.identities, ["left", "right"]);
        assert!(matches!(
            report.components[0].component.outcome,
            xf::Outcome::Infeasible { .. }
        ));
    }

    #[test]
    fn half_open_convention_excludes_a_closed_tie_witness() {
        let mut definition = sum("tie", "total", "2030FY", &[("member", "2030FY", 1)]);
        let readings = vec![
            reading("total", "2", "2030FY"),
            reading("member", "1", "2030FY"),
        ];
        let closed = assess(
            &registry(vec![definition.clone()]),
            &readings,
            &[document()],
        );
        assert!(matches!(
            closed.components[0].component.outcome,
            xf::Outcome::Feasible { .. }
        ));
        definition.convention = "half_up".into();
        let open = assess(&registry(vec![definition]), &readings, &[document()]);
        assert!(matches!(
            open.components[0].component.outcome,
            xf::Outcome::Infeasible { .. }
        ));
        assert_ne!(closed.digest(), open.digest());
    }

    #[test]
    fn missing_ambiguous_and_mislabeled_readings_are_retained_as_refused_identities() {
        let (registry, readings) = roll_forward();
        let mut cases = Vec::new();
        let mut missing = readings.clone();
        missing.pop();
        cases.push(missing);
        let mut ambiguous = readings.clone();
        ambiguous.push(readings[0].clone());
        cases.push(ambiguous);
        let mut unit = readings.clone();
        unit[0].unit = "koz".into();
        cases.push(unit);
        let mut period = readings.clone();
        period[0].period = "2028FY".into();
        cases.push(period);
        let mut basis = readings.clone();
        basis[0].basis = "100%;contained".into();
        cases.push(basis);
        let mut reader = readings.clone();
        reader[0].readers = vec!["reader_a".into(), "reader_a".into()];
        cases.push(reader);
        let mut hash = readings.clone();
        hash[0].doc_sha256 = "b".repeat(64);
        cases.push(hash);
        let mut value = readings.clone();
        value[0].value = parse_token("101").unwrap();
        cases.push(value);
        for case in cases {
            let report = assess(&registry, &case, &[document()]);
            assert!(report.components.is_empty());
            assert_eq!(report.refused.len(), 1);
            assert_eq!(report.refused[0].identity, "reserves");
            assert!(!report.refused[0].evidence.is_empty());
        }
    }

    #[test]
    fn colliding_display_locators_cannot_merge_distinct_operands_into_a_false_witness() {
        let definition = sum(
            "collision",
            "total",
            "2030FY",
            &[("first", "2030FY", 1), ("second", "2030FY", 1)],
        );
        let mut first = reading("first", "100", "2030FY");
        first.locator.row = "a/b".into();
        first.locator.col = "c".into();
        let mut second = reading("second", "3", "2030FY");
        second.locator.row = "a".into();
        second.locator.col = "b/c".into();
        assert_ne!(first.locator, second.locator);
        assert_eq!(first.locator.to_string(), second.locator.to_string());
        let report = assess(
            &registry(vec![definition]),
            &[first, second, reading("total", "6", "2030FY")],
            &[document()],
        );
        assert!(report.components.is_empty());
        assert_eq!(report.refused.len(), 1);
        assert!(
            report.refused[0]
                .reason
                .contains("ambiguous rendered locator")
        );
        assert_eq!(report.refused[0].evidence.len(), 3);
    }

    #[test]
    fn independent_table_survives_missing_members_and_permutations_are_byte_identical() {
        let (mut registry, mut readings) = roll_forward();
        registry.identities.push(sum(
            "other",
            "other_total",
            "2030FY",
            &[("other_part", "2030FY", 1)],
        ));
        readings.extend([
            reading("other_total", "9", "2030FY"),
            reading("other_part", "9", "2030FY"),
        ]);
        readings.retain(|a| a.quantity != "open");
        let first = assess(&registry, &readings, &[document()]);
        assert_eq!(first.refused.len(), 1);
        assert_eq!(first.components[0].component.identities, ["other"]);
        readings.reverse();
        for reading in &mut readings {
            reading.readers.reverse();
        }
        registry.identities.reverse();
        for identity in &mut registry.identities {
            identity.terms.reverse();
        }
        let reordered = assess(&registry, &readings, &[document()]);
        assert_eq!(first.to_json(), reordered.to_json());
        assert_eq!(first.digest(), reordered.digest());
    }

    #[test]
    fn filing_vintages_remain_separate_and_source_metadata_changes_the_digest() {
        let (mut registry, mut readings) = roll_forward();
        let mut later = document();
        later.file = "Later.pdf".into();
        later.sha256 = "b".repeat(64);
        later.published_at = "2032-02-01".into();
        later.kind = "investor_presentation".into();
        let mut definition = registry.identities[0].clone();
        definition.id = "later".into();
        definition.doc = later.file.clone();
        registry.identities.push(definition);
        let later_rows: Vec<_> = readings
            .iter()
            .map(|r| {
                let mut r = r.clone();
                r.locator.doc = later.file.clone();
                r.doc_sha256 = later.sha256.clone();
                if r.quantity == "close" {
                    r.token = "70".into();
                    r.value = parse_token("70").unwrap();
                }
                r
            })
            .collect();
        readings.extend(later_rows);
        let report = assess(&registry, &readings, &[document(), later.clone()]);
        assert_eq!(report.components.len(), 2);
        assert!(
            report
                .components
                .iter()
                .any(|c| matches!(c.component.outcome, xf::Outcome::Feasible { .. }))
        );
        assert!(
            report
                .components
                .iter()
                .any(|c| matches!(c.component.outcome, xf::Outcome::Infeasible { .. }))
        );
        assert_eq!(report.cells.len(), 8);
        later.published_at = "2032-03-01".into();
        assert_ne!(
            report.digest(),
            assess(&registry, &readings, &[document(), later]).digest()
        );
    }
