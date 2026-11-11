use super::*;
use fmine_io::xlsx::{
    MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx_styled,
};

const CUT: &str = "2030-10-03T00:00:00Z";

#[test]
fn source_specific_retrieval_controls_the_information_cut_without_retiming_history() {
    let (registry, sources) = fixture(|_, _| {});
    let registry = registry.replace("2030-10-02T10:00:00Z/10:01:00Z", "2030-08-01T00:00:00Z");
    let inject = |time: &str| {
        registry.replace(
            "file = \"Synthetic_Q3.xlsx\"",
            &format!("file = \"Synthetic_Q3.xlsx\"\nretrieved_utc = \"{time}\""),
        )
    };
    let report = run(&inject("2030-10-02T10:00:00Z/10:01:00Z"), sources.clone()).unwrap();
    assert_eq!(report.sources[0].retrieved_utc, "2030-08-01T00:00:00Z");
    assert_eq!(report.sources[1].retrieved_utc, "2030-08-01T00:00:00Z");
    assert_eq!(
        report.sources[2].retrieved_utc,
        "2030-10-02T10:00:00Z/10:01:00Z"
    );
    assert_eq!(report.sources[2].available_by_utc, "2030-10-02T10:01:00Z");
    for bad in ["2030-09-30T23:59:59Z", "2030-10-03T00:00:01Z"] {
        assert_eq!(
            run(&inject(bad), sources.clone()).unwrap_err().code,
            "FMINE-REF-108"
        );
    }
    let earlier = run(&inject("2030-10-02T10:00:00Z"), sources).unwrap();
    assert_eq!(earlier.candidate.csv, report.candidate.csv);
    assert_ne!(
        earlier.digest, report.digest,
        "timing evidence remains receipt-bound"
    );
}
const RULES: &str = r#"
rules_id = "synthetic-rules"
record_id = "synthetic-record"
issuer = "synthetic"
alpha = "0.2"
interval_score = "synthetic interval scoring rule"
quantile_estimator = "inverse_ecdf"
rho_low = "0"
rho_high = "0.5"
coverage_level = "0.95"
report = "all declared sites"
calibration_claim_min_n = 35
calibration_claim = "uncalibrated"
nowcast_from_utc = "2030-10-01T00:00:00Z"
proof_by_utc = "2030-12-31T23:59:59Z"
outcome_vintage = "original only"
excluded_rows = "no aggregate rows"

[[target]]
id = "production"
quantity = "100% gold production"
scored_quantity = "Q4 implied production"
field = "au_prod_100_koz"
outcome = "fy_minus_9m"
[[target]]
id = "unit_cost"
quantity = "AISC per attributable ounce sold"
scored_quantity = "printed Q4 AISC"
field = "aisc_usd_per_oz"
outcome = "q4_qtd"

[[basis_rule]]
id = "ownership_change"
rule = "no implicit conversion"

[[baseline]]
id = "run_rate_persistence"
target = "production"
rule = "consecutive positive quarterly ratios"
source = "invented workbooks"
pool_from = "2030Q1"
pool_through = "2030Q2"
point_quarter = "2030Q3"
[[baseline]]
id = "run_rate_persistence"
target = "unit_cost"
rule = "consecutive positive quarterly ratios"
source = "invented workbooks"
pool_from = "2030Q1"
pool_through = "2030Q2"
point_quarter = "2030Q3"

[[site]]
name = "Site A"
status = "live"
locator = "synthetic:QTD!A3"
[[site]]
name = "Site B"
status = "live"
locator = "synthetic:QTD!A4"
[[site]]
name = "Retired Site"
status = "not_published"
locator = "synthetic:QTD!A5"
reason = "invented closed site remains in the universe"
"#;

