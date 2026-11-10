//! DESIGN: source-bound OMF ultimate-pit composition and rendering (BD-194/250).
//! The certificate applies only to the declared scaled integer model and finite slope cones.
//! Determinism: DeterministicPerIsa

use fmine_core::{Digest32, anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_optimize::pitflow::slope::{SlopeTable, SlopeTangent};

pub const MAX_BYTES: u64 = 1024 * 1024 * 1024;
pub const NO_CLAIM: &str = "The maximum-value closed set for the OMF adapter's scaled integer block values and the declared uniform finite-depth slope cones. The certificate covers that integer model only; no optimum for the unrounded attribute, geological validity, approved slope, reserve, schedule, mine plan or execution authorization is supplied.";
pub const WARNING: &str = "OMF float values use binary64 scaling before rounding; the exact certificate applies to the resulting integer model. Slope cones are a finite-depth approximation, not a geotechnical sign-off.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PitSpec {
    pub block_model: String,
    pub value_attribute: String,
    pub per_unit: u32,
    pub slope_rise: u32,
    pub slope_run: u32,
    pub slope_levels: u32,
}

/// Rendered evidence for the declared scaled integer model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub json: String,
    pub markdown: String,
}

pub(crate) fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

pub(crate) fn source_json(source: &SourceAnchor, bytes: usize) -> String {
    format!(
        "{{\"path\":\"{}\",\"sha256\":\"{}\",\"bytes\":{bytes}}}",
        escape_json(&source.artifact_path),
        Digest32(source.artifact_sha256)
    )
}

/// Read and solve the declared OMF parent-block model, retaining its exact source bytes.
///
/// # Errors
/// Refuses oversized or incorrectly bound source bytes, and all unsupported OMF model/value/slope inputs.
pub fn evaluate(
    bytes: &[u8],
    source: &SourceAnchor,
    input: &PitSpec,
    list_blocks: bool,
) -> Result<Report, Refusal> {
    if bytes.len() as u64 > MAX_BYTES || sha256(bytes).0 != source.artifact_sha256 {
        return Err(Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            "OMF source exceeds 1 GiB or its bytes do not match the supplied SHA-256",
        ));
    }
    let bind_refusal = |mut refusal: Refusal| {
        refusal.what.push_str(&format!(
            " [source {}; sha256={}]",
            source.artifact_path,
            fmine_core::Digest32(source.artifact_sha256)
        ));
        refusal
    };
    let project = super::read(bytes).map_err(&bind_refusal)?;
    let value_attribute =
        super::pit::value_attribute(&project, &input.block_model, &input.value_attribute)
            .map_err(&bind_refusal)?;
    let slopes = SlopeTable::uniform(
        SlopeTangent::new(input.slope_rise, input.slope_run).map_err(&bind_refusal)?,
    );
    let model = super::pit::PitModel::new(
        &project,
        &input.block_model,
        &slopes,
        None,
        input.slope_levels,
    )
    .map_err(&bind_refusal)?;
    // The admitted adapter constructs the problem; solve verifies its final flow certificate before return.
    let pit = model
        .ultimate_pit(&input.value_attribute, input.per_unit)
        .map_err(&bind_refusal)?;
    let stats = pit.stats();
    let mut payload = format!(
        "{{\"schema\":\"fmine.plan-pit.omf.v1\",\"model\":{{\"format\":\"omf2\",\"name\":\"{}\",\"source\":{},\"block_model\":\"{}\",\"value_attribute\":\"{}\",\"value_units\":\"{}\",\"coordinate_reference_system\":\"{}\",\"length_units\":\"{}\"}},\"blocks\":{},\"arcs\":{},\"slope\":{{\"kind\":\"uniform\",\"rise\":{},\"run\":{},\"levels\":{}}},\"rounding\":\"nearest_integer_half_away_from_zero\",\"per_unit\":{},\"scaled_value\":\"{}\",\"value\":\"{}/{}\",\"raw_attribute_optimality_bound\":null,\"pit_blocks\":{},\"block_order\":\"omf_u_then_v_then_w_bottom_up\",\"certificate\":{{\"verified\":true,\"method\":\"pseudoflow final arc flows; pit closed, no flow enters it, excess non-negative inside and non-positive outside\"}},\"solver\":{{\"mergers\":{},\"splits\":{},\"relabels\":{},\"max_label\":{}}}",
        escape_json(&project.name),
        source_json(source, bytes.len()),
        escape_json(&input.block_model),
        escape_json(&input.value_attribute),
        escape_json(&value_attribute.units),
        escape_json(&project.coordinate_reference_system),
        escape_json(&project.units),
        model.grid().block_count(),
        model.arc_count(),
        input.slope_rise,
        input.slope_run,
        input.slope_levels,
        input.per_unit,
        pit.value(),
        pit.value(),
        input.per_unit,
        pit.pit_size(),
        stats.mergers,
        stats.splits,
        stats.relabels,
        stats.max_label,
    );
    if list_blocks {
        let ids: Vec<String> = model
            .omf_order(pit.in_pit())
            .iter()
            .enumerate()
            .filter(|(_, inside)| **inside)
            .map(|(b, _)| b.to_string())
            .collect();
        payload.push_str(&format!(",\"in_pit\":[{}]", ids.join(",")));
    }
    payload.push('}');
    let markdown = format!(
        "# Ultimate pit: {}\n\nOMF block model `{}`, value attribute `{}`. Source SHA-256: `{}`.\n\n| blocks | arcs | pit blocks | exact scaled value | units per attribute unit | certificate |\n|---:|---:|---:|---:|---:|---|\n| {} | {} | {} | {} | {} | verified |\n\nUniform slope tangent {}/{}, {} levels. Listed block IDs use OMF order (U, V, then W from bottom upward).\n\n{NO_CLAIM}\n",
        project.name,
        input.block_model,
        input.value_attribute,
        fmine_core::Digest32(source.artifact_sha256),
        model.grid().block_count(),
        model.arc_count(),
        pit.pit_size(),
        pit.value(),
        input.per_unit,
        input.slope_rise,
        input.slope_run,
        input.slope_levels,
    );
    Ok(Report {
        json: payload,
        markdown,
    })
}
