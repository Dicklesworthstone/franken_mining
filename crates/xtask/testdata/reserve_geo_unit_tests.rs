use super::*;
use fmine_registry::transcriptions::{Locator, parse_token};

fn fixture() -> (TableIdentityRegistry, Vec<Accepted>, Vec<DocumentRow>) {
    let documents: Vec<_> = ["Report.pdf", "Alternative.pdf"]
        .into_iter()
        .enumerate()
        .map(|(i, file)| DocumentRow {
            file: file.into(),
            dir: "synthetic".into(),
            kind: "mda".into(),
            url: format!("https://example.test/{file}"),
            url_basis: "synthetic".into(),
            published_at: format!("203{}-02-01", i + 1),
            published_at_basis: "FACT: invented fixture".into(),
            pages: 2,
            bytes: 100,
            sha256: if i == 0 { "a" } else { "b" }.repeat(64),
        })
        .collect();
    let mut rows = Vec::new();
    let mut readings = Vec::new();
    let mut add =
        |doc: &DocumentRow, quantity: &str, token: &str, unit: &str, basis: &str, period: &str| {
            readings.push(Accepted {
                locator: Locator {
                    doc: doc.file.clone(),
                    page: 1,
                    table: "Invented GEO".into(),
                    row: quantity.into(),
                    col: period.into(),
                },
                doc_sha256: doc.sha256.clone(),
                token: token.into(),
                value: parse_token(token).unwrap(),
                quantity: quantity.into(),
                unit: unit.into(),
                basis: basis.into(),
                period: period.into(),
                readers: vec!["reader_a".into(), "reader_b".into()],
            });
        };
    for (id, metal, unit, token, doc_index) in [
        ("gold", "gold", "moz", "10.0", 0),
        ("copper", "copper", "mlb", "1000", 0),
        ("geo", "geo", "moz", "12.0", 0),
        ("alternative", "geo", "moz", "99.0", 1),
    ] {
        let quoted = metal == "geo";
        let year = 2030 + doc_index as u16;
        let row = ReserveDeclaration {
            id: id.into(),
            doc: documents[doc_index].file.clone(),
            quantity: id.into(),
            subject: "invented_issuer".into(),
            item: "net_change".into(),
            reserve_year: year,
            period: format!("{year}FY"),
            metal: metal.into(),
            unit: unit.into(),
            ownership: "full".into(),
            support: "contained".into(),
            convention: "closed".into(),
            deck: if quoted { "quoted" } else { "unstated" }.into(),
            deck_period: quoted.then(|| format!("{year}FY")),
            deck_basis: quoted.then(|| "reserve_prices".into()),
            gold_price_quantity: quoted.then(|| "gold_price".into()),
            copper_price_quantity: quoted.then(|| "copper_price".into()),
        };
        add(
            &documents[doc_index],
            id,
            token,
            unit,
            &row.basis(),
            &row.period,
        );
        if quoted {
            add(
                &documents[doc_index],
                "gold_price",
                "1500",
                "usd_per_oz",
                "reserve_prices",
                &row.period,
            );
            add(
                &documents[doc_index],
                "copper_price",
                if doc_index == 0 { "3.00" } else { "4.50" },
                "usd_per_lb",
                "reserve_prices",
                &row.period,
            );
        }
        rows.push(row);
    }
    (
        TableIdentityRegistry {
            issuer: "synthetic".into(),
            identities: Vec::new(),
            reserve_rows: rows,
            geo_checks: vec![GeoCheckDeclaration {
                id: "conversion".into(),
                gold_row: "gold".into(),
                copper_row: "copper".into(),
                geo_row: "geo".into(),
                candidate_deck_rows: vec!["alternative".into()],
            }],
            cutoffs: Vec::new(),
        },
        readings,
        documents,
    )
}

fn token(readings: &mut [Accepted], doc: &str, quantity: &str, token: &str) {
    let reading = readings
        .iter_mut()
        .find(|a| a.locator.doc == doc && a.quantity == quantity)
        .unwrap();
    reading.token = token.into();
    reading.value = parse_token(token).unwrap();
}