fn sheet(q: u8) -> MinimalSheet {
    let text = |s: &str| MinimalCell::Text(s.into());
    let (production, cost) = match q {
        1 => ([50.0, 100.0], [1000.0, 2000.0]),
        2 => ([100.0, 300.0], [1200.0, 2200.0]),
        _ => ([10.0, 40.0], [1300.0, 2300.0]),
    };
    MinimalSheet {
        name: "QTD".into(),
        hidden: false,
        cells: vec![
            ("A1".into(), text("For the three months ended")),
            ("B1".into(), MinimalCell::Number(2030.0)),
            ("B2".into(), text("Gold produced (000s oz, 100%)")),
            ("C2".into(), text("All-in sustaining costs ($/oz)")),
            ("A3".into(), text("Site A (50%)")),
            ("B3".into(), MinimalCell::Number(production[0])),
            ("C3".into(), MinimalCell::Number(cost[0])),
            ("A4".into(), text("Site B (75%)")),
            ("B4".into(), MinimalCell::Number(production[1])),
            ("C4".into(), MinimalCell::Number(cost[1])),
        ],
    }
}

fn fixture(edit: impl Fn(u8, &mut MinimalSheet)) -> (String, Vec<LoadedSource>) {
    let mut registry = String::from(
        "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"invented\"\n\
         retrieved_utc = \"2030-10-02T10:00:00Z/10:01:00Z\"\nurl_prefix = \"https://example.invalid/\"\n",
    );
    let mut sources = Vec::new();
    for q in 1..=3 {
        let file_name = format!("Synthetic_Q{q}.xlsx");
        let quarter = format!("2030Q{q}");
        let mut s = sheet(q);
        edit(q, &mut s);
        let bytes = build_minimal_xlsx_styled(
            &[s],
            Some(&MinimalStyles {
                num_fmts: vec![],
                xfs: vec![0, 1],
            }),
        );
        let workbook = Workbook::from_bytes(format!("invented/{file_name}"), &bytes).unwrap();
        registry.push_str(&format!(
            "\n[[source]]\nfile = \"{file_name}\"\nkind = \"mine_stats\"\nquarter = \"{quarter}\"\n\
             path = \"{file_name}\"\nurl_basis = \"fetched and verified 2030-10-02\"\nbytes = {}\nsha256 = \"{}\"\n",
            bytes.len(),
            sha256(&bytes)
        ));
        sources.push(LoadedSource {
            file_name,
            kind: SourceKind::MineStats,
            quarter,
            url: "https://example.invalid/synthetic".into(),
            url_basis: "fetched and verified 2030-10-02".into(),
            bytes: bytes.len() as u64,
            census: workbook.cell_census(),
            workbook,
        });
    }
    registry.push_str("\n[[subject]]\nname = \"Site A\"\n[[subject]]\nname = \"Site B\"\n");
    (registry, sources)
}

fn run(registry: &str, sources: Vec<LoadedSource>) -> Result<PublicCandidate> {
    finish(
        prepare(RULES.as_bytes(), registry.as_bytes(), CUT)?,
        sources,
        2,
    )
}

fn replace_cell(sheet: &mut MinimalSheet, cell: &str, value: MinimalCell) {
    sheet
        .cells
        .iter_mut()
        .find(|(name, _)| name == cell)
        .unwrap()
        .1 = value;
}

fn copper_rules() -> String {
    let mut rules = RULES
        .replace("100% gold production", "100% copper production")
        .replace("au_prod_100_koz", "cu_prod_100_kt")
        .replace(
            "AISC per attributable ounce sold",
            "C1 per attributable pound sold",
        )
        .replace("printed Q4 AISC", "printed Q4 C1")
        .replace("aisc_usd_per_oz", "c1_usd_per_lb");
    rules.push_str(
        r#"
[[target]]
id = "copper_aisc"
quantity = "AISC per attributable copper pound sold"
scored_quantity = "printed Q4 copper AISC"
field = "aisc_usd_per_lb"
outcome = "q4_qtd"
[[baseline]]
id = "run_rate_persistence"
target = "copper_aisc"
rule = "consecutive positive quarterly ratios"
source = "invented copper workbooks"
pool_from = "2030Q1"
pool_through = "2030Q2"
point_quarter = "2030Q3"
"#,
    );
    rules
}

