//! Known-answer and mutation checks for the bounded cited-equality contract, not general prose validation.

use fmine_core::hash::{Digest32, sha256};
use fmine_io::xlsx::{
    MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx_styled,
};
use fmine_public::key::{
    KeyedCell, LineKey, NilReason, OwnershipBasis, Period, PeriodKind, PublicKey, PublicValue,
    Scope, ShareSource, Vintage, classify, from_mine_stats, public_claim,
};
use fmine_public::lint::{LintMode, Verdict, lint_cited, lint_discover};
use fmine_public::mine_stats::read_mine_stats;
use fmine_public::synthetic::synthetic_profile;

#[allow(clippy::expect_used)] // A malformed invented fixture must fail its test immediately.
fn observation(value: &str, edit: impl FnOnce(&mut PublicKey)) -> KeyedCell {
    let mut key = PublicKey {
        issuer: "synthetic".into(),
        subject: "Site A".into(),
        valid_period: Period {
            year: 2030,
            kind: PeriodKind::Quarter(1),
        },
        reporting_vintage: Vintage {
            release: "2030Q1".into(),
            file_sha256: String::new(),
        },
        ownership_basis: OwnershipBasis::Unprinted,
        scope: Scope::Site,
        unit: "usd_per_oz".into(),
        line: LineKey {
            label: "summary:aisc_usd_per_oz".into(),
            occurrence: 0,
        },
    };
    edit(&mut key);
    let file = format!("synthetic-lint-{}.xlsx", key.reporting_vintage.release);
    let format = value.split_once('.').map_or_else(
        || "0".into(),
        |(_, frac)| format!("0.{}", "0".repeat(frac.len())),
    );
    let bytes = build_minimal_xlsx_styled(
        &[MinimalSheet {
            name: "Data".into(),
            hidden: false,
            cells: vec![(
                "B1".into(),
                MinimalCell::Styled {
                    raw: value.into(),
                    xf: 0,
                },
            )],
        }],
        Some(&MinimalStyles {
            num_fmts: vec![(164, format)],
            xfs: vec![164],
        }),
    );
    let workbook = Workbook::from_bytes(&file, &bytes).expect("synthetic source");
    key.reporting_vintage.file_sha256 = Digest32(workbook.sha256).to_hex();
    let value = classify(workbook.sheets[0].cell(1, 2), None).expect("source value");
    public_claim(&workbook, "Data", "B1", key, value)
}

fn sentence(c: &KeyedCell, amount: &str) -> String {
    format!(
        "{} Q1 2030 ownership basis unprinted AISC per ounce sold is [{amount}](fmine:{}).",
        c.key.subject, c.claim.claim_id
    )
}

fn verdict(text: &str, claims: &[KeyedCell]) -> Verdict {
    let report = lint_cited(text, claims);
    assert_eq!(report.findings.len(), 1, "{}", report.to_json());
    report.findings[0].verdict
}

#[allow(clippy::expect_used)]
fn copper_cost_claims(release: &str, site_table: bool) -> Vec<KeyedCell> {
    let text = |s: &str| MinimalCell::Text(s.into());
    let number = |s: &str| MinimalCell::Styled {
        raw: s.into(),
        xf: 1,
    };
    let sheet = if site_table {
        MinimalSheet {
            name: "Minestats1".into(),
            hidden: false,
            cells: vec![
                ("B2".into(), text("Site A")),
                ("A3".into(), text("For the three months ended March 31,")),
                ("B3".into(), MinimalCell::Number(2030.0)),
                ("A4".into(), text("Costs per pound ($/lb)")),
                ("A5".into(), text("C1 cash costs")),
                ("B5".into(), number("3.40")),
                ("A6".into(), text("All-in sustaining costs")),
                ("B6".into(), number("3.75")),
                ("A7".into(), text("Cost of sales")),
                ("B7".into(), number("3.90")),
            ],
        }
    } else {
        MinimalSheet {
            name: "QTD".into(),
            hidden: false,
            cells: vec![
                ("A1".into(), text("For the three months ended March 31,")),
                ("B1".into(), MinimalCell::Number(2030.0)),
                ("B2".into(), text("C1 Cash Costs ($/lb)")),
                ("C2".into(), text("All-in sustaining costs ($/lb)")),
                ("D2".into(), text("Cost of sales ($/lb)")),
                ("A3".into(), text("Site A")),
                ("B3".into(), number("3.40")),
                ("C3".into(), number("3.75")),
                ("D3".into(), number("3.90")),
            ],
        }
    };
    let bytes = build_minimal_xlsx_styled(
        &[sheet],
        Some(&MinimalStyles {
            num_fmts: vec![(164, "0.00".into())],
            xfs: vec![0, 164],
        }),
    );
    let book = Workbook::from_bytes(format!("copper-{release}.xlsx"), &bytes)
        .expect("invented copper source workbook");
    let vocabulary = ["C1 cash costs", "All-in sustaining costs", "Cost of sales"]
        .into_iter()
        .map(|label| (Some("Costs per pound ($/lb)".into()), label.into()))
        .collect();
    let read = read_mine_stats(&book, release, Some(&vocabulary)).expect("copper workbook read");
    assert!(read.refusals.is_empty(), "{:?}", read.refusals);
    let profile = synthetic_profile();
    let claims: Vec<_> = read
        .values
        .iter()
        .map(|value| {
            from_mine_stats(&profile, &book, release, value).expect("canonical copper Claim")
        })
        .collect();
    assert_eq!(claims.len(), 3, "{:?}", read.values);
    assert!(claims.iter().all(|c| c.key.unit == "usd_per_lb"));
    claims
}

