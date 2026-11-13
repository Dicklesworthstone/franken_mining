use super::*;
use std::collections::BTreeMap;

use fmine_calib::ledger::binding::PublicMetric::{
    AiscUsdPerLb, AiscUsdPerOz, C1UsdPerLb, CapexProjectsUsdM, CapexSustainingUsdM, CapexUsdM,
    ProductionKoz, ProductionKt, TccUsdPerOz,
};
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
        retrieved_utc: None,
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
    cell.label_as_printed = match metric {
        AiscUsdPerOz => Some("All-in sustaining costs ($/oz)".into()),
        AiscUsdPerLb => Some("All-in sustaining costs ($/lb)".into()),
        TccUsdPerOz => Some("Total cash costs ($/oz)".into()),
        C1UsdPerLb => Some("C1 cash costs ($/lb)".into()),
        CapexUsdM => Some("Total capital expenditures ($ millions)".into()),
        CapexSustainingUsdM => Some("Minesite sustaining".into()),
        CapexProjectsUsdM => Some("Projects".into()),
        _ => None,
    };
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
    for metric in [
        ProductionKoz,
        AiscUsdPerOz,
        ProductionKt,
        TccUsdPerOz,
        C1UsdPerLb,
        AiscUsdPerLb,
        CapexUsdM,
        CapexSustainingUsdM,
        CapexProjectsUsdM,
    ] {
        let (policy, id, source, cell) = example(metric);
        let ResolvedOriginalOutcome::Published(result) =
            bind(&source, &policy, id, std::slice::from_ref(&cell)).expect("binding")
        else {
            panic!("numeric source must resolve as a published outcome");
        };
        assert_eq!(result.value.exact, Rat::from_integer(400));
        assert_eq!(result.value.stored, "400.4");
        assert_eq!(result.source.claim.claim_id, cell.claim.claim_id);
        assert_eq!(
            result.available_at_ns,
            CivilDate::new(2031, 2, 16).expect("date").to_epoch_days() * NS_PER_DAY
        );
        assert_eq!(result.published_at, "2031-02-15");
        assert_eq!(
            result.published_at_basis,
            "FACT: invented test release date"
        );
        let receipt = ReceiptClaim {
            policy: &policy,
            forecast_id: id,
            citation: &result.citation,
            ownership_source: result.ownership_source,
            anchor_path: &cell.claim.anchors[0].artifact_path,
        };
        let body = receipt
            .number(&result.value.stored, &result.value.displayed, false)
            .expect("reconstructed original source");
        assert_eq!(body, cell.claim.canonical_body());
        assert_ne!(
            receipt
                .number(&result.value.stored, "401", false)
                .expect("different assertion"),
            body
        );
        assert_ne!(
            receipt
                .number("400.5", &result.value.displayed, false)
                .expect("different stored token"),
            body
        );
    }
}

#[test]
fn capital_scopes_preserve_signed_amounts_and_refuse_label_alias_reinterpretation() {
    use fmine_calib::ledger::binding::PublicVintagePolicy;
    let mut rules = std::collections::BTreeSet::new();
    for metric in [CapexUsdM, CapexSustainingUsdM, CapexProjectsUsdM] {
        let (policy, id, registered, mut cell) = example(metric);
        assert_eq!(PublicMetric::parse(metric.as_str()).unwrap(), metric);
        assert_eq!(metric.unit(), "usd_m");
        assert!(metric.is_extensive());
        assert!(metric.is_site_row());
        assert!(!metric.is_site_physical());
        for vintages in [
            PublicVintagePolicy::OriginalOnly,
            PublicVintagePolicy::RegisteredComparatives,
        ] {
            let rule = policy.to_resolution_rule_with_vintages(vintages).unwrap();
            assert!(
                rules.insert(rule.clone()),
                "capital scopes must remain distinct"
            );
            assert_eq!(
                PublicResolutionPolicy::parse_resolution_rule_with_vintages(&rule).unwrap(),
                Some((policy.clone(), vintages))
            );
        }
        cell.claim.value = PublicValue::Number {
            stored: "-12.504".into(),
            displayed: Ok("-12.50".into()),
            under_status: None,
        };
        reseal(&mut cell);
        let ResolvedOriginalOutcome::Published(outcome) =
            bind(&registered, &policy, id, std::slice::from_ref(&cell)).unwrap()
        else {
            panic!("signed capital outturn must remain numeric");
        };
        assert_eq!(outcome.value.exact, Rat::new(-25, 2).unwrap());
        assert_eq!(
            outcome.source.claim.additivity,
            AdditivityClass::ExtensiveAdditive
        );
        for label in [
            None,
            Some("Adjusted capital expenditures"),
            Some("Projects excluding leases"),
        ] {
            let mut changed = cell.clone();
            changed.label_as_printed = label.map(str::to_string);
            assert_eq!(
                bind(&registered, &policy, id, &[changed]).unwrap_err().code,
                "FMINE-REF-105"
            );
        }
    }
    for unsupported in ["capex_usd", "capex_aud_m", "capex_projects_per_oz"] {
        assert!(PublicMetric::parse(unsupported).is_err());
    }
}