fn copper_sheet(q: u8, sheet: &mut MinimalSheet) {
    replace_cell(
        sheet,
        "B2",
        MinimalCell::Text("Copper produced (kt 100%)".into()),
    );
    replace_cell(
        sheet,
        "C2",
        MinimalCell::Text("C1 cash costs ($/lb)".into()),
    );
    let c1 = [2.0, 4.0, 3.0][usize::from(q - 1)];
    let aisc = [3.0, 6.0, 5.0][usize::from(q - 1)];
    for (row, scale) in [(3, 1.0), (4, 2.0)] {
        replace_cell(sheet, &format!("C{row}"), MinimalCell::Number(scale * c1));
        sheet
            .cells
            .push((format!("D{row}"), MinimalCell::Number(scale * aisc)));
    }
    sheet.cells.push((
        "D2".into(),
        MinimalCell::Text("All-in sustaining costs ($/lb)".into()),
    ));
}

#[test]
fn copper_candidates_keep_kt_and_both_per_pound_costs_in_separate_source_bound_pools() {
    let rules = copper_rules();
    let (registry, sources) = fixture(copper_sheet);
    let report = finish(
        prepare(rules.as_bytes(), registry.as_bytes(), CUT).unwrap(),
        sources.clone(),
        2,
    )
    .unwrap();
    let rows = &report.candidate.exact_rows;
    assert_eq!(rows.len(), 6);
    for (site, target, expected) in [
        ("Site A", "production", 20),
        ("Site B", "production", 80),
        ("Site A", "unit_cost", 6),
        ("Site B", "unit_cost", 12),
        ("Site A", "copper_aisc", 10),
        ("Site B", "copper_aisc", 20),
    ] {
        assert_eq!(
            rows.iter()
                .find(|r| r.site == site && r.target == target)
                .unwrap()
                .point,
            Rat::from_integer(expected)
        );
    }
    assert_eq!(
        record::parse_forecasts(report.candidate.csv.as_bytes(), &report.rules).unwrap(),
        report.candidate.serialized_rows
    );
    for row in &report.observations {
        let claim = row.claim.as_ref().unwrap();
        assert_eq!(
            claim.key.unit,
            if row.target == "production" {
                "kt"
            } else {
                "usd_per_lb"
            }
        );
        assert_eq!(
            claim.key.line.label,
            match row.target.as_str() {
                "production" => "summary:cu_prod_100_kt",
                "unit_cost" => "summary:c1_usd_per_lb",
                _ => "summary:aisc_usd_per_lb",
            }
        );
        assert_eq!(
            claim.key.ownership_basis == OwnershipBasis::Consolidated100,
            row.target == "production"
        );
    }
    let repeated = finish(
        prepare(rules.as_bytes(), registry.as_bytes(), CUT).unwrap(),
        sources,
        2,
    )
    .unwrap();
    assert_eq!(report.digest, repeated.digest);
    assert_eq!(report.candidate.csv, repeated.candidate.csv);
}

#[test]
fn copper_candidate_refuses_unit_aliases_attributable_targets_and_ownership_changes() {
    let rules = copper_rules();
    let (registry, _) = fixture(copper_sheet);
    for bad in [
        rules.replace("cu_prod_100_kt", "cu_prod_attr_kt"),
        rules.replace("c1_usd_per_lb", "c1_usd_per_oz"),
    ] {
        assert_eq!(
            prepare(bad.as_bytes(), registry.as_bytes(), CUT)
                .err()
                .unwrap()
                .code,
            "FMINE-REF-107"
        );
    }
    let (registry, changed) = fixture(|q, sheet| {
        copper_sheet(q, sheet);
        if q == 3 {
            replace_cell(sheet, "A3", MinimalCell::Text("Site A (60%)".into()));
        }
    });
    assert_eq!(
        finish(
            prepare(rules.as_bytes(), registry.as_bytes(), CUT).unwrap(),
            changed,
            2
        )
        .unwrap_err()
        .code,
        "FMINE-REF-105"
    );
    let (registry, wrong_units) = fixture(|q, sheet| {
        copper_sheet(q, sheet);
        if q == 3 {
            replace_cell(
                sheet,
                "B2",
                MinimalCell::Text("Copper produced (millions of pounds 100%)".into()),
            );
        }
    });
    assert!(
        finish(
            prepare(rules.as_bytes(), registry.as_bytes(), CUT).unwrap(),
            wrong_units,
            2
        )
        .is_err()
    );
}

