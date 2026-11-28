//! Hierarchical bias posteriors per node, domain, and mining source (`BD-152`, plan §20.5,
//! `G-TOUCH-5`, `PO-TOUCH-5`).
//! Determinism: DeterministicPerIsa (uses equilibrated SPD Cholesky solves, canonical `BTreeMap`
//! parameter ordering, `fmine_core::det` transcendentals, `PhiloxStream` draws, and BLAKE3 digests).
//!
//! Implements the exact linear-Gaussian hierarchical partial-pooling model:
//! - For each pooling group `g`: group hyperparameter `mu_g ~ N(hyper_mean_g, hyper_sd_g^2)`.
//! - For each node `k` in group `g`: `b_k | mu_g ~ N(mu_g, between_node_sd_g^2)`.
//! - Standalone (unpooled) nodes carry an independent prior `b_k ~ N(prior_mean_k, prior_sd_k^2)`.
//! - Linear observations `y_t = sum_k h_{t,k} b_k + eps_t`, `eps_t ~ N(0, obs_sd_t^2)` update the
//!   joint posterior `(mu, b) | y ~ N(mu_post, Sigma_post)` analytically without approximate
//!   MCMC bias, so Simulation-Based Calibration (`fmine-gen::sbc`) is exact.
//!
//! Enforces §20.5 safeguards:
//! - Guard `G-TOUCH-5`: applying a reconciliation factor directly as a future multiplier is refused
//!   with `FMINE-REF-109`; updates must flow as logged bias-prior events.
//! - Surprise-monitor safeguard: when `surprise_alarm_active` is true, the emitted [`BiasUpdateEvent`]
//!   flags `requires_investigation_case = true` so silent retraining never occurs.

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::refclass::RefClassId;
use fmine_core::codec::Encoder;
use fmine_core::det;
use fmine_core::hash::blake3_hash;
use fmine_core::rand::{PhiloxStream, RngKey};

use crate::error::ReconError;
use crate::linear::solve_equilibrated_spd;

/// Maximum total hierarchical parameters (`groups + nodes`) in one exact joint update.
pub const MAX_HIERARCHICAL_PARAMS: usize = 128;

/// Upstream physical/operational target updated by a reconciliation bias posterior (plan §20.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BiasTargetKind {
    /// In-situ resource vs grade-control vs mill metal bias (Strata L2 `bias_prior`).
    InSituMetal,
    /// Dry bulk density bias (Strata density field / regression card).
    Density,
    /// Haul-truck payload and truck-factor bias (Headframe payload model).
    TruckPayload,
    /// Mill head/concentrate/tails sampler bias (Crucible measurement node).
    MillSampler,
    /// Belt weightometer / weighbridge scale bias (Crucible measurement node).
    Weightometer,
    /// Stockpile volume survey and bulking bias (Headframe survey-epoch model).
    StockpileSurvey,
    /// Unplanned dilution and ore-loss bias (Pushback dig-line / blast-movement models).
    Dilution,
    /// Plant recovery card residual bias (Crucible recovery cards).
    RecoveryCard,
    /// Wet-to-dry moisture bias (Crucible / Headframe moisture model).
    Moisture,
}

impl BiasTargetKind {
    /// Stable canonical string tag.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InSituMetal => "in_situ_metal",
            Self::Density => "density",
            Self::TruckPayload => "truck_payload",
            Self::MillSampler => "mill_sampler",
            Self::Weightometer => "weightometer",
            Self::StockpileSurvey => "stockpile_survey",
            Self::Dilution => "dilution",
            Self::RecoveryCard => "recovery_card",
            Self::Moisture => "moisture",
        }
    }
}

/// Mining or plant source category (supports opposite-sign source-specific biases, e.g. Porgera
/// underground `+36%` oz vs open pit `-8%` oz; plan §20.5, E298).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MiningSourceKind {
    OpenPit,
    Underground,
    Stockpile,
    Plant,
    Shared,
}

impl MiningSourceKind {
    /// Stable canonical string tag.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenPit => "open_pit",
            Self::Underground => "underground",
            Self::Stockpile => "stockpile",
            Self::Plant => "plant",
            Self::Shared => "shared",
        }
    }
}

/// Attempt to apply a reconciliation factor directly as a multiplier to a future estimate (`G-TOUCH-5`).
#[derive(Clone, Debug, PartialEq)]
pub struct FactorMultiplierAttempt {
    /// Factor identifier (e.g. `"F1"`, `"F2"`, `"MCF"`).
    pub factor_name: String,
    /// Proposed multiplier value (e.g. `0.93`).
    pub multiplier: f64,
    /// Target forecast or future estimate context.
    pub target_context: String,
}