#[test]
fn frozen_target_fields_cannot_be_chosen_at_resolution() {
    let (policy, id, source, cell) = example(PublicMetric::AiscUsdPerOz);
    let mut variants = vec![policy.clone(); 5];
    variants[0].issuer = "different".into();
    variants[1].members.insert(id, "Other Mine".into());
    variants[2].period = PublicPeriod::Quarter {
        year: 2030,
        quarter: 4,
    };
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
        bind(&source, &policy, id, &[later])
            .expect_err("no comparative fallback")
            .code,
        "FMINE-REF-201"
    );
    assert_eq!(
        bind(&source, &policy, id, &[cell.clone(), cell])
            .expect_err("ambiguous sources")
            .code,
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
    assert_eq!(
        bind(&source, &policy, id, &[changed])
            .expect_err("mutated value")
            .code,
        "FMINE-REF-507"
    );
    for modify in [
        (|c: &mut KeyedCell| c.key.unit = "usd_per_oz".into()) as fn(&mut KeyedCell),
        |c| c.key.scope = Scope::Company,
        |c| c.key.line.occurrence = 1,
    ] {
        let mut changed = cell.clone();
        modify(&mut changed);
        reseal(&mut changed);
        assert_eq!(
            bind(&source, &policy, id, &[changed])
                .expect_err("wrong support")
                .code,
            "FMINE-REF-105"
        );
    }
    let mut changed = cell.clone();
    changed.claim.confidentiality = Confidentiality::internal();
    reseal(&mut changed);
    assert_eq!(
        bind(&source, &policy, id, &[changed])
            .expect_err("nonpublic")
            .code,
        "FMINE-REF-301"
    );
    let mut changed = cell;
    changed.claim.stale = true;
    assert_eq!(
        bind(&source, &policy, id, &[changed])
            .expect_err("stale")
            .code,
        "FMINE-REF-502"
    );
    for metric in [AiscUsdPerOz, AiscUsdPerLb] {
        let (policy, id, source, mut cost) = example(metric);
        cost.printed_basis = OwnershipBasis::Unprinted;
        assert_eq!(
            bind(&source, &policy, id, &[cost])
                .expect_err("unprinted cost basis")
                .code,
            "FMINE-REF-105"
        );
    }
}

#[test]
fn cost_headers_admit_only_the_frozen_definition_and_sold_unit() {
    for (metric, name, unit, sold) in [
        (AiscUsdPerOz, "All-in sustaining costs", "$/oz", "ounce"),
        (TccUsdPerOz, "Total cash costs", "$/oz", "ounce"),
        (C1UsdPerLb, "C1 cash costs", "$/lb", "pound"),
        (AiscUsdPerLb, "All-in sustaining costs", "$/lb", "pound"),
    ] {
        let (policy, id, source, mut cell) = example(metric);
        for header in [
            format!("{name} ({unit})"),
            format!("{name}12 ({unit})8"),
            format!("{name} (2) ({unit}) (3) per sold {sold} (4)"),
            format!("{name} ({unit}) per {sold} sold"),
        ] {
            cell.label_as_printed = Some(header.clone());
            assert!(
                bind(&source, &policy, id, std::slice::from_ref(&cell)).is_ok(),
                "{header}"
            );
        }
        for header in [
            format!("{name} ({unit}) per {sold} produced"),
            format!("{name} ({unit}) per {sold} sold or produced"),
            format!("{name} ({unit}) per sold tonne"),
            format!("{name} ({unit}) adjusted"),
            format!("{name} ({unit}) co-product"),
            format!("Adjusted {name} ({unit})"),
            format!("{name} (CAD/{})", if sold == "ounce" { "oz" } else { "lb" }),
            format!("{name} ({})", if sold == "ounce" { "$/lb" } else { "$/oz" }),
        ] {
            cell.label_as_printed = Some(header.clone());
            assert_eq!(
                bind(&source, &policy, id, std::slice::from_ref(&cell))
                    .unwrap_err()
                    .code,
                "FMINE-REF-105",
                "{header}"
            );
        }
        cell.label_as_printed = None;
        assert_eq!(
            bind(&source, &policy, id, &[cell]).unwrap_err().code,
            "FMINE-REF-105"
        );
    }
}

