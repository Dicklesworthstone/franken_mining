//! Vehtari et al. (2021) rank-normalized split-R-hat, Bulk ESS, Tail ESS, E-BFMI, and fail-closed
//! `RhatEssCertificate` (`BD-232`, `PO-TOUCH-4`).
//! Determinism: BitwisePortable

use std::cmp::Ordering;

use fmine_core::det;
use fmine_evidence::{Refusal, RefusalCode};

/// Publication gate thresholds for [`RhatEssCertificate`] (`PO-TOUCH-4`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiagnosticThresholds {
    /// Maximum permitted rank-normalized split-$\hat{R}$ (default `1.01`, Vehtari et al. 2021 & `PO-TOUCH-4`).
    pub max_rhat: f64,
    /// Minimum permitted Bulk ESS across all chains (default `400.0`).
    pub min_bulk_ess: f64,
    /// Minimum permitted Tail ESS across all chains (default `400.0`).
    pub min_tail_ess: f64,
    /// Maximum permitted post-warmup Hamiltonian divergences (default `0`).
    pub max_divergences: usize,
    /// Minimum permitted per-chain Energy-BFMI (default `0.3`, Betancourt 2016).
    pub min_ebfmi: f64,
}

impl Default for DiagnosticThresholds {
    fn default() -> Self {
        Self {
            max_rhat: 1.01,
            min_bulk_ess: 400.0,
            min_tail_ess: 400.0,
            max_divergences: 0,
            min_ebfmi: 0.3,
        }
    }
}

impl DiagnosticThresholds {
    /// Validate threshold values.
    pub fn validate(&self) -> Result<(), Refusal> {
        if !(self.max_rhat.is_finite() && self.max_rhat > 1.0) {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("max_rhat must be finite and > 1.0, got {}", self.max_rhat),
            ));
        }
        if !(self.min_bulk_ess.is_finite() && self.min_bulk_ess > 0.0)
            || !(self.min_tail_ess.is_finite() && self.min_tail_ess > 0.0)
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "min_bulk_ess and min_tail_ess must be strictly positive and finite",
            ));
        }
        if !(self.min_ebfmi.is_finite() && self.min_ebfmi >= 0.0 && self.min_ebfmi <= 1.0) {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("min_ebfmi must lie in [0, 1], got {}", self.min_ebfmi),
            ));
        }
        Ok(())
    }
}

/// Per-chain NUTS runtime telemetry needed for divergence, treedepth, and E-BFMI certification.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainRuntimeStats {
    pub chain_id: usize,
    pub adapted_step_size: f64,
    pub post_warmup_divergences: usize,
    pub max_treedepth_hits: usize,
    pub mean_accept_prob: f64,
    /// Post-warmup Hamiltonian energies $H_n$ for E-BFMI computation.
    pub post_warmup_energies: Vec<f64>,
}

/// Posterior summary and Vehtari et al. (2021) convergence diagnostics for one scalar parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterDiagnostics {
    pub name: String,
    pub mean: f64,
    pub sd: f64,
    pub q05: f64,
    pub q50: f64,
    pub q95: f64,
    /// `max(split_rhat_bulk, split_rhat_folded)`; `None` only when draws are constant/degenerate.
    pub split_rhat: Option<f64>,
    pub split_rhat_bulk: Option<f64>,
    pub split_rhat_folded: Option<f64>,
    pub bulk_ess: f64,
    pub tail_ess: f64,
    pub raw_ess: f64,
    pub mcse_mean: Option<f64>,
}

/// Specific reason a [`RhatEssCertificate`] failed certification.
#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticFailure {
    DegenerateParameter {
        param: String,
    },
    RhatExceeded {
        param: String,
        rhat: f64,
        limit: f64,
    },
    BulkEssInsufficient {
        param: String,
        bulk_ess: f64,
        limit: f64,
    },
    TailEssInsufficient {
        param: String,
        tail_ess: f64,
        limit: f64,
    },
    DivergencesExceeded {
        total_divergences: usize,
        limit: usize,
    },
    EbfmiTooLow {
        chain_id: usize,
        ebfmi: f64,
        limit: f64,
    },
}

