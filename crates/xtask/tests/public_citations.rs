//! BD-329.S2 slice: source-to-report citation integrity with explicit mutation controls.
//! Invented workbook answers establish implementation behavior, not an issuer-data gate.

use fmine_core::hash::Digest32;
use fmine_io::xlsx::MinimalCell;
use fmine_public::aisc::{Calc, OperandRole, OperandValue};
use fmine_public::citation::{report_findings, validate_report};
use fmine_public::key::{self, PublicValue};
use fmine_public::{PublicAssuranceReport, run_synthetic_assurance};

fn fixture() -> PublicAssuranceReport {
    run_synthetic_assurance(&[]).expect("invented source-to-report run")
}

fn has(report: &PublicAssuranceReport, path: &str, kind: &str) {
    let findings = report_findings(report);
    assert!(
        findings.iter().any(|f| f.path == path && f.kind == kind),
        "missing {path}: {kind}; found {findings:?}"
    );
    let refusal = validate_report(report).expect_err("inconsistent citations must refuse");
    assert_eq!((refusal.code, refusal.exit_code), ("FMINE-REF-208", 3));
    assert!(refusal.what.contains(path));
}

fn numeric_cell(report: &PublicAssuranceReport) -> (usize, String) {
    report
        .public_claims
        .cells
        .iter()
        .enumerate()
        .find(|(_, (_, cell))| matches!(cell.claim.value, PublicValue::Number { .. }))
        .map(|(index, (at, _))| (index, at.clone()))
        .expect("the fixture contains a numeric Claim")
}

#[test]
fn valid_source_claims_and_existing_refusals_are_preserved() {
    let report = fixture();
    assert!(!report.public_claims.cells.is_empty());
    assert!(report.instances.iter().any(|i| matches!(i.calc, Calc::None)));
    assert!(report.instances.iter().any(|i| i.state.code() == "v"));
    assert!(
        report
            .public_claims
            .cells
            .values()
            .any(|c| matches!(c.claim.value, PublicValue::Nil { .. }))
    );
    assert!(report_findings(&report).is_empty());
    validate_report(&report).expect("valid citations");
    assert_eq!(report.to_json(), fixture().to_json());
}

#[test]
fn raw_and_displayed_numeric_mutations_name_the_exact_payload_path() {
    for displayed_only in [false, true] {
        let mut report = fixture();
        let (index, at) = numeric_cell(&report);
        let cell = report.public_claims.cells.get_mut(&at).expect("numeric cell");
        if let PublicValue::Number { stored, displayed, .. } = &mut cell.claim.value {
            if displayed_only {
                *displayed = Ok("999999".into());
            } else {
                *stored = "999999".into();
            }
        }
        has(
            &report,
            &format!("$.public_claims.cells[{index}].value"),
            "source_value_mismatch",
        );
    }
}

#[test]
fn missing_digest_anchor_and_rebound_vintage_refuse() {
    for case in 0..3 {
        let mut report = fixture();
        let (index, at) = numeric_cell(&report);
        let cell = report.public_claims.cells.get_mut(&at).expect("numeric cell");
        let (field, kind) = match case {
            0 => {
                cell.key.reporting_vintage.file_sha256.clear();
                ("sha256", "source_digest_mismatch")
            }
            1 => {
                cell.claim.anchors.clear();
                ("claim_id", "source_anchor_mismatch")
            }
            _ => {
                cell.key.reporting_vintage.release = "2099Q4".into();
                ("vintage", "source_identity_mismatch")
            }
        };
        has(
            &report,
            &format!("$.public_claims.cells[{index}].{field}"),
            kind,
        );
    }
}

#[test]
fn a_semantic_key_cannot_reuse_another_claims_content_address() {
    let mut report = fixture();
    let (index, at) = numeric_cell(&report);
    report.public_claims.cells.get_mut(&at).expect("cell").key.subject = "Another site".into();
    has(
        &report,
        &format!("$.public_claims.cells[{index}].claim_id"),
        "claim_value_tag_mismatch",
    );
}

#[test]
fn missing_unregistered_and_duplicate_source_locators_refuse() {
    let mut report = fixture();
    let (index, at) = numeric_cell(&report);
    report.public_claims.cells.get_mut(&at).expect("cell").locator =
        "unregistered.xlsx:S!A1".into();
    has(
        &report,
        &format!("$.public_claims.cells[{index}].at"),
        "unregistered_source",
    );

    let mut report = fixture();
    let source = report.sources[0].clone();
    let index = report.sources.len();
    report.sources.push(source);
    has(&report, &format!("$.sources[{index}].file"), "duplicate_source");

    let mut report = fixture();
    report.sources.clear();
    assert!(
        report_findings(&report)
            .iter()
            .any(|f| f.kind == "unregistered_source")
    );
    assert!(validate_report(&report).is_err());
}

