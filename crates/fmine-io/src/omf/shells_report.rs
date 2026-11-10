//! DESIGN: source-bound nested OMF revenue-factor shells over the admitted pitflow pipeline (plan §15.4).
//! Determinism: DeterministicPerIsa. Certificates cover the scaled integer values and declared slope cones.

use fmine_core::{Digest32, anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_optimize::pitflow::shells::RevenueFactor;
use fmine_optimize::pitflow::slope::{SlopeTable, SlopeTangent};

use super::json::{self, Json};
use super::pit::{self, PitModel};
use super::pit_report::{escape_json, source_json};

pub const MAX_SPEC_BYTES: u64 = 1024 * 1024;
pub const MAX_OMF_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_FACTORS: usize = 64;
const ROUNDING: &str = "nearest_integer_half_away_from_zero";
pub const NO_CLAIM: &str = "Each nested shell maximizes the declared revenue-factor objective for the scaled integer parent-block values and uniform finite-depth slope cones. Revenue and cost are input attributes, not a price deck or calibrated economic model. No optimum for raw floating-point values, approved slope, reserve, designed pushback, schedule, operational mine plan or execution authorization is established. Tied shells are not claimed to be minimal or maximal; flow witnesses are verified internally and are not exported.";
pub const ROBOT_DOCS_JSON: &str = r#"{"command":"plan shells --omf <model.omf> --spec <shells.json> [--list-blocks]","schema":"fmine.omf-shells.v1","input":"Regular OMF parent blocks; exact OMF and strict JSON declaration bytes SHA-256 bound. All fields are required; duplicate and unknown keys refuse","spec":"block_model, revenue_attribute, cost_attribute, per_unit, value_rounding=nearest_integer_half_away_from_zero; slope{rise,run,levels}; factors[{num,den}]","scaling":"Revenue and cost must have the same nonempty unit label. No conversion. Integer attributes scale exactly; float attributes use binary64 multiplication then nearest integer, halves away from zero. Scaled revenue must be nonnegative; costs may have either sign","factors":"1..64 positive exact i64 numerator/denominator pairs, strictly increasing as written. No sorting, normalization or deduplication. At num/den the objective is sum(num*scaled_revenue-den*scaled_cost)/(per_unit*den)","method":"Existing PitModel::shells solves and verifies a maximum closure at every factor. Its running union supplies nested shells; each union's closure and exact objective are checked against that factor's certified optimum. Each factor is a fresh solve","work":"Spec<=1MiB; OMF<=1GiB. Before slope allocation, blocks*factor_count<=1000000. During reduced slope construction, (retained_arcs+blocks)*factor_count<=8000000 is enforced before target-buffer growth","output":"fmine.plan-shells.omf.v1: both sources, units/selectors/scales, slope, exact per-factor values, shell counts and added counts, verified nestedness. --list-blocks emits first_shell in OMF U/V/W bottom-up order; null means absent at every factor","limits":"Scaled integer objective only; no raw-float optimality bound, parametric warm start, exported flow witnesses, empirical calibration, schedule or reserve/execution claim"}"#;

/// Pure adapter result; the caller supplies its response envelope and performs file IO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub json: String,
    pub markdown: String,
}

struct Spec {
    block_model: String,
    revenue_attribute: String,
    cost_attribute: String,
    per_unit: u32,
    slope: [u32; 3],
    factors: Vec<RevenueFactor>,
}

fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("OMF shells declaration: {}", detail.into()),
    )
}

