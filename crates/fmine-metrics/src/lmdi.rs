//! Exact Logarithmic Mean Divisia Index (`LMDI`) 5-factor AISC waterfall (`GB-26`, `BD-065`, `BD-274`).
//!
//! Every logarithm goes through `fmine_core::det` (bd-hp2j.2), never the platform libm.

use fmine_core::det;
use fmine_evidence::Refusal;

/// Operational state for a single period used in the 5-factor multiplicative AISC identity:
///
/// $$\text{AISC} = \frac{C}{O_{\text{sold}}} = C \cdot T^{-1} \cdot G^{-1} \cdot R^{-1} \cdot S^{-1} \cdot k$$
///
/// where:
/// - $C$: Total AISC spend (`$M` or `$`)
/// - $T$: Ore tonnes processed
/// - $G$: Head grade (`g/t`)
/// - $R$: Metallurgical recovery fraction (`(0, 1]`)
/// - $S = O_{\text{sold}} / O_{\text{prod}}$: Sales-to-production timing ratio
/// - $O_{\text{prod}} = T \cdot G \cdot R / k$: Produced ounces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LmdiState {
    pub aisc_per_oz: f64,
    pub total_aisc_spend: f64,
    pub tonnes_processed: f64,
    pub head_grade_g_t: f64,
    pub recovery_fraction: f64,
    pub oz_produced: f64,
    pub oz_sold: f64,
}

/// Exact additive LMDI decomposition of `delta_aisc = aisc_1 - aisc_0` with zero residual (`GB-26`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LmdiWaterfall {
    pub aisc_0: f64,
    pub aisc_1: f64,
    pub total_delta: f64,
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
    /// Effect of in-circuit inventory / rounding bridge between $T \cdot G \cdot R$ and $O_{\text{prod}}$.
    pub circuit_inventory_effect: f64,
}

impl LmdiWaterfall {
    /// Sum of all decomposed factor effects (equals `total_delta` to machine precision).
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
    let u = (a - b) / (a + b);
    let u2 = u * u;
    0.5 * (a + b) / (1.0 + u2 * (1.0 / 3.0 + u2 * (1.0 / 5.0 + u2 / 7.0)))
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

/// Logarithmic mean $L(a, b) = (a - b) / (\ln a - \ln b)$, with a series branch near $a = b$ (`GB-26`).
#[must_use]
pub fn logarithmic_mean(a: f64, b: f64) -> f64 {
    let mean = 0.5 * (a + b);
    // `max(1e-15)` only selects the series branch near a = b; it never changes the returned value's inputs, and
    // callers refuse non-positive states (FMINE-REF-301) before reaching here. Not a domain clamp.
    if (a - b).abs() / mean.max(1e-15) < SERIES_SWITCH {
        series_branch(a, b)
    } else {
        log_branch(a, b)
    }
}

/// Computes the exact 5-factor (+ circuit bridge) LMDI waterfall between `s0` and `s1` (`GB-26`, `BD-065`, `BD-274`).
pub fn decompose_aisc_lmdi(s0: &LmdiState, s1: &LmdiState) -> Result<LmdiWaterfall, Refusal> {
    for st in [s0, s1] {
        if st.aisc_per_oz <= 0.0
            || st.total_aisc_spend <= 0.0
            || st.tonnes_processed <= 0.0
            || st.head_grade_g_t <= 0.0
            || st.recovery_fraction <= 0.0
            || st.oz_produced <= 0.0
            || st.oz_sold <= 0.0
        {
            return Err(Refusal::from_code(
                "FMINE-REF-105",
                "all LMDI state variables must be strictly positive",
            ));
        }
    }

    let l_weight = logarithmic_mean(s1.aisc_per_oz, s0.aisc_per_oz);
    let total_delta = s1.aisc_per_oz - s0.aisc_per_oz;

    // Exact identity:
    // AISC = (aisc_per_oz * oz_sold / total_aisc_spend) * total_aisc_spend / oz_sold
    // and oz_sold = tonnes * grade * recovery * (oz_prod / (tonnes*grade*recovery)) * (oz_sold / oz_prod)
    let unit_spend_0 = s0.aisc_per_oz * s0.oz_sold;
    let unit_spend_1 = s1.aisc_per_oz * s1.oz_sold;

    let spend_effect = l_weight * det::ln(unit_spend_1 / unit_spend_0);
    let throughput_effect = -l_weight * det::ln(s1.tonnes_processed / s0.tonnes_processed);
    let grade_effect = -l_weight * det::ln(s1.head_grade_g_t / s0.head_grade_g_t);
    let recovery_effect = -l_weight * det::ln(s1.recovery_fraction / s0.recovery_fraction);

    let s_ratio_0 = s0.oz_sold / s0.oz_produced;
    let s_ratio_1 = s1.oz_sold / s1.oz_produced;
    let sales_timing_effect = -l_weight * det::ln(s_ratio_1 / s_ratio_0);

    let circuit_0 =
        s0.oz_produced / (s0.tonnes_processed * s0.head_grade_g_t * s0.recovery_fraction);
    let circuit_1 =
        s1.oz_produced / (s1.tonnes_processed * s1.head_grade_g_t * s1.recovery_fraction);
    let circuit_inventory_effect = -l_weight * det::ln(circuit_1 / circuit_0);

    Ok(LmdiWaterfall {
        aisc_0: s0.aisc_per_oz,
        aisc_1: s1.aisc_per_oz,
        total_delta,
        spend_effect,
        throughput_effect,
        grade_effect,
        recovery_effect,
        sales_timing_effect,
        circuit_inventory_effect,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Sales timing effect (72 koz sold vs 54 koz produced):
        // -L(2582, 1297) * ln(72 / 54), with L ≈ 1866.35 and ln(4/3) ≈ 0.287682: about -$536.91/oz (dilutes unit
        // AISC while spend surged).
        assert!(wf.sales_timing_effect < -500.0 && wf.sales_timing_effect > -550.0);
        assert!(wf.spend_effect > 1800.0);
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
