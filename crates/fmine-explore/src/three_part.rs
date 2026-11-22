//! Determinism: BitwisePortable
//! USGS three-part mineral-resource assessment N0 degenerate model (`MapMark4` parity baseline,
//! `BD-235`, plan §11.2, `PO-LODE-5`, `GATE-LODE-P2`).
//!
//! Combines a discrete subjective undiscovered-deposit count PMF (`N_r`, from Mark3B / IDEA
//! quantiles `N90 <= N50 <= N10` or explicit negative-binomial weights) with independent draws
//! from an empirical grade-tonnage model using deterministic `Philox4x32-10` streams, plus exact
//! two-sample Kolmogorov–Smirnov distance checking (`KS < 0.02` target for `GATE-LODE-P2`).
//!
//! Zero bare `f64` parameters in public signatures (L-6): all grades, tonnages, probabilities,
//! quantiles, and KS statistics use exact `fmine_units::ExactQty`.

use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_units::ExactQty;

use crate::ExploreError;

/// Discrete probability mass function over undiscovered deposit counts `N ∈ {0, 1, ..., max_n}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndiscoveredCountPmf {
    /// Probability `P(N = k)` for `k = 0..probs.len()`. Sums exactly to `1`.
    pub probs: Vec<ExactQty>,
}

impl UndiscoveredCountPmf {
    /// Construct and validate a discrete count PMF (`probs[k] >= 0`, `Σ probs == 1`).
    pub fn new(probs: Vec<ExactQty>) -> Result<Self, ExploreError> {
        if probs.is_empty() {
            return Err(ExploreError::InvalidSimplex {
                detail: "UndiscoveredCountPmf must contain at least one count state".to_string(),
            });
        }
        let zero = ExactQty::from_integer(0);
        let one = ExactQty::from_integer(1);
        let mut sum = zero;
        for &p in &probs {
            if p.checked_cmp(zero)?.is_lt() {
                return Err(ExploreError::InvalidSimplex {
                    detail: "count PMF probability cannot be negative".to_string(),
                });
            }
            sum = sum.checked_add(p)?;
        }
        if sum.checked_cmp(one)?.is_ne() {
            return Err(ExploreError::InvalidSimplex {
                detail: format!(
                    "count PMF probabilities must sum to 1, got {}/{}",
                    sum.num(),
                    sum.den()
                ),
            });
        }
        Ok(Self { probs })
    }

    /// Construct the USGS Mark3B piecewise-uniform discrete PMF from elicited quantiles
    /// `(n90, n50, n10)` with `0 <= n90 <= n50 <= n10` (10% mass on `[0, n90]`, 40% on `[n90, n50]`,
    /// 40% on `[n50, n10]`, 10% on `n10` upper tail anchor). Exact sum = `1`.
    pub fn from_usgs_mark3b_quantiles(n90: u32, n50: u32, n10: u32) -> Result<Self, ExploreError> {
        if n90 > n50 || n50 > n10 {
            return Err(ExploreError::InvalidRecord {
                field: "mark3b_quantiles",
                detail: format!(
                    "quantiles must satisfy n90 <= n50 <= n10, got ({n90}, {n50}, {n10})"
                ),
            });
        }
        let max_n = usize::try_from(n10).map_err(|_| ExploreError::Overflow)?;
        let mut probs = vec![ExactQty::from_integer(0); max_n + 1];

        let mut add_uniform =
            |lo: u32, hi: u32, mass_num: i128, mass_den: i128| -> Result<(), ExploreError> {
                let count = i128::from(hi - lo) + 1;
                let per_bin = ExactQty::new(mass_num, mass_den * count)?;
                for k in lo..=hi {
                    let idx = usize::try_from(k).map_err(|_| ExploreError::Overflow)?;
                    probs[idx] = probs[idx].checked_add(per_bin)?;
                }
                Ok(())
            };

        add_uniform(0, n90, 1, 10)?;
        add_uniform(n90, n50, 4, 10)?;
        add_uniform(n50, n10, 4, 10)?;
        let top_idx = usize::try_from(n10).map_err(|_| ExploreError::Overflow)?;
        probs[top_idx] = probs[top_idx].checked_add(ExactQty::new(1, 10)?)?;

        Self::new(probs)
    }

    /// Exact expected deposit count `E[N] = Σ k · P(N = k)`.
    pub fn exact_mean(&self) -> Result<ExactQty, ExploreError> {
        let mut acc = ExactQty::from_integer(0);
        for (k, &p) in self.probs.iter().enumerate() {
            let k_qty =
                ExactQty::from_integer(i128::try_from(k).map_err(|_| ExploreError::Overflow)?);
            acc = acc.checked_add(k_qty.checked_mul(p)?)?;
        }
        Ok(acc)
    }

