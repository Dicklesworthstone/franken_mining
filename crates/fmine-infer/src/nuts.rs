//! `BitwisePortable` No-U-Turn Sampler (Hoffman & Gelman 2014 + Betancourt 2017 multinomial
//! trajectory sampling) with dual-averaging step-size adaptation, Stan 3-stage windowed diagonal
//! mass-matrix adaptation, and deterministic `Philox4x32-10` RNG streams (`BD-232`, `BD-020`).
//! Determinism: BitwisePortable

use fmine_core::det;
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::{Refusal, RefusalCode};

use crate::ad::{LogDensity, log_add_exp};
use crate::diagnostics::{
    ChainRuntimeStats, DiagnosticThresholds, RhatEssCertificate, certify_chains,
};

/// Configuration for multi-chain NUTS sampling (`BD-232`).
#[derive(Debug, Clone, PartialEq)]
pub struct NutsConfig {
    /// Root seed folded into canonical `(seed_root, partition, domain, var_group, chain_id, 0)` `RngKey`s.
    pub seed_root: u64,
    /// Number of warmup (adaptation) iterations per chain.
    pub num_warmup: usize,
    /// Number of post-warmup draws retained per chain.
    pub num_draws: usize,
    /// Target Metropolis acceptance probability for dual averaging (default `0.80`).
    pub target_accept: f64,
    /// Maximum binary tree depth per transition (`2^max_treedepth` max leapfrog steps, default `10`).
    pub max_treedepth: usize,
    /// Hamiltonian energy jump threshold for declaring a divergence (default `1000.0`, Stan).
    pub max_delta_h: f64,
    /// Optional fixed step size; when `Some(eps)`, disables step-size adaptation (useful for planted-negative tests).
    pub fixed_step_size: Option<f64>,
    /// Whether to adapt the diagonal mass matrix during warmup (default `true`).
    pub adapt_mass_matrix: bool,
    /// Publication gate thresholds for the returned [`RhatEssCertificate`].
    pub thresholds: DiagnosticThresholds,
}

impl Default for NutsConfig {
    fn default() -> Self {
        Self {
            seed_root: 0x464D_494E_455F_4E55,
            num_warmup: 500,
            num_draws: 500,
            target_accept: 0.80,
            max_treedepth: 10,
            max_delta_h: 1000.0,
            fixed_step_size: None,
            adapt_mass_matrix: true,
            thresholds: DiagnosticThresholds::default(),
        }
    }
}

impl NutsConfig {
    /// Validate sampler configuration, refusing invalid counts, step sizes, or thresholds.
    pub fn validate(&self) -> Result<(), Refusal> {
        self.thresholds.validate()?;
        if self.num_draws < 4 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("num_draws must be >= 4, got {}", self.num_draws),
            ));
        }
        if !(self.target_accept.is_finite() && self.target_accept > 0.0 && self.target_accept < 1.0)
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "target_accept must lie in (0, 1), got {}",
                    self.target_accept
                ),
            ));
        }
        if self.max_treedepth == 0 || self.max_treedepth > 14 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "max_treedepth must lie in 1..=14, got {}",
                    self.max_treedepth
                ),
            ));
        }
        if !(self.max_delta_h.is_finite() && self.max_delta_h > 0.0) {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("max_delta_h must be > 0, got {}", self.max_delta_h),
            ));
        }
        if let Some(eps) = self.fixed_step_size
            && !(eps.is_finite() && eps > 0.0)
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("fixed_step_size must be > 0, got {eps}"),
            ));
        }
        Ok(())
    }
}

/// Complete output of a multi-chain NUTS run, always carrying its [`RhatEssCertificate`].
#[derive(Debug, Clone, PartialEq)]
pub struct NutsResult {
    /// Unconstrained posterior draws indexed as `chains[chain_id][draw_idx][param_idx]`.
    pub chains: Vec<Vec<Vec<f64>>>,
    /// Per-chain runtime telemetry (step sizes, divergences, treedepth hits, energies).
    pub chain_stats: Vec<ChainRuntimeStats>,
    /// Automated Vehtari et al. (2021) convergence and ESS certificate (`PO-TOUCH-4`).
    pub certificate: RhatEssCertificate,
}

