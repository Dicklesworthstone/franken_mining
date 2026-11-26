//! Read-only arithmetic over a bounded, locally transcribed drill-release worksheet (BD-350).
//! Determinism: Exact

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use fmine_evidence::Refusal;
use fmine_explore::intercept::{
    DisclosureFlag, GradeUnit, InclusionRelation, Intercept, LengthUnit, PrintedQty,
    ReleaseDisclosureContext, RemainderResult, audit_drill_release_indexed,
};
use fmine_io::intercept::{
    CitedInterceptRow, ParseOutcome, intercept_from_cited_row, parse_intercept_phrase,
};
use fmine_registry::toml_subset::{TomlRow, parse_subset};
use fmine_units::ExactQty;

use super::investment::{Fields, fields, get, invalid, number};
use crate::{Envelope, json_string as js, worksheet_input};

pub(crate) const VERB: &str = "check drill-release";
pub(crate) const USAGE: &str = "check drill-release <worksheet.toml>";
const SCHEMA: &str = "fmine.drill-release-check.v1";
const NO_CLAIM: &str = "Exact arithmetic and review flags on a local transcription under its stated grade basis and rounding. Original document locators and disclosure context are supplied by the reader. Remainder grade is an inferred interval average. Hole-sequence gaps are review signals. Mineral classification, issuer authentication and regulatory approval require independent evidence.";
const MAX_BYTES: u64 = 65_536;
const MAX_ROWS: usize = 128;
const MAX_DEPTH: usize = 16;

type Result<T> = std::result::Result<T, Refusal>;

fn boolean(values: &Fields<'_>, key: &str) -> Result<bool> {
    match values.get(key).copied() {
        None | Some("false") => Ok(false),
        Some("true") => Ok(true),
        _ => Err(invalid(format!("{key} must be a quoted true or false"))),
    }
}

fn grade_unit(value: &str) -> Result<GradeUnit> {
    match value {
        "g/t" => Ok(GradeUnit::GramsPerTonne),
        "oz/st" => Ok(GradeUnit::OzPerShortTon),
        "%" => Ok(GradeUnit::Percent),
        _ => Err(invalid("grade_unit requires g/t, oz/st or %")),
    }
}

fn context(top: &Fields<'_>) -> Result<ReleaseDisclosureContext> {
    let composite_cutoff = match (top.get("cutoff"), top.get("cutoff_unit")) {
        (Some(value), Some(unit)) => Some((number(value)?, grade_unit(unit)?)),
        (None, None) => None,
        _ => return Err(invalid("cutoff requires a value and cutoff_unit together")),
    };
    let declared_hole_count = top
        .get("declared_holes")
        .map(|n| {
            n.parse::<u32>()
                .ok()
                .filter(|n| *n != 0)
                .ok_or_else(|| invalid("declared_holes requires a positive whole number"))
        })
        .transpose()?;
    Ok(ReleaseDisclosureContext {
        composite_cutoff,
        declared_hole_count,
        internal_dilution_stated: boolean(top, "internal_dilution_stated")?,
        top_cut_stated: boolean(top, "top_cut_stated")?,
        metal_eq_prices_stated: boolean(top, "metal_eq_prices_stated")?,
        metal_eq_recoveries_stated: boolean(top, "metal_eq_recoveries_stated")?,
    })
}

fn cited(values: &Fields<'_>) -> Result<Intercept> {
    let length_unit = match get(values, "length_unit")? {
        "m" => LengthUnit::Metres,
        "ft" => LengthUnit::Feet,
        _ => return Err(invalid("length_unit requires m or ft")),
    };
    intercept_from_cited_row(CitedInterceptRow {
        hole: get(values, "hole")?,
        from_token: values.get("from").copied(),
        to_token: values.get("to").copied(),
        length_token: get(values, "length")?,
        length_unit,
        grade_token: get(values, "grade")?,
        grade_unit: grade_unit(get(values, "grade_unit")?)?,
        metal_symbol: values.get("metal").copied(),
        is_metal_equivalent: false,
        true_width_stated: boolean(values, "true_width_stated")?,
        relation: if values.contains_key("parent") {
            InclusionRelation::Including
        } else {
            InclusionRelation::Host
        },
        children: Vec::new(),
    })
}

