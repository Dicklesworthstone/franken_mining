//! Public key contracts crossing the claim, evidence, RNG and registry crates.
use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal};
use fmine_io::styles::NumberFormat;
use fmine_io::xlsx::{CellValue, ExtractedCell};
use fmine_public::key::{
    KeyRefusal, LineKey, NilReason, OwnershipBasis, Period, PeriodKind, PublicKey, PublicValue,
    Scope, ShareSource, StatusKind, SubjectStatus, Vintage, classify, parse_ownership,
    parse_sheet_period, status_for,
};
use fmine_public::{cell_to_public_claim, run_synthetic_assurance, synthetic};
use std::collections::BTreeMap;

fn p(year: u16, kind: PeriodKind) -> Period {
    Period { year, kind }
}

/// The 23 distinct sheet names of the eight registered AISC workbooks (40 sheets), hand-listed from their
/// workbook parts; the expected period of each is read from its name.
#[test]
fn test_parse_every_aisc_sheet_name() {
    use PeriodKind::{FullYear, H1, NineMonths, Quarter};
    let want: [(&str, Period); 23] = [
        ("2024", p(2024, FullYear)),
        ("2025", p(2025, FullYear)),
        ("Q1 2024", p(2024, Quarter(1))),
        ("Q1 2025", p(2025, Quarter(1))),
        ("Q1 2026", p(2026, Quarter(1))),
        ("Q2 2024", p(2024, Quarter(2))),
        ("Q2 2025", p(2025, Quarter(2))),
        ("Q2 2026", p(2026, Quarter(2))),
        ("Q3 2023", p(2023, Quarter(3))),
        ("Q3 2024", p(2024, Quarter(3))),
        ("Q3 2025", p(2025, Quarter(3))),
        ("Q4 2023", p(2023, Quarter(4))),
        ("Q4 2024", p(2024, Quarter(4))),
        ("Q4 2025", p(2025, Quarter(4))),
        ("YTD Dec 31 2023", p(2023, FullYear)),
        ("YTD Dec 31 2024", p(2024, FullYear)),
        ("YTD Q2 2024", p(2024, H1)),
        ("YTD Q2 2025", p(2025, H1)),
        ("YTD Q2 2026", p(2026, H1)),
        ("YTD Q3 2024", p(2024, NineMonths)),
        ("YTD Q3 2025", p(2025, NineMonths)),
        ("YTD Sept 30 2023", p(2023, NineMonths)),
        ("YTD Sept 30 2024", p(2024, NineMonths)),
    ];
    for (name, period) in want {
        assert_eq!(parse_sheet_period(name), Ok(period), "{name}");
    }
    assert_eq!(p(2026, Quarter(2)).code(), "2026Q2");
    assert_eq!(p(2026, H1).code(), "2026H1");
    assert_eq!(p(2024, NineMonths).code(), "20249M");
    assert_eq!(p(2025, FullYear).code(), "2025FY");
}

#[test]
fn test_unknown_sheet_name_refused() {
    for name in [
        "",
        "Q5 2024",
        "Q0 2024",
        "Q01 2024",
        "Q1 24",
        "Q1  2024",
        " Q1 2024",
        "Q1 2024 ",
        "YTD Q1 2024",
        "YTD Q4 2024",
        "YTD June 30 2024",
        "YTD Sept 30 202",
        "FY2024",
        "Sheet1",
        "Scratch",
    ] {
        assert_eq!(
            parse_sheet_period(name),
            Err(KeyRefusal::SheetPeriod {
                name: name.to_string()
            }),
            "{name:?}"
        );
    }
}

#[test]
fn test_ownership_porgera_site_g_hemlo() {
    let printed = |num, den| OwnershipBasis::Attributable {
        num,
        den,
        source: ShareSource::Printed,
    };
    // 24.5% = 245/1000 = 49/200; 60% = 3/5; 61.5% = 123/200.
    assert_eq!(parse_ownership("Porgera (24.5%)1"), Ok(printed(49, 200)));
    assert_eq!(parse_ownership("Site G (60%)1"), Ok(printed(3, 5)));
    assert_eq!(
        parse_ownership("Long Canyon (61.5%)1"),
        Ok(printed(123, 200))
    );
    assert_eq!(parse_ownership("Carlin (61.5%)"), Ok(printed(123, 200)));
    // No share printed: unprinted, never 100%.
    for label in ["Hemlo2", "Hemlo", "Kibali", "Region North12"] {
        assert_eq!(
            parse_ownership(label),
            Ok(OwnershipBasis::Unprinted),
            "{label}"
        );
    }
    // A % that does not read as a share in (0, 100) refuses.
    for label in [
        "Site (abc%)",
        "Site (0%)",
        "Site (150%)",
        "Site (1.2.3%)",
        "Site (60%)123",
        "Site (60%) of the JV",
        "Site 60%",
    ] {
        assert!(
            matches!(parse_ownership(label), Err(KeyRefusal::PrintedShare { .. })),
            "{label}"
        );
    }
}