/// Run multi-chain `BitwisePortable` NUTS on `model` starting from `initial_positions` (one per
/// chain, requiring `>= 2` chains for split-$\hat{R}$ certification).
pub fn sample_nuts<M: LogDensity>(
    model: &M,
    initial_positions: &[Vec<f64>],
    config: &NutsConfig,
) -> Result<NutsResult, Refusal> {
    config.validate()?;
    let dim = model.dim();
    let param_names = model.param_names();
    if dim == 0 || param_names.len() != dim {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "LogDensity dim must be > 0 and match param_names.len()",
        ));
    }
    if initial_positions.len() < 2 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "sample_nuts requires at least 2 chains for R-hat certification, got {}",
                initial_positions.len()
            ),
        ));
    }
    for (ci, init) in initial_positions.iter().enumerate() {
        if init.len() != dim || init.iter().any(|v| !v.is_finite()) {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("chain {ci} initial position has wrong dimension or non-finite coordinate"),
            ));
        }
    }

    let mut chains = Vec::with_capacity(initial_positions.len());
    let mut chain_stats = Vec::with_capacity(initial_positions.len());
    for (chain_id, init) in initial_positions.iter().enumerate() {
        let (draws, stats) = run_single_chain(model, chain_id, init, config)?;
        chains.push(draws);
        chain_stats.push(stats);
    }

    let certificate = certify_chains(&chains, &param_names, &chain_stats, &config.thresholds)?;
    Ok(NutsResult {
        chains,
        chain_stats,
        certificate,
    })
}

#[derive(Clone)]
struct PhasePoint {
    q: Vec<f64>,
    p: Vec<f64>,
    log_p: f64,
    grad: Vec<f64>,
}

struct DualAveraging {
    mu: f64,
    log_eps: f64,
    log_eps_bar: f64,
    h_bar: f64,
    m: usize,
    target_accept: f64,
    gamma: f64,
    t0: f64,
    kappa: f64,
}

impl DualAveraging {
    fn new(init_eps: f64, target_accept: f64) -> Self {
        let clamped = init_eps.clamp(1e-8, 1e2);
        Self {
            mu: det::ln(10.0 * clamped),
            log_eps: det::ln(clamped),
            log_eps_bar: 0.0,
            h_bar: 0.0,
            m: 0,
            target_accept,
            gamma: 0.05,
            t0: 10.0,
            kappa: 0.75,
        }
    }

    fn reset(&mut self, init_eps: f64) {
        let clamped = init_eps.clamp(1e-8, 1e2);
        self.mu = det::ln(10.0 * clamped);
        self.log_eps = det::ln(clamped);
        self.log_eps_bar = 0.0;
        self.h_bar = 0.0;
        self.m = 0;
    }

    fn update(&mut self, accept_stat: f64) -> f64 {
        self.m += 1;
        let mf = self.m as f64;
        let w = 1.0 / (mf + self.t0);
        self.h_bar = (1.0 - w) * self.h_bar + w * (self.target_accept - accept_stat);
        let raw_log_eps = self.mu - (det::sqrt(mf) / self.gamma) * self.h_bar;
        self.log_eps = raw_log_eps.clamp(-20.0, 4.0);
        // m^{-kappa} = exp(-kappa * ln(m)) via BitwisePortable det::exp / det::ln
        let eta = det::exp(-self.kappa * det::ln(mf));
        self.log_eps_bar = eta * self.log_eps + (1.0 - eta) * self.log_eps_bar;
        det::exp(self.log_eps)
    }

    fn final_step_size(&self) -> f64 {
        if self.m == 0 {
            det::exp(self.log_eps)
        } else {
            det::exp(self.log_eps_bar.clamp(-20.0, 4.0))
        }
    }
}

struct WelfordDiag {
    count: usize,
    mean: Vec<f64>,
    m2: Vec<f64>,
}

impl WelfordDiag {
    fn new(dim: usize) -> Self {
        Self {
            count: 0,
            mean: vec![0.0; dim],
            m2: vec![0.0; dim],
        }
    }

    fn reset(&mut self) {
        self.count = 0;
        self.mean.fill(0.0);
        self.m2.fill(0.0);
    }

    fn push(&mut self, q: &[f64]) {
        self.count += 1;
        let cf = self.count as f64;
        for (i, &x) in q.iter().enumerate() {
            let delta = x - self.mean[i];
            self.mean[i] += delta / cf;
            let delta2 = x - self.mean[i];
            self.m2[i] += delta * delta2;
        }
    }