#[test]
fn copper_aisc_and_cost_of_sales_follow_registered_pound_denominators() {
    for site_table in [false, true] {
        let claims = copper_cost_claims("2030Q1", site_table);
        for (index, metric, amount, discoverable) in [
            (1, "AISC", "$3.75/lb", true),
            (2, "cost of sales", "$3.90/lb", false),
        ] {
            let text = format!(
                "Site A Q1 2030 ownership basis unprinted {metric} per pound sold is {amount}."
            );
            let source = &claims[index];
            let cited = text.replace(
                amount,
                &format!("[{amount}](fmine:{})", source.claim.claim_id),
            );
            assert_eq!(verdict(&cited, &claims), Verdict::Verified, "{cited}");
            for (from, to, expected) in [
                ("per pound sold", "per ounce sold", Verdict::BasisMismatch),
                (
                    "per pound sold",
                    "per pound produced",
                    Verdict::BasisMismatch,
                ),
                ("per pound sold", "per lb sold", Verdict::Verified),
                ("per pound sold", "", Verdict::Ambiguous),
                ("Q1 2030", "Q2 2030", Verdict::PeriodMismatch),
                (
                    "ownership basis unprinted",
                    "100% basis",
                    Verdict::BasisMismatch,
                ),
                (
                    "ownership basis unprinted",
                    "ownership basis unprinted gold ounces",
                    Verdict::BasisMismatch,
                ),
                ("/lb", "/oz", Verdict::BasisMismatch),
            ] {
                assert_eq!(
                    verdict(&cited.replace(from, to), &claims),
                    expected,
                    "{cited}: {from} -> {to}"
                );
            }
            let precise = amount.replace("/lb", "0/lb");
            assert_eq!(
                verdict(&cited.replace(amount, &precise), &claims),
                Verdict::RoundingBeyondTolerance
            );
            assert_eq!(
                verdict(&cited.replace(amount, "$4.50/lb"), &claims),
                Verdict::ValueMismatch
            );
            if discoverable {
                let report = lint_discover(&text, &claims);
                assert_eq!(report.findings.len(), 1, "{}", report.to_json());
                assert_eq!(
                    report.findings[0].verdict,
                    Verdict::Verified,
                    "{}",
                    report.to_json()
                );
                assert_eq!(
                    report.findings[0].source.as_ref().map(KeyedCell::to_json),
                    Some(source.to_json())
                );
                for changed in [
                    text.replace("per pound sold", "per ounce sold"),
                    text.replace("/lb", "/oz"),
                    text.replace("per pound sold", "per pound produced"),
                ] {
                    assert_eq!(
                        lint_discover(&changed, &claims).findings[0].verdict,
                        Verdict::Uncited,
                        "{changed}"
                    );
                }
            } else {
                assert_eq!(
                    lint_discover(&text, &claims).findings[0].verdict,
                    Verdict::Unchecked
                );
            }
        }
    }
}

#[test]
fn copper_aisc_keeps_competing_vintages_and_source_identity_checks() {
    let original = copper_cost_claims("2030Q1", false);
    let mut later = copper_cost_claims("2031Q1", false);
    let draft = "Site A Q1 2030 ownership basis unprinted AISC per pound sold is $3.75/lb.";
    let cited = draft.replace(
        "$3.75/lb",
        &format!("[$3.75/lb](fmine:{})", original[1].claim.claim_id),
    );
    later.extend(original.clone());
    let report = lint_discover(draft, &later);
    assert_eq!(report.findings[0].verdict, Verdict::Ambiguous);
    assert_eq!(report.findings[0].candidates.len(), 2);
    assert!(report.findings[0].source.is_none());
    assert_eq!(verdict(&cited, &later), Verdict::Stale);
    later.reverse();
    assert_eq!(lint_discover(draft, &later).to_json(), report.to_json());
    let mut changed = original;
    if let PublicValue::Number { displayed, .. } = &mut changed[1].claim.value {
        *displayed = Ok("4.50".into());
    } else {
        panic!("expected numeric AISC source");
    }
    let report = lint_discover(draft, &changed);
    assert_eq!(report.findings[0].verdict, Verdict::Unchecked);
    assert_eq!(report.findings[0].reason, "source_claim_identity_mismatch");
}

#[allow(clippy::expect_used)]
fn physical_claims(
    (section, grade, grade_format): (&str, &str, &str),
    (recovery, recovery_format): (&str, &str),
    release: &str,
) -> Vec<KeyedCell> {
    let text = |s: &str| MinimalCell::Text(s.into());
    let number = |raw: &str, xf| MinimalCell::Styled {
        raw: raw.into(),
        xf,
    };
    let bytes = build_minimal_xlsx_styled(
        &[MinimalSheet {
            name: "Minestats1".into(),
            hidden: false,
            cells: vec![
                ("B2".into(), text("Site A")),
                ("A3".into(), text("For the three months ended March 31,")),
                ("B3".into(), MinimalCell::Number(2030.0)),
                ("A4".into(), text(section)),
                ("A5".into(), text("Processed")),
                ("B5".into(), number(grade, 1)),
                ("A6".into(), text("Open Pit mined")),
                ("B6".into(), number(grade, 1)),
                ("A7".into(), text("Recovery rate (percent)")),
                ("B7".into(), number(recovery, 2)),
                ("A8".into(), text("Oxide Mill")),
                ("B8".into(), number(recovery, 2)),
            ],
        }],
        Some(&MinimalStyles {
            num_fmts: vec![(164, grade_format.into()), (165, recovery_format.into())],
            xfs: vec![0, 164, 165],
        }),
    );
    let book = Workbook::from_bytes(format!("physical-{release}.xlsx"), &bytes)
        .expect("physical source workbook");
    let read = read_mine_stats(&book, release, None).expect("physical source read");
    assert!(read.refusals.is_empty(), "{:?}", read.refusals);
    let profile = synthetic_profile();
    let claims: Vec<_> = read
        .values
        .iter()
        .map(|v| from_mine_stats(&profile, &book, release, v).expect("canonical physical Claim"))
        .collect();
    assert_eq!(claims.len(), 4);
    claims
}

