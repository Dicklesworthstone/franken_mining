//! Source-bound OMF scheduling declarations over the existing planning engines (plan §15.4).
//! Determinism: DeterministicPerIsa. The emitted integer discount factors define both objective and bound.

use fmine_core::{Digest32, anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_optimize::pitflow::slope::{SlopeTable, SlopeTangent};
use fmine_optimize::toposort::{ScheduleCheck, check, lp, search};

use super::json::{self, Json};
use super::pit::{self, Capacity, PitModel, ScheduleSpec};
use super::pit_report::{escape_json, source_json};
use super::{Geometry, Grid3, Project};

pub const MAX_SPEC_BYTES: u64 = 1024 * 1024;
pub const MAX_OMF_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_NODES: u128 = 1_000_000;
const MAX_ARCS: u128 = 8_000_000;
const ROUNDING: &str = "nearest_integer_half_away_from_zero";
pub const NO_CLAIM: &str = "A feasible schedule and re-verified upper bound for the declared scaled integer OMF model and emitted integer discount factors. Values, slope cones, capacities and rounding are declarations; the search is heuristic and sampling/calibration errors are not measured. No raw-attribute feasibility or optimality, approved slope, reserve, operational mine plan or execution authorization is established.";
pub const ROBOT_DOCS_JSON: &str = r#"{"command":"plan schedule --omf <model.omf> --spec <schedule.json> [--list-schedule]","schema":"fmine.omf-schedule.v1","input":"Regular OMF parent blocks; exact OMF and strict JSON declaration bytes SHA-256 bound. All spec fields are mandatory except candidate; duplicate and unknown keys refuse","spec":"block_model, value_attribute, value_per_unit, value_rounding=nearest_integer_half_away_from_zero; slope{rise,run,levels}; periods; discount{rate,scale}; capacities[{attribute,per_unit,limits_scaled}]; search_rounds; bound_steps; optional candidate{attribute,first}","scaling":"Positive integer value/resource scales. Values use binary64 multiplication then nearest integer, halves away from zero; resource uses round up. Resource limits are already-scaled nonnegative integers, one per period. No units are converted","method":"Ultimate-pit ranks seed TopoSort and local search. An optional OMF period attribute is checked as an untrusted candidate; null leaves a block out, first maps to period0. Only feasible candidates are improved or selected. The empty0 schedule remains an incumbent. Final feasibility and value are independently recomputed; Lagrangian prices are replayed against exactly the same integer discount factors","work":"Spec<=1MiB; OMF<=1GiB; periods1..256; capacities<=32; search_rounds0..1000; bound_steps1..256. Before slope allocation, conservative uniform-cone bounds require <=1000000 expanded nodes and <=8000000 expanded arcs","output":"fmine.plan-schedule.omf.v1: both sources, units/selectors/scales, declared capacity limits, exact period values/resource use, selected and candidate results, integer discount factors, same-objective bound/gap, replayed prices and bound history. --list-schedule emits OMF-order period arrays; first_unmet witnesses also use OMF order","limits":"Single destination; uniform finite-depth declared slopes; no LP guidance, price deck, raw-float objective certificate, empirical calibration or reserve/execution claim"}"#;

/// Pure adapter result; the caller supplies its response envelope and performs file IO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub json: String,
    pub markdown: String,
}

struct Resource {
    attribute: String,
    per_unit: u32,
    limits: Vec<i64>,
}

struct Candidate {
    attribute: String,
    first: i64,
}

struct Spec {
    block_model: String,
    value_attribute: String,
    value_per_unit: u32,
    slope: [u32; 3],
    periods: usize,
    discount: [i64; 2],
    capacities: Vec<Resource>,
    search_rounds: usize,
    bound_steps: usize,
    candidate: Option<Candidate>,
}

fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("OMF schedule declaration: {}", detail.into()),
    )
}

