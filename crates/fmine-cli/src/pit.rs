//! `plan pit`: certified ultimate pits from MineLib pairs or regular OMF block models (BD-194/250).
//! MineLib candidates remain untrusted (bd-nq06); OMF slopes and value scaling are explicit declarations.
//! Determinism: DeterministicPerIsa (OMF value scaling); MineLib arithmetic is Exact.

use std::path::{Path, PathBuf};

use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::omf;
use fmine_optimize::pitflow::slope::{SlopeTable, SlopeTangent};
use fmine_optimize::pitflow::{self, candidate, minelib};

use crate::{Envelope, NextAction, escape_json, worksheet_input};

const MAX_BYTES: u64 = 1024 * 1024 * 1024;
pub const USAGE: &str = "plan pit --upit <file.upit> --prec <file.prec> [--decimals N] [--round] [--list-blocks] [--candidate <file>] | plan pit --omf <file.omf> --block-model <name> --value-attribute <name> --per-unit N --slope-rise N --slope-run N --slope-levels N --round [--list-blocks]";
const NO_CLAIM: &str = "The maximum-value closed set of the supplied block values and precedence arcs. Values and \
arcs are taken as given: no price, cost, recovery or slope is derived or checked, and the pit is not a reserve, \
a schedule or a mine plan.";
pub const ROBOT_DOCS_JSON: &str = r#"{"input":"MineLib UPIT pair: <name>.upit (NAME, TYPE: UPIT, NBLOCKS, OBJECTIVE_FUNCTION, one '<block> <value>' line per block) and <name>.prec (one '<block> <count> <required>...' line per block); each file at most 1 GiB, SHA-256 bound","values":"Plain decimals scaled to integers at --decimals places (default 6, at most 9); a value with more non-zero places refuses unless --round, which rounds half away from zero and reports the count and the bound on the value","output":"Exact value at the declared places, pit and model sizes, the pseudoflow certificate re-verified (closed pit, no flow into it, excess signs), solver counters; --list-blocks adds the in-pit block ids","candidate":"MineLib only: --candidate <file>, whitespace-separated block ids ('%' comments), SHA-256 bound; reports its exact value, unmet requirements and witnesses, certified gap when closed, and verdict not_closed, optimal or suboptimal; candidate claims are ignored","omf":{"command":"plan pit --omf <file.omf> --block-model <name> --value-attribute <name> --per-unit N --slope-rise N --slope-run N --slope-levels N --round [--list-blocks]","input":"OMF 2.x archive, regular file at most 1 GiB, exact bytes SHA-256 bound; one uniquely named regular parent-block model with W straight up and a unique Number attribute on Primitives","slopes":"Uniform declared slope tangent rise/run with positive integer parts; positive slope-levels sets the finite cone depth. No geotechnical slope approval is inferred","values":"Positive integer per-unit scale is mandatory. --round explicitly accepts the existing OMF adapter's nearest-integer scaling, halves away from zero. Certificate covers this scaled integer model only; raw-attribute optimum is not certified","block_order":"Listed block ids are OMF order: U fastest, then V, then W from bottom upward","output":"fmine.plan-pit.omf.v1: source, selectors, coordinate context, slope declaration, scale, exact scaled value and ratio, pit/arc counts and verified pseudoflow certificate","limits":"No sub-block values, tensor grid, sector/domain-specific slope table, candidate, schedule, price/cost derivation or reserve claim; MineLib-only flags refuse"},"no_claim":"Not a reserve, schedule or mine plan"}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    MineLib { upit: PathBuf, prec: PathBuf },
    Omf(OmfInput),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmfInput {
    pub path: PathBuf,
    pub block_model: String,
    pub value_attribute: String,
    pub per_unit: u32,
    pub slope_rise: u32,
    pub slope_run: u32,
    pub slope_levels: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub input: Input,
    pub decimals: u32,
    pub round: bool,
    pub list_blocks: bool,
    pub candidate: Option<PathBuf>,
}