/// `100%` is read before the footnote run is stripped: stripping first would read `(100%)1` as the
/// attributable share 1/1.
#[test]
fn test_ownership_100_detected_before_footnote_strip() {
    for label in [
        "Nevada Gold Mines (100%)1",
        "Turquoise Ridge (100%)",
        "Site 100%",
    ] {
        assert_eq!(
            parse_ownership(label),
            Ok(OwnershipBasis::Consolidated100),
            "{label}"
        );
    }
}

fn number(text: &str, v: f64) -> ExtractedCell {
    ExtractedCell {
        cell_ref: "B10".to_string(),
        row: 10,
        col: 2,
        value: CellValue::Number(v),
        formula: None,
        number_text: Some(text.to_string()),
        style: None,
        format: NumberFormat::Builtin(0),
    }
}

fn token(t: &str) -> ExtractedCell {
    ExtractedCell {
        value: CellValue::Placeholder(t.to_string()),
        number_text: None,
        ..number("0", 0.0)
    }
}

/// A number shown under General, which prints an integer as stored.
fn shown(stored: &str, under_status: Option<StatusKind>) -> PublicValue {
    PublicValue::Number {
        stored: stored.to_string(),
        displayed: Ok(stored.to_string()),
        under_status,
    }
}

#[test]
fn test_token_mapping_four_types() {
    assert_eq!(
        classify(Some(&number("1691", 1691.0)), None),
        Ok(shown("1691", None))
    );
    // A format can print a stored zero as a dash: the number keeps both texts.
    let em_dash = ExtractedCell {
        format: NumberFormat::Custom {
            id: 164,
            code: r#"_(#,##0_);_(\(#,##0\);_("—"_);_(@_)"#.to_string(),
        },
        ..number("0", 0.0)
    };
    assert_eq!(
        classify(Some(&em_dash), None),
        Ok(PublicValue::Number {
            stored: "0".to_string(),
            displayed: Ok("—".to_string()),
            under_status: None
        })
    );
    // General has no exact rendering of a non-integer: the refusal code is kept, the stored text too.
    assert_eq!(
        classify(Some(&number("1.25", 1.25)), None),
        Ok(PublicValue::Number {
            stored: "1.25".to_string(),
            displayed: Err(fmine_io::numfmt::GENERAL_NON_INTEGER),
            under_status: None
        })
    );
    for dash in ["—", "-", "–", "— "] {
        assert_eq!(
            classify(Some(&token(dash)), None),
            Ok(PublicValue::Nil {
                token: dash.to_string(),
                reason: NilReason::Unknown
            })
        );
    }
    for na in ["n/a", "N/A"] {
        assert_eq!(
            classify(Some(&token(na)), None),
            Ok(PublicValue::NotApplicable {
                token: na.to_string()
            })
        );
    }
    assert_eq!(classify(None, None), Ok(PublicValue::Missing));
    let text = ExtractedCell {
        value: CellValue::Text("see note".to_string()),
        ..number("0", 0.0)
    };
    let error = ExtractedCell {
        value: CellValue::FormulaError("#REF!".to_string()),
        ..number("0", 0.0)
    };
    let no_stored_text = ExtractedCell {
        number_text: None,
        ..number("0", 0.0)
    };
    for c in [text, error, no_stored_text] {
        assert_eq!(
            classify(Some(&c), None),
            Err(KeyRefusal::CellToken {
                cell: "B10".to_string()
            })
        );
    }
}

fn statuses() -> Vec<SubjectStatus> {
    vec![
        SubjectStatus::from_date("Tongon", StatusKind::Disposed, (2025, 12, 1), "E485"),
        SubjectStatus::from_date("Hemlo", StatusKind::Disposed, (2025, 11, 26), "E485"),
        SubjectStatus {
            subject: "Loulo-Gounkoto".to_string(),
            kind: StatusKind::Withheld,
            first: (2025, 1),
            last: Some((2025, 9)),
            evidence: "E818".to_string(),
        },
    ]
}