    /// Regularized sample variance estimate per Stan (`(n / (n + 5)) * var + 1e-3 * (5 / (n + 5))`).
    fn regularized_variance(&self) -> Vec<f64> {
        let n = self.count as f64;
        if self.count < 2 {
            return vec![1.0; self.mean.len()];
        }
        let w_sample = n / (n + 5.0);
        let w_prior = 5.0 / (n + 5.0);
        self.m2
            .iter()
            .map(|&m2| {
                let sample_var = (m2 / (n - 1.0)).max(0.0);
                (w_sample * sample_var + w_prior * 1e-3).clamp(1e-6, 1e6)
            })
            .collect()
    }
}

/// Stan 3-stage adaptation window schedule: returns `(slow_start, metric_window_ends)`.
fn metric_schedule(num_warmup: usize) -> (usize, Vec<usize>) {
    let mut ends = Vec::new();
    if num_warmup < 60 {
        return (usize::MAX, ends);
    }
    let init_buf = (num_warmup / 7).clamp(25, 75);
    let term_buf = (num_warmup / 8).clamp(25, 75);
    let slow_end = num_warmup.saturating_sub(term_buf);
    if slow_end <= init_buf + 10 {
        return (usize::MAX, ends);
    }
    let mut start = init_buf;
    let mut window = 25usize;
    while start < slow_end {
        let next_end = start + window;
        if next_end + window * 2 > slow_end {
            ends.push(slow_end);
            break;
        }
        ends.push(next_end);
        start = next_end;
        window *= 2;
    }
    (init_buf, ends)
}

fn kinetic_energy(p: &[f64], inv_mass: &[f64]) -> f64 {
    0.5 * p
        .iter()
        .zip(inv_mass.iter())
        .map(|(&pi, &mi)| pi * pi * mi)
        .sum::<f64>()
}

fn draw_momentum(rng: &mut PhiloxStream, inv_mass: &[f64]) -> Vec<f64> {
    // p_i ~ Normal(0, M_i) where M_i = 1 / inv_mass[i].
    inv_mass
        .iter()
        .map(|&inv_m| rng.next_normal() / det::sqrt(inv_m))
        .collect()
}

fn leapfrog_step<M: LogDensity>(
    model: &M,
    state: &PhasePoint,
    direction: f64,
    step_size: f64,
    inv_mass: &[f64],
) -> Option<PhasePoint> {
    let eps = direction * step_size;
    let dim = state.q.len();
    let mut p_half = Vec::with_capacity(dim);
    let mut q_new = Vec::with_capacity(dim);
    for i in 0..dim {
        let ph = state.p[i] + 0.5 * eps * state.grad[i];
        let qn = state.q[i] + eps * inv_mass[i] * ph;
        if !(ph.is_finite() && qn.is_finite()) {
            return None;
        }
        p_half.push(ph);
        q_new.push(qn);
    }
    let Ok((log_p_new, grad_new)) = model.log_density_and_grad(&q_new) else {
        return None;
    };
    if grad_new.len() != dim || !log_p_new.is_finite() || grad_new.iter().any(|g| !g.is_finite()) {
        return None;
    }
    let mut p_new = Vec::with_capacity(dim);
    for i in 0..dim {
        let pn = p_half[i] + 0.5 * eps * grad_new[i];
        if !pn.is_finite() {
            return None;
        }
        p_new.push(pn);
    }
    Some(PhasePoint {
        q: q_new,
        p: p_new,
        log_p: log_p_new,
        grad: grad_new,
    })
}

/// Betancourt (2017) / Stan 2.24+ de-biased generalized no-U-turn criterion using the trapezoidal
/// momentum integral `rho_mid = rho - 0.5 * (p_left + p_right)` (exact time-reversal symmetry;
/// eliminates endpoint self-bias that can mask U-turns on curved/hierarchical trajectories).
fn no_uturn_mid(rho: &[f64], left: &PhasePoint, right: &PhasePoint, inv_mass: &[f64]) -> bool {
    let mut dot_left = 0.0_f64;
    let mut dot_right = 0.0_f64;
    for i in 0..rho.len() {
        let mid = rho[i] - 0.5 * (left.p[i] + right.p[i]);
        dot_left += mid * (inv_mass[i] * left.p[i]);
        dot_right += mid * (inv_mass[i] * right.p[i]);
    }
    dot_left > 0.0 && dot_right > 0.0
}

