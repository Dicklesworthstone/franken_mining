//! R1 linear weighted-least-squares reconciliation over `fmine_flow::FlowGraph` with full
//! covariance, observable unmeasured variable inference, guards `G-TOUCH-1`/`G-TOUCH-2`/`G-TOUCH-3`,
//! and Bayesian bias identifiability analysis (`BD-149`, plan §20.2–§20.6, §20.12 `GB-24`/`GB-25`).
//! Determinism: DeterministicPerIsa (delegates LDLᵀ reconciliation to `fmine_numerics::reconciliation`
//! and uses canonical `BTreeMap` variable order + BLAKE3 digests).

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::codec::Encoder;
use fmine_core::det;
use fmine_core::hash::blake3_hash;
use fmine_flow::graph::{FlowGraph, NodeClass};
use fmine_numerics::cholesky_solve;
use fmine_numerics::reconciliation::partial::reconcile_with_unmeasured;
use fmine_numerics::reconciliation::reconcile_linear;

use crate::error::ReconError;

/// Posterior correlation threshold above which attribution between two parameters is refused as
/// non-identifiable (`G-TOUCH-2`, plan §20.6: `|corr| > 0.8`).
pub const IDENTIFIABILITY_CORRELATION_LIMIT: f64 = 0.80;

/// Relative conservation closure tolerance audited on every reconciled estimate (`PO-TOUCH-1`).
pub const PO_TOUCH_1_RELATIVE_TOLERANCE: f64 = 1.0e-9;

/// Extensive conserved physical channel reconciled by linear mass/metal balance (`G-TOUCH-1`).
///
/// Intensive grades are deliberately absent: grades are derived after extensive reconciliation
/// (`metal / dry_mass`) and never reconciled directly (`FMINE-REF-103`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExtensiveChannel {
    /// Dry solid mass (e.g. dry metric tonnes).
    DryMass,
    /// Water mass (e.g. tonnes of moisture/slurry water).
    WaterMass,
    /// Contained elemental metal mass (e.g. `"Au"`, `"Cu"`).
    Element(String),
}

impl ExtensiveChannel {
    /// Stable canonical tag for reporting and digest encoding.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::DryMass => "dry_mass".to_string(),
            Self::WaterMass => "water_mass".to_string(),
            Self::Element(el) => format!("element:{el}"),
        }
    }
}

/// Canonical identifier for one extensive state variable in a flow-graph balance.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VariableKey {
    /// Transfer flow along a registered directed stream.
    Stream {
        /// Stream ID in [`FlowGraph`].
        stream_id: String,
        /// Extensive quantity channel.
        channel: ExtensiveChannel,
    },
    /// Net period inventory change (`closing - opening`) at an inventory-holding node.
    InventoryDelta {
        /// Node ID in [`FlowGraph`].
        node_id: String,
        /// Extensive quantity channel.
        channel: ExtensiveChannel,
    },
    /// Generic named extensive variable (for custom multi-channel linear balances).
    Named {
        /// Variable identifier.
        name: String,
        /// Extensive quantity channel.
        channel: ExtensiveChannel,
    },
}

impl VariableKey {
    /// Canonical stable string representation.
    #[must_use]
    pub fn canonical_id(&self) -> String {
        match self {
            Self::Stream { stream_id, channel } => {
                format!("stream:{stream_id}:{}", channel.label())
            }
            Self::InventoryDelta { node_id, channel } => {
                format!("inv_delta:{node_id}:{}", channel.label())
            }
            Self::Named { name, channel } => {
                format!("named:{name}:{}", channel.label())
            }
        }
    }
}

/// One measured extensive variable with its declared 1-sigma measurement standard deviation.
#[derive(Clone, Debug, PartialEq)]
pub struct MeasuredVariable {
    /// Variable identifier.
    pub key: VariableKey,
    /// Observed value (same consistent mass unit across all variables in a channel).
    pub observed: f64,
    /// Declared 1-sigma measurement uncertainty (`> 0`).
    pub std_dev: f64,
}

/// Symmetric off-diagonal covariance entry `Cov(a, b) = Cov(b, a)` between two measured variables.
#[derive(Clone, Debug, PartialEq)]
pub struct CovarianceOffDiagonal {
    /// First variable key.
    pub a: VariableKey,
    /// Second variable key (distinct from `a`).
    pub b: VariableKey,
    /// Covariance value in squared channel units.
    pub covariance: f64,
}

/// Attempt to reconcile intensive grades directly instead of extensive tonnes and metal (`G-TOUCH-1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectGradeReconciliationAttempt {
    /// Name of the stream or circuit where direct grade reconciliation was requested.
    pub context: String,
    /// Element symbol whose grade was submitted directly (e.g. `"Au"`, `"Cu"`).
    pub element: String,
}

/// Guard `G-TOUCH-1` (plan §20.2, §20.11): unconditionally refuses reconciling intensive grades
/// directly (`FMINE-REF-103`). Reconcile extensive dry mass and elemental metal masses instead,
/// then derive grades as `metal / dry_mass`.
pub fn refuse_direct_grade_reconciliation(
    attempt: &DirectGradeReconciliationAttempt,
) -> Result<(), ReconError> {
    Err(ReconError::DirectGradeReconciliationRefused {
        detail: format!(
            "cannot reconcile intensive '{}' grade directly at '{}'; reconcile extensive dry mass and metal mass then derive grade = metal / dry_mass",
            attempt.element, attempt.context
        ),
    })
}

/// Request to run complete-data R1 linear WLS reconciliation for one [`ExtensiveChannel`] over a
/// [`FlowGraph`] (`BD-149`, plan §20.3 R1).
#[derive(Clone, Debug, PartialEq)]
pub struct LinearFlowRequest<'a> {
    /// Validated material-flow graph.
    pub graph: &'a FlowGraph,
    /// Conserved extensive channel to reconcile (`DryMass`, `WaterMass`, or `Element`).
    pub channel: ExtensiveChannel,
    /// Measured stream flows and (for inventory-holding nodes) measured `Δ inventory` observations.
    pub measurements: Vec<MeasuredVariable>,
    /// Optional off-diagonal error covariances among measurements.
    pub off_diagonal_covariances: Vec<CovarianceOffDiagonal>,
    /// If `true`, negative raw stream measurements are refused (inventory deltas may be negative).
    pub enforce_non_negative_streams: bool,
}