#[test]
fn exact_displayed_observations_produce_complete_source_bound_candidate() {
    let (registry, sources) = fixture(|q, sheet| {
        if q == 3 {
            replace_cell(
                sheet,
                "B3",
                MinimalCell::Styled {
                    raw: "10.4".into(),
                    xf: 1,
                },
            );
        }
    });
    let report = run(&registry, sources.clone()).unwrap();
    assert!(report.synthetic);
    assert_eq!(report.sources.len(), 3);
    assert_eq!(report.observations.len(), 12);
    assert_eq!(report.candidate.not_published_sites, ["Retired Site"]);
    assert_eq!(report.candidate.exact_rows.len(), 4);
    let p = report
        .candidate
        .exact_rows
        .iter()
        .find(|r| r.site == "Site A" && r.target == "production")
        .unwrap();
    assert_eq!(
        (p.point, p.lo, p.hi),
        (
            Rat::from_integer(20),
            Rat::from_integer(20),
            Rat::from_integer(30)
        )
    );
    let c = report
        .candidate
        .exact_rows
        .iter()
        .find(|r| r.site == "Site A" && r.target == "unit_cost")
        .unwrap();
    assert_eq!(
        (c.point, c.lo, c.hi),
        (
            Rat::from_integer(1430),
            Rat::from_integer(1430),
            Rat::from_integer(1560)
        )
    );
    let input = report
        .observations
        .iter()
        .find(|o| o.site == "Site A" && o.target == "production" && o.role == "point")
        .unwrap();
    assert_eq!(input.value, Some(Rat::from_integer(10)));
    assert_eq!(
        input.claim.as_ref().unwrap().claim.anchors[0].artifact_sha256,
        sources[2].workbook.sha256
    );
    assert_eq!(
        input.claim.as_ref().unwrap().locator,
        "Synthetic_Q3.xlsx:QTD!B3"
    );
    assert!(
        matches!(&input.claim.as_ref().unwrap().claim.value, PublicValue::Number { stored, displayed: Ok(displayed), .. } if stored == "10.4" && displayed == "10")
    );
    assert!(
        report
            .candidate
            .csv
            .contains("Site A,production,20.00,20.00,30.00,true")
    );
    assert_eq!(report.digest, run(&registry, sources).unwrap().digest);
}

#[test]
fn missing_rows_placeholders_and_nonpositive_history_are_audited_without_imputation() {
    let (registry, sources) = fixture(|q, sheet| {
        if q == 1 {
            replace_cell(sheet, "B3", MinimalCell::Text("—".into()));
            replace_cell(sheet, "C3", MinimalCell::Number(0.0));
        }
    });
    let report = run(&registry, sources).unwrap();
    for pool in &report.candidate.pools {
        assert_eq!(pool.ratios.len(), 1);
        assert_eq!(pool.excluded_pairs.len(), 1);
        assert_eq!(pool.excluded_pairs[0].site, "Site A");
    }
    let missing = report
        .observations
        .iter()
        .find(|o| o.site == "Site A" && o.target == "production" && o.quarter.q == 1)
        .unwrap();
    assert!(missing.value.is_none() && missing.claim.is_some());
    assert!(missing.reason.as_deref().unwrap().contains("nil"));
    let zero = report
        .observations
        .iter()
        .find(|o| o.site == "Site A" && o.target == "unit_cost" && o.quarter.q == 1)
        .unwrap();
    assert_eq!(zero.value, Some(Rat::from_integer(0)));
    assert_eq!(
        zero.reason.as_deref(),
        Some("non_positive_printed_number_excluded_by_model")
    );

    let (registry, sources) = fixture(|q, sheet| {
        if q == 1 {
            sheet
                .cells
                .retain(|(cell, _)| !["A3", "B3", "C3"].contains(&cell.as_str()));
            for (cell, _) in &mut sheet.cells {
                if cell.ends_with('4') {
                    *cell = cell.replace('4', "3");
                }
            }
        }
    });
    let report = run(&registry, sources).unwrap();
    let absent = report
        .observations
        .iter()
        .find(|o| o.site == "Site A" && o.quarter.q == 1)
        .unwrap();
    assert!(absent.claim.is_none());
    assert_eq!(
        absent.reason.as_deref(),
        Some("original_release_has_no_matching_row")
    );
    assert_eq!(absent.source, "Synthetic_Q1.xlsx");
    assert_eq!(absent.sha256.len(), 64);
}

