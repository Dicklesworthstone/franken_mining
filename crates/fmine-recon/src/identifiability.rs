//! Full bias identifiability analysis, observation-operator rank/nullspace decomposition,
//! `FMINE-REF-104` attribution refusal, and Measurement Value of Information (`BD-153`, plan §20.6,
//! `G-TOUCH-2`, `GB-25`).
//! Determinism: DeterministicPerIsa (column-pivoted Modified Gram–Schmidt QR with Kahan–Parlett
//! re-orthogonalization + equilibrated SPD precision inversion).

use std::collections::BTreeSet;

use fmine_core::codec::Encoder;
use fmine_core::det;
use fmine_core::hash::blake3_hash;

use crate::bias::HierarchicalBiasResult;
use crate::error::ReconError;
use crate::linear::{IDENTIFIABILITY_CORRELATION_LIMIT, solve_equilibrated_spd};

/// `sqrt(2 / pi)`: exact expected reduction in absolute error `E[|X|]` per unit SD reduction
/// under a zero-mean Gaussian posterior (normal linear-loss EVSI coefficient).
const SQRT_TWO_OVER_PI: f64 = 0.797_884_560_802_865_4;

/// Relative pivot cutoff for numerical rank determination of the weighted observation operator.
const RANK_REL_TOL: f64 = 1.0e-10;

/// Minimum prior-variance reduction below which a rank-deficient split is flagged prior-dominated.
const NULLSPACE_PROJECTION_TOL: f64 = 1.0e-10;

/// Candidate separating measurement category from plan §20.6.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CandidateMeasurementKind {
    SamplerAudit,
    CrossBeltAnalyser,
    StockpileResurvey,
    CheckAssayRoundRobin,
    DensityProgramme,
    GradeControlInfill,
}

impl CandidateMeasurementKind {
    /// Stable canonical tag.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SamplerAudit => "sampler_audit",
            Self::CrossBeltAnalyser => "cross_belt_analyser",
            Self::StockpileResurvey => "stockpile_resurvey",
            Self::CheckAssayRoundRobin => "check_assay_round_robin",
            Self::DensityProgramme => "density_programme",
            Self::GradeControlInfill => "grade_control_infill",
        }
    }
}

/// Request for full observation-operator rank + posterior correlation identifiability analysis
/// (`BD-153`, plan §20.6).
#[derive(Clone, Debug, PartialEq)]
pub struct IdentifiabilityRequest {
    /// Parameter names (`len = k`, unique non-empty).
    pub parameter_names: Vec<String>,
    /// Prior means (`len = k`).
    pub prior_means: Vec<f64>,
    /// Row-major `k x k` SPD prior covariance matrix `Sigma_0`.
    pub prior_covariance: Vec<f64>,
    /// Row-major `n_obs x k` linear observation operator `H` restricted to the bias parameters.
    pub observation_design: Vec<f64>,
    /// Observed scalar values `y` (`len = n_obs`).
    pub observed: Vec<f64>,
    /// Observation standard deviations `sigma` (`len = n_obs`, all `> 0`).
    pub obs_sds: Vec<f64>,
    /// Credible level in `(0, 1)` (e.g. `0.90` or `0.95`).
    pub confidence_level: f64,
}

/// Posterior summary of one orthonormal contrast `c^T b` spanning `row(H)` (always determined by
/// the data even when individual bias parameters are confounded).
#[derive(Clone, Debug, PartialEq)]
pub struct DeterminedContrastBasis {
    /// Orthonormal unit contrast vector `c` (`len = k`, `||c||_2 = 1`).
    pub weights: Vec<f64>,
    /// Human-readable formula naming non-zero parameter weights.
    pub formula: String,
    /// Posterior mean `c^T mu_post`.
    pub posterior_mean: f64,
    /// Posterior standard deviation `sqrt(c^T Sigma_post c)`.
    pub posterior_sd: f64,
}

/// Per-parameter identifiability status and marginal posterior summary.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterIdentifiability {
    /// Parameter name.
    pub name: String,
    /// Posterior mean.
    pub posterior_mean: f64,
    /// Posterior standard deviation.
    pub posterior_sd: f64,
    /// Lower central credible bound.
    pub ci_lower: f64,
    /// Upper central credible bound.
    pub ci_upper: f64,
    /// Fraction of marginal prior variance reduced by data: `1 - var_post / var_prior`.
    pub variance_reduction_fraction: f64,
    /// Squared projection of `e_i` onto `ker(H)` (`0.0` iff `e_i` lies in `row(H)`).
    pub nullspace_weight: f64,
    /// Maximum `|posterior_corr(i, j)|` across all `j != i`.
    pub max_abs_pair_correlation: f64,
    /// `true` iff `nullspace_weight <= 1e-10` AND `max_abs_pair_correlation <= 0.80`.
    pub identifiable: bool,
}

