//! Bounded PDLP guidance for exact production scheduling. The LP point only
//! supplies ranks and release periods; its objective never certifies the plan.
//! Determinism: DeterministicPerIsa

use fmine_evidence::{Refusal, RefusalCode};
use fmine_lp::pdlp::{self, LpProblem, PdlpOptions, PdlpStatus};
use fmine_optimize::toposort::pcpsp::{PcpspModel, lp, planning::GuidanceSearch};

pub(super) const DEFAULT_ITERATIONS: usize = 100_000;
pub(super) const MAX_ITERATIONS: usize = 1_000_000;
const MAX_COLUMNS: u128 = 1_000_000;
const MAX_ROWS: u128 = 2_000_000;
const MAX_ENTRIES: u128 = 8_000_000;
const TOLERANCE: f64 = 1e-6;

pub(super) struct Guidance {
    pub solution: lp::Solution,
    status: &'static str,
    iterations: u64,
    limit: usize,
    scoped_blocks: usize,
    signed_scope: bool,
    columns: usize,
    rows: usize,
    entries: usize,
}

fn refuse(detail: &str) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        format!("plan schedule LP guidance: {detail}"),
    )
}

/// Refuse oversized expansions before allocating the sparse relaxation. The
/// entry count includes repeated precedence entries, so it is conservative if
/// assembly combines duplicates. Full-model shares are bounded separately.
fn dimensions(model: &PcpspModel, blocks: &[usize]) -> Result<(), Refusal> {
    let (n, t, d) = (
        blocks.len() as u128,
        model.periods() as u128,
        model.destinations() as u128,
    );
    let arcs: u128 = blocks
        .iter()
        .map(|&b| model.requires(b).map_or(0, <[u32]>::len) as u128)
        .sum();
    let rows = arcs * t + n * (t - 1) + n * t + model.resources() as u128 * t;
    if n * t * (1 + d) > MAX_COLUMNS || model.blocks() as u128 * t > MAX_COLUMNS || rows > MAX_ROWS
    {
        return Err(refuse(
            "work budget exceeded: at most 1000000 columns/full-model block-period cells and 2000000 rows",
        ));
    }
    let mut resource_entries = 0_u128;
    for r in 0..model.resources() {
        for &b in blocks {
            for destination in 0..model.destinations() {
                resource_entries +=
                    u128::from(model.uses(r, b, destination).is_some_and(|q| q != 0));
            }
        }
    }
    let entries =
        2 * arcs * t + 2 * n * (t - 1) + n * (d + 1) + n * (t - 1) * (d + 2) + resource_entries * t;
    if entries > MAX_ENTRIES {
        return Err(refuse(
            "work budget exceeded: at most 8000000 sparse entries",
        ));
    }
    Ok(())
}

pub(super) fn solve(
    model: &PcpspModel,
    decimals: u32,
    iterations: usize,
) -> Result<Guidance, Refusal> {
    if !(1..=MAX_ITERATIONS).contains(&iterations) {
        return Err(refuse("iterations must be 1..=1000000"));
    }
    let scope = lp::scope(model)?;
    dimensions(model, &scope.blocks)?;
    let mut guidance = Guidance {
        solution: lp::Solution::new(model, &[], &[])?,
        status: "empty_scope",
        iterations: 0,
        limit: iterations,
        scoped_blocks: scope.blocks.len(),
        signed_scope: scope.whole,
        columns: 0,
        rows: 0,
        entries: 0,
    };
    if scope.blocks.is_empty() {
        return Ok(guidance);
    }
    let r = lp::relaxation(model, &scope.blocks, decimals)?;
    let problem = LpProblem::new(r.cost, &r.matrix, r.row_lo, r.row_hi, r.col_lo, r.col_hi)?;
    guidance.columns = problem.cols();
    guidance.rows = problem.rows();
    guidance.entries = problem.nnz();
    let solved = pdlp::solve(
        &problem,
        &PdlpOptions {
            tolerance: TOLERANCE,
            max_iterations: iterations as u64,
            threads: 1,
        },
    )?;
    guidance.status = match solved.status() {
        PdlpStatus::Optimal => "optimal",
        PdlpStatus::IterationLimit => "iteration_limit",
        PdlpStatus::Stopped => "stopped",
        PdlpStatus::Infeasible => {
            return Err(refuse(
                "LP solver reported infeasibility although the empty schedule satisfies every admitted row",
            ));
        }
    };
    guidance.iterations = solved.progress().iterations;
    guidance.solution = lp::Solution::new(model, &scope.blocks, solved.x())?;
    Ok(guidance)
}

impl Guidance {
    pub(super) fn json(&self, search: &GuidanceSearch, denominator: i128) -> String {
        format!(
            ",\"lp_guidance\":{{\"method\":\"pdlp_advisory_shares\",\"status\":\"{}\",\"iterations\":{},\"iteration_limit\":{},\"tolerance\":0.000001,\"threads\":1,\"scoped_blocks\":{},\"all_blocks_for_signed_resources\":{},\"columns\":{},\"rows\":{},\"nonzeros\":{},\"column_limit\":{MAX_COLUMNS},\"row_limit\":{MAX_ROWS},\"entry_limit\":{MAX_ENTRIES},\"starts\":{},\"searched_starts\":{},\"polished_seeds\":{},\"best_guided_value\":{},\"lp_objective_used_as_certificate\":false}}",
            self.status,
            self.iterations,
            self.limit,
            self.scoped_blocks,
            self.signed_scope,
            self.columns,
            self.rows,
            self.entries,
            search.starts,
            search.searched,
            search.polished,
            super::ratio(search.best_numerator, denominator)
        )
    }

    pub(super) fn markdown(&self, search: &GuidanceSearch) -> String {
        format!(
            "\nLP guidance: `{}` after {} of {} allowed iterations; {} generated starts, {} distinct searches and {} fractional seeds. LP shares are advisory, including at the iteration limit; every admitted schedule is checked exactly and the reported bound comes from full-model Lagrangian replay.\n",
            self.status,
            self.iterations,
            self.limit,
            search.starts,
            search.searched,
            search.polished
        )
    }
}