/// One linear conservation equation `sum_j coeff_j * x_j = target` over named extensive variables.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedLinearConstraint {
    /// Human-readable balance label (e.g. `"node:mill:dry_mass"`).
    pub label: String,
    /// Non-zero `(variable, coefficient)` terms.
    pub terms: Vec<(VariableKey, f64)>,
    /// Right-hand side target `b` (usually `0.0` for steady conservation).
    pub target: f64,
}

/// Explicit linear extensive balance request (supports multi-channel or coupled extensive balances).
#[derive(Clone, Debug, PartialEq)]
pub struct NamedLinearRequest {
    /// Measured extensive variables.
    pub measurements: Vec<MeasuredVariable>,
    /// Optional off-diagonal error covariances among measurements.
    pub off_diagonal_covariances: Vec<CovarianceOffDiagonal>,
    /// Independent linear balance equations `A x = b`.
    pub constraints: Vec<NamedLinearConstraint>,
}

/// A variable pair whose posterior correlation exceeds `|corr| > 0.8` (`G-TOUCH-2`).
#[derive(Clone, Debug, PartialEq)]
pub struct CorrelatedVariablePair {
    /// First variable.
    pub first: VariableKey,
    /// Second variable.
    pub second: VariableKey,
    /// Posterior Pearson correlation `Cov(x_i, x_j) / (sd_i * sd_j)`.
    pub correlation: f64,
}

/// Complete result of R1 linear WLS reconciliation (`BD-149`, `PO-TOUCH-1`, `PO-TOUCH-2`).
#[derive(Clone, Debug, PartialEq)]
pub struct LinearReconResult {
    /// Variables in deterministic canonical order (`len = n`).
    pub variables: Vec<VariableKey>,
    /// Original observed values `y` (`len = n`).
    pub observed: Vec<f64>,
    /// Declared row-major `n x n` measurement error covariance `V`.
    pub prior_covariance: Vec<f64>,
    /// Row-major `m x n` conservation incidence matrix `A`.
    pub constraints: Vec<f64>,
    /// Labels of the `m` conservation equations.
    pub constraint_names: Vec<String>,
    /// Right-hand side targets `b` (`len = m`).
    pub targets: Vec<f64>,
    /// Reconciled WLS estimates `x_hat` (`len = n`).
    pub estimates: Vec<f64>,
    /// Adjustments `a = x_hat - y` (`len = n`).
    pub adjustments: Vec<f64>,
    /// Row-major `n x n` Joseph-form posterior covariance `cov(x_hat) = V - W`.
    pub posterior_covariance: Vec<f64>,
    /// Marginal posterior standard deviations `sqrt(diag(cov(x_hat)))` (`len = n`).
    pub posterior_std_devs: Vec<f64>,
    /// Row-major `n x n` adjustment covariance `W = V A^T (A V A^T)^-1 A V`.
    pub adjustment_covariance: Vec<f64>,
    /// Pre-reconciliation balance residuals `A y - b` (`len = m`).
    pub raw_residuals: Vec<f64>,
    /// Post-reconciliation balance residuals `A x_hat - b` (`len = m`).
    pub reconciled_residuals: Vec<f64>,
    /// Maximum relative closure error across all constraints (audited `<= 1e-9`, `PO-TOUCH-1`).
    pub max_relative_closure_error: f64,
    /// Global chi-square statistic `(A y - b)^T (A V A^T)^-1 (A y - b)`.
    pub chi_square: f64,
    /// Degrees of freedom `m = rank(A)`.
    pub degrees_of_freedom: usize,
    /// Per-measurement normalized adjustments `a_i / sqrt(W_ii)` (`None` if non-redundant).
    pub normalized_adjustments: Vec<Option<f64>>,
    /// Variable pairs with posterior `|correlation| > 0.8` flagged under `G-TOUCH-2`.
    pub high_posterior_correlations: Vec<CorrelatedVariablePair>,
    /// Replay-stable BLAKE3 hex digest over variables, estimates, posterior covariance, and chi-square (`PO-TOUCH-6`).
    pub digest_hex: String,
}

impl LinearReconResult {
    /// Looks up the index of `key` in `self.variables`.
    #[must_use]
    pub fn index_of(&self, key: &VariableKey) -> Option<usize> {
        self.variables.iter().position(|v| v == key)
    }

    /// Looks up the reconciled estimate for `key`.
    #[must_use]
    pub fn estimate_for(&self, key: &VariableKey) -> Option<f64> {
        self.index_of(key).map(|i| self.estimates[i])
    }

    /// Looks up the marginal posterior standard deviation for `key`.
    #[must_use]
    pub fn posterior_sd_for(&self, key: &VariableKey) -> Option<f64> {
        self.index_of(key).map(|i| self.posterior_std_devs[i])
    }
}

/// Request to reconcile a [`FlowGraph`] with explicitly declared unmeasured (latent) variables
/// (`BD-149`, wired to `fmine_numerics::reconciliation::partial::reconcile_with_unmeasured`).
#[derive(Clone, Debug, PartialEq)]
pub struct PartialFlowRequest<'a> {
    /// Validated material-flow graph.
    pub graph: &'a FlowGraph,
    /// Extensive channel to reconcile.
    pub channel: ExtensiveChannel,
    /// Measured variables with declared uncertainties.
    pub measured: Vec<MeasuredVariable>,
    /// Optional off-diagonal covariances among measured variables.
    pub off_diagonal_covariances: Vec<CovarianceOffDiagonal>,
    /// Explicitly declared unmeasured stream or inventory-delta variables to infer if observable.
    pub unmeasured: Vec<VariableKey>,
    /// If `true`, negative raw stream measurements are refused.
    pub enforce_non_negative_streams: bool,
}

/// Result of partial linear reconciliation inferring observable unmeasured flows (`BD-149`).
#[derive(Clone, Debug, PartialEq)]
pub struct PartialReconResult {
    /// Measured variables in canonical order (`len = n_meas`).
    pub measured_variables: Vec<VariableKey>,
    /// Reconciled estimates of measured variables (`len = n_meas`).
    pub measured_estimates: Vec<f64>,
    /// Adjustments `x_m - y` (`len = n_meas`).
    pub measured_adjustments: Vec<f64>,
    /// Observable unmeasured variables in canonical order (`len = n_latent`).
    pub latent_variables: Vec<VariableKey>,
    /// Inferred estimates of unmeasured variables (`len = n_latent`).
    pub latent_estimates: Vec<f64>,
    /// Joint row-major `(n_meas + n_latent)^2` posterior covariance.
    pub posterior_covariance: Vec<f64>,
    /// Joint marginal posterior standard deviations (`measured` followed by `latent`).
    pub posterior_std_devs: Vec<f64>,
    /// Post-reconciliation residuals `A_m x_m + A_u x_u - b`.
    pub reconciled_residuals: Vec<f64>,
    /// Maximum relative closure error (`<= 1e-9`, `PO-TOUCH-1`).
    pub max_relative_closure_error: f64,
    /// Conditional chi-square objective (`0.0` when `degrees_of_freedom == 0`).
    pub chi_square: f64,
    /// Remaining redundancy `m - n_latent`.
    pub degrees_of_freedom: usize,
    /// Per-measured normalized adjustments.
    pub normalized_adjustments: Vec<Option<f64>>,
    /// High posterior correlation pairs (`|corr| > 0.8`) across measured + latent variables.
    pub high_posterior_correlations: Vec<CorrelatedVariablePair>,
    /// BLAKE3 determinism digest.
    pub digest_hex: String,
}