/// Pair of bias parameters refused separate attribution under `G-TOUCH-2` (`FMINE-REF-104`).
#[derive(Clone, Debug, PartialEq)]
pub struct ConfoundedBiasPair {
    /// First parameter name.
    pub first: String,
    /// Second parameter name.
    pub second: String,
    /// Posterior Pearson correlation (`|corr| > 0.80`).
    pub correlation: f64,
    /// `true` iff the pair also participates in a non-trivial nullspace of `H`.
    pub rank_deficient_split: bool,
}

/// Complete identifiability and rank-decomposition report (`BD-153`, plan §20.6).
#[derive(Clone, Debug, PartialEq)]
pub struct IdentifiabilityReport {
    /// Parameter names (`len = k`).
    pub parameter_names: Vec<String>,
    /// Total number of bias parameters `k`.
    pub parameter_count: usize,
    /// Numerical rank of the standardized observation operator `R^{-1/2} H`.
    pub observation_rank: usize,
    /// Dimension of `ker(H)` (`k - observation_rank`).
    pub nullspace_dimension: usize,
    /// Per-parameter identifiability diagnostics and marginal posteriors.
    pub parameters: Vec<ParameterIdentifiability>,
    /// Pairs with `|posterior_corr| > 0.80` refused separate attribution (`G-TOUCH-2`).
    pub confounded_pairs: Vec<ConfoundedBiasPair>,
    /// Orthonormal basis of `row(H)` — every linear contrast genuinely determined by observations.
    pub identifiable_contrasts: Vec<DeterminedContrastBasis>,
    /// Orthonormal basis of `ker(H)` — unobservable bias directions driven strictly by the prior.
    pub nullspace_directions: Vec<Vec<f64>>,
    /// Full `k x k` posterior precision matrix `Lambda_post` (retained for exact VOI updates).
    pub posterior_precision: Vec<f64>,
    /// Full `k x k` posterior covariance matrix `Sigma_post`.
    pub posterior_covariance: Vec<f64>,
    /// Weighted observation matrix `R^{-1/2} H` (`n_obs x k`) retained for exact post-measurement rank.
    pub weighted_design: Vec<f64>,
    /// `true` iff `observation_rank == parameter_count` AND `confounded_pairs.is_empty()`.
    pub is_fully_identifiable: bool,
    /// Deterministic BLAKE3 hex digest.
    pub digest_hex: String,
}

impl IdentifiabilityReport {
    /// Returns `true` iff parameter `name` is individually identifiable (zero nullspace projection
    /// and no partner correlation `> 0.80`).
    #[must_use]
    pub fn is_parameter_identifiable(&self, name: &str) -> bool {
        self.parameters
            .iter()
            .find(|p| p.name == name)
            .is_some_and(|p| p.identifiable)
    }
}

/// Candidate separating measurement whose Value of Information is scored under §20.6.
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateMeasurement {
    /// Stable identifier (e.g. `"mill_head_stopped_belt_audit"`).
    pub id: String,
    /// Measurement campaign class (§20.6).
    pub kind: CandidateMeasurementKind,
    /// Human-readable description of the campaign.
    pub description: String,
    /// Linear design row `h_cand` aligned with `parameter_names` (`len = k`).
    pub design_row: Vec<f64>,
    /// Expected 1-sigma measurement precision (`> 0`).
    pub obs_sd: f64,
    /// Direct campaign cost in USD (`>= 0`).
    pub cost_usd: f64,
}

/// Request to score and rank candidate separating measurements by exact posterior-variance
/// reduction and Gaussian linear-loss Expected Value of Sample Information (`BD-153`, §20.6).
#[derive(Clone, Debug, PartialEq)]
pub struct MeasurementVoiRequest<'a> {
    /// Baseline identifiability report before running any candidate campaign.
    pub baseline: &'a IdentifiabilityReport,
    /// Parameter indices (`0..k`) whose unresolved bias error drives decision value (empty = all).
    pub target_indices: Vec<usize>,
    /// Annualized decision value exposure in USD per `1.0` unit of bias error (e.g. annual revenue).
    pub unit_error_value_usd: f64,
    /// Candidate measurements to evaluate and rank.
    pub candidates: Vec<CandidateMeasurement>,
}

