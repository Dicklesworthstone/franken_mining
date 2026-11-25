//! BD-504e: independently specified cross-workbook answers through the actual XLSX readers.
//! Invented source bytes do not establish the real-issuer T18 census or a completed M0 gate.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use fmine_io::xlsx::{
    MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx_styled,
};
use fmine_numerics::exact_feas::Rat;
use fmine_public::consistency::workbooks::{WorkbookComparison, WorkbookState};
use fmine_public::key::{StatusKind, SubjectStatus};
use fmine_public::sources::{IssuerProfile, SourceKind};
use fmine_public::{Lane, LoadedSource, PublicAssuranceReport, assess, synthetic};

fn source(file: &str, kind: SourceKind, bytes: &[u8]) -> LoadedSource {
    let workbook = Workbook::from_bytes(file, bytes).expect("invented workbook");
    LoadedSource {
        file_name: file.into(),
        kind,
        quarter: "2030Q1".into(),
        url: "https://example.invalid/workbook.xlsx".into(),
        url_basis: "synthetic".into(),
        bytes: bytes.len() as u64,
        census: workbook.cell_census(),
        workbook,
    }
}

fn fixture(cost: &str, share: &str, withheld: bool) -> (IssuerProfile, Vec<LoadedSource>) {
    fixture_with_sold_header(
        cost,
        share,
        withheld,
        "Gold sold (000s ozs attributable share)",
    )
}

fn fixture_with_sold_header(
    cost: &str,
    share: &str,
    withheld: bool,
    sold_header: &str,
) -> (IssuerProfile, Vec<LoadedSource>) {
    let mut profile = synthetic::synthetic_profile();
    profile.subjects = vec!["Site H".into()];
    profile.regions.clear();
    profile.reconstructions.clear();
    profile.missing_facts.clear();
    if withheld {
        profile.subject_statuses.push(SubjectStatus {
            subject: "Site H".into(),
            kind: StatusKind::Withheld,
            first: (2030, 1),
            last: Some((2030, 3)),
            evidence: "synthetic withholding".into(),
        });
    }
    let mut cells = synthetic::synthetic_aisc_cells();
    for (at, value) in &mut cells {
        if at == "A1" {
            *value = MinimalCell::Text("Reconciliation of gold cost of sales to total cash costs and all-in sustaining costs".into());
        }
    }
    if withheld {
        for (at, value) in &mut cells {
            if at == "K14" {
                *value = MinimalCell::Number(0.0);
            }
        }
    }
    let aisc = build_minimal_xlsx_styled(
        &[MinimalSheet {
            name: "Q1 2030".into(),
            hidden: false,
            cells,
        }],
        Some(&synthetic::synthetic_aisc_styles()),
    );
    let text = |s: &str| MinimalCell::Text(s.into());
    let number = |s: &str| MinimalCell::Styled {
        raw: s.into(),
        xf: 1,
    };
    let stats = build_minimal_xlsx_styled(
        &[MinimalSheet {
            name: "QTD".into(),
            hidden: false,
            cells: vec![
                ("A1".into(), text("Summary of Operations")),
                ("A3".into(), text("For the three months ended March 31,")),
                ("B3".into(), MinimalCell::Number(2030.0)),
                ("B4".into(), text("Gold produced (000s ozs 100%)")),
                (
                    "C4".into(),
                    text("Gold produced (000s ozs attributable share)"),
                ),
                ("D4".into(), text(sold_header)),
                ("E4".into(), text("Cost of sales ($/oz)")),
                ("F4".into(), text("Total cash costs ($/oz)")),
                ("G4".into(), text("All-in sustaining costs ($/oz)")),
                ("A5".into(), text(&format!("Site H ({share}%)"))),
                ("B5".into(), number("100")),
                ("C5".into(), number("75")),
                ("D5".into(), number("150")),
                (
                    "E5".into(),
                    if withheld { text("— ") } else { number(cost) },
                ),
                ("F5".into(), number("1500")),
                ("G5".into(), number("1667")),
            ],
        }],
        Some(&MinimalStyles {
            num_fmts: vec![],
            xfs: vec![0, 1],
        }),
    );
    (
        profile,
        vec![
            source("aisc.xlsx", SourceKind::AiscReconciliation, &aisc),
            source("stats.xlsx", SourceKind::MineStats, &stats),
        ],
    )
}

