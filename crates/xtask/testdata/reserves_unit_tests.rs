use super::*;
use fmine_registry::transcriptions::{Locator, parse_token};

fn fixture() -> (TableIdentityRegistry, Vec<Accepted>, Vec<DocumentRow>) {
    let mut rows = Vec::new();
    let mut readings = Vec::new();
    let mut documents = Vec::new();
    for (id, kind, date, value, price, hash) in [
        ("old", "mda", "2031-02-01", "10.0", "1500", 'a'),
        ("new", "aif", "2031-03-01", "12.0", "1500", 'b'),
        ("deck", "investor_deck", "2031-04-01", "13.0", "1600", 'c'),
    ] {
        let doc = DocumentRow {
            file: format!("{id}.pdf"),
            dir: "synthetic".into(),
            kind: kind.into(),
            url: format!("https://example.test/{id}.pdf"),
            url_basis: "synthetic".into(),
            published_at: date.into(),
            published_at_basis: "FACT: invented fixture".into(),
            pages: 2,
            bytes: 100,
            sha256: hash.to_string().repeat(64),
        };
        let row = ReserveDeclaration {
            id: id.into(),
            doc: doc.file.clone(),
            quantity: "reserve_change".into(),
            subject: "invented_issuer".into(),
            item: "net_change".into(),
            reserve_year: 2030,
            period: "2030FY".into(),
            metal: "gold".into(),
            unit: "moz".into(),
            ownership: "full".into(),
            support: "contained".into(),
            convention: "closed".into(),
            deck: "quoted".into(),
            deck_period: Some("2030FY".into()),
            deck_basis: Some("reserve_prices".into()),
            gold_price_quantity: Some("gold_price".into()),
            copper_price_quantity: None,
        };
        for (quantity, token, unit, basis) in [
            ("reserve_change", value, "moz", row.basis()),
            ("gold_price", price, "usd_per_oz", "reserve_prices".into()),
        ] {
            readings.push(Accepted {
                locator: Locator {
                    doc: doc.file.clone(),
                    page: 1,
                    table: "Invented reserves".into(),
                    row: quantity.into(),
                    col: "2030FY".into(),
                },
                doc_sha256: doc.sha256.clone(),
                token: token.into(),
                value: parse_token(token).unwrap(),
                quantity: quantity.into(),
                unit: unit.into(),
                basis,
                period: "2030FY".into(),
                readers: vec!["reader_a".into(), "reader_b".into()],
            });
        }
        rows.push(row);
        documents.push(doc);
    }
    (
        TableIdentityRegistry {
            issuer: "synthetic".into(),
            identities: Vec::new(),
            reserve_rows: rows,
            geo_checks: Vec::new(),
            cutoffs: Vec::new(),
        },
        readings,
        documents,
    )
}

fn set_token(readings: &mut [Accepted], doc: &str, quantity: &str, token: &str) {
    let row = readings
        .iter_mut()
        .find(|a| a.locator.doc == doc && a.quantity == quantity)
        .unwrap();
    row.token = token.into();
    row.value = parse_token(token).unwrap();
}

#[test]
fn latest_filed_row_wins_and_later_deck_never_replaces_it() {
    let (registry, readings, docs) = fixture();
    let report = assess(&registry, &readings, &docs);
    assert!(report.refused.is_empty());
    assert!(report.refused_input_digest.is_none());
    assert_eq!(report.rows.len(), 3);
    assert_eq!(report.selections[0].status, "current");
    assert_eq!(report.selections[0].current_id.as_deref(), Some("new"));
    assert_eq!(
        report.selections[0].latest_filed_date.as_deref(),
        Some("2031-03-01")
    );
    assert!(
        report
            .comparisons
            .iter()
            .any(|c| c.left == "old" && c.right == "new" && c.status == "changed")
    );
    assert!(
        report
            .comparisons
            .iter()
            .any(|c| c.right == "deck" && c.status == "deck_differs")
    );
    for row in &report.rows {
        assert_eq!(row.evidence_class(), EvidenceClass::ISSUER);
    }
    let old = report
        .rows
        .iter()
        .find(|r| r.declaration.id == "old")
        .unwrap();
    let mut earlier = registry.clone();
    earlier.reserve_rows.retain(|r| r.id == "old");
    assert_eq!(
        old.digest(),
        assess(&earlier, &readings, &docs).rows[0].digest()
    );
}