/// Ranked VOI evaluation for one [`CandidateMeasurement`].
#[derive(Clone, Debug, PartialEq)]
pub struct MeasurementVoiScore {
    /// Candidate identifier.
    pub id: String,
    /// Measurement campaign class.
    pub kind: CandidateMeasurementKind,
    /// Human-readable description.
    pub description: String,
    /// Observation operator rank after adding this measurement.
    pub post_rank: usize,
    /// Maximum `|posterior_corr|` among pairs before the measurement.
    pub max_abs_corr_before: f64,
    /// Maximum `|posterior_corr|` among pairs after the measurement.
    pub max_abs_corr_after: f64,
    /// `true` iff adding this measurement yields full rank and `max_abs_corr_after <= 0.80`.
    pub resolves_identifiability: bool,
    /// Mean fractional posterior variance reduction `1 - var_after / var_before` across targets.
    pub mean_variance_reduction_fraction: f64,
    /// Expected decision value of information in USD (`sum_i sqrt(2/pi) * V * (sd_before - sd_after)`).
    pub expected_value_usd: f64,
    /// Campaign direct cost in USD.
    pub cost_usd: f64,
    /// Net Value of Information (`expected_value_usd - cost_usd`).
    pub net_voi_usd: f64,
}

struct OperatorSubspaces {
    rank: usize,
    row_basis: Vec<Vec<f64>>,
    null_basis: Vec<Vec<f64>>,
}

/// Column-pivoted Modified Gram–Schmidt QR with Kahan–Parlett re-orthogonalization on `m x k`
/// matrix `a`, returning exact numerical rank, orthonormal basis of `row(a)`, and orthonormal basis
/// of `ker(a)`.
fn decompose_operator_subspaces(a: &[f64], m: usize, k: usize) -> OperatorSubspaces {
    if m == 0 || k == 0 {
        let mut null_basis = Vec::with_capacity(k);
        for j in 0..k {
            let mut e = vec![0.0_f64; k];
            e[j] = 1.0;
            null_basis.push(e);
        }
        return OperatorSubspaces {
            rank: 0,
            row_basis: Vec::new(),
            null_basis,
        };
    }

    // Store columns of A (k vectors of length m).
    let mut cols: Vec<Vec<f64>> = (0..k)
        .map(|j| (0..m).map(|i| a[i * k + j]).collect())
        .collect();
    let mut perm: Vec<usize> = (0..k).collect();
    let max_steps = m.min(k);
    let mut r_mat = vec![0.0_f64; max_steps * k];
    let mut q_cols: Vec<Vec<f64>> = Vec::with_capacity(max_steps);
    let mut rank = 0usize;
    let mut max_pivot = 0.0_f64;

    for s in 0..max_steps {
        let mut best_col = s;
        let mut best_norm2 = 0.0_f64;
        for (j, col) in cols.iter().enumerate().skip(s) {
            let n2: f64 = col.iter().map(|v| v * v).sum();
            if n2 > best_norm2 {
                best_norm2 = n2;
                best_col = j;
            }
        }
        let pivot_norm = det::sqrt(best_norm2);
        if s == 0 {
            max_pivot = pivot_norm;
        }
        if max_pivot <= 0.0 || pivot_norm <= RANK_REL_TOL * max_pivot {
            break;
        }
        if best_col != s {
            cols.swap(s, best_col);
            perm.swap(s, best_col);
            for prev in 0..s {
                r_mat.swap(prev * k + s, prev * k + best_col);
            }
        }
        let q: Vec<f64> = cols[s].iter().map(|v| v / pivot_norm).collect();
        r_mat[s * k + s] = pivot_norm;

        for j in (s + 1)..k {
            let mut dot1 = 0.0_f64;
            for i in 0..m {
                dot1 += q[i] * cols[j][i];
            }
            for i in 0..m {
                cols[j][i] -= dot1 * q[i];
            }
            // Second pass (Kahan-Parlett re-orthogonalization) for machine-precision orthogonality.
            let mut dot2 = 0.0_f64;
            for i in 0..m {
                dot2 += q[i] * cols[j][i];
            }
            for i in 0..m {
                cols[j][i] -= dot2 * q[i];
            }
            r_mat[s * k + j] = dot1 + dot2;
        }
        q_cols.push(q);
        rank += 1;
    }

    // Orthonormal basis of row(A) from the first `rank` rows of R * P^T.
    let mut raw_rows: Vec<Vec<f64>> = Vec::with_capacity(rank);
    for s in 0..rank {
        let mut row = vec![0.0_f64; k];
        for j in s..k {
            row[perm[j]] = r_mat[s * k + j];
        }
        raw_rows.push(row);
    }
    let row_basis = orthonormalize_vectors(raw_rows);

    // Exact nullspace basis of ker(A) via back-substitution on R_{11} u = -R_{12}[:, f].
    let mut raw_null: Vec<Vec<f64>> = Vec::with_capacity(k - rank);
    for free in rank..k {
        let mut u = vec![0.0_f64; rank];
        for s in (0..rank).rev() {
            let mut rhs = -r_mat[s * k + free];
            for j in (s + 1)..rank {
                rhs -= r_mat[s * k + j] * u[j];
            }
            u[s] = rhs / r_mat[s * k + s];
        }
        let mut v = vec![0.0_f64; k];
        for s in 0..rank {
            v[perm[s]] = u[s];
        }
        v[perm[free]] = 1.0;
        raw_null.push(v);
    }
    let null_basis = orthonormalize_vectors(raw_null);

    OperatorSubspaces {
        rank,
        row_basis,
        null_basis,
    }
}