#[test]
fn cited_conversion_and_alternative_deck_findings_do_not_replace_the_stated_deck() {
    let (registry, mut readings, documents) = fixture();
    let report = super::super::assess(&registry, &readings, &documents);
    let checked = &report.geo_checks[0];
    assert!(checked.refusal.is_none());
    assert_eq!(checked.declared.as_ref().unwrap().1.fit, Fit::Proper);
    let stated = checked.declared.as_ref().unwrap().0.clone();
    assert_eq!(
        checked.discovery.as_ref().unwrap().finding,
        DeckFinding::Found(stated.clone())
    );
    token(&mut readings, "Report.pdf", "geo", "13.0");
    let report = super::super::assess(&registry, &readings, &documents);
    let checked = &report.geo_checks[0];
    assert_eq!(checked.declared.as_ref().unwrap().0, stated);
    assert_eq!(checked.declared.as_ref().unwrap().1.fit, Fit::Empty);
    assert_eq!(
        checked.discovery.as_ref().unwrap().finding,
        DeckFinding::Found(checked.decks[1].deck.as_ref().unwrap().key.clone().unwrap())
    );
    assert_eq!(
        checked.amounts[2].source.as_ref().unwrap().source.token,
        "13.0"
    );
    assert!(checked.to_json().contains("issuer's actual calculation"));
}

#[test]
fn equal_ratios_remain_ambiguous_and_identical_parameters_keep_every_source_provider() {
    let (mut registry, mut readings, documents) = fixture();
    token(&mut readings, "Alternative.pdf", "gold_price", "3000");
    token(&mut readings, "Alternative.pdf", "copper_price", "6.00");
    let report = super::super::assess(&registry, &readings, &documents);
    assert!(
        matches!(&report.geo_checks[0].discovery.as_ref().unwrap().finding, DeckFinding::Ambiguous(keys) if keys.len() == 2)
    );
    token(&mut readings, "Alternative.pdf", "gold_price", "1500");
    token(&mut readings, "Alternative.pdf", "copper_price", "3.00");
    registry.reserve_rows[3].deck_period = Some("2030FY".into());
    for reading in &mut readings {
        if reading.locator.doc == "Alternative.pdf" && reading.quantity.ends_with("price") {
            reading.period = "2030FY".into();
        }
    }
    let report = super::super::assess(&registry, &readings, &documents);
    let checked = &report.geo_checks[0];
    assert_eq!(checked.decks.len(), 2);
    assert_eq!(
        checked.decks[0].deck.as_ref().unwrap().key,
        checked.decks[1].deck.as_ref().unwrap().key
    );
    assert_eq!(checked.discovery.as_ref().unwrap().fits.len(), 4);
    assert_ne!(
        checked.decks[0].document.sha256,
        checked.decks[1].document.sha256
    );
    assert!(checked.to_markdown().contains("Alternative.pdf"));
}

#[test]
fn candidate_price_availability_is_independent_of_its_unrelated_reserve_amount() {
    let (registry, readings, documents) = fixture();
    let amounts_missing: Vec<_> = readings
        .iter()
        .filter(|a| a.quantity != "alternative")
        .cloned()
        .collect();
    let report = super::super::assess(&registry, &amounts_missing, &documents);
    assert!(
        report
            .refused
            .iter()
            .any(|r| r.declaration.id == "alternative")
    );
    assert!(report.geo_checks[0].refusal.is_none());
    assert!(report.geo_checks[0].discovery.is_some());
    let prices_missing: Vec<_> = readings
        .iter()
        .filter(|a| !(a.locator.doc == "Alternative.pdf" && a.quantity == "copper_price"))
        .cloned()
        .collect();
    let report = super::super::assess(&registry, &prices_missing, &documents);
    let checked = &report.geo_checks[0];
    assert_eq!(checked.decks.len(), 2);
    assert!(checked.declared.is_some());
    assert!(checked.discovery.is_none());
    assert!(checked.refusal.as_ref().unwrap().contains("alternative"));
    assert!(!checked.decks[1].evidence.is_empty());
}

