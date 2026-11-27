//! Left-censored assay distribution estimation: Type-I multi-detection-limit lognormal MLE
//! and robust Regression on Order Statistics (ROS), enforcing guard `G-CENS` (DL/2 substitution
//! is unconditionally refused; `BD-168`, plan §12.2).
//! Determinism: BitwisePortable (IEEE-754 arithmetic and `fmine_core::det` primitives only).

use fmine_core::det;
use fmine_drill::records::AssayValue;

use crate::error::QaqcError;
use crate::stats::{inverse_mills_lower, ln_normal_cdf, mean_and_sample_sd, sorted_quantile};

/// Attempted constant substitution of below-detection results, refused by guard `G-CENS`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubstitutionAttempt {
    /// Proposed fraction of detection limit (e.g. `0.5` for `DL/2`).
    pub dl_fraction: f64,
}

/// Guard `G-CENS` (plan §12.2): unconditionally refuses replacing `<DL` values with `DL/2` or any
/// constant fraction of the detection limit.
pub fn refuse_dl_substitution(attempt: &SubstitutionAttempt) -> Result<(), QaqcError> {
    Err(QaqcError::SubstitutionRefused {
        detail: format!(
            "substituting {} * DL for below-detection assays is forbidden by guard G-CENS; use censored MLE or ROS",
            attempt.dl_fraction
        ),
    })
}

/// Input dataset for left-censored lognormal MLE or ROS estimation.
#[derive(Clone, Debug, PartialEq)]
pub struct CensoredDataset {
    /// Assay observations (measured positive mass fractions and `<DL` left-censored bounds).
    /// `AboveUpper` and zero measured values are refused rather than silently dropped or shifted.
    pub values: Vec<AssayValue>,
}

/// Result of multi-detection-limit left-censored lognormal maximum-likelihood estimation (`BD-168`).
#[derive(Clone, Debug, PartialEq)]
pub struct LognormalMleFit {
    /// Total observations (`n_uncensored + n_censored`).
    pub n_total: usize,
    /// Uncensored measured observations.
    pub n_uncensored: usize,
    /// Left-censored (`<DL`) observations.
    pub n_censored: usize,
    /// Censoring fraction (`n_censored / n_total`).
    pub censored_fraction: f64,
    /// Fitted mean of `ln(X)` (`μ_ln`).
    pub mu_log: f64,
    /// Fitted standard deviation of `ln(X)` (`σ_ln > 0`).
    pub sigma_log: f64,
    /// Fitted lognormal median `exp(μ_ln)` (same unit as input).
    pub median: f64,
    /// Fitted lognormal arithmetic mean `exp(μ_ln + 0.5 * σ_ln^2)`.
    pub arithmetic_mean: f64,
    /// Fitted lognormal arithmetic SD `arithmetic_mean * sqrt(exp(σ_ln^2) - 1)`.
    pub arithmetic_sd: f64,
    /// Fitted 90th percentile `exp(μ_ln + Φ^-1(0.90) * σ_ln)`.
    pub p90: f64,
    /// Exact log-likelihood at the MLE on the original scale `X`.
    pub log_likelihood: f64,
    /// Newton–Raphson iterations taken to converge.
    pub iterations: usize,
    /// `true` when censoring exceeds 80% (Helsel reliability warning).
    pub high_censoring_warning: bool,
}

/// Result of robust multi-detection-limit lognormal Regression on Order Statistics (ROS, `BD-168`).
///
/// Censored order statistics are imputed purely inside this descriptive summary calculation and
/// never overwrite L0 assay records (`INV-003`, `G-CENS`).
#[derive(Clone, Debug, PartialEq)]
pub struct RosFit {
    /// Total observations.
    pub n_total: usize,
    /// Uncensored observations.
    pub n_uncensored: usize,
    /// Left-censored observations.
    pub n_censored: usize,
    /// Censoring fraction.
    pub censored_fraction: f64,
    /// Regression intercept on log scale (`μ_ros`).
    pub mu_log: f64,
    /// Regression slope on log scale (`σ_ros > 0`).
    pub sigma_log: f64,
    /// Regression `R²` of `ln(x_uncens)` against normal scores.
    pub r_squared: f64,
    /// Robust ROS arithmetic mean (actual uncensored + modeled censored order statistics).
    pub mean: f64,
    /// Robust ROS sample SD (actual uncensored + modeled censored order statistics).
    pub sd: f64,
    /// Robust ROS 25th percentile.
    pub p25: f64,
    /// Robust ROS median.
    pub median: f64,
    /// Robust ROS 75th percentile.
    pub p75: f64,
    /// Robust ROS 90th percentile.
    pub p90: f64,
    /// Number of modeled censored order statistics whose regression value exceeded their own DL
    /// (reported transparently without silent clamping).
    pub modeled_above_dl_count: usize,
    /// `true` when censoring exceeds 80%.
    pub high_censoring_warning: bool,
}