fn stream_carries_channel(
    stream: &fmine_flow::graph::FlowStream,
    channel: &ExtensiveChannel,
) -> bool {
    match channel {
        ExtensiveChannel::DryMass => true,
        ExtensiveChannel::WaterMass => stream.carries_water,
        ExtensiveChannel::Element(el) => stream.elements.contains(el),
    }
}

struct PreparedCovariance {
    variables: Vec<VariableKey>,
    observed: Vec<f64>,
    covariance: Vec<f64>,
    index_map: BTreeMap<VariableKey, usize>,
}

/// Solves symmetric positive-definite `matrix * x = rhs` with diagonal equilibration
/// (`D^{-1/2} A D^{-1/2}` has unit diagonal), preserving positive definiteness across extreme
/// physical scales (e.g. trace Au metal variance `~1e-16` where unscaled Cholesky rejects).
pub(crate) fn solve_equilibrated_spd(
    matrix: &[f64],
    rhs: &[f64],
    n: usize,
) -> Result<Vec<f64>, ReconError> {
    if n == 0 || matrix.len() != n * n || rhs.len() != n {
        return Err(ReconError::NumericalFailure {
            detail: format!(
                "equilibrated SPD solve dimension mismatch: n={n}, mat={}, rhs={}",
                matrix.len(),
                rhs.len()
            ),
        });
    }
    let mut scale = Vec::with_capacity(n);
    for i in 0..n {
        let diag = matrix[i * n + i];
        if !(diag.is_finite() && diag > 0.0) {
            return Err(ReconError::NumericalFailure {
                detail: format!("non-positive SPD diagonal at index {i}: {diag}"),
            });
        }
        scale.push(det::sqrt(diag));
    }
    let mut eq_mat = vec![0.0_f64; n * n];
    for i in 0..n {
        for j in 0..n {
            let v = (matrix[i * n + j] / scale[i]) / scale[j];
            if !v.is_finite() {
                return Err(ReconError::NumericalFailure {
                    detail: "non-finite entry during SPD equilibration".to_string(),
                });
            }
            eq_mat[i * n + j] = v;
        }
    }
    let mut eq_rhs = Vec::with_capacity(n);
    for i in 0..n {
        let v = rhs[i] / scale[i];
        if !v.is_finite() {
            return Err(ReconError::NumericalFailure {
                detail: "non-finite RHS during SPD equilibration".to_string(),
            });
        }
        eq_rhs.push(v);
    }
    let eq_sol = cholesky_solve(&eq_mat, &eq_rhs, n)?;
    let mut sol = Vec::with_capacity(n);
    for i in 0..n {
        let v = eq_sol[i] / scale[i];
        if !v.is_finite() {
            return Err(ReconError::NumericalFailure {
                detail: "non-finite solution during SPD unscaling".to_string(),
            });
        }
        sol.push(v);
    }
    Ok(sol)
}

fn validate_and_build_covariance(
    measurements: &[MeasuredVariable],
    off_diagonals: &[CovarianceOffDiagonal],
) -> Result<PreparedCovariance, ReconError> {
    if measurements.is_empty() {
        return Err(ReconError::InsufficientRedundancy {
            context: "linear.measurements".to_string(),
            detail: "at least one measured variable is required".to_string(),
        });
    }
    // Canonical sort by VariableKey for replay-stable matrix assembly.
    let mut ordered: BTreeMap<VariableKey, (f64, f64)> = BTreeMap::new();
    for m in measurements {
        if !m.observed.is_finite() {
            return Err(ReconError::invalid(
                "measurement.observed",
                format!(
                    "variable '{}' has non-finite observation",
                    m.key.canonical_id()
                ),
            ));
        }
        if !(m.std_dev.is_finite() && m.std_dev > 0.0) {
            return Err(ReconError::invalid(
                "measurement.std_dev",
                format!(
                    "variable '{}' requires finite std_dev > 0, got {}",
                    m.key.canonical_id(),
                    m.std_dev
                ),
            ));
        }
        if ordered
            .insert(m.key.clone(), (m.observed, m.std_dev))
            .is_some()
        {
            return Err(ReconError::invalid(
                "measurements",
                format!("duplicate variable '{}'", m.key.canonical_id()),
            ));
        }
    }

    let n = ordered.len();
    let mut variables = Vec::with_capacity(n);
    let mut observed = Vec::with_capacity(n);
    let mut cov = vec![0.0_f64; n * n];
    let mut index_map = BTreeMap::new();

    for (idx, (key, (obs, sd))) in ordered.into_iter().enumerate() {
        let var = sd * sd;
        if !var.is_finite() || var <= 0.0 {
            return Err(ReconError::invalid(
                "measurement.variance",
                format!(
                    "variable '{}' variance overflowed/underflowed",
                    key.canonical_id()
                ),
            ));
        }
        cov[idx * n + idx] = var;
        index_map.insert(key.clone(), idx);
        variables.push(key);
        observed.push(obs);
    }

    let mut seen_pairs = BTreeSet::new();
    for od in off_diagonals {
        if !od.covariance.is_finite() {
            return Err(ReconError::invalid(
                "off_diagonal_covariances",
                "covariance must be finite",
            ));
        }
        let Some(&ia) = index_map.get(&od.a) else {
            return Err(ReconError::invalid(
                "off_diagonal_covariances",
                format!("unknown variable '{}'", od.a.canonical_id()),
            ));
        };
        let Some(&ib) = index_map.get(&od.b) else {
            return Err(ReconError::invalid(
                "off_diagonal_covariances",
                format!("unknown variable '{}'", od.b.canonical_id()),
            ));
        };
        if ia == ib {
            return Err(ReconError::invalid(
                "off_diagonal_covariances",
                format!(
                    "off-diagonal covariance cannot reference the same variable '{}'",
                    od.a.canonical_id()
                ),
            ));
        }
        let pair = (ia.min(ib), ia.max(ib));
        if !seen_pairs.insert(pair) {
            return Err(ReconError::invalid(
                "off_diagonal_covariances",
                format!(
                    "duplicate covariance entry for ('{}', '{}')",
                    od.a.canonical_id(),
                    od.b.canonical_id()
                ),
            ));
        }
        cov[ia * n + ib] = od.covariance;
        cov[ib * n + ia] = od.covariance;
    }

    Ok(PreparedCovariance {
        variables,
        observed,
        covariance: cov,
        index_map,
    })
}