/// Stan 2.24+ full + cross-boundary sub-span U-turn check when merging `left_span` and `right_span`.
fn merged_no_uturn(
    rho_left: &[f64],
    left_outer: &PhasePoint,
    left_inner: &PhasePoint,
    rho_right: &[f64],
    right_inner: &PhasePoint,
    right_outer: &PhasePoint,
    inv_mass: &[f64],
) -> (Vec<f64>, bool) {
    let rho_all = add_vecs(rho_left, rho_right);
    if !no_uturn_mid(&rho_all, left_outer, right_outer, inv_mass) {
        return (rho_all, false);
    }
    let rho_l_cross = add_vecs(rho_left, &right_inner.p);
    if !no_uturn_mid(&rho_l_cross, left_outer, right_inner, inv_mass) {
        return (rho_all, false);
    }
    let rho_r_cross = add_vecs(&left_inner.p, rho_right);
    let ok = no_uturn_mid(&rho_r_cross, left_inner, right_outer, inv_mass);
    (rho_all, ok)
}

fn add_vecs(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b.iter()).map(|(&x, &y)| x + y).collect()
}

fn find_reasonable_step_size<M: LogDensity>(
    model: &M,
    q: &[f64],
    log_p: f64,
    grad: &[f64],
    inv_mass: &[f64],
    rng: &mut PhiloxStream,
) -> f64 {
    let p0 = draw_momentum(rng, inv_mass);
    let h0 = -log_p + kinetic_energy(&p0, inv_mass);
    let base = PhasePoint {
        q: q.to_vec(),
        p: p0,
        log_p,
        grad: grad.to_vec(),
    };
    let mut eps = 1.0_f64;
    let eval_log_ratio = |e: f64| -> f64 {
        match leapfrog_step(model, &base, 1.0, e, inv_mass) {
            Some(next) => {
                let h1 = -next.log_p + kinetic_energy(&next.p, inv_mass);
                if h1.is_finite() {
                    h0 - h1
                } else {
                    f64::NEG_INFINITY
                }
            }
            None => f64::NEG_INFINITY,
        }
    };

    let mut log_ratio = eval_log_ratio(eps);
    let log_half = -core::f64::consts::LN_2;
    let dir: f64 = if log_ratio > log_half { 1.0 } else { -1.0 };
    for _ in 0..24 {
        if dir > 0.0 {
            if log_ratio <= log_half || eps >= 16.0 {
                break;
            }
            eps *= 2.0;
        } else {
            if log_ratio > log_half || eps <= 1e-6 {
                break;
            }
            eps *= 0.5;
        }
        log_ratio = eval_log_ratio(eps);
    }
    eps.clamp(1e-6, 16.0)
}

struct Subtree {
    left: PhasePoint,
    right: PhasePoint,
    proposal: PhasePoint,
    rho: Vec<f64>,
    log_sum_weight: f64,
    valid: bool,
    divergent: bool,
    alpha_sum: f64,
    n_alpha: usize,
}

#[derive(Clone, Copy)]
struct TransitionParams<'a> {
    step_size: f64,
    max_treedepth: usize,
    max_delta_h: f64,
    inv_mass: &'a [f64],
}