fn split_positive_and_censored(
    dataset: &CensoredDataset,
) -> Result<(Vec<f64>, Vec<f64>), QaqcError> {
    if dataset.values.is_empty() {
        return Err(QaqcError::InsufficientData {
            context: "censored.dataset".to_string(),
            required: 2,
            actual: 0,
        });
    }
    let mut uncensored = Vec::new();
    let mut censored_limits = Vec::new();
    for (idx, val) in dataset.values.iter().enumerate() {
        match *val {
            AssayValue::Measured { mass_fraction } => {
                if !(mass_fraction.is_finite() && mass_fraction > 0.0 && mass_fraction <= 1.0) {
                    return Err(QaqcError::invalid(
                        "censored.measured",
                        format!(
                            "observation[{idx}]: lognormal fit requires strictly positive mass fraction in (0, 1], got {mass_fraction}"
                        ),
                    ));
                }
                uncensored.push(mass_fraction);
            }
            AssayValue::BelowDetection { limit } => {
                if !(limit.is_finite() && limit > 0.0 && limit <= 1.0) {
                    return Err(QaqcError::invalid(
                        "censored.limit",
                        format!("observation[{idx}]: detection limit {limit} outside (0, 1]"),
                    ));
                }
                censored_limits.push(limit);
            }
            AssayValue::AboveUpper { limit } => {
                return Err(QaqcError::UnusableInput {
                    context: format!("censored.observation[{idx}]"),
                    detail: format!(
                        "right-censored AboveUpper (>{limit}) is not supported by left-censored lognormal fit"
                    ),
                });
            }
        }
    }
    if uncensored.len() < 2 {
        return Err(QaqcError::InsufficientData {
            context: "censored.uncensored_observations".to_string(),
            required: 2,
            actual: uncensored.len(),
        });
    }
    Ok((uncensored, censored_limits))
}

