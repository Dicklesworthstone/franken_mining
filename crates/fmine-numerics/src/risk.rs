//! Provenance: frankensim crates/fs-robust/src/lib.rs @ 20463525d9775a18c1b237d2676f364f367175ab;
//! license: MIT/Apache-2.0. Coherent tail risk (`VaR`, `CVaR`) and drawdown metrics (`BD-022`).
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).

/// Summary of empirical loss distribution tail risk (`VaR_alpha`, `CVaR_alpha`).
///
/// Here `losses` represents loss scenarios (larger positive values = worse loss) or negative NPVs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TailRiskSummary {
    pub alpha: f64,
    pub mean_loss: f64,
    pub var_alpha: f64,
    pub cvar_alpha: f64,
}

/// Computes empirical `VaR_alpha` and coherent Rockafellar-Uryasev `CVaR_alpha` (`alpha in (0, 1)`, e.g. `0.95`)
/// over a slice of scenario losses.
#[must_use]
pub fn compute_cvar(losses: &[f64], alpha: f64) -> TailRiskSummary {
    if losses.is_empty() {
        return TailRiskSummary {
            alpha,
            mean_loss: 0.0,
            var_alpha: 0.0,
            cvar_alpha: 0.0,
        };
    }
    let a = alpha.clamp(1e-6, 1.0 - 1e-6);
    let mut sorted = losses.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    let mean_loss = sorted.iter().sum::<f64>() / (n as f64);

    // Exact discrete Rockafellar-Uryasev tail integral over empirical distribution:
    // Integrate quantile function from u = a to 1.0, divided by (1 - a).
    let idx_var = ((a * (n as f64)).ceil() as usize)
        .saturating_sub(1)
        .min(n - 1);
    let var_alpha = sorted[idx_var];

    let mut tail_integral = 0.0_f64;
    let inv_n = 1.0 / (n as f64);
    for (i, &loss) in sorted.iter().enumerate() {
        let u_lo = (i as f64) * inv_n;
        let u_hi = ((i + 1) as f64) * inv_n;
        if u_hi > a {
            let overlap = u_hi - u_lo.max(a);
            tail_integral += overlap * loss;
        }
    }
    let cvar_alpha = tail_integral / (1.0 - a);

    TailRiskSummary {
        alpha: a,
        mean_loss,
        var_alpha,
        cvar_alpha,
    }
}

/// Computes maximum peak-to-trough drawdown over a cumulative cashflow trajectory.
#[must_use]
pub fn max_drawdown(cumulative_cashflow: &[f64]) -> f64 {
    let mut peak = f64::NEG_INFINITY;
    let mut max_dd = 0.0_f64;
    for &v in cumulative_cashflow {
        if v > peak {
            peak = v;
        }
        let dd = peak - v;
        if dd > max_dd {
            max_dd = dd;
        }
    }
    max_dd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cvar_coherent_subadditivity_and_monotonicity() {
        let x = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0];
        let y = [50.0, 10.0, 120.0, 5.0, 80.0, 2.0, 15.0, 40.0];
        let xy: Vec<f64> = x.iter().zip(y.iter()).map(|(a, b)| a + b).collect();

        let alpha = 0.75;
        let rx = compute_cvar(&x, alpha);
        let ry = compute_cvar(&y, alpha);
        let rxy = compute_cvar(&xy, alpha);

        // 1. CVaR >= VaR >= mean_loss.
        assert!(rx.cvar_alpha >= rx.var_alpha - 1e-12);
        assert!(rx.var_alpha >= rx.mean_loss - 1e-12);

        // 2. Coherent subadditivity: CVaR(X + Y) <= CVaR(X) + CVaR(Y).
        assert!(
            rxy.cvar_alpha <= rx.cvar_alpha + ry.cvar_alpha + 1e-9,
            "CVaR subadditivity violated: {} > {} + {}",
            rxy.cvar_alpha,
            rx.cvar_alpha,
            ry.cvar_alpha
        );

        // 3. Positive homogeneity & translation equivariance: CVaR(2*X + 7) == 2*CVaR(X) + 7.
        let scaled: Vec<f64> = x.iter().map(|v| 2.0 * v + 7.0).collect();
        let r_scaled = compute_cvar(&scaled, alpha);
        assert!((r_scaled.cvar_alpha - (2.0 * rx.cvar_alpha + 7.0)).abs() < 1e-9);
    }
}
