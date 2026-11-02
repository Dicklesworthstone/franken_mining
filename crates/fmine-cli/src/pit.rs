//! `plan pit`: the ultimate pit of a MineLib UPIT pair, by pseudoflow, with its closure certificate checked again
//! here (bd-ip99, BD-250).
//! Determinism: Exact

use std::path::{Path, PathBuf};

use fmine_core::Digest32;
use fmine_core::anchor::SourceAnchor;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_optimize::pitflow::{self, minelib};

use crate::{Envelope, NextAction, escape_json, worksheet_input};

const MAX_BYTES: u64 = 1024 * 1024 * 1024;
pub const USAGE: &str =
    "plan pit --upit <file.upit> --prec <file.prec> [--decimals N] [--round] [--list-blocks]";
const NO_CLAIM: &str = "The maximum-value closed set of the supplied block values and precedence arcs. Values and \
arcs are taken as given: no price, cost, recovery or slope is derived or checked, and the pit is not a reserve, \
a schedule or a mine plan.";
pub const ROBOT_DOCS_JSON: &str = r#"{"input":"MineLib UPIT pair: <name>.upit (NAME, TYPE: UPIT, NBLOCKS, OBJECTIVE_FUNCTION, one '<block> <value>' line per block) and <name>.prec (one '<block> <count> <required>...' line per block); each file at most 1 GiB, SHA-256 bound","values":"Plain decimals scaled to integers at --decimals places (default 6, at most 9); a value with more non-zero places refuses unless --round, which rounds half away from zero and reports the count and the bound on the value","output":"Exact value at the declared places, pit and model sizes, the pseudoflow certificate re-verified (closed pit, no flow into it, excess signs), solver counters; --list-blocks adds the in-pit block ids","no_claim":"Values and arcs are taken as given; not a reserve, schedule or mine plan"}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub upit: PathBuf,
    pub prec: PathBuf,
    pub decimals: u32,
    pub round: bool,
    pub list_blocks: bool,
}

/// Common output/workspace options are removed by the main router.
pub fn parse(args: &[String]) -> Result<Command, String> {
    if args.first().map(String::as_str) != Some("plan")
        || args.get(1).map(String::as_str) != Some("pit")
    {
        return Err(USAGE.into());
    }
    let (mut upit, mut prec, mut decimals) = (None, None, None);
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
    Ok(Command {
        upit: upit.ok_or("plan pit requires --upit")?,
        prec: prec.ok_or("plan pit requires --prec")?,
        decimals: decimals.unwrap_or(6),
        round,
        list_blocks,
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

fn source_json(source: &SourceAnchor, bytes: usize) -> String {
    format!(
        "{{\"path\":\"{}\",\"sha256\":\"{}\",\"bytes\":{bytes}}}",
        escape_json(&source.artifact_path),
        Digest32(source.artifact_sha256)
    )
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (upit, upit_source) =
            worksheet_input::read(&root.join(&command.upit), "UPIT", MAX_BYTES)?;
        let (prec, prec_source) =
            worksheet_input::read(&root.join(&command.prec), "PREC", MAX_BYTES)?;
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
            source_json(&upit_source, upit.len()),
            source_json(&prec_source, prec.len()),
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
            "# Ultimate pit: {}\n\n| blocks | arcs | pit blocks | value | rounded values | certificate |\n|---:|---:|---:|---:|---:|---|\n| {} | {} | {} | {} | {} | verified |\n\n{NO_CLAIM}\n",
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
