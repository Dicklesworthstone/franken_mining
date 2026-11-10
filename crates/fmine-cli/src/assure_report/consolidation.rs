//! DESIGN (BD-273): source binding for exact same-table AISC component comparisons.
//! Explicit region/subtotal memberships and the retained company-versus-regions population select columns.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_claim::{
    AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport, UuTerm,
};
use fmine_core::{Encoder, anchor::SourceAnchor, blake3_hash, hash::Digest32};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_io::xlsx::{CellValue, Sheet, col_index_to_letters};
use fmine_metrics::consolidation::{
    self as kernel, Column, Comparison, Difference, Printed, Table,
};
use fmine_public::aisc::{self, AiscTable, OperandRole, OperandValue};
use fmine_public::key::{
    KeyedCell, LineKey, PublicValue, Scope, canonical_value_tag, parse_sheet_period,
};
use fmine_public::sources::SourceKind;
use fmine_public::{LoadedSource, PublicAssuranceReport};
use fmine_units::ExactQty as Rat;

type Result<T> = std::result::Result<T, Refusal>;

#[derive(Default)]
struct Selection {
    members: BTreeMap<String, String>,
    member_scopes: BTreeMap<String, Scope>,
    partial: bool,
    region: bool,
    company: bool,
    issue: Option<String>,
}

struct Assessment {
    file: String,
    sha256: String,
    release: String,
    sheet: String,
    period: String,
    subject: String,
    selection: Selection,
    inputs: Vec<KeyedCell>,
    context: Vec<(String, String)>,
    labels: Vec<String>,
    result: Result<Claim<Comparison>>,
}

pub(super) struct Report {
    rows: Vec<Assessment>,
}

fn refused(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::SourceCellUnusable208, detail)
}

fn js(text: &str) -> String {
    let mut s = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(s, "\\u{:04x}", c as u32);
            }
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

fn q(q: Rat) -> String {
    if q.den() == 1 {
        q.num().to_string()
    } else {
        format!("{}/{}", q.num(), q.den())
    }
}

fn difference(d: Difference) -> String {
    format!(
        "{{\"point\":{},\"interval\":[{},{}],\"rounding_contains_zero\":{}}}",
        js(&q(d.point)),
        js(&q(d.lo)),
        js(&q(d.hi)),
        d.contains_zero()
    )
}