/// Fits a Type-I multi-detection-limit left-censored lognormal distribution by exact maximum
/// likelihood (`BD-168`, plan §12.2).
///
/// Optimizes the exact Tobit/left-censored log-likelihood in `(μ_ln, η = ln σ_ln)` via damped
/// Newton–Raphson with Armijo line search, so `σ_ln = exp(η)` stays strictly positive without
/// clamping.
pub fn fit_censored_lognormal_mle(dataset: &CensoredDataset) -> Result<LognormalMleFit, QaqcError> {
    let (uncensored, censored_limits) = split_positive_and_censored(dataset)?;
    let log_u: Vec<f64> = uncensored.iter().map(|&x| det::ln(x)).collect();
    let log_c: Vec<f64> = censored_limits.iter().map(|&dl| det::ln(dl)).collect();
    let sum_log_u: f64 = log_u.iter().sum();

    let (init_mu, raw_init_sd) =
        mean_and_sample_sd(&log_u).ok_or_else(|| QaqcError::InsufficientData {
            context: "censored.mle".to_string(),
            required: 2,
            actual: log_u.len(),
        })?;

    let n_u = log_u.len();
    let n_c = log_c.len();
    let n_total = n_u + n_c;

    // Fast exact closed-form path when there is no censoring (MLE uses 1/n divisor).
    if n_c == 0 {
        if !(raw_init_sd.is_finite() && raw_init_sd > 1e-12) {
            return Err(QaqcError::NumericalFailure {
                context: "censored.mle".to_string(),
                detail: "uncensored log-values have zero variance; scale parameter is degenerate"
                    .to_string(),
            });
        }
        let mu = init_mu;
        let var_mle = log_u.iter().map(|&y| (y - mu) * (y - mu)).sum::<f64>() / (n_u as f64);
        let sigma = det::sqrt(var_mle);
        let ll = eval_loglik(&log_u, &log_c, sum_log_u, mu, sigma);
        return build_mle_fit(n_total, n_u, 0, mu, sigma, ll, 1);
    }

    // With left-censoring present, tied uncensored values still have a finite interior MLE iff at
    // least one detection limit lies strictly below the tied uncensored value.
    let init_sd = if raw_init_sd.is_finite() && raw_init_sd > 1e-12 {
        raw_init_sd
    } else {
        let min_log_c = log_c.iter().copied().fold(f64::INFINITY, f64::min);
        if min_log_c < init_mu - 1e-12 {
            (init_mu - min_log_c).max(0.5)
        } else {
            return Err(QaqcError::NumericalFailure {
                context: "censored.mle".to_string(),
                detail: "tied uncensored values with no detection limit strictly below them; scale parameter is degenerate"
                    .to_string(),
            });
        }
    };

    // Initialize in (mu, eta = ln sigma), then run damped Newton-Raphson with Armijo line search.
    let mut mu = init_mu;
    let mut eta = det::ln(init_sd);
    let mut ll = eval_loglik(&log_u, &log_c, sum_log_u, mu, det::exp(eta));

    const MAX_ITERS: usize = 200;
    const GRAD_TOL: f64 = 1e-10;
    let mut converged = false;
    let mut iters = 0usize;

    for iter in 1..=MAX_ITERS {
        iters = iter;
        let sigma = det::exp(eta);
        let (g_mu, g_eta, h_mm, h_me, h_ee) = grad_hess(&log_u, &log_c, mu, sigma);
        let grad_norm = det::hypot(g_mu, g_eta);
        if grad_norm < GRAD_TOL {
            converged = true;
            break;
        }

        // Solve (-H) * [d_mu, d_eta] = [g_mu, g_eta] if -H is positive definite; else ascent step.
        let a = -h_mm;
        let b = -h_me;
        let d = -h_ee;
        let det_neg_h = a * d - b * b;
        let (mut d_mu, mut d_eta) = if a > 0.0 && det_neg_h > 1e-18 * (a * d).max(1.0) {
            (
                (d * g_mu - b * g_eta) / det_neg_h,
                (-b * g_mu + a * g_eta) / det_neg_h,
            )
        } else {
            let scale = 1.0 / (grad_norm + 1.0);
            (g_mu * scale, g_eta * scale)
        };

        // Ensure ascent direction.
        let mut dir_deriv = g_mu * d_mu + g_eta * d_eta;
        if dir_deriv <= 0.0 {
            let scale = 1.0 / (grad_norm + 1.0);
            d_mu = g_mu * scale;
            d_eta = g_eta * scale;
            dir_deriv = g_mu * d_mu + g_eta * d_eta;
        }

        // Limit single-step eta jump to avoid exp() overflow during line search.
        let max_eta_step = 2.0;
        if d_eta.abs() > max_eta_step {
            let s = max_eta_step / d_eta.abs();
            d_mu *= s;
            d_eta *= s;
            dir_deriv *= s;
        }

        // Backtracking Armijo line search.
        let mut step = 1.0;
        let mut accepted = false;
        for _ in 0..40 {
            let cand_mu = mu + step * d_mu;
            let cand_eta = eta + step * d_eta;
            let cand_sigma = det::exp(cand_eta);
            if cand_sigma.is_finite() && cand_sigma > 1e-12 && cand_sigma < 1e6 {
                let cand_ll = eval_loglik(&log_u, &log_c, sum_log_u, cand_mu, cand_sigma);
                if cand_ll.is_finite() && cand_ll >= ll + 1e-4 * step * dir_deriv {
                    mu = cand_mu;
                    eta = cand_eta;
                    ll = cand_ll;
                    accepted = true;
                    break;
                }
            }
            step *= 0.5;
        }
        if !accepted {
            // Check if already numerically stationary at machine precision.
            if grad_norm < 1e-6 {
                converged = true;
                break;
            }
            return Err(QaqcError::NumericalFailure {
                context: "censored.mle".to_string(),
                detail: format!(
                    "line search failed to improve log-likelihood at iter {iter} (grad_norm={grad_norm:.3e})"
                ),
            });
        }
    }

    if !converged {
        return Err(QaqcError::NumericalFailure {
            context: "censored.mle".to_string(),
            detail: format!("failed to converge within {MAX_ITERS} iterations"),
        });
    }

    let sigma = det::exp(eta);
    build_mle_fit(n_total, n_u, n_c, mu, sigma, ll, iters)
}