struct Reading<'a> {
    line: usize,
    locator: &'a str,
    mode: &'static str,
}

struct Row<'a> {
    node: Intercept,
    parent: Option<usize>,
    evidence: Vec<Reading<'a>>,
}

fn rows<'a>(input: &'a [TomlRow], metal: &str) -> Result<(Vec<Intercept>, Vec<Vec<Reading<'a>>>)> {
    let mut rows: Vec<Row<'a>> = Vec::with_capacity(input.len());
    let mut ids = BTreeMap::new();
    for row in input {
        let (mut node, values, mode) = match row.table.as_str() {
            "phrase" => {
                let values = fields(
                    &row.fields,
                    &["locator", "text"],
                    &["hole", "true_width_stated"],
                )?;
                let text = get(&values, "text")?;
                let node = match parse_intercept_phrase(text, values.get("hole").copied()) {
                    ParseOutcome::Parsed(node) => node,
                    ParseOutcome::FallbackToCitedMode { reason, .. } => {
                        return Err(invalid(format!(
                            "line {}: phrase needs cited mode. {reason}",
                            row.line
                        )));
                    }
                };
                (*node, values, "phrase")
            }
            "cited" => {
                let values = fields(
                    &row.fields,
                    &[
                        "id",
                        "locator",
                        "hole",
                        "length",
                        "length_unit",
                        "grade",
                        "grade_unit",
                    ],
                    &["parent", "from", "to", "metal", "true_width_stated"],
                )?;
                if ids.insert(get(&values, "id")?, rows.len()).is_some() {
                    return Err(invalid(format!(
                        "line {}: cited IDs must be unique",
                        row.line
                    )));
                }
                (cited(&values)?, values, "cited")
            }
            table => {
                return Err(invalid(format!(
                    "line {}: unsupported table {table}",
                    row.line
                )));
            }
        };
        // A substring in prose is insufficient to assert true width.
        declare_basis(&mut node, metal, boolean(&values, "true_width_stated")?)?;
        let parent = values
            .get("parent")
            .map(|id| {
                ids.get(id)
                    .copied()
                    .filter(|index| *index < rows.len())
                    .ok_or_else(|| {
                        invalid(format!(
                            "line {}: parent must be an earlier cited row",
                            row.line
                        ))
                    })
            })
            .transpose()?;
        rows.push(Row {
            node,
            parent,
            evidence: vec![Reading {
                line: row.line,
                locator: get(&values, "locator")?,
                mode,
            }],
        });
    }
    let (mut roots, mut evidence) = (Vec::new(), Vec::new());
    while let Some(row) = rows.pop() {
        if let Some(parent) = row.parent {
            let host = &mut rows[parent];
            if host.node.hole != row.node.hole {
                return Err(invalid(format!(
                    "cited parent and inclusion must share hole {}",
                    host.node.hole
                )));
            }
            host.node.children.push(row.node);
            host.evidence.extend(row.evidence);
        } else {
            check_geometry(&row.node, 1)?;
            roots.push(row.node);
            evidence.push(row.evidence);
        }
    }
    Ok((roots, evidence))
}

fn declare_basis(node: &mut Intercept, metal: &str, width: bool) -> Result<()> {
    if node
        .metal_symbol
        .as_ref()
        .is_some_and(|symbol| !symbol.eq_ignore_ascii_case(metal))
    {
        return Err(invalid(format!(
            "hole {}: printed metal must match worksheet grade_basis {metal}",
            node.hole
        )));
    }
    node.is_metal_equivalent = metal.ends_with("Eq");
    node.true_width_stated = width;
    for child in &mut node.children {
        declare_basis(child, metal, width)?;
    }
    Ok(())
}

