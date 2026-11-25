//! Synthetic workbooks exercise the source adapters and identity ledgers, not mocked waterfall results.
//! Hand answers: C/Q = 60/50 -> 90/60 ($M/koz), hence 1200 -> 1500 USD/oz. Printed endpoints
//! 1201 -> 1501 have equal nonzero gaps of 1. The signed fallback frontier is TCC 50 -> 75 and
//! sustaining 10 -> 15: bars 1375/3, 275/3 and -250, summing exactly to 300.
#![cfg(test)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use fmine_io::xlsx::{
    MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx_styled,
    col_index_to_letters,
};
use fmine_numerics::exact_feas::Rat;
use fmine_public::key::{Period, PeriodKind, parse_period_code};
use fmine_public::sources::{IssuerProfile, RegisteredShare, SourceKind, load_issuer_profile};
use fmine_public::waterfall::{
    BarPoint, Bridge, PairOutcome, WaterfallMetric, WaterfallReport, pair_sites,
    pair_sites_between, pair_sites_between_for_metric,
};
use fmine_public::{
    Lane, LoadedSource, PublicAssuranceReport, assess, load_registered_sources, synthetic,
};

const SITES: [&str; 4] = ["Lmdi Site", "Heap Site", "Zero Site", "Basis Site"];

fn text(s: &str) -> MinimalCell {
    MinimalCell::Text(s.to_string())
}
fn number(n: i64) -> MinimalCell {
    MinimalCell::Number(n as f64)
}
fn styled(raw: &str, xf: u32) -> MinimalCell {
    MinimalCell::Styled {
        raw: raw.to_string(),
        xf,
    }
}

fn aisc(year: u16) -> Vec<MinimalSheet> {
    let mut cells = vec![
        (
            "A1".to_string(),
            text(
                "Reconciliation of gold cost of sales to total cash costs and all-in sustaining costs",
            ),
        ),
        ("A20".to_string(), text(synthetic::SYNTHETIC_SHARE_FOOTNOTE)),
    ];
    for (row, label) in [
        (4, "Cost of sales applicable to gold production"),
        (5, "Non-controlling interests"),
        (6, "Total cash costs"),
        (7, "Minesite sustaining"),
        (8, "All-in sustaining costs"),
        (9, "Ounces sold (000s ounces, attributable basis)"),
        (10, "Cost of sales per ounce"),
        (11, "Total cash costs per ounce"),
        (12, "All-in sustaining costs per ounce"),
    ] {
        cells.push((format!("A{row}"), text(label)));
    }
    for (i, site) in SITES.iter().enumerate() {
        let col = col_index_to_letters(2 + 2 * i as u32 + u32::from(year == 2031));
        cells.push((format!("{col}2"), text(site)));
        let later = year == 2031;
        let (tcc, sustaining, sold, rate) = if later {
            (75, 15, 60, 1501)
        } else {
            (50, 10, 50, 1201)
        };
        for (row, value) in [
            (4, 2 * tcc),
            (5, -tcc),
            (6, tcc),
            (7, sustaining),
            (8, tcc + sustaining),
            (
                9,
                if later && *site == "Zero Site" {
                    0
                } else {
                    sold
                },
            ),
            (10, 1000 * tcc / sold),
            (11, 1000 * tcc / sold),
            (12, rate),
        ] {
            cells.push((format!("{col}{row}"), number(value)));
        }
    }
    vec![MinimalSheet {
        name: format!("Q2 {year}"),
        hidden: false,
        cells,
    }]
}

fn stats(year: u16) -> Vec<MinimalSheet> {
    let mut site_cells = vec![
        ("A1".to_string(), text("Mine Statistics (Gold)")),
        (
            "A3".to_string(),
            text("For the three months ended June 30,"),
        ),
    ];
    for (row, label) in [
        (4, "Tonnes processed (thousands)"),
        (5, "Average grade (grams per tonne)"),
        (6, "Processed"),
        (7, "Recovery rate (percent)"),
        (8, "Production (thousands of ounces)"),
        (9, "Sales (thousands of ounces)"),
        (10, "Cost of sales per ounce ($/oz)"),
        (11, "Costs per ounce"),
        (12, "Total cash costs"),
        (13, "All-in sustaining costs"),
    ] {
        site_cells.push((format!("A{row}"), text(label)));
    }
    let mut summary = vec![
        ("A1".to_string(), text("Summary of Operations")),
        (
            "A3".to_string(),
            text("For the three months ended June 30,"),
        ),
        ("B3".to_string(), number(i64::from(year))),
    ];
    for (i, header) in [
        "Gold produced (000s oz 100%)",
        "Gold produced (000s oz attributable)",
        "Gold sold (000s oz attributable)",
        "Cost of sales ($/oz)",
        "Total cash costs ($/oz)",
        "All-in sustaining costs ($/oz)",
    ]
    .iter()
    .enumerate()
    {
        summary.push((
            format!("{}4", col_index_to_letters(2 + i as u32)),
            text(header),
        ));
    }
    for (i, site) in SITES.iter().enumerate() {
        let later = year == 2031;
        let col = col_index_to_letters(2 + 2 * i as u32 + u32::from(later));
        let (produced, cost_rate, rate) = if later {
            (60, 1250, 1501)
        } else {
            (50, 1000, 1201)
        };
        let sold = if later && *site == "Zero Site" {
            0
        } else {
            produced
        };
        let share = if later && *site == "Basis Site" {
            75
        } else {
            50
        };
        for (row, value) in [
            (2, text(&format!("{site} ({share}%)"))),
            (3, number(i64::from(year))),
            (4, number(if later { 1200 } else { 1000 })),
            (6, styled("2.00", 1)),
            (
                7,
                if *site == "Heap Site" {
                    text("n/a")
                } else {
                    styled("0.8", 2)
                },
            ),
            (8, number(produced)),
            (9, number(sold)),
            (10, number(cost_rate)),
            (12, number(cost_rate)),
            (13, number(rate)),
        ] {
            site_cells.push((format!("{col}{row}"), value));
        }
        let row = 5 + i;
        summary.push((format!("A{row}"), text(&format!("{site} (50%)"))));
        for (k, value) in [2 * produced, produced, sold, cost_rate, cost_rate, rate]
            .into_iter()
            .enumerate()
        {
            summary.push((
                format!("{}{row}", col_index_to_letters(2 + k as u32)),
                number(value),
            ));
        }
    }
    vec![
        MinimalSheet {
            name: "QTD".to_string(),
            hidden: false,
            cells: summary,
        },
        MinimalSheet {
            name: "Minestats1".to_string(),
            hidden: false,
            cells: site_cells,
        },
    ]
}