/// Guard `G-TOUCH-5` (plan §20.5, §20.11): unconditionally refuses applying a historical F-factor
/// directly as a multiplier to a future estimate; callers must instead fit a hierarchical bias
/// posterior with provenance and evidence cut.
pub fn refuse_factor_multiplier_application(
    attempt: &FactorMultiplierAttempt,
) -> Result<(), ReconError> {
    Err(ReconError::FactorMultiplierRefused {
        detail: format!(
            "cannot apply factor '{}' (multiplier {:.4}) directly to '{}'; publish a hierarchical bias posterior prior with evidence cut instead (§20.5)",
            attempt.factor_name, attempt.multiplier, attempt.target_context
        ),
    })
}

/// Hierarchical pooling group hyperprior: `mu_g ~ N(hyper_mean, hyper_sd^2)`, with member node
/// biases `b_k | mu_g ~ N(mu_g, between_node_sd^2)` (`BD-152`, §20.5).
#[derive(Clone, Debug, PartialEq)]
pub struct HierarchicalGroupPrior {
    /// Unique group identifier (e.g. `"open_pit_domains"`, `"underground_stopes"`).
    pub group_id: String,
    /// Mining source category.
    pub source: MiningSourceKind,
    /// Upstream bias target updated by this group.
    pub target: BiasTargetKind,
    /// Hyperprior mean for group bias `mu_g`.
    pub hyper_mean: f64,
    /// Hyperprior SD (`> 0`) for group bias `mu_g`.
    pub hyper_sd: f64,
    /// Between-node heterogeneity SD (`tau_g > 0`) governing partial pooling strength.
    pub between_node_sd: f64,
    /// Optional reference-class ID from `registries/refclass.toml` backing this prior (`BD-050`).
    pub refclass_id: Option<RefClassId>,
}

/// Standalone independent prior `b_k ~ N(mean, sd^2)` for an unpooled node bias.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StandaloneNodePrior {
    /// Prior mean.
    pub mean: f64,
    /// Prior standard deviation (`> 0`).
    pub std_dev: f64,
}

/// One node/domain bias parameter in a hierarchical reconciliation bias model (`BD-152`).
#[derive(Clone, Debug, PartialEq)]
pub struct NodeBiasSpec {
    /// Unique node/domain bias parameter ID (e.g. `"domain_12_gc"`, `"mill_head_sampler"`).
    pub node_id: String,
    /// Mining source category.
    pub source: MiningSourceKind,
    /// Upstream target updated by this node's posterior.
    pub target: BiasTargetKind,
    /// Pooling group ID if partially pooled; `None` if standalone (requires `standalone_prior`).
    pub group_id: Option<String>,
    /// Independent prior used when `group_id` is `None`; must be `None` when `group_id` is `Some`.
    pub standalone_prior: Option<StandaloneNodePrior>,
    /// Optional reference-class ID backing this node's prior (`BD-050`).
    pub refclass_id: Option<RefClassId>,
}

/// Linear Gaussian observation `sum_k coeff_k * b_k + eps = observed`, `eps ~ N(0, obs_sd^2)`.
#[derive(Clone, Debug, PartialEq)]
pub struct BiasObservation {
    /// Human-readable observation label (e.g. `"2026Q1_F2_domain12"`, `"mill_sampler_audit"`).
    pub label: String,
    /// Period index (0-based or monotonic period sequence).
    pub period_index: u32,
    /// Linear combination of node bias IDs `(node_id, coefficient)`.
    pub terms: Vec<(String, f64)>,
    /// Observed scalar residual / log-ratio / audit value.
    pub observed: f64,
    /// Observation standard deviation (`> 0`).
    pub obs_sd: f64,
}

/// Request to fit exact hierarchical bias posteriors across nodes and pooling groups (`BD-152`).
#[derive(Clone, Debug, PartialEq)]
pub struct HierarchicalBiasRequest {
    /// Evidence cut identifier recorded on the emitted [`BiasUpdateEvent`] (§20.5).
    pub evidence_cut_id: String,
    /// Whether an upstream surprise monitor alarm (`eproc` / CUSUM) is active; when `true`, the
    /// update is flagged `requires_investigation_case = true` so silent retraining never occurs.
    pub surprise_alarm_active: bool,
    /// Credible interval level in `(0, 1)` (e.g. `0.90` for `GATE-TOUCH-02`, `0.95` for `GB-25`).
    pub confidence_level: f64,
    /// Pooling groups (may be empty if all nodes are standalone).
    pub groups: Vec<HierarchicalGroupPrior>,
    /// Node/domain bias parameters (`>= 1`).
    pub nodes: Vec<NodeBiasSpec>,
    /// Linear Gaussian observations over node biases (may be empty to inspect the exact prior).
    pub observations: Vec<BiasObservation>,
}