fn check_geometry(node: &Intercept, depth: usize) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(invalid("intercept nesting exceeds 16 levels"));
    }
    if node.children.len() > 1
        && node
            .children
            .iter()
            .any(|child| child.from.is_none() || child.to.is_none())
    {
        return Err(invalid(format!(
            "hole {}: sibling inclusions need explicit from and to to check overlap",
            node.hole
        )));
    }
    for child in &node.children {
        if child.grade_unit.canonical() != node.grade_unit.canonical() {
            return Err(invalid(format!(
                "hole {}: incompatible grade units in parent and inclusion",
                node.hole
            )));
        }
        check_geometry(child, depth + 1)?;
    }
    Ok(())
}

fn exact(q: ExactQty) -> String {
    format!("{}/{}", q.num(), q.den())
}

fn printed(q: PrintedQty) -> String {
    format!(
        "{{\"value\":\"{}\",\"step\":\"{}\"}}",
        exact(q.value),
        exact(q.step)
    )
}

fn remainder(r: &RemainderResult) -> String {
    let nested = r
        .nested_results
        .iter()
        .map(remainder)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"length_m\":\"{}\",\"length_bounds_m\":[\"{}\",\"{}\"],\"grade\":\"{}\",\"grade_bounds\":[\"{}\",\"{}\"],\"nested\":[{nested}]}}",
        exact(r.remainder_length_m),
        exact(r.remainder_length_interval_m.lo),
        exact(r.remainder_length_interval_m.hi),
        exact(r.remainder_grade_point),
        exact(r.remainder_grade_interval.lo),
        exact(r.remainder_grade_interval.hi),
    )
}

fn flags(input: &[DisclosureFlag]) -> String {
    input.iter().map(|flag| {
        let details = match flag {
            DisclosureFlag::UnstatedCutoffOrDilution{missing_cutoff, missing_internal_dilution, missing_top_cut} =>
                format!(",\"missing_cutoff\":{missing_cutoff},\"missing_internal_dilution\":{missing_internal_dilution},\"missing_top_cut\":{missing_top_cut}"),
            DisclosureFlag::MetalEquivalentUnstatedTerms{missing_prices, missing_recoveries, ..} =>
                format!(",\"missing_prices\":{missing_prices},\"missing_recoveries\":{missing_recoveries}"),
            DisclosureFlag::HoleSequenceGap{declared_hole_count, observed_hole_count, missing_sequence_numbers} =>
                format!(",\"declared_holes\":{declared_hole_count},\"observed_holes\":{observed_hole_count},\"missing_numbers\":{missing_sequence_numbers:?}"),
            _ => String::new(),
        };
        format!("{{\"kind\":{}{details}}}", js(flag.kind_str()))
    }).collect::<Vec<_>>().join(",")
}