fn fixture(
    mut edit: impl FnMut(&str, &mut Vec<MinimalSheet>),
) -> (IssuerProfile, Vec<LoadedSource>) {
    let mut p = synthetic::synthetic_profile();
    p.subjects = SITES.iter().map(|s| s.to_string()).collect();
    p.shares = SITES
        .iter()
        .map(|s| RegisteredShare {
            subject: s.to_string(),
            label: s.to_string(),
            num: 1,
            den: 2,
            evidence: "synthetic declared share".to_string(),
        })
        .collect();
    p.regions.clear();
    p.region_members.clear();
    p.reconstructions.clear();
    p.missing_facts.clear();
    let styles = MinimalStyles {
        num_fmts: vec![(164, "0;(0);\"—\"".to_string())],
        xfs: vec![0, 2, 9, 164],
    };
    let mut sources = Vec::new();
    for year in [2030, 2031] {
        for (kind, suffix, mut sheets) in [
            (SourceKind::AiscReconciliation, "aisc", aisc(year)),
            (SourceKind::MineStats, "stats", stats(year)),
        ] {
            let file = format!("{year}_{suffix}.xlsx");
            edit(&file, &mut sheets);
            let bytes = build_minimal_xlsx_styled(&sheets, Some(&styles));
            let workbook = Workbook::from_bytes(&file, &bytes).unwrap();
            sources.push(LoadedSource {
                file_name: file,
                kind,
                quarter: format!("{year}Q2"),
                url: "synthetic".to_string(),
                url_basis: "synthetic".to_string(),
                bytes: bytes.len() as u64,
                census: workbook.cell_census(),
                workbook,
            });
        }
    }
    (p, sources)
}

fn replace(sheets: &mut [MinimalSheet], sheet: &str, at: &str, value: MinimalCell) {
    let s = sheets.iter_mut().find(|s| s.name == sheet).unwrap();
    let c = s.cells.iter_mut().find(|(cell, _)| cell == at).unwrap();
    c.1 = value;
}

fn run(edit: impl FnMut(&str, &mut Vec<MinimalSheet>)) -> PublicAssuranceReport {
    let (profile, sources) = fixture(edit);
    assess(&profile, Lane::Synthetic, sources)
}

fn site<'a>(report: &'a WaterfallReport, name: &str) -> &'a PairOutcome {
    report.pairs.iter().find(|p| p.subject == name).unwrap()
}

fn bridge(report: &WaterfallReport, name: &str) -> Bridge {
    site(report, name)
        .outcome
        .clone()
        .unwrap_or_else(|r| panic!("{name}: {r}"))
}

fn selected(
    profile: &IssuerProfile,
    report: &PublicAssuranceReport,
    from: Period,
    to: Period,
) -> Result<WaterfallReport, fmine_evidence::Refusal> {
    pair_sites_between(
        profile,
        &report.sources,
        &report.mine_stats,
        &report.public_claims,
        &report.instances,
        &report.mine_ledger,
        (from, to),
    )
}

fn quarter(code: &str) -> Period {
    parse_period_code(code).unwrap()
}

fn selected_metric(
    profile: &IssuerProfile,
    report: &PublicAssuranceReport,
    metric: WaterfallMetric,
) -> WaterfallReport {
    pair_sites_between_for_metric(
        profile,
        &report.sources,
        &report.mine_stats,
        &report.public_claims,
        &report.instances,
        &report.mine_ledger,
        (quarter("2030Q2"), quarter("2031Q2")),
        metric,
    )
    .unwrap()
}

#[test]
fn tcc_waterfall_recovers_selected_rate_and_signed_first_block_known_answers() {
    let (profile, mut sources) = fixture(|_, _| {});
    let report = assess(&profile, Lane::Synthetic, sources.clone());
    let explicit_aisc = selected_metric(&profile, &report, WaterfallMetric::Aisc);
    assert_eq!(explicit_aisc.to_json(), report.waterfall.to_json());
    assert_eq!(explicit_aisc.digest(), report.waterfall.digest());

    let tcc = selected_metric(&profile, &report, WaterfallMetric::TotalCashCost);
    assert_eq!(tcc.metric, WaterfallMetric::TotalCashCost);
    assert_eq!(tcc.pairs.len(), 4);
    assert_eq!(site(&tcc, "Lmdi Site").kind(), "lmdi");
    assert_eq!(site(&tcc, "Heap Site").kind(), "additive_fallback");
    assert_eq!(site(&tcc, "Zero Site").kind(), "refused");
    assert_eq!(site(&tcc, "Basis Site").kind(), "refused");
    for name in ["Lmdi Site", "Heap Site"] {
        let result = bridge(&tcc, name);
        assert_eq!(result.gap().total_delta, Rat::from_integer(250));
        assert_eq!(result.gap().printed_rate_gap_0, Rat::from_integer(0));
        assert_eq!(result.gap().printed_rate_gap_1, Rat::from_integer(0));
        assert_eq!(result.gap().printed_rate_gap, Rat::from_integer(0));
        assert!((result.bars().iter().map(|b| b.point.as_f64()).sum::<f64>() - 250.0).abs() < 1e-9);
        for bar in result.bars() {
            assert!(
                bar.enclosure[0] <= bar.point.as_f64() && bar.point.as_f64() <= bar.enclosure[1]
            );
        }
    }
    let fallback = bridge(&tcc, "Heap Site");
    let mut actual: Vec<_> = fallback
        .bars()
        .iter()
        .map(|bar| match bar.point {
            BarPoint::Exact(value) => value,
            _ => panic!("exact TCC fallback"),
        })
        .collect();
    actual.sort_by(|a, b| a.checked_cmp(*b).unwrap());
    // Hand calculation: average inverse sales is 11/600; average TCC is 125/2.
    // Gross change is 50, NCI change is -25 and inverse-sales change is -1/300.
    assert_eq!(
        actual,
        vec![
            Rat::new(-1375, 3).unwrap(),
            Rat::new(-625, 3).unwrap(),
            Rat::new(2750, 3).unwrap(),
        ]
    );
    assert!(
        fallback
            .bars()
            .iter()
            .any(|bar| bar.id.contains("non-controlling interests"))
    );
    assert!(
        fallback
            .bars()
            .iter()
            .all(|bar| !bar.id.contains("total_cash_costs_aggregate"))
    );
    let inputs = &site(&tcc, "Lmdi Site").inputs;
    for cell in ["B4", "B5", "B6", "B9", "B11"] {
        assert!(
            inputs
                .iter()
                .any(|input| input.locator == format!("2030_aisc.xlsx:Q2 2030!{cell}"))
        );
    }
    assert!(inputs.iter().all(|input| {
        !input.locator.contains("_aisc.xlsx:")
            || !input
                .locator
                .rsplit_once('!')
                .and_then(|(_, cell)| fmine_io::xlsx::parse_a1_ref(cell))
                .is_some_and(|(row, _)| matches!(row, 7 | 8 | 10 | 12))
    }));
    let json = tcc.to_json();
    assert!(json.contains("\"schema\":\"fmine.waterfall.v2\""));
    assert!(json.contains("\"metric\":\"tcc\""));
    assert!(json.contains("\"rules_version\":\"m0-tcc-waterfall/1\""));
    assert!(json.contains("\"tcc_0\":"));
    assert!(json.contains("\"printed_tcc_1\":"));
    assert!(!json.contains("\"aisc_0\":"));
    assert_ne!(tcc.digest(), explicit_aisc.digest());

    sources.reverse();
    let reordered = assess(&profile, Lane::Synthetic, sources);
    let repeated = selected_metric(&profile, &reordered, WaterfallMetric::TotalCashCost);
    assert_eq!(tcc.to_json(), repeated.to_json());
    assert_eq!(tcc.digest(), repeated.digest());
}