fn find_high_correlations(
    variables: &[VariableKey],
    covariance: &[f64],
) -> Vec<CorrelatedVariablePair> {
    let n = variables.len();
    let mut out = Vec::new();
    for i in 0..n {
        let vi = covariance[i * n + i];
        if vi <= 0.0 {
            continue;
        }
        let si = det::sqrt(vi);
        for j in (i + 1)..n {
            let vj = covariance[j * n + j];
            if vj <= 0.0 {
                continue;
            }
            let sj = det::sqrt(vj);
            let corr = (covariance[i * n + j] / si.max(sj)) / si.min(sj);
            if corr.is_finite() && corr.abs() > IDENTIFIABILITY_CORRELATION_LIMIT {
                out.push(CorrelatedVariablePair {
                    first: variables[i].clone(),
                    second: variables[j].clone(),
                    correlation: corr,
                });
            }
        }
    }
    out
}

fn compute_recon_digest(
    tag: [u8; 4],
    variables: &[VariableKey],
    estimates: &[f64],
    posterior_covariance: &[f64],
    chi_square: f64,
) -> String {
    let mut enc = Encoder::new_frame(tag, 1);
    enc.put_u32(variables.len() as u32);
    for v in variables {
        enc.put_str(&v.canonical_id());
    }
    enc.put_u32(estimates.len() as u32);
    for &x in estimates {
        enc.put_u64(x.to_bits());
    }
    enc.put_u32(posterior_covariance.len() as u32);
    for &c in posterior_covariance {
        enc.put_u64(c.to_bits());
    }
    enc.put_u64(chi_square.to_bits());
    blake3_hash(&enc.finish()).to_hex()
}

/// Reconciles an explicit set of named extensive variables and linear conservation equations
/// (`BD-149`, `PO-TOUCH-1`, `PO-TOUCH-2`).
pub fn reconcile_named_system(
    request: &NamedLinearRequest,
) -> Result<LinearReconResult, ReconError> {
    if request.constraints.is_empty() {
        return Err(ReconError::InsufficientRedundancy {
            context: "linear.named_system".to_string(),
            detail: "at least one linear conservation constraint is required".to_string(),
        });
    }
    let PreparedCovariance {
        variables,
        observed,
        covariance: prior_covariance,
        index_map,
    } = validate_and_build_covariance(&request.measurements, &request.off_diagonal_covariances)?;
    let n = variables.len();
    let m = request.constraints.len();

    let mut constraints = vec![0.0_f64; m * n];
    let mut constraint_names = Vec::with_capacity(m);
    let mut targets = Vec::with_capacity(m);

    for (r, c) in request.constraints.iter().enumerate() {
        if c.label.trim().is_empty() {
            return Err(ReconError::invalid(
                "constraint.label",
                "constraint label must not be empty",
            ));
        }
        if !c.target.is_finite() {
            return Err(ReconError::invalid(
                "constraint.target",
                format!("constraint '{}' has non-finite target", c.label),
            ));
        }
        let mut has_nonzero = false;
        for (var, coeff) in &c.terms {
            if !coeff.is_finite() {
                return Err(ReconError::invalid(
                    "constraint.coeff",
                    format!("constraint '{}' has non-finite coefficient", c.label),
                ));
            }
            let Some(&idx) = index_map.get(var) else {
                return Err(ReconError::invalid(
                    "constraint.term",
                    format!(
                        "constraint '{}' references unmeasured/unknown variable '{}'",
                        c.label,
                        var.canonical_id()
                    ),
                ));
            };
            constraints[r * n + idx] += *coeff;
            if constraints[r * n + idx] != 0.0 {
                has_nonzero = true;
            }
        }
        if !has_nonzero {
            return Err(ReconError::invalid(
                "constraint.terms",
                format!("constraint '{}' has all-zero coefficients", c.label),
            ));
        }
        constraint_names.push(c.label.clone());
        targets.push(c.target);
    }

    let sol = reconcile_linear(&observed, &prior_covariance, &constraints, &targets)?;

    // Audit PO-TOUCH-1: relative balance closure <= 1e-9 on every row (no unit floor so trace metals are audited strictly).
    let mut max_rel_err = 0.0_f64;
    for r in 0..m {
        let row = &constraints[r * n..(r + 1) * n];
        let scale = row
            .iter()
            .zip(&sol.estimates)
            .fold(targets[r].abs(), |acc, (&a, &x)| acc + (a * x).abs());
        let rel = if scale > 0.0 {
            sol.reconciled_residuals[r].abs() / scale
        } else {
            sol.reconciled_residuals[r].abs()
        };
        if rel > max_rel_err {
            max_rel_err = rel;
        }
        if !rel.is_finite() || rel > PO_TOUCH_1_RELATIVE_TOLERANCE {
            return Err(ReconError::NumericalFailure {
                detail: format!(
                    "PO-TOUCH-1 closure audit failed on '{}': relative residual {rel:.3e} > {:.1e}",
                    constraint_names[r], PO_TOUCH_1_RELATIVE_TOLERANCE
                ),
            });
        }
    }

    let posterior_std_devs: Vec<f64> = (0..n)
        .map(|i| det::sqrt(sol.posterior_covariance[i * n + i].max(0.0)))
        .collect();
    let high_posterior_correlations = find_high_correlations(&variables, &sol.posterior_covariance);
    let digest_hex = compute_recon_digest(
        *b"RLIN",
        &variables,
        &sol.estimates,
        &sol.posterior_covariance,
        sol.chi_square,
    );

    Ok(LinearReconResult {
        variables,
        observed,
        prior_covariance,
        constraints,
        constraint_names,
        targets,
        estimates: sol.estimates,
        adjustments: sol.adjustments,
        posterior_covariance: sol.posterior_covariance,
        posterior_std_devs,
        adjustment_covariance: sol.adjustment_covariance,
        raw_residuals: sol.raw_residuals,
        reconciled_residuals: sol.reconciled_residuals,
        max_relative_closure_error: max_rel_err,
        chi_square: sol.chi_square,
        degrees_of_freedom: sol.degrees_of_freedom,
        normalized_adjustments: sol.normalized_adjustments,
        high_posterior_correlations,
        digest_hex,
    })
}

