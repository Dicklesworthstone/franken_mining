//! Gross-error detection tests (global chi-square and per-measurement normalized adjustment),
//! commit gating (`FMINE-REF-101`), exact analytical detection power curves, and single-period /
//! quarterly Minimum Detectable Bias (`BD-150`; plan §20.2, §20.3, §20.6).
//! Determinism: DeterministicPerIsa (uses `fmine_numerics::cholesky_solve` for `(A V A^T)^-1`
//! and `fmine_core::det` special functions).

use std::collections::BTreeMap;

use fmine_core::det;

use crate::error::ReconError;
use crate::linear::{LinearReconResult, VariableKey, solve_equilibrated_spd};
use crate::stats::{
    chi_square_critical_upper, chi_square_upper_tail, noncentral_chi_square_ncp_for_power,
    noncentral_chi_square_upper_tail, normal_two_sided_ncp_for_power, normal_two_sided_p,
    normal_two_sided_power,
};

/// Multiplicity correction applied across redundant per-measurement tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultiplicityCorrection {
    /// Unadjusted per-measurement significance level `alpha`.
    None,
    /// Šidák family-wise error control: `alpha_per = 1 - (1 - alpha)^(1 / k)`.
    Sidak,
    /// Bonferroni family-wise error control: `alpha_per = alpha / k`.
    Bonferroni,
}

/// Policy governing gross-error detection, commit gating, and MDB power publication (`BD-150`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrossErrorPolicy {
    /// Significance level `α_global` for the global chi-square test (default `0.01`, plan `GB-24`).
    pub global_alpha: f64,
    /// Family-wise significance level `α_meas` for per-measurement normalized adjustments (default `0.05`).
    pub measurement_alpha: f64,
    /// Multiplicity adjustment across redundant measurement tests (default `Sidak`).
    pub multiplicity: MultiplicityCorrection,
    /// Target statistical detection power `1 - β` for Minimum Detectable Bias publication (default `0.80`).
    pub target_power: f64,
    /// Number of independent periods per quarter for quarterly MDB scaling (default `3` monthly periods;
    /// `1` gives single-period MDB; plan §20.6).
    pub periods_per_quarter: usize,
}

impl Default for GrossErrorPolicy {
    fn default() -> Self {
        Self {
            global_alpha: 0.01,
            measurement_alpha: 0.05,
            multiplicity: MultiplicityCorrection::Sidak,
            target_power: 0.80,
            periods_per_quarter: 3,
        }
    }
}

impl GrossErrorPolicy {
    /// Validates policy parameters without clamping.
    pub fn validate(&self) -> Result<(), ReconError> {
        if !(self.global_alpha.is_finite() && self.global_alpha > 0.0 && self.global_alpha < 1.0) {
            return Err(ReconError::invalid(
                "gross_error.global_alpha",
                format!("must lie in (0, 1), got {}", self.global_alpha),
            ));
        }
        if !(self.measurement_alpha.is_finite()
            && self.measurement_alpha > 0.0
            && self.measurement_alpha < 1.0)
        {
            return Err(ReconError::invalid(
                "gross_error.measurement_alpha",
                format!("must lie in (0, 1), got {}", self.measurement_alpha),
            ));
        }
        if !(self.target_power.is_finite() && self.target_power > 0.0 && self.target_power < 1.0) {
            return Err(ReconError::invalid(
                "gross_error.target_power",
                format!("must lie in (0, 1), got {}", self.target_power),
            ));
        }
        if self.periods_per_quarter == 0 {
            return Err(ReconError::invalid(
                "gross_error.periods_per_quarter",
                "must be >= 1",
            ));
        }
        Ok(())
    }

    fn per_measurement_alpha(&self, redundant_count: usize) -> f64 {
        if redundant_count <= 1 {
            return self.measurement_alpha;
        }
        let k = redundant_count as f64;
        match self.multiplicity {
            MultiplicityCorrection::None => self.measurement_alpha,
            MultiplicityCorrection::Bonferroni => self.measurement_alpha / k,
            MultiplicityCorrection::Sidak => {
                1.0 - det::exp(det::ln(1.0 - self.measurement_alpha) / k)
            }
        }
    }
}