/// Marginal posterior summary for one group hyperparameter or node bias.
#[derive(Clone, Debug, PartialEq)]
pub struct HierarchicalPosteriorSummary {
    /// Group or node identifier.
    pub id: String,
    /// `true` for a group hyperparameter `mu_g`; `false` for a node bias `b_k`.
    pub is_group_hyper: bool,
    /// Mining source category.
    pub source: MiningSourceKind,
    /// Upstream bias target.
    pub target: BiasTargetKind,
    /// Marginal prior mean.
    pub prior_mean: f64,
    /// Marginal prior standard deviation.
    pub prior_sd: f64,
    /// Posterior mean.
    pub posterior_mean: f64,
    /// Posterior standard deviation.
    pub posterior_sd: f64,
    /// Lower central credible bound at `confidence_level`.
    pub ci_lower: f64,
    /// Upper central credible bound at `confidence_level`.
    pub ci_upper: f64,
    /// `true` iff `[ci_lower, ci_upper]` excludes `0.0`.
    pub excludes_zero: bool,
    /// Fraction of marginal prior variance reduced by the observations: `1 - var_post / var_prior`.
    pub variance_reduction_fraction: f64,
    /// Number of observations directly touching this node (or its member nodes for a group).
    pub direct_observation_count: usize,
}

/// Logged bias-prior update event (plan §20.5 safeguard: no silent retraining).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BiasUpdateEvent {
    /// Evidence cut identifier.
    pub evidence_cut_id: String,
    /// Whether a surprise monitor alarm was active when this update ran.
    pub surprise_alarm_active: bool,
    /// `true` iff this update MUST be routed through an `InvestigationCase` before updating priors.
    pub requires_investigation_case: bool,
    /// Distinct upstream targets touched by this update.
    pub updated_targets: Vec<BiasTargetKind>,
    /// Reference-class IDs cited by group/node priors (`BD-050`).
    pub refclass_ids: Vec<RefClassId>,
    /// BLAKE3 hex digest binding evidence cut, posteriors, and alarm state.
    pub digest_hex: String,
}

/// Complete exact hierarchical bias posterior result (`BD-152`).
#[derive(Clone, Debug, PartialEq)]
pub struct HierarchicalBiasResult {
    /// Canonical group hyperparameter IDs (`len = G`).
    pub group_ids: Vec<String>,
    /// Canonical node bias IDs (`len = K`).
    pub node_ids: Vec<String>,
    /// Marginal summaries for groups (`len = G`, matching `group_ids`).
    pub group_posteriors: Vec<HierarchicalPosteriorSummary>,
    /// Marginal summaries for nodes (`len = K`, matching `node_ids`).
    pub node_posteriors: Vec<HierarchicalPosteriorSummary>,
    /// Joint posterior mean vector `[groups..., nodes...]` (`len = G + K`).
    pub joint_mean: Vec<f64>,
    /// Joint row-major `(G + K) x (G + K)` posterior covariance matrix.
    pub joint_covariance: Vec<f64>,
    /// Joint row-major `(G + K) x (G + K)` posterior precision matrix `Lambda_post`.
    pub joint_precision: Vec<f64>,
    /// Marginal node-only `K x K` posterior covariance block.
    pub node_posterior_covariance: Vec<f64>,
    /// Marginal node-only `K x K` prior covariance block.
    pub node_prior_covariance: Vec<f64>,
    /// Row-major `n_obs x K` observation design matrix over `node_ids` (used by `BD-153`).
    pub observation_design: Vec<f64>,
    /// Observation standard deviations aligned with rows of `observation_design`.
    pub observation_sds: Vec<f64>,
    /// Credible level used for `[ci_lower, ci_upper]`.
    pub confidence_level: f64,
    /// Logged update event carrying evidence cut and surprise-monitor safeguard (§20.5).
    pub update_event: BiasUpdateEvent,
}

impl HierarchicalBiasResult {
    /// Looks up a node posterior summary by `node_id`.
    #[must_use]
    pub fn node_posterior(&self, node_id: &str) -> Option<&HierarchicalPosteriorSummary> {
        self.node_posteriors.iter().find(|p| p.id == node_id)
    }

    /// Looks up a group hyperparameter posterior summary by `group_id`.
    #[must_use]
    pub fn group_posterior(&self, group_id: &str) -> Option<&HierarchicalPosteriorSummary> {
        self.group_posteriors.iter().find(|p| p.id == group_id)
    }
}

/// Request to draw exact deterministic posterior samples from a [`HierarchicalBiasResult`]
/// (e.g. for Talts et al. 2018 SBC evaluation via `fmine-gen::sbc`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PosteriorSampleRequest {
    /// Deterministic `PhiloxStream` key.
    pub seed: RngKey,
    /// Number of posterior draws `L` (`>= 1`).
    pub draw_count: usize,
}

