//! R2 Dynamic Latent Stockpile States, Kalman/RTS Smoothing (`PO-TOUCH-3`), Mixing-Model
//! Predictive Comparison, Metallurgical Oxidation State, and Plant-Cone / Unreported-Addition
//! Inference (`BD-154`, plan §20.8, `E194`, `E252–E255`, `E258`, `E666`).
//! Determinism: DeterministicPerIsa (Joseph-form 2×2 Kalman filter + RTS smoother, equilibrated
//! SPD joint batch Gaussian oracle, `fmine_core::det` special functions, and BLAKE3 digests).

use std::collections::VecDeque;

use fmine_core::codec::Encoder;
use fmine_core::det;
use fmine_core::hash::blake3_hash;
use fmine_flow::graph::StockpileMixingModel;

use crate::error::ReconError;
use crate::linear::solve_equilibrated_spd;

/// Exact SI grams per troy ounce (`31.1034768`), used when tonnes are in `kt` (`10^3 t`),
/// grades in `g/t`, and metal in `koz` (`10^3 troy oz`), since `kt * (g/t) / 31.1034768 = koz`.
pub const GRAMS_PER_TROY_OZ: f64 = 31.103_476_8;

/// Maximum periods supported in one exact Kalman/RTS + batch Gaussian stockpile trajectory.
pub const MAX_STOCKPILE_PERIODS: usize = 48;

/// Prior state distribution `s_0 = [dry_tonnes_0, contained_metal_0]^T ~ N(mu_0, P_0)` at `t = 0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StockpileStatePrior {
    /// Opening dry tonnage mean (`> 0`).
    pub opening_tonnes: f64,
    /// Opening contained metal mean (`>= 0`, in declared metal units).
    pub opening_metal: f64,
    /// Prior standard deviation on opening dry tonnage (`> 0`).
    pub tonnes_sd: f64,
    /// Prior standard deviation on opening contained metal (`> 0`).
    pub metal_sd: f64,
    /// Prior Pearson correlation between opening tonnes and opening metal in `(-0.9999, 0.9999)`.
    pub tonnes_metal_corr: f64,
}

/// Physical observation on the latent stockpile state `[M_t, m_t]^T` at period boundary `t`.
#[derive(Clone, Debug, PartialEq)]
pub enum StockpileObservation {
    /// Volumetric survey (`H = [1, 0]`): updates tonnage uncertainty while preserving conditional
    /// grade uncertainty (§20.8).
    VolumetricSurvey {
        observed_tonnes: f64,
        survey_sd: f64,
    },
    /// Contained-metal stocktake / sonic-drill metal estimate (`H = [0, 1]`).
    MetalStocktake { observed_metal: f64, metal_sd: f64 },
    /// Joint tonnage + contained-metal observation (`H = I_2`, e.g. survey + sonic-drill grid `E194`).
    JointStocktake {
        observed_tonnes: f64,
        observed_metal: f64,
        tonnes_sd: f64,
        metal_sd: f64,
        corr: f64,
    },
}

/// Net flow transition `s_{t+1} = s_t + u_t + w_t` over period `t -> t+1` (`t = 0..T-1`).
#[derive(Clone, Debug, PartialEq)]
pub struct StockpileFlowStep {
    /// Period label (e.g. `"2025Q1"`).
    pub period_label: String,
    /// Measured/declared dry tonnage added to stockpile (`>= 0`).
    pub inflow_tonnes: f64,
    /// Tonnage uncertainty SD on inflow (`>= 0`).
    pub inflow_tonnes_sd: f64,
    /// Grade-control grade of inflow (in `metal_unit * grade_scale / tonne`, e.g. `g/t`).
    pub inflow_grade: f64,
    /// Grade-control grade uncertainty SD (`>= 0`).
    pub inflow_grade_sd: f64,
    /// Measured/declared dry tonnage reclaimed from stockpile (`>= 0`).
    pub reclaim_tonnes: f64,
    /// Tonnage uncertainty SD on reclaim (`>= 0`).
    pub reclaim_tonnes_sd: f64,
    /// Declared reclaim grade (same grade unit as `inflow_grade`).
    pub reclaim_grade: f64,
    /// Reclaim grade uncertainty SD (`>= 0`).
    pub reclaim_grade_sd: f64,
    /// Additional unmodelled process tonnage SD (`> 0` if flow SDs are zero).
    pub process_tonnes_sd: f64,
    /// Additional unmodelled process metal SD (`> 0` if flow SDs are zero).
    pub process_metal_sd: f64,
}

/// Request to run the exact multi-period R2 latent stockpile Kalman filter + RTS smoother
/// (`BD-154`, `PO-TOUCH-3`).
#[derive(Clone, Debug, PartialEq)]
pub struct StockpileStateSpaceRequest {
    /// Stockpile or surge-cone identifier.
    pub stockpile_id: String,
    /// Conversion denominator from `tonnes * grade` to `contained_metal` (`31.1034768` for `kt * g/t -> koz`,
    /// `1.0` for `t * mass_fraction -> t`).
    pub grade_to_metal_divisor: f64,
    /// Opening latent state prior at `t = 0`.
    pub prior: StockpileStatePrior,
    /// Period-by-period flow transitions (`len = T >= 1`).
    pub steps: Vec<StockpileFlowStep>,
    /// Observations at each period boundary `0..=T` (`len = T + 1`; each period may have 0 or more observations).
    pub observations_by_time: Vec<Vec<StockpileObservation>>,
    /// Central credible interval level in `(0, 1)` (e.g. `0.90`).
    pub confidence_level: f64,
}

/// Smoothed posterior state summary at one period boundary `t`.
#[derive(Clone, Debug, PartialEq)]
pub struct StockpilePeriodEstimate {
    /// Time index `t in 0..=T`.
    pub time_index: usize,
    /// Smoothed dry tonnage mean `E[M_t | y_{0:T}]`.
    pub tonnes_mean: f64,
    /// Smoothed dry tonnage SD `sqrt(Var(M_t | y_{0:T}))`.
    pub tonnes_sd: f64,
    /// Filtered (forward-only) dry tonnage SD `sqrt(Var(M_t | y_{0:t}))`.
    pub filtered_tonnes_sd: f64,
    /// Smoothed contained metal mean `E[m_t | y_{0:T}]`.
    pub metal_mean: f64,
    /// Smoothed contained metal SD `sqrt(Var(m_t | y_{0:T}))`.
    pub metal_sd: f64,
    /// Smoothed `[M_t, m_t]` 2×2 row-major posterior covariance `[p00, p01, p10, p11]`.
    pub covariance_2x2: [f64; 4],
    /// Derived smoothed grade `grade_to_metal_divisor * m_t / M_t` (never reconciled directly; `G-TOUCH-1`).
    pub derived_grade_mean: f64,
    /// First-order delta-method posterior SD of `derived_grade`.
    pub derived_grade_sd: f64,
    /// Lower central credible bound on dry tonnage.
    pub tonnes_ci_lower: f64,
    /// Upper central credible bound on dry tonnage.
    pub tonnes_ci_upper: f64,
    /// Lower central credible bound on contained metal.
    pub metal_ci_lower: f64,
    /// Upper central credible bound on contained metal.
    pub metal_ci_upper: f64,
}