/// Overall verdict of a [`RhatEssCertificate`].
#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticVerdict {
    Certified,
    Refused(Vec<DiagnosticFailure>),
}

/// Complete MCMC convergence and effective-sample-size certificate (`BD-232`, `PO-TOUCH-4`).
#[derive(Debug, Clone, PartialEq)]
pub struct RhatEssCertificate {
    pub num_chains: usize,
    pub draws_per_chain: usize,
    pub total_divergences: usize,
    pub total_max_treedepth_hits: usize,
    pub per_chain_ebfmi: Vec<Option<f64>>,
    pub thresholds: DiagnosticThresholds,
    pub parameters: Vec<ParameterDiagnostics>,
    pub verdict: DiagnosticVerdict,
}

impl RhatEssCertificate {
    /// Returns `Ok(())` iff every parameter and chain passed every threshold; otherwise refuses
    /// with `FMINE-REF-207` (`PO-TOUCH-4`).
    pub fn require_certified(&self) -> Result<(), Refusal> {
        match &self.verdict {
            DiagnosticVerdict::Certified => Ok(()),
            DiagnosticVerdict::Refused(failures) => Err(Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                format!(
                    "MCMC R-hat/ESS certificate refused ({} failure(s)): {:?}",
                    failures.len(),
                    failures
                ),
            )),
        }
    }
}

/// Compute Energy Bayesian Fraction of Missing Information (E-BFMI, Betancourt 2016) for one chain.
/// Returns `None` if fewer than 2 finite energies exist or energy variance is zero.
#[must_use]
pub fn compute_ebfmi(energies: &[f64]) -> Option<f64> {
    let n = energies.len();
    if n < 2 || energies.iter().any(|e| !e.is_finite()) {
        return None;
    }
    let mean = energies.iter().sum::<f64>() / (n as f64);
    let denom: f64 = energies
        .iter()
        .map(|&e| {
            let d = e - mean;
            d * d
        })
        .sum();
    if !(denom.is_finite() && denom > 0.0) {
        return None;
    }
    let numer: f64 = energies
        .windows(2)
        .map(|w| {
            let d = w[1] - w[0];
            d * d
        })
        .sum();
    if !numer.is_finite() {
        return None;
    }
    Some(numer / denom)
}

