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
use fmine_public::sources::{IssuerProfile, RegisteredShare, SourceKind, load_issuer_profile};
use fmine_public::waterfall::{BarPoint, Bridge, PairOutcome, WaterfallReport, pair_sites};
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
#[ignore = "needs all registered public workbooks; independent source-only waterfall_census.py pins the counts"]
fn barrick_waterfall_counts_and_north_mara_match_independent_census() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile = load_issuer_profile(&root, "barrick").unwrap();
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