#[test]
fn test_disposed_zero_is_nil_nonzero_kept_flagged() {
    let s = statuses();
    let q1_2026 = p(2026, PeriodKind::Quarter(1));
    let disposed = status_for(&s, "Tongon", &q1_2026);
    assert_eq!(disposed, Some(StatusKind::Disposed));
    assert_eq!(
        classify(Some(&number("0", 0.0)), disposed),
        Ok(PublicValue::Nil {
            token: "0".to_string(),
            reason: NilReason::Disposed
        })
    );
    // E116's 1691 stays visible, flagged.
    assert_eq!(
        classify(Some(&number("1691", 1691.0)), disposed),
        Ok(shown("1691", Some(StatusKind::Disposed)))
    );
    // Withheld 2025Q1–Q3: H1 and nine months lie inside; the full year does not.
    for (period, applies) in [
        (p(2025, PeriodKind::Quarter(2)), true),
        (p(2025, PeriodKind::H1), true),
        (p(2025, PeriodKind::NineMonths), true),
        (p(2025, PeriodKind::Quarter(4)), false),
        (p(2025, PeriodKind::FullYear), false),
    ] {
        let st = status_for(&s, "Loulo-Gounkoto", &period);
        assert_eq!(st.is_some(), applies, "{period:?}");
        if applies {
            assert_eq!(
                classify(Some(&number("0", 0.0)), st),
                Ok(PublicValue::Nil {
                    token: "0".to_string(),
                    reason: NilReason::Withheld
                })
            );
        }
    }
    // Care and maintenance does not make a zero Nil.
    let cm = [SubjectStatus {
        subject: "Long Canyon".to_string(),
        kind: StatusKind::CareAndMaintenance,
        first: (2024, 1),
        last: None,
        evidence: "W1".to_string(),
    }];
    let st = status_for(&cm, "Long Canyon", &p(2024, PeriodKind::Quarter(1)));
    assert_eq!(classify(Some(&number("0", 0.0)), st), Ok(shown("0", None)));
}

/// A disposal on 1 December (Tongon) or 26 November (Hemlo) leaves the December quarter and the full year
/// with their numbers; the status applies from the first quarter after.
#[test]
fn test_stub_quarter_keeps_numbers() {
    let s = statuses();
    for subject in ["Tongon", "Hemlo"] {
        for period in [
            p(2025, PeriodKind::Quarter(4)),
            p(2025, PeriodKind::FullYear),
        ] {
            let st = status_for(&s, subject, &period);
            assert_eq!(st, None, "{subject} {period:?}");
            assert_eq!(classify(Some(&number("0", 0.0)), st), Ok(shown("0", None)));
        }
        assert_eq!(
            status_for(&s, subject, &p(2026, PeriodKind::Quarter(1))),
            Some(StatusKind::Disposed)
        );
    }
}

/// Planted negative: without its status row, a synthetic disposed site's zero reads as a number.
#[test]
fn test_remove_tongon_row_breaks_synthetic_zero_test() {
    let mut s = vec![SubjectStatus::from_date(
        "Site Z",
        StatusKind::Disposed,
        (2029, 6, 30),
        "synthetic",
    )];
    let q = p(2030, PeriodKind::Quarter(1));
    let zero = number("0", 0.0);
    assert!(matches!(
        classify(Some(&zero), status_for(&s, "Site Z", &q)),
        Ok(PublicValue::Nil { .. })
    ));
    s.clear();
    assert!(matches!(
        classify(Some(&zero), status_for(&s, "Site Z", &q)),
        Ok(PublicValue::Number { .. })
    ));
}