#[test]
fn physical_equalities_use_canonical_units_and_displayed_percent_precision() {
    for (grade, recovery, grade_amount, grade_unit, recovery_unit) in [
        (
            ("Average grade (grams per tonne)", "2.50", "0.00"),
            ("0.915", "0.0%"),
            "2.50 g/t",
            "g_per_t",
            "mass_fraction",
        ),
        (
            ("Average grade", "0.0125", "0.00%"),
            ("91.5", "0.0"),
            "1.25%",
            "mass_fraction",
            "percent",
        ),
        (
            ("Average grade (percent)", "1.25", "0.00"),
            ("0.915", "0.0%"),
            "1.25 percent",
            "percent",
            "mass_fraction",
        ),
    ] {
        let claims = physical_claims(grade, recovery, "2030Q1");
        assert_eq!(claims[0].key.unit, grade_unit);
        assert_eq!(claims[2].key.unit, recovery_unit);
        for (claim, metric, amount) in [
            (&claims[0], "processed grade", grade_amount),
            (&claims[0], "head grade", grade_amount),
            (&claims[2], "recovery", "91.5%"),
            (&claims[2], "recovery rate", "91.5 percent"),
        ] {
            let text = format!("Site A Q1 2030 ownership basis unprinted {metric} is {amount}.");
            let report = lint_discover(&text, &claims);
            assert_eq!(report.findings.len(), 1, "{}", report.to_json());
            let finding = &report.findings[0];
            assert_eq!(finding.verdict, Verdict::Verified, "{}", report.to_json());
            assert_eq!(&text[finding.span.0..finding.span.1], amount);
            assert_eq!(
                finding.source.as_ref().map(KeyedCell::to_json),
                Some(claim.to_json())
            );
            let cited = text.replace(
                amount,
                &format!("[{amount}](fmine:{})", claim.claim.claim_id),
            );
            assert_eq!(verdict(&cited, &claims), Verdict::Verified);
        }
        assert!(matches!(
            &claims[2].claim.value,
            PublicValue::Number { stored, displayed: Ok(displayed), .. }
                if stored == recovery.0 && displayed == if recovery_unit == "mass_fraction" { "91.5%" } else { "91.5" }
        ));
        for (amount, expected) in [
            ("91.50%", Verdict::RoundingBeyondTolerance),
            ("0.915%", Verdict::ValueMismatch),
            ("90.5%", Verdict::ValueMismatch),
        ] {
            let text = format!("Site A Q1 2030 ownership basis unprinted recovery is {amount}.");
            assert_eq!(lint_discover(&text, &claims).findings[0].verdict, expected);
            let cited = text.replace(
                amount,
                &format!("[{amount}](fmine:{})", claims[2].claim.claim_id),
            );
            assert_eq!(verdict(&cited, &claims), expected);
        }
    }
}

#[test]
fn physical_units_cannot_relabel_cost_or_production_claims() {
    for (line, metric, basis, ownership) in [
        (
            "summary:aisc_usd_per_oz",
            "AISC per ounce sold",
            "ownership basis unprinted",
            OwnershipBasis::Unprinted,
        ),
        (
            "summary:au_prod_100_koz",
            "gold production",
            "100% basis",
            OwnershipBasis::Consolidated100,
        ),
    ] {
        for (unit, amount) in [("percent", "91%"), ("g_per_t", "91 g/t")] {
            let claim = observation("91", |k| {
                k.line.label = line.into();
                k.unit = unit.into();
                k.ownership_basis = ownership;
            });
            let draft = format!("Site A Q1 2030 {basis} {metric} is {amount}.");
            let cited = draft.replace(
                amount,
                &format!("[{amount}](fmine:{})", claim.claim.claim_id),
            );
            assert_eq!(
                verdict(&cited, std::slice::from_ref(&claim)),
                Verdict::BasisMismatch
            );
            assert_eq!(
                lint_discover(&draft, &[claim]).findings[0].verdict,
                Verdict::Uncited
            );
        }
    }
}

#[test]
fn physical_discovery_keeps_supports_units_and_competing_sources_distinct() {
    let claims = physical_claims(
        ("Average grade (grams per tonne)", "2.50", "0.00"),
        ("0.915", "0.0%"),
        "2030Q1",
    );
    let draft = "Site A Q1 2030 ownership basis unprinted processed grade is 2.50 g/t.";
    for (from, to, expected) in [
        ("2.50 g/t", "2.500 g/t", Verdict::RoundingBeyondTolerance),
        ("2.50 g/t", "2.5 grams per tonne", Verdict::Verified),
        ("2.50 g/t", "3.50 g/t", Verdict::ValueMismatch),
        ("2.50 g/t", "2.50%", Verdict::Uncited),
        ("ownership basis unprinted", "", Verdict::Ambiguous),
        ("processed grade", "grade", Verdict::Unchecked),
        ("processed grade", "mined grade", Verdict::Unchecked),
        (
            "processed grade",
            "processed grade recovery",
            Verdict::Unchecked,
        ),
    ] {
        let report = lint_discover(&draft.replace(from, to), &claims);
        assert_eq!(report.findings[0].verdict, expected, "{from} -> {to}");
    }
    let cited = draft.replace(
        "2.50 g/t",
        &format!("[2.50 g/t](fmine:{})", claims[0].claim.claim_id),
    );
    assert_eq!(
        verdict(&cited.replace("2.50 g/t", "2.50%"), &claims),
        Verdict::BasisMismatch
    );
    assert_eq!(
        verdict(&cited.replace("Q1 2030", "Q2 2030"), &claims),
        Verdict::PeriodMismatch
    );
    assert_eq!(
        verdict(
            &cited.replace("ownership basis unprinted", "100% basis"),
            &claims
        ),
        Verdict::BasisMismatch
    );
    for (i, metric, amount) in [(1, "processed grade", "2.50 g/t"), (3, "recovery", "91.5%")] {
        let text = format!(
            "Site A Q1 2030 ownership basis unprinted {metric} is [{amount}](fmine:{}).",
            claims[i].claim.claim_id
        );
        assert_eq!(
            verdict(&text, &claims),
            Verdict::Unchecked,
            "mined/circuit source is not the requested support"
        );
        assert_eq!(
            lint_discover(
                &text.replace(
                    &format!("[{amount}](fmine:{})", claims[i].claim.claim_id),
                    amount
                ),
                &claims[i..=i]
            )
            .findings[0]
                .verdict,
            Verdict::Uncited
        );
    }
    let mut repeated = claims.clone();
    repeated.extend(physical_claims(
        ("Average grade (grams per tonne)", "9.99", "0.00"),
        ("91.5", "0.0"),
        "2031Q1",
    ));
    for text in [
        draft,
        "Site A Q1 2030 ownership basis unprinted recovery is 91.5%.",
    ] {
        let report = lint_discover(text, &repeated);
        assert_eq!(report.findings[0].verdict, Verdict::Ambiguous);
        assert_eq!(report.findings[0].candidates.len(), 2);
        assert!(report.findings[0].source.is_none());
    }
    let fraction_without_percent_display = observation("0.915", |k| {
        k.line.label = "site::recovery rate (percent)".into();
        k.unit = "mass_fraction".into();
    });
    let report = lint_discover(
        "Site A Q1 2030 ownership basis unprinted recovery is 0.915%.",
        &[fraction_without_percent_display],
    );
    assert_eq!(report.findings[0].verdict, Verdict::Unchecked);
}