/// Certify multi-chain posterior draws (`chains[chain][draw][param]`) against `thresholds`.
pub fn certify_chains(
    chains: &[Vec<Vec<f64>>],
    param_names: &[String],
    chain_stats: &[ChainRuntimeStats],
    thresholds: &DiagnosticThresholds,
) -> Result<RhatEssCertificate, Refusal> {
    thresholds.validate()?;
    let num_chains = chains.len();
    if num_chains < 2 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("R-hat/ESS certification requires at least 2 chains, got {num_chains}"),
        ));
    }
    if chain_stats.len() != num_chains {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "chain_stats count ({}) must match chain count ({num_chains})",
                chain_stats.len()
            ),
        ));
    }
    let draws_per_chain = chains[0].len();
    if draws_per_chain < 4 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "R-hat/ESS certification requires at least 4 post-warmup draws per chain, got {draws_per_chain}"
            ),
        ));
    }
    let dim = param_names.len();
    if dim == 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "at least one parameter name is required",
        ));
    }
    let mut seen_names = std::collections::BTreeSet::new();
    for name in param_names {
        if name.trim().is_empty() || !seen_names.insert(name.as_str()) {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("parameter name {name:?} is empty or duplicated"),
            ));
        }
    }
    for (c_idx, chain) in chains.iter().enumerate() {
        if chain.len() != draws_per_chain {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "chain {c_idx} draw count ({}) differs from chain 0 ({draws_per_chain})",
                    chain.len()
                ),
            ));
        }
        for (d_idx, draw) in chain.iter().enumerate() {
            if draw.len() != dim || draw.iter().any(|v| !v.is_finite()) {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("chain {c_idx} draw {d_idx} has wrong dimension or non-finite value"),
                ));
            }
        }
    }

    let mut failures = Vec::new();
    let total_divergences: usize = chain_stats
        .iter()
        .map(|s| s.post_warmup_divergences)
        .fold(0usize, usize::saturating_add);
    let total_max_treedepth_hits: usize = chain_stats
        .iter()
        .map(|s| s.max_treedepth_hits)
        .fold(0usize, usize::saturating_add);
    if total_divergences > thresholds.max_divergences {
        failures.push(DiagnosticFailure::DivergencesExceeded {
            total_divergences,
            limit: thresholds.max_divergences,
        });
    }

    let mut per_chain_ebfmi = Vec::with_capacity(num_chains);
    for stats in chain_stats {
        let ebfmi = compute_ebfmi(&stats.post_warmup_energies);
        match ebfmi {
            Some(val) if val < thresholds.min_ebfmi => {
                failures.push(DiagnosticFailure::EbfmiTooLow {
                    chain_id: stats.chain_id,
                    ebfmi: val,
                    limit: thresholds.min_ebfmi,
                });
            }
            None if thresholds.min_ebfmi > 0.0 => {
                failures.push(DiagnosticFailure::EbfmiTooLow {
                    chain_id: stats.chain_id,
                    ebfmi: 0.0,
                    limit: thresholds.min_ebfmi,
                });
            }
            _ => {}
        }
        per_chain_ebfmi.push(ebfmi);
    }

    let mut parameters = Vec::with_capacity(dim);
    for (p_idx, name) in param_names.iter().enumerate() {
        let scalar_chains: Vec<Vec<f64>> = chains
            .iter()
            .map(|c| c.iter().map(|d| d[p_idx]).collect())
            .collect();
        let diag = compute_parameter_diagnostics(name, &scalar_chains);
        match diag.split_rhat {
            None => failures.push(DiagnosticFailure::DegenerateParameter {
                param: name.clone(),
            }),
            Some(rhat) => {
                if rhat > thresholds.max_rhat {
                    failures.push(DiagnosticFailure::RhatExceeded {
                        param: name.clone(),
                        rhat,
                        limit: thresholds.max_rhat,
                    });
                }
            }
        }
        if diag.bulk_ess < thresholds.min_bulk_ess {
            failures.push(DiagnosticFailure::BulkEssInsufficient {
                param: name.clone(),
                bulk_ess: diag.bulk_ess,
                limit: thresholds.min_bulk_ess,
            });
        }
        if diag.tail_ess < thresholds.min_tail_ess {
            failures.push(DiagnosticFailure::TailEssInsufficient {
                param: name.clone(),
                tail_ess: diag.tail_ess,
                limit: thresholds.min_tail_ess,
            });
        }
        parameters.push(diag);
    }

    let verdict = if failures.is_empty() {
        DiagnosticVerdict::Certified
    } else {
        DiagnosticVerdict::Refused(failures)
    };

    Ok(RhatEssCertificate {
        num_chains,
        draws_per_chain,
        total_divergences,
        total_max_treedepth_hits,
        per_chain_ebfmi,
        thresholds: *thresholds,
        parameters,
        verdict,
    })
}