/// First publication and comparative: same series, different vintage, different tag.
#[test]
fn test_series_key_groups_vintages_without_changing_canonical_tags() {
    let first = PublicKey {
        issuer: "barrick".to_string(),
        subject: "Total Gold".to_string(),
        valid_period: p(2024, PeriodKind::Quarter(1)),
        reporting_vintage: Vintage {
            release: "2024Q1".to_string(),
            file_sha256: "a".repeat(64),
        },
        ownership_basis: OwnershipBasis::Unprinted,
        scope: Scope::Company,
        unit: "usd_per_oz".to_string(),
        line: LineKey {
            label: "all-in sustaining costs".to_string(),
            occurrence: 0,
        },
    };
    let comparative = PublicKey {
        reporting_vintage: Vintage {
            release: "2025Q1".to_string(),
            file_sha256: "b".repeat(64),
        },
        ..first.clone()
    };
    assert_eq!(first.series_key(), comparative.series_key());
    assert_ne!(
        first.canonical_tag("1479"),
        comparative.canonical_tag("1479")
    );
    assert_ne!(first.canonical_tag("1479"), first.canonical_tag("1474"));
}

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// `cell_to_public_claim` on a cell of the synthetic issuer table.
#[allow(clippy::expect_used)]
fn synthetic_claim(row: u32, col: u32) -> Result<Claim<Option<f64>>, Refusal> {
    let r = run_synthetic_assurance(&[]).expect("synthetic");
    let f = synthetic::SYNTHETIC_AISC_FILE;
    let aisc = r.sources.iter().find(|s| s.file_name == f).expect("aisc");
    let t = synthetic::SYNTHETIC_TABLE_SHEET;
    let sheet = aisc.workbook.sheet(t).expect("sheet");
    let cell = sheet.cell(row, col).expect("cell");
    let additive = AdditivityClass::ExtensiveAdditive;
    cell_to_public_claim(&aisc.workbook, &sheet.name, "m", cell, additive)
}

#[test]
fn test_issuer_cell_is_assertion_issuer_known() {
    let claim = synthetic_claim(5, 2).expect("B5");
    assert_eq!(claim.value, Some(500.0));
    assert_eq!(claim.anchors[0].locator, "Q1 2030!B5");
    assert_eq!(claim.evidence, EvidenceClass::ISSUER);
    assert_eq!(claim.state, KnowledgeState::Known);
    assert_eq!(claim.epistemic, EpistemicStatus::KnownKnown);
}

#[test]
fn test_placeholder_cell_is_assumed_unknown() {
    let claim = synthetic_claim(6, 6).expect("F6");
    assert_eq!(claim.value, None);
    assert_eq!(claim.evidence, EvidenceClass::Assumed);
    assert_eq!(claim.state, KnowledgeState::Unknown);
}

/// "Reproduced" means the issuer's figures agree with each other, so the result stays an assertion (NL-2).
#[test]
fn test_reproduction_result_class_is_assertion() {
    let cell = synthetic_claim(5, 2).expect("B5");
    let footing = fmine_claim::compose_binary(
        "component_sum",
        &cell,
        &cell,
        Some(1000.0),
        "1000",
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::ExtensiveAdditive,
        fmine_evidence::color::IntervalOp::Add,
    );
    assert_eq!(footing.evidence, EvidenceClass::ISSUER);
    assert!(footing.color().is_err());
}

/// A formula-error cell is a source cell unusable (208, exit 3), never 401 (unknown reactive ground, exit 7).
#[test]
fn test_formula_error_cell_refuses_208_exit_3() {
    let refusal = synthetic_claim(20, 2).expect_err("formula error");
    assert_eq!((refusal.code, refusal.exit_code), ("FMINE-REF-208", 3));
    assert!(
        refusal.what.starts_with("Source cell unusable")
            && refusal.what.contains("quarantined formula error")
            && refusal.what.ends_with("at Q1 2030!B20"),
        "{}",
        refusal.what
    );
}