#[test]
fn missing_original_and_ambiguous_registration_never_use_comparatives() {
    let (registry, _) = fixture(|_, _| {});
    let no_q1 = registry.replacen("quarter = \"2030Q1\"", "quarter = \"2029Q1\"", 1);
    let error = prepare(RULES.as_bytes(), no_q1.as_bytes(), CUT)
        .err()
        .unwrap();
    assert_eq!(error.code, RefusalCode::NoClaimInDomain201.as_str());
    assert!(error.what.contains("2030Q1"));
    let duplicate = registry.replace("quarter = \"2030Q2\"", "quarter = \"2030Q1\"");
    assert!(prepare(RULES.as_bytes(), duplicate.as_bytes(), CUT).is_err());
}

#[test]
fn availability_is_conservative_and_completed_quarters_cannot_look_ahead() {
    let (registry, _) = fixture(|_, _| {});
    assert!(
        prepare(
            RULES.as_bytes(),
            registry.as_bytes(),
            "2030-10-02T10:00:30Z"
        )
        .is_err()
    );
    let old = registry.replace("2030-10-02T10:00:00Z/10:01:00Z", "2030-09-30T23:59:59Z");
    assert!(prepare(RULES.as_bytes(), old.as_bytes(), CUT).is_err());
    let publication = registry.replace("\n[[subject]]\nname = \"Site A\"", "\npublished_at = \"2030-10-03\"\npublished_at_basis = \"FACT: synthetic date-only release\"\n[[subject]]\nname = \"Site A\"");
    assert!(
        prepare(
            RULES.as_bytes(),
            publication.as_bytes(),
            "2030-10-03T12:00:00Z"
        )
        .is_err()
    );
    let accepted = prepare(
        RULES.as_bytes(),
        publication.as_bytes(),
        "2030-10-04T00:00:00Z",
    )
    .unwrap();
    assert_eq!(accepted.sources[2].available_by_utc, "2030-10-03T23:59:59Z");
}