fn orthonormalize_vectors(mut vecs: Vec<Vec<f64>>) -> Vec<Vec<f64>> {
    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(vecs.len());
    for v in &mut vecs {
        for _ in 0..2 {
            for b in &basis {
                let proj: f64 = v.iter().zip(b).map(|(x, y)| x * y).sum();
                for (x, &y) in v.iter_mut().zip(b) {
                    *x -= proj * y;
                }
            }
        }
        let norm = det::sqrt(v.iter().map(|x| x * x).sum::<f64>());
        if norm > 1e-12 {
            let mut unit: Vec<f64> = v.iter().map(|x| x / norm).collect();
            // Canonicalize sign: first entry with |x| > 1e-9 is positive.
            if let Some(&first_nz) = unit.iter().find(|x| x.abs() > 1e-9)
                && first_nz < 0.0
            {
                for x in &mut unit {
                    *x = -*x;
                }
            }
            basis.push(unit);
        }
    }
    basis
}

fn format_contrast_formula(names: &[String], weights: &[f64]) -> String {
    let mut parts = Vec::new();
    for (name, &w) in names.iter().zip(weights) {
        if w.abs() > 1e-6 {
            parts.push(format!("{w:+.4}*{name}"));
        }
    }
    if parts.is_empty() {
        "0".to_string()
    } else {
        parts.join(" ")
    }
}

fn invert_spd(mat: &[f64], k: usize) -> Result<Vec<f64>, ReconError> {
    let mut inv = vec![0.0_f64; k * k];
    for j in 0..k {
        let mut ej = vec![0.0_f64; k];
        ej[j] = 1.0;
        let col = solve_equilibrated_spd(mat, &ej, k)?;
        for i in 0..k {
            inv[i * k + j] = col[i];
        }
    }
    for i in 0..k {
        for j in (i + 1)..k {
            let avg = 0.5 * (inv[i * k + j] + inv[j * k + i]);
            inv[i * k + j] = avg;
            inv[j * k + i] = avg;
        }
    }
    Ok(inv)
}

fn max_abs_offdiag_corr(cov: &[f64], k: usize) -> f64 {
    let mut best = 0.0_f64;
    for i in 0..k {
        let vi = cov[i * k + i];
        if vi <= 0.0 {
            continue;
        }
        let si = det::sqrt(vi);
        for j in (i + 1)..k {
            let vj = cov[j * k + j];
            if vj <= 0.0 {
                continue;
            }
            let sj = det::sqrt(vj);
            let c = ((cov[i * k + j] / si.max(sj)) / si.min(sj)).abs();
            if c.is_finite() && c > best {
                best = c;
            }
        }
    }
    best
}

