//! DESIGN: compare an OMF design with a certified integer ultimate-pit shell (plan §15.5, bd-hq5y).
//! Determinism: DeterministicPerIsa
//! Exact integer enclosures use the existing volcert geometry assumptions.

use fmine_core::{Digest32, anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_geom::volcert::MAX_DEPTH;
use fmine_optimize::pitflow::slope::{SlopeTable, SlopeTangent};

use super::json::{self, Json};
use super::pit::{self, PitModel, ShellToDesign};
use super::pit_report::{escape_json, source_json};
use super::proportions::element;
use super::{Geometry, Grid3, Project};

const ROUNDING: &str = "nearest_integer_half_away_from_zero";
pub const NO_CLAIM: &str = "The shell is optimal only for the declared scaled integer values and finite-depth uniform slope cones. Design quantities assume each scaled parent-block total is uniformly distributed within that block. Geometric enclosures inherit volcert's nominal binary64 sub-cell boundaries and winding-number classification; they are not statistical confidence intervals. The design's slope feasibility is not checked, so a negative shell-minus-design value is possible. No raw-float optimum, mineable design, geotechnical sign-off, reserve, schedule, regulatory compliance or execution authorization is established.";
pub const ROBOT_DOCS_JSON: &str = r#"{"command":"plan design --omf <model.omf> --spec <design.json> [--list-blocks]","schema":"fmine.omf-design.v1","input":"Both OMF and strict JSON declaration bytes are SHA-256 bound; all fields required, duplicate/unknown keys refuse","spec":"block_model, value_attribute, value_per_unit, value_rounding=nearest_integer_half_away_from_zero, surface, depth, slope{rise,run,levels}, quantities[{attribute,per_unit}]","quantities":"Zero to 16 additional unique Number attributes on parent blocks, each with explicit nonempty units and positive integer scale. Values and quantities round nearest, halves away from zero; signed values remain signed. No density, grade, currency or mass conversion is inferred","method":"Solve and verify the integer ultimate-pit shell, then use PitModel::design and Design::against for exact shell, design, left, added and shell-minus-design quantity enclosures. Each scaled total is uniform within its parent block. Depth0..6; sub-cells per block=8^depth","work":"OMF<=1GiB; spec<=1MiB. Existing 1000000-block/8000000-graph-work limits apply. Before geometry allocation: at most100000 vertices and100000 triangles; blocks*(1+8^depth)<=2000000 and triangles*blocks*8^depth<=64000000. Reduce depth or split a refused model","output":"fmine.plan-design.omf.v1; exact rational endpoint strings, actual source hashes, integer objective certificate, geometric method limits. --list-blocks gives shell flags and [inside,inside+band] sub-cell counts in OMF U/V/W bottom-up order","limits":"Surface must be closed, outward-oriented and entirely within the block model. Volcert winding classification is checked near an integer but not proved correctly rounded. No design slope validation, raw-float optimum, reserve or execution claim"}"#;

/// Rendered evidence; file reading and response envelopes belong to the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub json: String,
    pub markdown: String,
}

struct Quantity {
    attribute: String,
    per_unit: u32,
}

struct Spec {
    block_model: String,
    value: Quantity,
    surface: String,
    depth: u8,
    slope: [u32; 3],
    quantities: Vec<Quantity>,
}

fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("OMF design declaration: {}", detail.into()),
    )
}

fn numerical(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        format!("OMF design: {}", detail.into()),
    )
}

fn object(value: &Json, keys: &[&str], at: &str) -> Result<(), Refusal> {
    let fields = value
        .as_object()
        .ok_or_else(|| invalid(format!("{at} must be an object")))?;
    if let Some((key, _)) = fields.iter().find(|(key, _)| !keys.contains(&key.as_str())) {
        return Err(invalid(format!("unknown {at} field {key:?}")));
    }
    Ok(())
}

fn field<'a>(value: &'a Json, key: &str) -> Result<&'a Json, Refusal> {
    value
        .get(key)
        .ok_or_else(|| invalid(format!("missing {key}")))
}

fn text(value: &Json, key: &str) -> Result<String, Refusal> {
    field(value, key)?
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{key} must be a nonempty string")))
}

fn integer(value: &Json, key: &str, lo: i64, hi: i64) -> Result<i64, Refusal> {
    field(value, key)?
        .i64()
        .filter(|n| (lo..=hi).contains(n))
        .ok_or_else(|| invalid(format!("{key} must be an integer in {lo}..={hi}")))
}