fn build_tree<M: LogDensity>(
    model: &M,
    start: &PhasePoint,
    direction: f64,
    depth: usize,
    h0: f64,
    params: TransitionParams<'_>,
    rng: &mut PhiloxStream,
) -> Subtree {
    if depth == 0 {
        return match leapfrog_step(model, start, direction, params.step_size, params.inv_mass) {
            Some(next) => {
                let h1 = -next.log_p + kinetic_energy(&next.p, params.inv_mass);
                let delta_h = h1 - h0;
                if !delta_h.is_finite() || delta_h > params.max_delta_h {
                    Subtree {
                        rho: next.p.clone(),
                        left: next.clone(),
                        right: next.clone(),
                        proposal: next,
                        log_sum_weight: f64::NEG_INFINITY,
                        valid: false,
                        divergent: true,
                        alpha_sum: 0.0,
                        n_alpha: 1,
                    }
                } else {
                    let log_w = -delta_h;
                    let alpha = if delta_h <= 0.0 {
                        1.0
                    } else {
                        det::exp(-delta_h)
                    };
                    Subtree {
                        rho: next.p.clone(),
                        left: next.clone(),
                        right: next.clone(),
                        proposal: next,
                        log_sum_weight: log_w,
                        valid: true,
                        divergent: false,
                        alpha_sum: alpha,
                        n_alpha: 1,
                    }
                }
            }
            None => Subtree {
                rho: start.p.clone(),
                left: start.clone(),
                right: start.clone(),
                proposal: start.clone(),
                log_sum_weight: f64::NEG_INFINITY,
                valid: false,
                divergent: true,
                alpha_sum: 0.0,
                n_alpha: 1,
            },
        };
    }

    let first = build_tree(model, start, direction, depth - 1, h0, params, rng);
    if !first.valid {
        return first;
    }

    let next_start = if direction < 0.0 {
        &first.left
    } else {
        &first.right
    };
    let second = build_tree(model, next_start, direction, depth - 1, h0, params, rng);

    let combined_log_w = log_add_exp(first.log_sum_weight, second.log_sum_weight);
    let pick_second = if second.log_sum_weight == f64::NEG_INFINITY {
        false
    } else if first.log_sum_weight == f64::NEG_INFINITY {
        true
    } else {
        let p_second = det::exp((second.log_sum_weight - combined_log_w).min(0.0));
        rng.next_f64_open01() < p_second
    };
    let proposal = if pick_second {
        second.proposal
    } else {
        first.proposal
    };

    let (left, right, rho, span_ok) = if direction < 0.0 {
        let (rho, ok) = merged_no_uturn(
            &second.rho,
            &second.left,
            &second.right,
            &first.rho,
            &first.left,
            &first.right,
            params.inv_mass,
        );
        (second.left, first.right, rho, ok)
    } else {
        let (rho, ok) = merged_no_uturn(
            &first.rho,
            &first.left,
            &first.right,
            &second.rho,
            &second.left,
            &second.right,
            params.inv_mass,
        );
        (first.left, second.right, rho, ok)
    };
    let valid = second.valid && span_ok;

    Subtree {
        left,
        right,
        proposal,
        rho,
        log_sum_weight: combined_log_w,
        valid,
        divergent: first.divergent || second.divergent,
        alpha_sum: first.alpha_sum + second.alpha_sum,
        n_alpha: first.n_alpha + second.n_alpha,
    }
}

struct TransitionOut {
    q: Vec<f64>,
    log_p: f64,
    grad: Vec<f64>,
    energy: f64,
    accept_stat: f64,
    divergent: bool,
    hit_max_treedepth: bool,
}