#[test]
fn missing_or_unusable_amounts_and_prices_never_create_a_unique_finding() {
    let (registry, readings, documents) = fixture();
    for mutation in 0..6 {
        let mut broken = readings.clone();
        match mutation {
            0 => broken.retain(|a| a.quantity != "gold"),
            1 => broken
                .iter_mut()
                .find(|a| a.quantity == "copper")
                .unwrap()
                .readers
                .pop()
                .map(|_| ())
                .unwrap(),
            2 => {
                broken
                    .iter_mut()
                    .find(|a| a.locator.doc == "Alternative.pdf" && a.quantity == "gold_price")
                    .unwrap()
                    .unit = "usd_per_lb".into()
            }
            3 => {
                broken
                    .iter_mut()
                    .find(|a| a.locator.doc == "Alternative.pdf" && a.quantity == "gold_price")
                    .unwrap()
                    .period = "2029FY".into()
            }
            4 => token(&mut broken, "Report.pdf", "gold_price", "0"),
            _ => {
                let mut competitor = broken
                    .iter()
                    .find(|a| a.quantity == "copper")
                    .unwrap()
                    .clone();
                competitor.locator.row = "competing copper".into();
                broken.push(competitor);
            }
        }
        let report = super::super::assess(&registry, &broken, &documents);
        let checked = &report.geo_checks[0];
        assert!(checked.refusal.is_some(), "mutation {mutation}");
        assert!(checked.discovery.is_none(), "mutation {mutation}");
    }
}

#[test]
fn alternate_convention_does_not_override_the_declared_document_convention() {
    let (mut registry, mut readings, documents) = fixture();
    registry.geo_checks[0].candidate_deck_rows.clear();
    token(&mut readings, "Report.pdf", "geo", "20.06");
    token(&mut readings, "Report.pdf", "gold_price", "1000");
    token(&mut readings, "Report.pdf", "copper_price", "10");
    let closed = super::super::assess(&registry, &readings, &documents);
    assert_eq!(
        closed.geo_checks[0].declared.as_ref().unwrap().1.fit,
        Fit::Points
    );
    for row in &mut registry.reserve_rows[..3] {
        row.convention = "half_up".into();
    }
    let open = super::super::assess(&registry, &readings, &documents);
    let checked = &open.geo_checks[0];
    assert_eq!(checked.declared.as_ref().unwrap().1.fit, Fit::Empty);
    assert!(
        checked
            .discovery
            .as_ref()
            .unwrap()
            .conventions
            .contains(&Convention::Truncate)
    );
    assert_eq!(checked.declared_convention, "half_up");
}

#[test]
fn geo_checks_are_order_invariant_and_bind_candidate_evidence_and_membership() {
    let (mut registry, mut readings, mut documents) = fixture();
    let before = super::super::assess(&registry, &readings, &documents);
    registry.reserve_rows.reverse();
    readings.reverse();
    documents.reverse();
    for reading in &mut readings {
        reading.readers.reverse();
    }
    assert_eq!(
        before.to_json(),
        super::super::assess(&registry, &readings, &documents).to_json()
    );
    documents
        .iter_mut()
        .find(|d| d.file == "Alternative.pdf")
        .unwrap()
        .url_basis = "changed citation metadata".into();
    let changed = super::super::assess(&registry, &readings, &documents);
    assert_ne!(
        before.geo_checks[0].digest(),
        changed.geo_checks[0].digest()
    );
    registry.geo_checks[0]
        .candidate_deck_rows
        .push("unknown".into());
    let invalid = super::super::assess(&registry, &readings, &documents);
    assert!(invalid.registry_refusal.is_some());
    registry.geo_checks[0]
        .candidate_deck_rows
        .push("another_unknown".into());
    assert_ne!(
        invalid.digest(),
        super::super::assess(&registry, &readings, &documents).digest()
    );
}