fn rates_json(c: &Comparison, labels: &[String]) -> String {
    match &c.rates {
        Err(e) => format!(
            "{{\"rules_version\":{},\"status\":\"unavailable\",\"refusal\":{{\"code\":{},\"what\":{},\"fix\":{}}}}}",
            js(kernel::RATE_RULES_VERSION),
            js(e.code),
            js(&e.what),
            js(e.fix)
        ),
        Ok(r) => {
            let components = labels
                .iter()
                .zip(&r.components_usd_per_oz)
                .map(|(label, d)| {
                    format!(
                        "{{\"source_line\":{},\"difference_usd_per_oz\":{}}}",
                        js(label),
                        difference(*d)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"rules_version\":{},\"status\":\"computed\",\"unit\":\"USD/oz\",\"interpretation\":\"Rates recomputed from printed USD million numerators and attributable thousand ounces sold; these are not the printed per-ounce rates. Component and unexplained numerator differences use aggregate sales; a separate denominator term retains unequal sales totals. Exact arithmetic closure does not identify corporate or JV allocations.\",\"aggregate_usd_per_oz\":{},\"members_usd_per_oz\":{},\"gap_usd_per_oz\":{},\"components\":[{components}],\"unexplained_usd_per_oz\":{},\"denominator_usd_per_oz\":{},\"point_closure_verified\":{}}}",
                js(kernel::RATE_RULES_VERSION),
                difference(r.aggregate_usd_per_oz),
                difference(r.members_usd_per_oz),
                difference(r.gap_usd_per_oz),
                difference(r.unexplained_usd_per_oz),
                difference(r.denominator_usd_per_oz),
                r.point_closure_verified
            )
        }
    }
}

fn selections(
    report: &PublicAssuranceReport,
    release: &str,
    first: &str,
    last: &str,
) -> BTreeMap<String, Selection> {
    let mut groups: BTreeMap<String, Selection> = BTreeMap::new();
    for layer in report
        .mine_ledger
        .cost_layers
        .rows
        .iter()
        .filter(|r| r.release == release)
    {
        for m in &layer.region_map {
            let group = groups.entry(m.region.clone()).or_default();
            group.region = true;
            group.members.insert(
                m.member.clone(),
                format!(
                    "region_member: {} -> {}; release {} through {}",
                    m.region,
                    m.member,
                    m.first,
                    m.last.as_deref().unwrap_or("open")
                ),
            );
        }
    }
    for m in &report.subtotal_members {
        if m.overlaps_period(first, last) {
            let group = groups.entry(m.subtotal.clone()).or_default();
            group.partial |= !m.covers_period(first, last);
            group.members.insert(
                m.member.clone(),
                format!(
                    "subtotal_member: {} -> {}; physical period {} through {}; {}",
                    m.subtotal,
                    m.member,
                    m.period_from,
                    m.period_to.as_deref().unwrap_or("open"),
                    m.evidence
                ),
            );
        }
    }
    groups
}

fn region_scope<'a>(members: impl Iterator<Item = &'a str>) -> Scope {
    let mut enc = Encoder::new_frame(*b"RMAP", 1);
    for member in members.collect::<BTreeSet<_>>() {
        enc.put_str(member);
    }
    Scope::Region {
        map_version: blake3_hash(&enc.finish()).to_hex(),
    }
}

fn company_selection(
    report: &PublicAssuranceReport,
    release: &str,
    period: &str,
    first: &str,
    last: &str,
) -> Selection {
    let mut selection = Selection {
        company: true,
        ..Selection::default()
    };
    let layers: Vec<_> = report
        .mine_ledger
        .cost_layers
        .rows
        .iter()
        .filter(|r| r.release == release && r.period.code() == period)
        .collect();
    let Some(basis) = layers.first() else {
        selection.issue = Some(
            "company comparison needs a same-release, same-period declared region population"
                .into(),
        );
        return selection;
    };
    let map_key = |layer: &fmine_public::mine_ledger::CompanyCostLayer| {
        layer
            .region_map
            .iter()
            .map(|m| {
                (
                    m.region.clone(),
                    m.member.clone(),
                    m.first.clone(),
                    m.last.clone(),
                )
            })
            .collect::<BTreeSet<_>>()
    };
    let mut regions: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut assigned = BTreeMap::new();
    for m in &basis.region_map {
        if !m.holds_for(release)
            || m.region.trim().is_empty()
            || m.member.trim().is_empty()
            || m.region == m.member
            || assigned
                .insert(m.member.as_str(), m.region.as_str())
                .is_some()
        {
            selection.issue = Some(
                "company region population has duplicate, overlapping or inapplicable membership"
                    .into(),
            );
        }
        regions.entry(&m.region).or_default().insert(&m.member);
    }
    for layer in &layers {
        if map_key(layer) != map_key(basis)
            || layer.region_map.len() != basis.region_map.len()
            || layer.sources != basis.sources
            || layer.sources.len() != 1
            || layer.sources.iter().any(|(file, digest)| {
                !report.sources.iter().any(|s| {
                    s.kind == SourceKind::MineStats
                        && s.quarter == release
                        && s.workbook.artifact_path == *file
                        && Digest32(s.workbook.sha256) == *digest
                })
            })
        {
            selection.issue =
                Some("company region declarations or their source binding are ambiguous".into());
        }
        for region in &layer.regions {
            regions.entry(&region.region).or_default();
            if !region.mapped {
                selection.issue = Some(format!(
                    "printed company region has no active membership: {}",
                    region.region
                ));
            }
        }
    }
    if regions.is_empty() || regions.values().any(BTreeSet::is_empty) {
        selection.issue = Some(
            "company comparison requires nonempty membership for every declared region".into(),
        );
    }
    for member in assigned.keys() {
        if regions.contains_key(member) {
            selection.issue = Some("company regions must not also appear as site members".into());
        }
    }
    // Expand the registered flat subtotals before checking disjointness. Two different subtotals can
    // overlap through a shared mine even when neither subtotal is itself a selected leaf.
    let mut leaves = BTreeMap::new();
    for member in assigned.keys() {
        let children: Vec<_> = report
            .subtotal_members
            .iter()
            .filter(|m| m.subtotal.as_str() == *member && m.overlaps_period(first, last))
            .collect();
        if children.iter().any(|m| !m.covers_period(first, last)) {
            selection.partial = true;
            selection.issue = Some(format!(
                "company region population has partial-period subtotal membership: {member}"
            ));
        }
        let expanded: BTreeSet<_> = if children.is_empty() {
            BTreeSet::from([*member])
        } else {
            children.iter().map(|m| m.member.as_str()).collect()
        };
        if !children.is_empty() && expanded.len() != children.len() {
            selection.issue = Some(format!(
                "company region population repeats a constituent of subtotal {member}"
            ));
        }
        for leaf in expanded {
            if let Some(previous) = leaves.insert(leaf, *member) {
                selection.issue = Some(format!(
                    "company region population overlaps {previous} and {member} through {leaf}"
                ));
            }
        }
    }
    let sources = basis
        .sources
        .iter()
        .map(|(file, hash)| format!("{file}; sha256 {}", hash.to_hex()))
        .collect::<Vec<_>>()
        .join("; ");
    for (region, members) in regions {
        let rules = basis
            .region_map
            .iter()
            .filter(|m| m.region == region)
            .map(|m| {
                format!(
                    "{} -> {}; release {} through {}",
                    m.region,
                    m.member,
                    m.first,
                    m.last.as_deref().unwrap_or("open")
                )
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join("; ");
        let subtotals = report
            .subtotal_members
            .iter()
            .filter(|m| members.contains(m.subtotal.as_str()) && m.overlaps_period(first, last))
            .map(|m| {
                format!(
                    "subtotal {} -> {}; physical period {} through {}; {}",
                    m.subtotal,
                    m.member,
                    m.period_from,
                    m.period_to.as_deref().unwrap_or("open"),
                    m.evidence
                )
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join("; ");
        selection.members.insert(
            region.into(),
            format!("company-minus-declared-regions/1: Total Gold versus {region}; release {release}; physical period {period}; {rules}; {subtotals}; population source {sources}"),
        );
        selection
            .member_scopes
            .insert(region.into(), region_scope(members.into_iter()));
    }
    selection
}

fn source_line(sheet: &Sheet, table: &AiscTable, row: u32) -> Result<LineKey> {
    // Use the reader's registered anchors so abbreviations have the same canonical metric key.
    let anchors = [
        table.cos_row,
        table.tcc_row,
        table.aisc_row,
        table.ounces_row,
        table.cos_rate_row,
        table.tcc_rate_row,
        table.aisc_rate_row,
    ];
    let label = |r| {
        anchors
            .iter()
            .zip(aisc::ROW_ANCHORS)
            .find_map(|(&at, anchor)| (at == r).then(|| anchor.names[0].to_string()))
            .or_else(|| aisc::cell_text(sheet, r, 1).map(|s| s.to_lowercase()))
            .filter(|s| !s.is_empty())
            .ok_or_else(|| refused("source metric label is missing"))
    };
    let rows: BTreeSet<_> = table
        .block1
        .iter()
        .chain(&table.block2)
        .copied()
        .chain(anchors)
        .collect();
    let expected = label(row)?;
    let mut occurrence = 0;
    for &r in rows.range(..row) {
        if label(r)? == expected {
            occurrence += 1;
        }
    }
    Ok(LineKey {
        label: expected,
        occurrence,
    })
}

#[allow(clippy::too_many_arguments)]
fn reading(
    report: &PublicAssuranceReport,
    src: &LoadedSource,
    sheet: &Sheet,
    table: &AiscTable,
    row: u32,
    col: u32,
    subject: &str,
    unit: &str,
    expected_scope: &Scope,
    inputs: &mut Vec<KeyedCell>,
) -> Result<Printed> {
    let cell = format!("{}{row}", col_index_to_letters(col));
    let at = format!("{}:{}!{cell}", src.file_name, sheet.name);
    let source = report
        .public_claims
        .cells
        .get(&at)
        .ok_or_else(|| refused(format!("missing canonical source Claim: {at}")))?;
    inputs.push(source.clone());
    source.claim.check_id()?;
    let expected_line = source_line(sheet, table, row)?;
    let admitted_state = matches!(
        source.claim.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) || (source.claim.state == KnowledgeState::Unknown
        && source.claim.evidence == EvidenceClass::Assumed
        && source.key.issuer == "synthetic");
    if source.claim.value_tag() != canonical_value_tag(&source.key, &source.claim.value)
        || source.key.reporting_vintage.file_sha256 != Digest32(src.workbook.sha256).to_hex()
        || source.key.reporting_vintage.release != src.quarter
        || source.key.issuer != report.issuer
        || source.key.subject != subject
        || Some(source.key.valid_period) != parse_sheet_period(&sheet.name).ok()
        || source.key.unit != unit
        || source.locator != at
        || source.key.line != expected_line
        || source.claim.metric_key != expected_line.label
        || &source.key.scope != expected_scope
        || source.claim.stale
        || source.claim.calibration_failed
        || !admitted_state
        || !source.claim.confidentiality.is_public()
        || source.claim.support != SpatialSupport::PeriodAggregate
        || source.claim.additivity != AdditivityClass::ExtensiveAdditive
        || source.claim.representation != Representation::Deterministic
    {
        return Err(refused(format!(
            "source identity, unit, support, representation, currentness or public basis unavailable: {at}"
        )));
    }
    let PublicValue::Number {
        stored,
        displayed: Ok(displayed),
        under_status: None,
    } = &source.claim.value
    else {
        return Err(refused(format!(
            "placeholder, missing or status-qualified source amount: {at}"
        )));
    };
    let actual = sheet
        .cell(row, col)
        .ok_or_else(|| refused(format!("missing source cell: {at}")))?;
    if actual.number_text.as_ref() != Some(stored)
        || actual
            .displayed()
            .and_then(std::result::Result::ok)
            .is_none_or(|d| d.text() != *displayed)
        || !source
            .claim
            .anchors
            .contains(&src.workbook.make_anchor(&sheet.name, &cell))
    {
        return Err(refused(format!(
            "source observation no longer matches its Claim: {at}"
        )));
    }
    let OperandValue::Int(value) =
        aisc::operand_at(&src.file_name, sheet, row, col, OperandRole::Line).value
    else {
        return Err(refused(format!(
            "whole displayed source amount required: {at}"
        )));
    };
    Ok(Printed {
        point: Rat::from_integer(i128::from(value)),
        step: Rat::from_integer(1),
    })
}

#[allow(clippy::too_many_arguments)]
fn column(
    report: &PublicAssuranceReport,
    src: &LoadedSource,
    sheet: &Sheet,
    table: &AiscTable,
    subject: &str,
    expected_scope: &Scope,
    inputs: &mut Vec<KeyedCell>,
    context: &mut Vec<(String, String)>,
) -> Result<Column> {
    let headers: Vec<_> = sheet
        .row_cells(table.header_row)
        .into_iter()
        .filter(|h| {
            aisc::cell_text(sheet, table.header_row, h.col).is_some_and(|s| {
                aisc::match_subject(&s, &[subject]).is_some()
                    || (*expected_scope == Scope::Company
                        && subject == "Total Gold"
                        && aisc::match_subject(&s, &["Total"]).is_some())
            })
        })
        .collect();
    let [header] = headers.as_slice() else {
        return Err(refused(format!(
            "{}:{}: expected exactly one column for {subject}",
            src.file_name, sheet.name
        )));
    };
    let prefix = format!(
        "{}:{}!{}",
        src.file_name,
        sheet.name,
        col_index_to_letters(header.col)
    );
    context.push((
        format!("{prefix}{}", table.header_row),
        aisc::cell_text(sheet, table.header_row, header.col).unwrap_or_default(),
    ));
    let mut unlabelled = Vec::new();
    for r in table.cos_row..table.aisc_row {
        if r == table.tcc_row
            || sheet
                .cell(r, 1)
                .is_some_and(|c| matches!(&c.value, CellValue::Text(s) if !s.trim().is_empty()))
        {
            continue;
        }
        if let Some(cell) = sheet.cell(r, header.col)
            && matches!(cell.value, CellValue::Number(_))
        {
            let at = format!("{prefix}{r}");
            context.push((
                at.clone(),
                format!(
                    "unlabelled numeric component: {}",
                    cell.number_text.as_deref().unwrap_or("unavailable")
                ),
            ));
            unlabelled.push(at);
        }
    }
    // Read every known source operand before returning a refusal; a missing cell never shrinks a subtotal.
    let mut values: Vec<_> = table
        .block1
        .iter()
        .chain(&table.block2)
        .chain([&table.aisc_row, &table.ounces_row])
        .map(|&row| {
            reading(
                report,
                src,
                sheet,
                table,
                row,
                header.col,
                subject,
                if row == table.ounces_row {
                    "koz"
                } else {
                    "usd_m"
                },
                expected_scope,
                inputs,
            )
        })
        .collect();
    if !unlabelled.is_empty() {
        return Err(refused(format!(
            "unlabelled numeric source components: {}",
            unlabelled.join(", ")
        )));
    }
    if report
        .public_claims
        .unregistered_all_zero
        .iter()
        .any(|s| s == &prefix)
    {
        return Err(refused(format!(
            "unregistered all-zero source column: {prefix}"
        )));
    }
    let sold_koz = values
        .pop()
        .ok_or_else(|| refused("sold observation absent"))??;
    let numerator_usd_m = values
        .pop()
        .ok_or_else(|| refused("numerator observation absent"))??;
    Ok(Column {
        subject: subject.into(),
        components: values.into_iter().collect::<Result<_>>()?,
        numerator_usd_m,
        sold_koz,
    })
}

fn bind(
    report: &PublicAssuranceReport,
    src: &LoadedSource,
    sheet: &Sheet,
    table: &AiscTable,
    row: &mut Assessment,
) -> Result<Claim<Comparison>> {
    let aggregate_scope = if row.selection.company {
        Scope::Company
    } else if row.selection.region {
        // Match the source key's release-specific region perimeter (key::scope_for).
        region_scope(row.selection.members.keys().map(String::as_str))
    } else {
        Scope::Site
    };
    let mut selected = Vec::new();
    selected.push(column(
        report,
        src,
        sheet,
        table,
        &row.subject,
        &aggregate_scope,
        &mut row.inputs,
        &mut row.context,
    ));
    for member in row.selection.members.keys() {
        selected.push(column(
            report,
            src,
            sheet,
            table,
            member,
            row.selection
                .member_scopes
                .get(member)
                .unwrap_or(&Scope::Site),
            &mut row.inputs,
            &mut row.context,
        ));
    }
    for r in [1, 3, table.ounces_row]
        .into_iter()
        .chain(table.block1.iter().chain(&table.block2).copied())
    {
        row.context.push((
            format!("{}:{}!A{r}", src.file_name, sheet.name),
            aisc::cell_text(sheet, r, 1).unwrap_or_default(),
        ));
    }
    row.context.sort();
    row.context.dedup();
    row.inputs.sort_by(|a, b| a.locator.cmp(&b.locator));
    if let Some(reason) = &row.selection.issue {
        return Err(refused(reason));
    }
    if row.selection.company {
        let prefix = format!("{}:{}!", src.file_name, sheet.name);
        for (at, cell) in report.public_claims.cells.range(prefix.clone()..) {
            if !at.starts_with(&prefix) {
                break;
            }
            if matches!(cell.key.scope, Scope::Region { .. })
                && !row.selection.members.contains_key(&cell.key.subject)
            {
                return Err(refused(format!(
                    "printed AISC region is outside the declared company population: {} at {at}",
                    cell.key.subject
                )));
            }
        }
        for header in report
            .unregistered_headers
            .iter()
            .filter(|h| h.file == src.file_name && h.sheet == sheet.name)
        {
            if aisc::match_subject(&header.header, &["Total", "Total Gold"]).is_some() {
                continue;
            }
            let Some((_, col)) = fmine_io::xlsx::parse_a1_ref(&header.cell) else {
                return Err(refused("invalid unregistered AISC header locator"));
            };
            if table
                .block1
                .iter()
                .chain(&table.block2)
                .chain([&table.aisc_row, &table.ounces_row])
                .any(|r| {
                    sheet
                        .cell(*r, col)
                        .is_some_and(|c| matches!(c.value, CellValue::Number(_)))
                })
            {
                return Err(refused(format!(
                    "unregistered numeric AISC column cannot be omitted from a company comparison: {} at {prefix}{}",
                    header.header, header.cell
                )));
            }
        }
    }
    let title = aisc::cell_text(sheet, 1, 1)
        .unwrap_or_default()
        .to_lowercase();
    let units = aisc::cell_text(sheet, 3, 1)
        .unwrap_or_default()
        .to_lowercase();
    let sold = aisc::cell_text(sheet, table.ounces_row, 1)
        .unwrap_or_default()
        .to_lowercase();
    if row.selection.partial
        || !title.contains("all-in sustaining costs")
        || (row.selection.company && !title.contains("gold cost of sales"))
        || !aisc::is_usd_millions_label(&units)
        || !sold.contains("attributable basis")
        || !(sold.contains("000s ounces") || sold.contains("thousands of ounces"))
    {
        return Err(refused(
            "full-period membership and stated AISC, money and attributable thousand-ounce labels are required",
        ));
    }
    let mut selected = selected.into_iter();
    let aggregate = selected
        .next()
        .ok_or_else(|| refused("aggregate absent"))??;
    let members = selected.collect::<Result<Vec<_>>>()?;
    let table = Table {
        issuer: report.issuer.clone(),
        release: src.quarter.clone(),
        period: row.period.clone(),
        basis: format!(
            "same-table signed USD million components, including printed NCI lines, with no ownership conversion; {units}; {sold}"
        ),
        membership: row.selection.members.clone(),
        component_labels: row.labels.clone(),
        aggregate,
        members,
    };
    let mut anchors: Vec<_> = row
        .inputs
        .iter()
        .flat_map(|c| c.claim.anchors.iter().cloned())
        .collect();
    anchors.extend(row.context.iter().map(|(at, _)| {
        SourceAnchor::new(
            &src.workbook.artifact_path,
            src.workbook.sha256,
            at.strip_prefix(&format!("{}:", src.file_name))
                .unwrap_or(at),
        )
    }));
    let evidence = row
        .inputs
        .iter()
        .fold(EvidenceClass::Derived, |a, c| a.meet(c.claim.evidence));
    let epistemic = row
        .inputs
        .iter()
        .fold(EpistemicStatus::KnownKnown, |a, c| a.and(c.claim.epistemic));
    let state = row
        .inputs
        .iter()
        .fold(KnowledgeState::Known, |a, c| a.and(c.claim.state));
    let mut uu_terms = Vec::<UuTerm>::new();
    for term in row.inputs.iter().flat_map(|c| &c.claim.uu_terms) {
        if !uu_terms.iter().any(|u| u.scenario_id == term.scenario_id) {
            uu_terms.push(term.clone());
        }
    }
    let tag = table.canonical_tag();
    let input = Claim::new(
        kernel::INPUT_METRIC,
        table,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        evidence,
        epistemic,
        state,
        Confidentiality::public(),
        uu_terms,
        row.inputs.iter().map(|c| c.claim.claim_id).collect(),
        anchors,
    );
    kernel::compare(&input)
}

pub(super) fn assess(report: &PublicAssuranceReport) -> Report {
    let subjects: Vec<_> = report
        .public_claims
        .cells
        .values()
        .map(|c| c.key.subject.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut rows = Vec::new();
    for src in report
        .sources
        .iter()
        .filter(|s| s.kind == SourceKind::AiscReconciliation)
    {
        for sheet in &src.workbook.sheets {
            let (Ok(Some(table)), Ok(period)) = (
                aisc::locate_table(sheet, &subjects),
                parse_sheet_period(&sheet.name),
            ) else {
                continue;
            };
            let ((year, first), (_, last)) = period.months();
            let first = format!("{year}Q{}", first.div_ceil(3));
            let last = format!("{year}Q{}", last.div_ceil(3));
            let mut selections = selections(report, &src.quarter, &first, &last);
            let company_column = sheet.row_cells(table.header_row).iter().any(|c| {
                aisc::cell_text(sheet, table.header_row, c.col)
                    .is_some_and(|s| aisc::match_subject(&s, &["Total", "Total Gold"]).is_some())
            });
            if company_column
                || report
                    .mine_ledger
                    .cost_layers
                    .rows
                    .iter()
                    .any(|r| r.release == src.quarter && r.period == period)
            {
                selections.insert(
                    "Total Gold".into(),
                    company_selection(report, &src.quarter, &period.code(), &first, &last),
                );
            }
            for (subject, selection) in selections {
                let mut row = Assessment {
                    file: src.file_name.clone(),
                    sha256: Digest32(src.workbook.sha256).to_hex(),
                    release: src.quarter.clone(),
                    sheet: sheet.name.clone(),
                    period: period.code(),
                    subject,
                    selection,
                    inputs: Vec::new(),
                    context: Vec::new(),
                    labels: table
                        .block1
                        .iter()
                        .chain(&table.block2)
                        .map(|r| {
                            format!(
                                "A{r}: {}",
                                aisc::cell_text(sheet, *r, 1).unwrap_or_default()
                            )
                        })
                        .collect(),
                    result: Err(refused("not assessed")),
                };
                row.result = bind(report, src, sheet, &table, &mut row);
                rows.push(row);
            }
        }
    }
    rows.sort_by(|a, b| (&a.file, &a.sheet, &a.subject).cmp(&(&b.file, &b.sheet, &b.subject)));
    Report { rows }
}

impl Assessment {
    fn json(&self) -> String {
        let members = self
            .selection
            .members
            .iter()
            .map(|(subject, rule)| format!("{{\"subject\":{},\"rule\":{}}}", js(subject), js(rule)))
            .collect::<Vec<_>>()
            .join(",");
        let inputs = self
            .inputs
            .iter()
            .map(KeyedCell::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let context = self
            .context
            .iter()
            .map(|(at, text)| format!("{{\"at\":{},\"text\":{}}}", js(at), js(text)))
            .collect::<Vec<_>>()
            .join(",");
        let result = match &self.result {
            Err(e) => format!(
                "\"status\":\"unavailable\",\"refusal\":{{\"code\":{},\"what\":{},\"fix\":{}}}",
                js(e.code),
                js(&e.what),
                js(e.fix)
            ),
            Ok(claim) => {
                let c = &claim.value;
                let uu_terms = claim
                    .uu_terms
                    .iter()
                    .map(|u| format!(
                        "{{\"scenario_id\":{},\"description\":{},\"downside_exposure_usd_m\":{},\"trigger_indicator\":{}}}",
                        js(&u.scenario_id), js(&u.description),
                        js(&u.downside_exposure_usd_m.to_string()), js(&u.trigger_indicator)
                    ))
                    .collect::<Vec<_>>()
                    .join(",");
                let lines = self
                    .labels
                    .iter()
                    .zip(&c.components_usd_m)
                    .map(|(name, d)| {
                        format!(
                            "{{\"source_line\":{},\"difference_usd_m\":{}}}",
                            js(name),
                            difference(*d)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let footings = c
                    .column_footings_usd_m
                    .iter()
                    .map(|(subject, d)| {
                        format!(
                            "{{\"subject\":{},\"residual_usd_m\":{}}}",
                            js(subject),
                            difference(*d)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "\"status\":{},\"claim_id\":{},\"evidence\":{},\"knowledge\":{},\"uu_terms\":[{uu_terms}],\"printed_arithmetic_closed\":{},\"components\":[{lines}],\"summed_components_usd_m\":{},\"numerator_difference_usd_m\":{},\"sold_difference_koz\":{},\"unexplained_usd_m\":{},\"column_footings\":[{footings}],\"per_ounce\":{}",
                    js(if c.printed_arithmetic_closed {
                        "printed_arithmetic_closed"
                    } else {
                        "unresolved"
                    }),
                    js(&claim.claim_id.to_string()),
                    js(&format!("{:?}", claim.evidence)),
                    js(&format!("{:?}", claim.state)),
                    c.printed_arithmetic_closed,
                    difference(c.summed_components_usd_m),
                    difference(c.numerator_usd_m),
                    difference(c.sold_koz),
                    difference(c.unexplained_usd_m),
                    rates_json(c, &self.labels)
                )
            }
        };
        format!(
            "{{\"file\":{},\"sha256\":{},\"release\":{},\"sheet\":{},\"period\":{},\"aggregate\":{},\"comparison_scope\":{},\"members\":[{members}],\"partial_membership\":{},\"accounting_reconciliation_verified\":false,\"inputs\":[{inputs}],\"context\":[{context}],{result}}}",
            js(&self.file),
            js(&self.sha256),
            js(&self.release),
            js(&self.sheet),
            js(&self.period),
            js(&self.subject),
            js(if self.selection.company {
                "company_minus_declared_regions"
            } else if self.selection.region {
                "region_minus_declared_sites"
            } else {
                "subtotal_minus_declared_sites"
            }),
            self.selection.partial
        )
    }
}

impl Report {
    pub(super) fn to_json(&self) -> String {
        let rows = self
            .rows
            .iter()
            .map(Assessment::json)
            .collect::<Vec<_>>()
            .join(",");
        let body = format!(
            "\"schema\":\"fmine.aisc_component_comparisons.v1\",\"interpretation\":{},\"membership_scope\":\"Explicit retained region and subtotal policies; company comparisons use the same-release, same-period declared region population and do not establish complete company consolidation\",\"rows\":[{rows}]",
            js(kernel::NO_CLAIM)
        );
        format!(
            "{{\"digest\":{},{} }}",
            js(&blake3_hash(body.as_bytes()).to_hex()),
            body
        )
    }

    pub(super) fn markdown(&self) -> String {
        let mut out = format!(
            "\n## AISC component comparisons\n\n{}\n\nSelections use retained region and subtotal membership. Company comparisons use the same-release, same-period declared region population; they do not establish complete company consolidation.\n",
            kernel::NO_CLAIM
        );
        if self.rows.is_empty() {
            out.push_str(
                "\nNo applicable explicit member selection is available in this report.\n",
            );
        }
        for r in &self.rows {
            let _ = writeln!(
                out,
                "\n### {} / {}\n\nSource: {}; SHA-256 `{}`. Members: {}.\n",
                super::text(&r.subject),
                super::text(&r.period),
                super::text(&format!("{}:{}", r.file, r.sheet)),
                r.sha256,
                super::text(
                    &r.selection
                        .members
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            );
            if r.selection.company {
                out.push_str("Company-minus-declared-regions comparison. The population below is preserved even when its evidence is unavailable.\n");
                super::table(&mut out, &["Declared region", "Population evidence"]);
                for (member, policy) in &r.selection.members {
                    super::row(&mut out, &[member.clone(), policy.clone()]);
                }
            }
            match &r.result {
                Err(e) => {
                    let _ = writeln!(
                        out,
                        "Comparison unavailable: {}. All selected members remain in the population.\n",
                        super::text(&e.to_string())
                    );
                }
                Ok(c) => {
                    super::table(
                        &mut out,
                        &[
                            "Printed source component",
                            "Aggregate minus members (USD M)",
                            "Closed rounding bounds",
                        ],
                    );
                    for (label, d) in r.labels.iter().zip(&c.value.components_usd_m) {
                        super::row(
                            &mut out,
                            &[
                                label.clone(),
                                q(d.point),
                                format!("[{}, {}]", q(d.lo), q(d.hi)),
                            ],
                        );
                    }
                    for (label, d) in [
                        ("Total named differences", c.value.summed_components_usd_m),
                        (
                            "Reported AISC numerator difference",
                            c.value.numerator_usd_m,
                        ),
                        ("Unexplained numerator remainder", c.value.unexplained_usd_m),
                    ] {
                        super::row(
                            &mut out,
                            &[
                                label.into(),
                                q(d.point),
                                format!("[{}, {}]", q(d.lo), q(d.hi)),
                            ],
                        );
                    }
                    let d = c.value.sold_koz;
                    let _ = writeln!(
                        out,
                        "\nSold-quantity difference: {} koz, rounding bounds [{}, {}]. Printed arithmetic closes: {}. Accounting reconciliation verified: false.\n",
                        q(d.point),
                        q(d.lo),
                        q(d.hi),
                        c.value.printed_arithmetic_closed
                    );
                    match &c.value.rates {
                        Err(e) => {
                            let _ = writeln!(
                                out,
                                "\nPer-ounce comparison unavailable: {}. The dollar comparison above is retained.\n",
                                super::text(&e.to_string())
                            );
                        }
                        Ok(rates) => {
                            out.push_str("\n#### Recomputed per-ounce comparison\n\nRates use printed USD million numerators and attributable thousand ounces sold. They are distinct from the issuer's printed per-ounce rates. Component and unexplained numerator differences use aggregate sales; the denominator term separately retains any difference in sales totals. Bounds reflect input rounding, not confidence.\n\n");
                            let _ = writeln!(out, "Rule: `{}`.\n", kernel::RATE_RULES_VERSION);
                            super::table(
                                &mut out,
                                &["Quantity", "USD/oz", "Closed rounding bounds"],
                            );
                            for (label, d) in [
                                ("Recomputed aggregate rate", rates.aggregate_usd_per_oz),
                                ("Recomputed combined member rate", rates.members_usd_per_oz),
                                ("Aggregate minus combined member rate", rates.gap_usd_per_oz),
                            ] {
                                super::row(
                                    &mut out,
                                    &[
                                        label.into(),
                                        q(d.point),
                                        format!("[{}, {}]", q(d.lo), q(d.hi)),
                                    ],
                                );
                            }
                            out.push('\n');
                            super::table(
                                &mut out,
                                &["Gap component", "USD/oz", "Closed rounding bounds"],
                            );
                            for (label, d) in r.labels.iter().zip(&rates.components_usd_per_oz) {
                                super::row(
                                    &mut out,
                                    &[
                                        label.clone(),
                                        q(d.point),
                                        format!("[{}, {}]", q(d.lo), q(d.hi)),
                                    ],
                                );
                            }
                            for (label, d) in [
                                (
                                    "Unexplained numerator remainder",
                                    rates.unexplained_usd_per_oz,
                                ),
                                ("Sales denominator difference", rates.denominator_usd_per_oz),
                            ] {
                                super::row(
                                    &mut out,
                                    &[
                                        label.into(),
                                        q(d.point),
                                        format!("[{}, {}]", q(d.lo), q(d.hi)),
                                    ],
                                );
                            }
                            let _ = writeln!(
                                out,
                                "\nExact point closure verified: {}. Arithmetic closure does not identify corporate or JV allocations and does not resolve an unexplained remainder.\n",
                                rates.point_closure_verified
                            );
                        }
                    }
                    for (subject, d) in &c.value.column_footings_usd_m {
                        let _ = writeln!(
                            out,
                            "\n{} printed numerator minus its own components: {} USD M, rounding bounds [{}, {}].\n",
                            super::text(subject),
                            q(d.point),
                            q(d.lo),
                            q(d.hi)
                        );
                    }
                }
            }
        }
        out
    }
}