#[test]
fn a_matching_cost_unit_cannot_substitute_a_different_definition() {
    let (policy, id, source, mut cell) = example(TccUsdPerOz);
    cell.label_as_printed = Some("All-in sustaining costs ($/oz)".into());
    assert_eq!(
        bind(&source, &policy, id, &[cell]).unwrap_err().code,
        "FMINE-REF-105"
    );
    let (policy, id, source, mut cell) = example(C1UsdPerLb);
    cell.label_as_printed = Some("All-in sustaining costs ($/lb)".into());
    assert_eq!(
        bind(&source, &policy, id, &[cell]).unwrap_err().code,
        "FMINE-REF-105"
    );
    let (policy, id, source, mut cell) = example(AiscUsdPerLb);
    cell.label_as_printed = Some("C1 cash costs ($/lb)".into());
    assert_eq!(
        bind(&source, &policy, id, &[cell]).unwrap_err().code,
        "FMINE-REF-105"
    );
    let (policy, id, source, mut cell) = example(AiscUsdPerLb);
    cell.key.unit = "usd_per_oz".into();
    reseal(&mut cell);
    assert_eq!(
        bind(&source, &policy, id, &[cell]).unwrap_err().code,
        "FMINE-REF-105"
    );
}

#[test]
fn full_production_column_does_not_adopt_the_site_rows_attributable_share() {
    let (policy, id, source, mut cell) = example(PublicMetric::ProductionKoz);
    for printed in [
        OwnershipBasis::Unprinted,
        OwnershipBasis::Attributable {
            num: 3,
            den: 5,
            source: ShareSource::Printed,
        },
    ] {
        cell.printed_basis = printed;
        let ResolvedOriginalOutcome::Published(result) =
            bind(&source, &policy, id, std::slice::from_ref(&cell))
                .expect("explicit 100% production column")
        else {
            panic!("numeric production must resolve as a published outcome");
        };
        assert_eq!(result.value.exact, Rat::from_integer(400));
    }
}

#[test]
fn wholly_owned_attributable_production_still_binds_the_attributable_column() {
    let (mut policy, id, source, mut cell) = example(PublicMetric::ProductionKoz);
    policy.ownership = PublicOwnership::Attributable { num: 1, den: 1 };
    assert!(bind(&source, &policy, id, std::slice::from_ref(&cell)).is_err());
    cell.key.line.label = policy.metric.field(policy.ownership).into();
    reseal(&mut cell);
    let ResolvedOriginalOutcome::Published(out) =
        bind(&source, &policy, id, std::slice::from_ref(&cell))
            .expect("explicit attributable column at a wholly owned site")
    else {
        panic!("numeric attributable production must resolve as a published outcome");
    };
    assert_eq!(out.ownership_source, "full");
    assert_eq!(out.source.claim.claim_id, cell.claim.claim_id);
    assert_eq!(out.value.exact, Rat::from_integer(400));
}

