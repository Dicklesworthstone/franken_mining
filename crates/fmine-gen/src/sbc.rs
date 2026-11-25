//! Determinism: BitwisePortable
//! Simulation-Based Calibration (SBC) rank-histogram and empirical-coverage harness
//! (`BD-144`, `GOAL-025`, `GATE-LODE-P1`, plan §33.3).
//!
//! Implements Talts et al. (2018) Simulation-Based Calibration:
//! 1. Draw true parameters `θ* ~ p(θ)` from the generative prior.
//! 2. Simulate synthetic data `y ~ p(y | θ*)`.
//! 3. Draw `L` posterior samples `θ_1, ..., θ_L` from the inference engine under test.
//! 4. Compute the rank statistic `r = #{θ_j < θ*} ∈ {0, ..., L}` (with deterministic keyed
//!    tie-splitting when `θ_j == θ*`), bin into `K` equal-width histogram buckets (`K | (L + 1)`),
//!    and test uniformity via Pearson's `χ²(K - 1)` tail and Kolmogorov–Smirnov ECDF deviation.
//! 5. Score multi-level empirical nominal coverage with exact two-sided Clopper–Pearson intervals.
//!
//! Every random draw uses `PhiloxStream` keyed by `(seed_root, partition, ..., realization)` and
//! every transcendental uses `fmine_core::det` so reports are `BitwisePortable` across ISAs.

use fmine_core::codec::Encoder;
use fmine_core::det::{erf, exp, ln, sqrt};
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_core::rand::{PhiloxStream, RngKey};

use crate::GenError;

/// Maximum permitted SBC replicates in a single run.
pub const MAX_SBC_REPLICATES: usize = 100_000;
/// Maximum permitted posterior draws per replicate.
pub const MAX_SBC_POSTERIOR_DRAWS: usize = 10_000;
/// Maximum supported histogram bin count (matches `chi_square_sf` df <= 200).
pub const MAX_SBC_BINS: usize = 200;
/// Partition tag `"SBC1"` (`0x5342_4331`) for SBC replicate RNG streams.
const SBC_PARTITION_TAG: u32 = 0x5342_4331;
/// Partition tag `"SBCT"` (`0x5342_4354`) for deterministic tie-breaking streams.
const SBC_TIE_PARTITION_TAG: u32 = 0x5342_4354;

/// Configuration for an SBC rank-histogram and coverage evaluation (`BD-144`).
#[derive(Debug, Clone, PartialEq)]
pub struct SbcConfig {
    /// Number of independent `(prior -> data -> posterior)` replicates `N`.
    pub replicates: usize,
    /// Number of posterior draws `L` per replicate. `(L + 1)` must be divisible by `bins`.
    pub posterior_draws: usize,
    /// Number of equal-width rank histogram bins `K` (`2 <= K <= MAX_SBC_BINS`).
    pub bins: usize,
    /// Significance threshold on the `χ²(K - 1)` uniformity p-value (default `0.01`, `GATE-LODE-P1`).
    pub chi2_alpha: f64,
    /// Strictly ascending nominal central coverage levels in `(0, 1)`, e.g. `[0.50, 0.80, 0.90, 0.95]`.
    pub coverage_levels: Vec<f64>,
    /// Two-sided significance level for exact Clopper–Pearson coverage intervals (default `0.05` -> 95% CI).
    pub coverage_ci_alpha: f64,
}

impl SbcConfig {
    /// Standard M1 calibration configuration: 500 replicates, 99 posterior draws, 10 bins,
    /// `χ²` threshold `0.01` (`GATE-LODE-P1`), coverage levels `[0.50, 0.80, 0.90, 0.95]` at
    /// 99% two-sided Clopper–Pearson CIs (`coverage_ci_alpha = 0.01`) so multi-level family-wise
    /// false-alarm rate stays below ~4% on calibrated engines (`KILL-PLUMB-6`).
    #[must_use]
    pub fn standard() -> Self {
        Self {
            replicates: 500,
            posterior_draws: 99,
            bins: 10,
            chi2_alpha: 0.01,
            coverage_levels: vec![0.50, 0.80, 0.90, 0.95],
            coverage_ci_alpha: 0.01,
        }
    }

    /// Validates all configuration bounds and divisibility invariants without clamping.
    pub fn validate(&self) -> Result<(), GenError> {
        if self.replicates == 0 || self.replicates > MAX_SBC_REPLICATES {
            return Err(GenError::InvalidConfig {
                field: "sbc.replicates",
                detail: format!(
                    "replicates {} outside 1..={MAX_SBC_REPLICATES}",
                    self.replicates
                ),
            });
        }
        if self.posterior_draws == 0 || self.posterior_draws > MAX_SBC_POSTERIOR_DRAWS {
            return Err(GenError::InvalidConfig {
                field: "sbc.posterior_draws",
                detail: format!(
                    "posterior_draws {} outside 1..={MAX_SBC_POSTERIOR_DRAWS}",
                    self.posterior_draws
                ),
            });
        }
        if self.bins < 2 || self.bins > MAX_SBC_BINS {
            return Err(GenError::InvalidConfig {
                field: "sbc.bins",
                detail: format!("bins {} outside 2..={MAX_SBC_BINS}", self.bins),
            });
        }
        let rank_states = self.posterior_draws + 1;
        if !rank_states.is_multiple_of(self.bins) {
            return Err(GenError::InvalidConfig {
                field: "sbc.bins",
                detail: format!(
                    "bins ({}) must evenly divide posterior_draws + 1 ({rank_states}) to avoid rank aliasing",
                    self.bins
                ),
            });
        }
        if !(self.chi2_alpha.is_finite() && self.chi2_alpha > 0.0 && self.chi2_alpha < 1.0) {
            return Err(GenError::InvalidConfig {
                field: "sbc.chi2_alpha",
                detail: format!("chi2_alpha {} must lie in (0, 1)", self.chi2_alpha),
            });
        }
        if !(self.coverage_ci_alpha.is_finite()
            && self.coverage_ci_alpha > 0.0
            && self.coverage_ci_alpha < 1.0)
        {
            return Err(GenError::InvalidConfig {
                field: "sbc.coverage_ci_alpha",
                detail: format!(
                    "coverage_ci_alpha {} must lie in (0, 1)",
                    self.coverage_ci_alpha
                ),
            });
        }
        if self.coverage_levels.is_empty() {
            return Err(GenError::InvalidConfig {
                field: "sbc.coverage_levels",
                detail: "at least one nominal coverage level is required".to_string(),
            });
        }
        let mut prev = 0.0_f64;
        for &lvl in &self.coverage_levels {
            if !(lvl.is_finite() && lvl > 0.0 && lvl < 1.0) {
                return Err(GenError::InvalidConfig {
                    field: "sbc.coverage_levels",
                    detail: format!("coverage level {lvl} must lie in (0, 1)"),
                });
            }
            if lvl <= prev {
                return Err(GenError::InvalidConfig {
                    field: "sbc.coverage_levels",
                    detail: format!(
                        "coverage levels must be strictly ascending; saw {lvl} after {prev}"
                    ),
                });
            }
            prev = lvl;
        }
        Ok(())
    }
}

