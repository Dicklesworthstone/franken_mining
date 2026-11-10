//! BD-273: exact arithmetic and source-bound selections over invented AISC workbooks.
#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_cli::packet;
use fmine_core::{anchor::SourceAnchor, sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
use fmine_io::xlsx::{MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx};
use fmine_metrics::consolidation::{self, Column, Printed, Table};
use fmine_public::sources::{RegionMemberRow, SourceKind, SubtotalMemberRow};
use fmine_public::{Lane, LoadedSource, PublicAssuranceReport, assess, synthetic};
use fmine_units::ExactQty as Rat;

#[path = "../../fmine-cli/src/assure_report.rs"]
mod assure_report;

fn n(x: i128) -> Rat {
    Rat::from_integer(x)
}
fn p(x: i128) -> Printed {
    Printed {
        point: n(x),
        step: n(1),
    }
}

fn table() -> Table {
    let column = |subject: &str, c: [i128; 5], total: i128, sold: i128| Column {
        subject: subject.into(),
        components: c.map(p).into(),
        numerator_usd_m: p(total),
        sold_koz: p(sold),
    };
    Table {
        issuer: "invented".into(),
        release: "2030Q1".into(),
        period: "2030Q1".into(),
        basis: "same printed USD million rows; attributable thousand ounces sold; no conversion"
            .into(),
        membership: ["Site A", "Site B"]
            .map(|s| (s.into(), format!("Declared subtotal contains {s}")))
            .into(),
        component_labels: [
            "cost of sales",
            "depreciation",
            "NCI",
            "G&A",
            "sustaining capital",
        ]
        .map(str::to_string)
        .into(),
        aggregate: column("Declared subtotal", [1000, -200, -100, 30, 120], 850, 100),
        members: vec![
            column("Site A", [400, -80, -40, 0, 50], 330, 40),
            column("Site B", [600, -120, -60, 0, 70], 490, 60),
        ],
    }
}

fn claim(table: Table) -> Claim<Table> {
    claim_state(table, KnowledgeState::Known)
}

fn claim_state(table: Table, state: KnowledgeState) -> Claim<Table> {
    let tag = table.canonical_tag();
    Claim::new(
        consolidation::INPUT_METRIC,
        table,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        state,
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            "invented.xlsx",
            sha256(b"invented table").0,
            "Q1 2030!B5:D13",
        )],
    )
}

#[test]
fn exact_signed_component_differences_and_rounding_remainder_have_known_answers() {
    let input = claim(table());
    let result = consolidation::compare(&input).unwrap();
    let c = &result.value;
    assert_eq!(
        c.components_usd_m
            .iter()
            .map(|d| d.point)
            .collect::<Vec<_>>(),
        vec![n(0), n(0), n(0), n(30), n(0)]
    );
    assert_eq!(c.numerator_usd_m.point, n(30));
    assert_eq!(c.numerator_usd_m.lo, Rat::new(57, 2).unwrap());
    assert_eq!(c.numerator_usd_m.hi, Rat::new(63, 2).unwrap());
    assert_eq!(c.summed_components_usd_m.point, n(30));
    assert_eq!(c.summed_components_usd_m.lo, Rat::new(45, 2).unwrap());
    assert_eq!(c.summed_components_usd_m.hi, Rat::new(75, 2).unwrap());
    assert_eq!(
        (
            c.unexplained_usd_m.point,
            c.unexplained_usd_m.lo,
            c.unexplained_usd_m.hi
        ),
        (n(0), n(-9), n(9))
    );
    assert!(c.printed_arithmetic_closed);
    assert!(c.column_footings_usd_m.iter().all(|(_, d)| d.point == n(0)));
    assert_eq!(result.evidence, EvidenceClass::Assumed);
    assert_eq!(result.parents, vec![input.claim_id]);
    let mut reordered = table();
    reordered.members.reverse();
    assert_eq!(consolidation::compare(&claim(reordered)).unwrap(), result);
}