#[test]
fn missing_new_value_or_deck_price_blocks_silent_old_filing_fallback() {
    let (registry, readings, docs) = fixture();
    for quantity in ["reserve_change", "gold_price"] {
        let mut incomplete = readings.clone();
        incomplete.retain(|a| !(a.locator.doc == "new.pdf" && a.quantity == quantity));
        let report = assess(&registry, &incomplete, &docs);
        assert_eq!(report.rows.len(), 2);
        assert_eq!(report.selections[0].status, "latest_unavailable");
        assert_eq!(report.selections[0].current_id, None);
        assert_eq!(report.selections[0].unavailable, ["new"]);
        assert_eq!(report.refused[0].declaration.id, "new");
        assert!(!report.refused[0].evidence.is_empty());
    }
}

#[test]
fn same_date_filings_remain_ambiguous_even_with_one_unusable_row() {
    let (registry, mut readings, mut docs) = fixture();
    docs[0].published_at = docs[1].published_at.clone();
    for missing in [false, true] {
        if missing {
            readings.retain(|a| !(a.locator.doc == "new.pdf" && a.quantity == "gold_price"));
        }
        let report = assess(&registry, &readings, &docs);
        assert_eq!(report.selections[0].status, "ambiguous_latest");
        assert_eq!(report.selections[0].current_id, None);
        assert_eq!(report.selections[0].candidates, ["new", "old"]);
        assert_eq!(report.selections[0].unavailable.len(), usize::from(missing));
    }
}

#[test]
fn deck_change_updates_current_without_claiming_a_reserve_restatement() {
    let (mut registry, mut readings, docs) = fixture();
    set_token(&mut readings, "new.pdf", "gold_price", "1600");
    let report = assess(&registry, &readings, &docs);
    assert_eq!(report.selections[0].current_id.as_deref(), Some("new"));
    let pair = report
        .comparisons
        .iter()
        .find(|c| c.left == "old" && c.right == "new")
        .unwrap();
    assert_eq!(pair.status, "deck_differs");
    assert!(pair.overlap.is_none());
    for row in &mut registry.reserve_rows {
        row.deck = "unstated".into();
        row.deck_period = None;
        row.deck_basis = None;
        row.gold_price_quantity = None;
        row.copper_price_quantity = None;
    }
    let unknown = assess(&registry, &readings, &docs);
    assert!(
        unknown
            .comparisons
            .iter()
            .all(|c| c.status == "deck_unstated" && c.overlap.is_none())
    );
}

#[test]
fn half_open_tie_bounds_do_not_report_false_compatibility() {
    let (mut registry, mut readings, docs) = fixture();
    registry.reserve_rows.retain(|r| r.id != "deck");
    set_token(&mut readings, "old.pdf", "reserve_change", "1");
    set_token(&mut readings, "new.pdf", "reserve_change", "2");
    let closed = assess(&registry, &readings, &docs);
    assert_eq!(closed.comparisons[0].status, "rounding_compatible");
    assert_eq!(
        closed.comparisons[0].overlap.unwrap().lo,
        Rat::new(3, 2).unwrap()
    );
    for r in &mut registry.reserve_rows {
        r.convention = "half_up".into();
    }
    let open = assess(&registry, &readings, &docs);
    assert_eq!(open.comparisons[0].status, "changed");
    assert!(open.comparisons[0].overlap.is_none());
}

#[test]
fn unsupported_sources_are_refused_and_deck_only_has_no_current_row() {
    let (mut registry, readings, docs) = fixture();
    for kind in ["partner_filing", "technical_report"] {
        let mut secondary = docs.clone();
        secondary[0].kind = kind.into();
        secondary[1].kind = kind.into();
        let report = assess(&registry, &readings, &secondary);
        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.refused.len(), 2);
        assert_eq!(report.selections[0].status, "no_filed_row");
        assert!(report.refused.iter().all(|r| r.reason.contains(kind)));
    }
    registry.reserve_rows.retain(|r| r.id == "deck");
    assert_eq!(
        assess(&registry, &readings, &docs).selections[0].status,
        "no_filed_row"
    );
}