/// Compute Vehtari et al. (2021) summary statistics, rank-normalized split-$\hat{R}$, Bulk ESS,
/// Tail ESS, and MCSE for one scalar parameter across `chains` (`M >= 2`, `N >= 4`).
#[must_use]
pub fn compute_parameter_diagnostics(name: &str, chains: &[Vec<f64>]) -> ParameterDiagnostics {
    let mut pooled: Vec<f64> = chains.iter().flat_map(|c| c.iter().copied()).collect();
    if pooled.is_empty() || pooled.iter().any(|v| !v.is_finite()) {
        return ParameterDiagnostics {
            name: name.to_string(),
            mean: 0.0,
            sd: 0.0,
            q05: 0.0,
            q50: 0.0,
            q95: 0.0,
            split_rhat: None,
            split_rhat_bulk: None,
            split_rhat_folded: None,
            bulk_ess: 0.0,
            tail_ess: 0.0,
            raw_ess: 0.0,
            mcse_mean: None,
        };
    }
    let total = pooled.len() as f64;
    let mean = pooled.iter().sum::<f64>() / total;
    let var = if pooled.len() > 1 {
        pooled
            .iter()
            .map(|&x| {
                let d = x - mean;
                d * d
            })
            .sum::<f64>()
            / ((pooled.len() - 1) as f64)
    } else {
        0.0
    };
    let sd = det::sqrt(var.max(0.0));
    pooled.sort_by(total_cmp_f64);
    let q05 = linear_quantile_sorted(&pooled, 0.05);
    let q50 = linear_quantile_sorted(&pooled, 0.50);
    let q95 = linear_quantile_sorted(&pooled, 0.95);

    // Require >= 2 equal-length chains with >= 4 draws each for split-R-hat and split-ESS.
    let equal_len = chains.first().map_or(0, Vec::len);
    let well_formed =
        chains.len() >= 2 && equal_len >= 4 && chains.iter().all(|c| c.len() == equal_len);
    if !well_formed {
        return ParameterDiagnostics {
            name: name.to_string(),
            mean,
            sd,
            q05,
            q50,
            q95,
            split_rhat: None,
            split_rhat_bulk: None,
            split_rhat_folded: None,
            bulk_ess: 0.0,
            tail_ess: 0.0,
            raw_ess: 0.0,
            mcse_mean: None,
        };
    }

    let split_raw = split_chains(chains);
    let rank_bulk = rank_normalize(&split_raw);
    let folded_raw: Vec<Vec<f64>> = split_raw
        .iter()
        .map(|c| c.iter().map(|&x| (x - q50).abs()).collect())
        .collect();
    let rank_folded = rank_normalize(&folded_raw);

    let split_rhat_bulk = classic_rhat(&rank_bulk);
    let split_rhat_folded = classic_rhat(&rank_folded);
    let split_rhat = match (split_rhat_bulk, split_rhat_folded) {
        (Some(a), Some(b)) => Some(a.max(b)),
        _ => None,
    };

    let bulk_ess = split_ess(&rank_bulk);
    let raw_ess = split_ess(&split_raw);

    let ind_lo: Vec<Vec<f64>> = split_raw
        .iter()
        .map(|c| {
            c.iter()
                .map(|&x| if x <= q05 { 1.0 } else { 0.0 })
                .collect()
        })
        .collect();
    let ind_hi: Vec<Vec<f64>> = split_raw
        .iter()
        .map(|c| {
            c.iter()
                .map(|&x| if x <= q95 { 1.0 } else { 0.0 })
                .collect()
        })
        .collect();
    let tail_ess = split_ess(&ind_lo).min(split_ess(&ind_hi));

    let mcse_mean = if raw_ess > 0.0 && sd.is_finite() {
        Some(sd / det::sqrt(raw_ess))
    } else {
        None
    };

    ParameterDiagnostics {
        name: name.to_string(),
        mean,
        sd,
        q05,
        q50,
        q95,
        split_rhat,
        split_rhat_bulk,
        split_rhat_folded,
        bulk_ess,
        tail_ess,
        raw_ess,
        mcse_mean,
    }
}

fn total_cmp_f64(a: &f64, b: &f64) -> Ordering {
    a.total_cmp(b)
}

fn linear_quantile_sorted(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let pos = p * ((sorted.len() - 1) as f64);
    let lo = pos.floor() as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    let frac = pos - (lo as f64);
    sorted[lo] * (1.0 - frac) + sorted[hi] * frac
}

/// Split each chain into two equal halves (dropping the first draw if length is odd).
fn split_chains(chains: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let mut out = Vec::with_capacity(chains.len() * 2);
    for c in chains {
        let half = c.len() / 2;
        let start = c.len() - 2 * half;
        out.push(c[start..start + half].to_vec());
        out.push(c[start + half..start + 2 * half].to_vec());
    }
    out
}

/// Blom rank-normalization $\Phi^{-1}((r - 3/8) / (S + 1/4))$ with average ranks for ties
/// (Vehtari et al. 2021 §3).
fn rank_normalize(chains: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let m = chains.len();
    let n = chains.first().map_or(0, Vec::len);
    let s = m * n;
    let mut tagged: Vec<(f64, usize, usize)> = Vec::with_capacity(s);
    for (ci, chain) in chains.iter().enumerate() {
        for (di, &val) in chain.iter().enumerate() {
            tagged.push((val, ci, di));
        }
    }
    tagged.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut out = vec![vec![0.0_f64; n]; m];
    let denom = (s as f64) + 0.25;
    let mut i = 0usize;
    while i < s {
        let mut j = i + 1;
        while j < s && tagged[j].0 == tagged[i].0 {
            j += 1;
        }
        // 1-based average rank over [i + 1, j].
        let avg_rank = 0.5 * ((i + 1) as f64 + (j as f64));
        let z = det::norm_ppf((avg_rank - 0.375) / denom);
        for &(_, ci, di) in &tagged[i..j] {
            out[ci][di] = z;
        }
        i = j;
    }
    out
}