#[test]
fn numerator_and_sales_discrepancies_remain_unresolved_without_a_plug() {
    let mut t = table();
    t.aggregate.numerator_usd_m.point = n(851);
    t.aggregate.sold_koz.point = n(102);
    let result = consolidation::compare(&claim(t)).unwrap();
    assert!(!result.value.printed_arithmetic_closed);
    assert_eq!(result.value.unexplained_usd_m.point, n(1));
    assert_eq!(result.value.sold_koz.point, n(2));
    assert!(!result.value.sold_koz.contains_zero());
    assert_eq!(result.value.components_usd_m[3].point, n(30));
}

#[test]
fn malformed_incomplete_stale_or_mutated_claims_never_produce_comparisons() {
    let mut mutated = claim(table());
    mutated.value.members[0].components[0].point = n(999);
    assert_eq!(
        consolidation::compare(&mutated).unwrap_err().code,
        "FMINE-REF-507"
    );
    let mut stale = claim(table());
    stale.stale = true;
    assert!(consolidation::compare(&stale).is_err());
    for change in 0..4 {
        let mut t = table();
        match change {
            0 => {
                t.members[0].components.pop();
            }
            1 => t.members[0].subject = t.aggregate.subject.clone(),
            2 => t.members[0].sold_koz.point = n(-1),
            _ => t.members[0].components[0].step = n(-1),
        }
        assert!(
            consolidation::compare(&claim(t)).is_err(),
            "change {change}"
        );
    }
    let mut t = table();
    t.members[0].components[0].point = n(i128::MAX);
    assert_eq!(
        consolidation::compare(&claim(t)).unwrap_err().code,
        "FMINE-REF-207"
    );
}

#[test]
fn knowledge_admission_and_complete_member_and_component_keys_are_enforced() {
    for state in [
        KnowledgeState::Unknown,
        KnowledgeState::Conflicted,
        KnowledgeState::NotObservable,
        KnowledgeState::Redacted,
        KnowledgeState::Indeterminate,
        KnowledgeState::NotApplicable,
    ] {
        let input = claim_state(table(), state);
        input.check_id().unwrap();
        assert!(consolidation::compare(&input).is_err(), "{state:?}");
    }
    let mut synthetic = table();
    synthetic.issuer = "synthetic".into();
    assert_eq!(
        consolidation::compare(&claim_state(synthetic, KnowledgeState::Unknown))
            .unwrap()
            .state,
        KnowledgeState::Unknown
    );
    for change in 0..4 {
        let mut t = table();
        match change {
            0 => {
                t.membership.remove("Site A");
            }
            1 => {
                t.membership
                    .insert("Absent member".into(), "explicit extra member".into());
            }
            2 => t.component_labels[1] = t.component_labels[0].clone(),
            _ => t.component_labels[1].clear(),
        }
        assert!(
            consolidation::compare(&claim(t)).is_err(),
            "change {change}"
        );
    }
}

fn source(changes: &[(&str, MinimalCell)], units: &str) -> LoadedSource {
    source_omitting_labels(changes, units, &[])
}

fn source_omitting_labels(
    changes: &[(&str, MinimalCell)],
    units: &str,
    omit: &[&str],
) -> LoadedSource {
    let mut cells: Vec<_> = [
        (
            "A1",
            "Reconciliation of gold cost of sales to total cash costs and all-in sustaining costs",
        ),
        ("A3", units),
        ("B2", "Declared subtotal"),
        ("C2", "Site A"),
        ("D2", "Site B"),
        ("A5", "Cost of sales applicable to gold production"),
        ("A6", "Depreciation"),
        ("A7", "Non-controlling interests"),
        ("A8", "Total cash costs"),
        ("A9", "General & administrative costs"),
        ("A10", "Minesite sustaining capital expenditures"),
        ("A11", "All-in sustaining costs"),
        ("A13", "Ounces sold - attributable basis (000s ounces)"),
        ("A14", "Cost of sales per ounce"),
        ("A15", "Total cash costs per ounce"),
        ("A16", "All-in sustaining costs per ounce"),
    ]
    .into_iter()
    .map(|(at, text)| (at.into(), MinimalCell::Text(text.into())))
    .collect();
    for (col, values) in [
        (
            "B",
            [1000, -200, -100, 700, 30, 120, 850, 100, 10000, 7000, 8500],
        ),
        ("C", [400, -80, -40, 280, 0, 50, 330, 40, 10000, 7000, 8250]),
        (
            "D",
            [600, -120, -60, 420, 0, 70, 490, 60, 10000, 7000, 8167],
        ),
    ] {
        for (r, value) in [5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 16]
            .into_iter()
            .zip(values)
        {
            cells.push((format!("{col}{r}"), MinimalCell::Number(f64::from(value))));
        }
    }
    for (at, value) in changes {
        cells.iter_mut().find(|(c, _)| c == at).unwrap().1 = value.clone();
    }
    cells.retain(|(at, _)| !omit.contains(&at.as_str()));
    let bytes = build_minimal_xlsx(&[MinimalSheet {
        name: "Q1 2030".into(),
        hidden: false,
        cells,
    }]);
    let workbook = Workbook::from_bytes("invented_aisc.xlsx", &bytes).unwrap();
    LoadedSource {
        file_name: "invented_aisc.xlsx".into(),
        kind: SourceKind::AiscReconciliation,
        quarter: "2030Q1".into(),
        url: "synthetic:invented".into(),
        url_basis: "synthetic test only".into(),
        bytes: bytes.len() as u64,
        census: workbook.cell_census(),
        workbook,
    }
}