#[test]
fn incompatible_basis_stays_separate_and_equivalent_share_spellings_share_a_key() {
    let (mut registry, mut readings, docs) = fixture();
    registry.reserve_rows.retain(|r| r.id != "deck");
    registry.reserve_rows[0].ownership = "attributable:6/8".into();
    registry.reserve_rows[1].ownership = "attributable:3/4".into();
    for row in &registry.reserve_rows {
        readings
            .iter_mut()
            .find(|a| a.locator.doc == row.doc && a.quantity == row.quantity)
            .unwrap()
            .basis = row.basis();
    }
    let report = assess(&registry, &readings, &docs);
    assert_eq!(report.selections.len(), 1);
    assert_eq!(
        report.selections[0].economic_key.ownership,
        "attributable:3/4"
    );
    registry.reserve_rows[1].support = "recoverable".into();
    readings
        .iter_mut()
        .find(|a| a.locator.doc == "new.pdf" && a.quantity == "reserve_change")
        .unwrap()
        .basis = registry.reserve_rows[1].basis();
    let split = assess(&registry, &readings, &docs);
    assert_eq!(split.selections.len(), 2);
    assert!(split.comparisons.is_empty());
}

#[test]
fn source_guards_reject_wrong_prices_units_periods_readers_and_metadata() {
    let (registry, readings, docs) = fixture();
    for mutation in 0..7 {
        let mut broken = readings.clone();
        let row = broken
            .iter_mut()
            .find(|a| a.locator.doc == "new.pdf" && a.quantity == "gold_price")
            .unwrap();
        match mutation {
            0 => row.unit = "usd_per_lb".into(),
            1 => row.period = "2029FY".into(),
            2 => row.basis = "spot_price".into(),
            3 => row.readers = vec!["reader_a".into()],
            4 => row.doc_sha256 = "f".repeat(64),
            5 => {
                row.token = "0".into();
                row.value = parse_token("0").unwrap();
            }
            _ => row.value = parse_token("99").unwrap(),
        }
        let report = assess(&registry, &broken, &docs);
        assert_eq!(report.selections[0].status, "latest_unavailable");
        assert_eq!(report.refused.len(), 1);
    }
}

#[test]
fn inputs_are_order_invariant_and_rejected_metadata_remains_digest_bound() {
    let (mut registry, mut readings, mut docs) = fixture();
    readings.retain(|a| !(a.locator.doc == "new.pdf" && a.quantity == "gold_price"));
    let report = assess(&registry, &readings, &docs);
    registry.reserve_rows.reverse();
    readings.reverse();
    docs.reverse();
    for a in &mut readings {
        a.readers.reverse();
    }
    assert_eq!(
        report.to_json(),
        assess(&registry, &readings, &docs).to_json()
    );
    docs.iter_mut()
        .find(|d| d.file == "new.pdf")
        .unwrap()
        .url_basis = "changed declared evidence".into();
    assert_ne!(
        report.digest(),
        assess(&registry, &readings, &docs).digest()
    );
}

#[test]
fn registry_refusal_binds_invalid_declarations_documents_and_raw_and_admitted_values() {
    let (mut registry, readings, docs) = fixture();
    registry.reserve_rows[0].ownership = "attributable:4/3".into();
    let before = assess(&registry, &readings, &docs);
    assert!(before.registry_refusal.is_some());
    assert!(before.refused_input_digest.is_some());
    for mutation in 0..4 {
        let (mut registry, mut readings, mut docs) =
            (registry.clone(), readings.clone(), docs.clone());
        match mutation {
            0 => registry.reserve_rows[0].ownership = "attributable:2/1".into(),
            1 => docs[0].url_basis = "changed rejected source metadata".into(),
            2 => readings[0].value = parse_token("99.0").unwrap(),
            _ => readings[0].token = "(10.0)".into(),
        }
        let after = assess(&registry, &readings, &docs);
        assert_eq!(before.registry_refusal, after.registry_refusal);
        assert_ne!(before.refused_input_digest, after.refused_input_digest);
        assert_ne!(before.digest(), after.digest());
    }
}