    /// Probability of zero undiscovered deposits `P(N = 0)`.
    #[must_use]
    pub fn p_zero(&self) -> ExactQty {
        self.probs[0]
    }
}

/// One historical deposit entry in a USGS empirical grade-tonnage model (`BD-235`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradeTonnageDeposit {
    pub deposit_id: String,
    /// Ore tonnage in kilotonnes (strictly positive).
    pub tonnage_kt: ExactQty,
    /// Mean metal grade as mass fraction in `(0, 1]` (e.g. `5/1000` for `0.5% Cu`).
    pub grade_fraction: ExactQty,
}

impl GradeTonnageDeposit {
    /// Validate deposit bounds and return exact contained metal in kilotonnes (`tonnage_kt * grade_fraction`).
    pub fn contained_metal_kt(&self) -> Result<ExactQty, ExploreError> {
        if self.deposit_id.trim().is_empty() {
            return Err(ExploreError::MissingProvenance {
                field: "deposit_id",
                detail: "grade-tonnage deposit_id must be non-empty".to_string(),
            });
        }
        let zero = ExactQty::from_integer(0);
        let one = ExactQty::from_integer(1);
        if self.tonnage_kt.signum() <= 0 {
            return Err(ExploreError::InvalidRecord {
                field: "tonnage_kt",
                detail: "deposit tonnage_kt must be strictly positive".to_string(),
            });
        }
        if self.grade_fraction.checked_cmp(zero)?.is_le()
            || self.grade_fraction.checked_cmp(one)?.is_gt()
        {
            return Err(ExploreError::InvalidRecord {
                field: "grade_fraction",
                detail: "deposit grade_fraction must lie in (0, 1]".to_string(),
            });
        }
        Ok(self.tonnage_kt.checked_mul(self.grade_fraction)?)
    }
}

/// Summary of a deterministic USGS three-part Monte Carlo endowment simulation (`BD-235`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreePartEndowmentSummary {
    pub tract_id: String,
    pub n_draws: u32,
    /// Sorted simulated total contained metal draws (kt) in ascending order.
    pub sorted_metal_draws_kt: Vec<ExactQty>,
    pub p10_metal_kt: ExactQty,
    pub p50_metal_kt: ExactQty,
    pub p90_metal_kt: ExactQty,
    pub empirical_mean_metal_kt: ExactQty,
    /// Exact analytical mean `E[N] · E[M_deposit]` (Wald's identity check!).
    pub analytical_mean_metal_kt: ExactQty,
    pub empirical_zero_fraction: ExactQty,
}