fn report(changes: &[(&str, MinimalCell)], units: &str, partial: bool) -> PublicAssuranceReport {
    report_source(source(changes, units), partial)
}

fn report_source(source: LoadedSource, partial: bool) -> PublicAssuranceReport {
    let mut p = synthetic::synthetic_profile();
    p.subjects = ["Declared subtotal", "Site A", "Site B"]
        .map(str::to_string)
        .into();
    p.regions.clear();
    p.reconstructions.clear();
    p.missing_facts.clear();
    p.shares.clear();
    p.subtotal_members = ["Site A", "Site B"]
        .map(|name| SubtotalMemberRow {
            subtotal: "Declared subtotal".into(),
            member: name.into(),
            period_from: "2030Q1".into(),
            period_to: None,
            evidence: "synthetic declared membership".into(),
        })
        .into();
    if partial {
        p.subtotal_members[0].period_from = "2030Q2".into();
    }
    assess(&p, Lane::Synthetic, vec![source])
}

const UNITS: &str = "($ millions, except per ounce information)";

#[test]
fn assurance_projection_preserves_source_claims_and_emits_named_differences() {
    let r = report(&[], UNITS, false);
    let json = assure_report::consolidation_json(&r);
    assert!(json.contains(&format!(
        "\"interpretation\":\"{}\"",
        consolidation::NO_CLAIM
    )));
    assert!(
        !json.contains("\"no_claim\":"),
        "no_claim belongs to the outer envelope"
    );
    assert!(
        json.contains("\"status\":\"printed_arithmetic_closed\""),
        "{json}"
    );
    assert!(json.contains("\"source_line\":\"A9: General & administrative costs\",\"difference_usd_m\":{\"point\":\"30\""));
    assert!(json.contains("\"unexplained_usd_m\":{\"point\":\"0\",\"interval\":[\"-9\",\"9\"]"));
    assert!(json.contains("\"accounting_reconciliation_verified\":false"));
    assert!(json.contains("\"evidence\":\"Assumed\""));
    for cell in [
        "B5", "C5", "D5", "B9", "C9", "D9", "B11", "C11", "D11", "B13", "C13", "D13",
    ] {
        let at = format!("invented_aisc.xlsx:Q1 2030!{cell}");
        let source = r.public_claims.cells.get(&at).unwrap();
        assert!(
            json.contains(&source.claim.claim_id.to_string()),
            "missing {cell}"
        );
        assert!(json.contains(&source.key.reporting_vintage.file_sha256));
    }
    assert_eq!(json, assure_report::consolidation_json(&r));
    let markdown = assure_report::markdown(&r).unwrap();
    assert!(markdown.contains("AISC component comparisons"));
    assert!(markdown.contains("Unexplained numerator remainder"));
    assert!(markdown.contains("Printed arithmetic closes: true"));
}