fn run(cost: &str) -> PublicAssuranceReport {
    let (profile, sources) = fixture(cost, "75", false);
    let report = assess(&profile, Lane::Synthetic, sources);
    fmine_public::citation::validate_report(&report).expect("authentic source Claims");
    assert!(report.mine_stats_claims.refusals.is_empty());
    report
}

fn cost(report: &PublicAssuranceReport) -> &WorkbookComparison {
    report
        .workbook_comparisons
        .rows
        .iter()
        .find(|r| r.subject == "Site H" && r.metric == "cos_usd_per_oz")
        .expect("cost comparison retained")
}

#[test]
fn original_workbook_values_match_at_displayed_precision_with_both_sources() {
    let report = run("2000.4");
    let cost = cost(&report);
    assert_eq!(cost.state, WorkbookState::Equal);
    assert_eq!(
        cost.difference.as_ref().unwrap().point,
        Rat::from_integer(0)
    );
    assert_eq!(cost.reconciliation[0].locator, "aisc.xlsx:Q1 2030!K14");
    assert_eq!(cost.mine_stats[0].locator, "stats.xlsx:QTD!E5");
    assert_eq!(report.workbook_comparisons.rows.len(), 4);
    let sold = report
        .workbook_comparisons
        .rows
        .iter()
        .find(|r| r.metric == "au_sold_attr_koz")
        .unwrap();
    assert_eq!(sold.state, WorkbookState::Equal);
    for metric in ["tcc_usd_per_oz", "aisc_usd_per_oz"] {
        let rate = report
            .workbook_comparisons
            .rows
            .iter()
            .find(|r| r.metric == metric)
            .unwrap();
        assert_eq!(rate.state, WorkbookState::Equal);
        assert_eq!(rate.confirmed_share, Some(Rat::new(3, 4).unwrap()));
        assert_eq!(rate.basis_citations.len(), 3);
        assert_eq!(rate.basis_citations[0].at, "aisc.xlsx:Q1 2030!A1");
        assert_eq!(rate.basis_citations[1].at, "aisc.xlsx:Q1 2030!A13");
        assert_eq!(rate.basis_citations[2].at, "stats.xlsx:QTD!A5");
    }
    let json = report.to_json();
    for expected in [
        "\"workbook_comparisons\"",
        "2000.4",
        "\"point\":\"0/1\"",
        "aisc.xlsx:Q1 2030!K14",
        "stats.xlsx:QTD!E5",
    ] {
        assert!(json.contains(expected), "missing {expected}");
    }
}

#[test]
fn changed_workbook_values_distinguish_rounding_overlap_and_disjoint_bounds() {
    let same = run("2000");
    let touch = run("2001");
    let different = run("2002");
    assert_eq!(cost(&touch).state, WorkbookState::RoundingOverlap);
    assert_eq!(
        cost(&touch).difference.as_ref().unwrap().upper,
        Rat::from_integer(0)
    );
    let row = cost(&different);
    assert_eq!(row.state, WorkbookState::Different);
    let difference = row.difference.as_ref().unwrap();
    assert_eq!(
        (&difference.point, &difference.lower, &difference.upper),
        (
            &Rat::from_integer(-2),
            &Rat::from_integer(-3),
            &Rat::from_integer(-1)
        )
    );
    assert_ne!(
        same.workbook_comparisons.digest(),
        different.workbook_comparisons.digest()
    );
    assert_ne!(
        same.ledger_anchor.state_root,
        different.ledger_anchor.state_root
    );
}

#[test]
fn missing_duplicate_and_incompatible_counterparts_cannot_supply_agreement() {
    let (profile, mut sources) = fixture("2000", "75", false);
    let missing = assess(&profile, Lane::Synthetic, sources[..1].to_vec());
    assert_eq!(
        cost(&missing).reason,
        Some("mine_stats_counterpart_missing")
    );
    assert_eq!(missing.workbook_comparisons.rows.len(), 4);
    let mut duplicate = sources[1].clone();
    duplicate.file_name = "another-stats.xlsx".into();
    duplicate.workbook.artifact_path = duplicate.file_name.clone();
    sources.push(duplicate);
    let ambiguous = assess(&profile, Lane::Synthetic, sources);
    assert_eq!(cost(&ambiguous).reason, Some("ambiguous_mine_stats"));
    assert_eq!(cost(&ambiguous).mine_stats.len(), 2);
    let (profile, sources) = fixture("2000", "50", false);
    let basis = assess(&profile, Lane::Synthetic, sources);
    assert_eq!(cost(&basis).reason, Some("ownership_unprinted"));
    assert!(cost(&basis).difference.is_none());
}

