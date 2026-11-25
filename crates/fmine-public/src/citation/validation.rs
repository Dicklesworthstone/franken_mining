//! DESIGN (BD-329.S2 slice): authenticate typed workbook citations before emission.
//! This checks AISC calculation inputs and rendered footing certificates, including hidden exceptions,
//! and the same-release workbook projection.
//! It does not certify every JSON number, historical source completeness or issuer truth.
//!
//! Determinism: Exact (ordered source projections yield sorted, deduplicated findings).

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::hash::Digest32;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::xlsx::{CellValue, Sheet, col_index_to_letters, parse_a1_ref};

use super::{Cited, RULES, derived, locator};
use crate::aisc::{self, Calc, IdentityInstance, IdentityState, OperandRole, OperandValue};
use crate::key::{self, KeyedCell, NilReason, PublicValue, StatusKind};
use crate::sources::SourceKind;
use crate::{Lane, LoadedSource, PublicAssuranceReport};

/// A deterministic report-field path, with the offending source kept separately.
/// Public Claim and visible AISC paths also identify the emitted payload fields.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CitationFinding {
    pub path: String,
    pub kind: &'static str,
    pub at: String,
}

type Sources<'a> = BTreeMap<&'a str, &'a LoadedSource>;

fn resolve<'a>(
    sources: &Sources<'a>,
    at: &str,
    include_hidden: bool,
) -> Result<(&'a LoadedSource, &'a Sheet, u32, u32), &'static str> {
    let (file, rest) = at.split_once(':').ok_or("invalid_locator")?;
    let (sheet_name, cell_ref) = rest.rsplit_once('!').ok_or("invalid_locator")?;
    let source = *sources.get(file).ok_or("unregistered_source")?;
    let workbook = &source.workbook;
    let sheet = workbook
        .sheet(sheet_name)
        .or_else(|| {
            if !include_hidden {
                return None;
            }
            workbook
                .hidden_sheets
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(sheet_name))
        })
        .ok_or("unavailable_sheet")?;
    let (row, col) = parse_a1_ref(cell_ref).ok_or("invalid_locator")?;
    if format!("{}{row}", col_index_to_letters(col)) != cell_ref {
        return Err("invalid_locator");
    }
    Ok((source, sheet, row, col))
}

fn claim_issue(
    sources: &Sources<'_>,
    issuer: &str,
    cell: &KeyedCell,
) -> Result<(), (&'static str, &'static str)> {
    let (source, sheet, row, col) =
        resolve(sources, &cell.locator, false).map_err(|kind| ("at", kind))?;
    if cell.key.issuer != issuer || cell.key.reporting_vintage.release != source.quarter {
        return Err(("vintage", "source_identity_mismatch"));
    }
    if cell.key.reporting_vintage.file_sha256 != Digest32(source.workbook.sha256).to_hex() {
        return Err(("sha256", "source_digest_mismatch"));
    }
    let cell_ref = format!("{}{row}", col_index_to_letters(col));
    let anchor = source.workbook.make_anchor(&sheet.name, &cell_ref);
    if cell.claim.anchors.iter().filter(|a| *a == &anchor).count() != 1 {
        return Err(("claim_id", "source_anchor_mismatch"));
    }
    let status = match &cell.claim.value {
        PublicValue::Number { under_status, .. } => *under_status,
        PublicValue::Nil {
            reason: NilReason::Disposed,
            ..
        } => Some(StatusKind::Disposed),
        PublicValue::Nil {
            reason: NilReason::Withheld,
            ..
        } => Some(StatusKind::Withheld),
        _ => None,
    };
    if status.is_some() && cell.status_evidence.as_deref().is_none_or(str::is_empty) {
        return Err(("status_evidence", "uncited_status_interpretation"));
    }
    let expected = key::classify(sheet.cell(row, col), status)
        .map_err(|_| ("value", "source_value_unusable"))?;
    if expected != cell.claim.value {
        return Err(("value", "source_value_mismatch"));
    }
    if cell.claim.value_tag() != key::canonical_value_tag(&cell.key, &cell.claim.value) {
        return Err(("claim_id", "claim_value_tag_mismatch"));
    }
    if cell.claim.metric_key != cell.key.line.label
        || !cell.claim.parents.is_empty()
        || cell.claim.check_id().is_err()
    {
        return Err(("claim_id", "claim_body_mismatch"));
    }
    Ok(())
}