/// One SBC replicate: the prior ground truth draw and the engine's `L` posterior samples.
#[derive(Debug, Clone, PartialEq)]
pub struct SbcReplicate {
    /// True parameter drawn from the prior `θ* ~ p(θ)`.
    pub prior_draw: f64,
    /// `L` posterior samples drawn from the engine under test given simulated data `y ~ p(y | θ*)`.
    pub posterior_draws: Vec<f64>,
}

/// Empirical coverage summary at one nominal central credible level `(1 - α)`.
#[derive(Debug, Clone, PartialEq)]
pub struct SbcCoverageRow {
    pub nominal: f64,
    pub covered: usize,
    pub total: usize,
    pub empirical: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
    pub nominal_in_ci: bool,
}

/// Diagnostic classification of an SBC rank histogram (Talts et al. 2018 §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SbcDiagnosis {
    /// Uniformity `p > chi2_alpha` and every nominal coverage level lies inside its Clopper–Pearson CI.
    Calibrated = 1,
    /// Excess probability mass in the extreme edge bins (U-shaped): posterior is overconfident / under-dispersed.
    UnderDispersedUShape = 2,
    /// Excess probability mass in the central bins (inverted-U): posterior is underconfident / over-dispersed.
    OverDispersedInvU = 3,
    /// Systematic directional shift in mean normalized rank away from `0.5`: posterior is location-biased.
    LocationBiased = 4,
    /// Uniformity or coverage failed without matching a single canonical failure shape.
    MiscalibratedOther = 5,
}

/// Complete SBC rank-histogram and empirical-coverage report (`BD-144`).
#[derive(Debug, Clone, PartialEq)]
pub struct SbcReport {
    pub replicates: usize,
    pub posterior_draws: usize,
    pub bins: usize,
    pub rank_histogram: Vec<usize>,
    pub tie_count: usize,
    pub expected_per_bin: f64,
    pub chi_square: f64,
    pub degrees_of_freedom: u32,
    pub chi2_p_value: f64,
    pub mean_normalized_rank: f64,
    pub ecdf_max_deviation: f64,
    pub coverage: Vec<SbcCoverageRow>,
    pub diagnosis: SbcDiagnosis,
    pub passed: bool,
}

impl SbcReport {
    /// Canonical `BitwisePortable` BLAKE3 digest of all report fields.
    #[must_use]
    pub fn canonical_digest(&self) -> Digest32 {
        let mut enc = Encoder::new_frame(*b"SBC1", 1);
        enc.put_u64(self.replicates as u64);
        enc.put_u64(self.posterior_draws as u64);
        enc.put_u64(self.bins as u64);
        enc.put_u64(self.rank_histogram.len() as u64);
        for &count in &self.rank_histogram {
            enc.put_u64(count as u64);
        }
        enc.put_u64(self.tie_count as u64);
        enc.put_u64(self.expected_per_bin.to_bits());
        enc.put_u64(self.chi_square.to_bits());
        enc.put_u32(self.degrees_of_freedom);
        enc.put_u64(self.chi2_p_value.to_bits());
        enc.put_u64(self.mean_normalized_rank.to_bits());
        enc.put_u64(self.ecdf_max_deviation.to_bits());
        enc.put_u64(self.coverage.len() as u64);
        for row in &self.coverage {
            enc.put_u64(row.nominal.to_bits());
            enc.put_u64(row.covered as u64);
            enc.put_u64(row.total as u64);
            enc.put_u64(row.empirical.to_bits());
            enc.put_u64(row.ci_lower.to_bits());
            enc.put_u64(row.ci_upper.to_bits());
            enc.put_u8(u8::from(row.nominal_in_ci));
        }
        enc.put_u8(self.diagnosis as u8);
        enc.put_u8(u8::from(self.passed));
        blake3_hash(&enc.finish())
    }
}

/// Exact `P(X >= t)` for `X ~ χ²(df)` (`1 <= df <= 200`, `t >= 0`), `BitwisePortable`.
///
/// Matches `scripts/oracle/chi_square_known_answers.py` (scipy 1.18.1) to `1e-12` relative + `1e-16` absolute.
pub fn chi_square_sf(t: f64, df: u32) -> Result<f64, GenError> {
    if !(1..=MAX_SBC_BINS as u32).contains(&df) {
        return Err(GenError::InvalidConfig {
            field: "chi_square_sf.df",
            detail: format!("df {df} outside 1..={MAX_SBC_BINS}"),
        });
    }
    if !(t.is_finite() && t >= 0.0) {
        return Err(GenError::InvalidPrediction {
            detail: format!("chi-square statistic must be finite and >= 0, got {t}"),
        });
    }
    if t == 0.0 {
        return Ok(1.0);
    }
    let x = 0.5 * t;
    let sf = if df.is_multiple_of(2) {
        let mut term = exp(-x);
        let mut sum = term;
        for i in 1..df / 2 {
            term *= x / f64::from(i);
            sum += term;
        }
        sum
    } else {
        let rx = sqrt(x);
        let mut term = exp(-x) * core::f64::consts::FRAC_2_SQRT_PI * rx;
        let mut sum = 1.0 - erf(rx);
        for i in 1..=(df - 1) / 2 {
            sum += term;
            term *= x / (f64::from(i) + 0.5);
        }
        sum
    };
    Ok(sf.clamp(0.0, 1.0))
}