#[test]
fn rate_basis_requires_the_gold_title_attributable_denominator_and_matching_label() {
    for remove_denominator in [false, true] {
        let (profile, mut sources) = fixture("2000", "75", false);
        let mut cells = synthetic::synthetic_aisc_cells();
        if remove_denominator {
            for (at, value) in &mut cells {
                if at == "A1" {
                    *value = MinimalCell::Text("Reconciliation of gold cost of sales to total cash costs and all-in sustaining costs".into());
                } else if at == "A13" {
                    *value = MinimalCell::Text("Ounces sold (000s ounces)".into());
                }
            }
        }
        let bytes = build_minimal_xlsx_styled(
            &[MinimalSheet {
                name: "Q1 2030".into(),
                hidden: false,
                cells,
            }],
            Some(&synthetic::synthetic_aisc_styles()),
        );
        sources[0] = source("aisc.xlsx", SourceKind::AiscReconciliation, &bytes);
        let report = assess(&profile, Lane::Synthetic, sources);
        for metric in ["tcc_usd_per_oz", "aisc_usd_per_oz"] {
            let row = report
                .workbook_comparisons
                .rows
                .iter()
                .find(|r| r.metric == metric)
                .unwrap();
            assert_eq!(row.reason, Some("ownership_unprinted"));
            assert!(row.confirmed_share.is_none());
            assert!(row.basis_citations.is_empty());
            assert!(row.difference.is_none());
        }
    }
    let (profile, sources) = fixture("2000", "50", false);
    let mismatch = assess(&profile, Lane::Synthetic, sources);
    for row in &mismatch.workbook_comparisons.rows {
        assert_eq!(row.state, WorkbookState::Unavailable);
    }
}

#[test]
fn unregistered_shares_use_only_explicit_sold_and_cost_basis_evidence() {
    for (sold_header, cos_footnote) in [
        ("Gold sold (000s ozs attributable share)", true),
        ("Gold sold (000s ozs attributable share)", false),
        ("Gold sold (000s ozs 100% basis)", true),
    ] {
        let (mut profile, mut sources) = fixture_with_sold_header("2000", "75", false, sold_header);
        profile.shares.clear();
        if !cos_footnote {
            let mut cells = synthetic::synthetic_aisc_cells();
            for (at, value) in &mut cells {
                match at.as_str() {
                    "A1" => *value = MinimalCell::Text("Reconciliation of gold cost of sales to total cash costs and all-in sustaining costs".into()),
                    "A30" => *value = MinimalCell::Text("No cost basis is stated.".into()),
                    _ => {}
                }
            }
            let bytes = build_minimal_xlsx_styled(
                &[MinimalSheet {
                    name: "Q1 2030".into(),
                    hidden: false,
                    cells,
                }],
                Some(&synthetic::synthetic_aisc_styles()),
            );
            sources[0] = source("aisc.xlsx", SourceKind::AiscReconciliation, &bytes);
        }
        let report = assess(&profile, Lane::Synthetic, sources);
        let sold = report
            .workbook_comparisons
            .rows
            .iter()
            .find(|row| row.metric == "au_sold_attr_koz")
            .expect("sold comparison");
        let cost = cost(&report);
        for (row, admitted, citation_count) in [
            (sold, sold_header.contains("attributable share"), 3),
            (cost, cos_footnote, 4),
        ] {
            assert_eq!(
                row.reconciliation[0].key.ownership_basis,
                fmine_public::key::OwnershipBasis::Unprinted
            );
            if admitted {
                assert_eq!(row.state, WorkbookState::Equal);
                assert_eq!(row.confirmed_share, Some(Rat::new(3, 4).unwrap()));
                assert_eq!(row.basis_citations.len(), citation_count);
                assert_eq!(row.basis_citations[2].at, "stats.xlsx:QTD!A5");
                if citation_count == 4 {
                    assert_eq!(row.basis_citations[3].at, "aisc.xlsx:Q1 2030!A30");
                }
            } else {
                assert_eq!(row.reason, Some("ownership_unprinted"));
                assert!(row.basis_citations.is_empty());
                assert!(row.difference.is_none());
            }
        }
    }
}