#[allow(clippy::expect_used)]
fn copper_claims(cost_section: &str, production_label: &str) -> Vec<KeyedCell> {
    let text = |s: &str| MinimalCell::Text(s.into());
    let number = |raw: &str, xf| MinimalCell::Styled {
        raw: raw.into(),
        xf,
    };
    let bytes = build_minimal_xlsx_styled(
        &[
            MinimalSheet {
                name: "QTD".into(),
                hidden: false,
                cells: vec![
                    ("A1".into(), text("Summary of Operations")),
                    ("A3".into(), text("For the three months ended March 31,")),
                    ("B3".into(), MinimalCell::Number(2030.0)),
                    (
                        "B4".into(),
                        text("Copper produced (thousands of tonnes 100%)"),
                    ),
                    (
                        "C4".into(),
                        text("Copper produced (thousands of tonnes attributable)"),
                    ),
                    ("D4".into(), text("C1 Cash Costs ($/lb)4")),
                    ("A5".into(), text("Site A (50%)")),
                    ("B5".into(), number("50.0", 2)),
                    ("C5".into(), number("25.0", 2)),
                    ("D5".into(), number("1.25", 1)),
                ],
            },
            MinimalSheet {
                name: "Minestats4".into(),
                hidden: false,
                cells: vec![
                    ("A1".into(), text("Mine Statistics (Copper)")),
                    ("B2".into(), text("Site A (50%)")),
                    ("A3".into(), text("For the three months ended March 31,")),
                    ("B3".into(), MinimalCell::Number(2030.0)),
                    ("A4".into(), text(cost_section)),
                    ("A5".into(), text("C1 Cash Costs1")),
                    ("B5".into(), number("1.25", 1)),
                    ("A6".into(), text(production_label)),
                    ("B6".into(), number("25.0", 2)),
                ],
            },
        ],
        Some(&MinimalStyles {
            num_fmts: vec![(164, "0.00".into()), (165, "0.0".into())],
            xfs: vec![0, 164, 165],
        }),
    );
    let book = Workbook::from_bytes("synthetic-copper.xlsx", &bytes).expect("copper workbook");
    let read = read_mine_stats(&book, "2030Q1", None).expect("copper source read");
    assert!(read.refusals.is_empty(), "{:?}", read.refusals);
    let profile = synthetic_profile();
    let claims: Vec<_> = read
        .values
        .iter()
        .map(|v| from_mine_stats(&profile, &book, "2030Q1", v).expect("canonical copper Claim"))
        .collect();
    assert_eq!(claims.len(), 5);
    claims
}

#[test]
fn copper_equalities_read_summary_and_site_keys_with_exact_units() {
    for section in ["Costs per pound", "Costs per pound ($/lb)"] {
        let claims = copper_claims(section, "Production (Thousands of tonnes)");
        for claim in &claims {
            let c1 = claim.key.line.label.contains("c1");
            let (basis, amount) = match claim.key.ownership_basis {
                OwnershipBasis::Consolidated100 => ("100% basis", "50.0 kt"),
                OwnershipBasis::Attributable { .. } => {
                    ("attributable", if c1 { "US$1.25/lb" } else { "25.0 kt" })
                }
                other => panic!("unexpected canonical copper basis: {other:?}"),
            };
            assert_eq!(claim.key.unit, if c1 { "usd_per_lb" } else { "kt" });
            let metrics: &[&str] = if c1 {
                &[
                    "C1",
                    "copper C1",
                    "C1 cash costs",
                    "non-GAAP copper C1 cash costs",
                ]
            } else {
                &["copper production", "copper produced"]
            };
            for metric in metrics {
                let denominator = if c1 { "per pound sold" } else { "" };
                let draft = format!("Site A Q1 2030 {basis} {metric} {denominator} is {amount}.");
                let cited = draft.replace(
                    amount,
                    &format!("[{amount}](fmine:{})", claim.claim.claim_id),
                );
                assert_eq!(
                    verdict(&cited, std::slice::from_ref(claim)),
                    Verdict::Verified,
                    "{cited}"
                );
                let report = lint_discover(&draft, std::slice::from_ref(claim));
                assert_eq!(report.findings.len(), 1, "{}", report.to_json());
                assert_eq!(report.findings[0].verdict, Verdict::Verified, "{draft}");
                assert_eq!(
                    report.findings[0].claim_id.as_deref(),
                    Some(claim.claim.claim_id.to_string().as_str())
                );
            }
        }
    }
}

