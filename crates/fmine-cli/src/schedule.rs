//! Checked MineLib production schedules and global Lagrangian bounds (plan §15.4).
//! Determinism: DeterministicPerIsa
//! The model, candidate, scales and discount factors are explicit.

use crate::{Envelope, NextAction, escape_json, worksheet_input};
use fmine_evidence::Refusal;
use fmine_optimize::{
    pitflow::minelib::Rounding,
    toposort::{
        pcpsp::{self, candidate, planning},
        search::DISCOUNT_SCALE,
    },
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const USAGE: &str = "plan schedule --pcpsp <file.pcpsp> --prec <file.prec> [--candidate <file.sol>] [--decimals N] [--use-decimals N] [--share-decimals N] [--round] [--rounds N] [--bound-steps N] [--list-schedule]";
const NO_CLAIM: &str = "A checked schedule and upper bound for the supplied deterministic PCPSP model and explicit integer discount factors. Values, capacities and precedence are declarations; sampling error and input calibration are not measured. A positive gap may reflect a loose bound. No reserve, execution authority or operational feasibility beyond the declared rows is established.";
pub const ROBOT_DOCS_JSON: &str = r#"{"input":"MineLib PCPSP and PREC text; optional four-column block/destination/period/fraction solution; each input at most 1 GiB and SHA-256 bound","scales":"--decimals and --use-decimals default6; --share-decimals default9; all 0..9. Values refuse excess precision unless --round; resource uses round up and limits down; fractions never round","method":"TopoSort, period/destination local search, exact fractional polishing of initial and improved whole schedules using bounded pricing directions, exact candidate check, and a global Lagrangian upper bound replayed from its prices. Fractional polishing preserves mining periods and is a heuristic, not a destination-LP optimum. Whole-block candidates can warm-start search; split candidates retain their exact fractions","work":"--rounds 0..1000 default20; --bound-steps 1..4096 default64; fractional pricing at most256 columns and1024 master steps per period/seed","discount":"period0 factor2^40; each subsequent factor=floor(previous*rate_scale/(rate_scale+rate)); factors emitted","output":"Exact rational discounted values and bound gap; model rounding counts, resource uses, candidate infeasibility witnesses, certificate prices; --list-schedule emits canonical MineLib solution text","scope":"Only upper resource limits and no general side constraints; signed blend coefficients accepted. Structural candidate errors refuse; numerical infeasibility is reported and never admitted as an incumbent. No tolerance or claimed external bound is trusted","example":"tests/planning/fractional.pcpsp"}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub pcpsp: PathBuf,
    pub prec: PathBuf,
    pub candidate: Option<PathBuf>,
    pub decimals: u32,
    pub use_decimals: u32,
    pub options: planning::Options,
    pub round: bool,
    pub list_schedule: bool,
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    if args.first().map(String::as_str) != Some("plan")
        || args.get(1).map(String::as_str) != Some("schedule")
    {
        return Err(USAGE.into());
    }
    let mut fields = BTreeMap::new();
    let mut i = 2;
    while i < args.len() {
        let flag = args[i].as_str();
        if !matches!(
            flag,
            "--pcpsp"
                | "--prec"
                | "--candidate"
                | "--decimals"
                | "--use-decimals"
                | "--share-decimals"
                | "--rounds"
                | "--bound-steps"
                | "--round"
                | "--list-schedule"
        ) {
            return Err(format!("plan schedule: unsupported option {flag}; {USAGE}"));
        }
        let value = if matches!(flag, "--round" | "--list-schedule") {
            "true"
        } else {
            i += 1;
            args.get(i)
                .filter(|v| !v.trim().is_empty() && !v.starts_with("--"))
                .ok_or_else(|| format!("plan schedule: missing value for {flag}"))?
                .as_str()
        };
        if fields.insert(flag, value).is_some() {
            return Err(format!("plan schedule: duplicate {flag}"));
        }
        i += 1;
    }
    let count = |flag, default, lo, hi| -> Result<usize, String> {
        fields.get(flag).map_or(Ok(default), |v| {
            v.parse::<usize>()
                .ok()
                .filter(|n| (lo..=hi).contains(n))
                .ok_or_else(|| format!("plan schedule: {flag} must be {lo}..={hi}"))
        })
    };
    let path = |flag| {
        fields
            .get(flag)
            .map(PathBuf::from)
            .ok_or_else(|| format!("plan schedule requires {flag}"))
    };
    Ok(Command {
        pcpsp: path("--pcpsp")?,
        prec: path("--prec")?,
        candidate: fields.get("--candidate").map(PathBuf::from),
        decimals: count("--decimals", 6, 0, 9)? as u32,
        use_decimals: count("--use-decimals", 6, 0, 9)? as u32,
        options: planning::Options {
            rounds: count("--rounds", 20, 0, 1000)?,
            bound_steps: count("--bound-steps", 64, 1, 4096)?,
            share_decimals: count("--share-decimals", 9, 0, 9)? as u32,
        },
        round: fields.contains_key("--round"),
        list_schedule: fields.contains_key("--list-schedule"),
    })
}