/// Smoothed net inventory change `Delta s_t = s_{t+1} - s_t` over period `t -> t+1`.
#[derive(Clone, Debug, PartialEq)]
pub struct StockpilePeriodDelta {
    /// Period label.
    pub period_label: String,
    /// Smoothed `M_{t+1} - M_t`.
    pub delta_tonnes_mean: f64,
    /// Exact smoothed SD incorporating lag-1 cross-covariance `Cov(s_{t+1}, s_t | y_{0:T})`.
    pub delta_tonnes_sd: f64,
    /// Smoothed `m_{t+1} - m_t`.
    pub delta_metal_mean: f64,
    /// Exact smoothed SD of `m_{t+1} - m_t`.
    pub delta_metal_sd: f64,
}

/// Complete exact Kalman/RTS smoother posterior over a multi-period latent stockpile (`BD-154`).
#[derive(Clone, Debug, PartialEq)]
pub struct StockpileTrajectoryPosterior {
    pub stockpile_id: String,
    pub periods: Vec<StockpilePeriodEstimate>,
    pub deltas: Vec<StockpilePeriodDelta>,
    /// Exact Gaussian marginal log-likelihood `ln p(y_{0:T})`.
    pub log_likelihood: f64,
    /// Replay-stable BLAKE3 hex digest.
    pub digest_hex: String,
}

/// Independent joint batch Gaussian posterior solution (`PO-TOUCH-3` oracle).
#[derive(Clone, Debug, PartialEq)]
pub struct BatchGaussianStockpileSolution {
    /// Stacked joint posterior mean `[M_0, m_0, M_1, m_1, ..., M_T, m_T]`.
    pub joint_mean: Vec<f64>,
    /// Stacked `2(T+1) x 2(T+1)` row-major joint posterior covariance.
    pub joint_covariance: Vec<f64>,
}

/// Exact 2×2 SPD inverse with determinant validation.
fn inv_2x2(m: [f64; 4], context: &str) -> Result<[f64; 4], ReconError> {
    let det_v = m[0] * m[3] - m[1] * m[2];
    if !(det_v.is_finite() && det_v > 0.0 && m[0] > 0.0 && m[3] > 0.0) {
        return Err(ReconError::NumericalFailure {
            detail: format!("{context}: 2x2 matrix is not strictly SPD (det={det_v})"),
        });
    }
    let inv_d = 1.0 / det_v;
    let out = [m[3] * inv_d, -m[1] * inv_d, -m[2] * inv_d, m[0] * inv_d];
    if out.iter().any(|v| !v.is_finite()) {
        return Err(ReconError::NumericalFailure {
            detail: format!("{context}: 2x2 inverse overflow"),
        });
    }
    Ok(out)
}

fn mul_2x2(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
    ]
}

fn trans_2x2(a: [f64; 4]) -> [f64; 4] {
    [a[0], a[2], a[1], a[3]]
}

fn sym_2x2(a: [f64; 4]) -> [f64; 4] {
    let off = 0.5 * (a[1] + a[2]);
    [a[0], off, off, a[3]]
}