fn validate_flow_measurement_keys(
    graph: &FlowGraph,
    channel: &ExtensiveChannel,
    measurements: &[MeasuredVariable],
    enforce_non_negative_streams: bool,
) -> Result<BTreeSet<VariableKey>, ReconError> {
    let mut keys = BTreeSet::new();
    for m in measurements {
        match &m.key {
            VariableKey::Stream {
                stream_id,
                channel: ch,
            } => {
                if ch != channel {
                    return Err(ReconError::invalid(
                        "measurements",
                        format!(
                            "stream '{}' channel '{}' does not match requested '{}'",
                            stream_id,
                            ch.label(),
                            channel.label()
                        ),
                    ));
                }
                let Some(stream) = graph.stream(stream_id) else {
                    return Err(ReconError::invalid(
                        "measurements",
                        format!("unknown stream '{stream_id}' in FlowGraph"),
                    ));
                };
                if !stream_carries_channel(stream, channel) {
                    return Err(ReconError::invalid(
                        "measurements",
                        format!(
                            "stream '{stream_id}' does not carry channel '{}'",
                            channel.label()
                        ),
                    ));
                }
                if enforce_non_negative_streams && m.observed < 0.0 {
                    return Err(ReconError::invalid(
                        "measurement.observed",
                        format!(
                            "stream '{stream_id}' flow {} cannot be negative",
                            m.observed
                        ),
                    ));
                }
            }
            VariableKey::InventoryDelta {
                node_id,
                channel: ch,
            } => {
                if ch != channel {
                    return Err(ReconError::invalid(
                        "measurements",
                        format!(
                            "inventory delta '{}' channel '{}' does not match requested '{}'",
                            node_id,
                            ch.label(),
                            channel.label()
                        ),
                    ));
                }
                let Some(node) = graph.node(node_id) else {
                    return Err(ReconError::invalid(
                        "measurements",
                        format!("unknown node '{node_id}' in FlowGraph"),
                    ));
                };
                if !node.holds_inventory {
                    return Err(ReconError::invalid(
                        "measurements",
                        format!("node '{node_id}' does not hold inventory"),
                    ));
                }
            }
            VariableKey::Named { name, .. } => {
                return Err(ReconError::invalid(
                    "measurements",
                    format!(
                        "flow-graph reconciliation requires Stream or InventoryDelta keys, got Named '{name}'"
                    ),
                ));
            }
        }
        keys.insert(m.key.clone());
    }
    Ok(keys)
}

/// Runs complete-data R1 linear WLS reconciliation for one [`ExtensiveChannel`] over every
/// interior conservation node of `request.graph` (`BD-149`, `PO-TOUCH-1`, `PO-TOUCH-2`, `G-TOUCH-3`).
pub fn reconcile_flow_graph(
    request: &LinearFlowRequest<'_>,
) -> Result<LinearReconResult, ReconError> {
    let measured_keys = validate_flow_measurement_keys(
        request.graph,
        &request.channel,
        &request.measurements,
        request.enforce_non_negative_streams,
    )?;

    let mut constraints = Vec::new();
    for node in request.graph.interior_nodes() {
        let incoming: Vec<_> = request
            .graph
            .incoming_streams(&node.id)
            .into_iter()
            .filter(|s| stream_carries_channel(s, &request.channel))
            .collect();
        let outgoing: Vec<_> = request
            .graph
            .outgoing_streams(&node.id)
            .into_iter()
            .filter(|s| stream_carries_channel(s, &request.channel))
            .collect();
        if incoming.is_empty() && outgoing.is_empty() {
            continue;
        }
        let mut terms = Vec::new();
        for s in incoming {
            let key = VariableKey::Stream {
                stream_id: s.id.clone(),
                channel: request.channel.clone(),
            };
            if !measured_keys.contains(&key) {
                return Err(ReconError::invalid(
                    "measurements",
                    format!(
                        "missing measurement for incoming stream '{}' at node '{}' (use reconcile_partial_flow_graph for unmeasured streams)",
                        s.id, node.id
                    ),
                ));
            }
            terms.push((key, 1.0));
        }
        for s in outgoing {
            let key = VariableKey::Stream {
                stream_id: s.id.clone(),
                channel: request.channel.clone(),
            };
            if !measured_keys.contains(&key) {
                return Err(ReconError::invalid(
                    "measurements",
                    format!(
                        "missing measurement for outgoing stream '{}' at node '{}' (use reconcile_partial_flow_graph for unmeasured streams)",
                        s.id, node.id
                    ),
                ));
            }
            terms.push((key, -1.0));
        }
        let inv_key = VariableKey::InventoryDelta {
            node_id: node.id.clone(),
            channel: request.channel.clone(),
        };
        if measured_keys.contains(&inv_key) {
            terms.push((inv_key, -1.0));
        } else if node.class == NodeClass::StockpileOrPad {
            return Err(ReconError::StockpileDecouplingRefused {
                detail: format!(
                    "stockpile node '{}' requires an explicit InventoryDelta observation (cannot assume Δ inventory = 0)",
                    node.id
                ),
            });
        }
        constraints.push(NamedLinearConstraint {
            label: format!("node:{}:{}", node.id, request.channel.label()),
            terms,
            target: 0.0,
        });
    }

    if constraints.is_empty() {
        return Err(ReconError::InsufficientRedundancy {
            context: "linear.flow_graph".to_string(),
            detail: format!(
                "no interior balance equations found for channel '{}'",
                request.channel.label()
            ),
        });
    }

    reconcile_named_system(&NamedLinearRequest {
        measurements: request.measurements.clone(),
        off_diagonal_covariances: request.off_diagonal_covariances.clone(),
        constraints,
    })
}

