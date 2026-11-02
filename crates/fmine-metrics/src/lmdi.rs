//! Logarithmic Mean Divisia Index (`LMDI`) AISC accounting bridge (`GB-26`, BD-333.S1).
//! The factors close on supplied spend divided by ounces sold; printed-rate reconciliation stays separate.
//!
//! Every logarithm goes through `fmine_core::det` (bd-hp2j.2), never the platform libm.
//! Determinism: BitwisePortable (its two golden digests agree on x86_64-linux and aarch64-macos in CI run
//! 41237993223 at 84f32ac, bd-hp2j.3).

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

include!("../../xtask/testdata/metrics_lmdi_unit_tests.rs");