#[test]
fn original_historical_production_survives_source_proven_per_ounce_withholding() {
    let note = "1 As a result of temporary suspension of operations at Site A starting January 14, 2030, no per ounce data is provided";
    let (registry, sources) = fixture(|q, sheet| {
        if q == 1 {
            replace_cell(sheet, "B3", MinimalCell::Number(22.0));
            replace_cell(sheet, "C3", MinimalCell::Text("—".into()));
            sheet
                .cells
                .push(("A8".into(), MinimalCell::Text(note.into())));
        }
    });
    let mut prepared = prepare(RULES.as_bytes(), registry.as_bytes(), CUT).unwrap();
    prepared.profile.subject_statuses.push(key::SubjectStatus {
        subject: "Site A".into(),
        kind: key::StatusKind::Withheld,
        first: (2030, 1),
        last: Some((2030, 3)),
        evidence: "synthetic withholding".into(),
    });
    let report = finish(prepared, sources.clone(), 2).unwrap();
    let row = report
        .observations
        .iter()
        .find(|o| o.site == "Site A" && o.target == "production" && o.quarter.q == 1)
        .unwrap();
    assert_eq!(row.value, Some(Rat::from_integer(22)));
    assert_eq!(
        row.reason.as_deref(),
        Some("included_production_with_per_ounce_only_note:Synthetic_Q1.xlsx:QTD!A8")
    );
    let claim = row.claim.as_ref().unwrap();
    assert!(
        matches!(&claim.claim.value, PublicValue::Number { stored, displayed: Ok(displayed), under_status: Some(key::StatusKind::Withheld) }
        if stored == "22" && displayed == "22")
    );
    assert_eq!(
        claim.status_evidence.as_deref(),
        Some("synthetic withholding")
    );
    assert_eq!(
        claim.claim.anchors[0].artifact_sha256,
        sources[0].workbook.sha256
    );
    let production = report
        .candidate
        .exact_rows
        .iter()
        .find(|r| r.site == "Site A" && r.target == "production")
        .unwrap();
    assert_eq!(
        (production.lo, production.point, production.hi),
        (
            Rat::from_integer(30),
            Rat::from_integer(30),
            Rat::new(500, 11).unwrap()
        )
    );
    let cost = report
        .observations
        .iter()
        .find(|o| o.site == "Site A" && o.target == "unit_cost" && o.quarter.q == 1)
        .unwrap();
    assert!(cost.value.is_none());
    assert!(matches!(
        cost.claim.as_ref().unwrap().claim.value,
        PublicValue::Nil { .. }
    ));
}

#[test]
fn withholding_exception_never_supplies_missing_notes_costs_points_or_disposal_support() {
    for fault in [
        "missing",
        "subject",
        "broader",
        "middle_clause",
        "month",
        "date",
        "signed_day",
        "year",
        "duplicate",
        "cost",
        "point",
        "disposed",
        "care",
        "conflicting_status",
    ] {
        let quarter = if fault == "point" { 3 } else { 1 };
        let (registry, sources) = fixture(|q, sheet| {
            if q != quarter {
                return;
            }
            replace_cell(
                sheet,
                "C3",
                if fault == "cost" {
                    MinimalCell::Number(1000.0)
                } else {
                    MinimalCell::Text("—".into())
                },
            );
            let mut note = "1 As a result of temporary suspension of operations at Site A starting January 14, 2030, no per ounce data is provided".to_string();
            match fault {
                "subject" => note = note.replace("Site A", "Site B"),
                "broader" => note = note.replace("no per ounce", "no operating or per ounce"),
                "middle_clause" => {
                    note = note.replace("January 14", "January 14; no production is provided");
                }
                "month" => note = note.replace("January", "Notamonth"),
                "date" => note = note.replace("January 14", "February 30"),
                "signed_day" => note = note.replace("January 14", "January +14"),
                "year" => note = note.replace("2030", "2031"),
                _ => {}
            }
            if fault != "missing" {
                sheet
                    .cells
                    .push(("A8".into(), MinimalCell::Text(note.clone())));
            }
            if fault == "duplicate" {
                sheet.cells.push(("A9".into(), MinimalCell::Text(note)));
            }
        });
        let mut prepared = prepare(RULES.as_bytes(), registry.as_bytes(), CUT).unwrap();
        let status = key::SubjectStatus {
            subject: "Site A".into(),
            kind: match fault {
                "disposed" => key::StatusKind::Disposed,
                "care" => key::StatusKind::CareAndMaintenance,
                _ => key::StatusKind::Withheld,
            },
            first: (2030, 3 * quarter - 2),
            last: Some((2030, 3 * quarter)),
            evidence: "synthetic withholding".into(),
        };
        prepared.profile.subject_statuses.push(status.clone());
        if fault == "conflicting_status" {
            prepared.profile.subject_statuses.push(key::SubjectStatus {
                kind: key::StatusKind::CareAndMaintenance,
                ..status
            });
        }
        let error = finish(prepared, sources, 2).unwrap_err();
        assert_eq!(
            error.code,
            RefusalCode::BasisMismatch105.as_str(),
            "{fault}"
        );
        assert!(error.what.contains("status"), "{fault}: {error:?}");
    }
    for missing in [MinimalCell::Number(0.0), MinimalCell::Text("—".into())] {
        let (registry, sources) = fixture(|q, sheet| {
            if q == 1 {
                replace_cell(sheet, "B3", missing.clone());
                replace_cell(sheet, "C3", missing.clone());
            }
        });
        let mut prepared = prepare(RULES.as_bytes(), registry.as_bytes(), CUT).unwrap();
        prepared.profile.subject_statuses.push(key::SubjectStatus {
            subject: "Site A".into(),
            kind: key::StatusKind::Withheld,
            first: (2030, 1),
            last: Some((2030, 3)),
            evidence: "synthetic withholding".into(),
        });
        let report = finish(prepared, sources, 2).unwrap();
        assert!(
            report
                .candidate
                .pools
                .iter()
                .all(|p| p.ratios.len() == 1 && p.excluded_pairs.len() == 1)
        );
        for row in report
            .observations
            .iter()
            .filter(|r| r.site == "Site A" && r.quarter.q == 1)
        {
            assert!(matches!(
                row.claim.as_ref().unwrap().claim.value,
                PublicValue::Nil { .. }
            ));
            assert!(!row.reason.as_deref().unwrap_or("").starts_with("included_"));
        }
    }
}