#[test]
fn copper_c1_rejects_wrong_denominators_definitions_units_and_precision() {
    let claims = copper_claims("Costs per pound", "Production (Thousands of tonnes)");
    let claim = claims
        .iter()
        .find(|c| c.key.line.label == "summary:c1_usd_per_lb")
        .unwrap();
    let draft = "Site A Q1 2030 attributable copper C1 cash costs per pound sold is $1.25/lb.";
    let cited = draft.replace(
        "$1.25/lb",
        &format!("[$1.25/lb](fmine:{})", claim.claim.claim_id),
    );
    for (from, to, cited_verdict, discovered_verdict) in [
        (
            "per pound sold",
            "per lb sold",
            Verdict::Verified,
            Verdict::Verified,
        ),
        ("per pound sold", "", Verdict::Ambiguous, Verdict::Ambiguous),
        (
            "per pound sold",
            "per pound produced",
            Verdict::BasisMismatch,
            Verdict::Uncited,
        ),
        (
            "per pound sold",
            "per ounce sold",
            Verdict::BasisMismatch,
            Verdict::Uncited,
        ),
        (
            "per pound sold",
            "per oz produced",
            Verdict::BasisMismatch,
            Verdict::Uncited,
        ),
        (
            "per pound sold",
            "per lb sold per ounce sold",
            Verdict::BasisMismatch,
            Verdict::Uncited,
        ),
        ("attributable", "", Verdict::Ambiguous, Verdict::Ambiguous),
        (
            "attributable",
            "100% basis",
            Verdict::BasisMismatch,
            Verdict::Uncited,
        ),
        (
            "attributable",
            "attributable by-product",
            Verdict::BasisMismatch,
            Verdict::Uncited,
        ),
        (
            "attributable",
            "attributable co-product",
            Verdict::BasisMismatch,
            Verdict::Uncited,
        ),
        (
            "attributable",
            "attributable gold ounces",
            Verdict::BasisMismatch,
            Verdict::Uncited,
        ),
        (
            "attributable",
            "attributable API scorecard",
            Verdict::BasisMismatch,
            Verdict::Uncited,
        ),
        (
            "$1.25/lb",
            "$1.25/oz",
            Verdict::BasisMismatch,
            Verdict::Uncited,
        ),
        (
            "$1.25/lb",
            "C$1.25/lb",
            Verdict::Unchecked,
            Verdict::Unchecked,
        ),
        (
            "$1.25/lb",
            "$1.30/lb",
            Verdict::ValueMismatch,
            Verdict::ValueMismatch,
        ),
        (
            "$1.25/lb",
            "$1.250/lb",
            Verdict::RoundingBeyondTolerance,
            Verdict::RoundingBeyondTolerance,
        ),
        (
            "copper C1 cash costs",
            "TCC",
            Verdict::BasisMismatch,
            Verdict::Ambiguous,
        ),
        (" is ", " is not ", Verdict::Unchecked, Verdict::Unchecked),
    ] {
        assert_eq!(
            verdict(&cited.replace(from, to), std::slice::from_ref(claim)),
            cited_verdict,
            "{from} -> {to}"
        );
        let report = lint_discover(&draft.replace(from, to), std::slice::from_ref(claim));
        assert_eq!(
            report.findings.len(),
            1,
            "{from} -> {to}: {}",
            report.to_json()
        );
        assert_eq!(
            report.findings[0].verdict,
            discovered_verdict,
            "{from} -> {to}: {}",
            report.to_json()
        );
    }
}

#[test]
fn copper_discovery_keeps_metals_vintages_and_source_units_distinct() {
    let claims = copper_claims("Costs per pound", "Production (Thousands of tonnes)");
    for draft in [
        "Site A Q1 2030 attributable copper C1 per pound sold is $1.25/lb.",
        "Site A Q1 2030 attributable copper production is 25.0 kt.",
    ] {
        let report = lint_discover(draft, &claims);
        assert_eq!(report.findings[0].verdict, Verdict::Ambiguous);
        assert_eq!(
            report.findings[0].candidates.len(),
            2,
            "summary and site are distinct source assertions"
        );
        assert!(report.findings[0].source.is_none());
    }
    let prod = claims
        .iter()
        .find(|c| c.key.line.label == "summary:cu_prod_attr_kt")
        .unwrap();
    let draft = "Site A Q1 2030 attributable copper production is 25.0 kt.";
    let cited = draft.replace(
        "25.0 kt",
        &format!("[25.0 kt](fmine:{})", prod.claim.claim_id),
    );
    for (from, to) in [
        ("copper production", "gold production"),
        ("copper production", "production"),
        ("copper production", "copper production per pound sold"),
        ("copper production", "copper production gold ounces"),
        ("25.0 kt", "25.0 koz"),
        ("25.0 kt", "25.0 tonnes"),
        ("25.0 kt", "US$25.0 kt"),
    ] {
        assert_eq!(
            verdict(&cited.replace(from, to), std::slice::from_ref(prod)),
            Verdict::BasisMismatch,
            "{from} -> {to}"
        );
        assert_eq!(
            lint_discover(&draft.replace(from, to), std::slice::from_ref(prod)).findings[0].verdict,
            Verdict::Uncited,
            "{from} -> {to}"
        );
    }
    let historical = copper_claims("Costs per pound", "Production (Millions of pounds)");
    let source = historical
        .iter()
        .find(|c| c.key.line.label == "site::production (millions of pounds)")
        .unwrap();
    assert_ne!(source.key.unit, "kt");
    let wrongly_converted = draft.replace(
        "25.0 kt",
        &format!("[25.0 kt](fmine:{})", source.claim.claim_id),
    );
    assert_eq!(
        verdict(&wrongly_converted, std::slice::from_ref(source)),
        Verdict::Unchecked
    );
    assert_eq!(
        lint_discover(draft, std::slice::from_ref(source)).findings[0].verdict,
        Verdict::Uncited
    );

    let gold = observation("25.0", |k| {
        k.line.label = "summary:au_prod_attr_koz".into();
        k.unit = "koz".into();
        k.ownership_basis = prod.key.ownership_basis;
    });
    assert_eq!(
        lint_discover(draft, &[gold]).findings[0].verdict,
        Verdict::Uncited
    );
    let later = observation("99.0", |k| {
        k.line.label = prod.key.line.label.clone();
        k.unit = prod.key.unit.clone();
        k.ownership_basis = prod.key.ownership_basis;
        k.reporting_vintage.release = "2030Q2".into();
    });
    let report = lint_discover(draft, &[prod.clone(), later.clone()]);
    assert_eq!(report.findings[0].verdict, Verdict::Ambiguous);
    assert_eq!(
        report.findings[0].candidates.len(),
        2,
        "value and vintage must not break a semantic tie"
    );
    assert_eq!(verdict(&cited, &[prod.clone(), later]), Verdict::Stale);
}

#[test]
fn printed_intervals_are_contained_exactly_without_inventing_precision() {
    let c = observation("1637", |_| {});
    for (amount, expected) in [
        ("$1,637/oz", Verdict::Verified),
        ("US$1,637/oz", Verdict::Verified),
        ("$1.637k/oz", Verdict::Verified),
        ("$1.6 thousand/oz", Verdict::Verified),
        ("$1,640/oz", Verdict::ValueMismatch),
        ("$1.63k/oz", Verdict::ValueMismatch),
        ("$1,637.0/oz", Verdict::RoundingBeyondTolerance),
        ("$1,637.2/oz", Verdict::RoundingBeyondTolerance),
    ] {
        assert_eq!(
            verdict(&sentence(&c, amount), std::slice::from_ref(&c)),
            expected,
            "{amount}"
        );
    }
    let huge = observation("9007199254740992", |_| {});
    assert_eq!(
        verdict(
            &sentence(&huge, "$9007199254740990/oz"),
            std::slice::from_ref(&huge)
        ),
        Verdict::Verified,
        "the source format renders fifteen significant digits: {:?}",
        huge.claim.value
    );
    assert_ne!(
        verdict(
            &sentence(&huge, "$9007199254740992/oz"),
            std::slice::from_ref(&huge)
        ),
        Verdict::Verified
    );
    let negative = observation("-708", |_| {});
    assert_eq!(
        verdict(
            &sentence(&negative, "($708/oz)"),
            std::slice::from_ref(&negative)
        ),
        Verdict::Verified
    );
}