fn numerical(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        format!("OMF schedule: {}", detail.into()),
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
            "value_attribute",
            "value_per_unit",
            "value_rounding",
            "slope",
            "periods",
            "discount",
            "capacities",
            "search_rounds",
            "bound_steps",
            "candidate",
        ],
        "schedule",
    )?;
    if text(&value, "schema")? != "fmine.omf-schedule.v1" {
        return Err(invalid("schema must be fmine.omf-schedule.v1"));
    }
    if text(&value, "value_rounding")? != ROUNDING {
        return Err(invalid(format!(
            "value_rounding must explicitly accept {ROUNDING}"
        )));
    }
    let slope = field(&value, "slope")?;
    object(slope, &["rise", "run", "levels"], "slope")?;
    let slope = ["rise", "run", "levels"]
        .map(|key| integer(slope, key, 1, i64::from(u32::MAX)).map(|n| n as u32));
    let [rise, run, levels] = slope;
    let discount = field(&value, "discount")?;
    object(discount, &["rate", "scale"], "discount")?;
    let periods = integer(&value, "periods", 1, 256)? as usize;
    let rows = field(&value, "capacities")?
        .as_array()
        .ok_or_else(|| invalid("capacities must be an array"))?;
    if rows.len() > 32 {
        return Err(numerical("at most 32 capacity rows are supported"));
    }
    let capacities = rows
        .iter()
        .map(|row| {
            object(row, &["attribute", "per_unit", "limits_scaled"], "capacity")?;
            let limits = field(row, "limits_scaled")?
                .as_array()
                .filter(|a| a.len() == periods)
                .ok_or_else(|| invalid("limits_scaled must contain exactly one limit per period"))?
                .iter()
                .map(|n| {
                    n.i64().filter(|n| *n >= 0).ok_or_else(|| {
                        invalid("limits_scaled must contain nonnegative i64 integers")
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Resource {
                attribute: text(row, "attribute")?,
                per_unit: integer(row, "per_unit", 1, i64::from(u32::MAX))? as u32,
                limits,
            })
        })
        .collect::<Result<Vec<_>, Refusal>>()?;
    let candidate = value
        .get("candidate")
        .map(|candidate| -> Result<Candidate, Refusal> {
            object(candidate, &["attribute", "first"], "candidate")?;
            Ok(Candidate {
                attribute: text(candidate, "attribute")?,
                first: integer(candidate, "first", i64::MIN, i64::MAX)?,
            })
        })
        .transpose()?;
    Ok(Spec {
        block_model: text(&value, "block_model")?,
        value_attribute: text(&value, "value_attribute")?,
        value_per_unit: integer(&value, "value_per_unit", 1, i64::from(u32::MAX))? as u32,
        slope: [rise?, run?, levels?],
        periods,
        discount: [
            integer(discount, "rate", 0, i64::MAX)?,
            integer(discount, "scale", 1, i64::MAX)?,
        ],
        capacities,
        search_rounds: integer(&value, "search_rounds", 0, 1000)? as usize,
        bound_steps: integer(&value, "bound_steps", 1, 256)? as usize,
        candidate,
    })
}

/// Bound even the unreduced slope CSR before allocating it. The exact integer-size admission is shared with
/// PitModel; every cone lies in this rectangular offset box, including boundary blocks and truncated levels.
fn dimensions(project: &Project, spec: &Spec) -> Result<(), Refusal> {
    let e = super::proportions::element(project, &spec.block_model)?;
    let Geometry::BlockModel(model) = &e.geometry else {
        return Err(invalid("selected element is not a BlockModel"));
    };
    let Grid3::Regular { size, count } = &model.grid else {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "OMF schedule requires a regular grid",
        ));
    };
    let [nx, ny, nz] = count.map(u128::from);
    let n = nx * ny * nz;
    let periods = spec.periods as u128;
    if n == 0 || n * periods > MAX_NODES {
        return Err(numerical(format!(
            "work budget exceeded: {n} blocks times {periods} periods; at most {MAX_NODES} expanded nodes"
        )));
    }
    let (sizes, _) = pit::integer_sizes(*size).ok_or_else(|| {
        Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "OMF block sizes are not integers below 2^32 after permitted power-of-two scaling",
        )
    })?;
    let [dx, dy, dz] = sizes.map(u128::from);
    let levels = u128::from(spec.slope[2]).min(nz - 1);
    let horizontal = dz * levels * u128::from(spec.slope[1]) / u128::from(spec.slope[0]);
    let (rx, ry) = ((horizontal / dx).min(nx), (horizontal / dy).min(ny));
    let box_size = (2 * rx + 1) * (2 * ry + 1) * levels;
    let arcs = n * box_size * periods + n * periods;
    if arcs > MAX_ARCS {
        return Err(numerical(format!(
            "work budget exceeded: uniform-cone bound is {arcs} expanded arcs; at most {MAX_ARCS}; use fewer blocks, periods or levels, or steeper slopes"
        )));
    }
    Ok(())
}