/// Reconciles a [`FlowGraph`] when one or more streams or inventory deltas are explicitly
/// unmeasured, inferring them strictly when algebraically observable and refusing unobservable
/// systems with `FMINE-REF-104` (`BD-149`, wired to `fmine_numerics::reconciliation::partial`).
pub fn reconcile_partial_flow_graph(
    request: &PartialFlowRequest<'_>,
) -> Result<PartialReconResult, ReconError> {
    if request.unmeasured.is_empty() {
        return Err(ReconError::invalid(
            "partial.unmeasured",
            "at least one unmeasured variable is required; use reconcile_flow_graph for complete data",
        ));
    }

    let measured_keys = validate_flow_measurement_keys(
        request.graph,
        &request.channel,
        &request.measured,
        request.enforce_non_negative_streams,
    )?;

    let mut latent_set = BTreeSet::new();
    for key in &request.unmeasured {
        if measured_keys.contains(key) {
            return Err(ReconError::invalid(
                "partial.unmeasured",
                format!(
                    "variable '{}' cannot be both measured and unmeasured",
                    key.canonical_id()
                ),
            ));
        }
        match key {
            VariableKey::Stream {
                stream_id,
                channel: ch,
            } => {
                if ch != &request.channel {
                    return Err(ReconError::invalid(
                        "partial.unmeasured",
                        format!("unmeasured stream '{stream_id}' channel mismatch"),
                    ));
                }
                let Some(s) = request.graph.stream(stream_id) else {
                    return Err(ReconError::invalid(
                        "partial.unmeasured",
                        format!("unknown unmeasured stream '{stream_id}'"),
                    ));
                };
                if !stream_carries_channel(s, &request.channel) {
                    return Err(ReconError::invalid(
                        "partial.unmeasured",
                        format!(
                            "unmeasured stream '{stream_id}' does not carry '{}'",
                            request.channel.label()
                        ),
                    ));
                }
            }
            VariableKey::InventoryDelta {
                node_id,
                channel: ch,
            } => {
                if ch != &request.channel {
                    return Err(ReconError::invalid(
                        "partial.unmeasured",
                        format!("unmeasured inventory '{node_id}' channel mismatch"),
                    ));
                }
                let Some(node) = request.graph.node(node_id) else {
                    return Err(ReconError::invalid(
                        "partial.unmeasured",
                        format!("unknown node '{node_id}'"),
                    ));
                };
                if !node.holds_inventory {
                    return Err(ReconError::invalid(
                        "partial.unmeasured",
                        format!("node '{node_id}' does not hold inventory"),
                    ));
                }
            }
            VariableKey::Named { name, .. } => {
                return Err(ReconError::invalid(
                    "partial.unmeasured",
                    format!("expected Stream or InventoryDelta key, got Named '{name}'"),
                ));
            }
        }
        if !latent_set.insert(key.clone()) {
            return Err(ReconError::invalid(
                "partial.unmeasured",
                format!("duplicate unmeasured variable '{}'", key.canonical_id()),
            ));
        }
    }

    let PreparedCovariance {
        variables: measured_variables,
        observed,
        covariance: prior_cov,
        index_map: meas_idx,
    } = validate_and_build_covariance(&request.measured, &request.off_diagonal_covariances)?;
    let latent_variables: Vec<VariableKey> = latent_set.into_iter().collect();
    let latent_idx: BTreeMap<VariableKey, usize> = latent_variables
        .iter()
        .enumerate()
        .map(|(i, k)| (k.clone(), i))
        .collect();

    let n = measured_variables.len();
    let p = latent_variables.len();

    let mut rows_am: Vec<Vec<f64>> = Vec::new();
    let mut rows_au: Vec<Vec<f64>> = Vec::new();
    for node in request.graph.interior_nodes() {
        let incoming: Vec<_> = request
            .graph
            .incoming_streams(&node.id)
            .into_iter()
            .filter(|s| stream_carries_channel(s, &request.channel))
            .collect();
        let outgoing: Vec<_> = request
            .graph
            .outgoing_streams(&node.id)
            .into_iter()
            .filter(|s| stream_carries_channel(s, &request.channel))
            .collect();
        if incoming.is_empty() && outgoing.is_empty() {
            continue;
        }
        let mut am_row = vec![0.0_f64; n];
        let mut au_row = vec![0.0_f64; p];

        let mut place = |key: VariableKey, coeff: f64| -> Result<(), ReconError> {
            if let Some(&i) = meas_idx.get(&key) {
                am_row[i] += coeff;
                Ok(())
            } else if let Some(&j) = latent_idx.get(&key) {
                au_row[j] += coeff;
                Ok(())
            } else {
                Err(ReconError::invalid(
                    "partial.measurements",
                    format!(
                        "variable '{}' at node '{}' is neither measured nor declared unmeasured",
                        key.canonical_id(),
                        node.id
                    ),
                ))
            }
        };

        for s in incoming {
            place(
                VariableKey::Stream {
                    stream_id: s.id.clone(),
                    channel: request.channel.clone(),
                },
                1.0,
            )?;
        }
        for s in outgoing {
            place(
                VariableKey::Stream {
                    stream_id: s.id.clone(),
                    channel: request.channel.clone(),
                },
                -1.0,
            )?;
        }
        let inv_key = VariableKey::InventoryDelta {
            node_id: node.id.clone(),
            channel: request.channel.clone(),
        };
        if meas_idx.contains_key(&inv_key) || latent_idx.contains_key(&inv_key) {
            place(inv_key, -1.0)?;
        } else if node.class == NodeClass::StockpileOrPad {
            return Err(ReconError::StockpileDecouplingRefused {
                detail: format!(
                    "stockpile node '{}' must declare its InventoryDelta as measured or unmeasured",
                    node.id
                ),
            });
        }
        rows_am.push(am_row);
        rows_au.push(au_row);
    }

    let m = rows_am.len();
    if m == 0 {
        return Err(ReconError::InsufficientRedundancy {
            context: "linear.partial_flow_graph".to_string(),
            detail: "no interior conservation equations found".to_string(),
        });
    }

    let mut am_flat = Vec::with_capacity(m * n);
    let mut au_flat = Vec::with_capacity(m * p);
    for r in 0..m {
        am_flat.extend_from_slice(&rows_am[r]);
        au_flat.extend_from_slice(&rows_au[r]);
    }
    let targets = vec![0.0_f64; m];

    let sol = reconcile_with_unmeasured(&observed, &prior_cov, &am_flat, &au_flat, &targets, p)?;

    let mut joint_vars = measured_variables.clone();
    joint_vars.extend(latent_variables.iter().cloned());
    let mut joint_est = sol.measured_estimates.clone();
    joint_est.extend_from_slice(&sol.latent_estimates);

    let states = n + p;
    let mut max_rel_err = 0.0_f64;
    for r in 0..m {
        let mut scale = 0.0_f64;
        for i in 0..n {
            scale += (rows_am[r][i] * sol.measured_estimates[i]).abs();
        }
        for j in 0..p {
            scale += (rows_au[r][j] * sol.latent_estimates[j]).abs();
        }
        let rel = if scale > 0.0 {
            sol.reconciled_residuals[r].abs() / scale
        } else {
            sol.reconciled_residuals[r].abs()
        };
        if rel > max_rel_err {
            max_rel_err = rel;
        }
        if !rel.is_finite() || rel > PO_TOUCH_1_RELATIVE_TOLERANCE {
            return Err(ReconError::NumericalFailure {
                detail: format!(
                    "PO-TOUCH-1 partial balance closure failed: relative residual {rel:.3e}"
                ),
            });
        }
    }

    let posterior_std_devs: Vec<f64> = (0..states)
        .map(|i| det::sqrt(sol.posterior_covariance[i * states + i].max(0.0)))
        .collect();
    let high_posterior_correlations =
        find_high_correlations(&joint_vars, &sol.posterior_covariance);
    let digest_hex = compute_recon_digest(
        *b"RPRT",
        &joint_vars,
        &joint_est,
        &sol.posterior_covariance,
        sol.chi_square,
    );

    Ok(PartialReconResult {
        measured_variables,
        measured_estimates: sol.measured_estimates,
        measured_adjustments: sol.measured_adjustments,
        latent_variables,
        latent_estimates: sol.latent_estimates,
        posterior_covariance: sol.posterior_covariance,
        posterior_std_devs,
        reconciled_residuals: sol.reconciled_residuals,
        max_relative_closure_error: max_rel_err,
        chi_square: sol.chi_square,
        degrees_of_freedom: sol.degrees_of_freedom,
        normalized_adjustments: sol.normalized_adjustments,
        high_posterior_correlations,
        digest_hex,
    })
}