#[test]
fn source_mutation_changes_the_report_and_never_hides_an_unexplained_amount() {
    let before = assure_report::consolidation_json(&report(&[], UNITS, false));
    let after = assure_report::consolidation_json(&report(
        &[("B11", MinimalCell::Number(851.0))],
        UNITS,
        false,
    ));
    assert_ne!(before, after);
    assert!(after.contains("\"status\":\"unresolved\""));
    assert!(after.contains("\"unexplained_usd_m\":{\"point\":\"1\""));
    assert!(after.contains("\"source_line\":\"A9: General & administrative costs\",\"difference_usd_m\":{\"point\":\"30\""));
}

#[test]
fn source_bound_per_ounce_gap_keeps_named_components_and_recomputed_rates() {
    let r = report(&[], UNITS, false);
    let json = assure_report::consolidation_json(&r);
    assert!(
        json.contains(&format!(
            "\"per_ounce\":{{\"rules_version\":\"{}\",\"status\":\"computed\"",
            consolidation::RATE_RULES_VERSION
        )),
        "{json}"
    );
    // Independent arithmetic: 1000*850/100 = 8500, 1000*(330+490)/(40+60) = 8200.
    // Only the G&A difference is nonzero: 1000*30/100 = 300 USD/oz.
    for fragment in [
        "\"aggregate_usd_per_oz\":{\"point\":\"8500\"",
        "\"members_usd_per_oz\":{\"point\":\"8200\"",
        "\"gap_usd_per_oz\":{\"point\":\"300\"",
        "\"source_line\":\"A9: General & administrative costs\",\"difference_usd_per_oz\":{\"point\":\"300\"",
        "\"unexplained_usd_per_oz\":{\"point\":\"0\"",
        "\"denominator_usd_per_oz\":{\"point\":\"0\"",
        "\"point_closure_verified\":true",
        "\"accounting_reconciliation_verified\":false",
    ] {
        assert!(json.contains(fragment), "missing {fragment}: {json}");
    }
    let markdown = assure_report::markdown(&r).unwrap();
    assert!(markdown.contains("Recomputed per-ounce comparison"));
    assert!(markdown.contains("| Recomputed aggregate rate | 8500 |"));
    assert!(markdown.contains("| Recomputed combined member rate | 8200 |"));
    assert!(markdown.contains("| Aggregate minus combined member rate | 300 |"));
    assert!(markdown.contains("Exact point closure verified: true"));
    // Printed site rates 8250 and 8167 are deliberately rounded independently. The bridge uses dollars
    // and sales, not a weighted average of those rates, and does not quietly treat them as exact inputs.
    let changed_rates = report(
        &[
            ("C16", MinimalCell::Number(8251.0)),
            ("D16", MinimalCell::Number(8168.0)),
        ],
        UNITS,
        false,
    );
    let changed_json = assure_report::consolidation_json(&changed_rates);
    assert!(changed_json.contains("\"members_usd_per_oz\":{\"point\":\"8200\""));
    assert_ne!(
        json, changed_json,
        "source hash still commits to printed rate changes"
    );
}

#[test]
fn unequal_sales_have_a_separate_signed_per_ounce_denominator_effect() {
    let r = report(&[("B13", MinimalCell::Number(102.0))], UNITS, false);
    let json = assure_report::consolidation_json(&r);
    // Exact hand calculation: aggregate = 425000/51, member = 8200; gap = 6800/51.
    // G&A contributes 5000/17 and 820000*(1/102 - 1/100) contributes -8200/51.
    for fragment in [
        "\"status\":\"unresolved\"",
        "\"aggregate_usd_per_oz\":{\"point\":\"425000/51\"",
        "\"members_usd_per_oz\":{\"point\":\"8200\"",
        "\"gap_usd_per_oz\":{\"point\":\"6800/51\"",
        "\"source_line\":\"A9: General & administrative costs\",\"difference_usd_per_oz\":{\"point\":\"5000/17\"",
        "\"denominator_usd_per_oz\":{\"point\":\"-8200/51\"",
        "\"point_closure_verified\":true",
    ] {
        assert!(json.contains(fragment), "missing {fragment}: {json}");
    }
    let markdown = assure_report::markdown(&r).unwrap();
    assert!(markdown.contains("| Sales denominator difference | -8200/51 |"));
    assert!(markdown.contains("Printed arithmetic closes: false"));
}