fn integers<T: std::fmt::Display>(values: &[T]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|n| format!("\"{n}\""))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn matrix<T: std::fmt::Display>(values: &[Vec<T>]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|row| integers(row))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn ratio(numerator: i128, denominator: i128) -> String {
    format!("{{\"numerator\":\"{numerator}\",\"denominator\":\"{denominator}\"}}")
}

fn periods(values: &[Option<u32>]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|n| n.map_or_else(|| "null".into(), |n| n.to_string()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn checked(result: &ScheduleCheck, model: &PitModel<'_>) -> Result<String, Refusal> {
    let id = |b| {
        model
            .omf_index(b as usize)
            .ok_or_else(|| numerical("a schedule witness has an invalid block index"))
    };
    let unmet = result
        .first_unmet
        .iter()
        .map(|&(b, required)| Ok(format!("[{},{}]", id(b)?, id(required)?)))
        .collect::<Result<Vec<_>, Refusal>>()?
        .join(",");
    let over = result
        .over_limit
        .iter()
        .map(|(r, t)| format!("[{r},{t}]"))
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        "{{\"feasible\":{},\"mined_blocks\":{},\"period_values\":{},\"resource_used\":{},\"unmet_requirements\":{},\"first_unmet\":[{unmet}],\"over_limit\":[{over}]}}",
        result.feasible(),
        result.mined,
        integers(&result.period_values),
        matrix(&result.used),
        result.unmet
    ))
}

fn md(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('`', "&#96;")
        .replace('|', "&#124;")
        .replace(['\r', '\n'], " ")
}

