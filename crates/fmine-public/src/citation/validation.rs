//! DESIGN (BD-329.S2 slice): authenticate typed workbook citations before emission.
//! This checks source Claims and operands behind the four AISC derived-number rules.
//! It does not certify every JSON number, historical source completeness or issuer truth.
//!
//! Determinism: Exact. Ordered source projections yield sorted, deduplicated findings.

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::hash::Digest32;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::xlsx::{CellValue, Sheet, col_index_to_letters, parse_a1_ref};

use super::{Cited, RULES, derived, locator};
use crate::aisc::{self, Calc, IdentityInstance, OperandRole, OperandValue};
use crate::key::{self, KeyedCell, NilReason, PublicValue, StatusKind};
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
) -> Result<(&'a LoadedSource, &'a Sheet, u32, u32), &'static str> {
    let (file, rest) = at.split_once(':').ok_or("invalid_locator")?;
    let (sheet_name, cell_ref) = rest.rsplit_once('!').ok_or("invalid_locator")?;
    let source = *sources.get(file).ok_or("unregistered_source")?;
    let sheet = source.workbook.sheet(sheet_name).ok_or("unavailable_sheet")?;
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
        resolve(sources, &cell.locator).map_err(|kind| ("at", kind))?;
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

fn placeholder(sources: &Sources<'_>, operand: &aisc::Operand) -> bool {
    resolve(sources, &locator(operand)).is_ok_and(|(_, sheet, row, col)| {
        sheet
            .cell(row, col)
            .is_some_and(|cell| matches!(cell.value, CellValue::Placeholder(_)))
    })
}

fn calculation_matches(sources: &Sources<'_>, instance: &IdentityInstance) -> bool {
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
                _ if placeholder(sources, o) => Some(sum),
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
                    matches!(o.value, OperandValue::Int(_)) || placeholder(sources, o)
                })
                && i64::try_from(count).ok() == Some(rate.k)
                && one(OperandRole::Numerator) == Some(rate.n)
                && one(OperandRole::Ounces) == Some(rate.q)
                && one(OperandRole::Rate) == Some(rate.r)
        }
    }
}

/// Rechecks canonical workbook Claims and visible AISC citations against loaded sources.
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
        if sources
            .insert(source.file_name.as_str(), source)
            .is_some()
        {
            add(
                format!("$.sources[{index}].file"),
                "duplicate_source",
                &source.file_name,
            );
        }
    }
    if (report.lane == Lane::Synthetic) != (report.issuer == crate::synthetic::SYNTHETIC_ISSUER)
    {
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
        let path = format!("$.instances[{index}].cited");
        let mut seen = BTreeSet::new();
        for (operand_index, operand) in instance.operands.iter().enumerate() {
            let at = locator(operand);
            let issue = resolve(&sources, &at).map(|(source, sheet, row, col)| {
                aisc::operand_at(&source.file_name, sheet, row, col, operand.role) == *operand
            });
            let kind = match issue {
                Err(kind) => Some(kind),
                Ok(false) => Some("source_value_mismatch"),
                Ok(true) => None,
            };
            if let Some(kind) = kind {
                add(format!("{path}.operands[{operand_index}]"), kind, &at);
            }
            if !seen.insert((operand.role.as_str(), at.clone())) {
                add(
                    format!("{path}.operands[{operand_index}]"),
                    "duplicate_operand",
                    &at,
                );
            }
        }
        let at = format!("{}:{}!{}", instance.file, instance.sheet, instance.cell);
        if !calculation_matches(&sources, instance) {
            add(format!("{path}.derived"), "derived_input_mismatch", &at);
            continue;
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
    for (index, cell) in report.mine_stats_claims.claims.iter().enumerate() {
        if report.public_claims.cells.get(&cell.locator) != Some(cell) {
            add(
                format!("$.mine_stats_claims.claims[{index}]"),
                "noncanonical_mine_stats_claim",
                &cell.locator,
            );
        }
    }
    if key::claims_digest(&report.mine_stats_claims.claims) != report.mine_stats_claims.digest
    {
        add(
            "$.mine_stats_claims.digest".into(),
            "projection_digest_mismatch",
            "",
        );
    }
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