/// Commit disposition of a reconciled period (plan §20.2, `INV-005`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitDisposition {
    /// No global or measurement gross error triggered; eligible to commit.
    Admit,
    /// Global or measurement test rejected; commit must be refused (`FMINE-REF-101`) and routed to
    /// an `InvestigationCase` with ranked suspects.
    RefuseForInvestigation,
}

/// Per-measurement gross-error diagnostic ranked by descending `|z_i|`.
#[derive(Clone, Debug, PartialEq)]
pub struct SuspectMeasurement {
    /// Variable key.
    pub variable: VariableKey,
    /// Raw adjustment `a_i = x_hat_i - y_i`.
    pub adjustment: f64,
    /// Standard deviation of the adjustment `sqrt(W_ii)`.
    pub adjustment_sd: f64,
    /// Signed normalized adjustment `z_i = a_i / sqrt(W_ii)`.
    pub normalized_adjustment: f64,
    /// Two-sided unadjusted normal p-value `P(|Z| >= |z_i|)`.
    pub two_sided_p: f64,
    /// `true` iff `|z_i| > measurement_z_critical`.
    pub flagged: bool,
}

/// Published Minimum Detectable Bias (MDB) at the declared power and false-alarm rate for one
/// redundant measurement node (plan §20.6).
#[derive(Clone, Debug, PartialEq)]
pub struct NodeDetectableBias {
    /// Variable key.
    pub variable: VariableKey,
    /// Unit-bias non-centrality sensitivity `|E[Z_i] / delta_i|` for the per-measurement test.
    pub measurement_unit_sensitivity: f64,
    /// Minimum detectable single-period bias magnitude (channel units) for the measurement test.
    pub measurement_mdb_single_period: f64,
    /// Minimum detectable persistent bias magnitude over `periods_per_quarter` for the measurement test.
    pub measurement_mdb_quarter: f64,
    /// Quarterly measurement-test MDB as a percentage of `|observed|` (`None` if `observed == 0`).
    pub measurement_mdb_quarter_pct: Option<f64>,
    /// Minimum detectable single-period bias magnitude (channel units) for the global chi-square test.
    pub global_mdb_single_period: f64,
    /// Minimum detectable persistent bias magnitude over `periods_per_quarter` for the global test.
    pub global_mdb_quarter: f64,
}

/// Complete gross-error evaluation and detection-power report for one [`LinearReconResult`] (`BD-150`).
#[derive(Clone, Debug, PartialEq)]
pub struct GrossErrorReport {
    /// Global chi-square statistic `h = (A y - b)^T (A V A^T)^-1 (A y - b)`.
    pub global_chi_square: f64,
    /// Degrees of freedom `df = rank(A)`.
    pub degrees_of_freedom: usize,
    /// Upper-tail chi-square p-value `P(ChiSq_df >= h)`.
    pub global_p_value: f64,
    /// Critical value `chi^2_{1 - global_alpha, df}`.
    pub global_critical_value: f64,
    /// `true` iff `global_chi_square > global_critical_value`.
    pub global_rejected: bool,
    /// Number of redundant measurements eligible for per-measurement testing.
    pub redundant_measurement_count: usize,
    /// Effective per-measurement significance level after [`MultiplicityCorrection`].
    pub per_measurement_alpha: f64,
    /// Critical value `z_{1 - per_measurement_alpha / 2}`.
    pub measurement_z_critical: f64,
    /// Redundant measurements ranked descending by `|normalized_adjustment|`.
    pub suspect_ranking: Vec<SuspectMeasurement>,
    /// Published single-period and quarterly Minimum Detectable Bias per redundant measurement (§20.6).
    pub detectable_biases: Vec<NodeDetectableBias>,
    /// Commit disposition (`Admit` vs `RefuseForInvestigation`).
    pub disposition: CommitDisposition,
}

/// Candidate systematic bias vector `δ` (in channel units) whose exact analytical detection power
/// is evaluated by [`analyze_bias_power`] (`BD-150`).
#[derive(Clone, Debug, PartialEq)]
pub struct BiasVectorSpec {
    /// Non-zero `(variable, bias_magnitude)` entries (`δ_i`).
    pub biases: Vec<(VariableKey, f64)>,
    /// Number of independent periods averaged (`1` for single-period test; `P` for `P`-period test).
    pub periods: usize,
}