/// Lanczos `ln(Γ(z))` for `z > 0` (`g = 7`, 9-term coefficients; `BitwisePortable`).
fn ln_gamma(z: f64) -> f64 {
    const COEFFS: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    let z_minus_1 = z - 1.0;
    let mut x = COEFFS[0];
    for (idx, &c) in COEFFS.iter().enumerate().skip(1) {
        x += c / (z_minus_1 + idx as f64);
    }
    let t = z_minus_1 + 7.5;
    0.5 * ln(2.0 * core::f64::consts::PI) + (z_minus_1 + 0.5) * ln(t) - t + ln(x)
}

/// Continued fraction core for regularized incomplete beta `I_x(a, b)` (modified Lentz algorithm).
fn betacf(x: f64, a: f64, b: f64) -> f64 {
    const MAX_ITER: usize = 250;
    const EPS: f64 = 3.0e-15;
    const FPMIN: f64 = 1.0e-300;

    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0_f64;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < FPMIN {
        d = FPMIN;
    }
    d = 1.0 / d;
    let mut h = d;
    for m_idx in 1..=MAX_ITER {
        let m = m_idx as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < EPS {
            break;
        }
    }
    h
}

/// Regularized incomplete beta function `I_x(a, b)` for `x ∈ [0, 1]`, `a > 0`, `b > 0`.
fn reg_incomplete_beta(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let ln_bt = ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * ln(x) + b * ln(1.0 - x);
    let bt = exp(ln_bt);
    if x < (a + 1.0) / (a + b + 2.0) {
        (bt * betacf(x, a, b) / a).clamp(0.0, 1.0)
    } else {
        (1.0 - bt * betacf(1.0 - x, b, a) / b).clamp(0.0, 1.0)
    }
}