/// Common output/workspace options are removed by the main router.
pub fn parse(args: &[String]) -> Result<Command, String> {
    if args.first().map(String::as_str) != Some("plan")
        || args.get(1).map(String::as_str) != Some("pit")
    {
        return Err(USAGE.into());
    }
    let (mut upit, mut prec, mut decimals, mut candidate) = (None, None, None, None);
    let (mut omf, mut block_model, mut value_attribute) = (None, None, None);
    let (mut per_unit, mut slope_rise, mut slope_run, mut slope_levels) = (None, None, None, None);
    let (mut round, mut list_blocks) = (false, false);
    let mut i = 2;
    while i < args.len() {
        let flag = args[i].as_str();
        if let Some(seen) = match flag {
            "--round" => Some(std::mem::replace(&mut round, true)),
            "--list-blocks" => Some(std::mem::replace(&mut list_blocks, true)),
            _ => None,
        } {
            if seen {
                return Err(format!("plan pit: duplicate {flag}"));
            }
            i += 1;
            continue;
        }
        let value = args
            .get(i + 1)
            .filter(|v| !v.trim().is_empty() && !v.starts_with("--"))
            .ok_or_else(|| format!("plan pit: missing value for {flag}; {USAGE}"))?;
        let fresh = match flag {
            "--upit" => upit.replace(PathBuf::from(value)).is_none(),
            "--prec" => prec.replace(PathBuf::from(value)).is_none(),
            "--candidate" => candidate.replace(PathBuf::from(value)).is_none(),
            "--omf" => omf.replace(PathBuf::from(value)).is_none(),
            "--block-model" => block_model.replace(value.clone()).is_none(),
            "--value-attribute" => value_attribute.replace(value.clone()).is_none(),
            "--per-unit" | "--slope-rise" | "--slope-run" | "--slope-levels" => {
                let number = value
                    .parse::<u32>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| format!("plan pit: {flag} must be a positive u32 integer"))?;
                match flag {
                    "--per-unit" => per_unit.replace(number).is_none(),
                    "--slope-rise" => slope_rise.replace(number).is_none(),
                    "--slope-run" => slope_run.replace(number).is_none(),
                    _ => slope_levels.replace(number).is_none(),
                }
            }
            "--decimals" => {
                let d = value
                    .parse::<u32>()
                    .ok()
                    .filter(|d| *d <= minelib::MAX_DECIMALS)
                    .ok_or_else(|| {
                        format!(
                            "plan pit: --decimals must be 0 to {}",
                            minelib::MAX_DECIMALS
                        )
                    })?;
                decimals.replace(d).is_none()
            }
            _ => return Err(format!("plan pit: unsupported option {flag}; {USAGE}")),
        };
        if !fresh {
            return Err(format!("plan pit: duplicate {flag}"));
        }
        i += 2;
    }
    let input = if let Some(path) = omf {
        if upit.is_some() || prec.is_some() || decimals.is_some() || candidate.is_some() {
            return Err(
                "plan pit: --omf cannot combine with --upit, --prec, --decimals or --candidate"
                    .into(),
            );
        }
        if !round {
            return Err(
                "plan pit: --omf requires --round to accept scaled integer block values".into(),
            );
        }
        Input::Omf(OmfInput {
            path,
            block_model: block_model.ok_or("plan pit --omf requires --block-model")?,
            value_attribute: value_attribute.ok_or("plan pit --omf requires --value-attribute")?,
            per_unit: per_unit.ok_or("plan pit --omf requires --per-unit")?,
            slope_rise: slope_rise.ok_or("plan pit --omf requires --slope-rise")?,
            slope_run: slope_run.ok_or("plan pit --omf requires --slope-run")?,
            slope_levels: slope_levels.ok_or("plan pit --omf requires --slope-levels")?,
        })
    } else {
        if block_model.is_some()
            || value_attribute.is_some()
            || per_unit.is_some()
            || slope_rise.is_some()
            || slope_run.is_some()
            || slope_levels.is_some()
        {
            return Err("plan pit: OMF model, value and slope options require --omf".into());
        }
        Input::MineLib {
            upit: upit.ok_or("plan pit requires --upit or --omf")?,
            prec: prec.ok_or("plan pit requires --prec")?,
        }
    };
    Ok(Command {
        input,
        decimals: decimals.unwrap_or(6),
        round,
        list_blocks,
        candidate,
    })
}

