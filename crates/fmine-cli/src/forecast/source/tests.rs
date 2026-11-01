use super::*;
use std::collections::BTreeMap;

use fmine_core::id::PrefixKind;
use fmine_io::xlsx::{MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx};
use fmine_public::key::public_claim;

/// Invented standalone source Claims; registered-workbook integration exercises the actual reader.
fn example(metric: PublicMetric) -> (PublicResolutionPolicy, ObjectId, SourceRow, KeyedCell) {
    let id = ObjectId::from_canonical_bytes(PrefixKind::Forecast, b"invented public target");
    let policy = PublicResolutionPolicy {
        issuer: "example".into(),
        metric,
        period: PublicPeriod::Year(2030),
        ownership: PublicOwnership::Full,
        members: BTreeMap::from([(id, "Example Mine".into())]),
    };
    let bytes = build_minimal_xlsx(&[MinimalSheet {
        name: "YTD".into(),
        hidden: false,
        cells: vec![("B5".into(), MinimalCell::Number(400.4))],
    }]);
    let workbook = Workbook::from_bytes("corpus/Original.xlsx", &bytes).expect("invented workbook");
    let registered = SourceRow {
        file: "Original.xlsx".into(),
        kind: "mine_stats".into(),
        quarter: "2030Q4".into(),
        url: "https://example.invalid/Original.xlsx".into(),
        url_basis: "W1 table".into(),
        bytes: bytes.len() as u64,
        sha256: Digest32(workbook.sha256).to_hex(),
        published_at: Some("2031-02-15".into()),
        published_at_basis: Some("FACT: invented test release date".into()),
    };
    let key = PublicKey {
        issuer: policy.issuer.clone(),
        subject: policy.members[&id].clone(),
        valid_period: parse_period_code("2030FY").expect("period"),
        reporting_vintage: Vintage {
            release: "2030Q4".into(),
            file_sha256: registered.sha256.clone(),
        },
        ownership_basis: OwnershipBasis::Consolidated100,
        scope: Scope::Site,
        unit: metric.unit().into(),
        line: LineKey {
            label: metric.field(policy.ownership).into(),
            occurrence: 0,
        },
    };
    let mut cell = public_claim(
        &workbook,
        "YTD",
        "B5",
        key,
        PublicValue::Number {
            stored: "400.4".into(),
            displayed: Ok("400".into()),
            under_status: None,
        },
    );
    if metric == PublicMetric::AiscUsdPerOz {
        cell.label_as_printed = Some("All-in sustaining costs ($/oz)".into());
    }
    (policy, id, registered, cell)
}