// ============================================================================
// Guard G-TOUCH-2: Linear-Gaussian Bias Identifiability Analysis (GB-25, §20.6)
// ============================================================================

/// Prior `b_i ~ N(prior_mean, prior_sd^2)` on one candidate bias parameter (`§20.6`, `GB-25`).
#[derive(Clone, Debug, PartialEq)]
pub struct BiasPriorSpec {
    /// Parameter name (e.g. `"b_gc"`, `"b_mill"`).
    pub name: String,
    /// Prior mean (typically `0.0`).
    pub prior_mean: f64,
    /// Prior standard deviation (`> 0`).
    pub prior_sd: f64,
}

/// Linear Gaussian observation `sum_j h_j * b_j + eps = observed`, `eps ~ N(0, obs_sd^2)`.
#[derive(Clone, Debug, PartialEq)]
pub struct BiasLinearObservation {
    /// Observation label (e.g. `"F2_log_ratio"`, `"mill_sampler_audit"`).
    pub label: String,
    /// Design vector `h` aligned with `priors` (`len == priors.len()`).
    pub design_row: Vec<f64>,
    /// Observed scalar value.
    pub observed: f64,
    /// Measurement standard deviation (`> 0`).
    pub obs_sd: f64,
}

/// Request for Bayesian linear-Gaussian bias identifiability & attribution analysis (`G-TOUCH-2`, `GB-25`).
#[derive(Clone, Debug, PartialEq)]
pub struct BiasIdentifiabilityRequest {
    /// Candidate bias parameters with independent Gaussian priors.
    pub priors: Vec<BiasPriorSpec>,
    /// Linear Gaussian observations (ratios, audits, check-assays, resurveys).
    pub observations: Vec<BiasLinearObservation>,
    /// Optional linear contrast `c^T b` that IS determined even when individuals are confounded
    /// (e.g. `[1.0, -1.0]` for `b_gc - b_mill` in `GB-25`).
    pub determined_contrast: Option<(String, Vec<f64>)>,
    /// Confidence level for marginal posterior intervals (default `0.95`).
    pub confidence_level: f64,
}

/// Marginal posterior estimate of one bias parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct BiasPosteriorEstimate {
    /// Parameter name.
    pub name: String,
    /// Posterior mean.
    pub posterior_mean: f64,
    /// Posterior standard deviation.
    pub posterior_sd: f64,
    /// Lower normal quantile bound at `confidence_level`.
    pub ci_lower: f64,
    /// Upper normal quantile bound at `confidence_level`.
    pub ci_upper: f64,
    /// `true` when `[ci_lower, ci_upper]` excludes `0.0`.
    pub excludes_zero: bool,
    /// Fraction of prior variance reduced by the observations: `1 - var_post / var_prior`.
    pub variance_reduction_fraction: f64,
}

/// Posterior estimate of a determined linear contrast `c^T b`.
#[derive(Clone, Debug, PartialEq)]
pub struct ContrastEstimate {
    /// Contrast label (e.g. `"b_gc - b_mill"`).
    pub label: String,
    /// Posterior mean `c^T mu_post`.
    pub mean: f64,
    /// Posterior SD `sqrt(c^T Sigma_post c)`.
    pub sd: f64,
}

/// Result of Bayesian linear-Gaussian bias identifiability analysis (`G-TOUCH-2`, `GB-25`).
#[derive(Clone, Debug, PartialEq)]
pub struct BiasIdentifiabilityReport {
    /// Marginal posteriors for all parameters.
    pub estimates: Vec<BiasPosteriorEstimate>,
    /// Full row-major posterior covariance matrix `Sigma_post`.
    pub posterior_covariance: Vec<f64>,
    /// Pairs `(name_i, name_j, corr)` whose `|posterior_corr| > 0.8`.
    pub confounded_pairs: Vec<(String, String, f64)>,
    /// Optional determined contrast `c^T b` (always reported even when individuals are confounded).
    pub determined_contrast: Option<ContrastEstimate>,
    /// `true` iff no pair exceeds `|corr| > 0.8` (individual attribution is admissible).
    pub attribution_admissible: bool,
}