#[test]
fn withholding_zero_and_dash_remain_missing_observations() {
    let (profile, sources) = fixture("0", "75", true);
    let report = assess(&profile, Lane::Synthetic, sources);
    let row = cost(&report);
    assert_eq!(row.state, WorkbookState::NotPublished);
    assert!(row.difference.is_none());
    assert!(
        matches!(&row.reconciliation[0].claim.value, fmine_public::key::PublicValue::Nil { token, .. } if token == "0")
    );
    assert!(
        matches!(&row.mine_stats[0].claim.value, fmine_public::key::PublicValue::Nil { token, .. } if token == "— ")
    );
}

#[test]
fn source_order_preserves_projection_and_comparatives_do_not_fill_missing_originals() {
    let (profile, mut sources) = fixture("2000", "75", false);
    let baseline = assess(&profile, Lane::Synthetic, sources.clone());
    sources.reverse();
    let reversed = assess(&profile, Lane::Synthetic, sources.clone());
    assert_eq!(
        baseline.workbook_comparisons.to_json(),
        reversed.workbook_comparisons.to_json()
    );
    sources[0].quarter = "2031Q1".into();
    let comparative = assess(&profile, Lane::Synthetic, sources);
    assert_eq!(
        cost(&comparative).reason,
        Some("mine_stats_counterpart_missing")
    );
}

#[test]
fn contradictory_duplicate_claims_have_a_permutation_stable_refusal() {
    let (profile, sources) = fixture("2000", "75", false);
    let report = assess(&profile, Lane::Synthetic, sources.clone());
    let row = cost(&report);
    let mut duplicate = row.mine_stats[0].clone();
    // Deliberately malformed competing assertion at the same locator: never choose either value.
    duplicate.claim = fmine_public::key::public_claim(
        &sources[1].workbook,
        "QTD",
        "E5",
        duplicate.key.clone(),
        fmine_public::key::PublicValue::Number {
            stored: "2002".into(),
            displayed: Ok("2002".into()),
            under_status: None,
        },
    )
    .claim;
    let mut claims = vec![row.mine_stats[0].clone(), duplicate];
    let first = fmine_public::consistency::workbooks::compare(
        &profile,
        &sources,
        &row.reconciliation,
        &claims,
    );
    claims.reverse();
    let second = fmine_public::consistency::workbooks::compare(
        &profile,
        &sources,
        &row.reconciliation,
        &claims,
    );
    assert_eq!(first.to_json(), second.to_json());
    assert_eq!(first.rows[0].reason, Some("ambiguous_mine_stats"));
    assert!(first.rows[0].difference.is_none());
}

#[test]
fn assurance_json_and_markdown_include_the_separate_workbook_projection() {
    let envelope = fmine_cli::execute_command(
        &fmine_cli::CliCommand::AssureSynthetic,
        std::path::Path::new("."),
    );
    assert_eq!(envelope.exit_code, 0, "{:?}", envelope.refusal);
    assert!(
        envelope
            .payload_json
            .contains("fmine.workbook-comparisons.v1")
    );
    let markdown = envelope.rendered_markdown.expect("human report");
    assert!(markdown.contains("Same-release workbook comparisons"));
    assert!(markdown.contains("Reconciliation minus Mine Stats"));
    assert!(markdown.contains("mine\\_stats\\_counterpart\\_missing"));
}

#[test]
fn assurance_refuses_corrupted_workbook_claims_differences_and_basis_citations() {
    let mut report = run("2000");
    let rows = &mut report.workbook_comparisons.rows;
    let index = |metric| {
        rows.iter()
            .position(|row| row.metric == metric)
            .expect("known comparison")
    };
    let sold = index("au_sold_attr_koz");
    let cos = index("cos_usd_per_oz");
    let aisc = index("aisc_usd_per_oz");
    let tcc = index("tcc_usd_per_oz");
    rows[sold].mine_stats[0].locator = "stats.xlsx:QTD!C5".into();
    rows[cos].difference.as_mut().expect("equal rates").point = Rat::from_integer(1);
    rows[aisc].basis_citations[0].text = "Edited after extraction".into();
    rows[tcc].confirmed_share = Some(Rat::new(4, 5).unwrap());
    let refusal =
        fmine_public::citation::validate_report(&report).expect_err("corrupted comparisons");
    assert_eq!(refusal.code, "FMINE-REF-208");
    for (index, field) in [
        (sold, "mine_stats[0]"),
        (cos, "difference"),
        (aisc, "basis_citations[0]"),
        (tcc, "confirmed_share"),
    ] {
        assert!(
            refusal
                .what
                .contains(&format!("$.workbook_comparisons.rows[{index}].{field}")),
            "missing {field}: {}",
            refusal.what
        );
    }
}