#[test]
fn cached_aisc_operands_cannot_differ_from_the_loaded_cell() {
    let mut report = fixture();
    let (index, operand_index) = report
        .instances
        .iter()
        .enumerate()
        .find_map(|(i, instance)| {
            instance
                .operands
                .iter()
                .position(|o| matches!(o.value, OperandValue::Int(_)))
                .map(|j| (i, j))
        })
        .expect("numeric arithmetic operand");
    if let OperandValue::Int(value) = &mut report.instances[index].operands[operand_index].value {
        *value += 1;
    }
    has(
        &report,
        &format!("$.instances[{index}].cited.operands[{operand_index}]"),
        "source_value_mismatch",
    );
}

#[test]
fn cached_sums_and_numeric_line_counts_need_their_original_ancestry() {
    for rate_case in [false, true] {
        let mut report = fixture();
        let index = report
            .instances
            .iter()
            .position(|i| {
                if rate_case {
                    matches!(i.calc, Calc::Rate(_))
                } else {
                    matches!(i.calc, Calc::Footing { .. })
                }
            })
            .expect("the fixture contains both calculations");
        match &mut report.instances[index].calc {
            Calc::Footing { component_sum, .. } => *component_sum += 1,
            Calc::Rate(rate) => rate.k += 1,
            Calc::None => panic!("selected arithmetic calculation"),
        }
        has(
            &report,
            &format!("$.instances[{index}].cited.derived"),
            "derived_input_mismatch",
        );
    }
    let mut report = fixture();
    let index = report
        .instances
        .iter()
        .position(|i| matches!(i.calc, Calc::Footing { .. }))
        .expect("footing");
    report.instances[index].operands.retain(|o| o.role != OperandRole::Line);
    has(
        &report,
        &format!("$.instances[{index}].cited.derived"),
        "derived_input_mismatch",
    );
}

#[test]
fn missing_source_cells_are_retained_as_missing_without_fabricating_a_number() {
    let mut report = fixture();
    let (_, at) = numeric_cell(&report);
    let mut key = report.public_claims.cells[&at].key.clone();
    let file = at.split_once(':').expect("source locator").0;
    let source = report.sources.iter().find(|s| s.file_name == file).expect("source");
    let sheet = &source.workbook.sheets[0];
    assert!(sheet.cell(999, 26).is_none());
    key.line.label = "declared missing observation".into();
    let missing = key::public_claim(&source.workbook, &sheet.name, "Z999", key, PublicValue::Missing);
    report.public_claims.cells.insert(missing.locator.clone(), missing);
    validate_report(&report).expect("a supported missing value remains a missing value");
}

#[test]
fn copied_mine_stats_claims_and_their_digest_are_checked() {
    let mut report = fixture();
    assert!(!report.mine_stats_claims.claims.is_empty());
    report.mine_stats_claims.claims[0].key.subject = "Unbound copy".into();
    has(
        &report,
        "$.mine_stats_claims.claims[0]",
        "noncanonical_mine_stats_claim",
    );
    let mut report = fixture();
    report.mine_stats_claims.digest = Digest32([0; 32]);
    has(&report, "$.mine_stats_claims.digest", "projection_digest_mismatch");
}

#[test]
fn additional_interpretation_anchors_survive_the_primary_source_check() {
    let mut report = fixture();
    let (_, at) = numeric_cell(&report);
    let original = report.public_claims.cells[&at].clone();
    let primary = original.claim.anchors[0].clone();
    let mut interpretation = primary.clone();
    interpretation.locator = "Q1 2030!A1".into();
    let rebuilt = key::public_claim_anchored(
        primary,
        vec![interpretation],
        original.key.clone(),
        original.claim.value.clone(),
    );
    report.public_claims.cells.get_mut(&at).expect("cell").claim = rebuilt.claim;
    let replacement = report.public_claims.cells[&at].clone();
    for cell in &mut report.mine_stats_claims.claims {
        if cell.locator == at {
            *cell = replacement.clone();
        }
    }
    report.mine_stats_claims.digest = key::claims_digest(&report.mine_stats_claims.claims);
    validate_report(&report).expect("additional interpretation provenance remains intact");
}

#[test]
fn every_loader_placeholder_token_preserves_the_existing_arithmetic() {
    let denominator = fixture().instances.len();
    for token in ["-", "–", "—", "n/a", "N/A", "NA", "na", " NA "] {
        let report = run_synthetic_assurance(&[(
            "B7".into(),
            MinimalCell::Text(token.into()),
        )])
        .expect("a loader-recognized placeholder is an exact zero line");
        assert_eq!(report.instances.len(), denominator);
        assert!(report.instances.iter().any(|instance| {
            instance.subject == "Site A" && matches!(instance.calc, Calc::Footing { .. })
        }));
        assert!(report.instances.iter().flat_map(|i| &i.operands).any(|operand| {
            operand.file == "Synthetic_AISC.xlsx"
                && operand.sheet == "Q1 2030"
                && operand.cell == "B7"
                && matches!(&operand.value, OperandValue::Token(stored) if stored == token)
        }));
        validate_report(&report).expect("placeholder semantics agree with the loader");
    }
}