#[test]
fn equal_value_links_cannot_swap_subject_metric_period_ownership_or_units() {
    let a = observation("1637", |_| {});
    let b = observation("1637", |k| k.subject = "Site B".into());
    let cost = observation("1637", |k| k.line.label = "summary:cos_usd_per_oz".into());
    let claims = [a.clone(), b.clone(), cost.clone()];
    let valid = sentence(&a, "$1,637/oz");
    assert_eq!(
        verdict(
            &valid.replace(&a.claim.claim_id.to_string(), &b.claim.claim_id.to_string()),
            &claims
        ),
        Verdict::BasisMismatch
    );
    assert_eq!(
        verdict(
            &valid.replace(
                &a.claim.claim_id.to_string(),
                &cost.claim.claim_id.to_string()
            ),
            &claims
        ),
        Verdict::BasisMismatch
    );
    for (from, to, expected) in [
        ("Q1 2030", "Q2 2030", Verdict::PeriodMismatch),
        (
            "ownership basis unprinted",
            "100% basis",
            Verdict::BasisMismatch,
        ),
        (
            "per ounce sold",
            "per ounce produced",
            Verdict::BasisMismatch,
        ),
        ("$1,637/oz", "$1,637/lb", Verdict::BasisMismatch),
        ("$1,637/oz", "C$1,637/oz", Verdict::Unchecked),
        ("$1,637/oz", "C$1,637 USD/oz", Verdict::BasisMismatch),
        ("ownership basis unprinted ", "", Verdict::Ambiguous),
        ("per ounce sold ", "", Verdict::Ambiguous),
        ("AISC", "scorecard AISC", Verdict::Unchecked),
        ("AISC", "API scorecard AISC", Verdict::BasisMismatch),
    ] {
        assert_eq!(
            verdict(&valid.replace(from, to), &claims),
            expected,
            "{from} -> {to}"
        );
    }
    let cos = sentence(&cost, "$1,637/oz").replace("AISC", "cost of sales");
    assert_eq!(verdict(&cos, &claims), Verdict::Verified);
    assert_eq!(
        verdict(
            &cos.replace("cost of sales", "by-product cost of sales"),
            &claims
        ),
        Verdict::BasisMismatch
    );
}

#[test]
fn distinct_credit_and_production_bases_have_positive_known_answers() {
    let co = observation("1100", |k| {
        k.line.label = "total cash costs per ounce (on a co-product basis)".into()
    });
    let draft = sentence(&co, "$1,100/oz").replace("AISC", "co-product TCC");
    assert_eq!(
        verdict(&draft, std::slice::from_ref(&co)),
        Verdict::Verified
    );
    let discovered = draft.replace(
        &format!("[$1,100/oz](fmine:{})", co.claim.claim_id),
        "$1,100/oz",
    );
    assert_eq!(
        lint_discover(&discovered, std::slice::from_ref(&co)).findings[0].verdict,
        Verdict::Verified
    );
    assert_eq!(
        verdict(
            &draft.replace("co-product", "by-product"),
            std::slice::from_ref(&co)
        ),
        Verdict::BasisMismatch
    );
    assert_eq!(
        verdict(&draft.replace("co-product ", ""), &[co]),
        Verdict::Ambiguous
    );
    for (basis, word) in [
        (OwnershipBasis::Consolidated100, "100% basis"),
        (
            OwnershipBasis::Attributable {
                num: 21,
                den: 25,
                source: ShareSource::Printed,
            },
            "attributable",
        ),
    ] {
        let c = observation("35", |k| {
            k.unit = "koz".into();
            k.ownership_basis = basis;
            k.line.label = if basis == OwnershipBasis::Consolidated100 {
                "summary:au_prod_100_koz"
            } else {
                "summary:au_prod_attr_koz"
            }
            .into();
        });
        let draft = format!(
            "Site A Q1 2030 {word} gold production is [35 koz](fmine:{}).",
            c.claim.claim_id
        );
        assert_eq!(verdict(&draft, std::slice::from_ref(&c)), Verdict::Verified);
        let discovered = draft.replace(&format!("[35 koz](fmine:{})", c.claim.claim_id), "35 koz");
        assert_eq!(
            lint_discover(&discovered, std::slice::from_ref(&c)).findings[0].verdict,
            Verdict::Verified
        );
        assert_eq!(
            verdict(&draft.replace("35 koz", "0.035 Moz"), &[c]),
            Verdict::BasisMismatch,
            "no implicit unit conversion"
        );
    }
}

#[test]
fn negation_comparisons_ranges_extra_links_and_probabilities_do_not_verify() {
    let c = observation("1637", |_| {});
    let valid = sentence(&c, "$1,637/oz");
    for draft in [
        valid.replace(" is ", " is not "),
        valid.replace(" is ", " is above "),
        valid.replace(" is ", " is probably "),
        format!("If production rises, {valid}"),
        valid.replace(".", "?"),
        valid.replace("$1,637/oz", "$1,637 to 1,640/oz"),
        valid.replace("AISC", "AISC SECRET_DRAFT_TEXT"),
        valid.replace(").", &format!(")[not](fmine:{}).", c.claim.claim_id)),
    ] {
        let report = lint_cited(&draft, std::slice::from_ref(&c));
        assert!(!report.findings.is_empty(), "{draft}");
        assert!(
            report
                .findings
                .iter()
                .all(|f| f.verdict != Verdict::Verified),
            "{draft}"
        );
        assert!(!report.unchecked_spans.is_empty());
        assert!(!report.to_json().contains("SECRET_DRAFT_TEXT"));
        assert!(!report.to_markdown().contains("SECRET_DRAFT_TEXT"));
    }
    assert_eq!(
        verdict(
            &valid.replace("AISC", "probability of AISC"),
            std::slice::from_ref(&c)
        ),
        Verdict::UncalibratedProbability
    );
    let unlinked = valid.replace(
        &format!("[$1,637/oz](fmine:{})", c.claim.claim_id),
        "$1,637/oz",
    );
    assert_eq!(
        verdict(&unlinked, std::slice::from_ref(&c)),
        Verdict::Uncited
    );
    assert_eq!(
        verdict(
            &valid.replace(
                &c.claim.claim_id.to_string(),
                "clm_00000000000000000000000000000000"
            ),
            &[c]
        ),
        Verdict::Ambiguous
    );
    let prose = lint_cited(
        "A qualitative statement remains outside the supported predicates.",
        &[],
    );
    assert!(prose.findings.is_empty());
    assert_eq!(prose.unchecked_spans.len(), 1);
}