fn integers<T: std::fmt::Display>(values: &[T]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|v| format!("\"{v}\""))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn pairs<A: std::fmt::Display, B: std::fmt::Display>(values: &[(A, B)]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|(a, b)| format!("[\"{a}\",\"{b}\"]"))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn matrix<T: std::fmt::Display>(rows: &[Vec<T>]) -> String {
    format!(
        "[{}]",
        rows.iter()
            .map(|row| integers(row))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn checked(c: &candidate::SplitCheck) -> String {
    format!(
        "{{\"feasible\":{},\"mined_blocks\":{},\"share_scale\":\"{}\",\"period_value_numerators\":{},\"resource_use_numerators\":{},\"incomplete_blocks\":{},\"first_incomplete\":{},\"unmet_requirements\":{},\"first_unmet\":{},\"over_limit\":{}}}",
        c.feasible(),
        c.mined,
        c.scale,
        integers(&c.period_values),
        matrix(&c.used),
        c.incomplete,
        pairs(&c.first_incomplete),
        c.unmet,
        pairs(&c.first_unmet),
        pairs(&c.over_limit)
    )
}
fn ratio(n: i128, d: i128) -> String {
    format!("{{\"numerator\":\"{n}\",\"denominator\":\"{d}\"}}")
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let read =
            |path: &Path, kind| worksheet_input::read(&root.join(path), kind, 1024 * 1024 * 1024);
        let (text, source) = read(&command.pcpsp, "PCPSP")?;
        let (prec, prec_source) = read(&command.prec, "PREC")?;
        let model = pcpsp::parse(
            &text,
            &prec,
            command.decimals,
            if command.round {
                Rounding::HalfAwayFromZero
            } else {
                Rounding::Refuse
            },
            command.use_decimals,
        )?;
        let outside = command
            .candidate
            .as_ref()
            .map(|path| {
                let (text, source) = read(path, "CANDIDATE")?;
                Ok::<_, Refusal>((
                    candidate::parse_solution(&text, &model, command.options.share_decimals)?,
                    worksheet_input::source_json(&source, text.len()),
                ))
            })
            .transpose()?;
        let plan = planning::solve(&model, command.options, outside.as_ref().map(|(s, _)| s))?;
        let denominator = 10_i128.pow(command.decimals)
            * i128::from(DISCOUNT_SCALE)
            * i128::from(plan.schedule.scale());
        let bound_denominator = 10_i128.pow(command.decimals) * i128::from(DISCOUNT_SCALE);
        let external = match (&plan.candidate, &outside) {
            (Some(c), Some((_, source))) => format!(
                "{{\"source\":{source},\"check\":{},\"discounted_value\":{}}}",
                checked(&c.checked),
                ratio(c.discounted_numerator, denominator)
            ),
            _ => "null".into(),
        };
        let solution = command
            .list_schedule
            .then(|| candidate::write_solution(&plan.schedule));
        let payload = format!(
            "{{\"schema\":\"fmine.plan-schedule.v1\",\"model\":{{\"format\":\"minelib_pcpsp\",\"name\":\"{}\",\"pcpsp\":{},\"prec\":{},\"blocks\":{},\"periods\":{},\"destinations\":{},\"resources\":{}}},\"value_decimals\":{},\"use_decimals\":{},\"share_decimals\":{},\"rounding\":{{\"values\":{},\"uses_up\":{},\"limits_down\":{}}},\"discount_factors\":{},\"discount_scale\":\"{DISCOUNT_SCALE}\",\"selected\":{{\"origin\":\"{}\",\"check\":{},\"discounted_value\":{},\"solution\":{}}},\"candidate\":{external},\"bound\":{{\"method\":\"lagrangian_full_model\",\"verified\":true,\"value\":{},\"prices\":{},\"steps\":{},\"gap\":{}}},\"search\":{{\"cycles\":{},\"round_limit\":{},\"bound_step_limit\":{}}}}}",
            escape_json(&model.name),
            worksheet_input::source_json(&source, text.len()),
            worksheet_input::source_json(&prec_source, prec.len()),
            model.blocks(),
            model.periods(),
            model.destinations(),
            model.resources(),
            command.decimals,
            command.use_decimals,
            command.options.share_decimals,
            model.rounded_values,
            model.rounded_uses,
            model.rounded_limits,
            integers(&plan.factors),
            plan.origin,
            checked(&plan.checked),
            ratio(plan.discounted_numerator, denominator),
            solution
                .as_ref()
                .map_or_else(|| "null".into(), |s| format!("\"{}\"", escape_json(s))),
            ratio(plan.bound.bound, bound_denominator),
            matrix(&plan.bound.prices),
            plan.bound.history.len(),
            ratio(plan.gap_numerator, denominator),
            plan.heuristic.cycles,
            command.options.rounds,
            command.options.bound_steps,
        );
        let mut envelope = Envelope::ok("plan schedule", payload, Vec::new());
        envelope.no_claim = Some(NO_CLAIM);
        if model.rounded_values + model.rounded_uses + model.rounded_limits > 0 {
            envelope.warnings.push("The certificate applies to the rounded model; resource uses round up and limits down, and all rounding counts are reported.".into());
        }
        let candidate_line = plan.candidate.as_ref().map_or_else(String::new, |c| format!("\nExternal candidate: feasible={}, incomplete blocks={}, unmet requirements={}, exceeded limits={}.\n", c.checked.feasible(), c.checked.incomplete, c.checked.unmet, c.checked.over_limit.len()));
        envelope.rendered_markdown = Some(format!(
            "# Production schedule: {}\n\nSelected `{}`: {} mined blocks. Feasibility and discounted value checked.\n\n| Discounted value | Verified upper bound | Gap to bound |\n|---|---|---|\n| {}/{} | {}/{} | {}/{} |\n\nValues are exact fractions in the input model's currency units.\n{candidate_line}\n{}\n\n{NO_CLAIM}\n",
            crate::guidance::text(&model.name),
            plan.origin,
            plan.checked.mined,
            plan.discounted_numerator,
            denominator,
            plan.bound.bound,
            bound_denominator,
            plan.gap_numerator,
            denominator,
            solution.map_or_else(String::new, |s| format!("```text\n{s}```"))
        ));
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "plan schedule",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect PCPSP inputs, exact scaling, candidate format and solver work limits",
            )],
        )
    })
}
