//! Logarithmic Mean Divisia Index (`LMDI`) AISC accounting bridge (`GB-26`, BD-333.S1).
//! The factors close on supplied spend divided by ounces sold; printed-rate reconciliation stays separate.
//!
//! Every logarithm goes through `fmine_core::det` (bd-hp2j.2), never the platform libm.
//! Determinism: DeterministicPerIsa (BitwisePortable once one CI run passes its digests on both hosts, bd-hp2j.3).

use fmine_core::det;
use fmine_core::hash::blake3_hash;
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::{Refusal, RefusalCode};

pub mod waterfall;
pub use waterfall::{
    Bar, BarPoint, BasisTag, Bridge, Factor, PrintedQty, RateReconciliation, WATERFALL_NO_CLAIM,
    WATERFALL_RULES_VERSION, WaterfallPeriod, decompose_aisc_waterfall,
};

/// Operational state for one period in the multiplicative AISC accounting identity:
///
/// $$\text{AISC} = \frac{C}{O_{\text{sold}}} = C \cdot T^{-1} \cdot G^{-1} \cdot R^{-1} \cdot B^{-1} \cdot S^{-1}$$
///
/// where:
/// - $C$: Supplied total AISC spend, in the same quantity scale as ounces sold: USD/oz or thousand USD/koz.
///   A public `$M` numerator paired with `koz` must first be multiplied by 1,000; no scale is inferred here.
/// - $T$: Ore tonnes processed
/// - $G$: Head grade (`g/t`)
/// - $R$: Metallurgical recovery fraction (`(0, 1]`)
/// - $S = O_{\text{sold}} / O_{\text{prod}}$: Sales-to-production timing ratio
/// - $B = O_{\text{prod}} / (T \cdot G \cdot R)$: Metal-balance/in-circuit factor, including constant units.
///
/// Both periods must use the same units and ownership basis. This legacy floating-point input does not encode
/// either; [`decompose_aisc_waterfall`] supplies the typed public API, rounding intervals and signed fallback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LmdiState {
    /// Printed rate, retained for reconciliation; it never supplies a factor or the logarithmic-mean weight.
    pub aisc_per_oz: f64,
    pub total_aisc_spend: f64,
    pub tonnes_processed: f64,
    pub head_grade_g_t: f64,
    pub recovery_fraction: f64,
    pub oz_produced: f64,
    pub oz_sold: f64,
}

/// Descriptive LMDI decomposition of the recomputed rate change, with printed gaps and numerical residual.
/// Closure is an accounting identity, not evidence that a bar identifies a cause (plan §21.6, E699).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LmdiWaterfall {
    /// Recomputed endpoint rates, each supplied spend / ounces sold.
    pub aisc_0: f64,
    pub aisc_1: f64,
    pub printed_aisc_0: f64,
    pub printed_aisc_1: f64,
    /// Change in recomputed rates; the six factors sum to this up to `closure_residual`.
    pub total_delta: f64,
    /// Each observed endpoint discrepancy, printed minus recomputed. Equal nonzero gaps remain visible.
    pub printed_rate_gap_0: f64,
    pub printed_rate_gap_1: f64,
    /// Printed rate change minus recomputed rate change. A reconciliation item, never a factor bar.
    pub printed_rate_gap: f64,
    /// Measured `total_delta - sum_of_factors()`, never forced to zero or allocated to a bar.
    pub closure_residual: f64,
    /// Effect of total AISC spend change: $+L(\text{AISC}_1, \text{AISC}_0) \ln(C_1 / C_0)$.
    pub spend_effect: f64,
    /// Effect of throughput change: $-L(\text{AISC}_1, \text{AISC}_0) \ln(T_1 / T_0)$.
    pub throughput_effect: f64,
    /// Effect of head grade change: $-L(\text{AISC}_1, \text{AISC}_0) \ln(G_1 / G_0)$.
    pub grade_effect: f64,
    /// Effect of metallurgical recovery change: $-L(\text{AISC}_1, \text{AISC}_0) \ln(R_1 / R_0)$.
    pub recovery_effect: f64,
    /// Effect of sales/production timing ratio $S = O_{\text{sold}} / O_{\text{prod}}$:
    /// $-L(\text{AISC}_1, \text{AISC}_0) \ln(S_1 / S_0)$.
    pub sales_timing_effect: f64,
    /// Effect of $B = O_{\text{prod}} / (T \cdot G \cdot R)$, not a plug for printed-rate discrepancies.
    pub circuit_inventory_effect: f64,
}

impl LmdiWaterfall {
    /// Sum of the six factor effects. Printed-rate gaps are deliberately excluded.
    #[must_use]
    pub fn sum_of_factors(&self) -> f64 {
        self.spend_effect
            + self.throughput_effect
            + self.grade_effect
            + self.recovery_effect
            + self.sales_timing_effect
            + self.circuit_inventory_effect
    }
}