/// Runs full linear-Gaussian identifiability analysis: checks the rank and nullspace of the
/// standardized observation operator `R^{-1/2} H`, computes posterior correlations and prior
/// variance reductions, and identifies every confounded pair (`BD-153`, plan §20.6, `G-TOUCH-2`).
pub fn analyze_identifiability(
    request: &IdentifiabilityRequest,
) -> Result<IdentifiabilityReport, ReconError> {
    let k = request.parameter_names.len();
    if k == 0 || k > 64 {
        return Err(ReconError::invalid(
            "identifiability.parameter_names",
            format!("require 1..=64 parameters, got {k}"),
        ));
    }
    if request.prior_means.len() != k || request.prior_covariance.len() != k * k {
        return Err(ReconError::invalid(
            "identifiability.prior",
            "prior_means or prior_covariance dimensions do not match parameter_names",
        ));
    }
    let n_obs = request.observed.len();
    if request.obs_sds.len() != n_obs || request.observation_design.len() != n_obs * k {
        return Err(ReconError::invalid(
            "identifiability.observations",
            "observation_design, observed, or obs_sds dimensions mismatch",
        ));
    }
    if !(request.confidence_level.is_finite()
        && request.confidence_level > 0.0
        && request.confidence_level < 1.0)
    {
        return Err(ReconError::invalid(
            "identifiability.confidence_level",
            format!("must lie in (0, 1), got {}", request.confidence_level),
        ));
    }

    let mut seen_names = BTreeSet::new();
    for name in &request.parameter_names {
        if name.trim().is_empty() || !seen_names.insert(name.as_str()) {
            return Err(ReconError::invalid(
                "identifiability.parameter_names",
                format!("empty or duplicate parameter name '{name}'"),
            ));
        }
    }
    if request
        .prior_means
        .iter()
        .chain(&request.prior_covariance)
        .chain(&request.observation_design)
        .chain(&request.observed)
        .any(|v| !v.is_finite())
    {
        return Err(ReconError::invalid(
            "identifiability",
            "all prior and observation inputs must be finite",
        ));
    }
    if request.obs_sds.iter().any(|s| !s.is_finite() || *s <= 0.0) {
        return Err(ReconError::invalid(
            "identifiability.obs_sds",
            "all observation standard deviations must be finite and > 0",
        ));
    }

    // Prior precision Lambda_0 = Sigma_0^-1 and prior RHS Lambda_0 * mu_0.
    let prior_prec = invert_spd(&request.prior_covariance, k)?;
    let mut post_prec = prior_prec.clone();
    let mut post_rhs = vec![0.0_f64; k];
    for i in 0..k {
        let mut s = 0.0_f64;
        for j in 0..k {
            s += prior_prec[i * k + j] * request.prior_means[j];
        }
        post_rhs[i] = s;
    }

    let mut weighted_design = vec![0.0_f64; n_obs * k];
    for row_idx in 0..n_obs {
        let inv_sd = 1.0 / request.obs_sds[row_idx];
        let inv_v = inv_sd * inv_sd;
        for i in 0..k {
            let h_i = request.observation_design[row_idx * k + i];
            weighted_design[row_idx * k + i] = h_i * inv_sd;
            post_rhs[i] += inv_v * h_i * request.observed[row_idx];
            for j in 0..k {
                let h_j = request.observation_design[row_idx * k + j];
                post_prec[i * k + j] += inv_v * h_i * h_j;
            }
        }
    }

    let mu_post = solve_equilibrated_spd(&post_prec, &post_rhs, k)?;
    let post_cov = invert_spd(&post_prec, k)?;

    build_identifiability_report(
        request.parameter_names.clone(),
        &request.prior_covariance,
        post_prec,
        mu_post,
        post_cov,
        weighted_design,
        n_obs,
        request.confidence_level,
    )
}