/// Run the N0 MapMark4-degenerate three-part endowment simulation using `Philox4x32-10`
/// (`BD-235`, plan §11.2, `PO-LODE-5`).
pub fn simulate_three_part_endowment(
    tract_id: &str,
    count_pmf: &UndiscoveredCountPmf,
    grade_tonnage_model: &[GradeTonnageDeposit],
    n_draws: u32,
    rng_key: RngKey,
) -> Result<ThreePartEndowmentSummary, ExploreError> {
    if tract_id.trim().is_empty() {
        return Err(ExploreError::MissingProvenance {
            field: "tract_id",
            detail: "tract_id must be non-empty".to_string(),
        });
    }
    if n_draws == 0 {
        return Err(ExploreError::InvalidRecord {
            field: "n_draws",
            detail: "n_draws must be >= 1".to_string(),
        });
    }
    if grade_tonnage_model.is_empty() {
        return Err(ExploreError::InvalidRecord {
            field: "grade_tonnage_model",
            detail: "grade-tonnage model must contain at least one deposit".to_string(),
        });
    }

    // Precompute exact per-deposit contained metal and analytical mean E[M_deposit].
    let mut deposit_metals = Vec::with_capacity(grade_tonnage_model.len());
    let mut sum_dep_metal = ExactQty::from_integer(0);
    for dep in grade_tonnage_model {
        let m = dep.contained_metal_kt()?;
        sum_dep_metal = sum_dep_metal.checked_add(m)?;
        deposit_metals.push(m);
    }
    let n_dep_i128 = i128::try_from(deposit_metals.len()).map_err(|_| ExploreError::Overflow)?;
    let mean_dep_metal = sum_dep_metal.checked_div(ExactQty::from_integer(n_dep_i128))?;
    let analytical_mean_metal_kt = count_pmf.exact_mean()?.checked_mul(mean_dep_metal)?;

    // Build exact CDF thresholds scaled to 2^53 for uniform Philox draws without float drift.
    const SCALE_U64: u64 = 1_u64 << 53;
    let scale_qty = ExactQty::from_integer(i128::from(SCALE_U64));
    let mut cdf_thresholds: Vec<u64> = Vec::with_capacity(count_pmf.probs.len());
    let mut cum = ExactQty::from_integer(0);
    for (idx, &p) in count_pmf.probs.iter().enumerate() {
        cum = cum.checked_add(p)?;
        if idx + 1 == count_pmf.probs.len() {
            cdf_thresholds.push(SCALE_U64);
        } else {
            let scaled = cum.checked_mul(scale_qty)?;
            let thresh =
                u64::try_from(scaled.num() / scaled.den()).map_err(|_| ExploreError::Overflow)?;
            cdf_thresholds.push(thresh);
        }
    }

    let mut stream = PhiloxStream::new(rng_key);
    let n_draws_usize = usize::try_from(n_draws).map_err(|_| ExploreError::Overflow)?;
    let mut draws = Vec::with_capacity(n_draws_usize);
    let mut total_sum = ExactQty::from_integer(0);
    let mut zero_count = 0_u64;
    let n_dep_u64 = u64::try_from(deposit_metals.len()).map_err(|_| ExploreError::Overflow)?;

    for _ in 0..n_draws {
        // Uniform integer in [0, 2^53).
        let u_count = stream.next_u64() >> 11;
        let k = cdf_thresholds
            .partition_point(|&t| u_count >= t)
            .min(count_pmf.probs.len() - 1);

        let mut metal = ExactQty::from_integer(0);
        for _ in 0..k {
            let dep_idx = usize::try_from(stream.next_u64() % n_dep_u64)
                .map_err(|_| ExploreError::Overflow)?;
            metal = metal.checked_add(deposit_metals[dep_idx])?;
        }
        if k == 0 {
            zero_count += 1;
        }
        total_sum = total_sum.checked_add(metal)?;
        draws.push(metal);
    }

    // Canonical exact sort using checked_cmp (no float NaN ordering hazards).
    draws.sort_by(|a, b| a.checked_cmp(*b).unwrap_or(core::cmp::Ordering::Equal));

    let quantile = |num: usize, den: usize| -> ExactQty {
        let idx = ((draws.len() - 1) * num) / den;
        draws[idx]
    };
    let p10_metal_kt = quantile(10, 100);
    let p50_metal_kt = quantile(50, 100);
    let p90_metal_kt = quantile(90, 100);
    let empirical_mean_metal_kt =
        total_sum.checked_div(ExactQty::from_integer(i128::from(n_draws)))?;
    let empirical_zero_fraction = ExactQty::new(i128::from(zero_count), i128::from(n_draws))?;

    Ok(ThreePartEndowmentSummary {
        tract_id: tract_id.to_string(),
        n_draws,
        sorted_metal_draws_kt: draws,
        p10_metal_kt,
        p50_metal_kt,
        p90_metal_kt,
        empirical_mean_metal_kt,
        analytical_mean_metal_kt,
        empirical_zero_fraction,
    })
}