/// Per-variable exact theoretical detection power under a candidate [`BiasVectorSpec`].
#[derive(Clone, Debug, PartialEq)]
pub struct VariableBiasPower {
    /// Variable key.
    pub variable: VariableKey,
    /// Expected normalized adjustment `E[Z_i]` under the candidate bias vector (scaled by `sqrt(periods)`).
    pub non_centrality_mean: f64,
    /// Exact two-sided rejection probability `P(|Z_i| >= z_crit)`.
    pub detection_power: f64,
}

/// Exact analytical detection power under a candidate bias vector `δ` (`BD-150`).
#[derive(Clone, Debug, PartialEq)]
pub struct BiasPowerAnalysis {
    /// Global chi-square non-centrality parameter `λ = periods * (A δ)^T (A V A^T)^-1 (A δ)`.
    pub global_non_centrality: f64,
    /// Global critical value `chi^2_{1 - global_alpha, df}`.
    pub global_critical_value: f64,
    /// Exact theoretical global chi-square detection power `P(ChiSq(df, λ) >= chi_crit)`.
    pub global_power: f64,
    /// Per-measurement critical value `z_{1 - per_measurement_alpha / 2}`.
    pub measurement_z_critical: f64,
    /// Exact per-measurement detection powers across all redundant variables (in canonical order).
    pub per_variable_power: Vec<VariableBiasPower>,
}

/// Computes `C = A V A^T` (`m x m`) and `V A^T` (`n x m`) from a validated [`LinearReconResult`].
fn assemble_balance_covariance(
    recon: &LinearReconResult,
) -> Result<(Vec<f64>, Vec<f64>), ReconError> {
    let n = recon.variables.len();
    let m = recon.degrees_of_freedom;
    if m == 0 || n == 0 || recon.constraints.len() != m * n || recon.prior_covariance.len() != n * n
    {
        return Err(ReconError::InsufficientRedundancy {
            context: "gross_error".to_string(),
            detail: "reconciliation result has invalid dimensions or zero degrees of freedom"
                .to_string(),
        });
    }
    let mut va = vec![0.0_f64; n * m];
    for i in 0..n {
        for r in 0..m {
            let mut sum = 0.0;
            for j in 0..n {
                sum += recon.prior_covariance[i * n + j] * recon.constraints[r * n + j];
            }
            va[i * m + r] = sum;
        }
    }
    let mut c = vec![0.0_f64; m * m];
    for r in 0..m {
        for s in r..m {
            let mut sum = 0.0;
            for i in 0..n {
                sum += recon.constraints[r * n + i] * va[i * m + s];
            }
            c[r * m + s] = sum;
            c[s * m + r] = sum;
        }
    }
    Ok((c, va))
}