/// The claim id differs whenever any key field or the raw token differs: 10⁴ (key, token) pairs drawn from
/// small alphabets that include the tag's own separators, so near-collisions are common.
#[test]
fn test_property_claim_id_injective_on_key_and_token() {
    let mut rng = PhiloxStream::new(RngKey {
        seed_root: 332,
        partition: 1,
        domain: 0,
        var_group: 0,
        realization: 0,
        brick: 0,
    });
    let alphabet = ["", "a", "b", "|", ":", "1:", "a|b", "|1:a"];
    let mut pick = |n: usize| (rng.next_u64() % n as u64) as usize;
    let mut seen: BTreeMap<String, (PublicKey, String)> = BTreeMap::new();
    let mut ids = std::collections::BTreeSet::new();
    for _ in 0..10_000 {
        let s = |i: usize| alphabet[i].to_string();
        let kinds = [
            PeriodKind::Quarter(1),
            PeriodKind::Quarter(2),
            PeriodKind::H1,
            PeriodKind::FullYear,
        ];
        let bases = [
            OwnershipBasis::Consolidated100,
            OwnershipBasis::Unprinted,
            OwnershipBasis::Attributable {
                num: 1,
                den: 2,
                source: ShareSource::Printed,
            },
            OwnershipBasis::Attributable {
                num: 1,
                den: 2,
                source: ShareSource::Registry,
            },
        ];
        let key = PublicKey {
            issuer: s(pick(3)),
            subject: s(pick(8)),
            valid_period: p(2024 + pick(2) as u16, kinds[pick(4)]),
            reporting_vintage: Vintage {
                release: s(pick(4)),
                file_sha256: s(pick(3)),
            },
            ownership_basis: bases[pick(4)],
            scope: match pick(3) {
                0 => Scope::Site,
                1 => Scope::Company,
                _ => Scope::Region {
                    map_version: s(pick(8)),
                },
            },
            unit: s(pick(8)),
            line: LineKey {
                label: s(pick(8)),
                occurrence: pick(2) as u32,
            },
        };
        let raw = s(pick(8));
        let tag = key.canonical_tag(&raw);
        if let Some(prev) = seen.get(&tag) {
            assert_eq!(prev, &(key.clone(), raw.clone()), "tag collision: {tag}");
            continue;
        }
        let claim = Claim::new(
            "public.aisc",
            0_i64,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::IntensiveWeighted,
            EvidenceClass::Derived,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        assert!(ids.insert(claim.claim_id), "claim id collision: {tag}");
        seen.insert(tag, (key, raw));
    }
    // The draws revisit pairs, and every distinct pair got its own tag and id.
    assert!(seen.len() > 5_000, "{}", seen.len());
    assert_eq!(ids.len(), seen.len());
}

/// The registered statuses, hand-written from E486–E489 and E818, and what each covers.
#[test]
fn test_subject_status_registry_golden() {
    let profile = fmine_public::sources::load_issuer_profile(&workspace_root(), "barrick")
        .expect("barrick registry loads");
    let row = |subject: &str, kind, first, last, evidence: &str| SubjectStatus {
        subject: subject.to_string(),
        kind,
        first,
        last,
        evidence: evidence.to_string(),
    };
    assert_eq!(
        profile.subject_statuses,
        vec![
            row(
                "Hemlo",
                StatusKind::Disposed,
                (2025, 12),
                None,
                "E486, E488"
            ),
            row(
                "Tongon",
                StatusKind::Disposed,
                (2026, 1),
                None,
                "E487, E488"
            ),
            row(
                "Loulo-Gounkoto",
                StatusKind::Withheld,
                (2025, 1),
                Some((2025, 9)),
                "E818"
            ),
            row(
                "Long Canyon",
                StatusKind::CareAndMaintenance,
                (2024, 1),
                None,
                "E489"
            ),
        ]
    );
    let s = &profile.subject_statuses;
    let q = |y, n| p(y, PeriodKind::Quarter(n));
    for (subject, period, want) in [
        ("Hemlo", q(2025, 4), None),
        ("Hemlo", p(2025, PeriodKind::FullYear), None),
        ("Hemlo", q(2026, 1), Some(StatusKind::Disposed)),
        ("Hemlo", p(2026, PeriodKind::H1), Some(StatusKind::Disposed)),
        ("Tongon", q(2025, 4), None),
        ("Tongon", q(2026, 2), Some(StatusKind::Disposed)),
        ("Loulo-Gounkoto", q(2024, 4), None),
        ("Loulo-Gounkoto", q(2025, 1), Some(StatusKind::Withheld)),
        (
            "Loulo-Gounkoto",
            p(2025, PeriodKind::NineMonths),
            Some(StatusKind::Withheld),
        ),
        ("Loulo-Gounkoto", q(2025, 4), None),
        ("Long Canyon", q(2023, 4), None),
        (
            "Long Canyon",
            q(2026, 2),
            Some(StatusKind::CareAndMaintenance),
        ),
        ("Kibali", q(2026, 2), None),
    ] {
        assert_eq!(
            status_for(s, subject, &period),
            want,
            "{subject} {period:?}"
        );
    }
}

#[test]
fn test_units_registry_has_usd_m_usd_per_oz() {
    let catalog = fmine_registry::RegistryCatalog::load_from_root(&workspace_root())
        .expect("registries load");
    for code in ["usd_m", "usd_per_oz"] {
        assert!(
            catalog.unit_symbols.iter().any(|u| u == code),
            "{code} not registered"
        );
    }
}