fn add_2x2(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn sub_2x2(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

fn mv_2x2(a: [f64; 4], x: [f64; 2]) -> [f64; 2] {
    [a[0] * x[0] + a[1] * x[1], a[2] * x[0] + a[3] * x[1]]
}

fn build_cov_2x2(sd0: f64, sd1: f64, corr: f64, context: &str) -> Result<[f64; 4], ReconError> {
    if !(sd0.is_finite()
        && sd0 > 0.0
        && sd1.is_finite()
        && sd1 > 0.0
        && corr.is_finite()
        && corr.abs() < 0.9999)
    {
        return Err(ReconError::invalid(
            context,
            format!("invalid 2x2 SD/corr: sd0={sd0}, sd1={sd1}, corr={corr}"),
        ));
    }
    let v0 = sd0 * sd0;
    let v1 = sd1 * sd1;
    let cov = corr * sd0 * sd1;
    if !(v0.is_finite() && v0 > 0.0 && v1.is_finite() && v1 > 0.0 && cov.is_finite()) {
        return Err(ReconError::NumericalFailure {
            detail: format!("{context}: variance overflow/underflow"),
        });
    }
    Ok([v0, cov, cov, v1])
}

/// Computes exact flow control vector `u_t` and 2×2 SPD transition noise `Q_t` for one flow step.
fn step_control_and_cov(
    step: &StockpileFlowStep,
    divisor: f64,
) -> Result<([f64; 2], [f64; 4]), ReconError> {
    if step.period_label.trim().is_empty() {
        return Err(ReconError::invalid(
            "stockpile.steps",
            "period_label must not be empty",
        ));
    }
    let vals = [
        step.inflow_tonnes,
        step.inflow_tonnes_sd,
        step.inflow_grade,
        step.inflow_grade_sd,
        step.reclaim_tonnes,
        step.reclaim_tonnes_sd,
        step.reclaim_grade,
        step.reclaim_grade_sd,
        step.process_tonnes_sd,
        step.process_metal_sd,
    ];
    if vals.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Err(ReconError::invalid(
            "stockpile.steps",
            format!(
                "step '{}' values and SDs must be finite and >= 0",
                step.period_label
            ),
        ));
    }
    let u_t = step.inflow_tonnes - step.reclaim_tonnes;
    let u_m = (step.inflow_tonnes * step.inflow_grade - step.reclaim_tonnes * step.reclaim_grade)
        / divisor;

    // First-order extensive metal covariance from independent tonnage & grade errors on inflow and reclaim:
    // dm = (g * dM + M * dg) / divisor => Var(dm) = (g^2 Var(dM) + M^2 Var(dg)) / divisor^2, Cov(dM, dm) = g * Var(dM) / divisor.
    let v_in_t = step.inflow_tonnes_sd * step.inflow_tonnes_sd;
    let v_in_g = step.inflow_grade_sd * step.inflow_grade_sd;
    let v_rec_t = step.reclaim_tonnes_sd * step.reclaim_tonnes_sd;
    let v_rec_g = step.reclaim_grade_sd * step.reclaim_grade_sd;

    let inv_div = 1.0 / divisor;
    let inv_div2 = inv_div * inv_div;
    let q00 = v_in_t + v_rec_t + step.process_tonnes_sd * step.process_tonnes_sd;
    let q11 = (step.inflow_grade * step.inflow_grade * v_in_t
        + step.inflow_tonnes * step.inflow_tonnes * v_in_g
        + step.reclaim_grade * step.reclaim_grade * v_rec_t
        + step.reclaim_tonnes * step.reclaim_tonnes * v_rec_g)
        * inv_div2
        + step.process_metal_sd * step.process_metal_sd;
    let q01 = (step.inflow_grade * v_in_t + step.reclaim_grade * v_rec_t) * inv_div;

    let q = [q00, q01, q01, q11];
    let _ = inv_2x2(q, &format!("step '{}' transition Q_t", step.period_label))?;
    Ok(([u_t, u_m], q))
}

fn validate_request(req: &StockpileStateSpaceRequest) -> Result<[f64; 4], ReconError> {
    if req.stockpile_id.trim().is_empty() {
        return Err(ReconError::invalid(
            "stockpile.stockpile_id",
            "stockpile_id must not be empty",
        ));
    }
    if !(req.grade_to_metal_divisor.is_finite() && req.grade_to_metal_divisor > 0.0) {
        return Err(ReconError::invalid(
            "stockpile.grade_to_metal_divisor",
            "grade_to_metal_divisor must be finite and > 0",
        ));
    }
    if req.steps.is_empty() || req.steps.len() > MAX_STOCKPILE_PERIODS {
        return Err(ReconError::invalid(
            "stockpile.steps",
            format!("steps count must lie in 1..={MAX_STOCKPILE_PERIODS}"),
        ));
    }
    if req.observations_by_time.len() != req.steps.len() + 1 {
        return Err(ReconError::invalid(
            "stockpile.observations_by_time",
            format!(
                "expected {} observation lists (t=0..=T), got {}",
                req.steps.len() + 1,
                req.observations_by_time.len()
            ),
        ));
    }
    if !(req.confidence_level.is_finite()
        && req.confidence_level > 0.0
        && req.confidence_level < 1.0)
    {
        return Err(ReconError::invalid(
            "stockpile.confidence_level",
            "confidence_level must lie in (0, 1)",
        ));
    }
    if !(req.prior.opening_tonnes.is_finite()
        && req.prior.opening_tonnes > 0.0
        && req.prior.opening_metal.is_finite()
        && req.prior.opening_metal >= 0.0)
    {
        return Err(ReconError::invalid(
            "stockpile.prior",
            "opening_tonnes must be finite > 0 and opening_metal finite >= 0",
        ));
    }
    let p0 = build_cov_2x2(
        req.prior.tonnes_sd,
        req.prior.metal_sd,
        req.prior.tonnes_metal_corr,
        "stockpile.prior",
    )?;
    for (t, obs_list) in req.observations_by_time.iter().enumerate() {
        for obs in obs_list {
            match *obs {
                StockpileObservation::VolumetricSurvey {
                    observed_tonnes,
                    survey_sd,
                } => {
                    if !(observed_tonnes.is_finite()
                        && observed_tonnes >= 0.0
                        && survey_sd.is_finite()
                        && survey_sd > 0.0
                        && (survey_sd * survey_sd).is_finite()
                        && (survey_sd * survey_sd) > 0.0)
                    {
                        return Err(ReconError::invalid(
                            "stockpile.observations",
                            format!("invalid VolumetricSurvey at t={t}"),
                        ));
                    }
                }
                StockpileObservation::MetalStocktake {
                    observed_metal,
                    metal_sd,
                } => {
                    if !(observed_metal.is_finite()
                        && observed_metal >= 0.0
                        && metal_sd.is_finite()
                        && metal_sd > 0.0
                        && (metal_sd * metal_sd).is_finite()
                        && (metal_sd * metal_sd) > 0.0)
                    {
                        return Err(ReconError::invalid(
                            "stockpile.observations",
                            format!("invalid MetalStocktake at t={t}"),
                        ));
                    }
                }
                StockpileObservation::JointStocktake {
                    observed_tonnes,
                    observed_metal,
                    tonnes_sd,
                    metal_sd,
                    corr,
                } => {
                    if !(observed_tonnes.is_finite()
                        && observed_tonnes >= 0.0
                        && observed_metal.is_finite()
                        && observed_metal >= 0.0)
                    {
                        return Err(ReconError::invalid(
                            "stockpile.observations",
                            format!("invalid JointStocktake values at t={t}"),
                        ));
                    }
                    let _ =
                        build_cov_2x2(tonnes_sd, metal_sd, corr, &format!("JointStocktake t={t}"))?;
                }
            }
        }
    }
    Ok(p0)
}

/// Applies one scalar observation `y = h^T s + eps`, `eps ~ N(0, r)` in exact Joseph form.
fn kalman_update_scalar(
    mu: [f64; 2],
    p: [f64; 4],
    h: [f64; 2],
    y: f64,
    r: f64,
) -> Result<([f64; 2], [f64; 4], f64), ReconError> {
    let ph = mv_2x2(p, h);
    let s_inn = h[0] * ph[0] + h[1] * ph[1] + r;
    if !(s_inn.is_finite() && s_inn > 0.0) {
        return Err(ReconError::NumericalFailure {
            detail: format!("scalar Kalman innovation variance invalid: {s_inn}"),
        });
    }
    let innov = y - (h[0] * mu[0] + h[1] * mu[1]);
    let k = [ph[0] / s_inn, ph[1] / s_inn];
    let mu_new = [mu[0] + k[0] * innov, mu[1] + k[1] * innov];
    // Joseph form: P_new = (I - K h^T) P (I - K h^T)^T + K r K^T
    let ikh = [
        1.0 - k[0] * h[0],
        -k[0] * h[1],
        -k[1] * h[0],
        1.0 - k[1] * h[1],
    ];
    let term1 = mul_2x2(mul_2x2(ikh, p), trans_2x2(ikh));
    let term2 = [
        k[0] * k[0] * r,
        k[0] * k[1] * r,
        k[1] * k[0] * r,
        k[1] * k[1] * r,
    ];
    let p_new = sym_2x2(add_2x2(term1, term2));
    let ll = -0.5 * (det::ln(2.0 * std::f64::consts::PI * s_inn) + (innov * innov) / s_inn);
    Ok((mu_new, p_new, ll))
}

/// Applies a full 2D observation `y = s + eps`, `eps ~ N(0, R)` in exact Joseph form.
fn kalman_update_joint(
    mu: [f64; 2],
    p: [f64; 4],
    y: [f64; 2],
    r: [f64; 4],
) -> Result<([f64; 2], [f64; 4], f64), ReconError> {
    let s_mat = add_2x2(p, r);
    let s_inv = inv_2x2(s_mat, "joint Kalman innovation S")?;
    let det_s = s_mat[0] * s_mat[3] - s_mat[1] * s_mat[2];
    let innov = [y[0] - mu[0], y[1] - mu[1]];
    let k = mul_2x2(p, s_inv);
    let k_inn = mv_2x2(k, innov);
    let mu_new = [mu[0] + k_inn[0], mu[1] + k_inn[1]];
    let ik = [1.0 - k[0], -k[1], -k[2], 1.0 - k[3]];
    let term1 = mul_2x2(mul_2x2(ik, p), trans_2x2(ik));
    let term2 = mul_2x2(mul_2x2(k, r), trans_2x2(k));
    let p_new = sym_2x2(add_2x2(term1, term2));
    let sinv_inn = mv_2x2(s_inv, innov);
    let quad = innov[0] * sinv_inn[0] + innov[1] * sinv_inn[1];
    let ll = -0.5 * (2.0 * det::ln(2.0 * std::f64::consts::PI) + det::ln(det_s) + quad);
    Ok((mu_new, p_new, ll))
}

/// Runs the exact R2 forward Kalman filter and backward Rauch–Tung–Striebel (RTS) smoother over
/// all periods `0..=T` (`BD-154`, plan §20.8, `PO-TOUCH-3`).
pub fn run_stockpile_kalman_smoother(
    req: &StockpileStateSpaceRequest,
) -> Result<StockpileTrajectoryPosterior, ReconError> {
    let p0 = validate_request(req)?;
    let t_steps = req.steps.len();
    let n_times = t_steps + 1;

    let mut controls = Vec::with_capacity(t_steps);
    let mut q_mats = Vec::with_capacity(t_steps);
    for step in &req.steps {
        let (u, q) = step_control_and_cov(step, req.grade_to_metal_divisor)?;
        controls.push(u);
        q_mats.push(q);
    }

    let mut pred_mu = vec![[0.0_f64; 2]; n_times];
    let mut pred_p = vec![[0.0_f64; 4]; n_times];
    let mut filt_mu = vec![[0.0_f64; 2]; n_times];
    let mut filt_p = vec![[0.0_f64; 4]; n_times];
    let mut log_likelihood = 0.0_f64;

    pred_mu[0] = [req.prior.opening_tonnes, req.prior.opening_metal];
    pred_p[0] = p0;

    for t in 0..n_times {
        if t > 0 {
            pred_mu[t] = [
                filt_mu[t - 1][0] + controls[t - 1][0],
                filt_mu[t - 1][1] + controls[t - 1][1],
            ];
            pred_p[t] = sym_2x2(add_2x2(filt_p[t - 1], q_mats[t - 1]));
        }
        let mut mu = pred_mu[t];
        let mut p = pred_p[t];
        for obs in &req.observations_by_time[t] {
            let (m_next, p_next, ll_obs) = match *obs {
                StockpileObservation::VolumetricSurvey {
                    observed_tonnes,
                    survey_sd,
                } => {
                    kalman_update_scalar(mu, p, [1.0, 0.0], observed_tonnes, survey_sd * survey_sd)?
                }
                StockpileObservation::MetalStocktake {
                    observed_metal,
                    metal_sd,
                } => kalman_update_scalar(mu, p, [0.0, 1.0], observed_metal, metal_sd * metal_sd)?,
                StockpileObservation::JointStocktake {
                    observed_tonnes,
                    observed_metal,
                    tonnes_sd,
                    metal_sd,
                    corr,
                } => {
                    let r_mat = build_cov_2x2(tonnes_sd, metal_sd, corr, "JointStocktake R")?;
                    kalman_update_joint(mu, p, [observed_tonnes, observed_metal], r_mat)?
                }
            };
            mu = m_next;
            p = p_next;
            log_likelihood += ll_obs;
        }
        filt_mu[t] = mu;
        filt_p[t] = p;
    }

    // Backward Rauch-Tung-Striebel (RTS) smoother pass.
    let mut smooth_mu = vec![[0.0_f64; 2]; n_times];
    let mut smooth_p = vec![[0.0_f64; 4]; n_times];
    let mut rts_gains = vec![[0.0_f64; 4]; t_steps];
    smooth_mu[t_steps] = filt_mu[t_steps];
    smooth_p[t_steps] = filt_p[t_steps];

    for t in (0..t_steps).rev() {
        let p_pred_inv = inv_2x2(pred_p[t + 1], &format!("RTS pred_p[{}]", t + 1))?;
        let g_t = mul_2x2(filt_p[t], p_pred_inv);
        rts_gains[t] = g_t;
        let diff_mu = [
            smooth_mu[t + 1][0] - pred_mu[t + 1][0],
            smooth_mu[t + 1][1] - pred_mu[t + 1][1],
        ];
        let corr_mu = mv_2x2(g_t, diff_mu);
        smooth_mu[t] = [filt_mu[t][0] + corr_mu[0], filt_mu[t][1] + corr_mu[1]];
        let diff_p = sub_2x2(smooth_p[t + 1], pred_p[t + 1]);
        smooth_p[t] = sym_2x2(add_2x2(
            filt_p[t],
            mul_2x2(mul_2x2(g_t, diff_p), trans_2x2(g_t)),
        ));
    }

    let z_crit = det::norm_ppf(0.5 + 0.5 * req.confidence_level);
    let mut periods = Vec::with_capacity(n_times);
    for t in 0..n_times {
        let m_t = smooth_mu[t][0];
        let metal_t = smooth_mu[t][1];
        if !(m_t.is_finite() && m_t > 0.0) {
            return Err(ReconError::NumericalFailure {
                detail: format!("smoothed tonnage at t={t} became non-positive ({m_t})"),
            });
        }
        let sd_t = det::sqrt(smooth_p[t][0].max(0.0));
        let sd_m = det::sqrt(smooth_p[t][3].max(0.0));
        let filt_sd_t = det::sqrt(filt_p[t][0].max(0.0));

        // Derived grade g = divisor * metal / tonnes + exact first-order delta-method variance.
        let grade = req.grade_to_metal_divisor * metal_t / m_t;
        let dg_dm = -req.grade_to_metal_divisor * metal_t / (m_t * m_t);
        let dg_dmetal = req.grade_to_metal_divisor / m_t;
        let var_g = (dg_dm * dg_dm * smooth_p[t][0]
            + 2.0 * dg_dm * dg_dmetal * smooth_p[t][1]
            + dg_dmetal * dg_dmetal * smooth_p[t][3])
            .max(0.0);
        let sd_g = det::sqrt(var_g);

        periods.push(StockpilePeriodEstimate {
            time_index: t,
            tonnes_mean: m_t,
            tonnes_sd: sd_t,
            filtered_tonnes_sd: filt_sd_t,
            metal_mean: metal_t,
            metal_sd: sd_m,
            covariance_2x2: smooth_p[t],
            derived_grade_mean: grade,
            derived_grade_sd: sd_g,
            tonnes_ci_lower: m_t - z_crit * sd_t,
            tonnes_ci_upper: m_t + z_crit * sd_t,
            metal_ci_lower: metal_t - z_crit * sd_m,
            metal_ci_upper: metal_t + z_crit * sd_m,
        });
    }

    // Exact smoothed net inventory changes Delta s_t = s_{t+1} - s_t using lag-1 cross-covariance
    // Cov(s_{t+1}, s_t | y_{0:T}) = P_{t+1|T} * G_t^T.
    let mut deltas = Vec::with_capacity(t_steps);
    for t in 0..t_steps {
        let cov_next_curr = mul_2x2(smooth_p[t + 1], trans_2x2(rts_gains[t]));
        let cov_delta = sym_2x2(sub_2x2(
            add_2x2(smooth_p[t + 1], smooth_p[t]),
            add_2x2(cov_next_curr, trans_2x2(cov_next_curr)),
        ));
        deltas.push(StockpilePeriodDelta {
            period_label: req.steps[t].period_label.clone(),
            delta_tonnes_mean: smooth_mu[t + 1][0] - smooth_mu[t][0],
            delta_tonnes_sd: det::sqrt(cov_delta[0].max(0.0)),
            delta_metal_mean: smooth_mu[t + 1][1] - smooth_mu[t][1],
            delta_metal_sd: det::sqrt(cov_delta[3].max(0.0)),
        });
    }

    let mut enc = Encoder::new_frame(*b"SKP1", 1);
    enc.put_str(&req.stockpile_id);
    enc.put_u32(n_times as u32);
    let canon = |v: f64| {
        if v == 0.0 {
            0.0_f64.to_bits()
        } else {
            v.to_bits()
        }
    };
    for p in &periods {
        enc.put_u64(canon(p.tonnes_mean));
        enc.put_u64(canon(p.metal_mean));
        for c in p.covariance_2x2 {
            enc.put_u64(canon(c));
        }
    }
    let digest_hex = blake3_hash(&enc.finish()).to_hex();

    Ok(StockpileTrajectoryPosterior {
        stockpile_id: req.stockpile_id.clone(),
        periods,
        deltas,
        log_likelihood,
        digest_hex,
    })
}

/// Independent exact joint batch Gaussian posterior solver over the stacked `2(T+1)` state vector
/// (`PO-TOUCH-3` proof obligation verifying RTS smoother = batch Gaussian posterior).
pub fn solve_batch_gaussian_stockpile(
    req: &StockpileStateSpaceRequest,
) -> Result<BatchGaussianStockpileSolution, ReconError> {
    let p0 = validate_request(req)?;
    let t_steps = req.steps.len();
    let n_times = t_steps + 1;
    let dim = 2 * n_times;

    let mut prec = vec![0.0_f64; dim * dim];
    let mut rhs = vec![0.0_f64; dim];

    // Prior block at t = 0: Lambda_0 = P_0^{-1}, rhs_0 = Lambda_0 * mu_0.
    let p0_inv = inv_2x2(p0, "batch prior P_0")?;
    let mu0 = [req.prior.opening_tonnes, req.prior.opening_metal];
    let eta0 = mv_2x2(p0_inv, mu0);
    for a in 0..2 {
        rhs[a] += eta0[a];
        for b in 0..2 {
            prec[a * dim + b] += p0_inv[a * 2 + b];
        }
    }

    // Transition blocks (s_{t+1} - s_t - u_t)^T Q_t^{-1} (s_{t+1} - s_t - u_t).
    for (t, step) in req.steps.iter().enumerate() {
        let (u, q) = step_control_and_cov(step, req.grade_to_metal_divisor)?;
        let q_inv = inv_2x2(q, "batch transition Q_t")?;
        let qinv_u = mv_2x2(q_inv, u);
        let i0 = 2 * t;
        let i1 = 2 * (t + 1);
        for a in 0..2 {
            rhs[i0 + a] -= qinv_u[a];
            rhs[i1 + a] += qinv_u[a];
            for b in 0..2 {
                let v = q_inv[a * 2 + b];
                prec[(i0 + a) * dim + (i0 + b)] += v;
                prec[(i1 + a) * dim + (i1 + b)] += v;
                prec[(i0 + a) * dim + (i1 + b)] -= v;
                prec[(i1 + a) * dim + (i0 + b)] -= v;
            }
        }
    }

    // Observation blocks H_t^T R_t^{-1} H_t and H_t^T R_t^{-1} y_t.
    for (t, obs_list) in req.observations_by_time.iter().enumerate() {
        let base = 2 * t;
        for obs in obs_list {
            match *obs {
                StockpileObservation::VolumetricSurvey {
                    observed_tonnes,
                    survey_sd,
                } => {
                    let inv_v = 1.0 / (survey_sd * survey_sd);
                    prec[base * dim + base] += inv_v;
                    rhs[base] += inv_v * observed_tonnes;
                }
                StockpileObservation::MetalStocktake {
                    observed_metal,
                    metal_sd,
                } => {
                    let inv_v = 1.0 / (metal_sd * metal_sd);
                    prec[(base + 1) * dim + (base + 1)] += inv_v;
                    rhs[base + 1] += inv_v * observed_metal;
                }
                StockpileObservation::JointStocktake {
                    observed_tonnes,
                    observed_metal,
                    tonnes_sd,
                    metal_sd,
                    corr,
                } => {
                    let r_mat = build_cov_2x2(tonnes_sd, metal_sd, corr, "batch JointStocktake R")?;
                    let r_inv = inv_2x2(r_mat, "batch JointStocktake R_inv")?;
                    let rinv_y = mv_2x2(r_inv, [observed_tonnes, observed_metal]);
                    for a in 0..2 {
                        rhs[base + a] += rinv_y[a];
                        for b in 0..2 {
                            prec[(base + a) * dim + (base + b)] += r_inv[a * 2 + b];
                        }
                    }
                }
            }
        }
    }

    let joint_mean = solve_equilibrated_spd(&prec, &rhs, dim)?;
    let mut joint_covariance = vec![0.0_f64; dim * dim];
    for j in 0..dim {
        let mut ej = vec![0.0_f64; dim];
        ej[j] = 1.0;
        let col = solve_equilibrated_spd(&prec, &ej, dim)?;
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

    Ok(BatchGaussianStockpileSolution {
        joint_mean,
        joint_covariance,
    })
}

/// One parcel dumped onto a stockpile with grade and metallurgical sulphur/carbon attributes (§20.8).
#[derive(Clone, Debug, PartialEq)]
pub struct StockpileParcel {
    pub parcel_id: String,
    pub dump_period: usize,
    pub dry_tonnes: f64,
    pub grade: f64,
    pub stot_pct: f64,
    pub sulfide_s_pct: f64,
    pub corg_pct: f64,
}

/// One observed reclaim event used to score competing stockpile mixing models (§20.8).
#[derive(Clone, Debug, PartialEq)]
pub struct ReclaimObservation {
    pub period: usize,
    pub reclaim_tonnes: f64,
    pub observed_grade: f64,
    pub obs_grade_sd: f64,
}

/// Request to score all three declared [`StockpileMixingModel`] candidates on predictive accuracy.
#[derive(Clone, Debug, PartialEq)]
pub struct MixingComparisonRequest {
    pub opening_parcels: Vec<StockpileParcel>,
    pub dumped_parcels: Vec<StockpileParcel>,
    pub reclaims: Vec<ReclaimObservation>,
}

/// Predictive score of one candidate [`StockpileMixingModel`].
#[derive(Clone, Debug, PartialEq)]
pub struct MixingModelScore {
    pub model: StockpileMixingModel,
    /// Predicted reclaim grades per reclaim event in chronological order.
    pub predicted_grades: Vec<f64>,
    /// Total Gaussian one-step-ahead predictive log-likelihood (higher is better).
    pub predictive_log_likelihood: f64,
    /// Mean Gaussian CRPS across reclaim events (lower is better).
    pub mean_crps: f64,
    /// Root mean squared grade prediction error.
    pub rmse_grade: f64,
}

/// Ranked predictive comparison across `WellMixed`, `Fifo`, and `Layered` mixing models (§20.8).
#[derive(Clone, Debug, PartialEq)]
pub struct MixingModelComparison {
    /// Candidate models ranked descending by `predictive_log_likelihood`.
    pub ranked_models: Vec<MixingModelScore>,
    /// Log-likelihood margin between top-ranked and second-ranked mixing model.
    pub top_log_likelihood_margin: f64,
}

fn gaussian_crps(y: f64, mu: f64, sigma: f64) -> f64 {
    let z = (y - mu) / sigma;
    let phi = det::exp(-0.5 * z * z) / det::sqrt(2.0 * std::f64::consts::PI);
    let big_phi = det::norm_cdf(z);
    let inv_sqrt_pi = 1.0 / det::sqrt(std::f64::consts::PI);
    sigma * (z * (2.0 * big_phi - 1.0) + 2.0 * phi - inv_sqrt_pi)
}

/// Simulates `WellMixed`, `Fifo` (oldest-first queue), and `Layered` (stratified top-down LIFO
/// stack) on identical parcel inflows and reclaims, ranking them strictly by predictive
/// log-likelihood and CRPS (§20.8).
pub fn compare_mixing_models(
    req: &MixingComparisonRequest,
) -> Result<MixingModelComparison, ReconError> {
    if req.reclaims.is_empty() {
        return Err(ReconError::invalid(
            "stockpile.mixing.reclaims",
            "at least one reclaim observation is required",
        ));
    }
    for p in req.opening_parcels.iter().chain(&req.dumped_parcels) {
        if !(p.dry_tonnes.is_finite()
            && p.dry_tonnes > 0.0
            && p.grade.is_finite()
            && p.grade >= 0.0)
        {
            return Err(ReconError::invalid(
                "stockpile.mixing.parcels",
                format!("parcel '{}' has invalid dry_tonnes or grade", p.parcel_id),
            ));
        }
    }
    for r in &req.reclaims {
        let s2 = r.obs_grade_sd * r.obs_grade_sd;
        if !(r.reclaim_tonnes.is_finite()
            && r.reclaim_tonnes > 0.0
            && r.observed_grade.is_finite()
            && r.observed_grade >= 0.0
            && r.obs_grade_sd.is_finite()
            && r.obs_grade_sd > 0.0
            && s2.is_finite()
            && s2 > 0.0
            && (1.0 / s2).is_finite())
        {
            return Err(ReconError::invalid(
                "stockpile.mixing.reclaims",
                format!("invalid reclaim observation at period {}", r.period),
            ));
        }
    }

    let mut reclaims_sorted = req.reclaims.clone();
    reclaims_sorted.sort_by_key(|r| r.period);
    let mut dumps_sorted = req.dumped_parcels.clone();
    dumps_sorted.sort_by_key(|p| p.dump_period);

    let candidates = [
        StockpileMixingModel::WellMixed,
        StockpileMixingModel::Fifo,
        StockpileMixingModel::Layered,
    ];
    let mut scores = Vec::with_capacity(candidates.len());

    for model in candidates {
        let mut queue: VecDeque<(f64, f64)> = req
            .opening_parcels
            .iter()
            .map(|p| (p.dry_tonnes, p.grade))
            .collect();
        let mut dump_idx = 0usize;
        let mut predicted_grades = Vec::with_capacity(reclaims_sorted.len());
        let mut ll_sum = 0.0_f64;
        let mut crps_sum = 0.0_f64;
        let mut sq_err_sum = 0.0_f64;

        for rec in &reclaims_sorted {
            while dump_idx < dumps_sorted.len() && dumps_sorted[dump_idx].dump_period <= rec.period
            {
                let p = &dumps_sorted[dump_idx];
                queue.push_back((p.dry_tonnes, p.grade));
                dump_idx += 1;
            }
            let total_t: f64 = queue.iter().map(|(t, _)| *t).sum();
            if total_t + 1.0e-9 < rec.reclaim_tonnes {
                return Err(ReconError::invalid(
                    "stockpile.mixing.reclaims",
                    format!(
                        "reclaim {:.2} t at period {} exceeds available stockpile {:.2} t",
                        rec.reclaim_tonnes, rec.period, total_t
                    ),
                ));
            }
            let pred_grade = match model {
                StockpileMixingModel::WellMixed => {
                    let total_metal: f64 = queue.iter().map(|(t, g)| t * g).sum();
                    let g_mix = total_metal / total_t;
                    let rem_t = (total_t - rec.reclaim_tonnes).max(0.0);
                    queue.clear();
                    if rem_t > 0.0 {
                        queue.push_back((rem_t, g_mix));
                    }
                    g_mix
                }
                StockpileMixingModel::Fifo => {
                    let mut need = rec.reclaim_tonnes;
                    let mut taken_metal = 0.0_f64;
                    while need > 1.0e-12 {
                        let Some((mut t_front, g_front)) = queue.pop_front() else {
                            break;
                        };
                        let take = t_front.min(need);
                        taken_metal += take * g_front;
                        t_front -= take;
                        need -= take;
                        if t_front > 1.0e-12 {
                            queue.push_front((t_front, g_front));
                        }
                    }
                    taken_metal / rec.reclaim_tonnes
                }
                StockpileMixingModel::Layered => {
                    let mut need = rec.reclaim_tonnes;
                    let mut taken_metal = 0.0_f64;
                    while need > 1.0e-12 {
                        let Some((mut t_top, g_top)) = queue.pop_back() else {
                            break;
                        };
                        let take = t_top.min(need);
                        taken_metal += take * g_top;
                        t_top -= take;
                        need -= take;
                        if t_top > 1.0e-12 {
                            queue.push_back((t_top, g_top));
                        }
                    }
                    taken_metal / rec.reclaim_tonnes
                }
            };

            let err = rec.observed_grade - pred_grade;
            let s2 = rec.obs_grade_sd * rec.obs_grade_sd;
            ll_sum += -0.5 * (det::ln(2.0 * std::f64::consts::PI * s2) + (err * err) / s2);
            crps_sum += gaussian_crps(rec.observed_grade, pred_grade, rec.obs_grade_sd);
            sq_err_sum += err * err;
            predicted_grades.push(pred_grade);
        }

        let n = reclaims_sorted.len() as f64;
        scores.push(MixingModelScore {
            model,
            predicted_grades,
            predictive_log_likelihood: ll_sum,
            mean_crps: crps_sum / n,
            rmse_grade: det::sqrt(sq_err_sum / n),
        });
    }

    scores.sort_by(|a, b| {
        b.predictive_log_likelihood
            .total_cmp(&a.predictive_log_likelihood)
            .then_with(|| a.model.cmp(&b.model))
    });
    let top_log_likelihood_margin = if scores.len() >= 2 {
        scores[0].predictive_log_likelihood - scores[1].predictive_log_likelihood
    } else {
        0.0
    };

    Ok(MixingModelComparison {
        ranked_models: scores,
        top_log_likelihood_margin,
    })
}

/// Sulphur-class metallurgical recovery card (e.g. Pueblo Viejo `E255`: L1 `< 7.0%`, L2 `7.0–8.5%`,
/// L3 `> 8.5%` Stot).
#[derive(Clone, Debug, PartialEq)]
pub struct SulfurDegradationCard {
    pub card_id: String,
    pub ore_type: String,
    pub l1_max_stot_pct: f64,
    pub l2_max_stot_pct: f64,
    pub recovery_l1: f64,
    pub recovery_l2: f64,
    pub recovery_l3: f64,
    /// Additional recovery penalty per unit oxidation index `theta_ox` (`>= 0`).
    pub oxidation_penalty_per_unit: f64,
}

impl SulfurDegradationCard {
    /// Pueblo Viejo BSD POX recovery matrix (`FACT E255`: 87% / 87% / 74% across L1/L2/L3 Stot).
    #[must_use]
    pub fn pueblo_viejo_e255_bsd() -> Self {
        Self {
            card_id: "pv.pox.bsd.e255@v1".to_string(),
            ore_type: "BSD".to_string(),
            l1_max_stot_pct: 7.0,
            l2_max_stot_pct: 8.5,
            recovery_l1: 0.87,
            recovery_l2: 0.87,
            recovery_l3: 0.74,
            oxidation_penalty_per_unit: 0.0,
        }
    }

    /// Pueblo Viejo VCL/SP POX recovery matrix (`FACT E255`: 88% / 82% / 82% across L1/L2/L3 Stot).
    #[must_use]
    pub fn pueblo_viejo_e255_vcl_sp() -> Self {
        Self {
            card_id: "pv.pox.vcl_sp.e255@v1".to_string(),
            ore_type: "VCL/SP".to_string(),
            l1_max_stot_pct: 7.0,
            l2_max_stot_pct: 8.5,
            recovery_l1: 0.88,
            recovery_l2: 0.82,
            recovery_l3: 0.82,
            oxidation_penalty_per_unit: 0.0,
        }
    }
}

/// Request to evaluate metallurgical oxidation & degraded-recovery state on a stockpile parcel (§20.8).
#[derive(Clone, Debug, PartialEq)]
pub struct MetallurgicalStateRequest {
    pub stockpile_id: String,
    pub stot_pct_mean: f64,
    pub stot_pct_sd: f64,
    pub sulfide_s_pct_mean: f64,
    pub corg_pct_mean: f64,
    pub exposure_days: f64,
    pub oxidation_rate_per_day: f64,
    pub card: SulfurDegradationCard,
}

/// Posterior metallurgical & sulphur-class degradation state (non-classifying; `G-HALL-1`).
#[derive(Clone, Debug, PartialEq)]
pub struct StockpileMetallurgicalState {
    pub stockpile_id: String,
    pub card_id: String,
    /// Integrated oxidation exposure `theta_ox = rate * exposure_days * (sulfide_s_pct / 100)`.
    pub oxidation_exposure_theta: f64,
    /// Posterior probability mass in L1 (`Stot < l1_max`), L2 (`[l1_max, l2_max]`), L3 (`> l2_max`).
    pub prob_l1: f64,
    pub prob_l2: f64,
    pub prob_l3: f64,
    /// Expected recovery integrating over sulphur-class uncertainty and oxidation penalty.
    pub expected_recovery: f64,
    /// Recovery standard deviation induced by sulphur-class boundary uncertainty.
    pub recovery_sd: f64,
}

impl StockpileMetallurgicalState {
    /// `INV-002` / `G-HALL-1`: degradation uncertainty feeds the QP support pack, never assigns a reserve class.
    #[must_use]
    pub const fn reserve_disclosure_claim(&self) -> bool {
        false
    }
}

/// Evaluates sulphur-class probabilities, oxidation exposure `theta_ox`, and degraded-recovery
/// uncertainty for a stockpile parcel (`BD-154`, §20.8, `E252–E255`).
pub fn evaluate_stockpile_metallurgical_state(
    req: &MetallurgicalStateRequest,
) -> Result<StockpileMetallurgicalState, ReconError> {
    if req.stockpile_id.trim().is_empty() || req.card.card_id.trim().is_empty() {
        return Err(ReconError::invalid(
            "stockpile.metallurgical",
            "stockpile_id and card_id must not be empty",
        ));
    }
    if !(req.stot_pct_mean.is_finite()
        && (0.0..=100.0).contains(&req.stot_pct_mean)
        && req.stot_pct_sd.is_finite()
        && req.stot_pct_sd > 0.0
        && req.sulfide_s_pct_mean.is_finite()
        && (0.0..=100.0).contains(&req.sulfide_s_pct_mean)
        && req.corg_pct_mean.is_finite()
        && (0.0..=100.0).contains(&req.corg_pct_mean)
        && req.exposure_days.is_finite()
        && req.exposure_days >= 0.0
        && req.oxidation_rate_per_day.is_finite()
        && req.oxidation_rate_per_day >= 0.0
        && req.card.l1_max_stot_pct.is_finite()
        && req.card.l2_max_stot_pct.is_finite()
        && req.card.l1_max_stot_pct > 0.0
        && req.card.l1_max_stot_pct < req.card.l2_max_stot_pct
        && req.card.l2_max_stot_pct < 100.0
        && req.card.recovery_l1.is_finite()
        && (0.0..=1.0).contains(&req.card.recovery_l1)
        && req.card.recovery_l2.is_finite()
        && (0.0..=1.0).contains(&req.card.recovery_l2)
        && req.card.recovery_l3.is_finite()
        && (0.0..=1.0).contains(&req.card.recovery_l3)
        && req.card.oxidation_penalty_per_unit.is_finite()
        && req.card.oxidation_penalty_per_unit >= 0.0)
    {
        return Err(ReconError::invalid(
            "stockpile.metallurgical",
            "invalid sulphur, carbon, exposure, or degradation card parameters",
        ));
    }
    let z1 = (req.card.l1_max_stot_pct - req.stot_pct_mean) / req.stot_pct_sd;
    let z2 = (req.card.l2_max_stot_pct - req.stot_pct_mean) / req.stot_pct_sd;
    let p_l1 = det::norm_cdf(z1);
    let p_le_l2 = det::norm_cdf(z2);
    let p_l2 = (p_le_l2 - p_l1).max(0.0);
    let p_l3 = (1.0 - p_le_l2).max(0.0);

    let theta = req.oxidation_rate_per_day * req.exposure_days * (req.sulfide_s_pct_mean / 100.0);
    let pen = req.card.oxidation_penalty_per_unit * theta;
    let r1 = (req.card.recovery_l1 - pen).clamp(0.0, 1.0);
    let r2 = (req.card.recovery_l2 - pen).clamp(0.0, 1.0);
    let r3 = (req.card.recovery_l3 - pen).clamp(0.0, 1.0);

    let mean_r = p_l1 * r1 + p_l2 * r2 + p_l3 * r3;
    let var_r = (p_l1 * (r1 - mean_r).powi(2)
        + p_l2 * (r2 - mean_r).powi(2)
        + p_l3 * (r3 - mean_r).powi(2))
    .max(0.0);

    Ok(StockpileMetallurgicalState {
        stockpile_id: req.stockpile_id.clone(),
        card_id: req.card.card_id.clone(),
        oxidation_exposure_theta: theta,
        prob_l1: p_l1,
        prob_l2: p_l2,
        prob_l3: p_l3,
        expected_recovery: mean_r,
        recovery_sd: det::sqrt(var_r),
    })
}

/// Request to infer unreported stockpile additions and plant surge-cone swing by exact conservation
/// (`BD-154`, §20.8, Pueblo Viejo 2025 MCF Table 15-10 `E258`, `E259`, `E666`).
#[derive(Clone, Debug, PartialEq)]
pub struct StockpileAdditionInferenceRequest {
    pub site_id: String,
    pub period_id: String,
    pub grade_to_metal_divisor: f64,
    pub opening_tonnes: f64,
    pub opening_tonnes_sd: f64,
    pub opening_metal: f64,
    pub opening_metal_sd: f64,
    pub closing_tonnes: f64,
    pub closing_tonnes_sd: f64,
    pub closing_metal: f64,
    pub closing_metal_sd: f64,
    pub reclaim_to_crusher_tonnes: f64,
    pub reclaim_to_crusher_tonnes_sd: f64,
    pub reclaim_to_crusher_metal: f64,
    pub reclaim_to_crusher_metal_sd: f64,
    pub pit_to_crusher_tonnes: f64,
    pub cone_opening_tonnes: f64,
    pub cone_opening_grade: f64,
    pub cone_closing_tonnes: f64,
    pub cone_closing_grade: f64,
    pub confidence_level: f64,
}

/// Exact conservation-inferred unreported stockpile additions and plant surge-cone swing (`E258`, `E666`).
#[derive(Clone, Debug, PartialEq)]
pub struct StockpileAdditionInference {
    pub site_id: String,
    pub period_id: String,
    pub inferred_addition_tonnes: f64,
    pub inferred_addition_tonnes_sd: f64,
    pub inferred_addition_metal: f64,
    pub inferred_addition_metal_sd: f64,
    pub inferred_addition_grade: f64,
    pub inferred_addition_grade_sd: f64,
    pub reclaim_share_of_crusher_feed_pct: f64,
    pub cone_delta_tonnes: f64,
    pub cone_delta_metal: f64,
    pub addition_tonnes_ci: (f64, f64),
    pub addition_metal_ci: (f64, f64),
}

/// Infers unreported stockpile additions (`M_add = M_close - M_open + M_reclaim`,
/// `m_add = m_close - m_open + m_reclaim`) and plant surge-cone swing with exact uncertainty
/// propagation (`BD-154`, `E258`, `E666`).
pub fn infer_stockpile_additions_and_cone(
    req: &StockpileAdditionInferenceRequest,
) -> Result<StockpileAdditionInference, ReconError> {
    if req.site_id.trim().is_empty() || req.period_id.trim().is_empty() {
        return Err(ReconError::invalid(
            "stockpile.addition_inference",
            "site_id and period_id must not be empty",
        ));
    }
    if !(req.grade_to_metal_divisor.is_finite()
        && req.grade_to_metal_divisor > 0.0
        && req.confidence_level.is_finite()
        && req.confidence_level > 0.0
        && req.confidence_level < 1.0)
    {
        return Err(ReconError::invalid(
            "stockpile.addition_inference",
            "invalid grade_to_metal_divisor or confidence_level",
        ));
    }
    let all_nonneg = [
        req.opening_tonnes,
        req.opening_tonnes_sd,
        req.opening_metal,
        req.opening_metal_sd,
        req.closing_tonnes,
        req.closing_tonnes_sd,
        req.closing_metal,
        req.closing_metal_sd,
        req.reclaim_to_crusher_tonnes,
        req.reclaim_to_crusher_tonnes_sd,
        req.reclaim_to_crusher_metal,
        req.reclaim_to_crusher_metal_sd,
        req.pit_to_crusher_tonnes,
        req.cone_opening_tonnes,
        req.cone_opening_grade,
        req.cone_closing_tonnes,
        req.cone_closing_grade,
    ];
    if all_nonneg.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Err(ReconError::invalid(
            "stockpile.addition_inference",
            "all stock, flow, and SD inputs must be finite and >= 0",
        ));
    }
    let crusher_total = req.reclaim_to_crusher_tonnes + req.pit_to_crusher_tonnes;
    if crusher_total <= 0.0 {
        return Err(ReconError::invalid(
            "stockpile.addition_inference",
            "crusher feed total must be > 0",
        ));
    }

    let add_t = req.closing_tonnes - req.opening_tonnes + req.reclaim_to_crusher_tonnes;
    let add_m = req.closing_metal - req.opening_metal + req.reclaim_to_crusher_metal;
    if add_t <= 0.0 || add_m < 0.0 {
        return Err(ReconError::invalid(
            "stockpile.addition_inference",
            format!("implied stockpile addition (tonnes={add_t}, metal={add_m}) is non-physical"),
        ));
    }
    let var_t = req.closing_tonnes_sd.powi(2)
        + req.opening_tonnes_sd.powi(2)
        + req.reclaim_to_crusher_tonnes_sd.powi(2);
    let var_m = req.closing_metal_sd.powi(2)
        + req.opening_metal_sd.powi(2)
        + req.reclaim_to_crusher_metal_sd.powi(2);
    let sd_t = det::sqrt(var_t);
    let sd_m = det::sqrt(var_m);

    let add_g = req.grade_to_metal_divisor * add_m / add_t;
    let dg_dt = -req.grade_to_metal_divisor * add_m / (add_t * add_t);
    let dg_dm = req.grade_to_metal_divisor / add_t;
    let sd_g = det::sqrt((dg_dt * dg_dt * var_t + dg_dm * dg_dm * var_m).max(0.0));

    let cone_open_m = req.cone_opening_tonnes * req.cone_opening_grade / req.grade_to_metal_divisor;
    let cone_close_m =
        req.cone_closing_tonnes * req.cone_closing_grade / req.grade_to_metal_divisor;

    let z = det::norm_ppf(0.5 + 0.5 * req.confidence_level);
    Ok(StockpileAdditionInference {
        site_id: req.site_id.clone(),
        period_id: req.period_id.clone(),
        inferred_addition_tonnes: add_t,
        inferred_addition_tonnes_sd: sd_t,
        inferred_addition_metal: add_m,
        inferred_addition_metal_sd: sd_m,
        inferred_addition_grade: add_g,
        inferred_addition_grade_sd: sd_g,
        reclaim_share_of_crusher_feed_pct: 100.0 * req.reclaim_to_crusher_tonnes / crusher_total,
        cone_delta_tonnes: req.cone_closing_tonnes - req.cone_opening_tonnes,
        cone_delta_metal: cone_close_m - cone_open_m,
        addition_tonnes_ci: (add_t - z * sd_t, add_t + z * sd_t),
        addition_metal_ci: (add_m - z * sd_m, add_m + z * sd_m),
    })
}