fn parse(input: &str) -> Result<Spec, Refusal> {
    let value = json::parse(input.as_bytes()).map_err(invalid)?;
    object(
        &value,
        &[
            "schema",
            "block_model",
            "value_attribute",
            "value_per_unit",
            "value_rounding",
            "surface",
            "depth",
            "slope",
            "quantities",
        ],
        "design",
    )?;
    if text(&value, "schema")? != "fmine.omf-design.v1" {
        return Err(invalid("schema must be fmine.omf-design.v1"));
    }
    if text(&value, "value_rounding")? != ROUNDING {
        return Err(invalid(format!(
            "value_rounding must explicitly accept {ROUNDING}"
        )));
    }
    let value_quantity = Quantity {
        attribute: text(&value, "value_attribute")?,
        per_unit: integer(&value, "value_per_unit", 1, i64::from(u32::MAX))? as u32,
    };
    let slope = field(&value, "slope")?;
    object(slope, &["rise", "run", "levels"], "slope")?;
    let [rise, run, levels] = ["rise", "run", "levels"]
        .map(|key| integer(slope, key, 1, i64::from(u32::MAX)).map(|n| n as u32));
    let rows = field(&value, "quantities")?
        .as_array()
        .filter(|rows| rows.len() <= 16)
        .ok_or_else(|| invalid("quantities must be an array of at most 16 entries"))?;
    let mut quantities: Vec<Quantity> = Vec::with_capacity(rows.len());
    for row in rows {
        object(row, &["attribute", "per_unit"], "quantity")?;
        let attribute = text(row, "attribute")?;
        if attribute == value_quantity.attribute
            || quantities.iter().any(|q| q.attribute == attribute)
        {
            return Err(invalid(format!(
                "duplicate quantity attribute {attribute:?}"
            )));
        }
        quantities.push(Quantity {
            attribute,
            per_unit: integer(row, "per_unit", 1, i64::from(u32::MAX))? as u32,
        });
    }
    Ok(Spec {
        block_model: text(&value, "block_model")?,
        value: value_quantity,
        surface: text(&value, "surface")?,
        depth: integer(&value, "depth", 0, i64::from(MAX_DEPTH))? as u8,
        slope: [rise?, run?, levels?],
        quantities,
    })
}

/// Bound the triangle binning, sub-cell labels and worst-case winding work before mesh/grid allocation.
fn geometry_work(project: &Project, spec: &Spec) -> Result<(), Refusal> {
    let Geometry::BlockModel(model) = &element(project, &spec.block_model)?.geometry else {
        return Err(invalid("block_model must select a BlockModel"));
    };
    let Grid3::Regular { count, .. } = model.grid else {
        return Err(invalid("block_model must have a regular grid"));
    };
    let Geometry::Surface {
        vertices,
        triangles,
        ..
    } = &element(project, &spec.surface)?.geometry
    else {
        return Err(invalid("surface must select a closed Surface"));
    };
    let blocks = count.into_iter().map(u128::from).product::<u128>();
    let cells = 8_u128.pow(u32::from(spec.depth));
    let labels = blocks.checked_mul(1 + cells);
    let work = blocks
        .checked_mul(cells)
        .and_then(|n| n.checked_mul(triangles.len() as u128));
    if vertices.len() > 100_000
        || triangles.len() > 100_000
        || labels.is_none_or(|n| n > 2_000_000)
        || work.is_none_or(|n| n > 64_000_000)
    {
        return Err(numerical(format!(
            "geometry work budget exceeded: {blocks} blocks, {} vertices, {} triangles, {cells} sub-cells/block; limits are 100000 vertices/triangles, 2000000 labels and 64000000 triangle-cell work units; reduce depth or split the model",
            vertices.len(),
            triangles.len()
        )));
    }
    Ok(())
}

fn md(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('`', "&#96;")
        .replace('|', "&#124;")
        .replace(['\r', '\n'], " ")
}

fn interval_json(bounds: [i128; 2], denominator: i128) -> String {
    format!(
        "{{\"lower\":\"{}\",\"upper\":\"{}\",\"denominator\":\"{denominator}\"}}",
        bounds[0], bounds[1]
    )
}