/// `units` at 10^`places` as an exact decimal.
fn decimal(units: i128, places: u32) -> String {
    let scale = 10_i128.pow(places);
    let (sign, abs) = if units < 0 {
        ("-", units.unsigned_abs())
    } else {
        ("", units.unsigned_abs())
    };
    let (int, frac) = (abs / scale.unsigned_abs(), abs % scale.unsigned_abs());
    if places == 0 {
        format!("{sign}{int}")
    } else {
        format!("{sign}{int}.{frac:0width$}", width = places as usize)
    }
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    match &command.input {
        Input::MineLib { upit, prec } => execute_minelib(command, root, upit, prec),
        Input::Omf(input) => execute_omf(input, command.list_blocks, root),
    }
}

fn execute_minelib(command: &Command, root: &Path, upit_path: &Path, prec_path: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (upit, upit_source) = worksheet_input::read(&root.join(upit_path), "UPIT", MAX_BYTES)?;
        let (prec, prec_source) = worksheet_input::read(&root.join(prec_path), "PREC", MAX_BYTES)?;
        let rounding = if command.round {
            minelib::Rounding::HalfAwayFromZero
        } else {
            minelib::Rounding::Refuse
        };
        let model = minelib::parse(&upit, &prec, command.decimals, rounding)?;
        let pit = pitflow::solve(&model.problem)?;
        // `solve` verifies before it returns; the certificate is checked once more against the parsed problem.
        if pitflow::verify(&model.problem, pit.in_pit(), pit.arc_flow())? != pit.value() {
            return Err(Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                "plan pit: the re-verified value differs from the solver's".to_string(),
            ));
        }
        let d = command.decimals;
        let stats = pit.stats();
        // Each rounding moves a pit's value by at most half a unit at `d` places: count × 5 at `d + 1` places.
        let bound = i128::try_from(model.rounded)
            .ok()
            .and_then(|r| r.checked_mul(5))
            .ok_or_else(|| {
                Refusal::new(
                    RefusalCode::NumericalCertificateFailed207,
                    "plan pit: the rounding bound overflows".to_string(),
                )
            })?;
        let mut payload = format!(
            "{{\"schema\":\"fmine.plan-pit.v1\",\"model\":{{\"format\":\"minelib_upit\",\"name\":\"{}\",\"upit\":{},\"prec\":{}}},\"blocks\":{},\"arcs\":{},\"decimals\":{d},\"rounding\":\"{}\",\"rounded_values\":{},\"value_bound\":\"{}\",\"value\":\"{}\",\"pit_blocks\":{},\"certificate\":{{\"verified\":true,\"method\":\"pseudoflow final arc flows; pit closed, no flow enters it, excess non-negative inside and non-positive outside\"}},\"solver\":{{\"mergers\":{},\"splits\":{},\"relabels\":{},\"max_label\":{}}}",
            escape_json(&model.name),
            worksheet_input::source_json(&upit_source, upit.len()),
            worksheet_input::source_json(&prec_source, prec.len()),
            model.problem.node_count(),
            model.problem.arc_count(),
            if command.round {
                "half_away_from_zero"
            } else {
                "refuse"
            },
            model.rounded,
            decimal(bound, d + 1),
            decimal(i128::from(pit.value()), d),
            pit.pit_size(),
            stats.mergers,
            stats.splits,
            stats.relabels,
            stats.max_label,
        );
        if command.list_blocks {
            let ids: Vec<String> = pit
                .in_pit()
                .iter()
                .enumerate()
                .filter(|(_, inside)| **inside)
                .map(|(b, _)| b.to_string())
                .collect();
            payload.push_str(",\"in_pit\":[");
            payload.push_str(&ids.join(","));
            payload.push(']');
        }
        let mut candidate_line = String::new();
        if let Some(path) = &command.candidate {
            let (text, source) = worksheet_input::read(&root.join(path), "CANDIDATE", MAX_BYTES)?;
            let in_set = candidate::parse_ids(&text, model.problem.node_count())?;
            let c = candidate::check(&model.problem, &in_set)?;
            let gap = i128::from(pit.value()) - i128::from(c.value);
            // A closed set worth more than the certified optimum would contradict the certificate.
            if c.closed() && gap < 0 {
                return Err(Refusal::new(
                    RefusalCode::NumericalCertificateFailed207,
                    "plan pit: a closed candidate exceeds the certified optimum".to_string(),
                ));
            }
            let verdict = match (c.closed(), gap) {
                (false, _) => "not_closed",
                (true, 0) => "optimal",
                (true, _) => "suboptimal",
            };
            let gap_text = if c.closed() {
                format!("\"{}\"", decimal(gap, d))
            } else {
                "null".to_string()
            };
            let witnesses: Vec<String> = c
                .first_unmet
                .iter()
                .map(|(b, p)| format!("[{b},{p}]"))
                .collect();
            payload.push_str(&format!(
                ",\"candidate\":{{\"source\":{},\"blocks\":{},\"value\":\"{}\",\"unmet_requirements\":{},\"first_unmet\":[{}],\"gap_to_optimum\":{gap_text},\"verdict\":\"{verdict}\"}}",
                worksheet_input::source_json(&source, text.len()),
                c.blocks,
                decimal(i128::from(c.value), d),
                c.unmet,
                witnesses.join(","),
            ));
            candidate_line = format!(
                "\n**Candidate** `{}`: {verdict}. {} blocks, value {}, {} unmet requirements, gap to the certified optimum {}.\n",
                path.display(),
                c.blocks,
                decimal(i128::from(c.value), d),
                c.unmet,
                gap_text.trim_matches('"'),
            );
        }
        payload.push('}');
        let mut envelope = Envelope::ok("plan pit", payload, Vec::new());
        envelope.no_claim = Some(NO_CLAIM);
        if model.rounded > 0 {
            envelope.warnings.push(format!(
                "{} values were rounded to {d} places; the value is exact for the rounded problem and within the stated bound of the file's optimum.",
                model.rounded
            ));
        }
        envelope.rendered_markdown = Some(format!(
            "# Ultimate pit: {}\n\n| blocks | arcs | pit blocks | value | rounded values | certificate |\n|---:|---:|---:|---:|---:|---|\n| {} | {} | {} | {} | {} | verified |\n{candidate_line}\n{NO_CLAIM}\n",
            model.name,
            model.problem.node_count(),
            model.problem.arc_count(),
            pit.pit_size(),
            decimal(i128::from(pit.value()), d),
            model.rounded
        ));
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "plan pit",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the MineLib UPIT input format, decimal places and rounding rule",
            )],
        )
    })
}

