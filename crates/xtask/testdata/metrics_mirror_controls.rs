    use super::*;
    use fmine_claim::UuTerm;

    fn rat(num: i128, den: i128) -> Rat {
        Rat::new(num, den).unwrap()
    }

    fn interval(lo: Rat, hi: Rat) -> Interval {
        Interval {
            lo,
            hi,
            lo_strict: false,
            hi_strict: false,
        }
    }

    pub(super) fn reading(
        reporter: &str,
        suffix: &str,
        point: i128,
        ownership: Ownership,
    ) -> Claim<MirrorReading> {
        let file = format!("synthetic_{reporter}_{suffix}.pdf");
        let hash = fmine_core::sha256(file.as_bytes()).0;
        let source = SourceAnchor::new(
            &file,
            hash,
            format!("{file}:p1:Invented production:{suffix}/2025"),
        );
        let context = SourceAnchor::new(
            &file,
            hash,
            format!("{file}:p2:Invented ownership and perimeter"),
        );
        let value = MirrorReading {
            reporter: reporter.into(),
            source: source.clone(),
            basis_source: None,
            perimeter: Perimeter {
                id: "invented-common-perimeter".into(),
                operations: vec!["invented-operation".into()],
                valid_from: CivilDate::new(2024, 1, 1).unwrap(),
                valid_through: CivilDate::new(2026, 12, 31).unwrap(),
            },
            quantity: MirrorQuantity {
                subject: "invented-asset".into(),
                period: crate::guidance::Period {
                    year: 2025,
                    first_month: 1,
                    last_month: 12,
                },
                ownership,
                kind: MirrorMetric::ProductionKoz,
                capex_basis: None,
                interval: interval(rat(2 * point - 1, 2), rat(2 * point + 1, 2)),
            },
        };
        let tag = value.canonical_tag();
        Claim::new(
            READING_METRIC,
            value,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![source, context],
        )
    }

    fn pair(points: [i128; 3]) -> Vec<Claim<MirrorReading>> {
        vec![
            reading("left", "full", points[0], Ownership::Full),
            reading(
                "left",
                "attributable",
                points[1],
                Ownership::Attributable {
                    share: rat(123, 200),
                },
            ),
            reading(
                "right",
                "attributable",
                points[2],
                Ownership::Attributable {
                    share: rat(77, 200),
                },
            ),
        ]
    }

    pub(super) fn reseal(c: &mut Claim<MirrorReading>) {
        let (stale, calibration_failed) = (c.stale, c.calibration_failed);
        *c = Claim::new(
            c.metric_key.clone(),
            c.value.clone(),
            &c.value.canonical_tag(),
            c.representation.clone(),
            c.support,
            c.additivity,
            c.evidence,
            c.epistemic,
            c.state,
            c.confidentiality.clone(),
            c.uu_terms.clone(),
            c.parents.clone(),
            c.anchors.clone(),
        );
        c.stale = stale;
        c.calibration_failed = calibration_failed;
    }

    #[test]
    fn exact_three_reported_boxes_and_closed_tie_have_checked_witnesses() {
        for (points, expected) in [
            ([1000, 615, 385], interval(rat(1999, 2), rat(2001, 2))),
            (
                [100, 62, 38],
                interval(Rat::from_integer(100), Rat::from_integer(100)),
            ),
        ] {
            let report = compare(&pair(points)).unwrap();
            let MirrorVerdict::Consistent { witness } = &report.verdict else {
                panic!("{report:?}");
            };
            assert_eq!(
                report.common.as_ref().unwrap().value.quantity.interval,
                expected
            );
            assert!(report.physical_nonnegative);
            assert_eq!(report.rules.convention, xf::Convention::Closed);
            xf::check_witness(&report.cells, &report.identities, &report.rules, witness).unwrap();
            let mut broken = witness.clone();
            let first = broken.values_mut().next().unwrap();
            *first = first.checked_add(Rat::from_integer(10)).unwrap();
            assert!(
                xf::check_witness(&report.cells, &report.identities, &report.rules, &broken)
                    .is_err()
            );
            assert!(
                report
                    .identities
                    .iter()
                    .all(|id| matches!(id.kind, xf::IdentityKind::Scale { .. }))
            );
        }
    }

    #[test]
    fn exact_exclusion_has_independently_checked_minimal_source_pairs() {
        let report = compare(&pair([1000, 615, 450])).unwrap();
        let MirrorVerdict::Conflict {
            certificate,
            certificate_digest,
            blame,
            minimal_source_sets,
        } = &report.verdict
        else {
            panic!("{report:?}");
        };
        assert!(report.common.is_none());
        xf::check_certificate(
            &report.cells,
            &report.identities,
            &report.rules,
            certificate,
        )
        .unwrap();
        assert_eq!(
            certificate_digest,
            &xf::certificate_digest(
                &report.cells,
                &report.identities,
                &report.rules,
                certificate
            )
            .unwrap()
        );
        assert!(!minimal_source_sets.is_empty());
        for indices in minimal_source_sets {
            assert_eq!(indices.len(), 2);
            let reporters: BTreeSet<_> = indices
                .iter()
                .map(|&i| report.inputs[i].value.reporter.as_str())
                .collect();
            assert_eq!(reporters, BTreeSet::from(["left", "right"]));
        }
        let xf::BlameOutcome::Blamed(blame) = blame else {
            panic!("{blame:?}");
        };
        for conflict in &blame.conflicts {
            xf::check_conflict_set(&report.cells, &report.identities, &report.rules, conflict)
                .unwrap();
        }
        assert_eq!(blame.label, xf::CorrectionLabel::Minimum);
        assert_eq!(blame.corrections.len(), 1);
        assert_eq!(blame.corrections[0].cells.len(), 1);
        assert!(blame.corrections[0].cells[0].starts_with("right:"));
        let corrupt = xf::Certificate::Farkas {
            multipliers: vec![],
        };
        assert!(
            xf::check_certificate(&report.cells, &report.identities, &report.rules, &corrupt)
                .is_err()
        );
    }

    #[test]
    fn zero_keeps_source_rounding_and_declares_the_nonnegative_intersection() {
        let report = compare(&[
            reading("left", "zero", 0, Ownership::Full),
            reading("right", "zero", 0, Ownership::Full),
        ])
        .unwrap();
        assert_eq!(
            report.common.as_ref().unwrap().value.quantity.interval,
            interval(ZERO, rat(1, 2))
        );
        for source in &report.normalized {
            assert_eq!(
                source.value.quantity.interval,
                interval(rat(-1, 2), rat(1, 2))
            );
        }
        for cell in &report.cells {
            assert_eq!(cell.printed, ZERO);
            assert_eq!(cell.step, ONE);
        }
        let MirrorVerdict::Consistent { witness } = &report.verdict else {
            panic!("{report:?}");
        };
        assert!(witness.values().all(|value| *value == ZERO));
        xf::check_witness(&report.cells, &report.identities, &report.rules, witness).unwrap();
    }

    #[test]
    fn partial_reporting_ownership_and_sales_are_valid_without_a_full_owner() {
        let mut inputs = vec![
            reading(
                "left",
                "sales",
                45,
                Ownership::Attributable { share: rat(9, 20) },
            ),
            reading(
                "right",
                "sales",
                45,
                Ownership::Attributable { share: rat(9, 20) },
            ),
        ];
        for input in &mut inputs {
            input.value.quantity.kind = MirrorMetric::SoldKoz;
            input.value.quantity.period.first_month = 7;
            reseal(input);
        }
        let common = compare(&inputs).unwrap().common.unwrap();
        assert_eq!(common.value.quantity.kind, MirrorMetric::SoldKoz);
        assert_eq!(
            common.value.quantity.interval,
            interval(rat(890, 9), rat(910, 9))
        );
        assert_eq!(common.value.quantity.ownership, Ownership::Full);
        assert_eq!(common.value.reporters, ["left", "right"]);
    }

    #[test]
    fn input_order_and_exact_full_basis_restatement_preserve_the_result() {
        let mut inputs = pair([1000, 615, 385]);
        let before = compare(&inputs).unwrap();
        inputs.reverse();
        assert_eq!(compare(&inputs).unwrap(), before);
        for input in &mut inputs {
            let factor = share(&input.value.quantity);
            input.value.quantity.interval.lo = input
                .value
                .quantity
                .interval
                .lo
                .checked_div(factor)
                .unwrap();
            input.value.quantity.interval.hi = input
                .value
                .quantity
                .interval
                .hi
                .checked_div(factor)
                .unwrap();
            input.value.quantity.ownership = Ownership::Full;
            reseal(input);
        }
        let after = compare(&inputs).unwrap();
        assert!(matches!(after.verdict, MirrorVerdict::Consistent { .. }));
        assert_eq!(
            before.common.unwrap().value.quantity,
            after.common.unwrap().value.quantity
        );
    }

    #[test]
    fn dated_scope_provenance_and_weaker_metadata_survive_normalization() {
        let mut inputs = pair([1000, 615, 385]);
        inputs[0].evidence = EvidenceClass::ISSUER;
        inputs[0].state = KnowledgeState::Estimated;
        inputs[0].epistemic = EpistemicStatus::UnmodelledBudget;
        for (i, input) in inputs.iter_mut().enumerate() {
            input.uu_terms.push(UuTerm {
                scenario_id: "same-id-different-source-context".into(),
                description: format!("invented uncertainty {i}"),
                downside_exposure_usd_m: 0.0,
                trigger_indicator: "explicitly unquantified".into(),
            });
            reseal(input);
        }
        inputs[1].value.perimeter.valid_from = CivilDate::new(2025, 1, 1).unwrap();
        inputs[1].value.perimeter.valid_through = CivilDate::new(2025, 12, 31).unwrap();
        reseal(&mut inputs[1]);
        let report = compare(&inputs).unwrap();
        for (input, normalized) in report.inputs.iter().zip(&report.normalized) {
            assert_eq!(normalized.parents, [input.claim_id]);
            assert_eq!(
                normalized.evidence,
                EvidenceClass::Derived.meet(input.evidence)
            );
            assert_eq!(normalized.state, input.state);
            assert_eq!(normalized.uu_terms, input.uu_terms);
            assert_eq!(normalized.value.reporter, input.value.reporter);
            assert!(input.anchors.iter().all(|a| normalized.anchors.contains(a)));
            normalized.check_id().unwrap();
            assert_eq!(normalized.value_tag(), normalized.value.canonical_tag());
        }
        let common = report.common.unwrap();
        assert_eq!(common.evidence, EvidenceClass::ISSUER);
        assert_eq!(common.state, KnowledgeState::Estimated);
        assert_eq!(common.epistemic, EpistemicStatus::UnmodelledBudget);
        assert!(common.confidentiality.is_public());
        assert_eq!(common.uu_terms.len(), 3); // Same scenario id must not discard a different source's term.
        assert_eq!(common.parents.len(), 3);
        for input in &inputs {
            assert!(common.parents.contains(&input.claim_id));
            assert!(input.anchors.iter().all(|a| common.anchors.contains(a)));
        }
        assert_eq!(
            common.value.perimeter.valid_from,
            CivilDate::new(2025, 1, 1).unwrap()
        );
        assert_eq!(
            common.value.perimeter.valid_through,
            CivilDate::new(2025, 12, 31).unwrap()
        );
        common.check_id().unwrap();
        assert_eq!(common.value_tag(), common.value.canonical_tag());
    }

    #[test]
    fn issuer_qualified_cells_cannot_alias_identical_document_basenames() {
        let mut inputs = vec![
            reading("left", "a", 100, Ownership::Full),
            reading("right", "a", 100, Ownership::Full),
        ];
        for input in &mut inputs {
            input.value.source =
                SourceAnchor::new("Report.pdf", [7; 32], "Report.pdf:p1:Production/2025");
            input.anchors = vec![input.value.source.clone()];
            reseal(input);
        }
        let report = compare(&inputs).unwrap();
        assert_ne!(report.cells[0].locator, report.cells[1].locator);
        assert!(matches!(report.verdict, MirrorVerdict::Consistent { .. }));
    }

    #[test]
    fn unsupported_or_unusable_semantics_refuse_before_solving() {
        for case in 0..24 {
            let mut inputs = pair([1000, 615, 385]);
            let c = &mut inputs[2];
            let code = match case {
                0 => {
                    c.value.quantity.kind = MirrorMetric::SoldKoz;
                    "105"
                }
                1 => {
                    c.value.quantity.subject = "another-asset".into();
                    "105"
                }
                2 => {
                    c.value.quantity.period.first_month = 2;
                    "102"
                }
                3 => {
                    c.value
                        .perimeter
                        .operations
                        .push("another-operation".into());
                    "104"
                }
                4 => {
                    c.value.perimeter.valid_from = CivilDate::new(2025, 1, 2).unwrap();
                    "104"
                }
                5 => {
                    c.value.perimeter.valid_through = CivilDate {
                        year: 2025,
                        month: 2,
                        day: 30,
                    };
                    "102"
                }
                6 => {
                    c.value.quantity.ownership = Ownership::Attributable { share: ZERO };
                    "105"
                }
                7 => {
                    c.value.quantity.ownership = Ownership::Attributable {
                        share: Rat::from_integer(2),
                    };
                    "105"
                }
                8 => {
                    c.value.quantity.interval = interval(ONE, ZERO);
                    "109"
                }
                9 => {
                    c.value.quantity.interval.lo_strict = true;
                    "203"
                }
                10 => {
                    c.value.perimeter.operations.clear();
                    "104"
                }
                11 => {
                    c.metric_key = "some.other.metric".into();
                    "105"
                }
                12 => {
                    c.representation = Representation::Placeholder {
                        token: "—".into()
                    };
                    "208"
                }
                13 => {
                    c.state = KnowledgeState::Unknown;
                    "202"
                }
                14 => {
                    c.confidentiality = Confidentiality::internal();
                    "301"
                }
                15 => {
                    c.anchors.clear();
                    "201"
                }
                16 => {
                    c.stale = true;
                    "502"
                }
                17 => {
                    c.calibration_failed = true;
                    "205"
                }
                18 => {
                    c.additivity = AdditivityClass::IntensiveWeighted;
                    "103"
                }
                19 => {
                    c.value
                        .perimeter
                        .operations
                        .push(c.value.perimeter.operations[0].clone());
                    "104"
                }
                20 => {
                    c.value.reporter = "Uncanonical".into();
                    "107"
                }
                21 => {
                    c.value.quantity.interval = interval(rat(-3, 2), rat(-1, 2));
                    "109"
                }
                22 => {
                    c.value.quantity.capex_basis = Some(CapexBasis {
                        scope: CapexScope::Sustaining,
                        timing: CapexTiming::Cash,
                        leases: LeaseTreatment::Included,
                    });
                    "105"
                }
                _ => {
                    c.state = KnowledgeState::Conflicted;
                    "202"
                }
            };
            reseal(c);
            assert_eq!(
                compare(&inputs).unwrap_err().code,
                format!("FMINE-REF-{code}"),
                "case {case}"
            );
        }
        assert_eq!(compare(&[]).unwrap_err().code, "FMINE-REF-201");
        assert_eq!(
            compare(&pair([1000, 615, 385])[..2]).unwrap_err().code,
            "FMINE-REF-201"
        );
        let mut duplicate = pair([1000, 615, 385]);
        duplicate.push(duplicate[0].clone());
        assert_eq!(compare(&duplicate).unwrap_err().code, "FMINE-REF-104");
        let mut shares = pair([1000, 615, 385]);
        shares.push(reading(
            "left",
            "incompatible-share",
            500,
            Ownership::Attributable { share: rat(1, 2) },
        ));
        assert_eq!(compare(&shares).unwrap_err().code, "FMINE-REF-105");
    }

    #[test]
    fn canonical_identity_mutation_and_numerical_indeterminacy_never_become_verdicts() {
        let mut inputs = pair([1000, 615, 385]);
        inputs[0].value.reporter = "different-reporter".into();
        assert_eq!(compare(&inputs).unwrap_err().code, "FMINE-REF-507");
        inputs = pair([1000, 615, 385]);
        inputs[0].anchors[0].artifact_sha256[0] ^= 1;
        assert_eq!(compare(&inputs).unwrap_err().code, "FMINE-REF-507");
        let capped = compare_with_caps(&pair([1000, 615, 385]), 0, 0).unwrap();
        assert!(matches!(
            capped.verdict,
            MirrorVerdict::Indeterminate { .. }
        ));
        assert!(capped.common.is_none());
        let mut overflow = pair([1000, 615, 385]);
        overflow[2].value.quantity.interval =
            interval(Rat::from_integer(i128::MAX), Rat::from_integer(i128::MAX));
        reseal(&mut overflow[2]);
        assert_eq!(compare(&overflow).unwrap_err().code, "FMINE-REF-207");
    }

    #[test]
    fn independently_rounded_integer_worlds_keep_their_known_common_quantity() {
        // Truth is n/10 koz; integer arithmetic rounds each reporter independently to whole koz.
        // This is a finite known-answer sweep, not the full BD-346.S6 power study.
        for n in 1000_i128..1256 {
            let inputs = pair([
                (n + 5) / 10,
                (123 * n + 1000) / 2000,
                (77 * n + 1000) / 2000,
            ]);
            let report = compare(&inputs).unwrap();
            assert!(
                matches!(report.verdict, MirrorVerdict::Consistent { .. }),
                "n={n}: {report:?}"
            );
            assert!(
                report
                    .common
                    .unwrap()
                    .value
                    .quantity
                    .interval
                    .contains(rat(n, 10))
                    .unwrap(),
                "n={n}"
            );
        }
    }