fn comparison(
    quantity: &Quantity,
    units: &str,
    cells: u32,
    result: ShellToDesign,
) -> (String, String) {
    let denominator = i128::from(quantity.per_unit) * i128::from(cells);
    let bounds = [
        ("shell", [result.shell, result.shell]),
        ("design", result.design),
        ("left", result.left),
        ("added", result.added),
        ("loss", result.loss),
    ];
    let fields = bounds
        .iter()
        .map(|(key, range)| format!("\"{key}\":{}", interval_json(*range, denominator)))
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        "{{\"attribute\":\"{}\",\"units\":\"{}\",\"per_unit\":{},\"uniform_within_parent_block\":true,{fields}}}",
        escape_json(&quantity.attribute),
        escape_json(units),
        quantity.per_unit
    );
    let ranges = bounds
        .iter()
        .map(|(_, range)| format!("[{}/{denominator}, {}/{denominator}]", range[0], range[1]))
        .collect::<Vec<_>>()
        .join(" | ");
    let markdown = format!("| {} ({}) | {ranges} |", md(&quantity.attribute), md(units));
    (json, markdown)
}

/// Certify the declared shell and compare a closed design surface with its exact block proportions.
///
/// # Errors
/// 505 for incorrect source hashes; 506 for malformed declarations; 107 for absent units; 207 for work
/// budgets or failed certificates, plus the existing OMF, slope and closed-surface refusals.
pub fn evaluate(
    omf_bytes: &[u8],
    omf_source: &SourceAnchor,
    spec_text: &str,
    spec_source: &SourceAnchor,
    list_blocks: bool,
) -> Result<Report, Refusal> {
    let omf_digest = sha256(omf_bytes);
    let spec_digest = sha256(spec_text.as_bytes());
    let run = || -> Result<Report, Refusal> {
        if omf_bytes.len() as u64 > 1024 * 1024 * 1024 || spec_text.len() as u64 > 1024 * 1024 {
            return Err(invalid(
                "OMF must be at most 1 GiB and design declaration at most 1 MiB",
            ));
        }
        for (source, actual) in [(omf_source, omf_digest), (spec_source, spec_digest)] {
            if source.artifact_path.trim().is_empty() {
                return Err(invalid("source path must be nonempty"));
            }
            if source.artifact_sha256 != actual.0 {
                return Err(Refusal::new(
                    RefusalCode::DigestMismatch505,
                    format!(
                        "OMF design source {} does not bind supplied bytes; declared sha256={}, actual sha256={actual}",
                        source.artifact_path,
                        Digest32(source.artifact_sha256)
                    ),
                ));
            }
        }
        let spec = parse(spec_text)?;
        let project = super::read(omf_bytes)?;
        let quantities = std::iter::once(&spec.value)
            .chain(&spec.quantities)
            .collect::<Vec<_>>();
        let attributes = quantities
            .iter()
            .map(|q| {
                let attr = pit::value_attribute(&project, &spec.block_model, &q.attribute)?;
                if attr.units.trim().is_empty() {
                    return Err(Refusal::new(
                        RefusalCode::UndeclaredUnitBasisOrTz107,
                        format!(
                            "OMF design attribute {:?} must declare nonempty units",
                            q.attribute
                        ),
                    ));
                }
                Ok(attr)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let max_arcs = pit::planning_work_limit(
            &project,
            &spec.block_model,
            spec.slope,
            1,
            "design",
            "shells",
        )?;
        geometry_work(&project, &spec)?;
        let [rise, run, levels] = spec.slope;
        let slopes = SlopeTable::uniform(SlopeTangent::new(rise, run)?);
        let model =
            PitModel::new_bounded(&project, &spec.block_model, &slopes, None, levels, max_arcs)?;
        let shell = model.ultimate_pit(&spec.value.attribute, spec.value.per_unit)?;
        let design = model.design(&spec.surface, spec.depth)?;
        let cells = design.cells_per_block();
        let mut comparisons = Vec::with_capacity(quantities.len());
        let mut rows = Vec::with_capacity(quantities.len());
        for (quantity, attribute) in quantities.iter().zip(attributes) {
            let weights = model.weights(&quantity.attribute, quantity.per_unit)?;
            let result = design.against(shell.in_pit(), &weights)?;
            let (json, row) = comparison(quantity, &attribute.units, cells, result);
            comparisons.push(json);
            rows.push(row);
        }
        let mut json = format!(
            "{{\"schema\":\"fmine.plan-design.omf.v1\",\"sources\":{{\"omf\":{},\"spec\":{}}},\"model\":{{\"block_model\":\"{}\",\"surface\":\"{}\",\"value_attribute\":\"{}\",\"value_per_unit\":{},\"coordinate_reference_system\":\"{}\",\"length_units\":\"{}\"}},\"blocks\":{},\"arcs\":{},\"shell_blocks\":{},\"slope\":{{\"rise\":{rise},\"run\":{run},\"levels\":{levels}}},\"rounding\":\"{ROUNDING}\",\"depth\":{},\"cells_per_block\":{cells},\"block_order\":\"omf_u_then_v_then_w_bottom_up\",\"certificate\":{{\"integer_shell_verified\":true,\"scaled_shell_value\":\"{}\",\"design_slopes_checked\":false,\"flow_witness_exported\":false,\"geometry_method\":\"volcert_nominal_subcells_checked_winding_labels\",\"statistical_confidence_interval\":false}},\"quantities\":[{}]",
            source_json(omf_source, omf_bytes.len()),
            source_json(spec_source, spec_text.len()),
            escape_json(&spec.block_model),
            escape_json(&spec.surface),
            escape_json(&spec.value.attribute),
            spec.value.per_unit,
            escape_json(&project.coordinate_reference_system),
            escape_json(&project.units),
            model.grid().block_count(),
            model.arc_count(),
            shell.pit_size(),
            spec.depth,
            shell.value(),
            comparisons.join(",")
        );
        if list_blocks {
            let flags = model
                .omf_order(shell.in_pit())
                .iter()
                .map(bool::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let proportions = (0..model.grid().block_count())
                .map(|block| {
                    design
                        .cells(block)
                        .ok_or_else(|| numerical("design omitted a parent block"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let proportions = model
                .omf_order(&proportions)
                .iter()
                .map(|[lo, hi]| format!("[{lo},{hi}]"))
                .collect::<Vec<_>>()
                .join(",");
            json.push_str(&format!(
                ",\"shell_membership\":[{flags}],\"design_cells\":[{proportions}]"
            ));
        }
        json.push('}');
        let markdown = format!(
            "# OMF shell-to-design comparison\n\nBlock model **{}**, design surface **{}**. Certified shell: {} blocks, scaled integer value {} at {} units per value unit.\n\n| Attribute (declared units) | Shell | Design | Shell left outside design | Design added outside shell | Shell minus design |\n|---|---|---|---|---|---|\n{}\n\nEach interval is exact in the displayed scaled quantity model. Depth {} uses {cells} sub-cells per parent block. Quantities are uniform within parent blocks; no density, grade, mass or currency conversion is inferred. A signed loss may be negative because the design's slopes are not checked.\n\nUniform shell slope tangent {rise}/{run}, {levels} levels. Optional membership and proportion arrays use OMF U/V/W bottom-up order.\n\nOMF SHA-256: {omf_digest}. Declaration SHA-256: {spec_digest}.\n\n{NO_CLAIM}\n",
            md(&spec.block_model),
            md(&spec.surface),
            shell.pit_size(),
            shell.value(),
            spec.value.per_unit,
            rows.join("\n"),
            spec.depth
        );
        Ok(Report { json, markdown })
    };
    run().map_err(|mut refusal| {
        refusal.what.push_str(&format!(
            " [OMF source {}; sha256={omf_digest}; design source {}; sha256={spec_digest}]",
            omf_source.artifact_path, spec_source.artifact_path
        ));
        refusal
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplied_hashes_cannot_substitute_for_either_actual_source() {
        let (bytes, spec) = (b"synthetic unparsed archive".as_slice(), "{}");
        let model_source = SourceAnchor::new("model.omf", sha256(bytes).0, "whole archive");
        let spec_source = SourceAnchor::new(
            "design.json",
            sha256(spec.as_bytes()).0,
            "whole declaration",
        );
        for altered_model in [false, true] {
            let (mut model, mut declaration) = (model_source.clone(), spec_source.clone());
            if altered_model {
                model.artifact_sha256 = sha256(b"different archive").0;
            } else {
                declaration.artifact_sha256 = sha256(b"different declaration").0;
            }
            let refusal = evaluate(bytes, &model, spec, &declaration, false).unwrap_err();
            assert_eq!(refusal.code, "FMINE-REF-505");
            assert!(refusal.what.contains(&sha256(bytes).to_hex()));
            assert!(refusal.what.contains(&sha256(spec.as_bytes()).to_hex()));
            assert!(refusal.what.contains("model.omf") && refusal.what.contains("design.json"));
        }
        let refusal = evaluate(bytes, &model_source, spec, &spec_source, false).unwrap_err();
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(refusal.what.contains("missing schema"));
    }
}