/// Below this relative difference `|a − b| / mean`, [`logarithmic_mean`] sums the series; at or above it, it
/// takes the logarithm. Both branches agree to about 1e-14 here (bd-hp2j.2).
const SERIES_SWITCH: f64 = 1e-2;

/// The series branch: with $m = (a + b)/2$ and $u = (a - b)/(a + b)$, $\ln a - \ln b = 2\,\operatorname{atanh} u$, so
/// $L = m / (1 + u^2/3 + u^4/5 + u^6/7 + \dots)$. For $|u| < 0.005$ the first omitted term is below $10^{-19}$.
fn series_branch(a: f64, b: f64) -> f64 {
    let sum = a + b;
    let (mean, u) = if sum.is_finite() {
        (0.5 * sum, (a - b) / sum)
    } else {
        let mean = 0.5 * a + 0.5 * b;
        (mean, (0.5 * (a - b)) / mean)
    };
    let u2 = u * u;
    mean / (1.0 + u2 * (1.0 / 3.0 + u2 * (1.0 / 5.0 + u2 / 7.0)))
}

/// The logarithm branch, on the ratio: `ln(a / b)` has no cancellation, where `ln a − ln b` loses about
/// `|ln a| · 1e-16 / |ln(a/b)|` relative (1e-8 at a relative difference of 1e-7, measured). A ratio that
/// overflows falls back to the difference, which is then far from cancelling. The larger argument is always the
/// numerator, so `L(a, b)` and `L(b, a)` are the same bits.
fn log_branch(a: f64, b: f64) -> f64 {
    let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
    let ratio = hi / lo;
    let log = if ratio.is_normal() {
        det::ln(ratio)
    } else {
        det::ln(hi) - det::ln(lo)
    };
    (hi - lo) / log
}

/// Logarithmic mean $L(a, b) = (a - b) / (\ln a - \ln b)$ for positive finite inputs, else NaN.
/// The near-equality test is relative at every scale; an absolute floor would misclassify tiny wide ratios.
#[must_use]
pub fn logarithmic_mean(a: f64, b: f64) -> f64 {
    if !a.is_finite() || !b.is_finite() || a <= 0.0 || b <= 0.0 {
        return f64::NAN;
    }
    if a == b {
        return a;
    }
    let sum = a + b;
    let mean = if sum.is_finite() {
        0.5 * sum
    } else {
        0.5 * a + 0.5 * b
    };
    if (a - b).abs() / mean < SERIES_SWITCH {
        series_branch(a, b)
    } else {
        log_branch(a, b)
    }
}

/// `ln(a/b)` without overflowing or underflowing a positive input ratio. In the ordinary range the ratio
/// preserves the near-equality precision of the existing logarithmic-mean kernel.
fn log_ratio(a: f64, b: f64) -> f64 {
    let ratio = a / b;
    if ratio.is_normal() {
        det::ln(ratio)
    } else {
        det::ln(a) - det::ln(b)
    }
}

/// Computes the spend-based six-effect bridge. Printed endpoints are reconciled separately (BD-333.S1).
/// A non-finite result or closure error above 1e-9 USD/oz refuses instead of supplying a zero residual.
pub fn decompose_aisc_lmdi(s0: &LmdiState, s1: &LmdiState) -> Result<LmdiWaterfall, Refusal> {
    // `x <= 0.0` is false for NaN, so the test is "finite and positive", which a NaN fails.
    let in_domain = |x: f64| x.is_finite() && x > 0.0;
    for (period, st) in [s0, s1].into_iter().enumerate() {
        let vars = [
            ("aisc_per_oz", st.aisc_per_oz),
            ("total_aisc_spend", st.total_aisc_spend),
            ("tonnes_processed", st.tonnes_processed),
            ("head_grade_g_t", st.head_grade_g_t),
            ("recovery_fraction", st.recovery_fraction),
            ("oz_produced", st.oz_produced),
            ("oz_sold", st.oz_sold),
        ];
        if let Some((name, _)) = vars.into_iter().find(|(_, x)| !in_domain(*x)) {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("LMDI period {period} {name} must be finite and strictly positive"),
            ));
        }
        if st.recovery_fraction > 1.0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("LMDI period {period} recovery_fraction must not exceed one"),
            ));
        }
    }

    let aisc_0 = s0.total_aisc_spend / s0.oz_sold;
    let aisc_1 = s1.total_aisc_spend / s1.oz_sold;
    for (period, rate) in [aisc_0, aisc_1].into_iter().enumerate() {
        if !in_domain(rate) {
            return Err(Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                format!(
                    "LMDI period {period} recomputed AISC is outside finite positive arithmetic"
                ),
            ));
        }
    }
    let l_weight = logarithmic_mean(aisc_1, aisc_0);
    let total_delta = aisc_1 - aisc_0;
    let printed_rate_gap_0 = s0.aisc_per_oz - aisc_0;
    let printed_rate_gap_1 = s1.aisc_per_oz - aisc_1;
    let printed_rate_gap = (s1.aisc_per_oz - s0.aisc_per_oz) - total_delta;

    let spend_effect = l_weight * log_ratio(s1.total_aisc_spend, s0.total_aisc_spend);
    let tonnes = log_ratio(s1.tonnes_processed, s0.tonnes_processed);
    let grade = log_ratio(s1.head_grade_g_t, s0.head_grade_g_t);
    let recovery = log_ratio(s1.recovery_fraction, s0.recovery_fraction);
    let produced = log_ratio(s1.oz_produced, s0.oz_produced);
    let sold = log_ratio(s1.oz_sold, s0.oz_sold);
    let throughput_effect = -l_weight * tonnes;
    let grade_effect = -l_weight * grade;
    let recovery_effect = -l_weight * recovery;
    // Work in log space: finite physical inputs must not become NaN from an intermediate T*g*R or S ratio.
    let sales_timing_effect = -l_weight * (sold - produced);
    let circuit_inventory_effect = -l_weight * (produced - tonnes - grade - recovery);
    let closure_residual = total_delta
        - (spend_effect
            + throughput_effect
            + grade_effect
            + recovery_effect
            + sales_timing_effect
            + circuit_inventory_effect);
    if ![
        l_weight,
        spend_effect,
        throughput_effect,
        grade_effect,
        recovery_effect,
        sales_timing_effect,
        circuit_inventory_effect,
        printed_rate_gap,
        closure_residual,
    ]
    .into_iter()
    .all(f64::is_finite)
        || l_weight <= 0.0
        || closure_residual.abs() > 1e-9
    {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "LMDI factors or reconciliation are non-finite, or closure residual exceeds 1e-9 USD/oz",
        ));
    }

    Ok(LmdiWaterfall {
        aisc_0,
        aisc_1,
        printed_aisc_0: s0.aisc_per_oz,
        printed_aisc_1: s1.aisc_per_oz,
        total_delta,
        printed_rate_gap_0,
        printed_rate_gap_1,
        printed_rate_gap,
        closure_residual,
        spend_effect,
        throughput_effect,
        grade_effect,
        recovery_effect,
        sales_timing_effect,
        circuit_inventory_effect,
    })
}