#[test]
fn numeric_zero_is_preserved_and_unknown_or_status_values_refuse() {
    let (policy, id, source, cell) = example(PublicMetric::ProductionKoz);
    let mut zero = cell.clone();
    zero.claim.value = PublicValue::Number {
        stored: "0".into(),
        displayed: Ok("0".into()),
        under_status: None,
    };
    reseal(&mut zero);
    let ResolvedOriginalOutcome::Published(result) =
        bind(&source, &policy, id, &[zero]).expect("printed zero")
    else {
        panic!("printed numeric zero must not become a missing outcome");
    };
    assert_eq!(result.value.exact, Rat::from_integer(0));
    for value in [
        PublicValue::Number {
            stored: "0".into(),
            displayed: Ok("—".into()),
            under_status: None,
        },
        PublicValue::Number {
            stored: "2".into(),
            displayed: Ok("2".into()),
            under_status: Some(fmine_public::key::StatusKind::Withheld),
        },
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
    let ResolvedOriginalOutcome::Published(result) =
        bind(&source, &policy, id, &[cell]).expect("valid publication")
    else {
        panic!("numeric publication must resolve as a published outcome");
    };
    let mut citation = result.citation.clone();
    citation.locator = "Different.xlsx:YTD!B5".into();
    assert!(
        ReceiptClaim {
            policy: &policy,
            forecast_id: id,
            citation: &citation,
            ownership_source: result.ownership_source,
            anchor_path: "corpus/Original.xlsx",
        }
        .number(&result.value.stored, &result.value.displayed, false)
        .is_err()
    );
    citation = result.citation.clone();
    citation.vintage = "2031Q1".into();
    assert!(
        ReceiptClaim {
            policy: &policy,
            forecast_id: id,
            citation: &citation,
            ownership_source: result.ownership_source,
            anchor_path: "corpus/Original.xlsx",
        }
        .number(&result.value.stored, &result.value.displayed, false)
        .is_err()
    );
    let mut synthetic = policy;
    synthetic.issuer = fmine_public::synthetic::SYNTHETIC_ISSUER.into();
    citation.source = synthetic.issuer.clone();
    citation.vintage = synthetic.period.original_release();
    assert!(
        ReceiptClaim {
            policy: &synthetic,
            forecast_id: id,
            citation: &citation,
            ownership_source: result.ownership_source,
            anchor_path: "corpus/Original.xlsx",
        }
        .number(&result.value.stored, &result.value.displayed, false)
        .is_err()
    );
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
    for metric in [
        ProductionKoz,
        AiscUsdPerOz,
        ProductionKt,
        TccUsdPerOz,
        C1UsdPerLb,
        AiscUsdPerLb,
    ] {
        let (policy, id, registered, cell) = comparative(metric);
        let ResolvedOriginalOutcome::Published(result) = bind_value(
            &registered,
            &policy,
            id,
            "2031Q4",
            std::slice::from_ref(&cell),
            None,
        )
        .expect("selected comparative") else {
            panic!("an explicit comparative must be numeric");
        };
        assert_eq!(result.value.exact, Rat::from_integer(390));
        assert_eq!(
            result.source.key.valid_period,
            parse_period_code("2030FY").unwrap()
        );
        assert_eq!(result.citation.vintage, "2031Q4");
        assert_eq!(result.source.claim.claim_id, cell.claim.claim_id);
        assert_eq!(
            result.available_at_ns,
            CivilDate::new(2032, 2, 16).unwrap().to_epoch_days() * NS_PER_DAY
        );
        let receipt = ReceiptClaim {
            policy: &policy,
            forecast_id: id,
            citation: &result.citation,
            ownership_source: result.ownership_source,
            anchor_path: &cell.claim.anchors[0].artifact_path,
        };
        assert_eq!(
            receipt
                .number(&result.value.stored, &result.value.displayed, true)
                .unwrap(),
            cell.claim.canonical_body()
        );
        assert!(bind(&registered, &policy, id, std::slice::from_ref(&cell)).is_err());
        assert!(
            receipt
                .number(&result.value.stored, &result.value.displayed, false)
                .is_err()
        );
        assert!(bind_value(&registered, &policy, id, "2032Q4", &[cell], None).is_err());
    }
}

#[test]
fn comparative_cannot_change_target_basis_or_authenticate_another_workbook() {
    let (policy, id, registered, cell) = comparative(PublicMetric::AiscUsdPerOz);
    for mutate in [
        (|c: &mut KeyedCell| c.key.valid_period = parse_period_code("2031FY").unwrap())
            as fn(&mut KeyedCell),
        |c| c.key.subject = "Other Mine".into(),
        |c| c.key.unit = "usd_per_lb".into(),
        |c| c.printed_basis = OwnershipBasis::Unprinted,
        |c| c.label_as_printed = Some("Cash costs ($/oz)".into()),
        |c| c.key.reporting_vintage.file_sha256 = "ab".repeat(32),
    ] {
        let mut changed = cell.clone();
        mutate(&mut changed);
        reseal(&mut changed);
        assert!(bind_value(&registered, &policy, id, "2031Q4", &[changed], None).is_err());
    }
    assert!(
        bind_value(
            &registered,
            &policy,
            id,
            "2031Q4",
            &[cell.clone(), cell],
            None
        )
        .is_err()
    );
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
        assert!(
            bind_value(
                &bad,
                &policy,
                id,
                "2031Q4",
                std::slice::from_ref(&cell),
                None
            )
            .is_err()
        );
    }
    let mut bad = source;
    bad.published_at_basis = None;
    assert!(bind_value(&bad, &policy, id, "2031Q4", &[cell], None).is_err());
}