fn execute_omf(input: &OmfInput, list_blocks: bool, root: &Path) -> Envelope {
    const NO_CLAIM: &str = "The maximum-value closed set for the OMF adapter's scaled integer block values and the declared uniform finite-depth slope cones. The certificate covers that integer model only; no optimum for the unrounded attribute, geological validity, approved slope, reserve, schedule, mine plan or execution authorization is supplied.";
    let run = || -> Result<Envelope, Refusal> {
        let (bytes, source) =
            worksheet_input::read_bytes(&root.join(&input.path), "OMF", MAX_BYTES)?;
        let bind_refusal = |mut refusal: Refusal| {
            refusal.what.push_str(&format!(
                " [source {}; sha256={}]",
                source.artifact_path,
                fmine_core::Digest32(source.artifact_sha256)
            ));
            refusal
        };
        let project = omf::read(&bytes).map_err(&bind_refusal)?;
        let value_attribute =
            omf::pit::value_attribute(&project, &input.block_model, &input.value_attribute)
                .map_err(&bind_refusal)?;
        let slopes = SlopeTable::uniform(
            SlopeTangent::new(input.slope_rise, input.slope_run).map_err(&bind_refusal)?,
        );
        let model = omf::pit::PitModel::new(
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
            worksheet_input::source_json(&source, bytes.len()),
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
        let mut envelope = Envelope::ok("plan pit", payload, Vec::new());
        envelope.no_claim = Some(NO_CLAIM);
        envelope.warnings.push("OMF float values use binary64 scaling before rounding; the exact certificate applies to the resulting integer model. Slope cones are a finite-depth approximation, not a geotechnical sign-off.".into());
        envelope.rendered_markdown = Some(format!(
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
        ));
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "plan pit",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the supported OMF parent grid, value attribute and explicit slope/scale declarations",
            )],
        )
    })
}