/// Evaluates global chi-square and per-measurement gross-error tests plus single-period and
/// quarterly Minimum Detectable Bias publication for a [`LinearReconResult`] (`BD-150`).
pub fn evaluate_gross_errors(
    recon: &LinearReconResult,
    policy: &GrossErrorPolicy,
) -> Result<GrossErrorReport, ReconError> {
    policy.validate()?;
    let n = recon.variables.len();
    let m = recon.degrees_of_freedom;
    if m == 0 {
        return Err(ReconError::InsufficientRedundancy {
            context: "gross_error.evaluate".to_string(),
            detail: "zero degrees of freedom; gross-error test has no statistical power"
                .to_string(),
        });
    }
    let (c_mat, va) = assemble_balance_covariance(recon)?;

    let global_p_value = chi_square_upper_tail(recon.chi_square, m);
    let global_critical_value = chi_square_critical_upper(policy.global_alpha, m);
    let global_rejected = recon.chi_square > global_critical_value;

    let redundant_indices: Vec<usize> = (0..n)
        .filter(|&i| recon.normalized_adjustments[i].is_some())
        .collect();
    let redundant_count = redundant_indices.len();
    let per_measurement_alpha = policy.per_measurement_alpha(redundant_count);
    let measurement_z_critical = det::norm_ppf(1.0 - 0.5 * per_measurement_alpha);

    let mut suspect_ranking = Vec::with_capacity(redundant_count);
    let mut any_measurement_flagged = false;

    for &i in &redundant_indices {
        let z = recon.normalized_adjustments[i].unwrap_or(0.0);
        let w_ii = recon.adjustment_covariance[i * n + i].max(0.0);
        let adj_sd = det::sqrt(w_ii);
        let p_val = normal_two_sided_p(z);
        let flagged = z.abs() > measurement_z_critical;
        if flagged {
            any_measurement_flagged = true;
        }
        suspect_ranking.push(SuspectMeasurement {
            variable: recon.variables[i].clone(),
            adjustment: recon.adjustments[i],
            adjustment_sd: adj_sd,
            normalized_adjustment: z,
            two_sided_p: p_val,
            flagged,
        });
    }
    suspect_ranking.sort_by(|a, b| {
        b.normalized_adjustment
            .abs()
            .total_cmp(&a.normalized_adjustment.abs())
            .then_with(|| a.variable.cmp(&b.variable))
    });

    // Compute exact single-period & quarterly Minimum Detectable Bias per redundant variable (§20.6).
    let target_mu = normal_two_sided_ncp_for_power(measurement_z_critical, policy.target_power);
    let target_lambda =
        noncentral_chi_square_ncp_for_power(global_critical_value, m, policy.target_power);
    let sqrt_p = det::sqrt(policy.periods_per_quarter as f64);

    let mut detectable_biases = Vec::with_capacity(redundant_count);
    for &i in &redundant_indices {
        let a_col: Vec<f64> = (0..m).map(|r| recon.constraints[r * n + i]).collect();
        let u = solve_equilibrated_spd(&c_mat, &a_col, m)?;
        // Global unit-bias non-centrality: lambda_1 = (A e_i)^T C^-1 (A e_i).
        let lambda_unit: f64 = a_col.iter().zip(&u).map(|(&a, &ui)| a * ui).sum();
        // Measurement unit-bias mean adjustment: E[a_i | delta_i = 1] = -(V A^T u)_i.
        let e_adj_unit: f64 = -(0..m).map(|r| va[i * m + r] * u[r]).sum::<f64>();
        let w_ii = recon.adjustment_covariance[i * n + i].max(0.0);
        let adj_sd = det::sqrt(w_ii);
        if !(adj_sd > 0.0 && e_adj_unit.abs() > 0.0 && lambda_unit > 0.0) {
            continue;
        }
        let meas_sens = e_adj_unit.abs() / adj_sd;
        let meas_mdb_1 = target_mu / meas_sens;
        let meas_mdb_q = meas_mdb_1 / sqrt_p;
        let obs_abs = recon.observed[i].abs();
        let meas_mdb_q_pct = if obs_abs > 0.0 {
            Some(100.0 * meas_mdb_q / obs_abs)
        } else {
            None
        };
        let glob_mdb_1 = det::sqrt(target_lambda / lambda_unit);
        let glob_mdb_q = glob_mdb_1 / sqrt_p;

        detectable_biases.push(NodeDetectableBias {
            variable: recon.variables[i].clone(),
            measurement_unit_sensitivity: meas_sens,
            measurement_mdb_single_period: meas_mdb_1,
            measurement_mdb_quarter: meas_mdb_q,
            measurement_mdb_quarter_pct: meas_mdb_q_pct,
            global_mdb_single_period: glob_mdb_1,
            global_mdb_quarter: glob_mdb_q,
        });
    }

    let disposition = if global_rejected || any_measurement_flagged {
        CommitDisposition::RefuseForInvestigation
    } else {
        CommitDisposition::Admit
    };

    Ok(GrossErrorReport {
        global_chi_square: recon.chi_square,
        degrees_of_freedom: m,
        global_p_value,
        global_critical_value,
        global_rejected,
        redundant_measurement_count: redundant_count,
        per_measurement_alpha,
        measurement_z_critical,
        suspect_ranking,
        detectable_biases,
        disposition,
    })
}