#[test]
fn unexplained_per_ounce_amount_survives_exact_closure_without_becoming_an_allocation() {
    let r = report(&[("B11", MinimalCell::Number(851.0))], UNITS, false);
    let json = assure_report::consolidation_json(&r);
    for fragment in [
        "\"status\":\"unresolved\"",
        "\"gap_usd_per_oz\":{\"point\":\"310\"",
        "\"unexplained_usd_per_oz\":{\"point\":\"10\"",
        "\"point_closure_verified\":true",
        "\"accounting_reconciliation_verified\":false",
    ] {
        assert!(json.contains(fragment), "missing {fragment}: {json}");
    }
    let markdown = assure_report::markdown(&r).unwrap();
    assert!(markdown.contains("| Unexplained numerator remainder | 10 |"));
    assert!(markdown.contains("does not resolve an unexplained remainder"));
}

#[test]
fn zero_sales_refuse_only_the_per_ounce_comparison_and_keep_source_dollars() {
    for changes in [
        vec![("B13", MinimalCell::Number(0.0))],
        vec![
            ("C13", MinimalCell::Number(0.0)),
            ("D13", MinimalCell::Number(0.0)),
        ],
    ] {
        let r = report(&changes, UNITS, false);
        let json = assure_report::consolidation_json(&r);
        assert!(
            json.contains(&format!(
                "\"per_ounce\":{{\"rules_version\":\"{}\",\"status\":\"unavailable\"",
                consolidation::RATE_RULES_VERSION
            )),
            "{json}"
        );
        assert!(json.contains("\"numerator_difference_usd_m\":{\"point\":\"30\""));
        assert!(json.contains("\"source_line\":\"A9: General & administrative costs\",\"difference_usd_m\":{\"point\":\"30\""));
        assert!(!json.contains("\"aggregate_usd_per_oz\":"));
        for cell in ["B13", "C13", "D13"] {
            let at = format!("invented_aisc.xlsx:Q1 2030!{cell}");
            let source = r.public_claims.cells.get(&at).unwrap();
            assert!(json.contains(&source.claim.claim_id.to_string()));
        }
        let markdown = assure_report::markdown(&r).unwrap();
        assert!(markdown.contains("Per-ounce comparison unavailable"));
        assert!(markdown.contains("The dollar comparison above is retained"));
    }
}

#[test]
fn missing_member_placeholder_and_wrong_units_refuse_instead_of_summing_a_subset() {
    for (changes, units) in [
        (
            vec![("D2", MinimalCell::Text("Unregistered member".into()))],
            UNITS,
        ),
        (vec![("D10", MinimalCell::Text("—".into()))], UNITS),
        (vec![], "(C$ millions, except per ounce information)"),
        (vec![], "(AUS$ millions, except per ounce information)"),
        (vec![], "(CAD millions; US$ millions comparative)"),
        (vec![], "($ millionsAUD, except per ounce information)"),
        (
            vec![(
                "A13",
                MinimalCell::Text("Ounces produced (000s ounces)".into()),
            )],
            UNITS,
        ),
    ] {
        let json = assure_report::consolidation_json(&report(&changes, units, false));
        // A structurally absent ounces-sold row is already a named table refusal in the source report.
        if changes.iter().any(|(at, _)| *at == "A13") {
            assert!(!report(&changes, units, false).table_refusals.is_empty());
        } else {
            assert!(json.contains("\"status\":\"unavailable\""), "{json}");
            assert!(json.contains("\"subject\":\"Site B\""));
        }
        assert!(!json.contains("\"printed_arithmetic_closed\":true"));
    }
}

#[test]
fn exact_currency_labels_and_repeated_source_metric_occurrences_are_admitted() {
    for units in [
        "$ millions",
        "(US$ millions, except per ounce information)",
        "USD millions, except per ounce information in dollars",
    ] {
        let json = assure_report::consolidation_json(&report(&[], units, false));
        assert!(
            json.contains("\"gap_usd_per_oz\":{\"point\":\"300\""),
            "{json}"
        );
    }
    let r = report(
        &[("A10", MinimalCell::Text("Non-controlling interests".into()))],
        UNITS,
        false,
    );
    let second = &r.public_claims.cells["invented_aisc.xlsx:Q1 2030!C10"];
    assert_eq!(second.key.line.label, "non-controlling interests");
    assert_eq!(second.key.line.occurrence, 1);
    let json = assure_report::consolidation_json(&r);
    assert!(
        json.contains("\"gap_usd_per_oz\":{\"point\":\"300\""),
        "{json}"
    );
}