fn eval_loglik(log_u: &[f64], log_c: &[f64], sum_log_u: f64, mu: f64, sigma: f64) -> f64 {
    const HALF_LN_TWO_PI: f64 = 0.918_938_533_204_672_7;
    let n_u = log_u.len() as f64;
    let inv_s = 1.0 / sigma;
    let sq_sum: f64 = log_u
        .iter()
        .map(|&y| {
            let u = (y - mu) * inv_s;
            u * u
        })
        .sum();
    let uncens_ll = -n_u * (HALF_LN_TWO_PI + det::ln(sigma)) - sum_log_u - 0.5 * sq_sum;
    let cens_ll: f64 = log_c.iter().map(|&c| ln_normal_cdf((c - mu) * inv_s)).sum();
    uncens_ll + cens_ll
}

fn grad_hess(log_u: &[f64], log_c: &[f64], mu: f64, sigma: f64) -> (f64, f64, f64, f64, f64) {
    let inv_s = 1.0 / sigma;
    let mut sum_u = 0.0;
    let mut sum_u2 = 0.0;
    for &y in log_u {
        let u = (y - mu) * inv_s;
        sum_u += u;
        sum_u2 += u * u;
    }

    let mut sum_h = 0.0;
    let mut sum_zh = 0.0;
    let mut sum_w = 0.0;
    let mut sum_zw = 0.0;
    let mut sum_z2w = 0.0;
    for &c in log_c {
        let z = (c - mu) * inv_s;
        let h = inverse_mills_lower(z);
        let w = h * (z + h);
        sum_h += h;
        sum_zh += z * h;
        sum_w += w;
        sum_zw += z * w;
        sum_z2w += z * z * w;
    }

    let n_u = log_u.len() as f64;
    let g_mu = inv_s * (sum_u - sum_h);
    let g_eta = (sum_u2 - n_u) - sum_zh;
    let h_mm = (inv_s * inv_s) * (-n_u - sum_w);
    let h_me = inv_s * (-2.0 * sum_u + sum_h - sum_zw);
    let h_ee = -2.0 * sum_u2 + sum_zh - sum_z2w;
    (g_mu, g_eta, h_mm, h_me, h_ee)
}

fn build_mle_fit(
    n_total: usize,
    n_uncensored: usize,
    n_censored: usize,
    mu_log: f64,
    sigma_log: f64,
    log_likelihood: f64,
    iterations: usize,
) -> Result<LognormalMleFit, QaqcError> {
    let censored_fraction = (n_censored as f64) / (n_total as f64);
    let median = det::exp(mu_log);
    let s2 = sigma_log * sigma_log;
    let arithmetic_mean = det::exp(mu_log + 0.5 * s2);
    let arithmetic_sd = arithmetic_mean * det::sqrt(det::expm1(s2).max(0.0));
    let p90 = det::exp(mu_log + det::norm_ppf(0.90) * sigma_log);
    if !(mu_log.is_finite()
        && sigma_log.is_finite()
        && median.is_finite()
        && arithmetic_mean.is_finite()
        && arithmetic_sd.is_finite()
        && p90.is_finite()
        && log_likelihood.is_finite())
    {
        return Err(QaqcError::NumericalFailure {
            context: "censored.mle".to_string(),
            detail: format!(
                "back-transformed lognormal moments overflowed (mu_log={mu_log}, sigma_log={sigma_log})"
            ),
        });
    }
    Ok(LognormalMleFit {
        n_total,
        n_uncensored,
        n_censored,
        censored_fraction,
        mu_log,
        sigma_log,
        median,
        arithmetic_mean,
        arithmetic_sd,
        p90,
        log_likelihood,
        iterations,
        high_censoring_warning: censored_fraction > 0.80,
    })
}

