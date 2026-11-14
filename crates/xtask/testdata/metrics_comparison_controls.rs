// Shared exact public-API controls; all issuers, documents and values are invented.

fn basis(metric: Metric) -> String {
    let base = format!(
        "metric={};scope=company;ownership=attributable;operations=all;metal=gold;denominator={}",
        metric.as_str(),
        if metric == Metric::Aisc {
            "sold"
        } else {
            "produced"
        }
    );
    let cost = if metric == Metric::Aisc {
        ";allocation=by_product;credits=separate_line;inventory=included;ga=included;sustaining_leases=included;minesite_exploration=included;reclamation=operating_only;stripping=included;accounting=as_reported"
    } else {
        ""
    };
    format!(
        "{base}{cost};definition_page=2;definition_table=Invented reconciliation;definition_row=Metric definition;definition_col=2030"
    )
}

fn document(issuer: &str) -> DocumentRow {
    DocumentRow {
        file: format!("{issuer}.pdf"),
        dir: "synthetic".into(),
        kind: "mda".into(),
        url: format!("https://example.invalid/{issuer}.pdf"),
        url_basis: "synthetic fixture".into(),
        published_at: "2031-02-01".into(),
        published_at_basis: "FACT: invented release date".into(),
        pages: 2,
        bytes: 100,
        sha256: "11".repeat(32),
    }
}

fn row(issuer: &str, metric: Metric, token: &str) -> Accepted {
    Accepted {
        locator: Locator {
            doc: format!("{issuer}.pdf"),
            page: 1,
            table: "Invented annual results".into(),
            row: metric.as_str().into(),
            col: "2030".into(),
        },
        doc_sha256: "11".repeat(32),
        token: token.into(),
        value: parse_token(token).unwrap(),
        quantity: "peer_actual".into(),
        unit: metric.unit().into(),
        basis: basis(metric),
        period: "2030FY".into(),
        readers: vec!["reader-a".into(), "reader-b".into()],
    }
}

fn supplied(issuer: &str, metric: Metric, token: &str) -> Claim<PeerReading> {
    source::bind(
        issuer,
        metric,
        2030,
        &document(issuer),
        &row(issuer, metric, token),
        EvidenceClass::Assumed,
    )
    .unwrap()
}

fn rebind(input: &Claim<PeerReading>, value: PeerReading) -> Claim<PeerReading> {
    let tag = value.canonical_tag();
    Claim::new(
        INPUT_METRIC,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        input.additivity,
        input.evidence,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        input.anchors.clone(),
    )
}

fn bounded(issuer: &str, lo: i128, hi: i128) -> Claim<PeerReading> {
    let input = supplied(issuer, Metric::Aisc, "1");
    let mut value = input.value.clone();
    value.interval = Interval {
        lo: Rat::from_integer(lo),
        hi: Rat::from_integer(hi),
        lo_strict: false,
        hi_strict: false,
    };
    rebind(&input, value)
}

fn request(inputs: &[Claim<PeerReading>]) -> Request {
    Request {
        metric: inputs[0].value.metric,
        year: 2030,
        as_of: "2031-03-01".into(),
        issuers: inputs.iter().map(|i| i.value.issuer.clone()).collect(),
        claim: None,
    }
}

fn rank(report: &Report, issuer: &str) -> PeerRank {
    report
        .claim
        .value
        .peers
        .iter()
        .find(|p| p.issuer == issuer)
        .unwrap()
        .clone()
}

#[test]
fn peer_bead_known_answer_and_three_verdicts() {
    let inputs = vec![
        bounded("a", 1600, 1700),
        bounded("b", 1550, 1620),
        bounded("c", 1580, 1690),
        bounded("d", 1720, 1760),
    ];
    let mut req = request(&inputs);
    let report = compare(&req, &inputs).unwrap();
    for issuer in ["a", "b", "c"] {
        assert_eq!(
            rank(&report, issuer).ascending,
            RankBounds { min: 1, max: 3 }
        );
    }
    assert_eq!(rank(&report, "d").ascending, RankBounds { min: 4, max: 4 });
    assert_eq!(
        report.claim.parents.len(),
        4,
        "every source remains a result parent"
    );
    assert_eq!(report.claim.evidence, EvidenceClass::Assumed);
    assert_eq!(report.claim.additivity, AdditivityClass::NonAdditive);
    report.claim.check_id().unwrap();
    for (issuer, order, expected) in [
        ("b", RankOrder::Lowest, Verdict::Indeterminate),
        ("d", RankOrder::Highest, Verdict::Supported),
        ("d", RankOrder::Lowest, Verdict::Unsupported),
    ] {
        req.claim = Some(RankClaim {
            issuer: issuer.into(),
            order,
            at_most: 1,
        });
        assert_eq!(
            compare(&req, &inputs).unwrap().claim.value.verdict,
            Some(expected)
        );
    }
}