fn status_evidence(kind: StatusKind, first: (u16, u8), last: Option<(u16, u8)>) -> StatusEvidence {
    StatusEvidence {
        status: SubjectStatus {
            subject: "Example Mine".into(),
            kind,
            first,
            last,
            evidence: "FACT: invented status effective dates for a calendar boundary control"
                .into(),
        },
        registry_sha256: sha256(b"invented immutable status registry"),
    }
}

#[test]
fn status_interpretation_uses_full_calendar_support_and_the_disposal_closing_month() {
    let (mut policy, id, _, _) = example(ProductionKoz);
    let disposal = StatusEvidence {
        status: SubjectStatus::from_date(
            "Example Mine",
            StatusKind::Disposed,
            (2030, 12, 31),
            "FACT: invented disposal closed on 31 December 2030",
        ),
        registry_sha256: sha256(b"invented disposal registry"),
    };
    let withheld = status_evidence(StatusKind::Withheld, (2030, 4), Some((2030, 9)));
    let care = status_evidence(StatusKind::CareAndMaintenance, (2030, 1), None);
    for (status, period, expected) in [
        (&disposal, "2030Q3", None),
        (&disposal, "2030Q4", Some(MissingReason::DefinitionChanged)),
        (&disposal, "2030FY", Some(MissingReason::DefinitionChanged)),
        (&disposal, "2031Q1", Some(MissingReason::Divested)),
        (&disposal, "2031FY", Some(MissingReason::Divested)),
        (&withheld, "2030Q1", None),
        (&withheld, "2030Q2", Some(MissingReason::NotPublished)),
        (&withheld, "2030Q3", Some(MissingReason::NotPublished)),
        (&withheld, "2030Q4", None),
        (&withheld, "2030FY", Some(MissingReason::DefinitionChanged)),
        (&care, "2030Q1", None),
        (&care, "2030FY", None),
    ] {
        policy.period = PublicPeriod::parse(period).unwrap();
        assert_eq!(
            status.reason(&policy, id).unwrap(),
            expected,
            "{period}: {status:?}"
        );
    }
}

#[test]
fn status_interpretation_refuses_unbound_or_invalid_retained_evidence() {
    let (policy, id, _, _) = example(ProductionKoz);
    let status = status_evidence(StatusKind::Withheld, (2030, 1), None);
    for change in [
        (|s: &mut StatusEvidence| s.status.subject = "Another Mine".into())
            as fn(&mut StatusEvidence),
        |s| s.status.first = (0, 1),
        |s| s.status.first = (2030, 0),
        |s| s.status.first = (2030, 13),
        |s| s.status.last = Some((2029, 12)),
        |s| s.status.last = Some((2030, 13)),
        |s| s.status.evidence = " \n ".into(),
        |s| s.registry_sha256 = Digest32::default(),
    ] {
        let mut changed = status.clone();
        change(&mut changed);
        assert_eq!(
            changed.reason(&policy, id).unwrap_err().code,
            "FMINE-REF-208"
        );
    }
}

#[test]
fn partial_period_status_keeps_raw_number_but_never_resolves_it_as_full_period() {
    let (policy, id, registered, cell) = example(ProductionKoz);
    for status in [
        status_evidence(StatusKind::Disposed, (2030, 7), None),
        status_evidence(StatusKind::Withheld, (2030, 1), Some((2030, 9))),
    ] {
        let ResolvedOriginalOutcome::Missing(outcome) = bind_value(
            &registered,
            &policy,
            id,
            "2030Q4",
            std::slice::from_ref(&cell),
            Some(status.clone()),
        )
        .expect("a printed stub remains an unresolved full-period target") else {
            panic!("a partial-period number must not enter the full-period score");
        };
        assert_eq!(outcome.value.reason, MissingReason::DefinitionChanged);
        assert_eq!(outcome.value.value_kind, "number");
        assert_eq!(outcome.value.token, "400.4");
        assert_eq!(outcome.value.displayed.as_deref(), Some("400"));
        assert_eq!(outcome.source.claim, cell.claim);
        assert_eq!(
            outcome.value.status.unwrap().registry_sha256,
            status.registry_sha256
        );
        for value in [
            PublicValue::Missing,
            PublicValue::NotApplicable {
                token: "n/a".into(),
            },
            PublicValue::Number {
                stored: "400.4".into(),
                displayed: Err("FMINE-REF-208"),
                under_status: None,
            },
        ] {
            let mut changed = cell.clone();
            changed.claim.value = value;
            reseal(&mut changed);
            assert!(
                bind_value(
                    &registered,
                    &policy,
                    id,
                    "2030Q4",
                    &[changed],
                    Some(status.clone())
                )
                .is_err()
            );
        }
    }
}