// --- Golden digests (bd-hp2j.3): the same bits on every host, or the kernel is not BitwisePortable ---

/// The Philox key of the LMDI golden digests.
pub const LMDI_DIGEST_KEY: RngKey = RngKey {
    seed_root: 0x6c6d_6469,
    partition: 1,
    domain: 0,
    var_group: 0,
    realization: 0,
    brick: 0,
};

/// A value log-uniform on `[lo, hi]`, through `det` only, so every host draws the same inputs.
fn log_uniform(rng: &mut PhiloxStream, lo: f64, hi: f64) -> f64 {
    let (l, h) = (det::ln(lo), det::ln(hi));
    det::exp(l + rng.next_f64_open01() * (h - l))
}

/// BLAKE3 over each value's little-endian bits.
fn digest_bits(bits: impl IntoIterator<Item = u64>) -> String {
    let bytes: Vec<u8> = bits.into_iter().flat_map(u64::to_le_bytes).collect();
    blake3_hash(&bytes).to_hex()
}

/// The 10,000 `(a, b)` pairs of the logarithmic-mean digest, cycling through four kinds: a relative distance
/// below 1e-7 (the series branch), from 1e-7 to 1 (both branches), ratios up to e^300, and magnitudes from
/// 1e-307 to 1e-290, next to the subnormals.
#[must_use]
pub fn logarithmic_mean_digest_inputs(key: RngKey) -> Vec<(f64, f64)> {
    let mut rng = PhiloxStream::new(key);
    (0..10_000)
        .map(|i| {
            let (a, r) = match i % 4 {
                0 => (
                    log_uniform(&mut rng, 1e-3, 1e6),
                    log_uniform(&mut rng, 1e-16, 1e-7),
                ),
                1 => (
                    log_uniform(&mut rng, 1e-3, 1e6),
                    log_uniform(&mut rng, 1e-7, 1.0),
                ),
                2 => (
                    log_uniform(&mut rng, 1e-150, 1e150),
                    det::exp(300.0 * rng.next_f64_open01()) - 1.0,
                ),
                _ => (
                    log_uniform(&mut rng, 1e-307, 1e-290),
                    log_uniform(&mut rng, 1e-16, 1e3),
                ),
            };
            let b = if rng.next_f64_open01() < 0.5 {
                a * (1.0 + r)
            } else {
                a / (1.0 + r)
            };
            (a, b)
        })
        .collect()
}

/// The digest of [`logarithmic_mean`] over [`logarithmic_mean_digest_inputs`].
#[must_use]
pub fn digest_logarithmic_mean(key: RngKey) -> String {
    digest_bits(
        logarithmic_mean_digest_inputs(key)
            .into_iter()
            .map(|(a, b)| logarithmic_mean(a, b).to_bits()),
    )
}