#[test]
fn peer_point_ties_and_touching_endpoints_use_competition_ranks() {
    let inputs = vec![bounded("a", 5, 5), bounded("b", 5, 5), bounded("c", 9, 9)];
    let mut req = request(&inputs);
    req.claim = Some(RankClaim {
        issuer: "a".into(),
        order: RankOrder::Lowest,
        at_most: 1,
    });
    let report = compare(&req, &inputs).unwrap();
    assert_eq!(rank(&report, "a").ascending, RankBounds { min: 1, max: 1 });
    assert_eq!(rank(&report, "b").ascending, RankBounds { min: 1, max: 1 });
    assert_eq!(rank(&report, "c").ascending, RankBounds { min: 3, max: 3 });
    assert_eq!(rank(&report, "a").descending, RankBounds { min: 2, max: 2 });
    assert_eq!(
        report.claim.value.verdict,
        Some(Verdict::Supported),
        "lowest includes tied lowest"
    );
    let touching = vec![bounded("a", 1, 2), bounded("b", 2, 3)];
    let report = compare(&request(&touching), &touching).unwrap();
    assert_eq!(rank(&report, "a").ascending, RankBounds { min: 1, max: 1 });
    assert_eq!(rank(&report, "b").ascending, RankBounds { min: 1, max: 2 });
    assert_eq!(rank(&report, "a").descending, RankBounds { min: 1, max: 2 });
    assert_eq!(rank(&report, "b").descending, RankBounds { min: 1, max: 1 });
}

#[test]
fn peer_rank_extrema_match_exhaustive_small_closed_boxes() {
    let boxes = [(0, 0), (0, 1), (1, 1), (1, 2), (0, 2)];
    for a in boxes {
        for b in boxes {
            for c in boxes {
                let limits = [a, b, c];
                let inputs = vec![
                    bounded("a", a.0, a.1),
                    bounded("b", b.0, b.1),
                    bounded("c", c.0, c.1),
                ];
                let report = compare(&request(&inputs), &inputs).unwrap();
                for j in 0..3 {
                    let mut asc = RankBounds {
                        min: usize::MAX,
                        max: 0,
                    };
                    let mut desc = asc;
                    for mask in 0..8 {
                        let point = limits
                            .iter()
                            .enumerate()
                            .map(|(i, (lo, hi))| if mask & (1 << i) == 0 { *lo } else { *hi })
                            .collect::<Vec<_>>();
                        let up = 1 + point.iter().filter(|p| **p < point[j]).count();
                        let down = 1 + point.iter().filter(|p| **p > point[j]).count();
                        asc.min = asc.min.min(up);
                        asc.max = asc.max.max(up);
                        desc.min = desc.min.min(down);
                        desc.max = desc.max.max(down);
                    }
                    assert_eq!(
                        report.claim.value.peers[j].ascending, asc,
                        "ascending {limits:?}, peer {j}"
                    );
                    assert_eq!(
                        report.claim.value.peers[j].descending, desc,
                        "descending {limits:?}, peer {j}"
                    );
                }
            }
        }
    }
}