fn numerical(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        format!("OMF shells: {}", detail.into()),
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

fn parse(spec: &str) -> Result<Spec, Refusal> {
    let value = json::parse(spec.as_bytes()).map_err(invalid)?;
    object(
        &value,
        &[
            "schema",
            "block_model",
            "revenue_attribute",
            "cost_attribute",
            "per_unit",
            "value_rounding",
            "slope",
            "factors",
        ],
        "shells",
    )?;
    if text(&value, "schema")? != "fmine.omf-shells.v1" {
        return Err(invalid("schema must be fmine.omf-shells.v1"));
    }
    if text(&value, "value_rounding")? != ROUNDING {
        return Err(invalid(format!(
            "value_rounding must explicitly accept {ROUNDING}"
        )));
    }
    let slope = field(&value, "slope")?;
    object(slope, &["rise", "run", "levels"], "slope")?;
    let [rise, run, levels] = ["rise", "run", "levels"]
        .map(|key| integer(slope, key, 1, i64::from(u32::MAX)).map(|n| n as u32));
    let rows = field(&value, "factors")?
        .as_array()
        .filter(|f| (1..=MAX_FACTORS).contains(&f.len()))
        .ok_or_else(|| invalid(format!("factors must contain 1..={MAX_FACTORS} entries")))?;
    let factors = rows
        .iter()
        .map(|row| {
            object(row, &["num", "den"], "factor")?;
            RevenueFactor::new(
                integer(row, "num", i64::MIN, i64::MAX)?,
                integer(row, "den", i64::MIN, i64::MAX)?,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(pair) = factors.windows(2).find(|p| {
        i128::from(p[0].num()) * i128::from(p[1].den())
            >= i128::from(p[1].num()) * i128::from(p[0].den())
    }) {
        return Err(numerical(format!(
            "factors must strictly increase, but {}/{} is followed by {}/{}",
            pair[0].num(),
            pair[0].den(),
            pair[1].num(),
            pair[1].den()
        )));
    }
    Ok(Spec {
        block_model: text(&value, "block_model")?,
        revenue_attribute: text(&value, "revenue_attribute")?,
        cost_attribute: text(&value, "cost_attribute")?,
        per_unit: integer(&value, "per_unit", 1, i64::from(u32::MAX))? as u32,
        slope: [rise?, run?, levels?],
        factors,
    })
}

fn md(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\u{0060}', "&#96;")
        .replace('|', "&#124;")
        .replace(['\r', '\n'], " ")
}

/// Admit both byte sources and compose the existing exact nested-shell solver. Revenue and cost units are
/// checked before graph construction; scaled revenues must be nonnegative, while costs may have either sign.
///
/// # Errors
/// 505 for mismatched source hashes; 506 for malformed declarations or empty source paths; 107 for missing
/// attribute units; 105 for different units; 207 for work limits, invalid factors or failed certificates;
/// and existing OMF/PitModel refusal codes.
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
        if omf_bytes.len() as u64 > MAX_OMF_BYTES || spec_text.len() as u64 > MAX_SPEC_BYTES {
            return Err(invalid(
                "OMF must be at most 1 GiB and shells declaration at most 1 MiB",
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
                        "OMF shells source {} does not bind the supplied bytes; declared sha256={}, actual sha256={actual}",
                        source.artifact_path,
                        Digest32(source.artifact_sha256)
                    ),
                ));
            }
        }
        let spec = parse(spec_text)?;
        let project = super::read(omf_bytes)?;
        let revenue = pit::value_attribute(&project, &spec.block_model, &spec.revenue_attribute)?;
        let cost = pit::value_attribute(&project, &spec.block_model, &spec.cost_attribute)?;
        if revenue.units.trim().is_empty() || cost.units.trim().is_empty() {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "OMF shells revenue and cost attributes must declare nonempty units",
            ));
        }
        if revenue.units != cost.units {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                format!(
                    "OMF shells revenue units {:?} differ from cost units {:?}; no conversion is inferred",
                    revenue.units, cost.units
                ),
            ));
        }
        let max_arcs = pit::planning_work_limit(
            &project,
            &spec.block_model,
            spec.slope,
            spec.factors.len(),
            "shells",
            "factors",
        )?;
        let [rise, run, levels] = spec.slope;
        let slopes = SlopeTable::uniform(SlopeTangent::new(rise, run)?);
        let model =
            PitModel::new_bounded(&project, &spec.block_model, &slopes, None, levels, max_arcs)?;
        let nested = model.shells(
            &spec.revenue_attribute,
            &spec.cost_attribute,
            spec.per_unit,
            &spec.factors,
        )?;
        let mut previous = 0;
        let mut summaries = Vec::with_capacity(spec.factors.len());
        let mut rows = Vec::with_capacity(spec.factors.len());
        for (k, shell) in nested.shells().iter().enumerate() {
            let added = shell
                .blocks
                .checked_sub(previous)
                .ok_or_else(|| numerical("nested shell block counts decreased"))?;
            previous = shell.blocks;
            let (num, den) = (shell.factor.num(), shell.factor.den());
            let denominator = i128::from(spec.per_unit) * i128::from(den);
            summaries.push(format!(
                "{{\"factor\":{{\"num\":\"{num}\",\"den\":\"{den}\"}},\"scaled_value\":\"{}\",\"value\":{{\"numerator\":\"{}\",\"denominator\":\"{denominator}\"}},\"blocks\":{},\"solved_pit_blocks\":{},\"added_blocks\":{added}}}",
                shell.value, shell.value, shell.blocks, shell.solved_pit_blocks
            ));
            rows.push(format!(
                "| {k} | {num}/{den} | {} | {added} | {}/{denominator} |",
                shell.blocks, shell.value
            ));
        }
        let membership = if list_blocks {
            let first = model
                .omf_order(nested.first_shell())
                .iter()
                .map(|&k| {
                    if k == u32::MAX {
                        "null".into()
                    } else {
                        k.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(",\"first_shell\":[{first}]")
        } else {
            String::new()
        };
        let summaries = summaries.join(",");
        let json = format!(
            "{{\"schema\":\"fmine.plan-shells.omf.v1\",\"sources\":{{\"omf\":{},\"spec\":{}}},\"model\":{{\"name\":\"{}\",\"block_model\":\"{}\",\"revenue_attribute\":\"{}\",\"cost_attribute\":\"{}\",\"value_units\":\"{}\",\"per_unit\":{},\"value_rounding\":\"{ROUNDING}\",\"length_units\":\"{}\",\"coordinate_reference_system\":\"{}\",\"blocks\":{},\"arcs\":{}}},\"slope\":{{\"rise\":{rise},\"run\":{run},\"levels\":{levels}}},\"block_order\":\"omf_u_then_v_then_w_bottom_up\",\"shells\":[{summaries}],\"certificate\":{{\"kind\":\"integer_nested_closures\",\"verified\":true,\"nested\":true,\"flow_witnesses_exported\":false}}{membership}}}",
            source_json(omf_source, omf_bytes.len()),
            source_json(spec_source, spec_text.len()),
            escape_json(&project.name),
            escape_json(&spec.block_model),
            escape_json(&spec.revenue_attribute),
            escape_json(&spec.cost_attribute),
            escape_json(&revenue.units),
            spec.per_unit,
            escape_json(&project.units),
            escape_json(&project.coordinate_reference_system),
            model.grid().block_count(),
            model.arc_count()
        );
        let rows = rows.join("\n");
        let markdown = format!(
            "# Nested OMF pit shells\n\nBlock model **{}**. Revenue **{}**, cost **{}**; both in **{}** at {} integer units per attribute unit. Values round to nearest integer, halves away from zero.\n\n| shell (zero-based) | revenue factor | blocks | added blocks | exact objective |\n|---:|---:|---:|---:|---:|\n{rows}\n\nEvery shell is closed, optimal for its declared scaled integer objective, and contains the preceding shell. The existing solver verifies each flow internally and checks every union's closure and objective. Flow witnesses are not included in this report.\n\nUniform slope tangent {rise}/{run}, {levels} levels. OMF block IDs increase U, then V, then W from bottom upward. The first_shell array is included only with --list-blocks; null means the block is absent at every factor.\n\nOMF source SHA-256: {omf_digest}. Declaration SHA-256: {spec_digest}.\n\n{NO_CLAIM}\n",
            md(&spec.block_model),
            md(&spec.revenue_attribute),
            md(&spec.cost_attribute),
            md(&revenue.units),
            spec.per_unit
        );
        Ok(Report { json, markdown })
    };
    run().map_err(|mut refusal| {
        refusal.what.push_str(&format!(
            " [OMF source {}; sha256={omf_digest}; shells source {}; sha256={spec_digest}]",
            omf_source.artifact_path, spec_source.artifact_path
        ));
        refusal
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_sources_are_rechecked_and_refusals_retain_actual_hashes() {
        let (omf, spec) = (b"unparsed synthetic model".as_slice(), "{}");
        let original_model = SourceAnchor::new("model.omf", sha256(omf).0, "complete model");
        let original_spec = SourceAnchor::new(
            "shells.json",
            sha256(spec.as_bytes()).0,
            "complete declaration",
        );
        for model_changed in [true, false] {
            let (mut model_source, mut spec_source) =
                (original_model.clone(), original_spec.clone());
            if model_changed {
                model_source.artifact_sha256 = sha256(b"other model").0;
            } else {
                spec_source.artifact_sha256 = sha256(b"other spec").0;
            }
            let refusal = evaluate(omf, &model_source, spec, &spec_source, false).unwrap_err();
            assert_eq!(refusal.code, "FMINE-REF-505");
            assert!(refusal.what.contains(&sha256(omf).to_hex()));
            assert!(refusal.what.contains(&sha256(spec.as_bytes()).to_hex()));
            assert!(refusal.what.contains("model.omf") && refusal.what.contains("shells.json"));
        }
        let refusal = evaluate(omf, &original_model, spec, &original_spec, false).unwrap_err();
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(refusal.what.contains("missing schema"));
        assert!(refusal.what.contains(&sha256(omf).to_hex()));
        assert!(refusal.what.contains(&sha256(spec.as_bytes()).to_hex()));
    }
}