/// Classical Gelman-Rubin $\hat{R} = \sqrt{\widehat{\mathrm{var}}^+ / W}$ on already-split chains.
/// Returns `None` when within-chain variance $W = 0$ (constant/degenerate draws).
fn classic_rhat(chains: &[Vec<f64>]) -> Option<f64> {
    let m = chains.len();
    let n = chains.first().map_or(0, Vec::len);
    if m < 2 || n < 2 {
        return None;
    }
    let nf = n as f64;
    let mf = m as f64;
    let mut means = Vec::with_capacity(m);
    let mut w_sum = 0.0_f64;
    for chain in chains {
        let mu = chain.iter().sum::<f64>() / nf;
        means.push(mu);
        let s2 = chain
            .iter()
            .map(|&x| {
                let d = x - mu;
                d * d
            })
            .sum::<f64>()
            / (nf - 1.0);
        w_sum += s2;
    }
    let w = w_sum / mf;
    let grand = means.iter().sum::<f64>() / mf;
    let b = nf
        * means
            .iter()
            .map(|&mu| {
                let d = mu - grand;
                d * d
            })
            .sum::<f64>()
        / (mf - 1.0);

    if w == 0.0 {
        return None;
    }
    let var_plus = ((nf - 1.0) / nf) * w + b / nf;
    if !(var_plus.is_finite() && var_plus >= 0.0) {
        return None;
    }
    Some(det::sqrt(var_plus / w))
}

/// Multi-chain effective sample size via Geyer's Initial Positive Monotone Sequence
/// (Vehtari et al. 2021 §3.2) using `BitwisePortable` autocovariance estimation.
#[must_use]
pub(crate) fn split_ess(chains: &[Vec<f64>]) -> f64 {
    let m = chains.len();
    let n = chains.first().map_or(0, Vec::len);
    if m < 2 || n < 2 {
        return 0.0;
    }
    let nf = n as f64;
    let mf = m as f64;

    let mut means = Vec::with_capacity(m);
    let mut s2s = Vec::with_capacity(m);
    let mut acovs = Vec::with_capacity(m);
    for chain in chains {
        let mu = chain.iter().sum::<f64>() / nf;
        means.push(mu);
        let centered: Vec<f64> = chain.iter().map(|&x| x - mu).collect();
        let ac = chain_autocovariance(&centered);
        // s^2 = n/(n-1) * ac[0]
        let s2 = ac[0] * (nf / (nf - 1.0));
        s2s.push(s2);
        acovs.push(ac);
    }
    let w = s2s.iter().sum::<f64>() / mf;
    let grand = means.iter().sum::<f64>() / mf;
    let b = nf
        * means
            .iter()
            .map(|&mu| {
                let d = mu - grand;
                d * d
            })
            .sum::<f64>()
        / (mf - 1.0);
    let var_plus = ((nf - 1.0) / nf) * w + b / nf;
    if !(var_plus.is_finite() && var_plus > 0.0) {
        return 0.0;
    }

    // Combined multi-chain autocorrelation rho_hat(k) = 1 - (W - mean_acov(k)) / var_plus,
    // where mean_acov(k) scales biased ac[k] by n/(n-1) so mean_acov(0) == W and rho_hat(0) == 1.
    let scale = nf / (nf - 1.0);
    let mut rho_hat = Vec::with_capacity(n);
    for k in 0..n {
        let mean_ac = acovs.iter().map(|ac| ac[k] * scale).sum::<f64>() / mf;
        rho_hat.push(1.0 - (w - mean_ac) / var_plus);
    }

    // Geyer's Initial Positive Monotone Sequence over pairs P_t = rho_{2t} + rho_{2t+1}.
    let max_pairs = n / 2;
    let mut p_sum = 0.0_f64;
    let mut prev_pair = f64::INFINITY;
    for t in 0..max_pairs {
        let pair = rho_hat[2 * t] + rho_hat[2 * t + 1];
        if pair <= 0.0 || !pair.is_finite() {
            break;
        }
        let mono_pair = pair.min(prev_pair);
        prev_pair = mono_pair;
        p_sum += mono_pair;
    }

    let tau = (-1.0 + 2.0 * p_sum).max(1.0 / det::ln((m * n) as f64).max(1.0));
    let total = (m * n) as f64;
    // Cap super-efficient antithetic ESS at 2 * S to prevent pathological over-reporting.
    (total / tau).min(2.0 * total)
}