/// Fits robust multi-detection-limit lognormal Regression on Order Statistics (ROS; Helsel 2012,
/// Hirsch & Stedinger 1987 plotting positions; `BD-168`, plan §12.2).
pub fn fit_censored_ros(dataset: &CensoredDataset) -> Result<RosFit, QaqcError> {
    let (mut uncensored, mut censored_limits) = split_positive_and_censored(dataset)?;
    uncensored.sort_by(f64::total_cmp);
    censored_limits.sort_by(f64::total_cmp);

    let n_u = uncensored.len();
    let n_c = censored_limits.len();
    let n_total = n_u + n_c;

    // Build distinct sorted censoring thresholds (DL_1 < DL_2 < ... < DL_k).
    let mut thresholds: Vec<f64> = Vec::new();
    for &dl in &censored_limits {
        if thresholds.last().copied() != Some(dl) {
            thresholds.push(dl);
        }
    }

    // Compute Hirsch-Stedinger exceedance probabilities PE_j = P(X >= DL_j) for multi-DL ROS.
    let (uncens_probs, cens_probs_with_dl) = if thresholds.is_empty() {
        let up: Vec<f64> = (1..=n_u)
            .map(|r| (r as f64) / ((n_total + 1) as f64))
            .collect();
        (up, Vec::new())
    } else {
        hirsch_stedinger_plotting_positions(&uncensored, &censored_limits, &thresholds)
    };

    // Fit OLS regression of ln(x_u) on normal scores z_i = Phi^-1(p_i).
    let zs: Vec<f64> = uncens_probs.iter().map(|&p| det::norm_ppf(p)).collect();
    let ys: Vec<f64> = uncensored.iter().map(|&x| det::ln(x)).collect();
    let z_bar = zs.iter().sum::<f64>() / (n_u as f64);
    let y_bar = ys.iter().sum::<f64>() / (n_u as f64);
    let szz: f64 = zs.iter().map(|&z| (z - z_bar) * (z - z_bar)).sum();
    let syy: f64 = ys.iter().map(|&y| (y - y_bar) * (y - y_bar)).sum();
    let szy: f64 = zs
        .iter()
        .zip(&ys)
        .map(|(&z, &y)| (z - z_bar) * (y - y_bar))
        .sum();

    if !(szz > 0.0 && syy > 0.0) {
        return Err(QaqcError::NumericalFailure {
            context: "censored.ros".to_string(),
            detail: "degenerate uncensored values or normal scores; ROS slope undefined"
                .to_string(),
        });
    }
    let sigma_log = szy / szz;
    if !(sigma_log.is_finite() && sigma_log > 0.0) {
        return Err(QaqcError::NumericalFailure {
            context: "censored.ros".to_string(),
            detail: format!("non-positive ROS slope {sigma_log}"),
        });
    }
    let mu_log = y_bar - sigma_log * z_bar;
    if !mu_log.is_finite() {
        return Err(QaqcError::NumericalFailure {
            context: "censored.ros".to_string(),
            detail: format!("non-finite ROS intercept {mu_log}"),
        });
    }
    let r_squared = ((szy * szy) / (szz * syy)).clamp(0.0, 1.0);

    // Combine actual uncensored values with modeled censored order statistics for descriptive stats.
    let mut combined = uncensored;
    let mut modeled_above_dl_count = 0usize;
    for (p, dl) in cens_probs_with_dl {
        let z = det::norm_ppf(p);
        let modeled = det::exp(mu_log + sigma_log * z);
        if !modeled.is_finite() {
            return Err(QaqcError::NumericalFailure {
                context: "censored.ros".to_string(),
                detail: format!("modeled censored order statistic overflowed at p={p}"),
            });
        }
        if modeled > dl {
            modeled_above_dl_count += 1;
        }
        combined.push(modeled);
    }
    combined.sort_by(f64::total_cmp);

    let (mean, sd) = mean_and_sample_sd(&combined).ok_or_else(|| QaqcError::NumericalFailure {
        context: "censored.ros".to_string(),
        detail: "failed to compute ROS summary moments".to_string(),
    })?;
    let p25 = sorted_quantile(&combined, 0.25);
    let median = sorted_quantile(&combined, 0.50);
    let p75 = sorted_quantile(&combined, 0.75);
    let p90 = sorted_quantile(&combined, 0.90);
    if !(mean.is_finite()
        && sd.is_finite()
        && p25.is_finite()
        && median.is_finite()
        && p75.is_finite()
        && p90.is_finite())
    {
        return Err(QaqcError::NumericalFailure {
            context: "censored.ros".to_string(),
            detail: "ROS summary moments overflowed to non-finite values".to_string(),
        });
    }
    let censored_fraction = (n_c as f64) / (n_total as f64);

    Ok(RosFit {
        n_total,
        n_uncensored: n_u,
        n_censored: n_c,
        censored_fraction,
        mu_log,
        sigma_log,
        r_squared,
        mean,
        sd,
        p25,
        median,
        p75,
        p90,
        modeled_above_dl_count,
        high_censoring_warning: censored_fraction > 0.80,
    })
}