#[test]
fn registry_refusal_fingerprint_includes_identities_and_preserves_duplicates_and_order() {
    use fmine_registry::table_identities::{IdentityKind, TableIdentity, Term, TermRole};
    let (mut registry, mut readings, mut docs) = fixture();
    registry.reserve_rows[0].ownership = "attributable:4/3".into();
    registry.identities = ["sum_a", "sum_b"]
        .into_iter()
        .map(|id| TableIdentity {
            id: id.into(),
            doc: "old.pdf".into(),
            family: "invented_sum".into(),
            kind: IdentityKind::Sum,
            total: "total".into(),
            total_period: "2030FY".into(),
            unit: "moz".into(),
            basis: "gold;full;contained".into(),
            construction: "rounded_once".into(),
            convention: "closed".into(),
            replacement: None,
            terms: ["part_a", "part_b"]
                .into_iter()
                .map(|quantity| Term {
                    quantity: quantity.into(),
                    period: "2030FY".into(),
                    sign: 1,
                    role: TermRole::Member,
                })
                .collect(),
        })
        .collect();
    let before = assess(&registry, &readings, &docs);
    registry.identities.reverse();
    for identity in &mut registry.identities {
        identity.terms.reverse();
    }
    registry.reserve_rows.reverse();
    readings.reverse();
    for reading in &mut readings {
        reading.readers.reverse();
    }
    docs.reverse();
    assert_eq!(
        before.to_json(),
        assess(&registry, &readings, &docs).to_json()
    );
    let mut changed = registry.clone();
    changed.identities[0].total = "changed_total".into();
    let after = assess(&changed, &readings, &docs);
    assert_eq!(before.registry_refusal, after.registry_refusal);
    assert_ne!(before.digest(), after.digest());
    for duplicate in 0..6 {
        let (mut registry, mut readings, mut docs) =
            (registry.clone(), readings.clone(), docs.clone());
        match duplicate {
            0 => registry.reserve_rows.push(registry.reserve_rows[0].clone()),
            1 => registry.identities.push(registry.identities[0].clone()),
            2 => {
                let identity = &mut registry.identities[0];
                identity.terms.push(identity.terms[0].clone());
            }
            3 => docs.push(docs[0].clone()),
            4 => readings.push(readings[0].clone()),
            _ => {
                let reading = &mut readings[0];
                reading.readers.push(reading.readers[0].clone());
            }
        }
        assert_ne!(
            before.refused_input_digest,
            assess(&registry, &readings, &docs).refused_input_digest
        );
    }
}

#[test]
fn geo_requires_both_same_document_prices_and_renders_safe_source_labels() {
    let (mut registry, mut readings, docs) = fixture();
    registry.reserve_rows.retain(|r| r.id == "old");
    let row = &mut registry.reserve_rows[0];
    row.metal = "geo".into();
    assert!(registry.validate(&docs).is_err());
    registry.reserve_rows[0].copper_price_quantity = Some("copper_price".into());
    readings
        .iter_mut()
        .find(|a| a.locator.doc == "old.pdf" && a.quantity == "reserve_change")
        .unwrap()
        .basis = registry.reserve_rows[0].basis();
    let mut copper = readings
        .iter()
        .find(|a| a.locator.doc == "old.pdf" && a.quantity == "gold_price")
        .unwrap()
        .clone();
    copper.quantity = "copper_price".into();
    copper.locator.row = "<b>[label](bad)|`".into();
    copper.unit = "usd_per_lb".into();
    copper.token = "3.25".into();
    copper.value = parse_token("3.25").unwrap();
    readings.push(copper);
    let report = assess(&registry, &readings, &docs);
    assert!(report.refused.is_empty());
    assert!(report.rows[0].deck.copper_price.is_some());
    let md = report.to_markdown();
    assert!(!md.contains("<b>"));
    assert!(!md.contains("[label](bad)"));
    assert!(md.contains("&lt;b&gt;"));
    assert!(md.contains("&#124;"));
}