#[test]
fn peer_population_and_definition_never_shrink_or_restate_silently() {
    let inputs = vec![bounded("a", 1, 2), bounded("b", 3, 4), bounded("c", 5, 6)];
    let req = request(&inputs);
    assert!(
        compare(&req, &inputs[..2])
            .unwrap_err()
            .what
            .contains("missing=[c]")
    );
    let mut duplicate = inputs.clone();
    duplicate[2] = inputs[1].clone();
    assert!(compare(&req, &duplicate).is_err());
    let mut changed = inputs.clone();
    let mut value = changed[1].value.clone();
    value
        .definition
        .fields
        .insert("inventory".into(), "excluded".into());
    changed[1] = rebind(&changed[1], value);
    let refusal = compare(&req, &changed).unwrap_err();
    assert!(refusal.what.contains("definition mismatch inventory"));
    assert!(refusal.what.contains("a=included, b=excluded"));
    let mut value = changed[1].value.clone();
    value.definition.fields.remove("inventory");
    changed[1] = rebind(&changed[1], value);
    assert!(
        compare(&req, &changed)
            .unwrap_err()
            .what
            .contains("missing definition field inventory")
    );
}

#[test]
fn peer_sources_and_temporal_support_remain_bound() {
    let inputs = vec![
        supplied("a", Metric::Aisc, "1600.0"),
        supplied("b", Metric::Aisc, "1700"),
    ];
    assert_eq!(inputs[0].value.interval.lo, Rat::new(31999, 20).unwrap());
    assert_eq!(inputs[0].value.interval.hi, Rat::new(32001, 20).unwrap());
    let mut req = request(&inputs);
    let mut forged = inputs.clone();
    forged[0].value.interval.lo = Rat::from_integer(0);
    assert!(
        compare(&req, &forged)
            .unwrap_err()
            .what
            .contains("canonical interpretation")
    );
    req.as_of = "2031-01-01".into();
    assert!(
        compare(&req, &inputs)
            .unwrap_err()
            .what
            .contains("after the selected evidence cut")
    );
    req = request(&inputs);
    let mut changed = inputs.clone();
    let mut value = changed[0].value.clone();
    value.published_at = "2030-06-01".into();
    changed[0] = rebind(&changed[0], value);
    assert!(
        compare(&req, &changed)
            .unwrap_err()
            .what
            .contains("before the year ended")
    );
    req.claim = Some(RankClaim {
        issuer: "absent".into(),
        order: RankOrder::Lowest,
        at_most: 1,
    });
    assert!(compare(&req, &inputs).is_err());
}

#[test]
fn peer_transcription_binder_checks_readers_definition_units_and_document() {
    let original = row("a", Metric::Aisc, "1600");
    let doc = document("a");
    for changed in {
        let mut rows = Vec::new();
        let mut r = original.clone();
        r.readers.pop();
        rows.push(r);
        let mut r = original.clone();
        r.readers[1] = r.readers[0].clone();
        rows.push(r);
        let mut r = original.clone();
        r.doc_sha256 = "22".repeat(32);
        rows.push(r);
        let mut r = original.clone();
        r.unit = "USD/lb".into();
        rows.push(r);
        let mut r = original.clone();
        r.period = "2030H1".into();
        rows.push(r);
        let mut r = original.clone();
        r.basis = r.basis.replace("definition_page=2", "definition_page=3");
        rows.push(r);
        let mut r = original.clone();
        r.basis = r.basis.replace("denominator=sold", "denominator=produced");
        rows.push(r);
        let mut r = original.clone();
        r.token = "1500 - 1600".into();
        r.value = parse_token(&r.token).unwrap();
        rows.push(r);
        let mut r = original.clone();
        r.basis.push_str(";bridge=transferred");
        rows.push(r);
        rows
    } {
        assert!(
            source::bind(
                "a",
                Metric::Aisc,
                2030,
                &doc,
                &changed,
                EvidenceClass::Assumed
            )
            .is_err()
        );
    }
}

#[test]
fn peer_gold_production_and_ordering_have_a_positive_path() {
    let inputs = vec![
        supplied("a", Metric::GoldProduction, "4000.0"),
        supplied("b", Metric::GoldProduction, "3000.0"),
    ];
    let mut req = request(&inputs);
    req.claim = Some(RankClaim {
        issuer: "a".into(),
        order: RankOrder::Highest,
        at_most: 1,
    });
    let report = compare(&req, &inputs).unwrap();
    assert_eq!(report.claim.value.verdict, Some(Verdict::Supported));
    assert_eq!(report.inputs.len(), 2);
    let mut reversed = inputs.clone();
    reversed.reverse();
    req.issuers.reverse();
    assert_eq!(
        compare(&req, &reversed).unwrap().claim.claim_id,
        report.claim.claim_id
    );
}