#[test]
#[ignore = "needs the registered original Barrick Mine Stats corpus"]
fn real_q3_candidate_preserves_partial_production_and_matches_independent_fraction_pools() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rules = std::fs::read(root.join("forecasts/fy2026/rules.toml")).unwrap();
    assert_eq!(
        sha256(&rules).to_hex(),
        "edfba6801621dd9091ddf3c392aa972a4254713bff2abc62863d4fd5331fee76"
    );
    let report = build(&root, &rules, "2026-11-10T06:15:38Z", 6).unwrap();
    assert_eq!(report.candidate.exact_rows.len(), 22);
    assert_eq!(report.candidate.not_published_sites, ["Hemlo", "Tongon"]);
    for (target, count, excluded, ratios) in [
        ("production", 96, 3, [(25, 32), (113, 112), (53, 39)]),
        ("unit_cost", 94, 5, [(1407, 1774), (1732, 1643), (995, 773)]),
    ] {
        let pool = report
            .candidate
            .pools
            .iter()
            .find(|p| p.target == target)
            .unwrap();
        assert_eq!(
            (pool.ratios.len(), pool.excluded_pairs.len()),
            (count, excluded)
        );
        assert_eq!(
            [pool.lower_ratio, pool.median_ratio, pool.upper_ratio],
            ratios.map(|(n, d)| Rat::new(n, d).unwrap())
        );
    }
    let original = report
        .observations
        .iter()
        .find(|o| {
            o.site == "Loulo-Gounkoto"
                && o.target == "production"
                && o.quarter.to_string() == "2025Q1"
        })
        .unwrap();
    assert_eq!(original.value, Some(Rat::from_integer(22)));
    assert_eq!(
        original.sha256,
        "ec5158f8b4624643ee83d0099e4f3354dc550729ebb443a2dbfacfc29315998b"
    );
    assert_eq!(
        original.reason.as_deref(),
        Some(
            "included_production_with_per_ounce_only_note:Barrick_Q1_2025_Mine_Stats.xlsx:Minestats3!A35"
        )
    );
    assert!(matches!(
        original.claim.as_ref().unwrap().claim.value,
        PublicValue::Number {
            under_status: Some(key::StatusKind::Withheld),
            ..
        }
    ));
}

