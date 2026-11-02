//! Original FrankenMining code (not lifted). Upstream analogue: frankensim crates/fs-robust/src/lib.rs @ 20463525d (not copied).
//! Coherent tail risk (`VaR`, `CVaR`) and drawdown metrics (`BD-022`).
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).

pub mod exact;

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

/// Why a tail-risk or drawdown computation refused (bd-a482). There is no zero default and no clamped `alpha`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiskError {
    /// No scenarios, or an empty trajectory.
    Empty,
    /// The value at `index` is NaN or infinite.
    NonFinite { index: usize, value: f64 },
    /// `alpha` is not a finite value in `(0, 1)`.
    AlphaOutOfDomain(f64),
}

impl std::fmt::Display for RiskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "no values to measure"),
            Self::NonFinite { index, value } => write!(f, "value {index} is {value}, not finite"),
            Self::AlphaOutOfDomain(a) => write!(f, "alpha must lie in (0, 1), got {a}"),
        }
    }
}

impl std::error::Error for RiskError {}

/// The values, if there is at least one and every one is finite.
fn check_finite(values: &[f64]) -> Result<(), RiskError> {
    if values.is_empty() {
        return Err(RiskError::Empty);
    }
    match values.iter().position(|v| !v.is_finite()) {
        Some(index) => Err(RiskError::NonFinite {
            index,
            value: values[index],
        }),
        None => Ok(()),
    }
}

/// Computes empirical `VaR_alpha` and coherent Rockafellar-Uryasev `CVaR_alpha` (`alpha in (0, 1)`, e.g. `0.95`)
/// over a slice of scenario losses.
///
/// # Errors
/// [`RiskError::Empty`] for no losses, [`RiskError::NonFinite`] for a NaN or infinite loss, and
/// [`RiskError::AlphaOutOfDomain`] for `alpha` outside `(0, 1)`.
pub fn compute_cvar(losses: &[f64], alpha: f64) -> Result<TailRiskSummary, RiskError> {
    if !(alpha.is_finite() && alpha > 0.0 && alpha < 1.0) {
        return Err(RiskError::AlphaOutOfDomain(alpha));
    }
    check_finite(losses)?;
    let mut sorted = losses.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    let n_f = n as f64;
    let mean_loss = sorted.iter().sum::<f64>() / n_f;

    // Exact discrete Rockafellar-Uryasev tail integral over empirical distribution:
    // Integrate quantile function from u = alpha to 1.0, divided by (1 - alpha).
    let idx_var = ((alpha * n_f).ceil() as usize).saturating_sub(1).min(n - 1);
    let var_alpha = sorted[idx_var];

    // The bounds are `i / n`, divided rather than multiplied by `1 / n`, so the last one is exactly 1 and the
    // largest loss always carries weight `1 - max((n - 1) / n, alpha) > 0`. With `* (1 / n)`, n = 49 gives a last
    // bound of 1 - 2^-53, and an alpha that close to 1 would integrate nothing.
    let mut tail_integral = 0.0_f64;
    for (i, &loss) in sorted.iter().enumerate() {
        let u_lo = (i as f64) / n_f;
        let u_hi = ((i + 1) as f64) / n_f;
        if u_hi > alpha {
            let overlap = u_hi - u_lo.max(alpha);
            tail_integral += overlap * loss;
        }
    }
    let cvar_alpha = tail_integral / (1.0 - alpha);

    Ok(TailRiskSummary {
        alpha,
        mean_loss,
        var_alpha,
        cvar_alpha,
    })
}

/// Computes maximum peak-to-trough drawdown over a cumulative cashflow trajectory.
///
/// # Errors
/// [`RiskError::Empty`] for an empty trajectory and [`RiskError::NonFinite`] for a NaN or infinite value.
pub fn max_drawdown(cumulative_cashflow: &[f64]) -> Result<f64, RiskError> {
    check_finite(cumulative_cashflow)?;
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
    Ok(max_dd)
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
        let rx = compute_cvar(&x, alpha).expect("valid");
        let ry = compute_cvar(&y, alpha).expect("valid");
        let rxy = compute_cvar(&xy, alpha).expect("valid");

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
        let r_scaled = compute_cvar(&scaled, alpha).expect("valid");
        assert!((r_scaled.cvar_alpha - (2.0 * rx.cvar_alpha + 7.0)).abs() < 1e-9);
    }

    #[test]
    fn test_risk_refuses_empty_non_finite_and_alpha_out_of_domain() {
        // Before bd-a482 these gave zeros, a NaN CVaR, or an alpha clamped to 1e-6 or 1 - 1e-6.
        assert_eq!(compute_cvar(&[], 0.95), Err(RiskError::Empty));
        assert!(matches!(
            compute_cvar(&[1.0, f64::NAN, 3.0], 0.95),
            Err(RiskError::NonFinite { index: 1, .. })
        ));
        assert!(matches!(
            compute_cvar(&[1.0, f64::INFINITY], 0.5),
            Err(RiskError::NonFinite { index: 1, .. })
        ));
        for a in [0.0, 1.0, -0.5, 1.5, f64::NAN, f64::INFINITY] {
            assert!(
                matches!(
                    compute_cvar(&[1.0, 2.0], a),
                    Err(RiskError::AlphaOutOfDomain(_))
                ),
                "alpha {a}"
            );
        }
        assert_eq!(max_drawdown(&[]), Err(RiskError::Empty));
        assert!(matches!(
            max_drawdown(&[0.0, f64::NAN]),
            Err(RiskError::NonFinite { index: 1, .. })
        ));
        assert_eq!(max_drawdown(&[0.0, 5.0, 2.0, 7.0, 1.0]), Ok(6.0));

        // An alpha just below 1 is taken as given, and its CVaR is the largest loss.
        let alpha = 1.0 - f64::EPSILON / 2.0;
        let losses: Vec<f64> = (1..=49).map(f64::from).collect();
        let r = compute_cvar(&losses, alpha).expect("valid");
        assert_eq!(r.alpha, alpha);
        assert!((r.cvar_alpha - 49.0).abs() < 1e-6, "CVaR {}", r.cvar_alpha);
    }
}