/// [`digest_logarithmic_mean`] under [`LMDI_DIGEST_KEY`].
#[must_use]
pub fn golden_digest_logarithmic_mean_10k() -> String {
    digest_logarithmic_mean(LMDI_DIGEST_KEY)
}

fn digest_state(rng: &mut PhiloxStream) -> LmdiState {
    let aisc_per_oz = log_uniform(rng, 300.0, 5_000.0);
    let oz_sold = log_uniform(rng, 1.0, 2_000.0);
    let tonnes_processed = log_uniform(rng, 10.0, 1e5);
    let head_grade_g_t = log_uniform(rng, 0.2, 20.0);
    let recovery_fraction = 0.5 + 0.5 * rng.next_f64_open01();
    let oz_produced = oz_sold * log_uniform(rng, 0.5, 2.0);
    LmdiState {
        aisc_per_oz,
        total_aisc_spend: aisc_per_oz * oz_sold * log_uniform(rng, 0.8, 1.2),
        tonnes_processed,
        head_grade_g_t,
        recovery_fraction,
        oz_produced,
        oz_sold,
    }
}

/// The v2 digest of [`decompose_aisc_lmdi`] over 10,000 seeded positive state pairs with independently supplied
/// spend. Per pair it binds recomputed/printed endpoints, delta, endpoint gaps, gap change, measured residual
/// and the six effects (a refusal hashes as `u64::MAX`). The v1 digest closed on printed rates and omitted gaps.
#[must_use]
pub fn digest_decompose_aisc_lmdi(key: RngKey) -> String {
    let mut rng = PhiloxStream::new(key);
    let mut bits = Vec::with_capacity(150_000);
    for _ in 0..10_000 {
        let s0 = digest_state(&mut rng);
        let s1 = digest_state(&mut rng);
        match decompose_aisc_lmdi(&s0, &s1) {
            Ok(w) => bits.extend(
                [
                    w.aisc_0,
                    w.aisc_1,
                    w.printed_aisc_0,
                    w.printed_aisc_1,
                    w.total_delta,
                    w.printed_rate_gap_0,
                    w.printed_rate_gap_1,
                    w.printed_rate_gap,
                    w.closure_residual,
                    w.spend_effect,
                    w.throughput_effect,
                    w.grade_effect,
                    w.recovery_effect,
                    w.sales_timing_effect,
                    w.circuit_inventory_effect,
                ]
                .map(f64::to_bits),
            ),
            Err(_) => bits.push(u64::MAX),
        }
    }
    digest_bits(bits)
}

