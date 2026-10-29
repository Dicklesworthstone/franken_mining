//! Exact Logarithmic Mean Divisia Index (`LMDI`) 5-factor AISC waterfall (`GB-26`, `BD-065`, `BD-274`).

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

/// Logarithmic mean $L(a, b) = (a - b) / (\ln a - \ln b)$ with smooth $a \to b$ series expansion (`GB-26`).
#[must_use]
pub fn logarithmic_mean(a: f64, b: f64) -> f64 {
    let diff = a - b;
    let mean = 0.5 * (a + b);
    if diff.abs() / mean.max(1e-15) < 1e-7 {
        let u = diff / (a + b);
        mean * (1.0 - (u * u) / 3.0 - 4.0 * u.powi(4) / 45.0)
    } else {
        diff / (a.ln() - b.ln())
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

    let spend_effect = l_weight * (unit_spend_1 / unit_spend_0).ln();
    let throughput_effect = -l_weight * (s1.tonnes_processed / s0.tonnes_processed).ln();
    let grade_effect = -l_weight * (s1.head_grade_g_t / s0.head_grade_g_t).ln();
    let recovery_effect = -l_weight * (s1.recovery_fraction / s0.recovery_fraction).ln();

    let s_ratio_0 = s0.oz_sold / s0.oz_produced;
    let s_ratio_1 = s1.oz_sold / s1.oz_produced;
    let sales_timing_effect = -l_weight * (s_ratio_1 / s_ratio_0).ln();

    let circuit_0 = s0.oz_produced / (s0.tonnes_processed * s0.head_grade_g_t * s0.recovery_fraction);
    let circuit_1 = s1.oz_produced / (s1.tonnes_processed * s1.head_grade_g_t * s1.recovery_fraction);
    let circuit_inventory_effect = -l_weight * (circuit_1 / circuit_0).ln();

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
    fn test_north_mara_2024q1_to_q2_exact_lmdi_e137() {
        // Real Barrick corpus case E137 (barrick-mine-stats-2025-q4.xlsx & barrick-aisc-2024-q2.xlsx):
        // North Mara (84% Barrick):
        // 2024Q1: AISC = $1,297/oz, oz_produced = 47 koz, oz_sold = 47 koz (S0 = 1.000)
        // 2024Q2: AISC = $2,582/oz, oz_produced = 54 koz, oz_sold = 72 koz (S1 = 72/54 = 1.3333)
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
        // -L(2582, 1297) * ln(72 / 54) = -1865.629 * 0.287682 = -$536.71/oz (dilutes unit AISC while spend surged).
        assert!(wf.sales_timing_effect < -500.0 && wf.sales_timing_effect > -550.0);
        assert!(wf.spend_effect > 1800.0);
    }
}