/// Posterior draws aligned with `group_ids` followed by `node_ids`.
#[derive(Clone, Debug, PartialEq)]
pub struct HierarchicalPosteriorDraws {
    /// Parameter IDs (`group_ids` followed by `node_ids`, `len = D`).
    pub parameter_ids: Vec<String>,
    /// Per-parameter draw vectors (`draws_by_param[p].len() == draw_count`).
    pub draws_by_param: Vec<Vec<f64>>,
}

impl HierarchicalPosteriorDraws {
    /// Returns the `L` posterior draws for a named group or node parameter.
    #[must_use]
    pub fn draws_for(&self, id: &str) -> Option<&[f64]> {
        self.parameter_ids
            .iter()
            .position(|p| p == id)
            .map(|idx| self.draws_by_param[idx].as_slice())
    }
}

/// Computes the lower-triangular Cholesky factor `L` (`cov = L * L^T`) with diagonal equilibration
/// so extreme physical scales never fail unscaled pivot checks.
pub(crate) fn cholesky_lower_equilibrated(cov: &[f64], n: usize) -> Result<Vec<f64>, ReconError> {
    if n == 0 || cov.len() != n * n {
        return Err(ReconError::NumericalFailure {
            detail: format!(
                "cholesky factor dimension mismatch: n={n}, len={}",
                cov.len()
            ),
        });
    }
    let mut scale = Vec::with_capacity(n);
    for i in 0..n {
        let diag = cov[i * n + i];
        if !(diag.is_finite() && diag > 0.0) {
            return Err(ReconError::NumericalFailure {
                detail: format!("non-positive variance at index {i}: {diag}"),
            });
        }
        scale.push(det::sqrt(diag));
    }
    let mut lower = vec![0.0_f64; n * n];
    for i in 0..n {
        for j in 0..=i {
            let mut sum = (cov[i * n + j] / scale[i]) / scale[j];
            for k in 0..j {
                sum -= lower[i * n + k] * lower[j * n + k];
            }
            if i == j {
                if !(sum.is_finite() && sum > 1e-14) {
                    return Err(ReconError::NumericalFailure {
                        detail: format!(
                            "posterior covariance not strictly positive definite at pivot {i} ({sum:.3e})"
                        ),
                    });
                }
                lower[i * n + i] = det::sqrt(sum);
            } else {
                lower[i * n + j] = sum / lower[j * n + j];
            }
        }
    }
    // Unscale rows by D^{1/2}: L_phys = D^{1/2} * L_eq so L_phys * L_phys^T = cov.
    for i in 0..n {
        for j in 0..=i {
            lower[i * n + j] *= scale[i];
        }
    }
    Ok(lower)
}

fn checked_var_and_inv_var(sd: f64, context: &str) -> Result<(f64, f64), ReconError> {
    let var = sd * sd;
    if !(var.is_finite() && var > 0.0) {
        return Err(ReconError::NumericalFailure {
            detail: format!("{context} variance overflow/underflow (sd={sd})"),
        });
    }
    let inv_v = 1.0 / var;
    if !(inv_v.is_finite() && inv_v > 0.0) {
        return Err(ReconError::NumericalFailure {
            detail: format!("{context} precision overflow/underflow (sd={sd})"),
        });
    }
    Ok((var, inv_v))
}

#[inline]
fn canonical_bits(v: f64) -> u64 {
    if v == 0.0 {
        0.0_f64.to_bits()
    } else {
        v.to_bits()
    }
}