#[test]
fn registered_region_scope_is_reconstructed_from_the_same_release_membership() {
    let mut p = synthetic::synthetic_profile();
    p.subjects = ["Declared subtotal", "Site A", "Site B"]
        .map(str::to_string)
        .into();
    p.regions = vec!["Declared subtotal".into()];
    p.shares.clear();
    p.reconstructions.clear();
    p.missing_facts.clear();
    p.region_members = ["Site A", "Site B"]
        .map(|member| RegionMemberRow {
            region: "Declared subtotal".into(),
            member: member.into(),
            first: "2030Q1".into(),
            last: None,
        })
        .into();
    let mut cells: Vec<_> = [
        ("A1", "Summary of Operations"),
        ("A3", "For the three months ended March 31,"),
        ("B4", "Gold produced (000s oz 100%)"),
        ("C4", "Gold produced (000s oz attributable)"),
        ("D4", "Gold sold (000s oz attributable)"),
        ("E4", "Cost of sales ($/oz)"),
        ("F4", "Total cash costs ($/oz)"),
        ("G4", "All-In sustaining costs ($/oz)"),
        ("A5", "Declared subtotal"),
        ("A6", "Total Gold"),
    ]
    .into_iter()
    .map(|(at, text)| (at.into(), MinimalCell::Text(text.into())))
    .collect();
    cells.push(("B3".into(), MinimalCell::Number(2030.0)));
    for row in [5, 6] {
        for (col, value) in ["B", "C", "D", "E", "F", "G"]
            .into_iter()
            .zip([100.0, 100.0, 100.0, 10000.0, 7000.0, 8500.0])
        {
            cells.push((format!("{col}{row}"), MinimalCell::Number(value)));
        }
    }
    let bytes = build_minimal_xlsx(&[MinimalSheet {
        name: "QTD".into(),
        hidden: false,
        cells,
    }]);
    let workbook = Workbook::from_bytes("invented_stats.xlsx", &bytes).unwrap();
    let stats = LoadedSource {
        file_name: "invented_stats.xlsx".into(),
        kind: SourceKind::MineStats,
        quarter: "2030Q1".into(),
        url: "synthetic:invented".into(),
        url_basis: "synthetic test only".into(),
        bytes: bytes.len() as u64,
        census: workbook.cell_census(),
        workbook,
    };
    let mut r = assess(&p, Lane::Synthetic, vec![source(&[], UNITS), stats]);
    assert!(!r.mine_ledger.cost_layers.rows.is_empty());
    let json = assure_report::consolidation_json(&r);
    assert!(
        json.contains("\"gap_usd_per_oz\":{\"point\":\"300\""),
        "{json}"
    );
    let cell = r
        .public_claims
        .cells
        .get_mut("invented_aisc.xlsx:Q1 2030!B5")
        .unwrap();
    cell.key.scope = fmine_public::key::Scope::Region {
        map_version: "different-membership".into(),
    };
    let tag = fmine_public::key::canonical_value_tag(&cell.key, &cell.claim.value);
    let old = &cell.claim;
    cell.claim = Claim::new(
        &old.metric_key,
        old.value.clone(),
        &tag,
        old.representation.clone(),
        old.support,
        old.additivity,
        old.evidence,
        old.epistemic,
        old.state,
        old.confidentiality.clone(),
        old.uu_terms.clone(),
        old.parents.clone(),
        old.anchors.clone(),
    );
    cell.claim.check_id().unwrap();
    let json = assure_report::consolidation_json(&r);
    assert!(json.contains("\"status\":\"unavailable\""), "{json}");
    assert!(!json.contains("\"gap_usd_per_oz\":"));
}