/// Computes the exact linear-Gaussian posterior for bias parameters under independent priors and
/// linear observations, enforcing guard `G-TOUCH-2` (`plan §20.6`, `GB-25`).
///
/// When `refuse_if_confounded` is enforced by callers (via [`require_identifiable_attribution`]),
/// any pair with `|posterior_corr| > 0.8` refuses attribution with `FMINE-REF-104` while still
/// exposing the determined linear contrast (`b_gc - b_mill`).
pub fn analyze_bias_identifiability(
    request: &BiasIdentifiabilityRequest,
) -> Result<BiasIdentifiabilityReport, ReconError> {
    let k = request.priors.len();
    if k == 0 || k > 64 {
        return Err(ReconError::invalid(
            "bias.priors",
            format!("require 1..=64 bias parameters, got {k}"),
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
    if request.observations.is_empty() {
        return Err(ReconError::InsufficientRedundancy {
            context: "bias.identifiability".to_string(),
            detail: "at least one observation is required".to_string(),
        });
    }

    let mut names = BTreeSet::new();
    let mut prec = vec![0.0_f64; k * k];
    let mut rhs = vec![0.0_f64; k];
    for (i, p) in request.priors.iter().enumerate() {
        if p.name.trim().is_empty() || !names.insert(p.name.as_str()) {
            return Err(ReconError::invalid(
                "bias.priors.name",
                format!("empty or duplicate bias name '{}'", p.name),
            ));
        }
        if !(p.prior_mean.is_finite() && p.prior_sd.is_finite() && p.prior_sd > 0.0) {
            return Err(ReconError::invalid(
                "bias.priors",
                format!("invalid prior for '{}'", p.name),
            ));
        }
        let inv_var = 1.0 / (p.prior_sd * p.prior_sd);
        prec[i * k + i] = inv_var;
        rhs[i] = inv_var * p.prior_mean;
    }

    for obs in &request.observations {
        if obs.design_row.len() != k {
            return Err(ReconError::invalid(
                "bias.observations",
                format!(
                    "observation '{}' design_row length {} != {k}",
                    obs.label,
                    obs.design_row.len()
                ),
            ));
        }
        if !(obs.observed.is_finite()
            && obs.obs_sd.is_finite()
            && obs.obs_sd > 0.0
            && obs.design_row.iter().all(|v| v.is_finite()))
        {
            return Err(ReconError::invalid(
                "bias.observations",
                format!(
                    "non-finite observation or non-positive obs_sd in '{}'",
                    obs.label
                ),
            ));
        }
        let w = 1.0 / (obs.obs_sd * obs.obs_sd);
        for i in 0..k {
            rhs[i] += w * obs.design_row[i] * obs.observed;
            for j in 0..k {
                prec[i * k + j] += w * obs.design_row[i] * obs.design_row[j];
            }
        }
    }

    // Posterior mean mu = Prec^-1 rhs; posterior covariance Sigma = Prec^-1 (equilibrated SPD solve).
    let mu = solve_equilibrated_spd(&prec, &rhs, k)?;
    let mut post_cov = vec![0.0_f64; k * k];
    for j in 0..k {
        let mut ej = vec![0.0_f64; k];
        ej[j] = 1.0;
        let col = solve_equilibrated_spd(&prec, &ej, k)?;
        for i in 0..k {
            post_cov[i * k + j] = col[i];
        }
    }
    // Symmetrize floating-point rounding.
    for i in 0..k {
        for j in (i + 1)..k {
            let avg = 0.5 * (post_cov[i * k + j] + post_cov[j * k + i]);
            post_cov[i * k + j] = avg;
            post_cov[j * k + i] = avg;
        }
    }

    let z_crit = det::norm_ppf(0.5 + 0.5 * request.confidence_level);
    let mut estimates = Vec::with_capacity(k);
    for (i, p) in request.priors.iter().enumerate() {
        let var_post = post_cov[i * k + i].max(0.0);
        let sd_post = det::sqrt(var_post);
        let lo = mu[i] - z_crit * sd_post;
        let hi = mu[i] + z_crit * sd_post;
        let var_prior = p.prior_sd * p.prior_sd;
        estimates.push(BiasPosteriorEstimate {
            name: p.name.clone(),
            posterior_mean: mu[i],
            posterior_sd: sd_post,
            ci_lower: lo,
            ci_upper: hi,
            excludes_zero: lo > 0.0 || hi < 0.0,
            variance_reduction_fraction: (1.0 - var_post / var_prior).clamp(0.0, 1.0),
        });
    }

    let mut confounded_pairs = Vec::new();
    for i in 0..k {
        let si = estimates[i].posterior_sd;
        if si <= 0.0 {
            continue;
        }
        for j in (i + 1)..k {
            let sj = estimates[j].posterior_sd;
            if sj <= 0.0 {
                continue;
            }
            let corr = post_cov[i * k + j] / (si * sj);
            if corr.abs() > IDENTIFIABILITY_CORRELATION_LIMIT {
                confounded_pairs.push((
                    request.priors[i].name.clone(),
                    request.priors[j].name.clone(),
                    corr,
                ));
            }
        }
    }

    let determined_contrast = if let Some((label, c)) = &request.determined_contrast {
        if c.len() != k || c.iter().any(|v| !v.is_finite()) {
            return Err(ReconError::invalid(
                "bias.determined_contrast",
                format!("contrast '{label}' must have length {k} with finite entries"),
            ));
        }
        let mean: f64 = c.iter().zip(&mu).map(|(&ci, &mi)| ci * mi).sum();
        let mut var = 0.0_f64;
        for i in 0..k {
            for j in 0..k {
                var += c[i] * post_cov[i * k + j] * c[j];
            }
        }
        Some(ContrastEstimate {
            label: label.clone(),
            mean,
            sd: det::sqrt(var.max(0.0)),
        })
    } else {
        None
    };

    let attribution_admissible = confounded_pairs.is_empty();
    Ok(BiasIdentifiabilityReport {
        estimates,
        posterior_covariance: post_cov,
        confounded_pairs,
        determined_contrast,
        attribution_admissible,
    })
}

/// Guard `G-TOUCH-2` (plan §20.6, `GB-25`): refuses individual bias attribution with
/// `FMINE-REF-104` when any parameter pair has `|posterior_corr| > 0.8`.
pub fn require_identifiable_attribution(
    report: &BiasIdentifiabilityReport,
) -> Result<(), ReconError> {
    if let Some((a, b, corr)) = report.confounded_pairs.first() {
        let contrast_note = match &report.determined_contrast {
            Some(c) => format!(
                "; determined contrast '{}' = {:.5} ± {:.5}",
                c.label, c.mean, c.sd
            ),
            None => String::new(),
        };
        return Err(ReconError::NonIdentifiable {
            detail: format!(
                "bias pair ('{a}', '{b}') has posterior correlation {corr:.3} (|corr| > {:.2}); individual attribution refused{contrast_note}",
                IDENTIFIABILITY_CORRELATION_LIMIT
            ),
        });
    }
    Ok(())
}
