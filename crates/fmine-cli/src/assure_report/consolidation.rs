//! DESIGN (BD-273): source binding for exact same-table AISC component comparisons.
//! Only explicit region/subtotal memberships retained by the assurance report select columns.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_claim::{
    AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport, UuTerm,
};
use fmine_core::{anchor::SourceAnchor, blake3_hash, hash::Digest32};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_io::xlsx::{CellValue, Sheet, col_index_to_letters};
use fmine_metrics::consolidation::{
    self as kernel, Column, Comparison, Difference, Printed, Table,
};
use fmine_public::aisc::{self, AiscTable, OperandRole, OperandValue};
use fmine_public::key::{KeyedCell, PublicValue, canonical_value_tag, parse_sheet_period};
use fmine_public::sources::SourceKind;
use fmine_public::{LoadedSource, PublicAssuranceReport};
use fmine_units::ExactQty as Rat;

type Result<T> = std::result::Result<T, Refusal>;

#[derive(Default)]
struct Selection {
    members: BTreeMap<String, String>,
    partial: bool,
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
            groups.entry(m.region.clone()).or_default().members.insert(
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

#[allow(clippy::too_many_arguments)]
fn reading(
    report: &PublicAssuranceReport,
    src: &LoadedSource,
    sheet: &Sheet,
    row: u32,
    col: u32,
    subject: &str,
    unit: &str,
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

fn column(
    report: &PublicAssuranceReport,
    src: &LoadedSource,
    sheet: &Sheet,
    table: &AiscTable,
    subject: &str,
    inputs: &mut Vec<KeyedCell>,
    context: &mut Vec<(String, String)>,
) -> Result<Column> {
    let headers: Vec<_> = sheet
        .row_cells(table.header_row)
        .into_iter()
        .filter(|h| {
            aisc::cell_text(sheet, table.header_row, h.col)
                .is_some_and(|s| aisc::match_subject(&s, &[subject]).is_some())
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
                row,
                header.col,
                subject,
                if row == table.ounces_row {
                    "koz"
                } else {
                    "usd_m"
                },
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
    let mut selected = Vec::new();
    selected.push(column(
        report,
        src,
        sheet,
        table,
        &row.subject,
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
    let title = aisc::cell_text(sheet, 1, 1)
        .unwrap_or_default()
        .to_lowercase();
    let units = aisc::cell_text(sheet, 3, 1)
        .unwrap_or_default()
        .to_lowercase();
    let sold = aisc::cell_text(sheet, table.ounces_row, 1)
        .unwrap_or_default()
        .to_lowercase();
    let dollar_unit = units.starts_with("($ millions")
        || units.starts_with("$ millions")
        || units.contains("us$ millions")
        || units.contains("usd millions");
    if row.selection.partial
        || !title.contains("all-in sustaining costs")
        || !dollar_unit
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
            for (subject, selection) in selections(report, &src.quarter, &first, &last) {
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
                    "\"status\":{},\"claim_id\":{},\"evidence\":{},\"knowledge\":{},\"uu_terms\":[{uu_terms}],\"printed_arithmetic_closed\":{},\"components\":[{lines}],\"summed_components_usd_m\":{},\"numerator_difference_usd_m\":{},\"sold_difference_koz\":{},\"unexplained_usd_m\":{},\"column_footings\":[{footings}]",
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
                    difference(c.unexplained_usd_m)
                )
            }
        };
        format!(
            "{{\"file\":{},\"sha256\":{},\"release\":{},\"sheet\":{},\"period\":{},\"aggregate\":{},\"members\":[{members}],\"partial_membership\":{},\"accounting_reconciliation_verified\":false,\"inputs\":[{inputs}],\"context\":[{context}],{result}}}",
            js(&self.file),
            js(&self.sha256),
            js(&self.release),
            js(&self.sheet),
            js(&self.period),
            js(&self.subject),
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
            "\"schema\":\"fmine.aisc_component_comparisons.v1\",\"no_claim\":{},\"membership_scope\":\"Explicit retained region and subtotal policies only; no inferred company membership\",\"rows\":[{rows}]",
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
            "\n## AISC component comparisons\n\n{}\n\nSelections use retained region and subtotal membership; no company membership is inferred.\n",
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