fn placeholder(sources: &Sources<'_>, operand: &aisc::Operand, include_hidden: bool) -> bool {
    resolve(sources, &locator(operand), include_hidden).is_ok_and(|(_, sheet, row, col)| {
        sheet
            .cell(row, col)
            .is_some_and(|cell| matches!(cell.value, CellValue::Placeholder(_)))
    })
}

fn calculation_matches(
    sources: &Sources<'_>,
    instance: &IdentityInstance,
    include_hidden: bool,
) -> bool {
    let one = |role| {
        let mut values = instance.operands.iter().filter(|o| o.role == role);
        let first = values.next()?;
        if values.next().is_some() {
            return None;
        }
        match first.value {
            OperandValue::Int(value) => Some(value),
            _ => None,
        }
    };
    match instance.calc {
        Calc::None => true,
        Calc::Footing {
            printed,
            component_sum,
            ..
        } => {
            let lines: Vec<_> = instance
                .operands
                .iter()
                .filter(|o| o.role == OperandRole::Line)
                .collect();
            let sum = lines.iter().try_fold(0_i128, |sum, o| match &o.value {
                OperandValue::Int(value) => sum.checked_add(i128::from(*value)),
                _ if placeholder(sources, o, include_hidden) => Some(sum),
                _ => None,
            });
            !lines.is_empty()
                && one(OperandRole::Total) == Some(printed)
                && sum == Some(i128::from(component_sum))
                && printed.checked_sub(component_sum).is_some()
        }
        Calc::Rate(rate) => {
            let counted = if instance
                .operands
                .iter()
                .any(|o| o.role == OperandRole::Counted)
            {
                OperandRole::Counted
            } else {
                OperandRole::Numerator
            };
            let inputs: Vec<_> = instance
                .operands
                .iter()
                .filter(|o| o.role == counted)
                .collect();
            let count = inputs
                .iter()
                .filter(|o| matches!(o.value, OperandValue::Int(_)))
                .count();
            !inputs.is_empty()
                && inputs.iter().all(|o| {
                    matches!(o.value, OperandValue::Int(_))
                        || placeholder(sources, o, include_hidden)
                })
                && i64::try_from(count).ok() == Some(rate.k)
                && one(OperandRole::Numerator) == Some(rate.n)
                && one(OperandRole::Ounces) == Some(rate.q)
                && one(OperandRole::Rate) == Some(rate.r)
        }
    }
}

