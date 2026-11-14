use super::*;
use fmine_registry::transcriptions::parse_token;

const PERIOD: Period = Period {
    year: 2030,
    first_month: 1,
    last_month: 12,
};

fn fixture(
    reporter: &str,
    metric: CostMetric,
    amount: &str,
    share: &str,
    ga: &str,
    adjustment: Option<(DefinitionField, &str, &str)>,
) -> (DocumentRow, Vec<Accepted>) {
    let document = DocumentRow {
        file: format!("{reporter}.pdf"),
        dir: format!("synthetic/{reporter}"),
        kind: "partner_filing".into(),
        url: format!("https://example.invalid/{reporter}.pdf"),
        url_basis: "DESIGN: invented known-answer source".into(),
        published_at: "2031-01-15".into(),
        published_at_basis: "FACT: invented fixture date".into(),
        pages: 3,
        bytes: 1,
        sha256: fmine_core::sha256(reporter.as_bytes()).to_hex(),
    };
    let keys = bind::keys("invented-jv", PERIOD, metric);
    let physical = ";perimeter=invented-jv;members=mine-a+mine-b;valid=2030-01-01/2030-12-31;scope_page=2;scope_table=Scope;scope_row=Operations;scope_col=2030";
    let definition = format!(
        ";metal=gold;denominator=sold;allocation=by_product;credits=separate_line;inventory=included;ga={ga};reclamation=operating_only;accounting=as_reported;definition_page=3;definition_table=Cost policy;definition_row=Cost definition;definition_col=2030"
    );
    let row = |key: String, token: &str, mode: &str, unit: &str, extra: &str| Accepted {
        locator: Locator {
            doc: document.file.clone(),
            page: 1,
            table: "Invented costs".into(),
            row: key.clone(),
            col: "2030FY".into(),
        },
        doc_sha256: document.sha256.clone(),
        token: token.into(),
        value: parse_token(token).unwrap(),
        quantity: key,
        unit: unit.into(),
        basis: format!("{mode}{physical}{extra}"),
        period: "2030FY".into(),
        readers: vec!["invented-reader-a".into(), "invented-reader-b".into()],
    };
    let mut rows = vec![
        row(
            keys[1].clone(),
            amount,
            "attributable",
            "USD_m",
            &definition,
        ),
        row(keys.last().unwrap().clone(), share, "equity", "percent", ""),
    ];
    if let Some((field, token, target)) = adjustment {
        rows.push(row(
            format!(
                "mirror_invented_jv_2030fy_{}_{}_adjustment_attr_usd_m",
                metric.as_str(),
                field.as_str()
            ),
            token,
            "attributable",
            "USD_m",
            &format!("{definition};target={target}"),
        ));
    }
    (document, rows)
}

fn bound(reporter: &str, metric: CostMetric, source: &(DocumentRow, Vec<Accepted>)) -> BoundCosts {
    bind::bind(
        "invented-jv",
        PERIOD,
        metric,
        reporter,
        &source.0,
        &source.1,
        EvidenceClass::Assumed,
    )
    .unwrap()
}

fn pair(metric: CostMetric, bridge: bool) -> Vec<Claim<CostReading>> {
    let alpha = fixture(
        "alpha",
        metric,
        "60",
        "60",
        "included",
        bridge.then_some((DefinitionField::Ga, "0", "included")),
    );
    let beta = fixture(
        "beta",
        metric,
        if bridge { "36" } else { "40" },
        "40",
        if bridge { "excluded" } else { "included" },
        bridge.then_some((DefinitionField::Ga, "4", "included")),
    );
    [
        bound("alpha", metric, &alpha).inputs,
        bound("beta", metric, &beta).inputs,
    ]
    .concat()
}