/// Runs full `BD-153` identifiability analysis directly on the node biases of a fitted
/// [`HierarchicalBiasResult`] (`BD-152` -> `BD-153` bridge).
pub fn analyze_hierarchical_identifiability(
    hier: &HierarchicalBiasResult,
) -> Result<IdentifiabilityReport, ReconError> {
    let k = hier.node_ids.len();
    let n_obs = hier.observation_sds.len();
    if k == 0
        || hier.node_prior_covariance.len() != k * k
        || hier.node_posterior_covariance.len() != k * k
        || hier.observation_design.len() != n_obs * k
    {
        return Err(ReconError::NumericalFailure {
            detail: "hierarchical bias result has inconsistent node identifiability dimensions"
                .to_string(),
        });
    }
    let mu_post: Vec<f64> = hier
        .node_posteriors
        .iter()
        .map(|p| p.posterior_mean)
        .collect();
    let node_post_prec = invert_spd(&hier.node_posterior_covariance, k)?;
    let mut weighted_design = vec![0.0_f64; n_obs * k];
    for r in 0..n_obs {
        let inv_sd = 1.0 / hier.observation_sds[r];
        for c in 0..k {
            weighted_design[r * k + c] = hier.observation_design[r * k + c] * inv_sd;
        }
    }

    build_identifiability_report(
        hier.node_ids.clone(),
        &hier.node_prior_covariance,
        node_post_prec,
        mu_post,
        hier.node_posterior_covariance.clone(),
        weighted_design,
        n_obs,
        hier.confidence_level,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_identifiability_report(
    parameter_names: Vec<String>,
    prior_covariance: &[f64],
    post_prec: Vec<f64>,
    mu_post: Vec<f64>,
    post_cov: Vec<f64>,
    weighted_design: Vec<f64>,
    n_obs: usize,
    confidence_level: f64,
) -> Result<IdentifiabilityReport, ReconError> {
    let k = parameter_names.len();
    let subspaces = decompose_operator_subspaces(&weighted_design, n_obs, k);
    let z_crit = det::norm_ppf(0.5 + 0.5 * confidence_level);

    // Per-parameter squared projection onto ker(H).
    let mut null_weights = vec![0.0_f64; k];
    for v in &subspaces.null_basis {
        for (i, &vi) in v.iter().enumerate() {
            null_weights[i] += vi * vi;
        }
    }

    // Pairwise posterior correlations and confounded pairs.
    let mut max_pair_corr = vec![0.0_f64; k];
    let mut confounded_pairs = Vec::new();
    for i in 0..k {
        let vi = post_cov[i * k + i].max(0.0);
        let si = det::sqrt(vi);
        if si <= 0.0 {
            continue;
        }
        for j in (i + 1)..k {
            let vj = post_cov[j * k + j].max(0.0);
            let sj = det::sqrt(vj);
            if sj <= 0.0 {
                continue;
            }
            let corr = (post_cov[i * k + j] / si.max(sj)) / si.min(sj);
            let abs_c = corr.abs();
            if abs_c > max_pair_corr[i] {
                max_pair_corr[i] = abs_c;
            }
            if abs_c > max_pair_corr[j] {
                max_pair_corr[j] = abs_c;
            }
            if abs_c > IDENTIFIABILITY_CORRELATION_LIMIT {
                let rank_def = null_weights[i] > NULLSPACE_PROJECTION_TOL
                    && null_weights[j] > NULLSPACE_PROJECTION_TOL;
                confounded_pairs.push(ConfoundedBiasPair {
                    first: parameter_names[i].clone(),
                    second: parameter_names[j].clone(),
                    correlation: corr,
                    rank_deficient_split: rank_def,
                });
            }
        }
    }

    let mut parameters = Vec::with_capacity(k);
    for i in 0..k {
        let var_prior = prior_covariance[i * k + i].max(0.0);
        let var_post = post_cov[i * k + i].max(0.0);
        let sd_post = det::sqrt(var_post);
        let lo = mu_post[i] - z_crit * sd_post;
        let hi = mu_post[i] + z_crit * sd_post;
        let vrf = if var_prior > 0.0 {
            (1.0 - var_post / var_prior).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let identifiable = null_weights[i] <= NULLSPACE_PROJECTION_TOL
            && max_pair_corr[i] <= IDENTIFIABILITY_CORRELATION_LIMIT;
        parameters.push(ParameterIdentifiability {
            name: parameter_names[i].clone(),
            posterior_mean: mu_post[i],
            posterior_sd: sd_post,
            ci_lower: lo,
            ci_upper: hi,
            variance_reduction_fraction: vrf,
            nullspace_weight: null_weights[i],
            max_abs_pair_correlation: max_pair_corr[i],
            identifiable,
        });
    }

    let mut identifiable_contrasts = Vec::with_capacity(subspaces.row_basis.len());
    for c in subspaces.row_basis {
        let mean: f64 = c.iter().zip(&mu_post).map(|(&ci, &mi)| ci * mi).sum();
        let mut var = 0.0_f64;
        for i in 0..k {
            for j in 0..k {
                var += c[i] * post_cov[i * k + j] * c[j];
            }
        }
        let formula = format_contrast_formula(&parameter_names, &c);
        identifiable_contrasts.push(DeterminedContrastBasis {
            weights: c,
            formula,
            posterior_mean: mean,
            posterior_sd: det::sqrt(var.max(0.0)),
        });
    }

    let is_fully_identifiable = subspaces.rank == k && confounded_pairs.is_empty();

    let mut enc = Encoder::new_frame(*b"IDN1", 1);
    enc.put_u32(k as u32);
    enc.put_u32(subspaces.rank as u32);
    for p in &parameters {
        enc.put_str(&p.name);
        enc.put_u64(p.posterior_mean.to_bits());
        enc.put_u64(p.posterior_sd.to_bits());
        enc.put_u8(u8::from(p.identifiable));
    }
    enc.put_u32(confounded_pairs.len() as u32);
    for pair in &confounded_pairs {
        enc.put_str(&pair.first);
        enc.put_str(&pair.second);
        enc.put_u64(pair.correlation.to_bits());
    }
    let digest_hex = blake3_hash(&enc.finish()).to_hex();

    Ok(IdentifiabilityReport {
        parameter_names,
        parameter_count: k,
        observation_rank: subspaces.rank,
        nullspace_dimension: k - subspaces.rank,
        parameters,
        confounded_pairs,
        identifiable_contrasts,
        nullspace_directions: subspaces.null_basis,
        posterior_precision: post_prec,
        posterior_covariance: post_cov,
        weighted_design,
        is_fully_identifiable,
        digest_hex,
    })
}

/// Guard `G-TOUCH-2` (`BD-153`, plan §20.6): refuses full bias attribution with `FMINE-REF-104`
/// when the observation operator is rank-deficient or any bias pair has `|posterior_corr| > 0.80`,
/// naming the determined contrasts that *are* identifiable.
pub fn require_full_identifiability(report: &IdentifiabilityReport) -> Result<(), ReconError> {
    if report.is_fully_identifiable {
        return Ok(());
    }
    let contrast_summary = report
        .identifiable_contrasts
        .first()
        .map_or_else(String::new, |c| {
            format!(
                "; determined contrast '{}' = {:.5} ± {:.5}",
                c.formula, c.posterior_mean, c.posterior_sd
            )
        });
    if let Some(pair) = report.confounded_pairs.first() {
        return Err(ReconError::NonIdentifiable {
            detail: format!(
                "rank={}/{}, pair ('{}', '{}') has posterior corr={:.3} (|corr| > {:.2}); individual attribution refused{contrast_summary}",
                report.observation_rank,
                report.parameter_count,
                pair.first,
                pair.second,
                pair.correlation,
                IDENTIFIABILITY_CORRELATION_LIMIT
            ),
        });
    }
    Err(ReconError::NonIdentifiable {
        detail: format!(
            "observation operator rank {} < {} bias parameters (nullspace dim {}); individual attribution refused{contrast_summary}",
            report.observation_rank, report.parameter_count, report.nullspace_dimension
        ),
    })
}

/// Guard `G-TOUCH-2` for a single named parameter: returns `Ok(&ParameterIdentifiability)` iff that
/// parameter is individually identifiable, else refuses with `FMINE-REF-104`.
pub fn require_parameter_identifiable<'a>(
    report: &'a IdentifiabilityReport,
    parameter_name: &str,
) -> Result<&'a ParameterIdentifiability, ReconError> {
    let Some(param) = report.parameters.iter().find(|p| p.name == parameter_name) else {
        return Err(ReconError::invalid(
            "identifiability.parameter_name",
            format!("unknown bias parameter '{parameter_name}'"),
        ));
    };
    if !param.identifiable {
        return Err(ReconError::NonIdentifiable {
            detail: format!(
                "parameter '{}' is not separately identifiable (nullspace_weight={:.3}, max_abs_corr={:.3} > {:.2})",
                param.name,
                param.nullspace_weight,
                param.max_abs_pair_correlation,
                IDENTIFIABILITY_CORRELATION_LIMIT
            ),
        });
    }
    Ok(param)
}

/// Evaluates and ranks candidate separating measurements by exact posterior-variance reduction,
/// identifiability resolution (`rank == k` and `max |corr| <= 0.80`), and Gaussian linear-loss
/// Expected Value of Sample Information (`BD-153`, plan §20.6).
pub fn evaluate_measurement_voi(
    request: &MeasurementVoiRequest<'_>,
) -> Result<Vec<MeasurementVoiScore>, ReconError> {
    let k = request.baseline.parameter_count;
    if !(request.unit_error_value_usd.is_finite() && request.unit_error_value_usd >= 0.0) {
        return Err(ReconError::invalid(
            "voi.unit_error_value_usd",
            "unit_error_value_usd must be finite and >= 0",
        ));
    }
    if request.candidates.is_empty() {
        return Err(ReconError::invalid(
            "voi.candidates",
            "at least one candidate measurement is required",
        ));
    }
    let targets: Vec<usize> = if request.target_indices.is_empty() {
        (0..k).collect()
    } else {
        for &idx in &request.target_indices {
            if idx >= k {
                return Err(ReconError::invalid(
                    "voi.target_indices",
                    format!("target index {idx} >= parameter count {k}"),
                ));
            }
        }
        request.target_indices.clone()
    };

    let base_corr_max = max_abs_offdiag_corr(&request.baseline.posterior_covariance, k);
    let base_n_obs = request.baseline.weighted_design.len() / k;
    let mut seen_ids = BTreeSet::new();
    let mut scores = Vec::with_capacity(request.candidates.len());

    for cand in &request.candidates {
        if cand.id.trim().is_empty() || !seen_ids.insert(cand.id.as_str()) {
            return Err(ReconError::invalid(
                "voi.candidates",
                format!("empty or duplicate candidate id '{}'", cand.id),
            ));
        }
        if cand.design_row.len() != k
            || cand.design_row.iter().any(|v| !v.is_finite())
            || cand.design_row.iter().all(|&v| v == 0.0)
        {
            return Err(ReconError::invalid(
                "voi.candidates",
                format!(
                    "candidate '{}' must have finite non-zero design_row of len {k}",
                    cand.id
                ),
            ));
        }
        if !(cand.obs_sd.is_finite()
            && cand.obs_sd > 0.0
            && cand.cost_usd.is_finite()
            && cand.cost_usd >= 0.0)
        {
            return Err(ReconError::invalid(
                "voi.candidates",
                format!(
                    "candidate '{}' requires finite obs_sd > 0 and cost_usd >= 0",
                    cand.id
                ),
            ));
        }

        // Augmented precision: Lambda_new = Lambda_base + (1 / obs_sd^2) * h * h^T.
        let mut aug_prec = request.baseline.posterior_precision.clone();
        let inv_sd = 1.0 / cand.obs_sd;
        let inv_v = inv_sd * inv_sd;
        for i in 0..k {
            for j in 0..k {
                aug_prec[i * k + j] += inv_v * cand.design_row[i] * cand.design_row[j];
            }
        }
        let aug_cov = invert_spd(&aug_prec, k)?;

        // Augmented weighted design for post-measurement rank check.
        let mut aug_design = Vec::with_capacity((base_n_obs + 1) * k);
        aug_design.extend_from_slice(&request.baseline.weighted_design);
        for &h_j in &cand.design_row {
            aug_design.push(h_j * inv_sd);
        }
        let post_rank = decompose_operator_subspaces(&aug_design, base_n_obs + 1, k).rank;
        let post_corr_max = max_abs_offdiag_corr(&aug_cov, k);
        let resolves = post_rank == k && post_corr_max <= IDENTIFIABILITY_CORRELATION_LIMIT;

        let mut vrf_sum = 0.0_f64;
        let mut expected_value_usd = 0.0_f64;
        for &t in &targets {
            let var_before = request.baseline.posterior_covariance[t * k + t].max(0.0);
            let var_after = aug_cov[t * k + t].max(0.0);
            let sd_before = det::sqrt(var_before);
            let sd_after = det::sqrt(var_after);
            if var_before > 0.0 {
                vrf_sum += (1.0 - var_after / var_before).clamp(0.0, 1.0);
            }
            let sd_reduction = (sd_before - sd_after).max(0.0);
            expected_value_usd += SQRT_TWO_OVER_PI * request.unit_error_value_usd * sd_reduction;
        }
        let mean_vrf = vrf_sum / (targets.len() as f64);
        let net_voi_usd = expected_value_usd - cand.cost_usd;

        scores.push(MeasurementVoiScore {
            id: cand.id.clone(),
            kind: cand.kind,
            description: cand.description.clone(),
            post_rank,
            max_abs_corr_before: base_corr_max,
            max_abs_corr_after: post_corr_max,
            resolves_identifiability: resolves,
            mean_variance_reduction_fraction: mean_vrf,
            expected_value_usd,
            cost_usd: cand.cost_usd,
            net_voi_usd,
        });
    }

    scores.sort_by(|a, b| {
        b.resolves_identifiability
            .cmp(&a.resolves_identifiability)
            .then_with(|| b.net_voi_usd.total_cmp(&a.net_voi_usd))
            .then_with(|| a.id.cmp(&b.id))
    });

    Ok(scores)
}