/// Computes the exact joint hierarchical linear-Gaussian bias posterior across groups and nodes
/// (`BD-152`, plan §20.5).
pub fn fit_hierarchical_bias_posteriors(
    request: &HierarchicalBiasRequest,
) -> Result<HierarchicalBiasResult, ReconError> {
    if request.evidence_cut_id.trim().is_empty() {
        return Err(ReconError::invalid(
            "bias.evidence_cut_id",
            "evidence cut identifier must not be empty",
        ));
    }
    if !(request.confidence_level.is_finite()
        && request.confidence_level > 0.0
        && request.confidence_level < 1.0)
    {
        return Err(ReconError::invalid(
            "bias.confidence_level",
            format!("must lie in (0, 1), got {}", request.confidence_level),
        ));
    }
    if request.nodes.is_empty() {
        return Err(ReconError::invalid(
            "bias.nodes",
            "at least one node bias specification is required",
        ));
    }

    // Canonical BTreeMap ordering for groups and nodes guarantees replay stability.
    let mut group_map: BTreeMap<String, &HierarchicalGroupPrior> = BTreeMap::new();
    for g in &request.groups {
        if g.group_id.trim().is_empty() {
            return Err(ReconError::invalid("bias.groups", "empty group_id"));
        }
        if !(g.hyper_mean.is_finite()
            && g.hyper_sd.is_finite()
            && g.hyper_sd > 0.0
            && g.between_node_sd.is_finite()
            && g.between_node_sd > 0.0)
        {
            return Err(ReconError::invalid(
                "bias.groups",
                format!(
                    "group '{}' requires finite hyper_mean and strictly positive hyper_sd and between_node_sd",
                    g.group_id
                ),
            ));
        }
        if group_map.insert(g.group_id.clone(), g).is_some() {
            return Err(ReconError::invalid(
                "bias.groups",
                format!("duplicate group_id '{}'", g.group_id),
            ));
        }
    }

    let mut node_map: BTreeMap<String, &NodeBiasSpec> = BTreeMap::new();
    let mut group_member_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for node in &request.nodes {
        if node.node_id.trim().is_empty() {
            return Err(ReconError::invalid("bias.nodes", "empty node_id"));
        }
        if group_map.contains_key(&node.node_id) {
            return Err(ReconError::invalid(
                "bias.nodes",
                format!("node_id '{}' collides with a group_id", node.node_id),
            ));
        }
        match (&node.group_id, node.standalone_prior) {
            (Some(gid), None) => {
                let Some(&g) = group_map.get(gid) else {
                    return Err(ReconError::invalid(
                        "bias.nodes",
                        format!("node '{}' references unknown group '{gid}'", node.node_id),
                    ));
                };
                if node.target != g.target {
                    return Err(ReconError::invalid(
                        "bias.nodes",
                        format!(
                            "node '{}' target {:?} does not match pooling group '{gid}' target {:?}",
                            node.node_id, node.target, g.target
                        ),
                    ));
                }
                if node.source != g.source && g.source != MiningSourceKind::Shared {
                    return Err(ReconError::invalid(
                        "bias.nodes",
                        format!(
                            "node '{}' source {:?} is incompatible with non-shared pooling group '{gid}' source {:?}",
                            node.node_id, node.source, g.source
                        ),
                    ));
                }
                *group_member_counts.entry(gid.as_str()).or_insert(0) += 1;
            }
            (Some(_), Some(_)) => {
                return Err(ReconError::invalid(
                    "bias.nodes",
                    format!(
                        "node '{}' cannot specify both group_id and standalone_prior",
                        node.node_id
                    ),
                ));
            }
            (None, Some(sp)) => {
                if !(sp.mean.is_finite() && sp.std_dev.is_finite() && sp.std_dev > 0.0) {
                    return Err(ReconError::invalid(
                        "bias.nodes",
                        format!(
                            "standalone node '{}' requires finite mean and std_dev > 0",
                            node.node_id
                        ),
                    ));
                }
            }
            (None, None) => {
                return Err(ReconError::invalid(
                    "bias.nodes",
                    format!(
                        "node '{}' must belong to a group_id or declare a standalone_prior",
                        node.node_id
                    ),
                ));
            }
        }
        if node_map.insert(node.node_id.clone(), node).is_some() {
            return Err(ReconError::invalid(
                "bias.nodes",
                format!("duplicate node_id '{}'", node.node_id),
            ));
        }
    }

    for gid in group_map.keys() {
        if group_member_counts.get(gid.as_str()).copied().unwrap_or(0) == 0 {
            return Err(ReconError::invalid(
                "bias.groups",
                format!("pooling group '{gid}' has no member nodes"),
            ));
        }
    }

    let group_ids: Vec<String> = group_map.keys().cloned().collect();
    let node_ids: Vec<String> = node_map.keys().cloned().collect();
    let n_groups = group_ids.len();
    let n_nodes = node_ids.len();
    let dim = n_groups + n_nodes;
    if dim > MAX_HIERARCHICAL_PARAMS {
        return Err(ReconError::invalid(
            "bias.dimensions",
            format!("total parameters {dim} exceeds limit {MAX_HIERARCHICAL_PARAMS}"),
        ));
    }

    let group_idx: BTreeMap<&str, usize> = group_ids
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();
    let node_idx: BTreeMap<&str, usize> = node_ids
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();

    // Build exact hierarchical prior precision Lambda_0 and prior RHS r_0 over [groups..., nodes...].
    let mut prior_prec = vec![0.0_f64; dim * dim];
    let mut prior_rhs = vec![0.0_f64; dim];
    let mut prior_means = vec![0.0_f64; dim];
    let mut prior_vars = vec![0.0_f64; dim];

    for (g_i, gid) in group_ids.iter().enumerate() {
        let g = group_map[gid];
        let (var_g, inv_v) =
            checked_var_and_inv_var(g.hyper_sd, &format!("group '{gid}' hyper_sd"))?;
        let rhs_term = inv_v * g.hyper_mean;
        if !rhs_term.is_finite() {
            return Err(ReconError::NumericalFailure {
                detail: format!("group '{gid}' prior RHS overflow"),
            });
        }
        prior_prec[g_i * dim + g_i] += inv_v;
        prior_rhs[g_i] += rhs_term;
        prior_means[g_i] = g.hyper_mean;
        prior_vars[g_i] = var_g;
    }

    for (k_i, nid) in node_ids.iter().enumerate() {
        let slot = n_groups + k_i;
        let node = node_map[nid];
        if let Some(gid) = &node.group_id {
            let g_i = group_idx[gid.as_str()];
            let g = group_map[gid];
            let (tau_var, inv_tau2) = checked_var_and_inv_var(
                g.between_node_sd,
                &format!("group '{gid}' between_node_sd"),
            )?;
            let marginal_var = prior_vars[g_i] + tau_var;
            if !(marginal_var.is_finite() && marginal_var > 0.0) {
                return Err(ReconError::NumericalFailure {
                    detail: format!("node '{nid}' marginal prior variance overflow"),
                });
            }
            // (b_k - mu_g)^2 / tau_g^2
            prior_prec[slot * dim + slot] += inv_tau2;
            prior_prec[g_i * dim + g_i] += inv_tau2;
            prior_prec[slot * dim + g_i] -= inv_tau2;
            prior_prec[g_i * dim + slot] -= inv_tau2;
            if !(prior_prec[slot * dim + slot].is_finite()
                && prior_prec[g_i * dim + g_i].is_finite())
            {
                return Err(ReconError::NumericalFailure {
                    detail: format!("node '{nid}' hierarchical prior precision overflow"),
                });
            }
            prior_means[slot] = g.hyper_mean;
            prior_vars[slot] = marginal_var;
        } else if let Some(sp) = node.standalone_prior {
            let (sp_var, inv_v) =
                checked_var_and_inv_var(sp.std_dev, &format!("node '{nid}' standalone_prior"))?;
            let rhs_term = inv_v * sp.mean;
            if !rhs_term.is_finite() {
                return Err(ReconError::NumericalFailure {
                    detail: format!("node '{nid}' standalone prior RHS overflow"),
                });
            }
            prior_prec[slot * dim + slot] += inv_v;
            prior_rhs[slot] += rhs_term;
            prior_means[slot] = sp.mean;
            prior_vars[slot] = sp_var;
        }
    }

    // Build exact marginal node prior covariance matrix Sigma_{0,nodes} (K x K):
    // Cov(b_i, b_j) = hyper_sd_g^2 if same group, + between_node_sd_g^2 if i == j.
    let mut node_prior_covariance = vec![0.0_f64; n_nodes * n_nodes];
    for i in 0..n_nodes {
        node_prior_covariance[i * n_nodes + i] = prior_vars[n_groups + i];
        let ni = node_map[&node_ids[i]];
        if let Some(gi) = &ni.group_id {
            let g_var = group_map[gi].hyper_sd * group_map[gi].hyper_sd;
            for j in (i + 1)..n_nodes {
                let nj = node_map[&node_ids[j]];
                if nj.group_id.as_deref() == Some(gi.as_str()) {
                    node_prior_covariance[i * n_nodes + j] = g_var;
                    node_prior_covariance[j * n_nodes + i] = g_var;
                }
            }
        }
    }

    // Accumulate exact Gaussian likelihood H^T R^-1 H and H^T R^-1 y.
    let mut post_prec = prior_prec;
    let mut post_rhs = prior_rhs;
    let mut observation_design = Vec::with_capacity(request.observations.len() * n_nodes);
    let mut observation_sds = Vec::with_capacity(request.observations.len());
    let mut node_obs_counts = vec![0usize; n_nodes];

    for obs in &request.observations {
        if obs.label.trim().is_empty() {
            return Err(ReconError::invalid(
                "bias.observations",
                "observation label must not be empty",
            ));
        }
        if !(obs.observed.is_finite() && obs.obs_sd.is_finite() && obs.obs_sd > 0.0) {
            return Err(ReconError::invalid(
                "bias.observations",
                format!(
                    "observation '{}' requires finite value and obs_sd > 0",
                    obs.label
                ),
            ));
        }
        let mut row = vec![0.0_f64; n_nodes];
        for (nid, coeff) in &obs.terms {
            if !coeff.is_finite() {
                return Err(ReconError::invalid(
                    "bias.observations",
                    format!("observation '{}' has non-finite coefficient", obs.label),
                ));
            }
            let Some(&k_i) = node_idx.get(nid.as_str()) else {
                return Err(ReconError::invalid(
                    "bias.observations",
                    format!(
                        "observation '{}' references unknown node '{nid}'",
                        obs.label
                    ),
                ));
            };
            row[k_i] += *coeff;
        }
        if row.iter().all(|&v| v == 0.0) {
            return Err(ReconError::invalid(
                "bias.observations",
                format!("observation '{}' has all-zero net design row", obs.label),
            ));
        }
        let (_, inv_v) =
            checked_var_and_inv_var(obs.obs_sd, &format!("observation '{}'", obs.label))?;
        for i in 0..n_nodes {
            if row[i] != 0.0 {
                node_obs_counts[i] += 1;
                let si = n_groups + i;
                post_rhs[si] += inv_v * row[i] * obs.observed;
                if !post_rhs[si].is_finite() {
                    return Err(ReconError::NumericalFailure {
                        detail: format!("observation '{}' RHS accumulation overflow", obs.label),
                    });
                }
                for j in 0..n_nodes {
                    if row[j] != 0.0 {
                        let sj = n_groups + j;
                        post_prec[si * dim + sj] += inv_v * row[i] * row[j];
                        if !post_prec[si * dim + sj].is_finite() {
                            return Err(ReconError::NumericalFailure {
                                detail: format!(
                                    "observation '{}' precision accumulation overflow",
                                    obs.label
                                ),
                            });
                        }
                    }
                }
            }
        }
        observation_design.extend_from_slice(&row);
        observation_sds.push(obs.obs_sd);
    }

    // Solve exact joint posterior mean & covariance via equilibrated SPD Cholesky.
    let joint_mean = solve_equilibrated_spd(&post_prec, &post_rhs, dim)?;
    let mut joint_covariance = vec![0.0_f64; dim * dim];
    for j in 0..dim {
        let mut ej = vec![0.0_f64; dim];
        ej[j] = 1.0;
        let col = solve_equilibrated_spd(&post_prec, &ej, dim)?;
        for i in 0..dim {
            joint_covariance[i * dim + j] = col[i];
        }
    }
    for i in 0..dim {
        for j in (i + 1)..dim {
            let avg = 0.5 * (joint_covariance[i * dim + j] + joint_covariance[j * dim + i]);
            joint_covariance[i * dim + j] = avg;
            joint_covariance[j * dim + i] = avg;
        }
    }

    let mut node_posterior_covariance = vec![0.0_f64; n_nodes * n_nodes];
    for i in 0..n_nodes {
        for j in 0..n_nodes {
            node_posterior_covariance[i * n_nodes + j] =
                joint_covariance[(n_groups + i) * dim + (n_groups + j)];
        }
    }

    let z_crit = det::norm_ppf(0.5 + 0.5 * request.confidence_level);
    let mut group_obs_counts = vec![0usize; n_groups];
    for (k_i, nid) in node_ids.iter().enumerate() {
        if let Some(gid) = &node_map[nid].group_id {
            group_obs_counts[group_idx[gid.as_str()]] += node_obs_counts[k_i];
        }
    }

    let mut group_posteriors = Vec::with_capacity(n_groups);
    for (g_i, gid) in group_ids.iter().enumerate() {
        let g = group_map[gid];
        let var_post = joint_covariance[g_i * dim + g_i].max(0.0);
        let sd_post = det::sqrt(var_post);
        let lo = joint_mean[g_i] - z_crit * sd_post;
        let hi = joint_mean[g_i] + z_crit * sd_post;
        group_posteriors.push(HierarchicalPosteriorSummary {
            id: gid.clone(),
            is_group_hyper: true,
            source: g.source,
            target: g.target,
            prior_mean: prior_means[g_i],
            prior_sd: det::sqrt(prior_vars[g_i]),
            posterior_mean: joint_mean[g_i],
            posterior_sd: sd_post,
            ci_lower: lo,
            ci_upper: hi,
            excludes_zero: lo > 0.0 || hi < 0.0,
            variance_reduction_fraction: (1.0 - var_post / prior_vars[g_i]).clamp(0.0, 1.0),
            direct_observation_count: group_obs_counts[g_i],
        });
    }

    let mut node_posteriors = Vec::with_capacity(n_nodes);
    for (k_i, nid) in node_ids.iter().enumerate() {
        let slot = n_groups + k_i;
        let node = node_map[nid];
        let var_post = joint_covariance[slot * dim + slot].max(0.0);
        let sd_post = det::sqrt(var_post);
        let lo = joint_mean[slot] - z_crit * sd_post;
        let hi = joint_mean[slot] + z_crit * sd_post;
        node_posteriors.push(HierarchicalPosteriorSummary {
            id: nid.clone(),
            is_group_hyper: false,
            source: node.source,
            target: node.target,
            prior_mean: prior_means[slot],
            prior_sd: det::sqrt(prior_vars[slot]),
            posterior_mean: joint_mean[slot],
            posterior_sd: sd_post,
            ci_lower: lo,
            ci_upper: hi,
            excludes_zero: lo > 0.0 || hi < 0.0,
            variance_reduction_fraction: (1.0 - var_post / prior_vars[slot]).clamp(0.0, 1.0),
            direct_observation_count: node_obs_counts[k_i],
        });
    }

    let mut targets_set = BTreeSet::new();
    let mut refclass_set = BTreeSet::new();
    for g in group_map.values() {
        targets_set.insert(g.target);
        if let Some(rc) = &g.refclass_id {
            refclass_set.insert(rc.clone());
        }
    }
    for n in node_map.values() {
        targets_set.insert(n.target);
        if let Some(rc) = &n.refclass_id {
            refclass_set.insert(rc.clone());
        }
    }
    let updated_targets: Vec<BiasTargetKind> = targets_set.into_iter().collect();
    let refclass_ids: Vec<RefClassId> = refclass_set.into_iter().collect();

    let mut enc = Encoder::new_frame(*b"HBP1", 1);
    enc.put_str(&request.evidence_cut_id);
    enc.put_u8(u8::from(request.surprise_alarm_active));
    enc.put_u32(group_ids.len() as u32);
    for g in &group_ids {
        enc.put_str(g);
    }
    enc.put_u32(node_ids.len() as u32);
    for n in &node_ids {
        enc.put_str(n);
    }
    for &m in &joint_mean {
        enc.put_u64(canonical_bits(m));
    }
    for &c in &joint_covariance {
        enc.put_u64(canonical_bits(c));
    }
    let digest_hex = blake3_hash(&enc.finish()).to_hex();

    let update_event = BiasUpdateEvent {
        evidence_cut_id: request.evidence_cut_id.clone(),
        surprise_alarm_active: request.surprise_alarm_active,
        requires_investigation_case: request.surprise_alarm_active,
        updated_targets,
        refclass_ids,
        digest_hex,
    };

    Ok(HierarchicalBiasResult {
        group_ids,
        node_ids,
        group_posteriors,
        node_posteriors,
        joint_mean,
        joint_covariance,
        joint_precision: post_prec,
        node_posterior_covariance,
        node_prior_covariance,
        observation_design,
        observation_sds,
        confidence_level: request.confidence_level,
        update_event,
    })
}