#[test]
fn cost_same_definition_has_exact_common_interval_and_canonical_ancestry() {
    for metric in [
        CostMetric::CashCosts,
        CostMetric::Aisc,
        CostMetric::CostOfSales,
    ] {
        let inputs = pair(metric, false);
        let result = compare(&inputs).unwrap();
        assert_eq!(result.status(), "consistent");
        let common = result.common.unwrap();
        assert_eq!(common.value.interval.lo, Rat::new(595, 6).unwrap());
        assert_eq!(common.value.interval.hi, Rat::new(605, 6).unwrap());
        assert_eq!(common.evidence, EvidenceClass::Assumed);
        assert!(common.check_id().is_ok());
        assert_eq!(common.value_tag(), common.value.canonical_tag());
        for (_, aligned) in &result.aligned {
            assert!(common.parents.contains(&aligned.claim_id));
            assert_eq!(aligned.parents.len(), 1);
        }
        assert_eq!(result.inputs, inputs);
    }
}

#[test]
fn cost_cited_two_sided_signed_bridges_close_with_exact_rounding() {
    let inputs = pair(CostMetric::Aisc, true);
    let report = compare(&inputs).unwrap();
    assert_eq!(report.status(), "explained");
    assert_eq!(report.layers, vec![DefinitionField::Ga]);
    assert_eq!(
        report.common.as_ref().unwrap().value.interval.lo,
        Rat::new(295, 3).unwrap()
    );
    assert_eq!(
        report.common.as_ref().unwrap().value.interval.hi,
        Rat::new(305, 3).unwrap()
    );
    assert!(report.aligned.iter().all(|(_, c)| c.parents.len() == 2));
    let mut reversed = inputs.clone();
    reversed.reverse();
    assert_eq!(compare(&reversed).unwrap(), report);
    let alpha = fixture(
        "alpha",
        CostMetric::CashCosts,
        "60",
        "60",
        "included",
        Some((DefinitionField::Ga, "-6", "excluded")),
    );
    let beta = fixture(
        "beta",
        CostMetric::CashCosts,
        "36",
        "40",
        "excluded",
        Some((DefinitionField::Ga, "0", "excluded")),
    );
    let signed = compare(
        &[
            bound("alpha", CostMetric::CashCosts, &alpha).inputs,
            bound("beta", CostMetric::CashCosts, &beta).inputs,
        ]
        .concat(),
    )
    .unwrap();
    assert_eq!(signed.status(), "explained");
    assert_eq!(
        signed.common.unwrap().value.interval.lo,
        Rat::new(265, 3).unwrap()
    );
}

#[test]
fn cost_disjoint_assertions_carry_exact_separation_and_never_a_common_claim() {
    let alpha = fixture("alpha", CostMetric::Aisc, "60", "60", "included", None);
    let beta = fixture("beta", CostMetric::Aisc, "80", "40", "included", None);
    let result = compare(
        &[
            bound("alpha", CostMetric::Aisc, &alpha).inputs,
            bound("beta", CostMetric::Aisc, &beta).inputs,
        ]
        .concat(),
    )
    .unwrap();
    assert_eq!(result.status(), "conflict");
    assert!(result.common.is_none());
    let proof = result.separation.unwrap();
    assert_eq!(proof.lower_reporter, "beta");
    assert_eq!(proof.lower, Rat::new(795, 4).unwrap());
    assert_eq!(proof.upper, Rat::new(605, 6).unwrap());
    assert!(proof.lower.checked_cmp(proof.upper).unwrap().is_gt());
}

#[test]
fn cost_missing_layers_definitions_and_citations_never_select_a_survivor() {
    let mut rows = pair(CostMetric::Aisc, true);
    rows.retain(|r| r.value.reporter != "alpha" || r.value.layer.is_none());
    assert_eq!(compare(&rows).unwrap_err().code, "FMINE-REF-104");
    let source = fixture("alpha", CostMetric::Aisc, "60", "60", "included", None);
    for fault in 0..7 {
        let mut altered = source.clone();
        match fault {
            0 => altered.1[0].basis = altered.1[0].basis.replace(";credits=separate_line", ""),
            1 => {
                altered.1[0].basis = altered.1[0]
                    .basis
                    .replace("definition_page=3", "definition_page=4")
            }
            2 => altered.1[0].unit = "USD_per_oz".into(),
            3 => altered.1[0].doc_sha256 = "11".repeat(32),
            4 => {
                altered.1[1].basis = altered.1[1]
                    .basis
                    .replace("members=mine-a+mine-b", "members=mine-a")
            }
            5 => altered.1.push(altered.1[0].clone()),
            _ => {
                let mut full = altered.1[0].clone();
                full.quantity = bind::keys("invented-jv", PERIOD, CostMetric::Aisc)[0].clone();
                full.basis = full.basis.replacen("attributable", "100%", 1);
                altered.1.push(full);
            }
        }
        assert!(
            bind::bind(
                "invented-jv",
                PERIOD,
                CostMetric::Aisc,
                "alpha",
                &altered.0,
                &altered.1,
                EvidenceClass::Assumed
            )
            .is_err(),
            "fault {fault}"
        );
    }
    let beta = fixture("beta", CostMetric::Aisc, "40", "40", "excluded", None);
    assert_eq!(
        compare(
            &[
                bound("alpha", CostMetric::Aisc, &source).inputs,
                bound("beta", CostMetric::Aisc, &beta).inputs
            ]
            .concat()
        )
        .unwrap_err()
        .code,
        "FMINE-REF-104"
    );
}