#[test]
fn incompatible_targets_ownership_changes_and_status_conflicts_refuse() {
    let (registry, sources) = fixture(|q, sheet| {
        if q == 2 {
            replace_cell(sheet, "A3", MinimalCell::Text("Site A (60%)".into()));
        }
    });
    assert!(
        run(&registry, sources)
            .unwrap_err()
            .what
            .contains("changing ownership")
    );
    let (registry, sources) = fixture(|_, _| {});
    let unsupported = RULES.replace(
        "field = \"au_prod_100_koz\"",
        "field = \"au_prod_attr_koz\"",
    );
    assert!(prepare(unsupported.as_bytes(), registry.as_bytes(), CUT).is_err());
    let wrong_quarter = RULES.replace("point_quarter = \"2030Q3\"", "point_quarter = \"2030Q4\"");
    assert!(prepare(wrong_quarter.as_bytes(), registry.as_bytes(), CUT).is_err());
    let mut prepared = prepare(RULES.as_bytes(), registry.as_bytes(), CUT).unwrap();
    prepared.profile.subject_statuses.push(key::SubjectStatus {
        subject: "Site A".into(),
        kind: key::StatusKind::Withheld,
        first: (2030, 1),
        last: Some((2030, 3)),
        evidence: "synthetic withholding".into(),
    });
    assert!(
        finish(prepared, sources, 2)
            .unwrap_err()
            .what
            .contains("status")
    );
}

#[test]
fn point_missing_or_nonpositive_and_corrupt_source_bindings_refuse() {
    for bad in [MinimalCell::Number(0.0), MinimalCell::Text("—".into())] {
        let (registry, sources) = fixture(|q, sheet| {
            if q == 3 {
                replace_cell(sheet, "B3", bad.clone());
            }
        });
        assert!(run(&registry, sources).is_err());
    }
    let (registry, mut sources) = fixture(|_, _| {});
    sources[0].workbook.sha256[0] ^= 1;
    assert!(run(&registry, sources).unwrap_err().what.contains("digest"));
}

#[test]
fn disposal_in_final_month_refuses_unflagged_partial_quarter_point() {
    let (registry, sources) = fixture(|_, _| {});
    let mut prepared = prepare(RULES.as_bytes(), registry.as_bytes(), CUT).unwrap();
    let disposal = key::SubjectStatus::from_date(
        "Site A",
        key::StatusKind::Disposed,
        (2030, 9, 15),
        "synthetic September disposal",
    );
    assert_eq!(disposal.first, (2030, 10));
    prepared.profile.subject_statuses.push(disposal);
    let error = finish(prepared, sources.clone(), 2).unwrap_err();
    assert_eq!(error.code, RefusalCode::BasisMismatch105.as_str());
    assert!(error.what.contains("Synthetic_Q3.xlsx:QTD!B3") && error.what.contains("status"));

    let mut prepared = prepare(RULES.as_bytes(), registry.as_bytes(), CUT).unwrap();
    prepared
        .profile
        .subject_statuses
        .push(key::SubjectStatus::from_date(
            "Site A",
            key::StatusKind::Disposed,
            (2030, 10, 15),
            "synthetic later disposal",
        ));
    assert!(finish(prepared, sources, 2).is_ok());
}

#[test]
fn receipt_binds_rules_registry_information_cut_and_source_values() {
    let (registry, sources) = fixture(|_, _| {});
    let base = run(&registry, sources.clone()).unwrap();
    let registry_edit = format!("{registry}\n# independent source registry version\n");
    let changed = run(&registry_edit, sources.clone()).unwrap();
    assert_eq!(base.candidate.csv, changed.candidate.csv);
    assert_ne!(base.digest, changed.digest);
    let rules_edit = format!("{RULES}\n# explicitly selected rules bytes\n");
    let changed = finish(
        prepare(rules_edit.as_bytes(), registry.as_bytes(), CUT).unwrap(),
        sources.clone(),
        2,
    )
    .unwrap();
    assert_ne!(base.digest, changed.digest);
    let changed = finish(
        prepare(
            RULES.as_bytes(),
            registry.as_bytes(),
            "2030-10-04T00:00:00Z",
        )
        .unwrap(),
        sources,
        2,
    )
    .unwrap();
    assert_ne!(base.digest, changed.digest);
    let (registry, sources) = fixture(|q, sheet| {
        if q == 3 {
            replace_cell(sheet, "B3", MinimalCell::Number(11.0));
        }
    });
    let changed = run(&registry, sources).unwrap();
    assert_ne!(base.candidate.csv, changed.candidate.csv);
    assert_ne!(base.digest, changed.digest);
}