#[test]
fn bound_source_claims_cannot_change_knowledge_semantic_key_or_numeric_support() {
    use fmine_public::key::canonical_value_tag;
    for change in 0..15 {
        let mut r = report(&[], UNITS, false);
        let cell = r
            .public_claims
            .cells
            .get_mut("invented_aisc.xlsx:Q1 2030!C5")
            .unwrap();
        let state = match change {
            0 => KnowledgeState::Conflicted,
            1 => KnowledgeState::NotObservable,
            2 => KnowledgeState::Redacted,
            3 => {
                cell.key.subject = "Site B".into();
                KnowledgeState::Known
            }
            4 => {
                cell.key.valid_period.year = 2031;
                KnowledgeState::Known
            }
            5 => {
                cell.key.reporting_vintage.release = "2030Q2".into();
                KnowledgeState::Known
            }
            6 => {
                cell.key.issuer = "another-issuer".into();
                KnowledgeState::Known
            }
            10 => {
                cell.key.line.label = "unrelated monetary amount".into();
                KnowledgeState::Known
            }
            11 => {
                cell.key.line.occurrence = 1;
                KnowledgeState::Known
            }
            12 => {
                cell.key.scope = fmine_public::key::Scope::Company;
                KnowledgeState::Known
            }
            13 => {
                cell.locator = "invented_aisc.xlsx:Q1 2030!D5".into();
                KnowledgeState::Known
            }
            _ => KnowledgeState::Known,
        };
        let tag = canonical_value_tag(&cell.key, &cell.claim.value);
        let old = &cell.claim;
        cell.claim = Claim::new(
            if change == 14 {
                "unrelated monetary amount"
            } else {
                &cell.key.line.label
            },
            old.value.clone(),
            &tag,
            if change == 7 {
                Representation::Gaussian {
                    mean: 400.0,
                    std_dev: 1.0,
                }
            } else {
                old.representation.clone()
            },
            if change == 8 {
                SpatialSupport::PointCore
            } else {
                old.support
            },
            if change == 9 {
                AdditivityClass::NonAdditive
            } else {
                old.additivity
            },
            old.evidence,
            old.epistemic,
            state,
            old.confidentiality.clone(),
            old.uu_terms.clone(),
            old.parents.clone(),
            old.anchors.clone(),
        );
        cell.claim.check_id().unwrap();
        let json = assure_report::consolidation_json(&r);
        assert!(
            json.contains("\"status\":\"unavailable\""),
            "change {change}: {json}"
        );
        assert!(!json.contains("\"printed_arithmetic_closed\":true"));
    }
}

#[test]
fn source_unknown_unknown_scenarios_survive_once_in_the_comparison() {
    let mut r = report(&[], UNITS, false);
    for cell in ["C5", "D5"] {
        let source = r
            .public_claims
            .cells
            .get_mut(&format!("invented_aisc.xlsx:Q1 2030!{cell}"))
            .unwrap();
        source.claim.uu_terms.push(fmine_claim::UuTerm {
            scenario_id: "invented-tail".into(),
            description: "An invented source scenario".into(),
            downside_exposure_usd_m: 12.5,
            trigger_indicator: "An invented observable trigger".into(),
        });
    }
    let json = assure_report::consolidation_json(&r);
    assert!(json.contains("\"printed_arithmetic_closed\":true"));
    assert_eq!(json.matches("\"scenario_id\":\"invented-tail\"").count(), 1);
    assert!(json.contains("\"downside_exposure_usd_m\":\"12.5\""));
    assert!(json.contains("An invented observable trigger"));
}

#[test]
fn cancelling_unlabelled_numeric_components_are_retained_and_refused() {
    let changes = [
        ("B9", 30.0),
        ("B10", -30.0),
        ("B11", 700.0),
        ("C9", 10.0),
        ("C10", -10.0),
        ("C11", 280.0),
        ("D9", 20.0),
        ("D10", -20.0),
        ("D11", 420.0),
    ]
    .map(|(at, n)| (at, MinimalCell::Number(n)));
    let source = source_omitting_labels(&changes, UNITS, &["A9", "A10"]);
    let json = assure_report::consolidation_json(&report_source(source, false));
    assert!(
        json.contains("unlabelled numeric source components"),
        "{json}"
    );
    assert!(json.contains("\"status\":\"unavailable\""));
    assert!(!json.contains("\"printed_arithmetic_closed\":true"));
    for cell in ["B9", "B10", "C9", "C10", "D9", "D10"] {
        assert!(
            json.contains(&format!("invented_aisc.xlsx:Q1 2030!{cell}")),
            "missing {cell}"
        );
    }
}