/// Exact two-sided Clopper–Pearson `(1 - alpha)` confidence interval for `k` successes in `n` trials,
/// valid for arbitrary `n >= 1` via regularized incomplete beta bisection (`BitwisePortable`).
///
/// Uses the exact binomial-beta identities:
/// - `P(Bin(n, p) >= k) = I_p(k, n - k + 1)` (monotone increasing in `p`)
/// - `P(Bin(n, p) <= k) = 1 - I_p(k + 1, n - k)` (monotone decreasing in `p`)
pub fn clopper_pearson_interval(k: usize, n: usize, alpha: f64) -> Result<(f64, f64), GenError> {
    if n == 0 || k > n {
        return Err(GenError::InvalidConfig {
            field: "clopper_pearson",
            detail: format!("invalid binomial counts k={k}, n={n}"),
        });
    }
    if !(alpha.is_finite() && alpha > 0.0 && alpha < 1.0) {
        return Err(GenError::InvalidConfig {
            field: "clopper_pearson.alpha",
            detail: format!("alpha {alpha} must lie in (0, 1)"),
        });
    }
    let half = 0.5 * alpha;
    let lower = if k == 0 {
        0.0
    } else {
        let a = k as f64;
        let b = (n - k + 1) as f64;
        let mut lo = 0.0_f64;
        let mut hi = 1.0_f64;
        for _ in 0..100 {
            let mid = 0.5 * (lo + hi);
            if reg_incomplete_beta(mid, a, b) < half {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    };
    let upper = if k == n {
        1.0
    } else {
        let a = (k + 1) as f64;
        let b = (n - k) as f64;
        let mut lo = 0.0_f64;
        let mut hi = 1.0_f64;
        for _ in 0..100 {
            let mid = 0.5 * (lo + hi);
            if (1.0 - reg_incomplete_beta(mid, a, b)) > half {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    };
    Ok((lower, upper))
}

/// Evaluate Simulation-Based Calibration rank histogram and empirical coverage on pre-computed
/// `(prior_draw, posterior_draws)` replicates (`BD-144`).
pub fn evaluate_sbc_replicates(
    config: &SbcConfig,
    replicates: &[SbcReplicate],
    tie_seed: u64,
) -> Result<SbcReport, GenError> {
    config.validate()?;
    if replicates.len() != config.replicates {
        return Err(GenError::InvalidPrediction {
            detail: format!(
                "supplied {} replicates, config requires {}",
                replicates.len(),
                config.replicates
            ),
        });
    }

    let mut tie_rng = PhiloxStream::new(RngKey {
        seed_root: tie_seed,
        partition: SBC_TIE_PARTITION_TAG,
        domain: 0,
        var_group: 0,
        realization: 0,
        brick: 0,
    });

    let bin_width = (config.posterior_draws + 1) / config.bins;
    let mut histogram = vec![0usize; config.bins];
    let mut normalized_ranks = Vec::with_capacity(config.replicates);
    let mut covered_counts = vec![0usize; config.coverage_levels.len()];
    let mut tie_count = 0usize;
    let mut sorted = Vec::with_capacity(config.posterior_draws);

    for (idx, rep) in replicates.iter().enumerate() {
        if !rep.prior_draw.is_finite() {
            return Err(GenError::InvalidPrediction {
                detail: format!("non-finite prior_draw at replicate {idx}"),
            });
        }
        if rep.posterior_draws.len() != config.posterior_draws {
            return Err(GenError::InvalidPrediction {
                detail: format!(
                    "replicate {idx} has {} posterior draws, expected {}",
                    rep.posterior_draws.len(),
                    config.posterior_draws
                ),
            });
        }
        sorted.clear();
        let mut strictly_less = 0usize;
        let mut equal = 0usize;
        for &d in &rep.posterior_draws {
            if !d.is_finite() {
                return Err(GenError::InvalidPrediction {
                    detail: format!("non-finite posterior draw at replicate {idx}"),
                });
            }
            if d < rep.prior_draw {
                strictly_less += 1;
            } else if d == rep.prior_draw {
                equal += 1;
            }
            sorted.push(d);
        }
        let tie_offset = if equal == 0 {
            0
        } else {
            tie_count += equal;
            // Talts et al. (2018) §4.2: distribute ties uniformly across `{0, ..., equal}`.
            (tie_rng.next_u64() as usize) % (equal + 1)
        };
        let rank = strictly_less + tie_offset;
        let bin_idx = (rank / bin_width).min(config.bins - 1);
        histogram[bin_idx] += 1;
        normalized_ranks.push((rank as f64 + 0.5) / (config.posterior_draws as f64 + 1.0));

        sorted.sort_by(f64::total_cmp);
        for (lvl_idx, &nominal) in config.coverage_levels.iter().enumerate() {
            let tail = 0.5 * (1.0 - nominal);
            let lo = order_stat_quantile(&sorted, tail);
            let hi = order_stat_quantile(&sorted, 1.0 - tail);
            if rep.prior_draw >= lo && rep.prior_draw <= hi {
                covered_counts[lvl_idx] += 1;
            }
        }
    }

    let n_f = config.replicates as f64;
    let expected_per_bin = n_f / (config.bins as f64);
    let chi_square: f64 = histogram
        .iter()
        .map(|&obs| {
            let diff = (obs as f64) - expected_per_bin;
            (diff * diff) / expected_per_bin
        })
        .sum();
    let df = (config.bins - 1) as u32;
    let chi2_p_value = chi_square_sf(chi_square, df)?;

    let mean_normalized_rank = normalized_ranks.iter().sum::<f64>() / n_f;
    normalized_ranks.sort_by(f64::total_cmp);
    let mut ecdf_max_deviation = 0.0_f64;
    for (i, &u) in normalized_ranks.iter().enumerate() {
        let f_hi = (i + 1) as f64 / n_f;
        let f_lo = i as f64 / n_f;
        let dev = (f_hi - u).abs().max((u - f_lo).abs());
        if dev > ecdf_max_deviation {
            ecdf_max_deviation = dev;
        }
    }

    let mut coverage = Vec::with_capacity(config.coverage_levels.len());
    let mut all_coverage_ok = true;
    for (lvl_idx, &nominal) in config.coverage_levels.iter().enumerate() {
        let covered = covered_counts[lvl_idx];
        let (ci_lower, ci_upper) =
            clopper_pearson_interval(covered, config.replicates, config.coverage_ci_alpha)?;
        let nominal_in_ci = nominal >= ci_lower && nominal <= ci_upper;
        if !nominal_in_ci {
            all_coverage_ok = false;
        }
        coverage.push(SbcCoverageRow {
            nominal,
            covered,
            total: config.replicates,
            empirical: (covered as f64) / n_f,
            ci_lower,
            ci_upper,
            nominal_in_ci,
        });
    }

    let uniformity_ok = chi2_p_value > config.chi2_alpha;
    let passed = uniformity_ok && all_coverage_ok;
    let diagnosis = if passed {
        SbcDiagnosis::Calibrated
    } else {
        classify_miscalibration(&histogram, mean_normalized_rank, config.replicates)
    };

    Ok(SbcReport {
        replicates: config.replicates,
        posterior_draws: config.posterior_draws,
        bins: config.bins,
        rank_histogram: histogram,
        tie_count,
        expected_per_bin,
        chi_square,
        degrees_of_freedom: df,
        chi2_p_value,
        mean_normalized_rank,
        ecdf_max_deviation,
        coverage,
        diagnosis,
        passed,
    })
}

/// Continuous Hyndman–Fan type-6 quantile on a sorted sample (`E[F(X_(k))] = k / (L + 1)`),
/// avoiding discontinuous floor/ceil IEEE-754 jumps at exact nominal levels like `0.90` or `0.95`.
fn order_stat_quantile(sorted: &[f64], prob: f64) -> f64 {
    let n = sorted.len();
    let pos = (prob * ((n + 1) as f64) - 1.0).clamp(0.0, (n - 1) as f64);
    let lo_idx = pos.floor() as usize;
    let hi_idx = (lo_idx + 1).min(n - 1);
    let frac = pos - (lo_idx as f64);
    sorted[lo_idx] + frac * (sorted[hi_idx] - sorted[lo_idx])
}

fn classify_miscalibration(
    histogram: &[usize],
    mean_normalized_rank: f64,
    replicates: usize,
) -> SbcDiagnosis {
    // Standard error of mean of Uniform(0, 1) over N replicates is 1 / sqrt(12 * N).
    let mean_se = 1.0 / sqrt(12.0 * (replicates as f64));
    let mean_z = (mean_normalized_rank - 0.5).abs() / mean_se;
    let k = histogram.len();
    let edge_bins = (k / 5).max(1);
    let left_edge_mean = histogram[..edge_bins].iter().sum::<usize>() as f64 / (edge_bins as f64);
    let right_edge_mean =
        histogram[k - edge_bins..].iter().sum::<usize>() as f64 / (edge_bins as f64);
    let inner_count = k.saturating_sub(2 * edge_bins);
    let inner_mean = if inner_count == 0 {
        0.5 * (left_edge_mean + right_edge_mean)
    } else {
        histogram[edge_bins..k - edge_bins].iter().sum::<usize>() as f64 / (inner_count as f64)
    };
    let left_ratio = if inner_mean > 0.0 {
        left_edge_mean / inner_mean
    } else {
        f64::INFINITY
    };
    let right_ratio = if inner_mean > 0.0 {
        right_edge_mean / inner_mean
    } else {
        f64::INFINITY
    };

    // True U-shape elevates BOTH tails; true inverted-U depresses BOTH tails;
    // location bias shifts mean rank asymmetrically (one tail up, the other down).
    if left_ratio >= 1.25 && right_ratio >= 1.25 {
        SbcDiagnosis::UnderDispersedUShape
    } else if left_ratio <= 0.75 && right_ratio <= 0.75 {
        SbcDiagnosis::OverDispersedInvU
    } else if mean_z >= 2.5 {
        SbcDiagnosis::LocationBiased
    } else {
        SbcDiagnosis::MiscalibratedOther
    }
}

/// Run an SBC harness by invoking `replicate_fn(replicate_idx, &mut rng)` with an isolated
/// keyed `PhiloxStream` per replicate (`BD-144`).
pub fn run_sbc<F>(
    config: &SbcConfig,
    seed_root: u64,
    mut replicate_fn: F,
) -> Result<SbcReport, GenError>
where
    F: FnMut(usize, &mut PhiloxStream) -> Result<SbcReplicate, GenError>,
{
    config.validate()?;
    let mut replicates = Vec::with_capacity(config.replicates);
    for idx in 0..config.replicates {
        let mut rng = PhiloxStream::new(RngKey {
            seed_root,
            partition: SBC_PARTITION_TAG,
            domain: 0,
            var_group: 0,
            realization: idx as u32,
            brick: 0,
        });
        replicates.push(replicate_fn(idx, &mut rng)?);
    }
    evaluate_sbc_replicates(config, &replicates, seed_root)
}

// --- BitwisePortable conjugate samplers for known-calibrated & planted-miscalibrated controls ---

/// Deterministic `Gamma(shape, rate)` draw via Marsaglia–Tsang (2000) (`BitwisePortable`).
pub fn sample_gamma(rng: &mut PhiloxStream, shape: f64, rate: f64) -> Result<f64, GenError> {
    if !(shape.is_finite() && shape > 0.0 && rate.is_finite() && rate > 0.0) {
        return Err(GenError::InvalidConfig {
            field: "sample_gamma",
            detail: format!("shape ({shape}) and rate ({rate}) must be positive and finite"),
        });
    }
    if shape < 1.0 {
        // Exact boost: if G ~ Gamma(shape + 1, 1) and U ~ Uniform(0, 1), then G * U^(1/shape) ~ Gamma(shape, 1).
        let g = sample_gamma_ge_one(rng, shape + 1.0);
        let u = rng.next_f64_open01();
        return Ok(g * exp(ln(u) / shape) / rate);
    }
    Ok(sample_gamma_ge_one(rng, shape) / rate)
}

fn sample_gamma_ge_one(rng: &mut PhiloxStream, shape: f64) -> f64 {
    let d = shape - 1.0 / 3.0;
    let c = 1.0 / sqrt(9.0 * d);
    loop {
        let x = rng.next_normal();
        let v_root = 1.0 + c * x;
        if v_root <= 0.0 {
            continue;
        }
        let v = v_root * v_root * v_root;
        let u = rng.next_f64_open01();
        let x2 = x * x;
        if u < 1.0 - 0.0331 * (x2 * x2) {
            return d * v;
        }
        if ln(u) < 0.5 * x2 + d * (1.0 - v + ln(v)) {
            return d * v;
        }
    }
}

/// Deterministic `Beta(alpha, beta)` draw via two independent Gamma draws (`BitwisePortable`).
pub fn sample_beta(rng: &mut PhiloxStream, alpha: f64, beta: f64) -> Result<f64, GenError> {
    let g1 = sample_gamma(rng, alpha, 1.0)?;
    let g2 = sample_gamma(rng, beta, 1.0)?;
    Ok(g1 / (g1 + g2))
}

/// Exact deterministic `Poisson(lambda)` draw (`BitwisePortable`).
///
/// Splits large `lambda` into exact additive chunks `<= 20.0` so `exp(-chunk)` never underflows.
pub fn sample_poisson(rng: &mut PhiloxStream, lambda: f64) -> Result<u64, GenError> {
    if !(lambda.is_finite() && lambda >= 0.0) {
        return Err(GenError::InvalidConfig {
            field: "sample_poisson.lambda",
            detail: format!("lambda {lambda} must be finite and >= 0"),
        });
    }
    if lambda == 0.0 {
        return Ok(0);
    }
    let chunks = ((lambda / 20.0).ceil() as u64).max(1);
    let step = lambda / (chunks as f64);
    let threshold = exp(-step);
    let mut total = 0u64;
    for _ in 0..chunks {
        let mut p = 1.0_f64;
        let mut k = 0u64;
        loop {
            p *= rng.next_f64_open01();
            if p <= threshold {
                break;
            }
            k += 1;
        }
        total += k;
    }
    Ok(total)
}

/// Exact deterministic `Binomial(trials, p)` draw (`BitwisePortable`).
pub fn sample_binomial(rng: &mut PhiloxStream, trials: u32, p: f64) -> Result<u32, GenError> {
    if !(p.is_finite() && (0.0..=1.0).contains(&p)) {
        return Err(GenError::InvalidConfig {
            field: "sample_binomial.p",
            detail: format!("probability {p} must lie in [0, 1]"),
        });
    }
    let mut successes = 0u32;
    for _ in 0..trials {
        if rng.next_f64_open01() < p {
            successes += 1;
        }
    }
    Ok(successes)
}

/// Normal–Normal conjugate SBC control model (`BD-144`).
///
/// When `posterior_mean_bias == 0.0` and `posterior_std_scale == 1.0`, the sampler draws from the
/// exact conjugate posterior and MUST pass SBC. Non-zero bias or non-unit scale creates planted
/// miscalibrated negative controls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalNormalControl {
    pub prior_mean: f64,
    pub prior_std: f64,
    pub obs_std: f64,
    pub n_obs: usize,
    /// Additive bias injected into the posterior mean (`0.0` for exact control).
    pub posterior_mean_bias: f64,
    /// Multiplicative scale injected into the posterior standard deviation (`1.0` for exact control).
    pub posterior_std_scale: f64,
}

impl NormalNormalControl {
    /// Exact conjugate Normal–Normal positive control.
    #[must_use]
    pub const fn exact(prior_mean: f64, prior_std: f64, obs_std: f64, n_obs: usize) -> Self {
        Self {
            prior_mean,
            prior_std,
            obs_std,
            n_obs,
            posterior_mean_bias: 0.0,
            posterior_std_scale: 1.0,
        }
    }

    /// Execute SBC on this Normal–Normal control family.
    pub fn run(&self, config: &SbcConfig, seed_root: u64) -> Result<SbcReport, GenError> {
        if !(self.prior_mean.is_finite()
            && self.prior_std.is_finite()
            && self.prior_std > 0.0
            && self.obs_std.is_finite()
            && self.obs_std > 0.0
            && self.posterior_mean_bias.is_finite()
            && self.posterior_std_scale.is_finite()
            && self.posterior_std_scale > 0.0
            && self.n_obs > 0)
        {
            return Err(GenError::InvalidConfig {
                field: "NormalNormalControl",
                detail:
                    "all parameters must be finite with positive stds, positive scale and n_obs > 0"
                        .to_string(),
            });
        }
        let prior_prec = 1.0 / (self.prior_std * self.prior_std);
        let obs_prec = 1.0 / (self.obs_std * self.obs_std);
        let n_f = self.n_obs as f64;
        let post_prec = prior_prec + n_f * obs_prec;
        let post_std = sqrt(1.0 / post_prec) * self.posterior_std_scale;
        let draws_n = config.posterior_draws;

        run_sbc(config, seed_root, |_idx, rng| {
            let theta_true = self.prior_mean + self.prior_std * rng.next_normal();
            let mut sum_y = 0.0_f64;
            for _ in 0..self.n_obs {
                sum_y += theta_true + self.obs_std * rng.next_normal();
            }
            let post_mean = (prior_prec * self.prior_mean + obs_prec * sum_y) / post_prec
                + self.posterior_mean_bias;
            let mut draws = Vec::with_capacity(draws_n);
            for _ in 0..draws_n {
                draws.push(post_mean + post_std * rng.next_normal());
            }
            Ok(SbcReplicate {
                prior_draw: theta_true,
                posterior_draws: draws,
            })
        })
    }
}

/// Gamma–Poisson conjugate SBC control model (`BD-144`, rate parameter `λ > 0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GammaPoissonControl {
    pub prior_shape: f64,
    pub prior_rate: f64,
    pub n_obs: usize,
    /// Multiplicative distortion on posterior rate (`1.0` for exact conjugate control).
    pub posterior_rate_scale: f64,
}

impl GammaPoissonControl {
    /// Exact conjugate Gamma–Poisson positive control.
    #[must_use]
    pub const fn exact(prior_shape: f64, prior_rate: f64, n_obs: usize) -> Self {
        Self {
            prior_shape,
            prior_rate,
            n_obs,
            posterior_rate_scale: 1.0,
        }
    }

    /// Execute SBC on this Gamma–Poisson control family.
    pub fn run(&self, config: &SbcConfig, seed_root: u64) -> Result<SbcReport, GenError> {
        if !(self.prior_shape.is_finite()
            && self.prior_shape > 0.0
            && self.prior_rate.is_finite()
            && self.prior_rate > 0.0
            && self.posterior_rate_scale.is_finite()
            && self.posterior_rate_scale > 0.0
            && self.n_obs > 0)
        {
            return Err(GenError::InvalidConfig {
                field: "GammaPoissonControl",
                detail: "shape, rate, rate_scale must be positive/finite and n_obs > 0".to_string(),
            });
        }
        let draws_n = config.posterior_draws;
        run_sbc(config, seed_root, |_idx, rng| {
            let lambda_true = sample_gamma(rng, self.prior_shape, self.prior_rate)?;
            let mut sum_k = 0u64;
            for _ in 0..self.n_obs {
                sum_k += sample_poisson(rng, lambda_true)?;
            }
            let post_shape = self.prior_shape + (sum_k as f64);
            let post_rate = (self.prior_rate + (self.n_obs as f64)) * self.posterior_rate_scale;
            let mut draws = Vec::with_capacity(draws_n);
            for _ in 0..draws_n {
                draws.push(sample_gamma(rng, post_shape, post_rate)?);
            }
            Ok(SbcReplicate {
                prior_draw: lambda_true,
                posterior_draws: draws,
            })
        })
    }
}

/// Beta–Binomial conjugate SBC control model (`BD-144`, probability parameter `p ∈ (0, 1)`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BetaBinomialControl {
    pub prior_alpha: f64,
    pub prior_beta: f64,
    pub trials: u32,
    /// Concentration multiplier on posterior `(α_post, β_post)` (`1.0` for exact control).
    pub posterior_concentration_scale: f64,
}

impl BetaBinomialControl {
    /// Exact conjugate Beta–Binomial positive control.
    #[must_use]
    pub const fn exact(prior_alpha: f64, prior_beta: f64, trials: u32) -> Self {
        Self {
            prior_alpha,
            prior_beta,
            trials,
            posterior_concentration_scale: 1.0,
        }
    }

    /// Execute SBC on this Beta–Binomial control family.
    pub fn run(&self, config: &SbcConfig, seed_root: u64) -> Result<SbcReport, GenError> {
        if !(self.prior_alpha.is_finite()
            && self.prior_alpha > 0.0
            && self.prior_beta.is_finite()
            && self.prior_beta > 0.0
            && self.posterior_concentration_scale.is_finite()
            && self.posterior_concentration_scale > 0.0
            && self.trials > 0)
        {
            return Err(GenError::InvalidConfig {
                field: "BetaBinomialControl",
                detail: "alpha, beta, concentration_scale must be positive/finite and trials > 0"
                    .to_string(),
            });
        }
        let draws_n = config.posterior_draws;
        run_sbc(config, seed_root, |_idx, rng| {
            let p_true = sample_beta(rng, self.prior_alpha, self.prior_beta)?;
            let k = sample_binomial(rng, self.trials, p_true)?;
            let post_a = (self.prior_alpha + f64::from(k)) * self.posterior_concentration_scale;
            let post_b =
                (self.prior_beta + f64::from(self.trials - k)) * self.posterior_concentration_scale;
            let mut draws = Vec::with_capacity(draws_n);
            for _ in 0..draws_n {
                draws.push(sample_beta(rng, post_a, post_b)?);
            }
            Ok(SbcReplicate {
                prior_draw: p_true,
                posterior_draws: draws,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::clopper_pearson;

    #[test]
    fn test_chi_square_sf_matches_scipy_known_answers() {
        // scipy.stats.chi2.sf (scipy 1.18.1), verified independently by scripts/oracle/chi_square_known_answers.py
        let cases: [(f64, u32, f64); 12] = [
            (3.841_458_820_694_124, 1, 0.049_999_999_999_999_94),
            (0.5, 1, 0.479_500_122_186_953_37),
            (5.991_464_547_107_979, 2, 0.050_000_000_000_000_08),
            (7.814_727_903_251_178, 3, 0.050_000_000_000_000_02),
            (0.5, 3, 0.918_891_411_654_675_8),
            (2.0, 4, 0.735_758_882_342_884_7),
            (40.0, 7, 1.258_790_387_371_308e-6),
            (23.209_251_158_954_356, 10, 0.010_000_000_000_000_004),
            (3.073_8, 12, 0.995_000_181_621_716),
            (12.0, 12, 0.445_679_641_364_611_2),
            (28.299_5, 12, 0.005_000_031_844_890_244),
            (60.0, 25, 0.000_104_554_861_315_264_47),
        ];
        for (t, df, want) in cases {
            let got = chi_square_sf(t, df).expect("valid chi2");
            assert!(
                (got - want).abs() <= 1e-12 * want + 1e-16,
                "chi2_sf({t}, {df}) = {got}, want {want}"
            );
        }
        assert_eq!(chi_square_sf(0.0, 12).expect("zero"), 1.0);
        assert!(chi_square_sf(-0.1, 4).is_err());
        assert!(chi_square_sf(1.0, 0).is_err());
    }

    #[test]
    fn test_clopper_pearson_matches_exact_review_module_and_large_n() {
        for (k, n) in [(0, 10), (1, 10), (5, 10), (10, 10), (8, 20), (40, 50)] {
            let (lo_gen, hi_gen) = clopper_pearson_interval(k, n, 0.05).expect("general cp");
            let (lo_ref, hi_ref) = clopper_pearson(k as u32, n as u32, 0.05).expect("review cp");
            assert!(
                (lo_gen - lo_ref).abs() < 1e-11 && (hi_gen - hi_ref).abs() < 1e-11,
                "k={k}, n={n}: got ({lo_gen}, {hi_gen}) vs ref ({lo_ref}, {hi_ref})"
            );
        }
        // Large n = 500, k = 400 (0.80 empirical): interval strictly encloses 0.80 and is tight.
        let (lo, hi) = clopper_pearson_interval(400, 500, 0.05).expect("n=500 cp");
        assert!(lo > 0.76 && lo < 0.77, "unexpected lo={lo}");
        assert!(hi > 0.83 && hi < 0.84, "unexpected hi={hi}");
    }

    #[test]
    fn test_sbc_normal_normal_known_calibrated_control_passes() {
        let cfg = SbcConfig::standard();
        let ctrl = NormalNormalControl::exact(2.0, 0.8, 0.5, 6);
        let report = ctrl.run(&cfg, 0xBD_0144_0001).expect("normal-normal sbc");
        assert!(
            report.passed,
            "known-calibrated Normal-Normal control must pass: p={}, diag={:?}, hist={:?}, cov={:?}",
            report.chi2_p_value, report.diagnosis, report.rank_histogram, report.coverage
        );
        assert_eq!(report.diagnosis, SbcDiagnosis::Calibrated);
        assert!(report.chi2_p_value > cfg.chi2_alpha);
        assert_eq!(report.rank_histogram.iter().sum::<usize>(), cfg.replicates);
        assert!(report.coverage.iter().all(|c| c.nominal_in_ci));
    }

    #[test]
    fn test_sbc_gamma_poisson_known_calibrated_control_passes() {
        let cfg = SbcConfig::standard();
        let ctrl = GammaPoissonControl::exact(3.5, 1.2, 5);
        let report = ctrl.run(&cfg, 0xBD_0144_0002).expect("gamma-poisson sbc");
        assert!(
            report.passed,
            "known-calibrated Gamma-Poisson control must pass: p={}, diag={:?}, hist={:?}, cov={:?}",
            report.chi2_p_value, report.diagnosis, report.rank_histogram, report.coverage
        );
        assert_eq!(report.diagnosis, SbcDiagnosis::Calibrated);
        assert!(report.coverage.iter().all(|c| c.nominal_in_ci));
    }

    #[test]
    fn test_sbc_beta_binomial_known_calibrated_control_passes() {
        let cfg = SbcConfig::standard();
        let ctrl = BetaBinomialControl::exact(2.0, 3.0, 12);
        let report = ctrl.run(&cfg, 0xBD_0144_0003).expect("beta-binomial sbc");
        assert!(
            report.passed,
            "known-calibrated Beta-Binomial control must pass: p={}, diag={:?}, hist={:?}, cov={:?}",
            report.chi2_p_value, report.diagnosis, report.rank_histogram, report.coverage
        );
        assert_eq!(report.diagnosis, SbcDiagnosis::Calibrated);
        assert!(report.coverage.iter().all(|c| c.nominal_in_ci));
    }

    #[test]
    fn test_sbc_detects_planted_miscalibrated_controls() {
        let cfg = SbcConfig::standard();

        // 1. Overconfident / under-dispersed posterior (0.5x posterior SD -> U-shaped rank histogram).
        let under = NormalNormalControl {
            posterior_std_scale: 0.5,
            ..NormalNormalControl::exact(2.0, 0.8, 0.5, 6)
        };
        let r_under = under.run(&cfg, 0xBD_0144_0010).expect("under-dispersed");
        assert!(!r_under.passed, "overconfident posterior must fail SBC");
        assert_eq!(r_under.diagnosis, SbcDiagnosis::UnderDispersedUShape);
        assert!(r_under.chi2_p_value < 1e-6, "p={}", r_under.chi2_p_value);
        let cov80 = &r_under.coverage[1];
        assert!(
            cov80.empirical < 0.65 && !cov80.nominal_in_ci,
            "80% coverage must collapse under 0.5x SD: {cov80:?}"
        );

        // 2. Underconfident / over-dispersed posterior (2.0x posterior SD -> inverted-U histogram).
        let over = NormalNormalControl {
            posterior_std_scale: 2.0,
            ..NormalNormalControl::exact(2.0, 0.8, 0.5, 6)
        };
        let r_over = over.run(&cfg, 0xBD_0144_0011).expect("over-dispersed");
        assert!(!r_over.passed, "underconfident posterior must fail SBC");
        assert_eq!(r_over.diagnosis, SbcDiagnosis::OverDispersedInvU);
        assert!(r_over.chi2_p_value < 1e-6, "p={}", r_over.chi2_p_value);

        // 3. Location-biased posterior (+0.25 mean bias -> skewed histogram).
        let biased = NormalNormalControl {
            posterior_mean_bias: 0.25,
            ..NormalNormalControl::exact(2.0, 0.8, 0.5, 6)
        };
        let r_bias = biased.run(&cfg, 0xBD_0144_0012).expect("biased");
        assert!(!r_bias.passed, "location-biased posterior must fail SBC");
        assert_eq!(r_bias.diagnosis, SbcDiagnosis::LocationBiased);
        assert!(r_bias.chi2_p_value < 1e-6, "p={}", r_bias.chi2_p_value);

        // 4. Distorted Gamma-Poisson posterior rate (1.5x -> biased low).
        let gp_bad = GammaPoissonControl {
            posterior_rate_scale: 1.5,
            ..GammaPoissonControl::exact(3.5, 1.2, 5)
        };
        let r_gp_bad = gp_bad.run(&cfg, 0xBD_0144_0013).expect("gp bad");
        assert!(!r_gp_bad.passed, "distorted Gamma-Poisson must fail SBC");
        assert!(r_gp_bad.chi2_p_value < 1e-6, "p={}", r_gp_bad.chi2_p_value);

        // 5. Over-concentrated Beta-Binomial posterior (2.5x concentration -> under-dispersed).
        let bb_bad = BetaBinomialControl {
            posterior_concentration_scale: 2.5,
            ..BetaBinomialControl::exact(2.0, 3.0, 12)
        };
        let r_bb_bad = bb_bad.run(&cfg, 0xBD_0144_0014).expect("bb bad");
        assert!(
            !r_bb_bad.passed,
            "over-concentrated Beta-Binomial must fail SBC"
        );
        assert_eq!(r_bb_bad.diagnosis, SbcDiagnosis::UnderDispersedUShape);
        assert!(r_bb_bad.chi2_p_value < 1e-6, "p={}", r_bb_bad.chi2_p_value);
    }

    #[test]
    fn test_sbc_refuses_invalid_configs_and_non_finite_replicates() {
        let mut bad_bins = SbcConfig::standard();
        bad_bins.bins = 7; // 7 does not divide 100
        assert!(bad_bins.validate().is_err());

        let mut bad_levels = SbcConfig::standard();
        bad_levels.coverage_levels = vec![0.80, 0.50]; // not ascending
        assert!(bad_levels.validate().is_err());

        let mut bad_alpha = SbcConfig::standard();
        bad_alpha.chi2_alpha = 1.0;
        assert!(bad_alpha.validate().is_err());

        let tiny = SbcConfig {
            replicates: 2,
            posterior_draws: 9,
            bins: 5,
            chi2_alpha: 0.01,
            coverage_levels: vec![0.80],
            coverage_ci_alpha: 0.05,
        };
        let nan_rep = vec![
            SbcReplicate {
                prior_draw: f64::NAN,
                posterior_draws: vec![0.0; 9],
            },
            SbcReplicate {
                prior_draw: 0.0,
                posterior_draws: vec![0.0; 9],
            },
        ];
        assert!(evaluate_sbc_replicates(&tiny, &nan_rep, 1).is_err());
    }

    #[test]
    fn test_sbc_is_bitwise_deterministic_across_runs() {
        let cfg = SbcConfig {
            replicates: 100,
            posterior_draws: 39,
            bins: 8,
            chi2_alpha: 0.01,
            coverage_levels: vec![0.50, 0.80, 0.95],
            coverage_ci_alpha: 0.05,
        };
        let ctrl = NormalNormalControl::exact(1.0, 0.5, 0.4, 4);
        let r1 = ctrl.run(&cfg, 9999).expect("run 1");
        let r2 = ctrl.run(&cfg, 9999).expect("run 2");
        let r3 = ctrl.run(&cfg, 10000).expect("run 3");
        assert_eq!(r1, r2);
        assert_eq!(r1.canonical_digest(), r2.canonical_digest());
        assert_ne!(r1.canonical_digest(), r3.canonical_digest());
    }
}