#[test]
fn tcc_waterfall_ignores_unselected_sustaining_and_printed_rate_failures() {
    let (profile, sources) = fixture(|_, _| {});
    let baseline = assess(&profile, Lane::Synthetic, sources);
    let original = selected_metric(&profile, &baseline, WaterfallMetric::TotalCashCost);
    for (cell, value) in [
        ("C7", text("n/a")),
        ("C8", number(999)),
        ("C10", number(0)),
        ("C12", number(0)),
    ] {
        let (profile, sources) = fixture(|file, sheets| {
            if file == "2031_aisc.xlsx" {
                replace(sheets, "Q2 2031", cell, value.clone());
            }
        });
        let report = assess(&profile, Lane::Synthetic, sources);
        if matches!(cell, "C7" | "C8") {
            assert!(
                site(&report.waterfall, "Lmdi Site").outcome.is_err(),
                "{cell}"
            );
        }
        let tcc = selected_metric(&profile, &report, WaterfallMetric::TotalCashCost);
        let before = bridge(&original, "Lmdi Site");
        let after = bridge(&tcc, "Lmdi Site");
        assert_eq!(before.gap(), after.gap(), "{cell}");
        assert_eq!(before.bars().len(), after.bars().len(), "{cell}");
        for (a, b) in before.bars().iter().zip(after.bars()) {
            assert_eq!(a.id, b.id, "{cell}");
            assert_eq!(a.point, b.point, "{cell}");
            assert_eq!(a.enclosure, b.enclosure, "{cell}");
        }
        // Arithmetic does not use these cells; the receipt still binds their workbook bytes.
        assert_ne!(original.digest(), tcc.digest(), "{cell}");
    }
}

#[test]
fn tcc_waterfall_moves_with_reconciled_first_block_source_values() {
    let (profile, sources) = fixture(|_, _| {});
    let baseline = assess(&profile, Lane::Synthetic, sources);
    let original = selected_metric(&profile, &baseline, WaterfallMetric::TotalCashCost);
    let (profile, sources) = fixture(|file, sheets| {
        if file == "2031_aisc.xlsx" {
            for (cell, value) in [
                ("C4", 180),
                ("C5", -90),
                ("C6", 90),
                ("C8", 105),
                ("C10", 1500),
                ("C11", 1500),
                ("C12", 1751),
            ] {
                replace(sheets, "Q2 2031", cell, number(value));
            }
        } else if file == "2031_stats.xlsx" {
            for (sheet, cell, value) in [
                ("Minestats1", "C10", 1500),
                ("Minestats1", "C12", 1500),
                ("Minestats1", "C13", 1751),
                ("QTD", "E5", 1500),
                ("QTD", "F5", 1500),
                ("QTD", "G5", 1751),
            ] {
                replace(sheets, sheet, cell, number(value));
            }
        }
    });
    let report = assess(&profile, Lane::Synthetic, sources);
    let changed = selected_metric(&profile, &report, WaterfallMetric::TotalCashCost);
    let result = bridge(&changed, "Lmdi Site");
    // Original TCC is 1000 USD/oz; the revised source gives 1000 * 90/60 = 1500.
    assert_eq!(result.gap().total_delta, Rat::from_integer(500));
    assert_eq!(result.gap().printed_rate_gap, Rat::from_integer(0));
    assert!(
        (result
            .bars()
            .iter()
            .map(|bar| bar.point.as_f64())
            .sum::<f64>()
            - 500.0)
            .abs()
            < 1e-9
    );
    assert_ne!(original.digest(), changed.digest());
}

#[test]
fn tcc_waterfall_refuses_selected_values_and_changed_component_frontiers() {
    for (cell, value) in [
        ("C4", text("n/a")),
        ("C6", number(95)),
        ("C9", number(0)),
        ("C11", text("n/a")),
        ("A5", text("Other cash adjustment")),
    ] {
        let (profile, sources) = fixture(|file, sheets| {
            if file == "2031_aisc.xlsx" {
                replace(sheets, "Q2 2031", cell, value.clone());
            }
        });
        let report = assess(&profile, Lane::Synthetic, sources);
        let selected = selected_metric(&profile, &report, WaterfallMetric::TotalCashCost);
        let subject = if cell == "A5" {
            "Heap Site"
        } else {
            "Lmdi Site"
        };
        let outcome = site(&selected, subject);
        assert!(outcome.outcome.is_err(), "{cell}");
        assert!(!outcome.context.is_empty(), "{cell}");
    }
}

// The known quantities are unchanged; both original workbook bodies and their release metadata name
// the requested quarters. Later comparative sheets never manufacture an original release.
fn quarter_fixture(from: Period, to: Period) -> (IssuerProfile, PublicAssuranceReport) {
    let (profile, mut sources) = fixture(|file, sheets| {
        let period = if file.starts_with("2030") { from } else { to };
        let PeriodKind::Quarter(q) = period.kind else {
            panic!("quarter fixture");
        };
        if file.ends_with("aisc.xlsx") {
            sheets[0].name = format!("Q{q} {}", period.year);
        } else {
            let ends = ["March 31", "June 30", "September 30", "December 31"];
            let end = ends[usize::from(q - 1)];
            for sheet in sheets {
                for (at, cell) in &mut sheet.cells {
                    if at == "A3" {
                        *cell = text(&format!("For the three months ended {end},"));
                    } else if fmine_io::xlsx::parse_a1_ref(at).is_some_and(|(row, _)| row == 3) {
                        *cell = number(i64::from(period.year));
                    }
                }
            }
        }
    });
    for source in &mut sources {
        source.quarter = if source.file_name.starts_with("2030") {
            from.code()
        } else {
            to.code()
        };
    }
    let report = assess(&profile, Lane::Synthetic, sources);
    (profile, report)
}