/// Draws `request.draw_count` exact independent multivariate normal posterior samples
/// `theta^(s) = mu_post + L * z^(s)` from `result` using deterministic `PhiloxStream` Box–Muller
/// (`BD-152`, used for exact Talts et al. 2018 SBC evaluation).
pub fn sample_hierarchical_posterior(
    result: &HierarchicalBiasResult,
    request: &PosteriorSampleRequest,
) -> Result<HierarchicalPosteriorDraws, ReconError> {
    if request.draw_count == 0 {
        return Err(ReconError::invalid(
            "bias.sample.draw_count",
            "draw_count must be >= 1",
        ));
    }
    let dim = result.joint_mean.len();
    if dim == 0
        || result.joint_covariance.len() != dim * dim
        || result.group_ids.len() + result.node_ids.len() != dim
        || result.joint_mean.iter().any(|v| !v.is_finite())
        || result.joint_covariance.iter().any(|v| !v.is_finite())
    {
        return Err(ReconError::NumericalFailure {
            detail: "hierarchical bias result has inconsistent or non-finite joint parameters"
                .to_string(),
        });
    }
    let lower = cholesky_lower_equilibrated(&result.joint_covariance, dim)?;
    let mut parameter_ids = Vec::with_capacity(dim);
    parameter_ids.extend(result.group_ids.iter().cloned());
    parameter_ids.extend(result.node_ids.iter().cloned());

    let mut draws_by_param: Vec<Vec<f64>> = (0..dim)
        .map(|_| Vec::with_capacity(request.draw_count))
        .collect();
    let mut rng = PhiloxStream::new(request.seed);
    let mut z = vec![0.0_f64; dim];

    for _ in 0..request.draw_count {
        for zi in &mut z {
            let u1 = rng.next_f64_open01();
            let u2 = rng.next_f64_open01();
            *zi = det::sqrt(-2.0 * det::ln(u1)) * det::cos(2.0 * std::f64::consts::PI * u2);
        }
        for i in 0..dim {
            let mut val = result.joint_mean[i];
            for j in 0..=i {
                val += lower[i * dim + j] * z[j];
            }
            draws_by_param[i].push(val);
        }
    }

    Ok(HierarchicalPosteriorDraws {
        parameter_ids,
        draws_by_param,
    })
}
