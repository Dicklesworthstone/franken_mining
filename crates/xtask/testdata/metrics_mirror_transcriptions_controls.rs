    use super::*;

    const PERIOD: Period = Period {
        year: 2025,
        first_month: 1,
        last_month: 12,
    };

    fn fixture() -> (DocumentRow, Vec<Accepted>) {
        let document = DocumentRow {
            file: "Invented.pdf".into(),
            dir: "synthetic/alpha".into(),
            kind: "partner_filing".into(),
            url: "https://example.invalid/Invented.pdf".into(),
            url_basis: "DESIGN: invented source for a pure known-answer test".into(),
            published_at: "2026-01-15".into(),
            published_at_basis: "FACT: invented test date".into(),
            pages: 2,
            bytes: 1,
            sha256: "11".repeat(32),
        };
        let keys = keys("invented-jv", PERIOD, MirrorMetric::ProductionKoz);
        let rows = [(&keys[1], "615", "attributable", "koz"), (&keys[2], "61.5", "equity", "percent")]
            .into_iter()
            .map(|(key, token, mode, unit)| Accepted {
                locator: Locator {
                    doc: document.file.clone(), page: 1, table: "Invented assertions".into(),
                    row: key.clone(), col: "2025".into(),
                },
                doc_sha256: document.sha256.clone(), token: token.into(),
                value: parse_token(token).expect("invented numeric token"), quantity: key.clone(),
                unit: unit.into(),
                basis: format!("{mode};perimeter=invented-jv;members=mine-a+mine-b;valid=2025-01-01/2025-12-31;scope_page=2;scope_table=Scope;scope_row=Operations;scope_col=2025"),
                period: period_label(PERIOD), readers: vec!["synthetic-a".into(), "synthetic-b".into()],
            }).collect();
        (document, rows)
    }

    fn bind_fixture(document: &DocumentRow, rows: &[Accepted]) -> Result<BoundReadings> {
        bind(
            "invented-jv",
            PERIOD,
            MirrorMetric::ProductionKoz,
            "alpha",
            document,
            rows,
            EvidenceClass::Assumed,
        )
    }

    #[test]
    fn pure_binding_retains_exact_share_rounding_and_context_parents() {
        let (document, rows) = fixture();
        let bound = bind_fixture(&document, &rows).expect("bind agreed invented rows");
        let ownership = bound.ownership.expect("separately cited ownership");
        assert_eq!(
            ownership.value,
            Rat::new(123, 200).expect("exact legal share")
        );
        assert_eq!(ownership.evidence, EvidenceClass::Assumed);
        assert_eq!(bound.inputs.len(), 1);
        let input = &bound.inputs[0];
        assert_eq!(input.parents, vec![ownership.claim_id]);
        assert!(ownership.anchors.iter().all(|a| input.anchors.contains(a)));
        assert_eq!(
            input.value.quantity.ownership,
            Ownership::Attributable {
                share: ownership.value
            }
        );
        assert_eq!(
            input.value.quantity.interval,
            Interval {
                lo: Rat::new(1229, 2).expect("closed lower source edge"),
                hi: Rat::new(1231, 2).expect("closed upper source edge"),
                lo_strict: false,
                hi_strict: false,
            }
        );
        assert_eq!(input.evidence, EvidenceClass::Assumed);
        assert_eq!(input.value_tag(), input.value.canonical_tag());
        assert!(input.check_id().is_ok());
        assert_eq!(
            input.value.source.artifact_path,
            "synthetic/alpha/Invented.pdf"
        );
        assert!(
            input
                .anchors
                .iter()
                .any(|a| a.locator == "Invented.pdf:p2:Scope:Operations/2025")
        );
    }

    #[test]
    fn both_numeric_and_ownership_rows_must_match_the_declared_document() {
        let (document, rows) = fixture();
        for row in 0..rows.len() {
            for scenario in 0..4 {
                let mut altered = rows.clone();
                match scenario {
                    0 => altered[row].locator.doc = "Another.pdf".into(),
                    1 => altered[row].doc_sha256 = "22".repeat(32),
                    2 => altered[row].locator.page = 0,
                    _ => altered[row].locator.page = document.pages + 1,
                }
                assert_eq!(
                    bind_fixture(&document, &altered)
                        .expect_err("mismatched source metadata")
                        .code,
                    "FMINE-REF-105",
                    "row {row}, scenario {scenario}"
                );
            }
        }
        assert_eq!(
            bind(
                "invented-jv",
                PERIOD,
                MirrorMetric::Capex(CapexScope::Sustaining),
                "alpha",
                &document,
                &rows,
                EvidenceClass::Assumed
            )
            .expect_err("capital cannot borrow an ounce source")
            .code,
            "FMINE-REF-201"
        );
    }

    fn capital_fixture(scope: CapexScope) -> (DocumentRow, Vec<Accepted>) {
        let (document, mut rows) = fixture();
        rows[0].quantity = keys("invented-jv", PERIOD, MirrorMetric::Capex(scope))[1].clone();
        rows[0].unit = "USD_m".into();
        rows[0].basis.push_str(
            ";timing=cash;leases=included;definition_page=2;definition_table=Capital policy;definition_row=Scope timing and leases;definition_col=2025",
        );
        (document, rows)
    }

    fn bind_capital(
        document: &DocumentRow,
        rows: &[Accepted],
        scope: CapexScope,
    ) -> Result<BoundReadings> {
        bind(
            "invented-jv",
            PERIOD,
            MirrorMetric::Capex(scope),
            "alpha",
            document,
            rows,
            EvidenceClass::Assumed,
        )
    }

    #[test]
    fn capital_binding_requires_usd_m_and_retains_each_reporters_definition_page() {
        for scope in CapexScope::ALL {
            let (document, mut rows) = capital_fixture(scope);
            let bound = bind_capital(&document, &rows, scope).unwrap();
            let c = &bound.inputs[0];
            assert_eq!(c.value.quantity.kind, MirrorMetric::Capex(scope));
            assert_eq!(
                c.value.quantity.capex_basis,
                Some(CapexBasis {
                    scope,
                    timing: CapexTiming::Cash,
                    leases: LeaseTreatment::Included,
                })
            );
            let definition = c.value.basis_source.as_ref().unwrap();
            assert_eq!(
                definition.locator,
                "Invented.pdf:p2:Capital policy:Scope timing and leases/2025"
            );
            assert!(c.anchors.contains(definition));
            assert_eq!(definition.artifact_path, c.value.source.artifact_path);
            assert_eq!(definition.artifact_sha256, c.value.source.artifact_sha256);
            assert_eq!(c.parents, vec![bound.ownership.unwrap().claim_id]);
            assert!(
                keys("invented-jv", PERIOD, MirrorMetric::Capex(scope))[0].ends_with("_100_usd_m")
            );

            rows[0].token = "-10".into();
            rows[0].value = parse_token("-10").unwrap();
            let signed = bind_capital(&document, &rows, scope).unwrap();
            assert_eq!(
                signed.inputs[0].value.quantity.interval.lo,
                Rat::new(-21, 2).unwrap()
            );
            assert_eq!(
                signed.inputs[0].value.quantity.interval.hi,
                Rat::new(-19, 2).unwrap()
            );
        }
    }

    #[test]
    fn capital_binding_refuses_missing_uncited_or_ambiguous_definitions() {
        let (document, rows) = capital_fixture(CapexScope::Total);
        for (from, to) in [
            (";timing=cash", ""),
            ("timing=cash", "timing=unstated"),
            (";leases=included", ""),
            ("leases=included", "leases=unstated"),
            (";definition_page=2", ""),
            ("definition_page=2", "definition_page=3"),
            ("definition_row=Scope timing and leases", "definition_row="),
            ("definition_table=Capital policy", "definition_table=   "),
            (
                "definition_row=Scope timing and leases",
                "definition_row= \t ",
            ),
            ("definition_col=2025", "definition_col=   "),
            (";timing=cash", ";timing=cash;timing=accrual"),
            (
                ";timing=cash",
                ";timing=cash;accounting_basis=purchase_price_allocation",
            ),
        ] {
            let mut altered = rows.clone();
            altered[0].basis = altered[0].basis.replace(from, to);
            assert_eq!(
                bind_capital(&document, &altered, CapexScope::Total)
                    .unwrap_err()
                    .code,
                "FMINE-REF-104",
                "{from} -> {to}"
            );
        }
        for case in 0..4 {
            let mut altered = rows.clone();
            let expected = match case {
                0 => {
                    altered[0].unit = "USD".into();
                    "105"
                }
                1 => {
                    altered[0].period = "2024FY".into();
                    "105"
                }
                2 => {
                    altered[0].readers = vec!["only-one".into()];
                    "202"
                }
                _ => {
                    altered[0].token = "0".into();
                    "202"
                }
            };
            assert_eq!(
                bind_capital(&document, &altered, CapexScope::Total)
                    .unwrap_err()
                    .code,
                format!("FMINE-REF-{expected}")
            );
        }
    }