#[test]
fn explicit_quarter_bridges_recover_known_effects_across_calendar_boundaries() {
    for (from, to) in [
        ("2030Q4", "2031Q1"),
        ("2030Q1", "2030Q3"),
        ("2028Q2", "2031Q2"),
    ] {
        let (from, to) = (quarter(from), quarter(to));
        let (profile, report) = quarter_fixture(from, to);
        let selected = selected(&profile, &report, from, to).unwrap();
        assert_eq!(selected.pairs.len(), 4);
        assert!(selected.pairs.iter().all(|p| p.from == from && p.to == to));
        assert_eq!(site(&selected, "Lmdi Site").kind(), "lmdi");
        assert_eq!(site(&selected, "Heap Site").kind(), "additive_fallback");
        assert_eq!(site(&selected, "Zero Site").kind(), "refused");
        assert_eq!(site(&selected, "Basis Site").kind(), "refused");
        for name in ["Lmdi Site", "Heap Site"] {
            let bridge = bridge(&selected, name);
            assert_eq!(bridge.gap().total_delta, Rat::from_integer(300));
            assert_eq!(bridge.gap().printed_rate_gap, Rat::from_integer(0));
            assert!(
                (bridge.bars().iter().map(|b| b.point.as_f64()).sum::<f64>() - 300.0).abs() < 1e-9
            );
            assert!(site(&selected, name).inputs.iter().all(|c| {
                c.key.valid_period.code() == c.key.reporting_vintage.release
                    && [from.code(), to.code()].contains(&c.key.reporting_vintage.release)
            }));
        }
        assert!(report.waterfall.pairs.is_empty());
    }
}

#[test]
fn explicit_year_on_year_selection_preserves_default_values_and_order() {
    let (profile, mut sources) = fixture(|_, _| {});
    let report = assess(&profile, Lane::Synthetic, sources.clone());
    let before = (quarter("2030Q2"), quarter("2031Q2"));
    let picked = selected(&profile, &report, before.0, before.1).unwrap();
    assert_eq!(picked.to_json(), report.waterfall.to_json());
    assert_eq!(picked.digest(), report.waterfall.digest());
    sources.reverse();
    let reordered = assess(&profile, Lane::Synthetic, sources);
    let after = selected(&profile, &reordered, before.0, before.1).unwrap();
    assert_eq!(picked.to_json(), after.to_json());
    assert_eq!(picked.digest(), after.digest());
}

#[test]
fn explicit_bridge_retains_subjects_when_an_original_endpoint_is_missing() {
    for keep in ["2030", "2031"] {
        let (profile, mut sources) = fixture(|file, sheets| {
            if file == "2031_aisc.xlsx" {
                sheets.extend(aisc(2030));
            }
        });
        sources.retain(|s| s.file_name.starts_with(keep));
        let report = assess(&profile, Lane::Synthetic, sources);
        let picked = selected(&profile, &report, quarter("2030Q2"), quarter("2031Q2")).unwrap();
        assert_eq!(picked.pairs.len(), 4);
        for p in &picked.pairs {
            assert!(
                p.outcome
                    .as_ref()
                    .unwrap_err()
                    .what
                    .contains("first_published_aisc_missing")
            );
            assert_eq!((p.from, p.to), (quarter("2030Q2"), quarter("2031Q2")));
            assert!(!p.context.is_empty());
        }
        assert!(report.waterfall.pairs.is_empty());
    }
}

#[test]
fn explicit_bridge_rejects_incompatible_periods_and_an_empty_selection() {
    let (profile, sources) = fixture(|_, _| {});
    let report = assess(&profile, Lane::Synthetic, sources);
    for (from, to) in [
        (quarter("2031Q2"), quarter("2030Q2")),
        (quarter("2030Q2"), quarter("2030Q2")),
        (quarter("2030H1"), quarter("2031Q2")),
        (quarter("2030Q2"), quarter("2031FY")),
        (
            Period {
                year: 2030,
                kind: PeriodKind::Quarter(0),
            },
            quarter("2031Q2"),
        ),
        (
            quarter("2030Q2"),
            Period {
                year: 2031,
                kind: PeriodKind::Quarter(5),
            },
        ),
    ] {
        assert_eq!(
            selected(&profile, &report, from, to).unwrap_err().code,
            "FMINE-REF-105"
        );
    }
    let empty = selected(&profile, &report, quarter("2032Q1"), quarter("2032Q2"));
    assert!(
        empty
            .unwrap_err()
            .what
            .contains("no first-published site inputs")
    );
}