/// Enforces the commit-time gross-error gate (`INV-005`, plan §20.2): returns `Ok(())` iff
/// `report.disposition == CommitDisposition::Admit`, else refuses with `FMINE-REF-101`.
pub fn enforce_commit_gate(report: &GrossErrorReport) -> Result<(), ReconError> {
    if report.disposition == CommitDisposition::Admit {
        return Ok(());
    }
    let top_suspect = report.suspect_ranking.first().map_or_else(
        || "none".to_string(),
        |s| {
            format!(
                "{} (z={:.2}, adj={:.4})",
                s.variable.canonical_id(),
                s.normalized_adjustment,
                s.adjustment
            )
        },
    );
    Err(ReconError::ConservationGrossError {
        detail: format!(
            "chi_sq={:.3} (df={}, p={:.4e}, crit={:.3}); top suspect: {top_suspect}",
            report.global_chi_square,
            report.degrees_of_freedom,
            report.global_p_value,
            report.global_critical_value
        ),
    })
}

/// Computes exact theoretical global chi-square and per-measurement detection power for an
/// arbitrary candidate bias vector `spec` under `policy` (`BD-150`).
pub fn analyze_bias_power(
    recon: &LinearReconResult,
    policy: &GrossErrorPolicy,
    spec: &BiasVectorSpec,
) -> Result<BiasPowerAnalysis, ReconError> {
    policy.validate()?;
    if spec.periods == 0 {
        return Err(ReconError::invalid("bias_power.periods", "must be >= 1"));
    }
    let n = recon.variables.len();
    let m = recon.degrees_of_freedom;
    let (c_mat, va) = assemble_balance_covariance(recon)?;

    let idx_map: BTreeMap<&VariableKey, usize> = recon
        .variables
        .iter()
        .enumerate()
        .map(|(i, k)| (k, i))
        .collect();

    let mut delta = vec![0.0_f64; n];
    for (var, val) in &spec.biases {
        if !val.is_finite() {
            return Err(ReconError::invalid(
                "bias_power.biases",
                format!("non-finite bias for '{}'", var.canonical_id()),
            ));
        }
        let Some(&idx) = idx_map.get(var) else {
            return Err(ReconError::invalid(
                "bias_power.biases",
                format!("unknown variable '{}'", var.canonical_id()),
            ));
        };
        delta[idx] += *val;
    }

    // a_delta = A * delta (m-vector).
    let a_delta: Vec<f64> = (0..m)
        .map(|r| {
            (0..n)
                .map(|j| recon.constraints[r * n + j] * delta[j])
                .sum()
        })
        .collect();
    let u = solve_equilibrated_spd(&c_mat, &a_delta, m)?;
    let lambda_single: f64 = a_delta
        .iter()
        .zip(&u)
        .map(|(&a, &ui)| a * ui)
        .sum::<f64>()
        .max(0.0);
    let global_non_centrality = (spec.periods as f64) * lambda_single;

    let global_critical_value = chi_square_critical_upper(policy.global_alpha, m);
    let global_power =
        noncentral_chi_square_upper_tail(global_critical_value, m, global_non_centrality);

    let redundant_indices: Vec<usize> = (0..n)
        .filter(|&i| recon.normalized_adjustments[i].is_some())
        .collect();
    let per_measurement_alpha = policy.per_measurement_alpha(redundant_indices.len());
    let measurement_z_critical = det::norm_ppf(1.0 - 0.5 * per_measurement_alpha);
    let sqrt_p = det::sqrt(spec.periods as f64);

    let mut per_variable_power = Vec::with_capacity(redundant_indices.len());
    for &i in &redundant_indices {
        // E[a_i] = -(V A^T u)_i; Var(a_i) = W_ii. Over P periods, E[Z_i] scales by sqrt(P).
        let e_adj_single: f64 = -(0..m).map(|r| va[i * m + r] * u[r]).sum::<f64>();
        let w_ii = recon.adjustment_covariance[i * n + i].max(0.0);
        let adj_sd = det::sqrt(w_ii);
        let mu = if adj_sd > 0.0 {
            sqrt_p * e_adj_single / adj_sd
        } else {
            0.0
        };
        let power = normal_two_sided_power(mu, measurement_z_critical);
        per_variable_power.push(VariableBiasPower {
            variable: recon.variables[i].clone(),
            non_centrality_mean: mu,
            detection_power: power,
        });
    }

    Ok(BiasPowerAnalysis {
        global_non_centrality,
        global_critical_value,
        global_power,
        measurement_z_critical,
        per_variable_power,
    })
}