/// [`digest_decompose_aisc_lmdi`] under [`LMDI_DIGEST_KEY`] (partition 2).
#[must_use]
pub fn golden_digest_decompose_aisc_lmdi_10k() -> String {
    digest_decompose_aisc_lmdi(RngKey {
        partition: 2,
        ..LMDI_DIGEST_KEY
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bd-hp2j.3 log line: kernel, input count, the folded Philox key and counter prefix, digest, host.
    fn digest_line(kernel: &str, key: RngKey, digest: &str) -> String {
        let (k, c) = key.to_philox_key_and_hi_ctr();
        format!(
            "{{\"kernel\":\"{kernel}\",\"n\":10000,\"rng_key\":\"{:08x}{:08x}{:08x}{:08x}\",\"digest\":\"{digest}\",\
             \"target\":\"{}-{}\"}}",
            k[0],
            k[1],
            c[0],
            c[1],
            std::env::consts::ARCH,
            std::env::consts::OS
        )
    }

    // Version 2 TARGETs, first observed on x86_64-linux after the deliberate BD-333.S1 corrections: relative
    // log-mean branch selection, and supplied-spend endpoints with separate printed gaps and measured residual.
    // Independent high-precision answers below establish the mathematics; these snapshots check regression
    // and host agreement. Never refresh them merely to accommodate a different host (bd-hp2j.3).
    const LOGARITHMIC_MEAN_10K: &str =
        "bf89571e2deb3f4510a7d1184f812376fa81e899443f9b25e9aab6d183f3e490";
    const DECOMPOSE_AISC_LMDI_10K: &str =
        "ae31929c9609d83a39953010f652a8dbb4a64bbb905f01c3552be834f92d8656";

    #[test]
    fn test_golden_digest_logarithmic_mean_10k() {
        let d = golden_digest_logarithmic_mean_10k();
        println!("{}", digest_line("logarithmic_mean", LMDI_DIGEST_KEY, &d));
        assert_eq!(d, LOGARITHMIC_MEAN_10K);
    }

    #[test]
    fn test_golden_digest_decompose_aisc_lmdi_10k() {
        let d = golden_digest_decompose_aisc_lmdi_10k();
        let key = RngKey {
            partition: 2,
            ..LMDI_DIGEST_KEY
        };
        println!("{}", digest_line("decompose_aisc_lmdi", key, &d));
        assert_eq!(d, DECOMPOSE_AISC_LMDI_10K);
    }

    #[test]
    fn test_digest_inputs_cover_series_branch_and_tails() {
        let pairs = logarithmic_mean_digest_inputs(LMDI_DIGEST_KEY);
        let rel = |(a, b): (f64, f64)| (a - b).abs() / (0.5 * (a + b));
        let ratio = |(a, b): (f64, f64)| a.max(b) / a.min(b);
        assert!(
            pairs.iter().any(|&p| rel(p) < 1e-7),
            "no pair on the old series branch"
        );
        assert!(
            pairs
                .iter()
                .any(|&p| rel(p) < SERIES_SWITCH && rel(p) > 1e-7)
        );
        assert!(pairs.iter().any(|&p| rel(p) >= SERIES_SWITCH));
        assert!(pairs.iter().any(|&p| ratio(p) > 1e100), "no large ratio");
        assert!(
            pairs.iter().any(|&(a, b)| a.min(b) < 1e-300),
            "nothing near the subnormals"
        );
        assert!(
            pairs
                .iter()
                .all(|&(a, b)| a > 0.0 && b > 0.0 && a.is_finite() && b.is_finite())
        );
    }

    /// One Philox key word changed changes both digests.
    #[test]
    fn test_digests_change_with_the_key() {
        let other = RngKey {
            seed_root: LMDI_DIGEST_KEY.seed_root + 1,
            ..LMDI_DIGEST_KEY
        };
        assert_ne!(
            digest_logarithmic_mean(other),
            golden_digest_logarithmic_mean_10k()
        );
        let other2 = RngKey {
            seed_root: LMDI_DIGEST_KEY.seed_root + 1,
            partition: 2,
            ..LMDI_DIGEST_KEY
        };
        assert_ne!(
            digest_decompose_aisc_lmdi(other2),
            golden_digest_decompose_aisc_lmdi_10k()
        );
    }

    #[test]
    fn test_synthetic_two_period_exact_lmdi() {
        // Synthetic site, invented numbers. (These values were once labelled as a Barrick North Mara case,
        // E137; they match no published cell, R7-C054.)
        // Period 0: AISC = $1,297/oz, oz_produced = 47 koz, oz_sold = 47 koz (S0 = 1.000)
        // Period 1: AISC = $2,582/oz, oz_produced = 54 koz, oz_sold = 72 koz (S1 = 72/54 = 1.3333)
        let q1 = LmdiState {
            aisc_per_oz: 1297.0,
            total_aisc_spend: 1297.0 * 47.0,
            tonnes_processed: 720.0,
            head_grade_g_t: 2.25,
            recovery_fraction: 0.90,
            oz_produced: 47.0,
            oz_sold: 47.0,
        };
        let q2 = LmdiState {
            aisc_per_oz: 2582.0,
            total_aisc_spend: 2582.0 * 72.0,
            tonnes_processed: 735.0,
            head_grade_g_t: 2.52,
            recovery_fraction: 0.905,
            oz_produced: 54.0,
            oz_sold: 72.0,
        };

        let wf = decompose_aisc_lmdi(&q1, &q2).expect("valid LMDI");
        assert!((wf.total_delta - 1285.0).abs() < 1e-12);
        // Zero-residual guarantee (GB-26): sum of factors == +$1,285.00/oz to < 1e-11.
        assert!(
            (wf.sum_of_factors() - wf.total_delta).abs() < 1e-11,
            "LMDI residual non-zero: {} vs {}",
            wf.sum_of_factors(),
            wf.total_delta
        );
        // Independent 60-place Decimal calculation: L = 1866.3480233476275208 and the sales effect is
        // -536.9148672729303024. These invented endpoints have no printed-rate discrepancy.
        assert!((wf.sales_timing_effect + 536.9148672729303).abs() < 1e-9);
        assert_eq!(
            (
                wf.printed_rate_gap_0,
                wf.printed_rate_gap_1,
                wf.printed_rate_gap
            ),
            (0.0, 0.0, 0.0)
        );
        assert_eq!(wf.closure_residual, wf.total_delta - wf.sum_of_factors());
        assert!(wf.spend_effect > 1800.0);

        // A non-positive state refuses with 109 (outside its physical domain), never 105 (basis mismatch).
        let zero_sold = LmdiState { oz_sold: 0.0, ..q2 };
        let err = decompose_aisc_lmdi(&q1, &zero_sold).expect_err("non-positive");
        assert_eq!((err.code, err.exit_code), ("FMINE-REF-109", 3));
        // NaN and infinity are outside the domain too; a `<= 0.0` test alone let NaN through.
        for bad in [f64::NAN, f64::INFINITY] {
            let st = LmdiState {
                head_grade_g_t: bad,
                ..q2
            };
            let err = decompose_aisc_lmdi(&q1, &st).expect_err("non-finite");
            assert_eq!((err.code, err.exit_code), ("FMINE-REF-109", 3), "{bad}");
        }
    }

    fn simple_state() -> LmdiState {
        LmdiState {
            aisc_per_oz: 100.0,
            total_aisc_spend: 1000.0,
            tonnes_processed: 10.0,
            head_grade_g_t: 2.0,
            recovery_fraction: 0.5,
            oz_produced: 10.0,
            oz_sold: 10.0,
        }
    }

    fn effects(w: &LmdiWaterfall) -> [f64; 6] {
        [
            w.spend_effect,
            w.throughput_effect,
            w.grade_effect,
            w.recovery_effect,
            w.sales_timing_effect,
            w.circuit_inventory_effect,
        ]
    }

    fn assert_reconciled(w: &LmdiWaterfall) {
        assert_eq!(w.closure_residual, w.total_delta - w.sum_of_factors());
        assert!(w.closure_residual.abs() <= 1e-9, "{w:?}");
        assert!(
            (w.sum_of_factors() + w.closure_residual + w.printed_rate_gap
                - (w.printed_aisc_1 - w.printed_aisc_0))
                .abs()
                < 1e-9,
            "{w:?}"
        );
    }

    /// Hand identity: C doubles, Q and every physical factor stay fixed. Only the cost bar can move, by 100.
    #[test]
    fn test_supplied_cost_drives_factors_and_printed_rate_only_drives_gaps() {
        let p0 = LmdiState {
            aisc_per_oz: 105.0,
            ..simple_state()
        };
        let p1 = LmdiState {
            aisc_per_oz: 209.0,
            total_aisc_spend: 2000.0,
            ..simple_state()
        };
        let w = decompose_aisc_lmdi(&p0, &p1).unwrap();
        assert_eq!((w.aisc_0, w.aisc_1, w.total_delta), (100.0, 200.0, 100.0));
        assert_eq!(
            (
                w.printed_rate_gap_0,
                w.printed_rate_gap_1,
                w.printed_rate_gap
            ),
            (5.0, 9.0, 4.0)
        );
        assert!((w.spend_effect - 100.0).abs() < 1e-12);
        assert!(effects(&w)[1..].iter().all(|v| *v == 0.0));
        assert_reconciled(&w);

        let different_print = LmdiState {
            aisc_per_oz: 409.0,
            ..p1
        };
        let changed = decompose_aisc_lmdi(&p0, &different_print).unwrap();
        assert_eq!(
            effects(&w).map(f64::to_bits),
            effects(&changed).map(f64::to_bits)
        );
        assert_eq!(changed.total_delta, 100.0);
        assert_eq!(changed.printed_rate_gap, 204.0);
        assert_reconciled(&changed);

        // Conversely, unchanged printed rates must not hide a supplied-cost change.
        let unchanged_print = LmdiState {
            aisc_per_oz: 105.0,
            ..p1
        };
        let changed = decompose_aisc_lmdi(&p0, &unchanged_print).unwrap();
        assert!((changed.spend_effect - 100.0).abs() < 1e-12);
        assert_eq!(changed.printed_rate_gap, -100.0);
        assert_reconciled(&changed);
    }

    #[test]
    fn test_equal_endpoint_gaps_remain_visible_when_gap_change_is_zero() {
        let p0 = LmdiState {
            aisc_per_oz: 105.0,
            ..simple_state()
        };
        let p1 = LmdiState {
            aisc_per_oz: 205.0,
            total_aisc_spend: 2000.0,
            ..simple_state()
        };
        let w = decompose_aisc_lmdi(&p0, &p1).unwrap();
        assert_eq!((w.printed_rate_gap_0, w.printed_rate_gap_1), (5.0, 5.0));
        assert_eq!(w.printed_rate_gap, 0.0);
        assert_reconciled(&w);
    }

    /// Hand identity: fixed spend, twice the throughput/produced/sold quantity. Only throughput moves, -50.
    #[test]
    fn test_volume_change_closes_on_recomputed_rate() {
        let p0 = simple_state();
        let p1 = LmdiState {
            aisc_per_oz: 55.0,
            tonnes_processed: 20.0,
            oz_produced: 20.0,
            oz_sold: 20.0,
            ..p0
        };
        let w = decompose_aisc_lmdi(&p0, &p1).unwrap();
        assert_eq!((w.aisc_0, w.aisc_1, w.total_delta), (100.0, 50.0, -50.0));
        assert!((w.throughput_effect + 50.0).abs() < 1e-12);
        assert_eq!(w.spend_effect, 0.0);
        assert_eq!(w.printed_rate_gap, 5.0);
        assert_reconciled(&w);
    }

    /// Only C, Q and printed AISC are the bead's published North Mara inputs (BD-333.S1, E819):
    /// Q2 2025 AISC P18/P19/P23 = 64 $M / 50 koz / 1292; Q2 2026 M17/M18 = 93 $M / 37 koz, rate 2577.
    /// Physical factors below are invented to isolate reconciliation; this is not a site-driver attribution.
    #[test]
    fn test_north_mara_rate_gap_with_synthetic_physical_factors() {
        let p0 = LmdiState {
            aisc_per_oz: 1292.0,
            total_aisc_spend: 64_000.0,
            oz_sold: 50.0,
            ..simple_state()
        };
        let p1 = LmdiState {
            aisc_per_oz: 2577.0,
            total_aisc_spend: 93_000.0,
            oz_sold: 37.0,
            ..simple_state()
        };
        let w = decompose_aisc_lmdi(&p0, &p1).unwrap();
        assert_eq!(w.aisc_0, 1280.0);
        assert!((w.aisc_1 - 93_000.0 / 37.0).abs() < 1e-12);
        assert!((w.total_delta - 45_640.0 / 37.0).abs() < 1e-12);
        assert_eq!(w.printed_rate_gap_0, 12.0);
        assert!((w.printed_rate_gap - 1905.0 / 37.0).abs() < 1e-12);
        assert!((w.sum_of_factors() + w.printed_rate_gap - 1285.0).abs() < 1e-9);
        assert_reconciled(&w);
    }

    #[test]
    fn test_grade_change_reallocates_into_balance_without_identifying_a_cause() {
        let p0 = simple_state();
        let p1 = LmdiState {
            head_grade_g_t: 2.2,
            ..p0
        };
        let w = decompose_aisc_lmdi(&p0, &p1).unwrap();
        assert_eq!(w.total_delta, 0.0);
        assert!(w.grade_effect < 0.0 && w.circuit_inventory_effect > 0.0);
        assert!((w.grade_effect + w.circuit_inventory_effect).abs() < 1e-12);
        assert_reconciled(&w);
    }

    #[test]
    fn test_invalid_inputs_name_the_period_and_field() {
        let names = [
            "aisc_per_oz",
            "total_aisc_spend",
            "tonnes_processed",
            "head_grade_g_t",
            "recovery_fraction",
            "oz_produced",
            "oz_sold",
        ];
        for period in 0..2 {
            for (field, name) in names.into_iter().enumerate() {
                for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                    let mut pair = [simple_state(); 2];
                    let st = &mut pair[period];
                    match field {
                        0 => st.aisc_per_oz = bad,
                        1 => st.total_aisc_spend = bad,
                        2 => st.tonnes_processed = bad,
                        3 => st.head_grade_g_t = bad,
                        4 => st.recovery_fraction = bad,
                        5 => st.oz_produced = bad,
                        _ => st.oz_sold = bad,
                    }
                    let err = decompose_aisc_lmdi(&pair[0], &pair[1]).unwrap_err();
                    assert_eq!((err.code, err.exit_code), ("FMINE-REF-109", 3));
                    assert!(
                        err.what.contains(&format!("period {period} {name}")),
                        "{err:?}"
                    );
                }
            }
        }
        let bad = LmdiState {
            recovery_fraction: 1.01,
            ..simple_state()
        };
        let err = decompose_aisc_lmdi(&simple_state(), &bad).unwrap_err();
        assert_eq!(err.code, "FMINE-REF-109");
        assert!(err.what.contains("period 1 recovery_fraction"));
        let one = LmdiState {
            recovery_fraction: 1.0,
            ..simple_state()
        };
        assert_reconciled(&decompose_aisc_lmdi(&one, &one).unwrap());
    }

    #[test]
    fn test_unrepresentable_rates_refuse_instead_of_returning_nan() {
        for (cost, sold) in [(f64::MAX, f64::MIN_POSITIVE), (f64::MIN_POSITIVE, f64::MAX)] {
            let bad = LmdiState {
                total_aisc_spend: cost,
                oz_sold: sold,
                ..simple_state()
            };
            let err = decompose_aisc_lmdi(&simple_state(), &bad).unwrap_err();
            assert_eq!((err.code, err.exit_code), ("FMINE-REF-207", 3));
            assert!(err.what.contains("period 1 recomputed AISC"));
        }
        let large = LmdiState {
            total_aisc_spend: 1e308,
            oz_sold: 1.0,
            ..simple_state()
        };
        let too_large_effect = LmdiState {
            tonnes_processed: 1e308,
            ..large
        };
        assert_eq!(
            decompose_aisc_lmdi(&large, &too_large_effect)
                .unwrap_err()
                .code,
            "FMINE-REF-207"
        );
    }

    #[test]
    fn test_log_space_avoids_product_overflow_and_subnormal_ratio_rounding() {
        let large = LmdiState {
            tonnes_processed: 1e200,
            head_grade_g_t: 1e200,
            ..simple_state()
        };
        let unchanged = decompose_aisc_lmdi(&large, &large).unwrap();
        assert!(effects(&unchanged).iter().all(|v| *v == 0.0));
        assert_reconciled(&unchanged);

        // Independent review case: temporal cost and ounce ratios are positive subnormals, while both
        // endpoint rates are ordinary finite numbers. Logging those rounded subnormals breaks closure.
        let p0 = LmdiState {
            total_aisc_spend: 1e300,
            oz_sold: 1e297,
            oz_produced: 1e297,
            ..simple_state()
        };
        let p1 = LmdiState {
            total_aisc_spend: 1e-20,
            oz_sold: 1.1e-23,
            oz_produced: 1.1e-23,
            ..simple_state()
        };
        assert_reconciled(&decompose_aisc_lmdi(&p0, &p1).unwrap());
    }

    fn pairs(stream: u32) -> (fmine_core::rand::RngKey, Vec<(f64, f64)>) {
        let key = fmine_core::rand::RngKey {
            seed_root: 2_242,
            partition: stream,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        };
        let mut rng = fmine_core::rand::PhiloxStream::new(key);
        let out = (0..10_000)
            .map(|_| {
                // a in (1e-3, 1e6); b at a relative distance 10^-e, e in [0, 14), either side. The inputs come
                // from det too, so every host tests the same pairs.
                let ln10 = std::f64::consts::LN_10;
                let a = 1e-3 * det::exp(9.0 * ln10 * rng.next_f64_open01());
                let r = det::exp(-14.0 * ln10 * rng.next_f64_open01());
                let b = if rng.next_f64_open01() < 0.5 {
                    a * (1.0 + r)
                } else {
                    a / (1.0 + r)
                };
                (a, b)
            })
            .collect();
        (key, out)
    }

    #[test]
    fn test_logarithmic_mean_high_precision_tiny_and_large_known_answers() {
        // Independent Python Decimal (80 decimal digits), (a-b)/(ln(a)-ln(b)), on the decimal inputs.
        // These are mathematical expected values, not output snapshots from the candidate kernel.
        for (a, b, expected) in [
            (1.0, 0.001, 0.14462006247378287),
            (1e-18, 1e-21, 1.4462006247378286e-19),
            (1e-300, 1e-303, 1.4462006247378286e-301),
            (1e-308, 1e-310, 2.1497576854211e-309),
            (1.6e308, 1.5e308, 1.549462216322538e308),
            (1.6e308, 1.599e308, 1.5994999479003842e308),
        ] {
            let actual = logarithmic_mean(a, b);
            assert!(
                (actual / expected - 1.0).abs() < 2e-14,
                "L({a},{b})={actual}, expected {expected}"
            );
            assert_eq!(actual.to_bits(), logarithmic_mean(b, a).to_bits());
        }
        for x in [f64::from_bits(1), 1e-300, 1.0, f64::MAX] {
            assert_eq!(logarithmic_mean(x, x), x);
        }
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(logarithmic_mean(1.0, bad).is_nan());
            assert!(logarithmic_mean(bad, 1.0).is_nan());
        }
    }

    #[test]
    fn test_logarithmic_mean_scale_invariance_for_widely_separated_rates() {
        let unscaled = logarithmic_mean(1.0, 0.001);
        for scale in [1e-300, 1e-18, 1e-10, 1.0, 1e100, 1e300] {
            let actual = logarithmic_mean(scale, 0.001 * scale);
            assert!(
                (actual / (unscaled * scale) - 1.0).abs() < 2e-14,
                "scale={scale}"
            );
        }
    }

    #[test]
    fn test_logarithmic_mean_between_min_and_max() {
        let (key, pairs) = pairs(1);
        for &(a, b) in &pairs {
            let l = logarithmic_mean(a, b);
            assert!(a.min(b) <= l && l <= a.max(b), "L({a:e}, {b:e}) = {l:e}");
        }
        println!(
            "{{\"test\":\"between\",\"cases\":{},\"key\":\"{key:?}\"}}",
            pairs.len()
        );
    }

    #[test]
    fn test_logarithmic_mean_symmetric() {
        let (key, pairs) = pairs(2);
        for &(a, b) in &pairs {
            assert_eq!(
                logarithmic_mean(a, b).to_bits(),
                logarithmic_mean(b, a).to_bits()
            );
        }
        println!(
            "{{\"test\":\"symmetric\",\"cases\":{},\"key\":\"{key:?}\"}}",
            pairs.len()
        );
    }

    /// The jump across the switch is the two branches' difference there: both are evaluated at the same points,
    /// a relative distance of SERIES_SWITCH · (1 ± 1e-3) apart, on either side.
    #[test]
    fn test_logarithmic_mean_continuous_at_series_switch() {
        let (key, pairs) = pairs(3);
        let mut max_rel_jump = 0.0_f64;
        for (i, &(a, _)) in pairs.iter().enumerate() {
            let r = SERIES_SWITCH * (1.0 + 1e-3 * ((i % 21) as f64 - 10.0) / 10.0);
            let b = if i % 2 == 0 {
                a * (1.0 + r)
            } else {
                a / (1.0 + r)
            };
            let (s, l) = (series_branch(a, b), log_branch(a, b));
            max_rel_jump = max_rel_jump.max((s - l).abs() / l);
        }
        println!(
            "{{\"test\":\"continuous_at_switch\",\"cases\":{},\"key\":\"{key:?}\",\"max_rel_jump\":{max_rel_jump:e}}}",
            pairs.len()
        );
        assert!(max_rel_jump < 1e-12, "max relative jump {max_rel_jump:e}");
    }
}