/// Biased positive-definite autocovariance `(1/n) * sum_{t=0}^{n-1-k} u[t] * u[t+k]` for `k = 0..n`.
/// Uses direct summation for `n <= 256` and `BitwisePortable` radix-2 FFT for `n > 256`.
fn chain_autocovariance(centered: &[f64]) -> Vec<f64> {
    let n = centered.len();
    if n == 0 {
        return Vec::new();
    }
    let nf = n as f64;
    if n <= 256 {
        let mut out = vec![0.0_f64; n];
        for (k, slot) in out.iter_mut().enumerate() {
            let mut s = 0.0_f64;
            for t in 0..(n - k) {
                s += centered[t] * centered[t + k];
            }
            *slot = s / nf;
        }
        return out;
    }

    let fft_len = (2 * n).next_power_of_two();
    let mut re = vec![0.0_f64; fft_len];
    let mut im = vec![0.0_f64; fft_len];
    re[..n].copy_from_slice(centered);
    fft_radix2(&mut re, &mut im, false);
    for i in 0..fft_len {
        let mag2 = re[i] * re[i] + im[i] * im[i];
        re[i] = mag2;
        im[i] = 0.0;
    }
    fft_radix2(&mut re, &mut im, true);
    let inv_scale = 1.0 / ((fft_len as f64) * nf);
    re[..n].iter().map(|&v| v * inv_scale).collect()
}

/// In-place `BitwisePortable` radix-2 Cooley-Tukey FFT (`det::sin_cos`).
fn fft_radix2(re: &mut [f64], im: &mut [f64], inverse: bool) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);

    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while (j & bit) != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }

    let sign = if inverse { 1.0 } else { -1.0 };
    let mut len = 2usize;
    while len <= n {
        let half = len >> 1;
        let angle_step = sign * (2.0 * core::f64::consts::PI) / (len as f64);
        for start in (0..n).step_by(len) {
            for k in 0..half {
                let (s, c) = det::sin_cos(angle_step * (k as f64));
                let idx1 = start + k;
                let idx2 = idx1 + half;
                let tr = re[idx2] * c - im[idx2] * s;
                let ti = re[idx2] * s + im[idx2] * c;
                re[idx2] = re[idx1] - tr;
                im[idx2] = im[idx1] - ti;
                re[idx1] += tr;
                im[idx1] += ti;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::rand::{PhiloxStream, RngKey};

    #[test]
    fn test_fft_autocovariance_matches_direct_lag_sum() {
        let n = 300usize;
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 0xFF77_0300,
            partition: 9,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let raw: Vec<f64> = (0..n).map(|_| rng.next_normal()).collect();
        let mean = raw.iter().sum::<f64>() / (n as f64);
        let centered: Vec<f64> = raw.iter().map(|&x| x - mean).collect();

        // n = 300 > 256 exercises the BitwisePortable radix-2 FFT path.
        let fft_ac = chain_autocovariance(&centered);
        assert_eq!(fft_ac.len(), n);
        for (k, &got) in fft_ac.iter().enumerate() {
            let mut direct = 0.0_f64;
            for t in 0..(n - k) {
                direct += centered[t] * centered[t + k];
            }
            direct /= n as f64;
            assert!(
                (got - direct).abs() < 1e-12,
                "lag {k}: fft {got:.15} vs direct {direct:.15}"
            );
        }
    }
}