#[test]
fn snapshot_latest_and_explicit_staleness_preserve_old_assertions() {
    let first = observation("1479", |_| {});
    let later = observation("1474", |k| k.reporting_vintage.release = "2031Q1".into());
    let draft = sentence(&first, "$1,479/oz");
    assert_eq!(
        verdict(&draft, std::slice::from_ref(&first)),
        Verdict::Verified
    );
    let mut claims = vec![first.clone(), later.clone()];
    let report = lint_cited(&draft, &claims);
    assert_eq!(report.findings[0].verdict, Verdict::Stale);
    assert_eq!(
        report.findings[0].later[0].claim.claim_id,
        later.claim.claim_id
    );
    assert_eq!(
        verdict(&sentence(&later, "$1,474/oz"), &claims),
        Verdict::Verified
    );
    claims.reverse();
    assert_eq!(lint_cited(&draft, &claims).to_json(), report.to_json());
    let mut explicit = first.clone();
    explicit.claim.stale = true;
    assert_eq!(verdict(&draft, &[explicit]), Verdict::Stale);
    assert_eq!(
        verdict(&draft, &[first.clone(), first.clone()]),
        Verdict::Ambiguous
    );
    let concurrent = observation("1480", |_| {});
    assert_eq!(verdict(&draft, &[first, concurrent]), Verdict::Ambiguous);
}

#[test]
fn token_display_and_printed_basis_limits_never_become_verified_numbers() {
    let original = observation("0", |_| {});
    let draft = sentence(&original, "$0/oz");
    for value in [
        PublicValue::Missing,
        PublicValue::NotApplicable {
            token: "n/a".into(),
        },
        PublicValue::Nil {
            token: "0".into(),
            reason: NilReason::Withheld,
        },
        PublicValue::Number {
            stored: "1.2".into(),
            displayed: Err("general_non_integer"),
            under_status: None,
        },
    ] {
        let mut c = original.clone();
        c.claim.value = value;
        assert_eq!(verdict(&draft, &[c]), Verdict::Unchecked);
    }
    let mut c = observation("1637", |k| {
        k.ownership_basis = OwnershipBasis::Attributable {
            num: 49,
            den: 200,
            source: ShareSource::Registry,
        }
    });
    c.printed_basis = OwnershipBasis::Attributable {
        num: 19,
        den: 40,
        source: ShareSource::Printed,
    };
    c.basis_evidence = Some("SYNTHETIC-CORRECTION".into());
    let draft = sentence(&c, "$1,637/oz").replace("ownership basis unprinted", "attributable");
    assert_eq!(verdict(&draft, &[c]), Verdict::Ambiguous);
}

#[test]
fn report_binds_inputs_findings_and_exact_spans_without_copying_prose() {
    let c = observation("1637", |_| {});
    let draft = format!(
        "Private context remains unchecked.\n{}",
        sentence(&c, "$1,637/oz")
    );
    let report = lint_cited(&draft, std::slice::from_ref(&c));
    assert_eq!(report.draft_digest, sha256(draft.as_bytes()));
    let f = &report.findings[0];
    assert_eq!(&draft[f.span.0..f.span.1], f.raw);
    assert_eq!(f.source.as_ref().expect("canonical source"), &c);
    let json = report.to_json();
    assert!(json.contains("\"numbers\":\"1\""));
    assert!(json.contains("\"draft_sha256\""));
    assert!(json.contains(&c.key.reporting_vintage.file_sha256));
    assert!(!json.contains("Private context"));
    let mut mutated = report.clone();
    mutated.findings[0].verdict = Verdict::ValueMismatch;
    assert_ne!(mutated.digest(), report.digest());
    assert_ne!(
        lint_cited(&(draft.clone() + "\n"), &[c]).digest(),
        report.digest()
    );
}

fn unlinked(c: &KeyedCell, amount: &str) -> String {
    sentence(c, amount).replace(&format!("[{amount}](fmine:{})", c.claim.claim_id), amount)
}

#[test]
fn discovery_unique_claim_runs_the_same_exact_checks_and_preserves_spans() {
    let c = observation("1637", |_| {});
    for (amount, expected) in [
        ("$1,637/oz", Verdict::Verified),
        ("US$1,637/oz", Verdict::Verified),
        ("$1.6 thousand/oz", Verdict::Verified),
        ("$1,640/oz", Verdict::ValueMismatch),
        ("$1,637.0/oz", Verdict::RoundingBeyondTolerance),
    ] {
        let draft = unlinked(&c, amount);
        let report = lint_discover(&draft, std::slice::from_ref(&c));
        assert_eq!(report.mode, LintMode::Discovered);
        assert_eq!(report.findings.len(), 1);
        let f = &report.findings[0];
        assert_eq!((f.mode, f.verdict), (LintMode::Discovered, expected));
        assert_eq!(f.source.as_ref(), Some(&c));
        assert_eq!(
            f.claim_id.as_deref(),
            Some(c.claim.claim_id.to_string().as_str())
        );
        assert_eq!(&draft[f.span.0..f.span.1], f.raw);
        assert!(report.to_json().contains("\"mode\":\"discovered\""));
        let markdown = report.to_markdown();
        assert!(markdown.contains("| discovered |"));
        assert!(markdown.contains(&c.claim.claim_id.to_string()));
        assert!(markdown.contains(&c.locator));
        assert!(
            report
                .to_json()
                .contains("General-prose discovery precision has not been measured")
        );
        assert_eq!(
            lint_cited(&draft, std::slice::from_ref(&c)).findings[0].verdict,
            Verdict::Uncited
        );
    }
}