/// Hirsch & Stedinger (1987) / Helsel (2012) multi-DL ROS plotting positions.
fn hirsch_stedinger_plotting_positions(
    uncensored_sorted: &[f64],
    censored_sorted: &[f64],
    thresholds: &[f64],
) -> (Vec<f64>, Vec<(f64, f64)>) {
    let k = thresholds.len();
    // For each threshold j = 0..k:
    //   A_j = count of uncensored values in [DL_j, DL_{j+1}) (with DL_k = +inf)
    //   C_j = count of censored values at DL_j
    //   B_j = count of all observations (uncensored + censored) strictly below DL_j, plus C_j
    //         (i.e. all observations <= DL_j that are censored, plus uncensored < DL_j).
    let mut a_counts = vec![0usize; k];
    let mut c_counts = vec![0usize; k];
    let mut uncens_below_dl0 = 0usize;

    for &x in uncensored_sorted {
        if x < thresholds[0] {
            uncens_below_dl0 += 1;
        } else {
            let mut bucket = k - 1;
            for j in 0..k - 1 {
                if x >= thresholds[j] && x < thresholds[j + 1] {
                    bucket = j;
                    break;
                }
            }
            a_counts[bucket] += 1;
        }
    }
    for &dl in censored_sorted {
        if let Some(j) = thresholds.iter().position(|&t| t == dl) {
            c_counts[j] += 1;
        }
    }

    // Exceedance probability pe[j] = P(X >= DL_j), computed downward from j = k - 1 to 0.
    // pe[k] = 0.0 (above +infinity).
    let mut pe = vec![0.0; k + 1];
    for j in (0..k).rev() {
        // B_j = observations strictly below DL_j (uncensored < DL_j) + all censored with limit <= DL_j.
        let uncens_lt_j = uncensored_sorted
            .iter()
            .filter(|&&x| x < thresholds[j])
            .count();
        let cens_le_j = censored_sorted
            .iter()
            .filter(|&&dl| dl <= thresholds[j])
            .count();
        let b_j = uncens_lt_j + cens_le_j;
        let denom = (a_counts[j] + b_j) as f64;
        if denom > 0.0 {
            pe[j] = pe[j + 1] + ((a_counts[j] as f64) / denom) * (1.0 - pe[j + 1]);
        } else {
            pe[j] = pe[j + 1];
        }
    }

    // Uncensored plotting positions in sorted order.
    let mut uncens_probs = Vec::with_capacity(uncensored_sorted.len());
    // Uncensored observations strictly below the smallest detection limit DL_0 occupy the lower
    // Hirsch-Stedinger interval [0, 1 - pe[0]) (corresponding to pe[-1] = 1.0), spaced evenly as
    // (1 - pe[0]) * r / (uncens_below_dl0 + 1).
    let mut r_below = 0usize;
    let mut bucket_rank = vec![0usize; k];
    for &x in uncensored_sorted {
        if x < thresholds[0] {
            r_below += 1;
            let p = (1.0 - pe[0]) * (r_below as f64) / ((uncens_below_dl0 + 1) as f64);
            uncens_probs.push(p.clamp(1e-9, 1.0 - 1e-9));
        } else {
            let mut bucket = k - 1;
            for j in 0..k - 1 {
                if x >= thresholds[j] && x < thresholds[j + 1] {
                    bucket = j;
                    break;
                }
            }
            bucket_rank[bucket] += 1;
            let span = pe[bucket] - pe[bucket + 1];
            let p = (1.0 - pe[bucket])
                + span * (bucket_rank[bucket] as f64) / ((a_counts[bucket] + 1) as f64);
            uncens_probs.push(p.clamp(1e-9, 1.0 - 1e-9));
        }
    }

    // Censored plotting positions for each threshold j: evenly spaced in (0, 1 - pe[j]).
    let mut cens_probs = Vec::with_capacity(censored_sorted.len());
    for (j, &dl) in thresholds.iter().enumerate() {
        let cj = c_counts[j];
        for r in 1..=cj {
            let p = (1.0 - pe[j]) * (r as f64) / ((cj + 1) as f64);
            cens_probs.push((p.clamp(1e-9, 1.0 - 1e-9), dl));
        }
    }

    (uncens_probs, cens_probs)
}
