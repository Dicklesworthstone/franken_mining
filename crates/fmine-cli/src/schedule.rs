//! Checked MineLib production schedules and global Lagrangian bounds (plan §15.4).
//! Determinism: DeterministicPerIsa
//! The model, candidate, scales and discount factors are explicit.

use crate::{
    Envelope, NextAction, escape_json,
    plan_certificate::{self, CertificateBasis},
    worksheet_input,
};
use fmine_evidence::Refusal;
use fmine_optimize::{
    pitflow::minelib::Rounding,
    plan_cert::certify_pcpsp_plan,
    toposort::{
        pcpsp::{self, candidate, planning},
        search::DISCOUNT_SCALE,
    },
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod guidance;
pub mod omf;

pub const USAGE: &str = "plan schedule --pcpsp <file.pcpsp> --prec <file.prec> [--candidate <file.sol>] [--decimals N] [--use-decimals N] [--share-decimals N] [--round] [--rounds N] [--bound-steps N] [--lp-guidance [--lp-iterations N]] [--list-schedule] [--certificate-currency CODE --certificate-horizon strategic|tactical|short_term]";
const NO_CLAIM: &str = "A checked schedule and upper bound for the supplied deterministic PCPSP model and explicit integer discount factors. Values, capacities and precedence are declarations; sampling error and input calibration are not measured. A positive gap may reflect a loose bound. No reserve, execution authority or operational feasibility beyond the declared rows is established.";
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{",
    r#""input":"MineLib PCPSP and PREC text; optional four-column block/destination/period/fraction solution; each input at most 1 GiB and SHA-256 bound","#,
    r#""scales":"--decimals and --use-decimals default6; --share-decimals default9; all 0..9. Values refuse excess precision unless --round; resource uses round up and limits down; fractions never round","#,
    r#""method":"TopoSort, period/destination local search with joint whole-route moves and alternative-route insertions, then exact fractional polishing of initial and improved whole schedules using bounded pricing directions. Fractional polishing preserves mining periods and is not a destination-LP optimum. The best split incumbent, including an imported candidate, receives single-block and atomic two-block common-period moves preserving every destination fraction and the mined set. All affected signed rows and external precedence must remain feasible; larger groups, different pair target periods and temporary loss/infeasibility are not searched. The subsequent joint fractional pass is described separately below. Final feasibility and value are independently checked, and the global full-model Lagrangian upper bound is replayed from its prices","#,
    r#""fractional_insertions":"For positive --rounds, search unmined predecessor-ready blocks even from an empty incumbent. Every period and destination pair is checked against exact signed resource rows on the declared share grid; only a complete feasible block with strictly positive discounted gain is inserted. Existing assignments are preserved. Nontrivial precedence cycles, loss-making prerequisite bundles, simultaneous block admission and three-or-more-route insertions are outside this bounded neighborhood","#,
    r#""joint_fractional_moves":"After insertion, for positive --rounds and a nonempty incumbent, search each mined block's period and every destination pair together, including same-period rerouting. Exact signed resource inequalities determine an inclusive interval of integer fractions on the declared decimal grid. Only strict exact gains are retained. Both vacated and receiving periods and both precedence directions remain feasible. The mined set and original candidate assessment are preserved; each moved block uses at most two destinations. Existing larger splits remain unless a tested pair improves them. No global optimality is inferred","#,
    r#""lp_guidance":"Opt-in --lp-guidance solves the relaxation with PDLP, fixed tolerance1e-6 and one worker. --lp-iterations1..1000000 default100000 requires --lp-guidance. Six threshold/release starts are searched and fractionally polished; duplicate starts/seeds are reused. Only a strict exact improvement replaces the baseline or external split. Iteration-limit shares are admitted as guidance after finite[0,1] checks; row feasibility and the LP objective are never trusted. Status optimal describes the LP stopping rule, not plan optimality. An empty LP scope skips the solver. Numerical infeasibility refuses because the empty schedule is feasible","#,
    r#""work":"--rounds0..1000 default20; --bound-steps1..4096 default64; fractional pricing at most256 columns and1024 master steps per period/seed. Fixed-fraction period search, exact block insertion and joint fractional period/route search each share --rounds and each have a separate262144 work-unit limit. Fixed-fraction pairs consider at most64 following mined blocks in ID order. Exhaustion retains every complete feasible improvement. Zero rounds disables both period searches, exact block insertion and joint rerouting. LP guidance adds at most6 whole searches and12 fractional passes, with preallocation limits1000000 columns/full-model block-period cells,2000000 rows and8000000 sparse entries","#,
    r#""discount":"period0 factor2^40; each subsequent factor=floor(previous*rate_scale/(rate_scale+rate)); factors emitted","#,
    r#""output":"Exact rational discounted values and bound gap; model rounding counts, resource uses, original candidate infeasibility witnesses and value, certificate prices; --list-schedule emits canonical MineLib solution text. fractional_period_search retains fixed-fraction seed/value/work counters including pair_moves. fractional_insertion_search retains its immediate seed, exact before/after values, inserted block count, trials and work budget. joint_period_route_search retains its immediate seed, exact before/after values, period/route move counts, trials and budget exhaustion. With guidance, lp_guidance reports termination, iterations, dimensions, start counts and best exact guided value; selected.origin identifies the winning path. The numerical certificate remains the full-model integer Lagrangian replay","#,
    r#""scope":"Only upper resource limits and no general side constraints; signed blend coefficients accepted. Structural candidate errors refuse; numerical infeasibility is reported and never admitted as an incumbent. No tolerance or claimed external bound is trusted","#,
    r#""plan_certificate":"Optional MineLib FMT-PCERT-1 export. Supply --certificate-currency CODE and --certificate-horizon strategic|tactical|short_term together. Inputs must have portable workspace-relative locators. JSON plan_certificate.bytes_hex is the canonical frame. The optimizer replays feasibility, discounted value and the bound; the offline reader checks frame and source integrity. Sampling and calibration are unmeasured","#,
    r#""example":"tests/planning/fractional.pcpsp","#,
    r#""joint_move_example":"tests/planning/joint_route.pcpsp","#,
    r#""joint_fractional_example":"tests/planning/joint_fractional.pcpsp with matching .prec and --candidate .sol","#,
    r#""split_only_example":"tests/planning/split_only.pcpsp with matching .prec; the profitable half-and-half block is constructed without an external candidate""#,
    "}"
);

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
    pub lp_iterations: Option<usize>,
    certificate: Option<CertificateBasis>,
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
                | "--lp-guidance"
                | "--lp-iterations"
                | "--certificate-currency"
                | "--certificate-horizon"
        ) {
            return Err(format!("plan schedule: unsupported option {flag}; {USAGE}"));
        }
        let value = if matches!(flag, "--round" | "--list-schedule" | "--lp-guidance") {
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
    if fields.contains_key("--lp-iterations") && !fields.contains_key("--lp-guidance") {
        return Err("plan schedule: --lp-iterations requires --lp-guidance".into());
    }
    let certificate = CertificateBasis::parse(
        "plan schedule",
        fields.get("--certificate-currency").copied(),
        fields.get("--certificate-horizon").copied(),
    )?;
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
        lp_iterations: fields
            .contains_key("--lp-guidance")
            .then(|| {
                count(
                    "--lp-iterations",
                    guidance::DEFAULT_ITERATIONS,
                    1,
                    guidance::MAX_ITERATIONS,
                )
            })
            .transpose()?,
        certificate,
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

fn fractional_period_report(plan: &planning::Plan, denominator: i128) -> (String, String) {
    let Some(s) = &plan.fractional_periods else {
        return (String::new(), String::new());
    };
    let work_limit = pcpsp::fractional::PERIOD_WORK;
    let json = format!(
        ",\"fractional_period_search\":{{\"seed_origin\":\"{}\",\"before\":{},\"after\":{},\"rounds\":{},\"moves\":{},\"pair_moves\":{},\"period_trials\":{},\"work\":{},\"work_limit\":{work_limit},\"work_exhausted\":{},\"destination_fractions_preserved\":true,\"mined_set_preserved\":true}}",
        plan.period_seed_origin,
        ratio(s.before, denominator),
        ratio(s.after, denominator),
        s.rounds,
        s.moves,
        s.pair_moves,
        s.trials,
        s.work,
        s.work_exhausted,
    );
    let markdown = format!(
        "\nFixed-fraction period search from `{}`: {} accepted moves ({} paired) over {} rounds and {} period trials, using {} of {work_limit} work units (budget exhausted: {}). Exact value {}/{denominator} → {}/{denominator}. Every destination fraction and the mined set are preserved; all affected periods' signed resource limits and precedence are checked. This bounded neighborhood supplies no additional optimality claim.\n",
        plan.period_seed_origin,
        s.moves,
        s.pair_moves,
        s.rounds,
        s.trials,
        s.work,
        s.work_exhausted,
        s.before,
        s.after,
    );
    (json, markdown)
}

fn fractional_insertion_report(plan: &planning::Plan, denominator: i128) -> (String, String) {
    let Some(s) = &plan.fractional_insertions else {
        return (String::new(), String::new());
    };
    let work_limit = pcpsp::fractional::joint::WORK;
    let json = format!(
        ",\"fractional_insertion_search\":{{\"seed_origin\":\"{}\",\"before\":{},\"after\":{},\"rounds\":{},\"insertions\":{},\"trials\":{},\"work\":{},\"work_limit\":{work_limit},\"work_exhausted\":{},\"existing_assignments_preserved\":true,\"maximum_destinations_per_inserted_block\":2}}",
        plan.insertion_seed_origin,
        ratio(s.before, denominator),
        ratio(s.after, denominator),
        s.rounds,
        s.insertions,
        s.trials,
        s.work,
        s.work_exhausted,
    );
    let markdown = format!(
        "\nExact block insertion from `{}`: {} complete blocks added over {} rounds and {} trials, using {} of {work_limit} work units (budget exhausted: {}). Exact value {}/{denominator} → {}/{denominator}. Existing assignments stay unchanged; every inserted block satisfies precedence and all signed resource limits using at most two destinations on the declared decimal grid. This bounded neighborhood does not jointly admit prerequisite bundles or certify a global optimum.\n",
        plan.insertion_seed_origin,
        s.insertions,
        s.rounds,
        s.trials,
        s.work,
        s.work_exhausted,
        s.before,
        s.after,
    );
    (json, markdown)
}

fn joint_period_route_report(plan: &planning::Plan, denominator: i128) -> (String, String) {
    let Some(s) = &plan.joint_period_routes else {
        return (String::new(), String::new());
    };
    let work_limit = pcpsp::fractional::joint::WORK;
    let json = format!(
        ",\"joint_period_route_search\":{{\"seed_origin\":\"{}\",\"before\":{},\"after\":{},\"rounds\":{},\"moves\":{},\"period_moves\":{},\"route_moves\":{},\"trials\":{},\"work\":{},\"work_limit\":{work_limit},\"work_exhausted\":{},\"mined_set_preserved\":true,\"maximum_destinations_per_moved_block\":2}}",
        plan.joint_seed_origin,
        ratio(s.before, denominator),
        ratio(s.after, denominator),
        s.rounds,
        s.moves,
        s.period_moves,
        s.route_moves,
        s.trials,
        s.work,
        s.work_exhausted,
    );
    let markdown = format!(
        "\nJoint period and route search from `{}`: {} accepted moves ({} changing period, {} changing routes) over {} rounds and {} trials, using {} of {work_limit} work units (budget exhausted: {}). Exact value {}/{denominator} → {}/{denominator}. Each moved block uses at most two destinations on the declared decimal grid. The mined set, precedence and every affected signed resource limit are preserved. This bounded neighborhood supplies no additional optimality claim.\n",
        plan.joint_seed_origin,
        s.moves,
        s.period_moves,
        s.route_moves,
        s.rounds,
        s.trials,
        s.work,
        s.work_exhausted,
        s.before,
        s.after,
    );
    (json, markdown)
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
                    source,
                    text.len(),
                ))
            })
            .transpose()?;
        let cert_meta = command
            .certificate
            .as_ref()
            .map(|basis| {
                basis.metadata(
                    &model.name,
                    "declared-pcpsp",
                    10_u128.pow(command.decimals),
                    root,
                    [("pcpsp", &source), ("precedence", &prec_source)]
                        .into_iter()
                        .chain(outside.as_ref().map(|(_, source, _)| ("candidate", source))),
                )
            })
            .transpose()?;
        let guided = command
            .lp_iterations
            .map(|iterations| guidance::solve(&model, command.decimals, iterations))
            .transpose()?;
        let external_schedule = outside.as_ref().map(|(s, _, _)| s);
        let plan = match &guided {
            Some(g) => planning::solve_with_guidance(
                &model,
                command.options,
                external_schedule,
                &g.solution,
            )?,
            None => planning::solve(&model, command.options, external_schedule)?,
        };
        let denominator = 10_i128.pow(command.decimals)
            * i128::from(DISCOUNT_SCALE)
            * i128::from(plan.schedule.scale());
        let bound_denominator = 10_i128.pow(command.decimals) * i128::from(DISCOUNT_SCALE);
        let external = match (&plan.candidate, &outside) {
            (Some(c), Some((_, source, bytes))) => format!(
                "{{\"source\":{},\"check\":{},\"discounted_value\":{}}}",
                worksheet_input::source_json(source, *bytes),
                checked(&c.checked),
                ratio(c.discounted_numerator, denominator)
            ),
            _ => "null".into(),
        };
        let solution = command
            .list_schedule
            .then(|| candidate::write_solution(&plan.schedule));
        let (lp_json, lp_markdown) = guided.as_ref().zip(plan.guidance.as_ref()).map_or_else(
            || (String::new(), String::new()),
            |(g, s)| (g.json(s, denominator), g.markdown(s)),
        );
        let (fractional_json, fractional_markdown) = fractional_period_report(&plan, denominator);
        let (insertion_json, insertion_markdown) = fractional_insertion_report(&plan, denominator);
        let (joint_json, joint_markdown) = joint_period_route_report(&plan, denominator);
        let (cert_json, cert_markdown) = if let Some(meta) = cert_meta {
            plan_certificate::render(&certify_pcpsp_plan(&meta, &model, &plan)?)?
        } else {
            (String::new(), String::new())
        };
        let payload = format!(
            "{{\"schema\":\"fmine.plan-schedule.v1\",\"model\":{{\"format\":\"minelib_pcpsp\",\"name\":\"{}\",\"pcpsp\":{},\"prec\":{},\"blocks\":{},\"periods\":{},\"destinations\":{},\"resources\":{}}},\"value_decimals\":{},\"use_decimals\":{},\"share_decimals\":{},\"rounding\":{{\"values\":{},\"uses_up\":{},\"limits_down\":{}}},\"discount_factors\":{},\"discount_scale\":\"{DISCOUNT_SCALE}\",\"selected\":{{\"origin\":\"{}\",\"check\":{},\"discounted_value\":{},\"solution\":{}}},\"candidate\":{external},\"bound\":{{\"method\":\"lagrangian_full_model\",\"verified\":true,\"value\":{},\"prices\":{},\"steps\":{},\"gap\":{}}},\"search\":{{\"cycles\":{},\"round_limit\":{},\"bound_step_limit\":{}}}{lp_json}{fractional_json}{insertion_json}{joint_json}{cert_json}}}",
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
            "# Production schedule: {}\n\nSelected `{}`: {} mined blocks. Feasibility and discounted value checked.\n\n| Discounted value | Verified upper bound | Gap to bound |\n|---|---|---|\n| {}/{} | {}/{} | {}/{} |\n\nValues are exact fractions in the input model's currency units.\n{candidate_line}{lp_markdown}{fractional_markdown}{insertion_markdown}{joint_markdown}{cert_markdown}\n{}\n\n{NO_CLAIM}\n",
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