#[test]
fn accounting_bridge_is_only_admitted_for_cost_of_sales() {
    for metric in [
        CostMetric::Aisc,
        CostMetric::CashCosts,
        CostMetric::CostOfSales,
    ] {
        let source = fixture(
            "alpha",
            metric,
            "60",
            "60",
            "included",
            Some((DefinitionField::Accounting, "-6", "historical_cost")),
        );
        let result = bind::bind(
            "invented-jv",
            PERIOD,
            metric,
            "alpha",
            &source.0,
            &source.1,
            EvidenceClass::Assumed,
        );
        assert_eq!(result.is_ok(), metric == CostMetric::CostOfSales);
    }
    for field in [DefinitionField::Denominator, DefinitionField::Allocation] {
        let source = fixture(
            "alpha",
            CostMetric::CostOfSales,
            "60",
            "60",
            "included",
            Some((field, "0", "sold")),
        );
        assert!(
            bind::bind(
                "invented-jv",
                PERIOD,
                CostMetric::CostOfSales,
                "alpha",
                &source.0,
                &source.1,
                EvidenceClass::Assumed
            )
            .is_err()
        );
    }
}

#[test]
fn mutated_and_stale_cost_claims_and_cross_asset_bridges_are_rejected() {
    let inputs = pair(CostMetric::Aisc, true);
    for fault in 0..4 {
        let mut changed = inputs.clone();
        match fault {
            0 => changed[0].value.value.interval.lo = Rat::from_integer(0),
            1 => changed[0].stale = true,
            2 => changed[0].value.value.asset = "another-jv".into(),
            _ => changed[0].anchors.clear(),
        }
        assert!(compare(&changed).is_err(), "fault {fault}");
    }
}

#[test]
fn cost_metal_and_raw_reader_evidence_are_bound_without_cross_metal_bridges() {
    let alpha = fixture("alpha", CostMetric::Aisc, "60", "60", "included", None);
    let mut beta = fixture("beta", CostMetric::Aisc, "40", "40", "included", None);
    beta.1[0].basis = beta.1[0].basis.replace("metal=gold", "metal=copper");
    assert_eq!(
        compare(
            &[
                bound("alpha", CostMetric::Aisc, &alpha).inputs,
                bound("beta", CostMetric::Aisc, &beta).inputs
            ]
            .concat()
        )
        .unwrap_err()
        .code,
        "FMINE-REF-104"
    );
    let mut mutated = alpha.clone();
    mutated.1[0].token = "600".into();
    assert_eq!(
        bind::bind(
            "invented-jv",
            PERIOD,
            CostMetric::Aisc,
            "alpha",
            &mutated.0,
            &mutated.1,
            EvidenceClass::Assumed
        )
        .unwrap_err()
        .code,
        "FMINE-REF-202"
    );
    let original = bound("alpha", CostMetric::Aisc, &alpha);
    let mut changed = alpha;
    changed.1[0].readers[1] = "independent-reader-c".into();
    let revised = bound("alpha", CostMetric::Aisc, &changed);
    assert_eq!(
        original.inputs[0].value.value.interval,
        revised.inputs[0].value.value.interval
    );
    assert_ne!(original.inputs[0].claim_id, revised.inputs[0].claim_id);
}