fn nuts_transition<M: LogDensity>(
    model: &M,
    q: &[f64],
    log_p: f64,
    grad: &[f64],
    params: TransitionParams<'_>,
    rng: &mut PhiloxStream,
) -> TransitionOut {
    let p0 = draw_momentum(rng, params.inv_mass);
    let h0 = -log_p + kinetic_energy(&p0, params.inv_mass);
    let init = PhasePoint {
        q: q.to_vec(),
        p: p0,
        log_p,
        grad: grad.to_vec(),
    };

    let mut left = init.clone();
    let mut right = init.clone();
    let mut rho = init.p.clone();
    let mut current = init;
    let mut tree_log_w = 0.0_f64; // log w_0 = H_0 - H_0 = 0
    let mut valid = true;
    let mut divergent = false;
    let mut alpha_sum = 0.0_f64;
    let mut n_alpha = 0usize;
    let mut depth_reached = 0usize;

    for depth in 0..params.max_treedepth {
        if !valid {
            break;
        }
        depth_reached = depth + 1;
        let direction = if rng.next_f64_open01() < 0.5 {
            -1.0
        } else {
            1.0
        };
        let start = if direction < 0.0 { &left } else { &right };
        let sub = build_tree(model, start, direction, depth, h0, params, rng);
        alpha_sum += sub.alpha_sum;
        n_alpha += sub.n_alpha;
        divergent |= sub.divergent;

        if sub.valid {
            // Biased progressive multinomial acceptance (Betancourt 2017 §A.3.2).
            let log_accept = sub.log_sum_weight - tree_log_w;
            let accept = log_accept >= 0.0 || rng.next_f64_open01() < det::exp(log_accept);
            if accept {
                current = sub.proposal;
            }
        }
        tree_log_w = log_add_exp(tree_log_w, sub.log_sum_weight);
        let span_ok = if direction < 0.0 {
            let (new_rho, ok) = merged_no_uturn(
                &sub.rho,
                &sub.left,
                &sub.right,
                &rho,
                &left,
                &right,
                params.inv_mass,
            );
            rho = new_rho;
            left = sub.left;
            ok
        } else {
            let (new_rho, ok) = merged_no_uturn(
                &rho,
                &left,
                &right,
                &sub.rho,
                &sub.left,
                &sub.right,
                params.inv_mass,
            );
            rho = new_rho;
            right = sub.right;
            ok
        };
        valid = sub.valid && span_ok;
    }

    let accept_stat = if n_alpha > 0 {
        (alpha_sum / (n_alpha as f64)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let final_energy = -current.log_p + kinetic_energy(&current.p, params.inv_mass);
    TransitionOut {
        q: current.q,
        log_p: current.log_p,
        grad: current.grad,
        energy: final_energy,
        accept_stat,
        divergent,
        hit_max_treedepth: valid && depth_reached == params.max_treedepth,
    }
}

fn run_single_chain<M: LogDensity>(
    model: &M,
    chain_id: usize,
    init: &[f64],
    config: &NutsConfig,
) -> Result<(Vec<Vec<f64>>, ChainRuntimeStats), Refusal> {
    let dim = model.dim();
    let rng_key = RngKey {
        seed_root: config.seed_root,
        partition: 0x4E55_5453, // "NUTS"
        domain: 0,
        var_group: 0,
        realization: u32::try_from(chain_id).unwrap_or(u32::MAX),
        brick: 0,
    };
    let mut rng = PhiloxStream::new(rng_key);

    let (mut log_p, mut grad) = model.log_density_and_grad(init)?;
    if grad.len() != dim || !log_p.is_finite() || grad.iter().any(|g| !g.is_finite()) {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "chain {chain_id} initial position yields mismatched gradient length or non-finite log-density/gradient"
            ),
        ));
    }

    let mut q = init.to_vec();
    let mut inv_mass = vec![1.0_f64; dim];
    let mut step_size = match config.fixed_step_size {
        Some(eps) => eps,
        None => find_reasonable_step_size(model, &q, log_p, &grad, &inv_mass, &mut rng),
    };
    let mut da = DualAveraging::new(step_size, config.target_accept);
    let mut welford = WelfordDiag::new(dim);
    let (slow_start, metric_ends) = if config.adapt_mass_matrix {
        metric_schedule(config.num_warmup)
    } else {
        (usize::MAX, Vec::new())
    };
    let slow_end = metric_ends.last().copied().unwrap_or(0);
    let mut next_metric_idx = 0usize;

    for iter in 0..config.num_warmup {
        let out = nuts_transition(
            model,
            &q,
            log_p,
            &grad,
            TransitionParams {
                step_size,
                max_treedepth: config.max_treedepth,
                max_delta_h: config.max_delta_h,
                inv_mass: &inv_mass,
            },
            &mut rng,
        );
        q = out.q;
        log_p = out.log_p;
        grad = out.grad;

        if config.fixed_step_size.is_none() {
            step_size = da.update(out.accept_stat);
        }
        if iter >= slow_start && iter < slow_end {
            welford.push(&q);
        }
        if next_metric_idx < metric_ends.len() && iter + 1 == metric_ends[next_metric_idx] {
            inv_mass = welford.regularized_variance();
            welford.reset();
            next_metric_idx += 1;
            if config.fixed_step_size.is_none() {
                step_size = find_reasonable_step_size(model, &q, log_p, &grad, &inv_mass, &mut rng);
                da.reset(step_size);
            }
        }
    }

    let final_step_size = match config.fixed_step_size {
        Some(eps) => eps,
        None => da.final_step_size(),
    };

    let mut draws = Vec::with_capacity(config.num_draws);
    let mut energies = Vec::with_capacity(config.num_draws);
    let mut divergences = 0usize;
    let mut treedepth_hits = 0usize;
    let mut accept_sum = 0.0_f64;

    for _ in 0..config.num_draws {
        let out = nuts_transition(
            model,
            &q,
            log_p,
            &grad,
            TransitionParams {
                step_size: final_step_size,
                max_treedepth: config.max_treedepth,
                max_delta_h: config.max_delta_h,
                inv_mass: &inv_mass,
            },
            &mut rng,
        );
        q = out.q;
        log_p = out.log_p;
        grad = out.grad;
        if out.divergent {
            divergences += 1;
        }
        if out.hit_max_treedepth {
            treedepth_hits += 1;
        }
        accept_sum += out.accept_stat;
        energies.push(out.energy);
        draws.push(q.clone());
    }

    let stats = ChainRuntimeStats {
        chain_id,
        adapted_step_size: final_step_size,
        post_warmup_divergences: divergences,
        max_treedepth_hits: treedepth_hits,
        mean_accept_prob: accept_sum / (config.num_draws as f64),
        post_warmup_energies: energies,
    };
    Ok((draws, stats))
}