fn instance_findings(
    instance: &IdentityInstance,
    sources: &Sources<'_>,
    path: &str,
    include_hidden: bool,
    add: &mut impl FnMut(String, &'static str, &str),
) {
    let mut seen = BTreeSet::new();
    for (index, operand) in instance.operands.iter().enumerate() {
        let at = locator(operand);
        let issue = resolve(sources, &at, include_hidden).map(|(source, sheet, row, col)| {
            aisc::operand_at(&source.file_name, sheet, row, col, operand.role) == *operand
        });
        let kind = match issue {
            Err(kind) => Some(kind),
            Ok(false) => Some("source_value_mismatch"),
            Ok(true) => None,
        };
        if let Some(kind) = kind {
            add(format!("{path}.operands[{index}]"), kind, &at);
        }
        if !seen.insert((operand.role.as_str(), at.clone())) {
            add(
                format!("{path}.operands[{index}]"),
                "duplicate_operand",
                &at,
            );
        }
    }
    let at = format!("{}:{}!{}", instance.file, instance.sheet, instance.cell);
    if !calculation_matches(sources, instance, include_hidden) {
        add(format!("{path}.derived"), "derived_input_mismatch", &at);
        return;
    }
    for (index, (_, cited)) in derived(instance).into_iter().enumerate() {
        if let Cited::Derived { rule, inputs, .. } = cited
            && (!RULES.contains(&rule)
                || inputs.is_empty()
                || inputs
                    .iter()
                    .any(|at| !instance.operands.iter().any(|o| locator(o) == *at)))
        {
            add(
                format!("{path}.derived[{index}]"),
                "unbound_derivation",
                &at,
            );
        }
    }
}

fn footing_certificate_findings(
    instance: &IdentityInstance,
    path: &str,
    add: &mut impl FnMut(String, &'static str, &str),
) {
    let IdentityState::IssuerDiscrepancy(certificate) = &instance.state else {
        return;
    };
    let at = format!("{}:{}!{}", instance.file, instance.sheet, instance.cell);
    let Calc::Footing {
        printed,
        component_sum,
        ..
    } = instance.calc
    else {
        add(
            format!("{path}.certificate"),
            "footing_certificate_mismatch",
            &at,
        );
        return;
    };
    let residual = printed.checked_sub(component_sum);
    let same_sheet = |o: &aisc::Operand| o.file == instance.file && o.sheet == instance.sheet;
    let total_matches = certificate.total_cell == instance.cell
        && instance.operands.iter().any(|o| {
            o.role == OperandRole::Total && same_sheet(o) && o.cell == certificate.total_cell
        });
    let lines = || {
        instance
            .operands
            .iter()
            .filter(|o| o.role == OperandRole::Line)
    };
    let lines_match = lines().all(same_sheet)
        && lines()
            .map(|o| o.cell.as_str())
            .eq(certificate.line_cells.iter().map(String::as_str));
    let witness = lines().find_map(|o| match o.value {
        OperandValue::Int(value)
            if value != 0 && residual.map(i128::from) == Some(-2 * i128::from(value)) =>
        {
            Some(o.cell.as_str())
        }
        _ => None,
    });
    for (field, matches) in [
        ("printed", printed == certificate.printed),
        ("component_sum", component_sum == certificate.component_sum),
        (
            "residual",
            residual == Some(certificate.residual) && certificate.residual != 0,
        ),
        ("total_cell", total_matches),
        ("line_cells", lines_match),
        (
            "sign_flip_witness",
            witness == certificate.sign_flip_witness.as_deref(),
        ),
    ] {
        if !matches {
            add(
                format!("{path}.certificate.{field}"),
                "footing_certificate_mismatch",
                &at,
            );
        }
    }
}

fn workbook_findings(
    report: &PublicAssuranceReport,
    sources: &Sources<'_>,
    add: &mut impl FnMut(String, &'static str, &str),
) {
    for (index, row) in report.workbook_comparisons.rows.iter().enumerate() {
        let path = format!("$.workbook_comparisons.rows[{index}]");
        if row.reconciliation.is_empty() && row.mine_stats.is_empty() {
            add(path.clone(), "empty_workbook_comparison", "");
        }
        for (reconciliation, member, claims, kind) in [
            (
                true,
                "reconciliation",
                &row.reconciliation,
                SourceKind::AiscReconciliation,
            ),
            (false, "mine_stats", &row.mine_stats, SourceKind::MineStats),
        ] {
            for (i, claim) in claims.iter().enumerate() {
                if report.public_claims.cells.get(&claim.locator) != Some(claim)
                    || !row.matches_input(claim, reconciliation)
                    || !resolve(sources, &claim.locator, false)
                        .is_ok_and(|(source, _, _, _)| source.kind == kind)
                {
                    add(
                        format!("{path}.{member}[{i}]"),
                        "noncanonical_workbook_input",
                        &claim.locator,
                    );
                }
            }
        }
        let expected_citations = if row.confirmed_share.is_none() {
            0
        } else if row.metric == "cos_usd_per_oz" {
            4
        } else {
            3
        };
        if !row.confirmed_share_matches() {
            add(
                format!("{path}.confirmed_share"),
                "workbook_confirmed_share_mismatch",
                "",
            );
        }
        let mut seen = BTreeSet::new();
        if row.basis_citations.len() != expected_citations {
            add(
                format!("{path}.basis_citations"),
                "workbook_basis_evidence_count",
                "",
            );
        }
        for (i, note) in row.basis_citations.iter().enumerate() {
            let valid = resolve(sources, &note.at, false).is_ok_and(|(source, sheet, r, c)| {
                source.quarter == row.release
                    && note.sha256 == Digest32(source.workbook.sha256).to_hex()
                    && aisc::cell_text(sheet, r, c).as_deref() == Some(note.text.as_str())
                    && row
                        .reconciliation
                        .iter()
                        .chain(&row.mine_stats)
                        .any(|claim| {
                            claim
                                .locator
                                .split_once(':')
                                .is_some_and(|(file, _)| file == source.file_name)
                        })
            });
            if !valid || !seen.insert(&note.at) {
                add(
                    format!("{path}.basis_citations[{i}]"),
                    "workbook_basis_source_mismatch",
                    &note.at,
                );
            }
        }
        let (state, reason, difference) = row.expected_finding();
        for (field, matches) in [
            ("state", state == row.state),
            ("reason", reason == row.reason),
            ("difference", difference == row.difference),
        ] {
            if !matches {
                add(format!("{path}.{field}"), "workbook_finding_mismatch", "");
            }
        }
    }
}

/// Rechecks canonical workbook Claims, visible AISC citations and rendered hidden exceptions against loaded sources.
/// Blank cells, placeholders, unsupported displays and ledger refusals retain their meanings.
/// File-byte authentication remains the loader's responsibility; registry interpretations
/// are not re-proved here. This is not the general uncited-JSON-number validator of BD-329.S2.
///
/// Determinism: Exact. Findings sort by path, kind and source locator.
#[must_use]
pub fn report_findings(report: &PublicAssuranceReport) -> Vec<CitationFinding> {
    let mut findings = Vec::new();
    let mut add = |path: String, kind, at: &str| {
        findings.push(CitationFinding {
            path,
            kind,
            at: at.into(),
        });
    };
    let mut sources = Sources::new();
    for (index, source) in report.sources.iter().enumerate() {
        if sources.insert(source.file_name.as_str(), source).is_some() {
            add(
                format!("$.sources[{index}].file"),
                "duplicate_source",
                &source.file_name,
            );
        }
    }
    if (report.lane == Lane::Synthetic) != (report.issuer == crate::synthetic::SYNTHETIC_ISSUER) {
        add("$.lane".into(), "lane_issuer_mismatch", &report.issuer);
    }
    for (index, (at, cell)) in report.public_claims.cells.iter().enumerate() {
        let path = format!("$.public_claims.cells[{index}]");
        if at != &cell.locator {
            add(format!("{path}.at"), "claim_index_mismatch", at);
        }
        if let Err((field, kind)) = claim_issue(&sources, &report.issuer, cell) {
            add(format!("{path}.{field}"), kind, &cell.locator);
        }
    }
    for (index, instance) in report.instances.iter().enumerate() {
        instance_findings(
            instance,
            &sources,
            &format!("$.instances[{index}].cited"),
            false,
            &mut add,
        );
    }
    for (section, instances, hidden) in [
        ("identity_ledger", &report.instances, false),
        ("hidden_scope", &report.hidden_scope.instances, true),
    ] {
        for (index, instance) in instances
            .iter()
            .filter(|i| matches!(i.state.code(), "ii" | "iii" | "iv"))
            .enumerate()
        {
            let path = format!("$.{section}.exceptions[{index}]");
            if hidden {
                instance_findings(instance, &sources, &format!("{path}.cited"), true, &mut add);
            }
            footing_certificate_findings(instance, &path, &mut add);
        }
    }
    for (index, cell) in report.mine_stats_claims.claims.iter().enumerate() {
        if report.public_claims.cells.get(&cell.locator) != Some(cell) {
            add(
                format!("$.mine_stats_claims.claims[{index}]"),
                "noncanonical_mine_stats_claim",
                &cell.locator,
            );
        }
    }
    if key::claims_digest(&report.mine_stats_claims.claims) != report.mine_stats_claims.digest {
        add(
            "$.mine_stats_claims.digest".into(),
            "projection_digest_mismatch",
            "",
        );
    }
    workbook_findings(report, &sources, &mut add);
    findings.sort();
    findings.dedup();
    findings
}

/// Refuses an internally inconsistent citation before rendering (FMINE-REF-208, exit 3).
/// This bounded integrity check does not claim migration of every numeric payload field
/// to the future universal citation schema.
///
/// Determinism: Exact. The refusal contains the canonical finding order.
pub fn validate_report(report: &PublicAssuranceReport) -> Result<(), Refusal> {
    let findings = report_findings(report);
    if findings.is_empty() {
        return Ok(());
    }
    Err(Refusal::new(
        RefusalCode::SourceCellUnusable208,
        format!(
            "public citation integrity: {}",
            findings
                .iter()
                .map(|f| format!("{}: {} ({})", f.path, f.kind, f.at))
                .collect::<Vec<_>>()
                .join("; ")
        ),
    ))
}