/// Admit both byte sources, construct and check a single-destination period schedule, then independently replay
/// the Lagrangian prices on exactly the same integer objective. Numerically infeasible candidates are retained
/// as diagnostics and never become incumbents. Structural candidate errors refuse the whole operation.
///
/// # Errors
/// FMINE-REF-505 for mismatched source digests, 506 for malformed declarations or empty source paths,
/// 207 for work limits or failed final checks, and existing OMF/PitModel/TopoSort/Lagrangian refusal codes.
pub fn evaluate(
    omf_bytes: &[u8],
    omf_source: &SourceAnchor,
    spec_text: &str,
    spec_source: &SourceAnchor,
    list_schedule: bool,
) -> Result<Report, Refusal> {
    let omf_digest = sha256(omf_bytes);
    let spec_digest = sha256(spec_text.as_bytes());
    let run = || -> Result<Report, Refusal> {
        if omf_bytes.len() as u64 > MAX_OMF_BYTES || spec_text.len() as u64 > MAX_SPEC_BYTES {
            return Err(invalid(
                "OMF must be at most 1 GiB and schedule declaration at most 1 MiB",
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
                        "OMF schedule source {} does not bind the supplied bytes; declared sha256={}, actual sha256={actual}",
                        source.artifact_path,
                        Digest32(source.artifact_sha256)
                    ),
                ));
            }
        }
        let declaration = parse(spec_text)?;
        let project = super::read(omf_bytes)?;
        let value = pit::value_attribute(
            &project,
            &declaration.block_model,
            &declaration.value_attribute,
        )?;
        let resource_units = declaration
            .capacities
            .iter()
            .map(|c| {
                pit::value_attribute(&project, &declaration.block_model, &c.attribute)
                    .map(|a| a.units.clone())
            })
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(candidate) = &declaration.candidate {
            pit::value_attribute(&project, &declaration.block_model, &candidate.attribute)?;
        }
        dimensions(&project, &declaration)?;
        let [rise, run, levels] = declaration.slope;
        let slopes = SlopeTable::uniform(SlopeTangent::new(rise, run)?);
        let model = PitModel::new(&project, &declaration.block_model, &slopes, None, levels)?;
        let capacities = declaration
            .capacities
            .iter()
            .map(|c| Capacity {
                attribute: &c.attribute,
                per_unit: c.per_unit,
                limits: &c.limits,
            })
            .collect::<Vec<_>>();
        let spec = ScheduleSpec {
            value: &declaration.value_attribute,
            per_unit: declaration.value_per_unit,
            capacities: &capacities,
            periods: declaration.periods,
            discount: declaration.discount,
            max_rounds: declaration.search_rounds,
        };
        let pit = model.ultimate_pit(spec.value, spec.per_unit)?;
        let rank = model
            .omf_order(pit.in_pit())
            .iter()
            .map(|&inside| inside.then_some(0))
            .collect::<Vec<_>>();
        let baseline = model.schedule(&spec, &rank)?;
        let (problem, factors) = (baseline.problem(), baseline.factors());
        let denominator = i128::from(spec.per_unit) * i128::from(search::DISCOUNT_SCALE);
        let mut selected = vec![None; problem.blocks().node_count()];
        let (mut best, mut origin) = (0, "empty");
        if baseline.improvement().after > best {
            best = baseline.improvement().after;
            selected.clone_from(&baseline.improvement().schedule);
            origin = "pit_ranked";
        }
        let mut candidate_json = "null".to_string();
        if let Some(candidate) = &declaration.candidate {
            let raw = model.periods(&candidate.attribute, candidate.first)?;
            let assessment = model.candidate_schedule(&spec, &raw)?;
            let improvement = assessment.improvement();
            if let Some(improved) = improvement.filter(|i| i.after > best) {
                best = improved.after;
                selected.clone_from(&improved.schedule);
                origin = if improved.after > assessment.value() {
                    "candidate_improved"
                } else {
                    "candidate"
                };
            }
            candidate_json = format!(
                "{{\"attribute\":\"{}\",\"first\":{},\"checked\":{},\"discounted_value\":{},\"improved_value\":{}",
                escape_json(&candidate.attribute),
                candidate.first,
                checked(assessment.check(), &model)?,
                ratio(assessment.value(), denominator),
                improvement.map_or_else(|| "null".into(), |i| ratio(i.after, denominator))
            );
            if list_schedule {
                candidate_json.push_str(&format!(
                    ",\"periods\":{},\"improved_periods\":{}",
                    periods(&raw),
                    improvement
                        .map_or_else(|| "null".into(), |i| periods(&model.omf_order(&i.schedule)))
                ));
            }
            candidate_json.push('}');
        }
        let selected_check = check(problem, &selected)?;
        let objective = search::discounted_value(problem, &selected, factors)?;
        if !selected_check.feasible() || objective != best || best < 0 {
            return Err(numerical(
                "selected schedule failed independent feasibility/value checks",
            ));
        }
        let bound = lp::lagrangian_bound(problem, factors, best, declaration.bound_steps)?;
        let replay = lp::lagrangian_value(problem, factors, &bound.prices)?;
        if replay.bound != bound.bound || bound.bound < best {
            return Err(numerical(
                "Lagrangian price replay failed or bound lies below the incumbent",
            ));
        }
        let gap = bound
            .bound
            .checked_sub(best)
            .ok_or_else(|| numerical("bound gap overflows"))?;
        let selected_periods = if list_schedule {
            format!(",\"periods\":{}", periods(&model.omf_order(&selected)))
        } else {
            String::new()
        };
        let resources = declaration.capacities.iter().zip(&resource_units).map(|(c, units)| {
            format!("{{\"attribute\":\"{}\",\"units\":\"{}\",\"per_unit\":{},\"rounding\":\"ceiling\",\"limits_scaled\":{}}}",
                escape_json(&c.attribute), escape_json(units), c.per_unit, integers(&c.limits))
        }).collect::<Vec<_>>().join(",");
        let json = format!(
            "{{\"schema\":\"fmine.plan-schedule.omf.v1\",\"sources\":{{\"omf\":{},\"spec\":{}}},\"model\":{{\"name\":\"{}\",\"block_model\":\"{}\",\"value_attribute\":\"{}\",\"value_units\":\"{}\",\"value_per_unit\":{},\"value_rounding\":\"{ROUNDING}\",\"length_units\":\"{}\",\"coordinate_reference_system\":\"{}\",\"blocks\":{},\"arcs\":{}}},\"slope\":{{\"rise\":{rise},\"run\":{run},\"levels\":{levels}}},\"periods\":{},\"capacities\":[{resources}],\"search_rounds\":{},\"bound_steps\":{},\"discount\":{{\"rate\":{},\"scale\":{},\"factor_scale\":\"{}\",\"factors\":{}}},\"block_order\":\"omf_u_then_v_then_w_bottom_up\",\"selected\":{{\"origin\":\"{origin}\",\"checked\":{},\"discounted_value\":{}{selected_periods}}},\"candidate\":{candidate_json},\"certificate\":{{\"kind\":\"integer_lagrangian\",\"verified\":true,\"upper_bound\":{},\"gap\":{},\"prices\":{},\"history\":{},\"weight_shift\":{}}}}}",
            source_json(omf_source, omf_bytes.len()),
            source_json(spec_source, spec_text.len()),
            escape_json(&project.name),
            escape_json(&declaration.block_model),
            escape_json(&declaration.value_attribute),
            escape_json(&value.units),
            declaration.value_per_unit,
            escape_json(&project.units),
            escape_json(&project.coordinate_reference_system),
            model.grid().block_count(),
            model.arc_count(),
            declaration.periods,
            declaration.search_rounds,
            declaration.bound_steps,
            declaration.discount[0],
            declaration.discount[1],
            search::DISCOUNT_SCALE,
            integers(factors),
            checked(&selected_check, &model)?,
            ratio(best, denominator),
            ratio(bound.bound, denominator),
            ratio(gap, denominator),
            matrix(&bound.prices),
            integers(&bound.history),
            replay.shift,
        );
        let rows = selected_check
            .period_values
            .iter()
            .zip(factors)
            .enumerate()
            .map(|(t, (v, f))| format!("| {t} | {v} | {f} |"))
            .collect::<Vec<_>>()
            .join("\n");
        let resource_rows = declaration
            .capacities
            .iter()
            .zip(&resource_units)
            .enumerate()
            .map(|(r, (c, units))| {
                format!(
                    "| {} | {} | {} | {} | {} |",
                    md(&c.attribute),
                    md(units),
                    c.per_unit,
                    integers(&selected_check.used[r]),
                    integers(&c.limits)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let candidate_note = if declaration.candidate.is_some() {
            "The supplied candidate's original check and value, and any feasible local improvement, are retained in JSON."
        } else {
            "No external candidate was supplied."
        };
        let markdown = format!(
            "# Capacity-constrained OMF schedule\n\nModel `{}`, value attribute `{}` ({}). Selected `{origin}`: {} mined blocks.\n\nExact discounted value: **{best}/{denominator}**. Verified upper bound: **{}/{denominator}**; certified gap: **{gap}/{denominator}**. All three use the same emitted integer discount factors.\n\n| period (zero-based) | scaled value | discount factor |\n|---:|---:|---:|\n{rows}\n\nValues use {} scaled units per attribute unit; factor scale is {}.\n\n| capacity attribute | attribute units | scale | used by period | limits by period |\n|---|---|---:|---|---|\n{resource_rows}\n\nResource uses are rounded up; limits are declared in scaled units. Uniform slope tangent {rise}/{run}, {levels} levels. OMF block IDs increase U, then V, then W from bottom upward.\n\n{candidate_note}\n\nOMF source SHA-256: `{omf_digest}`. Declaration SHA-256: `{spec_digest}`.\n\n{NO_CLAIM}\n",
            md(&declaration.block_model),
            md(&declaration.value_attribute),
            md(&value.units),
            selected_check.mined,
            bound.bound,
            declaration.value_per_unit,
            search::DISCOUNT_SCALE,
        );
        Ok(Report { json, markdown })
    };
    run().map_err(|mut refusal| {
        refusal.what.push_str(&format!(
            " [OMF source {}; sha256={omf_digest}; schedule source {}; sha256={spec_digest}]",
            omf_source.artifact_path, spec_source.artifact_path
        ));
        refusal
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omf::{BlockModel, Element, Orient3};

    #[test]
    fn both_source_anchors_are_rechecked_and_refusals_bind_actual_bytes() {
        let (omf, spec) = (b"unparsed synthetic model".as_slice(), "{}");
        let model_source = SourceAnchor::new("model.omf", sha256(omf).0, "complete model");
        let spec_source = SourceAnchor::new(
            "schedule.json",
            sha256(spec.as_bytes()).0,
            "complete declaration",
        );
        for changed_model in [true, false] {
            let (mut model_anchor, mut spec_anchor) = (model_source.clone(), spec_source.clone());
            if changed_model {
                model_anchor.artifact_sha256 = sha256(b"different model").0;
            } else {
                spec_anchor.artifact_sha256 = sha256(b"different declaration").0;
            }
            let refusal = evaluate(omf, &model_anchor, spec, &spec_anchor, false).unwrap_err();
            assert_eq!(refusal.code, "FMINE-REF-505");
            assert!(refusal.what.contains("does not bind the supplied bytes"));
            assert!(refusal.what.contains(&sha256(omf).to_hex()));
            assert!(refusal.what.contains(&sha256(spec.as_bytes()).to_hex()));
        }
        let refusal = evaluate(omf, &model_source, spec, &spec_source, false).unwrap_err();
        assert!(refusal.what.contains("missing schema"));
        assert!(refusal.what.contains("model.omf") && refusal.what.contains("schedule.json"));
    }

    // Geometry metadata only: there are no per-block values, domains or arcs to allocate in this helper.
    fn project(count: [u32; 3]) -> Project {
        Project {
            format: "Open Mining Format 2.0".into(),
            name: "synthetic bounds".into(),
            description: String::new(),
            coordinate_reference_system: String::new(),
            units: "m".into(),
            origin: [0.0; 3],
            author: String::new(),
            application: String::new(),
            date: String::new(),
            metadata: vec![],
            elements: vec![Element {
                name: "section".into(),
                description: String::new(),
                color: None,
                metadata: vec![],
                attributes: vec![],
                geometry: Geometry::BlockModel(BlockModel {
                    orient: Orient3 {
                        origin: [0.0; 3],
                        u: [1.0, 0.0, 0.0],
                        v: [0.0, 1.0, 0.0],
                        w: [0.0, 0.0, 1.0],
                    },
                    grid: Grid3::Regular {
                        size: [12.5; 3],
                        count,
                    },
                    subblocks: None,
                }),
            }],
        }
    }

    #[test]
    fn work_bounds_refuse_nodes_and_broad_slope_arcs_before_model_allocation() {
        let mut spec = parse(
            r#"{
            "schema":"fmine.omf-schedule.v1","block_model":"section","value_attribute":"value",
            "value_per_unit":4,"value_rounding":"nearest_integer_half_away_from_zero",
            "slope":{"rise":1,"run":1,"levels":2},"periods":3,"discount":{"rate":1,"scale":10},
            "capacities":[],"search_rounds":20,"bound_steps":64
        }"#,
        )
        .unwrap();
        dimensions(&project([5, 1, 3]), &spec).unwrap();
        for count in [[1000, 1000, 1], [u32::MAX; 3]] {
            let refusal = dimensions(&project(count), &spec).unwrap_err();
            assert_eq!(refusal.code, "FMINE-REF-207");
            assert!(refusal.what.contains("expanded nodes"));
        }
        spec.slope = [1, u32::MAX, 1];
        let refusal = dimensions(&project([100, 100, 2]), &spec).unwrap_err();
        assert_eq!(refusal.code, "FMINE-REF-207");
        assert!(refusal.what.contains("expanded arcs"));
        // Levels beyond the top are unused; a one-layer model allocates no slope arcs.
        spec.slope[2] = u32::MAX;
        dimensions(&project([2, 1, 1]), &spec).unwrap();
    }
}