fn evaluate(path: &Path) -> Result<Envelope> {
    let (text, source) = worksheet_input::read(path, "drill-release worksheet", MAX_BYTES)?;
    let doc = parse_subset(&text).map_err(|error| invalid(error.to_string()))?;
    let top = fields(
        &doc.top,
        &["schema", "document", "evidence", "grade_basis"],
        &[
            "cutoff",
            "cutoff_unit",
            "declared_holes",
            "internal_dilution_stated",
            "top_cut_stated",
            "metal_eq_prices_stated",
            "metal_eq_recoveries_stated",
        ],
    )?;
    let evidence = get(&top, "evidence")?;
    let metal = get(&top, "grade_basis")?;
    if get(&top, "schema")? != SCHEMA || !matches!(evidence, "declared_public" | "synthetic") {
        return Err(invalid(format!(
            "{SCHEMA} requires evidence declared_public or synthetic"
        )));
    }
    if !matches!(
        metal.strip_suffix("Eq").unwrap_or(metal),
        "Au" | "Ag" | "Cu" | "Sn" | "Zn" | "Pb" | "Ni" | "Co" | "Mo" | "Pt" | "Pd"
    ) {
        return Err(invalid(
            "grade_basis requires a supported metal symbol or its Eq equivalent",
        ));
    }
    if doc.rows.is_empty() || doc.rows.len() > MAX_ROWS {
        return Err(invalid("provide 1–128 phrase or cited rows"));
    }
    let context = context(&top)?;
    let (trees, sources) = rows(&doc.rows, metal)?;
    if let Some((_, unit)) = context.composite_cutoff
        && trees
            .iter()
            .any(|tree| tree.grade_unit.canonical() != unit.canonical())
    {
        return Err(invalid(
            "cutoff_unit is incompatible with an intercept's grade unit",
        ));
    }
    let (audit, order) = audit_drill_release_indexed(&trees, &context)?;
    let document = get(&top, "document")?;
    let cell = worksheet_input::markdown_cell;
    let mut output = Vec::with_capacity(audit.intervals.len());
    let mut md = format!(
        "# Drill-release arithmetic\n\n{} · {} · {}\n\nWorksheet SHA-256 `{}`\n\n| Hole | Cited locations | Host length (m) | Host grade | Remainder grade and bounds | Review flags |\n|---|---|---:|---:|---|---|\n",
        cell(document),
        cell(evidence),
        cell(metal),
        fmine_core::Digest32(source.artifact_sha256)
    );
    for (item, index) in audit.intervals.iter().zip(order) {
        let locations = &sources[index];
        let anchors = locations
            .iter()
            .map(|loc| {
                format!(
                    "{{\"worksheet_line\":{},\"locator\":{},\"mode\":{}}}",
                    loc.line,
                    js(loc.locator),
                    js(loc.mode)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let value = item
            .remainder
            .as_ref()
            .map_or_else(|| "null".into(), remainder);
        let unit = item.canonical_grade_unit.as_str();
        output.push(format!(
            "{{\"hole\":{},\"length_m\":{},\"grade\":{},\"grade_unit\":{},\"remainder\":{value},\"flags\":[{}],\"sources\":[{anchors}]}}",
            js(&item.hole), printed(item.canonical_length_m), printed(item.canonical_grade), js(unit), flags(&item.flags)
        ));
        let visible = item.remainder.as_ref().map_or_else(
            || "—".into(),
            |r| {
                format!(
                    "{} [{} to {}] {unit}",
                    exact(r.remainder_grade_point),
                    exact(r.remainder_grade_interval.lo),
                    exact(r.remainder_grade_interval.hi)
                )
            },
        );
        let locs = locations
            .iter()
            .map(|loc| cell(loc.locator))
            .collect::<Vec<_>>()
            .join(", ");
        let kinds = item
            .flags
            .iter()
            .map(DisclosureFlag::kind_str)
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            md,
            "| {} | {locs} | {} | {} {unit} | {visible} | {kinds} |",
            cell(&item.hole),
            exact(item.canonical_length_m.value),
            exact(item.canonical_grade.value)
        );
    }
    for flag in &audit.release_flags {
        let _ = writeln!(md, "\nRelease flag `{}`", flag.kind_str());
    }
    let _ = writeln!(md, "\n{NO_CLAIM}");
    let payload = format!(
        "{{\"schema\":\"{SCHEMA}\",\"document\":{},\"evidence\":{},\"grade_basis\":{},\"worksheet\":{},\"arithmetic_digest\":\"{}\",\"intervals\":[{}],\"release_flags\":[{}],\"summary\":{}}}",
        js(document),
        js(evidence),
        js(metal),
        worksheet_input::source_json(&source, text.len()),
        audit.digest,
        output.join(","),
        flags(&audit.release_flags),
        audit.summary_ndjson_line(0),
    );
    let mut result = Envelope::ok(VERB, payload, Vec::new());
    result.no_claim = Some(NO_CLAIM);
    result.rendered_markdown = Some(md);
    Ok(result)
}

pub(crate) fn execute(path: &Path, root: &Path) -> Envelope {
    evaluate(&root.join(path))
        .unwrap_or_else(|refusal| Envelope::refused(VERB, refusal, Vec::new()))
}