/// Exact two-sample Kolmogorov–Smirnov statistic `sup_x |F_a(x) - F_b(x)|` over two non-empty
/// sorted exact-quantity empirical samples (`PO-LODE-5` / `GATE-LODE-P2`: target `KS < 0.02`).
pub fn ks_two_sample_exact(
    sorted_a: &[ExactQty],
    sorted_b: &[ExactQty],
) -> Result<ExactQty, ExploreError> {
    if sorted_a.is_empty() || sorted_b.is_empty() {
        return Err(ExploreError::InvalidRecord {
            field: "ks_samples",
            detail: "two-sample KS test requires non-empty samples".to_string(),
        });
    }
    for w in sorted_a.windows(2) {
        if w[0].checked_cmp(w[1])?.is_gt() {
            return Err(ExploreError::InvalidRecord {
                field: "sorted_a",
                detail: "KS sample A must be sorted ascending".to_string(),
            });
        }
    }
    for w in sorted_b.windows(2) {
        if w[0].checked_cmp(w[1])?.is_gt() {
            return Err(ExploreError::InvalidRecord {
                field: "sorted_b",
                detail: "KS sample B must be sorted ascending".to_string(),
            });
        }
    }

    let na = i128::try_from(sorted_a.len()).map_err(|_| ExploreError::Overflow)?;
    let nb = i128::try_from(sorted_b.len()).map_err(|_| ExploreError::Overflow)?;
    let mut i = 0_usize;
    let mut j = 0_usize;
    let mut max_diff = ExactQty::from_integer(0);

    while i < sorted_a.len() || j < sorted_b.len() {
        let next_x = match (sorted_a.get(i), sorted_b.get(j)) {
            (Some(&xa), Some(&xb)) => {
                if xa.checked_cmp(xb)?.is_le() {
                    xa
                } else {
                    xb
                }
            }
            (Some(&xa), None) => xa,
            (None, Some(&xb)) => xb,
            (None, None) => break,
        };
        while i < sorted_a.len() && sorted_a[i].checked_cmp(next_x)?.is_le() {
            i += 1;
        }
        while j < sorted_b.len() && sorted_b[j].checked_cmp(next_x)?.is_le() {
            j += 1;
        }
        let fa = ExactQty::new(i128::try_from(i).map_err(|_| ExploreError::Overflow)?, na)?;
        let fb = ExactQty::new(i128::try_from(j).map_err(|_| ExploreError::Overflow)?, nb)?;
        let diff = fa.checked_sub(fb)?;
        let abs_diff = if diff.signum() < 0 {
            ExactQty::from_integer(0).checked_sub(diff)?
        } else {
            diff
        };
        if abs_diff.checked_cmp(max_diff)?.is_gt() {
            max_diff = abs_diff;
        }
    }
    Ok(max_diff)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(n: i128, d: i128) -> ExactQty {
        ExactQty::new(n, d).unwrap()
    }

    #[test]
    fn test_three_part_n0_endowment_wald_mean_and_ks_stability() {
        // Mark3B quantile elicitation: N90=1, N50=3, N10=6.
        let pmf = UndiscoveredCountPmf::from_usgs_mark3b_quantiles(1, 3, 6).unwrap();
        let gt_model = vec![
            GradeTonnageDeposit {
                deposit_id: "dep-a".to_string(),
                tonnage_kt: ExactQty::from_integer(100_000),
                grade_fraction: q(5, 1000), // 0.5% Cu -> 500 kt Cu
            },
            GradeTonnageDeposit {
                deposit_id: "dep-b".to_string(),
                tonnage_kt: ExactQty::from_integer(200_000),
                grade_fraction: q(6, 1000), // 0.6% Cu -> 1200 kt Cu
            },
            GradeTonnageDeposit {
                deposit_id: "dep-c".to_string(),
                tonnage_kt: ExactQty::from_integer(50_000),
                grade_fraction: q(8, 1000), // 0.8% Cu -> 400 kt Cu
            },
        ];

        let key1 = RngKey {
            seed_root: 0x2350_0001,
            partition: 1,
            domain: 1,
            var_group: 1,
            realization: 0,
            brick: 0,
        };
        let key2 = RngKey {
            realization: 1,
            ..key1
        };

        let s1 =
            simulate_three_part_endowment("tract-cu-porphyry-1", &pmf, &gt_model, 10_000, key1)
                .unwrap();
        let s2 =
            simulate_three_part_endowment("tract-cu-porphyry-1", &pmf, &gt_model, 10_000, key2)
                .unwrap();

        // Empirical mean agrees with exact Wald analytical mean E[N] * E[M] within 3%.
        let rel_err = (s1.empirical_mean_metal_kt.to_f64() - s1.analytical_mean_metal_kt.to_f64())
            .abs()
            / s1.analytical_mean_metal_kt.to_f64();
        assert!(rel_err < 0.03, "Wald mean rel_err={rel_err}");

        // Independent Philox replicates agree with KS < 0.02 (PO-LODE-5 target).
        let ks = ks_two_sample_exact(&s1.sorted_metal_draws_kt, &s2.sorted_metal_draws_kt).unwrap();
        assert!(
            ks.checked_cmp(q(2, 100)).unwrap().is_lt(),
            "KS distance {} >= 0.02",
            ks.to_f64()
        );
        // Zero-deposit fraction matches P(N=0) = 0.05 within 0.01.
        assert!((s1.empirical_zero_fraction.to_f64() - pmf.p_zero().to_f64()).abs() < 0.01);
        assert!(
            s1.p10_metal_kt
                .checked_cmp(s1.p50_metal_kt)
                .unwrap()
                .is_le()
        );
        assert!(
            s1.p50_metal_kt
                .checked_cmp(s1.p90_metal_kt)
                .unwrap()
                .is_le()
        );
    }
}
