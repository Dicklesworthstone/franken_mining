//! DESIGN: conditional linear reconciliation with observable unmeasured flows.
//! Minimize (xm-y)' V^-1 (xm-y), subject to Am*xm + Au*xu = b.
//! This bounded owner requires independent measured balance rows (Am V Am' SPD).
//! The plant adapter guarantees that condition by retaining both stocktakes for
//! every unit. Unmeasured columns must be independently observable; no prior,
//! placeholder observation, variance floor or discarded constraint is supplied.
//!
//! With C=Am V Am', S=Au' C^-1 Au and r=Am y-b:
//! xu=-S^-1 Au' C^-1 r; lambda=C^-1(r+Au*xu);
//! xm=y-V Am' lambda. Existing scaled LDLT factors are reused for every solve.
//! One affine estimator map propagates the complete original V to the joint
//! measured/inferred covariance. Degrees of freedom are rows minus latent
//! columns; zero means inference without a redundant measurement test.
//! Determinism: DeterministicPerIsa

use super::{
    MAX_CONSTRAINTS, MAX_OBSERVATIONS, Result, ScaledSpd, audit, covariance_congruence, dot,
    finite, refuse,
};
use fmine_evidence::{Refusal, RefusalCode};

/// Numerical coordinates only; a domain owner supplies units and source Claims.
#[derive(Debug, Clone, PartialEq)]
pub struct PartialReconciliation {
    pub measured_estimates: Vec<f64>,
    pub measured_adjustments: Vec<f64>,
    pub latent_estimates: Vec<f64>,
    /// Full row-major covariance, measured coordinates followed by latent ones.
    pub posterior_covariance: Vec<f64>,
    /// Full covariance of the measured adjustments, in original measured order.
    pub adjustment_covariance: Vec<f64>,
    /// Am*y-b only. Missing flows prevent interpreting this as a full imbalance.
    pub raw_observed_residuals: Vec<f64>,
    pub reconciled_residuals: Vec<f64>,
    /// Conditional quadratic objective. With zero degrees of freedom this is
    /// exactly zero and supplies no gross-error acceptance or test power.
    pub chi_square: f64,
    pub degrees_of_freedom: usize,
    pub normalized_adjustments: Vec<Option<f64>>,
    /// Row-major derivative d[xm,xu]/dy. Targets are fixed, not random inputs.
    pub estimator_map: Vec<f64>,
}

fn unobservable(message: &str) -> Refusal {
    Refusal::new(RefusalCode::NonIdentifiableAttribution104, message)
}

fn transpose(values: &[f64], rows: usize, columns: usize) -> Vec<f64> {
    let mut result = vec![0.0; values.len()];
    for r in 0..rows {
        for c in 0..columns {
            result[c * rows + r] = values[r * columns + c];
        }
    }
    result
}

fn sum<const N: usize>(values: &[f64; N]) -> Result<f64> {
    dot(values, &[1.0; N])
}

// Refine the Schur equations using their already admitted factor. Forming
// C^-1(r+Au*u) can leave roundoff in the Au direction when a large inferred
// flow absorbs the observed contribution. Correct both u and lambda together;
// do not relax the stationarity audit or clamp small adjustments to zero.
fn refine_stationarity(
    au_columns: &[f64],
    inverse_c_au: &[f64],
    factor: &ScaledSpd,
    latent: &mut [f64],
    dual: &mut [f64],
) -> Result<()> {
    let m = dual.len();
    let p = latent.len();
    for _ in 0..2 {
        let residual = au_columns
            .chunks_exact(m)
            .map(|column| dot(column, dual))
            .collect::<Result<Vec<_>>>()?;
        if residual.iter().all(|v| *v == 0.0) {
            break;
        }
        let correction = factor
            .solve(&residual)?
            .into_iter()
            .map(|v| -v)
            .collect::<Vec<_>>();
        for (value, &change) in latent.iter_mut().zip(&correction) {
            *value = sum(&[*value, change])?;
        }
        for r in 0..m {
            dual[r] = sum(&[
                dual[r],
                dot(&inverse_c_au[r * p..(r + 1) * p], &correction)?,
            ])?;
        }
    }
    Ok(())
}