#[test]
fn explicit_waterfall_cli_reads_registered_workbooks_and_preserves_citations() {
    use std::fmt::Write as _;
    use std::time::{SystemTime, UNIX_EPOCH};

    let root = std::env::temp_dir().join(format!(
        "fmine-selected-waterfall-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("corpus")).unwrap();
    std::fs::create_dir_all(root.join("registries/sources")).unwrap();
    let styles = MinimalStyles {
        num_fmts: vec![(164, "0;(0);\"—\"".into())],
        xfs: vec![0, 2, 9, 164],
    };
    let (profile, mut sources) = fixture(|file, sheets| {
        if file == "2031_stats.xlsx" {
            let mut ytd = sheets.iter().find(|s| s.name == "QTD").unwrap().clone();
            ytd.name = "YTD".to_string();
            for (at, cell) in &mut ytd.cells {
                if at == "A3" {
                    *cell = text("For the six months ended June 30,");
                } else if fmine_io::xlsx::parse_a1_ref(at)
                    .is_some_and(|(row, col)| (5..=8).contains(&row) && (2..=4).contains(&col))
                {
                    let MinimalCell::Number(value) = cell else {
                        panic!("known cumulative flow");
                    };
                    *value *= 2.0;
                }
            }
            sheets.push(ytd);
            let site_sheet = sheets.iter_mut().find(|s| s.name == "Minestats1").unwrap();
            let mut cumulative = site_sheet.cells.clone();
            cumulative.retain(|(at, _)| at != "A1");
            for (at, cell) in &mut cumulative {
                let (row, col) = fmine_io::xlsx::parse_a1_ref(at).unwrap();
                if at == "A3" {
                    *cell = text("For the six months ended June 30,");
                } else if [4, 8, 9].contains(&row)
                    && let MinimalCell::Number(value) = cell
                {
                    *value *= 2.0;
                }
                *at = format!("{}{}", col_index_to_letters(col), row + 13);
            }
            site_sheet.cells.extend(cumulative);
        }
        let bytes = build_minimal_xlsx_styled(sheets, Some(&styles));
        std::fs::write(root.join("corpus").join(file), bytes).unwrap();
    });
    // Q2's cumulative summary needs its independently published Q1 operand. A source selection
    // that drops this support produces state (v), which must still protect the selected bridge.
    let incomplete = assess(&profile, Lane::Synthetic, sources.clone());
    let missing_support =
        selected(&profile, &incomplete, quarter("2030Q2"), quarter("2031Q2")).unwrap();
    assert!(
        site(&missing_support, "Lmdi Site")
            .outcome
            .as_ref()
            .unwrap_err()
            .what
            .contains("operand_state_v")
    );
    let mut prior_sheets = stats(2031);
    for sheet in &mut prior_sheets {
        for (at, cell) in &mut sheet.cells {
            if at == "A3" {
                *cell = text("For the three months ended March 31,");
            }
        }
    }
    let prior_file = "2031_q1_stats.xlsx";
    let bytes = build_minimal_xlsx_styled(&prior_sheets, Some(&styles));
    std::fs::write(root.join("corpus").join(prior_file), &bytes).unwrap();
    let workbook = Workbook::from_bytes(prior_file, &bytes).unwrap();
    sources.push(LoadedSource {
        file_name: prior_file.to_string(),
        kind: SourceKind::MineStats,
        quarter: "2031Q1".to_string(),
        url: "synthetic".to_string(),
        url_basis: "synthetic".to_string(),
        bytes: bytes.len() as u64,
        census: workbook.cell_census(),
        workbook,
    });
    let complete = assess(&profile, Lane::Synthetic, sources.clone());
    let supported = selected(&profile, &complete, quarter("2030Q2"), quarter("2031Q2")).unwrap();
    assert_eq!(site(&supported, "Lmdi Site").kind(), "lmdi");
    let mut registry = String::from(
        "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"corpus\"\n\
         retrieved_utc = \"synthetic\"\nurl_prefix = \"https://example.invalid/\"\n",
    );
    for source in &sources {
        writeln!(
            registry,
            "\n[[source]]\nfile = \"{}\"\nkind = \"{}\"\nquarter = \"{}\"\n\
             path = \"{}\"\nurl_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{}\"",
            source.file_name,
            source.kind.as_str(),
            source.quarter,
            source.file_name,
            source.bytes,
            fmine_core::Digest32(source.workbook.sha256).to_hex()
        )
        .unwrap();
    }
    for subject in &profile.subjects {
        writeln!(registry, "\n[[subject]]\nname = \"{subject}\"").unwrap();
    }
    for share in &profile.shares {
        writeln!(
            registry,
            "\n[[share]]\nsubject = \"{}\"\nnum = {}\nden = {}\nevidence = \"{}\"",
            share.subject, share.num, share.den, share.evidence
        )
        .unwrap();
    }
    // The selected read must not require unrelated workbook bytes merely because they are registered.
    writeln!(
        registry,
        "\n[[source]]\nfile = \"unselected.xlsx\"\nkind = \"mine_stats\"\nquarter = \"2035Q1\"\n\
         path = \"unselected.xlsx\"\nurl_basis = \"W1 table\"\nbytes = 1\nsha256 = \"{}\"",
        fmine_core::hash::sha256(b"x").to_hex()
    )
    .unwrap();
    std::fs::write(root.join("registries/sources/synthetic.toml"), registry).unwrap();
    let args = [
        "metrics",
        "waterfall",
        "--issuer",
        "synthetic",
        "--from",
        "2030Q2",
        "--to",
        "2031Q2",
        "--site",
        "Lmdi Site",
    ]
    .map(str::to_string);
    let (envelope, _) = fmine_cli::invoke(&args, &root);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert!(envelope.payload_json.contains("\"lane\":\"synthetic\""));
    assert!(envelope.payload_json.contains("\"kind\":\"lmdi\""));
    assert!(envelope.payload_json.contains("\"exact\":\"300\""));
    assert!(!envelope.payload_json.contains("\"subject\":\"Heap Site\""));
    let markdown = envelope.rendered_markdown.as_ref().unwrap();
    let original_aisc = sources
        .iter()
        .find(|source| source.file_name == "2030_aisc.xlsx")
        .expect("original AISC workbook retained");
    let original_hash = fmine_core::Digest32(original_aisc.workbook.sha256).to_hex();
    let citation = format!(r"2030\_aisc.xlsx:Q2 2030!B12; SHA-256 {original_hash}.");
    assert!(markdown.contains("## Lmdi Site"));
    assert!(
        markdown
            .lines()
            .any(|line| line.starts_with("- Source Claim ") && line.ends_with(&citation))
    );
    assert_eq!(
        envelope.to_json(),
        fmine_cli::invoke(&args, &root).0.to_json()
    );
    let mut explicit_aisc = args.to_vec();
    explicit_aisc.extend(["--metric".to_string(), "aisc".to_string()]);
    assert_eq!(
        envelope.to_json(),
        fmine_cli::invoke(&explicit_aisc, &root).0.to_json()
    );
    let mut tcc_args = args.to_vec();
    tcc_args.extend(["--metric".to_string(), "tcc".to_string()]);
    let (tcc, _) = fmine_cli::invoke(&tcc_args, &root);
    assert_eq!(tcc.exit_code, 0, "{}", tcc.to_json());
    assert!(tcc.payload_json.contains("\"metric\":\"tcc\""));
    assert!(tcc.payload_json.contains("\"kind\":\"lmdi\""));
    assert!(tcc.payload_json.contains("\"exact\":\"250\""));
    assert!(
        tcc.rendered_markdown
            .as_ref()
            .unwrap()
            .contains("# Quarterly TCC cost bridge")
    );
    let tcc_citation = format!(r"2030\_aisc.xlsx:Q2 2030!B11; SHA-256 {original_hash}.");
    assert!(
        tcc.rendered_markdown
            .as_ref()
            .unwrap()
            .lines()
            .any(|line| line.starts_with("- Source Claim ") && line.ends_with(&tcc_citation))
    );
    assert_eq!(
        tcc.to_json(),
        fmine_cli::invoke(&tcc_args, &root).0.to_json()
    );
    std::fs::write(root.join("corpus/2030_aisc.xlsx"), b"altered source").unwrap();
    let (refused, _) = fmine_cli::invoke(&args, &root);
    assert_eq!(refused.exit_code, 8, "{}", refused.to_json());
    assert_eq!(refused.refusal.unwrap().code, "FMINE-REF-505");
    let (refused_tcc, _) = fmine_cli::invoke(&tcc_args, &root);
    assert_eq!(refused_tcc.exit_code, 8, "{}", refused_tcc.to_json());
    assert_eq!(refused_tcc.refusal.unwrap().code, "FMINE-REF-505");
}

#[test]
fn test_synthetic_four_seeded_outcomes_and_exact_gap() {
    let report = run(|_, _| {});
    let w = &report.waterfall;
    assert_eq!(w.pairs.len(), 4);
    assert_eq!(site(w, "Lmdi Site").kind(), "lmdi");
    assert_eq!(site(w, "Heap Site").kind(), "additive_fallback");
    assert_eq!(site(w, "Zero Site").kind(), "refused");
    assert_eq!(site(w, "Basis Site").kind(), "refused");
    for name in ["Lmdi Site", "Heap Site"] {
        let b = bridge(w, name);
        let g = b.gap();
        assert_eq!(g.total_delta, Rat::from_integer(300));
        assert_eq!(g.printed_rate_gap_0, Rat::from_integer(1));
        assert_eq!(g.printed_rate_gap_1, Rat::from_integer(1));
        assert_eq!(g.printed_rate_gap, Rat::from_integer(0));
        assert!((b.bars().iter().map(|b| b.point.as_f64()).sum::<f64>() - 300.0).abs() < 1e-9);
        for bar in b.bars() {
            assert!(
                bar.enclosure[0] <= bar.point.as_f64() && bar.point.as_f64() <= bar.enclosure[1]
            );
        }
    }
    let fallback = bridge(w, "Heap Site");
    let mut exact: Vec<_> = fallback
        .bars()
        .iter()
        .map(|b| match b.point {
            BarPoint::Exact(v) => v,
            _ => panic!("exact fallback"),
        })
        .collect();
    exact.sort_by(|a, b| a.checked_cmp(*b).unwrap());
    assert_eq!(
        exact,
        vec![
            Rat::from_integer(-250),
            Rat::new(275, 3).unwrap(),
            Rat::new(1375, 3).unwrap()
        ]
    );
}

#[test]
fn test_basis_evidence_refuses_changed_title_denominator_share_and_duplicate_release() {
    for (sheet, cell, value) in [
        ("Q2 2031", "A1", "Copper reconciliation"),
        ("Q2 2031", "A9", "Ounces sold (000s ounces)"),
        ("QTD", "A5", "Lmdi Site (75%)"),
    ] {
        let report = run(|file, sheets| {
            if file
                == if sheet == "QTD" {
                    "2031_stats.xlsx"
                } else {
                    "2031_aisc.xlsx"
                }
            {
                replace(sheets, sheet, cell, text(value));
            }
        });
        assert!(
            site(&report.waterfall, "Lmdi Site").outcome.is_err(),
            "{cell}"
        );
    }
    let (profile, mut sources) = fixture(|_, _| {});
    sources.push(
        sources
            .iter()
            .find(|s| s.file_name == "2031_stats.xlsx")
            .unwrap()
            .clone(),
    );
    let report = assess(&profile, Lane::Synthetic, sources);
    assert!(
        site(&report.waterfall, "Lmdi Site")
            .outcome
            .as_ref()
            .unwrap_err()
            .what
            .contains("exactly one")
    );
}

#[test]
fn test_pairing_uses_first_published_visible_vintage_and_same_quarter() {
    let base = run(|_, _| {});
    let comparative = run(|file, sheets| {
        if file == "2031_aisc.xlsx" {
            let mut prior = aisc(2030);
            replace(&mut prior, "Q2 2030", "B8", number(999));
            sheets.extend(prior);
        }
    });
    for name in SITES {
        assert_eq!(
            site(&base.waterfall, name).kind(),
            site(&comparative.waterfall, name).kind()
        );
    }
    assert_eq!(
        bridge(&base.waterfall, "Lmdi Site").gap(),
        bridge(&comparative.waterfall, "Lmdi Site").gap()
    );
    let hidden = run(|file, sheets| {
        if file == "2031_aisc.xlsx" {
            sheets[0].hidden = true;
        }
    });
    assert!(
        site(&hidden.waterfall, "Lmdi Site")
            .outcome
            .as_ref()
            .unwrap_err()
            .what
            .contains("first_published_aisc_missing")
    );
    let (profile, mut sources) = fixture(|_, _| {});
    for s in &mut sources {
        if s.quarter == "2031Q2" {
            s.quarter = "2031Q3".to_string();
        }
    }
    assert!(
        assess(&profile, Lane::Synthetic, sources)
            .waterfall
            .pairs
            .is_empty()
    );
}

#[test]
fn test_displayed_precision_and_explicit_percent_scaling() {
    let base = run(|_, _| {});
    let shown = run(|file, sheets| {
        if file == "2031_stats.xlsx" {
            replace(sheets, "Minestats1", "C6", styled("2.004", 1));
            replace(sheets, "Minestats1", "C7", number(80));
        }
    });
    let b0 = bridge(&base.waterfall, "Lmdi Site");
    let b1 = bridge(&shown.waterfall, "Lmdi Site");
    for (a, b) in b0.bars().iter().zip(b1.bars()) {
        assert_eq!(a.point, b.point);
        assert_eq!(a.enclosure, b.enclosure);
    }
    assert_ne!(base.waterfall.digest(), shown.waterfall.digest());
}

#[test]
fn test_component_rounding_uses_each_displayed_precision() {
    let base = run(|_, _| {});
    let (profile, sources) = fixture(|file, sheets| {
        if file.ends_with("aisc.xlsx") {
            let later = file.starts_with("2031");
            let (sheet, columns, gross, sustaining) = if later {
                ("Q2 2031", ["C", "E"], "150", "15")
            } else {
                ("Q2 2030", ["B", "D"], "100", "10")
            };
            for col in columns {
                replace(sheets, sheet, &format!("{col}4"), styled(gross, 1));
                replace(sheets, sheet, &format!("{col}7"), styled(sustaining, 1));
            }
        }
    });
    let assessed = assess(&profile, Lane::Synthetic, sources);
    // The AISC identity ledger currently declares whole displayed amounts only. Its state-v
    // admission guard must remain effective even though the waterfall adapter reads decimals.
    for name in ["Lmdi Site", "Heap Site"] {
        assert!(
            site(&assessed.waterfall, name)
                .outcome
                .as_ref()
                .unwrap_err()
                .what
                .contains("operand_state_v")
        );
    }
    // Isolate the existing source adapter's arithmetic from identity-ledger admission. This is
    // not an end-to-end assurance result; values, claims and digests come from the synthetic XLSX files.
    let decimal = pair_sites(
        &profile,
        &assessed.sources,
        &assessed.mine_stats,
        &assessed.public_claims,
        &[],
        &assessed.mine_ledger,
    );
    // Hand intervals: gross and sustaining display two decimals (h = 0.005), while the signed
    // NCI line displays integers (h = 0.5). TCC therefore has h = 0.505, and AISC has h = 0.51.
    let fallback = bridge(&decimal, "Heap Site");
    let cash = fallback
        .bars()
        .iter()
        .find(|b| b.id == "line:total_cash_costs_aggregate")
        .unwrap();
    let expected_cash = [
        1000.0 * (25.0 - 1.01) * 0.5 * (1.0 / 50.5 + 1.0 / 60.5),
        1000.0 * (25.0 + 1.01) * 0.5 * (1.0 / 49.5 + 1.0 / 59.5),
    ];
    let volume = fallback.bars().iter().find(|b| b.id == "volume").unwrap();
    let expected_volume = [
        1000.0 * (75.0 + 0.51) * (1.0 / 60.5 - 1.0 / 49.5),
        1000.0 * (75.0 - 0.51) * (1.0 / 59.5 - 1.0 / 50.5),
    ];
    for (bar, expected) in [(cash, expected_cash), (volume, expected_volume)] {
        for (bound, hand_answer) in bar.enclosure.iter().zip(expected) {
            assert!((bound - hand_answer).abs() < 1e-9, "{}: {bar:?}", bar.id);
        }
    }
    // Formatting changes the uncertainty and source identity, never the nominal bridge or gap.
    for name in ["Lmdi Site", "Heap Site"] {
        let original = bridge(&base.waterfall, name);
        let precise = bridge(&decimal, name);
        assert_eq!(original.gap(), precise.gap());
        for (a, b) in original.bars().iter().zip(precise.bars()) {
            assert_eq!(a.point, b.point);
        }
    }
    let original = bridge(&base.waterfall, "Lmdi Site");
    let precise = bridge(&decimal, "Lmdi Site");
    let a = original.bars().iter().find(|b| b.id == "cost").unwrap();
    let b = precise.bars().iter().find(|b| b.id == "cost").unwrap();
    assert!(b.enclosure[0] > a.enclosure[0] && b.enclosure[1] < a.enclosure[1]);
    assert_ne!(base.waterfall.digest(), decimal.digest());
}

#[test]
fn test_formatted_zero_keeps_numeric_radius_and_negative_lines_stay_signed() {
    let mut widths = Vec::new();
    for numeric_zero in [true, false] {
        let report = run(|file, sheets| {
            let later = file.starts_with("2031");
            let (col, tcc, rate) = if later {
                ("E", 75, 1250)
            } else {
                ("D", 50, 1000)
            };
            if file.ends_with("aisc.xlsx") {
                let sheet = if later { "Q2 2031" } else { "Q2 2030" };
                replace(
                    sheets,
                    sheet,
                    &format!("{col}5"),
                    styled(&(-tcc).to_string(), 3),
                );
                replace(
                    sheets,
                    sheet,
                    &format!("{col}7"),
                    if numeric_zero {
                        styled("0", 3)
                    } else {
                        text("—")
                    },
                );
                replace(sheets, sheet, &format!("{col}8"), number(tcc));
                replace(sheets, sheet, &format!("{col}12"), number(rate));
            } else {
                replace(sheets, "Minestats1", &format!("{col}13"), number(rate));
                replace(sheets, "QTD", "G6", number(rate));
            }
        });
        let b = bridge(&report.waterfall, "Heap Site");
        assert_eq!(b.gap().total_delta, Rat::from_integer(250));
        let line = b
            .bars()
            .iter()
            .find(|b| b.id.contains("minesite sustaining"))
            .unwrap();
        assert_eq!(line.point, BarPoint::Exact(Rat::from_integer(0)));
        widths.push(line.enclosure[1] - line.enclosure[0]);
        let negative = site(&report.waterfall, "Heap Site")
            .inputs
            .iter()
            .find(|c| c.locator == "2030_aisc.xlsx:Q2 2030!D5")
            .unwrap();
        assert!(
            matches!(&negative.claim.value, fmine_public::key::PublicValue::Number { displayed: Ok(value), .. } if value == "-50")
        );
    }
    assert!(
        widths[0] > widths[1] + 1.0,
        "numeric zero is rounded; placeholder zero is exact: {widths:?}"
    );
}

#[test]
fn test_grade_control_preserves_closure_without_identification() {
    let base = run(|_, _| {});
    let changed = run(|file, sheets| {
        if file == "2031_stats.xlsx" {
            replace(sheets, "Minestats1", "C6", styled("2.20", 1));
        }
    });
    let a = bridge(&base.waterfall, "Lmdi Site");
    let b = bridge(&changed.waterfall, "Lmdi Site");
    let effect = |bridge: &Bridge, id: &str| {
        bridge
            .bars()
            .iter()
            .find(|b| b.id == id)
            .unwrap()
            .point
            .as_f64()
    };
    let grade_shift = effect(&b, "grade") - effect(&a, "grade");
    let balance_shift = effect(&b, "balance") - effect(&a, "balance");
    let independent = -300.0 / 1.25_f64.ln() * 1.1_f64.ln();
    assert!((grade_shift - independent).abs() < 1e-9);
    assert!((grade_shift + balance_shift).abs() < 1e-9);
    assert_eq!(a.gap(), b.gap());
    assert!((b.bars().iter().map(|b| b.point.as_f64()).sum::<f64>() - 300.0).abs() < 1e-9);
    assert!(
        changed
            .waterfall
            .to_json()
            .contains("\"interpretation\":\"non_identifying\"")
    );
}

#[test]
fn test_selected_operand_in_iii_refuses_pair() {
    let report = run(|file, sheets| {
        if file == "2031_stats.xlsx" {
            replace(sheets, "QTD", "C5", number(70));
        }
    });
    assert!(
        report
            .mine_ledger
            .instances
            .iter()
            .any(|i| i.subject == "Lmdi Site" && i.state.code() == "iii")
    );
    let r = site(&report.waterfall, "Lmdi Site")
        .outcome
        .as_ref()
        .unwrap_err();
    assert!(
        r.what.contains("operand_state_iii") && r.what.contains("site_sheet"),
        "{r}"
    );
}

#[test]
fn test_copper_and_cross_map_scope_are_named_refusals() {
    let copper = run(|file, sheets| {
        if file.ends_with("stats.xlsx") {
            replace(sheets, "Minestats1", "A1", text("Mine Statistics (Copper)"));
        }
    });
    assert!(
        copper
            .waterfall
            .pairs
            .iter()
            .all(|p| p.scope == "copper_site" && p.outcome.is_err())
    );
    let (mut profile, sources) = fixture(|file, sheets| {
        if file.ends_with("stats.xlsx") {
            sheets.retain(|s| s.name == "QTD");
        }
    });
    profile.subjects.retain(|s| s != "Lmdi Site");
    profile.regions.push("Lmdi Site".to_string());
    // With no registered membership, region scopes are release-specific, so no common perimeter is inferred.
    let report = assess(&profile, Lane::Synthetic, sources);
    let r = site(&report.waterfall, "Lmdi Site")
        .outcome
        .as_ref()
        .unwrap_err();
    assert!(r.what.contains("cross_map_region"));
}

#[test]
fn test_input_citations_json_and_digest_are_complete() {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    let report = run(|_, _| {});
    for p in &report.waterfall.pairs {
        if let Ok(b) = &p.outcome {
            let cited: BTreeSet<_> = p
                .inputs
                .iter()
                .map(|c| c.locator.as_str())
                .chain(p.context.iter().map(|(at, _)| at.as_str()))
                .collect();
            for at in b
                .bars()
                .iter()
                .flat_map(|b| &b.inputs)
                .chain(&b.gap().inputs)
            {
                assert!(cited.contains(at.as_str()), "uncited: {at}");
            }
            assert!(
                p.inputs.iter().all(|c| !c.claim.anchors.is_empty()
                    && c.key.reporting_vintage.file_sha256.len() == 64)
            );
        }
    }
    let script = "import json,sys\nw=json.load(sys.stdin)\nassert w['counts']['pairs']==4\nassert w['counts']['lmdi']==1 and w['counts']['additive_fallback']==1 and w['counts']['refused']==2\nassert w['descriptive_only'] and not w['identifying']\nfor p in w['pairs']:\n if p['kind']!='refused':\n  assert p['printed_rate_gap']['exact']=='0'\n  assert p['printed_rate_gap_0']['exact']==p['printed_rate_gap_1']['exact']=='1'\n  for b in p['bars']:\n   assert set(b['in']) <= set(p['closure_residual']['in'])\n   assert b['rules_version']=='m0-waterfall/1' and b['in']\n   assert b['interval'][0]<=b['v']<=b['interval'][1]\n";
    let mut child = Command::new("python3")
        .args(["-I", "-c", script])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(report.waterfall.to_json().as_bytes())
        .unwrap();
    let parsed = child.wait_with_output().unwrap();
    assert!(
        parsed.status.success(),
        "{}",
        String::from_utf8_lossy(&parsed.stderr)
    );
    let mut changed = report.waterfall.clone();
    if let Err(r) = &mut changed
        .pairs
        .iter_mut()
        .find(|p| p.kind() == "refused")
        .unwrap()
        .outcome
    {
        r.what.push_str(" wording only");
    }
    assert_eq!(changed.digest(), report.waterfall.digest());
    let p = changed
        .pairs
        .iter_mut()
        .find(|p| p.kind() == "lmdi")
        .unwrap();
    if let Ok(Bridge::Lmdi { bars, .. }) = &mut p.outcome {
        bars[0].sign_not_determined = !bars[0].sign_not_determined;
    }
    assert_ne!(changed.digest(), report.waterfall.digest());
}

#[test]
fn test_pair_order_and_missing_claim() {
    let (profile, mut sources) = fixture(|_, _| {});
    let first = assess(&profile, Lane::Synthetic, sources.clone());
    sources.reverse();
    let reordered = assess(&profile, Lane::Synthetic, sources);
    assert_eq!(first.waterfall.to_json(), reordered.waterfall.to_json());
    assert_eq!(first.waterfall.digest(), reordered.waterfall.digest());
    let mut claims = first.public_claims.clone();
    claims.cells.remove("2030_aisc.xlsx:Q2 2030!B9");
    let missing = pair_sites(
        &profile,
        &first.sources,
        &first.mine_stats,
        &claims,
        &first.instances,
        &first.mine_ledger,
    );
    assert!(
        site(&missing, "Lmdi Site")
            .outcome
            .as_ref()
            .unwrap_err()
            .what
            .contains("unkeyed_input")
    );
}

#[test]
#[ignore = "needs registered public workbooks through 2026Q2; waterfall_census.py pins the historical counts"]
fn barrick_waterfall_counts_and_north_mara_match_independent_census() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile = load_issuer_profile(&root, "barrick")
        .and_then(|profile| profile.through("2026Q2"))
        .expect("historical waterfall census source population");
    let report = assess(
        &profile,
        Lane::Public,
        load_registered_sources(&profile, &root).unwrap(),
    );
    let w = &report.waterfall;
    if let Ok(path) = std::env::var("FMINE_WATERFALL_CAPTURE") {
        std::fs::write(path, w.to_json()).unwrap();
    }
    let gold: Vec<_> = w.pairs.iter().filter(|p| p.scope == "gold_site").collect();
    for p in &gold {
        println!(
            "{} {} {} {}",
            p.subject,
            p.from.code(),
            p.kind(),
            p.outcome.as_ref().err().map_or("", |r| &r.what)
        );
    }
    assert_eq!(gold.len(), 84);
    assert_eq!(gold.iter().filter(|p| p.kind() == "lmdi").count(), 40);
    assert_eq!(
        gold.iter()
            .filter(|p| p.kind() == "additive_fallback")
            .count(),
        4
    );
    assert_eq!(gold.iter().filter(|p| p.kind() == "refused").count(), 40);
    assert_eq!(
        w.pairs.iter().filter(|p| p.scope == "copper_site").count(),
        18
    );
    let lmdi: Vec<_> = gold
        .iter()
        .filter_map(|p| match &p.outcome {
            Ok(b @ Bridge::Lmdi { .. }) => Some(b),
            _ => None,
        })
        .collect();
    assert_eq!(lmdi.iter().map(|b| b.bars().len()).sum::<usize>(), 240);
    assert_eq!(
        lmdi.iter()
            .flat_map(|b| b.bars())
            .filter(|b| b.sign_not_determined)
            .count(),
        70
    );
    let north = gold
        .iter()
        .find(|p| p.subject == "North Mara" && p.to.code() == "2026Q2")
        .unwrap();
    let b = north.outcome.as_ref().unwrap();
    assert_eq!(b.gap().total_delta, Rat::new(45640, 37).unwrap());
    assert_eq!(b.gap().printed_rate_gap, Rat::new(1905, 37).unwrap());
    assert!(
        (b.bars().iter().map(|b| b.point.as_f64()).sum::<f64>()
            + b.gap().printed_rate_gap.to_f64()
            - 1285.0)
            .abs()
            < 1e-9
    );
    assert!(
        gold.iter()
            .filter(|p| p.kind() == "additive_fallback")
            .all(|p| p.subject == "Veladero")
    );
}