fn reseal(cell: &mut KeyedCell) {
    let c = &cell.claim;
    cell.claim = Claim::new(
        &cell.key.line.label,
        c.value.clone(),
        &canonical_value_tag(&cell.key, &c.value),
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
}

#[test]
fn original_displayed_number_and_source_identity_survive_the_receipt() {
    for metric in [PublicMetric::ProductionKoz, PublicMetric::AiscUsdPerOz] {
        let (policy, id, source, cell) = example(metric);
        let result = bind(&source, &policy, id, std::slice::from_ref(&cell)).expect("binding");
        assert_eq!(result.value, Rat::from_integer(400));
        assert_eq!(result.stored, "400.4");
        assert_eq!(result.source.claim.claim_id, cell.claim.claim_id);
        assert_eq!(
            result.available_at_ns,
            CivilDate::new(2031, 2, 16).expect("date").to_epoch_days() * NS_PER_DAY
        );
        assert_eq!(result.published_at, "2031-02-15");
        assert_eq!(result.published_at_basis, "FACT: invented test release date");
        let body = receipt_claim_body(
            &policy,
            id,
            &result.citation,
            &result.stored,
            &result.displayed,
            result.ownership_source,
            &cell.claim.anchors[0].artifact_path,
        )
        .expect("reconstructed original source");
        assert_eq!(body, cell.claim.canonical_body());
        assert_ne!(
            receipt_claim_body(
                &policy,
                id,
                &result.citation,
                &result.stored,
                "401",
                result.ownership_source,
                &cell.claim.anchors[0].artifact_path
            )
            .expect("different assertion"),
            body
        );
        assert_ne!(
            receipt_claim_body(
                &policy,
                id,
                &result.citation,
                "400.5",
                &result.displayed,
                result.ownership_source,
                &cell.claim.anchors[0].artifact_path
            )
            .expect("different stored token"),
            body
        );
    }
}

#[test]
fn frozen_target_fields_cannot_be_chosen_at_resolution() {
    let (policy, id, source, cell) = example(PublicMetric::AiscUsdPerOz);
    let mut variants = vec![policy.clone(); 5];
    variants[0].issuer = "different".into();
    variants[1].members.insert(id, "Other Mine".into());
    variants[2].period = PublicPeriod::Quarter { year: 2030, quarter: 4 };
    variants[3].metric = PublicMetric::ProductionKoz;
    variants[4].ownership = PublicOwnership::Attributable { num: 4, den: 5 };
    for changed in variants {
        assert!(bind(&source, &changed, id, std::slice::from_ref(&cell)).is_err());
    }
    let other = ObjectId::from_canonical_bytes(PrefixKind::Forecast, b"different forecast");
    assert!(bind(&source, &policy, other, std::slice::from_ref(&cell)).is_err());
    let mut later = cell.clone();
    later.key.reporting_vintage.release = "2031Q1".into();
    reseal(&mut later);
    assert_eq!(
        bind(&source, &policy, id, &[later]).expect_err("no comparative fallback").code,
        "FMINE-REF-201"
    );
    assert_eq!(
        bind(&source, &policy, id, &[cell.clone(), cell]).expect_err("ambiguous sources").code,
        "FMINE-REF-104"
    );
}

#[test]
fn mutated_source_or_wrong_scope_never_becomes_an_outcome() {
    let (policy, id, source, cell) = example(PublicMetric::ProductionKoz);
    let mut changed = cell.clone();
    if let PublicValue::Number { displayed, .. } = &mut changed.claim.value {
        *displayed = Ok("999".into());
    }
    assert_eq!(bind(&source, &policy, id, &[changed]).expect_err("mutated value").code, "FMINE-REF-507");
    for modify in [
        (|c: &mut KeyedCell| c.key.unit = "usd_per_oz".into()) as fn(&mut KeyedCell),
        |c| c.key.scope = Scope::Company,
        |c| c.key.line.occurrence = 1,
    ] {
        let mut changed = cell.clone();
        modify(&mut changed);
        reseal(&mut changed);
        assert_eq!(bind(&source, &policy, id, &[changed]).expect_err("wrong support").code, "FMINE-REF-105");
    }
    let mut changed = cell.clone();
    changed.claim.confidentiality = Confidentiality::internal();
    reseal(&mut changed);
    assert_eq!(bind(&source, &policy, id, &[changed]).expect_err("nonpublic").code, "FMINE-REF-301");
    let mut changed = cell;
    changed.claim.stale = true;
    assert_eq!(bind(&source, &policy, id, &[changed]).expect_err("stale").code, "FMINE-REF-502");
    let (policy, id, source, mut cost) = example(PublicMetric::AiscUsdPerOz);
    cost.printed_basis = OwnershipBasis::Unprinted;
    assert_eq!(bind(&source, &policy, id, &[cost]).expect_err("unprinted cost basis").code, "FMINE-REF-105");
}

#[test]
fn full_production_column_does_not_adopt_the_site_rows_attributable_share() {
    let (policy, id, source, mut cell) = example(PublicMetric::ProductionKoz);
    for printed in [
        OwnershipBasis::Unprinted,
        OwnershipBasis::Attributable { num: 3, den: 5, source: ShareSource::Printed },
    ] {
        cell.printed_basis = printed;
        assert_eq!(
            bind(&source, &policy, id, std::slice::from_ref(&cell)).expect("explicit 100% production column").value,
            Rat::from_integer(400)
        );
    }
}

#[test]
fn wholly_owned_attributable_production_still_binds_the_attributable_column() {
    let (mut policy, id, source, mut cell) = example(PublicMetric::ProductionKoz);
    policy.ownership = PublicOwnership::Attributable { num: 1, den: 1 };
    assert!(bind(&source, &policy, id, std::slice::from_ref(&cell)).is_err());
    cell.key.line.label = policy.metric.field(policy.ownership).into();
    reseal(&mut cell);
    let out = bind(&source, &policy, id, std::slice::from_ref(&cell)).expect("explicit attributable column at a wholly owned site");
    assert_eq!(out.ownership_source, "full");
    assert_eq!(out.source.claim.claim_id, cell.claim.claim_id);
    assert_eq!(out.value, Rat::from_integer(400));
}

#[test]
fn numeric_zero_is_preserved_and_unknown_or_status_values_refuse() {
    let (policy, id, source, cell) = example(PublicMetric::ProductionKoz);
    let mut zero = cell.clone();
    zero.claim.value = PublicValue::Number { stored: "0".into(), displayed: Ok("0".into()), under_status: None };
    reseal(&mut zero);
    assert_eq!(bind(&source, &policy, id, &[zero]).expect("printed zero").value, Rat::from_integer(0));
    for value in [
        PublicValue::Number { stored: "0".into(), displayed: Ok("—".into()), under_status: None },
        PublicValue::Number { stored: "2".into(), displayed: Ok("2".into()), under_status: Some(fmine_public::key::StatusKind::Withheld) },
        PublicValue::Missing,
    ] {
        let mut changed = cell.clone();
        changed.claim.value = value;
        reseal(&mut changed);
        assert!(bind(&source, &policy, id, &[changed]).is_err());
    }
    let mut changed = cell;
    changed.claim.evidence = EvidenceClass::Assumed;
    changed.claim.state = KnowledgeState::Unknown;
    reseal(&mut changed);
    assert!(bind(&source, &policy, id, &[changed]).is_err());
}

#[test]
fn availability_requires_registered_publication_after_period_end() {
    let (policy, id, source, cell) = example(PublicMetric::ProductionKoz);
    for date in [None, Some("2030-12-31"), Some("2031-02-30")] {
        let mut changed = source.clone();
        changed.published_at = date.map(str::to_string);
        assert!(bind(&changed, &policy, id, std::slice::from_ref(&cell)).is_err());
    }
    let result = bind(&source, &policy, id, &[cell]).expect("valid publication");
    let mut citation = result.citation.clone();
    citation.locator = "Different.xlsx:YTD!B5".into();
    assert!(receipt_claim_body(&policy, id, &citation, &result.stored, &result.displayed, result.ownership_source, "corpus/Original.xlsx").is_err());
    citation = result.citation;
    citation.vintage = "2031Q1".into();
    assert!(receipt_value_tag(&policy, id, &citation, &result.stored, &result.displayed, result.ownership_source).is_err());
    let mut synthetic = policy;
    synthetic.issuer = fmine_public::synthetic::SYNTHETIC_ISSUER.into();
    citation.source = synthetic.issuer.clone();
    citation.vintage = synthetic.period.original_release();
    assert!(receipt_claim_body(&synthetic, id, &citation, &result.stored, &result.displayed, result.ownership_source, "corpus/Original.xlsx").is_err());
}

fn comparative(metric: PublicMetric) -> (PublicResolutionPolicy, ObjectId, SourceRow, KeyedCell) {
    let (policy, id, mut registered, mut cell) = example(metric);
    registered.quarter = "2031Q4".into();
    registered.published_at = Some("2032-02-15".into());
    cell.key.reporting_vintage.release = registered.quarter.clone();
    cell.claim.value = PublicValue::Number {
        stored: "390.4".into(),
        displayed: Ok("390".into()),
        under_status: None,
    };
    reseal(&mut cell);
    (policy, id, registered, cell)
}

#[test]
fn explicit_comparative_retains_frozen_target_and_reconstructs_exact_source_claim() {
    for metric in [PublicMetric::ProductionKoz, PublicMetric::AiscUsdPerOz] {
        let (policy, id, registered, cell) = comparative(metric);
        let result = bind_release(&registered, &policy, id, "2031Q4", std::slice::from_ref(&cell)).expect("selected comparative");
        assert_eq!(result.value, Rat::from_integer(390));
        assert_eq!(result.source.key.valid_period, parse_period_code("2030FY").unwrap());
        assert_eq!(result.citation.vintage, "2031Q4");
        assert_eq!(result.source.claim.claim_id, cell.claim.claim_id);
        assert_eq!(result.available_at_ns, CivilDate::new(2032, 2, 16).unwrap().to_epoch_days() * NS_PER_DAY);
        assert_eq!(
            receipt_claim_body_at_vintage(&policy, id, &result.citation, &result.stored, &result.displayed, result.ownership_source, &cell.claim.anchors[0].artifact_path).unwrap(),
            cell.claim.canonical_body()
        );
        assert!(bind(&registered, &policy, id, std::slice::from_ref(&cell)).is_err());
        assert!(receipt_claim_body(&policy, id, &result.citation, &result.stored, &result.displayed, result.ownership_source, &cell.claim.anchors[0].artifact_path).is_err());
        assert!(bind_release(&registered, &policy, id, "2032Q4", &[cell]).is_err());
    }
}

#[test]
fn comparative_cannot_change_target_basis_or_authenticate_another_workbook() {
    let (policy, id, registered, cell) = comparative(PublicMetric::AiscUsdPerOz);
    for mutate in [
        (|c: &mut KeyedCell| c.key.valid_period = parse_period_code("2031FY").unwrap()) as fn(&mut KeyedCell),
        |c| c.key.subject = "Other Mine".into(),
        |c| c.key.unit = "usd_per_lb".into(),
        |c| c.printed_basis = OwnershipBasis::Unprinted,
        |c| c.label_as_printed = Some("Cash costs ($/oz)".into()),
        |c| c.key.reporting_vintage.file_sha256 = "ab".repeat(32),
    ] {
        let mut changed = cell.clone();
        mutate(&mut changed);
        reseal(&mut changed);
        assert!(bind_release(&registered, &policy, id, "2031Q4", &[changed]).is_err());
    }
    assert!(bind_release(&registered, &policy, id, "2031Q4", &[cell.clone(), cell]).is_err());
}

#[test]
fn selected_release_requires_its_own_complete_period_and_publication_evidence() {
    let (policy, id, source, cell) = comparative(PublicMetric::ProductionKoz);
    for release in ["2030Q3", "2031FY", "2031Q5", "2031Q4/../original", "é031Q4"] {
        assert!(release_period(&policy, release).is_err(), "{release}");
    }
    for date in [None, Some("2031-02-15"), Some("2031-12-31")] {
        let mut bad = source.clone();
        bad.published_at = date.map(str::to_string);
        assert!(bind_release(&bad, &policy, id, "2031Q4", std::slice::from_ref(&cell)).is_err());
    }
    let mut bad = source;
    bad.published_at_basis = None;
    assert!(bind_release(&bad, &policy, id, "2031Q4", &[cell]).is_err());
}