/// Reconcile n measured coordinates and p explicitly unmeasured coordinates.
/// V is n×n, Am is m×n, Au is m×p, and b has m entries, all row-major.
/// Bounds: 1<=p<=m<=min(n,64), n+p<=128. V and C=Am V Am' must be SPD.
/// Au must have full column rank at the supported numerical resolution;
/// otherwise FMINE-REF-104 refuses inference. The latter is not repaired with
/// a large variance, regularization or an arbitrary split between missing flows.
pub fn reconcile_with_unmeasured(
    observations: &[f64],
    covariance: &[f64],
    measured_constraints: &[f64],
    latent_constraints: &[f64],
    targets: &[f64],
    latent_count: usize,
) -> Result<PartialReconciliation> {
    let n = observations.len();
    let m = targets.len();
    let p = latent_count;
    if p > m {
        return Err(unobservable(
            "partial reconciliation: more unmeasured coordinates than independent balances",
        ));
    }
    if n == 0
        || p == 0
        || m == 0
        || n > MAX_OBSERVATIONS
        || p > MAX_OBSERVATIONS - n
        || m > MAX_CONSTRAINTS
        || m > n
        || covariance.len() != n * n
        || measured_constraints.len() != m * n
        || latent_constraints.len() != m * p
        || observations
            .iter()
            .chain(measured_constraints)
            .chain(latent_constraints)
            .chain(targets)
            .any(|v| !v.is_finite())
    {
        return Err(refuse(
            "partial reconciliation: invalid dimensions, work bound or nonfinite input",
        ));
    }
    let states = n + p;
    let measurement_factor = ScaledSpd::new(covariance, n)?;
    // Store Am*V by rows, so the covariance products need no per-cell allocation.
    let mut am_v = vec![0.0; m * n];
    for r in 0..m {
        for i in 0..n {
            am_v[r * n + i] = dot(
                &measured_constraints[r * n..(r + 1) * n],
                &covariance[i * n..(i + 1) * n],
            )?;
        }
    }
    let mut c = vec![0.0; m * m];
    for r in 0..m {
        for s in r..m {
            let value = dot(
                &measured_constraints[r * n..(r + 1) * n],
                &am_v[s * n..(s + 1) * n],
            )?;
            c[r * m + s] = value;
            c[s * m + r] = value;
        }
    }
    let balance_factor = ScaledSpd::new(&c, m)?;
    let am_columns = transpose(measured_constraints, m, n);
    let au_columns = transpose(latent_constraints, m, p);
    let va = transpose(&am_v, m, n);
    let mut inverse_c_au_columns = Vec::with_capacity(p * m);
    for column in au_columns.chunks_exact(m) {
        inverse_c_au_columns.extend(balance_factor.solve(column)?);
    }
    let inverse_c_au = transpose(&inverse_c_au_columns, p, m);
    let mut s = vec![0.0; p * p];
    for i in 0..p {
        for j in i..p {
            let value = dot(
                &au_columns[i * m..(i + 1) * m],
                &inverse_c_au_columns[j * m..(j + 1) * m],
            )?;
            s[i * p + j] = value;
            s[j * p + i] = value;
        }
    }
    let observability_factor = ScaledSpd::new(&s, p).map_err(|_| {
        unobservable(
            "partial reconciliation: unmeasured columns are not independently identifiable at the supported numerical resolution",
        )
    })?;
    let raw_observed_residuals = (0..m)
        .map(|r| {
            finite(
                dot(&measured_constraints[r * n..(r + 1) * n], observations)? - targets[r],
                "observed balance contribution",
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let inverse_c_residual = balance_factor.solve(&raw_observed_residuals)?;
    let latent_rhs = au_columns
        .chunks_exact(m)
        .map(|column| dot(column, &inverse_c_residual))
        .collect::<Result<Vec<_>>>()?;
    let mut latent_estimates = observability_factor
        .solve(&latent_rhs)?
        .into_iter()
        .map(|v| -v)
        .collect::<Vec<_>>();
    let degrees_of_freedom = m - p;
    // A square observable Au can absorb every balance. The measurements are
    // mathematically unchanged, rather than rounded numerical near-identities.
    let mut multipliers = if degrees_of_freedom == 0 {
        vec![0.0; m]
    } else {
        let remaining = (0..m)
            .map(|r| {
                sum(&[
                    raw_observed_residuals[r],
                    dot(&latent_constraints[r * p..(r + 1) * p], &latent_estimates)?,
                ])
            })
            .collect::<Result<Vec<_>>>()?;
        balance_factor.solve(&remaining)?
    };
    refine_stationarity(
        &au_columns,
        &inverse_c_au,
        &observability_factor,
        &mut latent_estimates,
        &mut multipliers,
    )?;
    let measured_adjustments = va
        .chunks_exact(m)
        .map(|row| dot(row, &multipliers).map(|v| -v))
        .collect::<Result<Vec<_>>>()?;
    let measured_estimates = observations
        .iter()
        .zip(&measured_adjustments)
        .map(|(&y, &delta)| finite(y + delta, "partially reconciled measurement"))
        .collect::<Result<Vec<_>>>()?;

    let mut estimator_map = vec![0.0; states * n];
    let mut adjustment_map = vec![0.0; n * n];
    for j in 0..n {
        let inverse_c_column = balance_factor.solve(&am_columns[j * m..(j + 1) * m])?;
        let rhs = au_columns
            .chunks_exact(m)
            .map(|column| dot(column, &inverse_c_column))
            .collect::<Result<Vec<_>>>()?;
        let mut inferred_column = observability_factor
            .solve(&rhs)?
            .into_iter()
            .map(|v| -v)
            .collect::<Vec<_>>();
        let mut dual_column = if degrees_of_freedom == 0 {
            vec![0.0; m]
        } else {
            (0..m)
                .map(|r| {
                    sum(&[
                        inverse_c_column[r],
                        dot(&inverse_c_au[r * p..(r + 1) * p], &inferred_column)?,
                    ])
                })
                .collect::<Result<Vec<_>>>()?
        };
        refine_stationarity(
            &au_columns,
            &inverse_c_au,
            &observability_factor,
            &mut inferred_column,
            &mut dual_column,
        )?;
        for i in 0..p {
            estimator_map[(n + i) * n + j] = inferred_column[i];
        }
        for i in 0..n {
            let gain = -dot(&va[i * m..(i + 1) * m], &dual_column)?;
            adjustment_map[i * n + j] = gain;
            estimator_map[i * n + j] = finite(
                (if i == j { 1.0 } else { 0.0 }) + gain,
                "partial reconciliation estimator map",
            )?;
        }
    }
    let posterior_covariance = covariance_congruence(covariance, &estimator_map, n, states)?;
    let adjustment_covariance = covariance_congruence(covariance, &adjustment_map, n, n)?;
    let weighted_adjustments = measurement_factor.solve(&measured_adjustments)?;
    let chi_square = dot(&measured_adjustments, &weighted_adjustments)?;
    if chi_square < 0.0 {
        return Err(refuse(
            "partial reconciliation: negative quadratic objective",
        ));
    }
    let mut joint_estimates = measured_estimates.clone();
    joint_estimates.extend_from_slice(&latent_estimates);
    let mut joint_constraints = Vec::with_capacity(m * states);
    for r in 0..m {
        joint_constraints.extend_from_slice(&measured_constraints[r * n..(r + 1) * n]);
        joint_constraints.extend_from_slice(&latent_constraints[r * p..(r + 1) * p]);
    }
    let posterior_sigmas = (0..states)
        .map(|i| posterior_covariance[i * states + i].sqrt())
        .collect::<Vec<_>>();
    let mut reconciled_residuals = Vec::with_capacity(m);
    for r in 0..m {
        let row = &joint_constraints[r * states..(r + 1) * states];
        let residual = finite(
            dot(row, &joint_estimates)? - targets[r],
            "joint balance residual",
        )?;
        let scale = row
            .iter()
            .zip(&joint_estimates)
            .fold(targets[r].abs(), |a, (&c, &x)| a + (c * x).abs());
        audit(residual, scale, "partially observed conservation")?;
        reconciled_residuals.push(residual);
        // True cross-covariances may be zero while both marginal uncertainties
        // are positive. Judge cancellation against retained posterior marginal
        // scales, not against the roundoff cross-terms themselves. Using the
        // original V here would conceal loss of a much smaller retained variance.
        let marginal_scale = dot(
            &row.iter().map(|v| v.abs()).collect::<Vec<_>>(),
            &posterior_sigmas,
        )?;
        for j in 0..states {
            let column = (0..states)
                .map(|i| posterior_covariance[i * states + j])
                .collect::<Vec<_>>();
            let scale = finite(
                marginal_scale * posterior_sigmas[j],
                "joint covariance backward-error scale",
            )?;
            if marginal_scale != 0.0 && posterior_sigmas[j] != 0.0 && scale == 0.0 {
                return Err(refuse(
                    "partial reconciliation: covariance audit scale loses range",
                ));
            }
            audit(dot(row, &column)?, scale, "joint constrained covariance")?;
        }
    }
    let mut stationarity_scale = weighted_adjustments
        .iter()
        .fold(0.0_f64, |a, x| a.max(x.abs()));
    for column in am_columns.chunks_exact(m).chain(au_columns.chunks_exact(m)) {
        stationarity_scale = stationarity_scale.max(
            column
                .iter()
                .zip(&multipliers)
                .fold(0.0, |a, (&c, &x)| a + (c * x).abs()),
        );
    }
    for (i, column) in am_columns.chunks_exact(m).enumerate() {
        audit(
            weighted_adjustments[i] + dot(column, &multipliers)?,
            stationarity_scale,
            "measured stationarity",
        )?;
    }
    for column in au_columns.chunks_exact(m) {
        audit(
            dot(column, &multipliers)?,
            stationarity_scale,
            "unmeasured stationarity",
        )?;
    }
    let mut normalized_adjustments = Vec::with_capacity(n);
    for i in 0..n {
        for j in 0..n {
            let posterior = posterior_covariance[i * states + j];
            let adjustment = adjustment_covariance[i * n + j];
            audit(
                sum(&[posterior, adjustment, -covariance[i * n + j]])?,
                posterior.abs() + adjustment.abs() + covariance[i * n + j].abs(),
                "measured posterior plus adjustment covariance",
            )?;
        }
        let variance = adjustment_covariance[i * n + i];
        normalized_adjustments.push(if variance == 0.0 {
            if measured_adjustments[i] != 0.0 {
                return Err(refuse(
                    "partial reconciliation: nonzero adjustment with zero variance",
                ));
            }
            None
        } else {
            Some(finite(
                measured_adjustments[i] / variance.sqrt(),
                "partial normalized adjustment",
            )?)
        });
    }
    Ok(PartialReconciliation {
        measured_estimates,
        measured_adjustments,
        latent_estimates,
        posterior_covariance,
        adjustment_covariance,
        raw_observed_residuals,
        reconciled_residuals,
        chi_square,
        degrees_of_freedom,
        normalized_adjustments,
        estimator_map,
    })
}

#[cfg(test)]
mod tests;