#[test]
fn discovery_never_breaks_a_semantic_tie_by_value_vintage_or_source_order() {
    let first = observation("1637", |_| {});
    let wrong_value = observation("900", |_| {});
    let later = observation("1638", |k| k.reporting_vintage.release = "2031Q1".into());
    let draft = unlinked(&first, "$1,637/oz");
    for competitor in [wrong_value, later, first.clone()] {
        let mut claims = vec![first.clone(), competitor];
        let report = lint_discover(&draft, &claims);
        let f = &report.findings[0];
        assert_eq!(f.verdict, Verdict::Ambiguous);
        assert_eq!(f.candidates.len(), 2);
        assert!(f.source.is_none());
        assert!(f.claim_id.is_none());
        let markdown = report.to_markdown();
        for candidate in &f.candidates {
            assert!(markdown.contains(&candidate.claim.claim_id.to_string()));
            assert!(markdown.contains(&candidate.locator));
        }
        claims.reverse();
        assert_eq!(report.to_json(), lint_discover(&draft, &claims).to_json());
    }
    let mut stale = first.clone();
    stale.claim.stale = true;
    assert_eq!(
        lint_discover(&draft, &[stale]).findings[0].verdict,
        Verdict::Stale
    );
    let mut missing = first;
    missing.claim.value = PublicValue::Missing;
    assert_eq!(
        lint_discover(&draft, &[missing]).findings[0].verdict,
        Verdict::Unchecked
    );
}

#[test]
fn discovery_requires_every_semantic_key_and_never_repairs_explicit_citations() {
    let a = observation("1637", |_| {});
    let b = observation("1637", |k| k.subject = "Site B".into());
    let claims = [a.clone(), b.clone()];
    let valid = unlinked(&a, "$1,637/oz");
    for (from, to, expected) in [
        ("Site A", "Site A Site B", Verdict::Ambiguous),
        ("Site A", "Unregistered Mine", Verdict::Unchecked),
        ("AISC", "AISC TCC", Verdict::Unchecked),
        ("AISC", "recovery", Verdict::Uncited),
        ("AISC", "cost of sales", Verdict::Unchecked),
        ("AISC", "sales", Verdict::Unchecked),
        ("Q1 2030", "Q2 2030", Verdict::Uncited),
        ("Q1 2030", "", Verdict::Ambiguous),
        ("ownership basis unprinted", "", Verdict::Ambiguous),
        ("ownership basis unprinted", "100% basis", Verdict::Uncited),
        ("per ounce sold", "", Verdict::Ambiguous),
        ("per ounce sold", "per ounce produced", Verdict::Uncited),
        ("$1,637/oz", "$1,637/lb", Verdict::Uncited),
        (" is ", " is not ", Verdict::Unchecked),
    ] {
        let report = lint_discover(&valid.replace(from, to), &claims);
        assert_eq!(report.findings.len(), 1, "{from} -> {to}");
        assert_eq!(report.findings[0].verdict, expected, "{from} -> {to}");
    }
    let wrong_link = sentence(&a, "$1,637/oz")
        .replace(&a.claim.claim_id.to_string(), &b.claim.claim_id.to_string());
    let report = lint_discover(&wrong_link, &claims);
    assert_eq!(report.findings[0].mode, LintMode::Cited);
    assert_eq!(report.findings[0].verdict, Verdict::BasisMismatch);
    let malformed = sentence(&a, "$1,637/oz").replace(&a.claim.claim_id.to_string(), "not-a-claim");
    assert_ne!(
        lint_discover(&malformed, &claims).findings[0].verdict,
        Verdict::Verified
    );
    let private = format!("PRIVATE_DRAFT_PROSE.\n{valid}");
    let report = lint_discover(&private, &claims);
    assert_eq!(report.findings[0].verdict, Verdict::Verified);
    assert!(!report.to_json().contains("PRIVATE_DRAFT_PROSE"));
    assert!(!report.to_markdown().contains("PRIVATE_DRAFT_PROSE"));
    assert_eq!(report.unchecked_spans.len(), 1);
}

#[test]
#[ignore = "needs the registered Barrick workbooks"]
fn real_mine_stats_equalities_and_e474_staleness_use_canonical_source_claims() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile = fmine_public::load_issuer_profile(&root, "barrick").expect("profile");
    let inventory =
        fmine_public::key::mine_stats::load_claims(&profile, &root).expect("verified claims");
    assert!(inventory.refusals.is_empty());
    let at = |suffix: &str| {
        inventory
            .claims
            .iter()
            .find(|c| c.locator == suffix)
            .expect("cited source")
    };
    let mut draft = String::new();
    for (cell, basis, metric, amount) in [
        ("G19", "attributable", "AISC per ounce sold", "$2,577/oz"),
        (
            "F19",
            "attributable",
            "by-product TCC per ounce sold",
            "$2,164/oz",
        ),
        (
            "E19",
            "attributable",
            "cost of sales per ounce sold",
            "$2,736/oz",
        ),
        ("B19", "100% basis", "gold production", "41 koz"),
        ("C19", "attributable", "gold production", "35 koz"),
        ("D19", "attributable", "gold sales", "37 koz"),
    ] {
        let c = at(&format!("Barrick_Q2_2026_Mine_Stats.xlsx:QTD!{cell}"));
        draft.push_str(&format!(
            "North Mara Q2 2026 {basis} {metric} is [{amount}](fmine:{}).\n",
            c.claim.claim_id
        ));
    }
    let report = lint_cited(&draft, &inventory.claims);
    assert_eq!(report.findings.len(), 6);
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.verdict == Verdict::Verified),
        "{}",
        report.to_json()
    );
    let first = at("Barrick_Q1_2024_Mine_Stats.xlsx:QTD!G23");
    let e474 = format!(
        "Total Gold Q1 2024 ownership basis unprinted AISC per ounce sold is [$1,479/oz](fmine:{}).",
        first.claim.claim_id
    );
    let full = lint_cited(&e474, &inventory.claims);
    assert_eq!(full.findings[0].verdict, Verdict::Stale);
    assert!(
        full.findings[0]
            .later
            .iter()
            .any(|c| c.locator == "Barrick_Q1_2025_Mine_Stats.xlsx:QTD!N22")
    );
    let earlier: Vec<_> = inventory
        .claims
        .iter()
        .filter(|c| c.key.reporting_vintage.release.as_str() <= "2024Q1")
        .cloned()
        .collect();
    assert_eq!(verdict(&e474, &earlier), Verdict::Verified);
}
