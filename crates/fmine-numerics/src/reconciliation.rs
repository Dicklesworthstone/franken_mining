//! DESIGN: fully measured linear weighted-least-squares reconciliation (plan §20.3 R1).
//! For observations y, positive-definite covariance V and independent rows A,
//! x = y - V A' (A V A')^-1 (A y - b); cov(x) = V - V A' (A V A')^-1 A V.
//! Posterior covariance is evaluated as M V M', M = I - V A'(A V A')^-1 A,
//! rather than subtracting two nearly equal variances. Consistency and constrained
//! covariance audits refuse unresolved binary64 results; no entries are clipped.
//! The existing sparse LDLT owner supplies both reusable factorizations. Inputs
//! are numerical coordinates; a domain consumer must supply units, support,
//! measurement evidence and a gross-error policy. The complete-data entry point
//! estimates no missing observation, unknown bias, uncertain coefficient or
//! variance. The separate `partial` owner infers explicitly unmeasured variables
//! only when the retained measurements and constraints identify them.
//!
//! Reference: Tamhane and Mah (1985), Technometrics 27, 409–422, §§2–4,
//! <https://doi.org/10.1080/00401706.1985.10488080>. The chi-square interpretation
//! requires the declared unbiased Gaussian error model and correct covariance.
//! Individual normalized adjustments are correlated diagnostics, not fault
//! attribution or independent multiple-testing decisions.
//! Determinism: DeterministicPerIsa

use crate::sparse::{
    Coo,
    direct::{DirectOrdering, LdltFactor, LdltOptions, SymbolicLdlt},
};
use fmine_evidence::{Refusal, RefusalCode};

type Result<T> = std::result::Result<T, Refusal>;
pub const MAX_OBSERVATIONS: usize = 128;
pub const MAX_CONSTRAINTS: usize = 64;
const RELATIVE_AUDIT: f64 = 2.0e-10;

/// Observable unmeasured variables without fictitious observations or priors.
pub mod partial;

/// A complete numerical calculation; admission into a plant account belongs to its caller.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearReconciliation {
    pub estimates: Vec<f64>,
    /// Estimate minus original observation; no original observation is discarded.
    pub adjustments: Vec<f64>,
    /// Row-major full covariance of the reconciled estimates.
    pub posterior_covariance: Vec<f64>,
    /// Row-major covariance of the adjustments, V - cov(x).
    pub adjustment_covariance: Vec<f64>,
    pub raw_residuals: Vec<f64>,
    pub reconciled_residuals: Vec<f64>,
    /// (Ay-b)'(AVA')^-1(Ay-b), checked against adjustment' V^-1 adjustment.
    pub chi_square: f64,
    /// Independent balance rows; a rank-deficient declaration refuses.
    pub degrees_of_freedom: usize,
    /// Signed adjustment / its own standard deviation, not observation sigma.
    /// None means this variable is unaffected by the constraints.
    pub normalized_adjustments: Vec<Option<f64>>,
}

fn refuse(message: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::NumericalCertificateFailed207, message)
}
fn finite(value: f64, context: &str) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(refuse(format!(
            "linear reconciliation: nonfinite {context}"
        )))
    }
}
fn dot(a: &[f64], b: &[f64]) -> Result<f64> {
    let (mut sum, mut correction) = (0.0, 0.0);
    for (&x, &y) in a.iter().zip(b) {
        let term = finite(x * y, "product")?;
        if x != 0.0 && y != 0.0 && term == 0.0 {
            return Err(refuse("linear reconciliation: nonzero product underflows"));
        }
        let next = finite(sum + term, "sum")?;
        correction += if sum.abs() >= term.abs() {
            (sum - next) + term
        } else {
            (term - next) + sum
        };
        sum = next;
    }
    finite(sum + correction, "compensated sum")
}
fn audit(error: f64, scale: f64, context: &str) -> Result<()> {
    if !error.is_finite() || !scale.is_finite() || error.abs() > RELATIVE_AUDIT * scale {
        Err(refuse(format!(
            "linear reconciliation: {context} audit failed ({error}, scale {scale})"
        )))
    } else {
        Ok(())
    }
}

// Necessary covariance consistency checks after finite-precision propagation.
// They are not an independent PSD certificate: the covariance construction uses
// the originally admitted positive-definite V and one linear map on both sides.
fn audit_covariance(matrix: &[f64], n: usize, context: &str) -> Result<()> {
    for i in 0..n {
        let variance = matrix[i * n + i];
        if !variance.is_finite() || variance < 0.0 {
            return Err(refuse(format!(
                "linear reconciliation: {context} has invalid variance"
            )));
        }
    }
    for i in 0..n {
        for j in i + 1..n {
            let cross = matrix[i * n + j];
            let a = matrix[i * n + i].sqrt();
            let b = matrix[j * n + j].sqrt();
            if a == 0.0 || b == 0.0 {
                if cross != 0.0 {
                    return Err(refuse(format!(
                        "linear reconciliation: {context} has zero variance with nonzero covariance"
                    )));
                }
            } else {
                // Divide by the larger standard deviation first; forming a*b
                // can overflow even when the correlation is representable.
                let correlation = finite((cross / a.max(b)) / a.min(b), "correlation")?;
                if correlation.abs() > 1.0 + RELATIVE_AUDIT {
                    return Err(refuse(format!(
                        "linear reconciliation: {context} exceeds unit absolute correlation"
                    )));
                }
            }
        }
    }
    Ok(())
}

// Diagonal equilibration prevents a physical unit choice from becoming the
// factorization's pivot threshold. Neither a diagonal nugget nor rank dropping
// is permitted. The factor owner's inertia certifies positive definiteness.
struct ScaledSpd {
    factor: LdltFactor,
    scale: Vec<f64>,
}
impl ScaledSpd {
    fn new(matrix: &[f64], n: usize) -> Result<Self> {
        let mut scale = Vec::with_capacity(n);
        for i in 0..n {
            let diagonal = matrix[i * n + i];
            if !diagonal.is_finite() || diagonal <= 0.0 {
                return Err(refuse(
                    "linear reconciliation: covariance/constraint variance must be positive",
                ));
            }
            scale.push(diagonal.sqrt());
        }
        let mut coo = Coo::new(n, n).map_err(|e| refuse(e.to_string()))?;
        for i in 0..n {
            for j in 0..n {
                let entry = matrix[i * n + j];
                if !entry.is_finite() || entry != matrix[j * n + i] {
                    return Err(refuse(
                        "linear reconciliation: covariance must be finite and exactly symmetric",
                    ));
                }
                let normalized = finite((entry / scale[i]) / scale[j], "equilibrated covariance")?;
                if entry != 0.0 && normalized == 0.0 {
                    return Err(refuse(
                        "linear reconciliation: covariance scaling underflow",
                    ));
                }
                coo.push(i, j, normalized)
                    .map_err(|e| refuse(e.to_string()))?;
            }
        }
        let csr = coo.assemble();
        let symbolic = SymbolicLdlt::analyze(&csr, DirectOrdering::Natural)
            .map_err(|e| refuse(format!("linear reconciliation: {e}")))?;
        let factor = symbolic
            .factor(
                &csr,
                &LdltOptions {
                    max_factor_bytes: Some(16 * 1024 * 1024),
                    ..LdltOptions::default()
                },
            )
            .map_err(|e| refuse(format!("linear reconciliation: {e}")))?;
        if factor.inertia().positive != n || factor.inertia().negative != 0 {
            return Err(refuse(
                "linear reconciliation: covariance is not positive definite or constraints are dependent",
            ));
        }
        Ok(Self { factor, scale })
    }
    fn solve(&self, rhs: &[f64]) -> Result<Vec<f64>> {
        let normalized = rhs
            .iter()
            .zip(&self.scale)
            .map(|(&r, &s)| finite(r / s, "scaled right-hand side"))
            .collect::<Result<Vec<_>>>()?;
        let solution = self
            .factor
            .solve(&normalized)
            .map_err(|e| refuse(e.to_string()))?;
        solution
            .iter()
            .zip(&self.scale)
            .map(|(&v, &s)| finite(v / s, "solution"))
            .collect()
    }
}

/// Propagate a declared covariance through a fixed linear map: `J V J'`.
/// `covariance` is row-major input_count² and `jacobian` is row-major
/// output_count × input_count. Both counts must be in 1..=128. The original
/// covariance must be finite, exactly symmetric and positive definite under
/// the existing scaled LDLT admission; no independence or diagonal nugget is
/// supplied. The result may be singular and may contain genuine zero rows.
///
/// These are numerical coordinates. Units, source binding and any first-order
/// linearization of a nonlinear measurement belong to the domain caller. For
/// example, a dimensionless correlation matrix and a Jacobian with standard
/// deviations already incorporated produce the full output covariance. This
/// operation supplies no Gaussian assumption or calibration claim. Lost range,
/// lost positive variance and failed covariance consistency checks refuse.
pub fn propagate_linear_covariance(
    covariance: &[f64],
    jacobian: &[f64],
    input_count: usize,
    output_count: usize,
) -> Result<Vec<f64>> {
    if !(1..=MAX_OBSERVATIONS).contains(&input_count)
        || !(1..=MAX_OBSERVATIONS).contains(&output_count)
        || covariance.len() != input_count * input_count
        || jacobian.len() != output_count * input_count
        || jacobian.iter().any(|v| !v.is_finite())
    {
        return Err(refuse(
            "covariance propagation: invalid dimensions, work limit or nonfinite map",
        ));
    }
    // Reuse the same source-covariance admission as the WLS owner.
    ScaledSpd::new(covariance, input_count)?;
    covariance_congruence(covariance, jacobian, input_count, output_count)
}

// Both callers have already admitted V as SPD and supplied a finite map of
// the stated dimensions. WLS reuses its original admission and this arithmetic.
fn covariance_congruence(
    covariance: &[f64],
    jacobian: &[f64],
    input_count: usize,
    output_count: usize,
) -> Result<Vec<f64>> {
    let mut intermediate = vec![0.0; output_count * input_count];
    for i in 0..output_count {
        for j in 0..input_count {
            // The original covariance is exactly symmetric, so its jth row
            // is its jth column without copying or changing the source.
            intermediate[i * input_count + j] = dot(
                &jacobian[i * input_count..(i + 1) * input_count],
                &covariance[j * input_count..(j + 1) * input_count],
            )?;
        }
    }
    let mut result = vec![0.0; output_count * output_count];
    for i in 0..output_count {
        for j in i..output_count {
            let value = dot(
                &intermediate[i * input_count..(i + 1) * input_count],
                &jacobian[j * input_count..(j + 1) * input_count],
            )?;
            result[i * output_count + j] = value;
            result[j * output_count + i] = value;
        }
        // Positive-definite V gives positive variance for every nonzero row.
        // An unrepresentable variance must not be reported as certainty.
        if result[i * output_count + i] == 0.0
            && jacobian[i * input_count..(i + 1) * input_count]
                .iter()
                .any(|v| *v != 0.0)
        {
            return Err(refuse(
                "covariance propagation: nonzero map lost its positive variance",
            ));
        }
    }
    audit_covariance(&result, output_count, "propagated covariance")?;
    Ok(result)
}

/// Reconcile all observations against `A x = b`. `covariance` is row-major n×n,
/// `constraints` row-major m×n, n=observations.len(), m=targets.len().
/// The bounded complete-data problem needs 1..=128 observations and 1..=64
/// independent constraints. Singular covariance/constraints, nonfinite arithmetic
/// or failed conservation/stationarity/covariance audits refuse without a result.
pub fn reconcile_linear(
    observations: &[f64],
    covariance: &[f64],
    constraints: &[f64],
    targets: &[f64],
) -> Result<LinearReconciliation> {
    let n = observations.len();
    let m = targets.len();
    if n == 0
        || n > MAX_OBSERVATIONS
        || m == 0
        || m > MAX_CONSTRAINTS
        || m > n
        || covariance.len() != n * n
        || constraints.len() != m * n
        || observations
            .iter()
            .chain(constraints)
            .chain(targets)
            .any(|v| !v.is_finite())
    {
        return Err(refuse(
            "linear reconciliation: invalid dimensions, work limit or nonfinite input",
        ));
    }
    let measurement_factor = ScaledSpd::new(covariance, n)?;
    let mut va = vec![0.0; n * m];
    for i in 0..n {
        for r in 0..m {
            va[i * m + r] = dot(
                &covariance[i * n..(i + 1) * n],
                &constraints[r * n..(r + 1) * n],
            )?;
        }
    }
    let mut residual_covariance = vec![0.0; m * m];
    for r in 0..m {
        for s in r..m {
            let column: Vec<_> = (0..n).map(|i| va[i * m + s]).collect();
            let value = dot(&constraints[r * n..(r + 1) * n], &column)?;
            residual_covariance[r * m + s] = value;
            residual_covariance[s * m + r] = value;
        }
    }
    let balance_factor = ScaledSpd::new(&residual_covariance, m)?;
    let raw_residuals = (0..m)
        .map(|r| {
            finite(
                dot(&constraints[r * n..(r + 1) * n], observations)? - targets[r],
                "raw balance residual",
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let multipliers = balance_factor.solve(&raw_residuals)?;
    let adjustments = (0..n)
        .map(|i| dot(&va[i * m..(i + 1) * m], &multipliers).map(|v| -v))
        .collect::<Result<Vec<_>>>()?;
    let estimates = observations
        .iter()
        .zip(&adjustments)
        .map(|(&y, &delta)| finite(y + delta, "reconciled estimate"))
        .collect::<Result<Vec<_>>>()?;
    let mut reconciled_residuals = Vec::with_capacity(m);
    for r in 0..m {
        let row = &constraints[r * n..(r + 1) * n];
        let residual = finite(dot(row, &estimates)? - targets[r], "reconciled residual")?;
        let scale = row
            .iter()
            .zip(&estimates)
            .fold(targets[r].abs(), |s, (&a, &x)| s + (a * x).abs());
        audit(residual, scale, "balance closure")?;
        reconciled_residuals.push(residual);
    }
    let weighted_adjustment = measurement_factor.solve(&adjustments)?;
    // A component whose exact KKT term is zero still inherits roundoff from
    // the coupled solve. Use the vector infinity-norm backward-error scale,
    // rather than dividing that component's roundoff by itself.
    let mut stationarity_scale = weighted_adjustment
        .iter()
        .fold(0.0_f64, |s, x| s.max(x.abs()));
    for i in 0..n {
        let scale = (0..m).fold(0.0, |s, r| {
            s + (constraints[r * n + i] * multipliers[r]).abs()
        });
        stationarity_scale = stationarity_scale.max(scale);
    }
    for i in 0..n {
        let column: Vec<_> = (0..m).map(|r| constraints[r * n + i]).collect();
        let dual = dot(&column, &multipliers)?;
        audit(
            weighted_adjustment[i] + dual,
            stationarity_scale,
            "stationarity",
        )?;
    }
    let chi_square = dot(&raw_residuals, &multipliers)?;
    let objective = dot(&adjustments, &weighted_adjustment)?;
    if chi_square < 0.0 || objective < 0.0 {
        return Err(refuse(
            "linear reconciliation: negative quadratic objective",
        ));
    }
    audit(
        chi_square - objective,
        chi_square.abs() + objective.abs(),
        "objective",
    )?;
    let solved_rows = (0..n)
        .map(|i| balance_factor.solve(&va[i * m..(i + 1) * m]))
        .collect::<Result<Vec<_>>>()?;
    // solved_rows[i] is row i of K = V A' (A V A')^-1. Propagate the
    // observation errors with M=I-KA (Joseph form for exact constraints).
    // V - K A V can erase the entire retained variance of a well-constrained
    // coordinate when its original variance is much larger than the remainder.
    let mut propagation = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let column: Vec<_> = (0..m).map(|r| constraints[r * n + j]).collect();
            propagation[i * n + j] = finite(
                (if i == j { 1.0 } else { 0.0 }) - dot(&solved_rows[i], &column)?,
                "reconciliation propagation",
            )?;
        }
    }
    let posterior_covariance = covariance_congruence(covariance, &propagation, n, n)?;
    let mut adjustment_covariance = vec![0.0; n * n];
    for i in 0..n {
        for j in i..n {
            let reduction = dot(&va[i * m..(i + 1) * m], &solved_rows[j])?;
            let posterior = posterior_covariance[i * n + j];
            adjustment_covariance[i * n + j] = reduction;
            adjustment_covariance[j * n + i] = reduction;
            audit(
                dot(&[posterior, reduction, -covariance[i * n + j]], &[1.0; 3])?,
                posterior.abs() + reduction.abs() + covariance[i * n + j].abs(),
                "posterior plus adjustment covariance",
            )?;
        }
    }
    audit_covariance(&adjustment_covariance, n, "adjustment covariance")?;
    let mut normalized_adjustments = Vec::with_capacity(n);
    for i in 0..n {
        let variance = adjustment_covariance[i * n + i];
        // Positive-definite source matrices give positive quadratic variance
        // for every nonzero row. A zero caused by range loss is not certainty.
        if variance == 0.0 && va[i * m..(i + 1) * m].iter().any(|v| *v != 0.0) {
            return Err(refuse(
                "linear reconciliation: nonzero covariance propagation lost its variance",
            ));
        }
        normalized_adjustments.push(if variance == 0.0 {
            if adjustments[i] != 0.0 {
                return Err(refuse(
                    "linear reconciliation: adjustment has zero variance",
                ));
            }
            None
        } else {
            Some(finite(
                adjustments[i] / variance.sqrt(),
                "normalized adjustment",
            )?)
        });
    }
    for r in 0..m {
        for j in 0..n {
            let row = &constraints[r * n..(r + 1) * n];
            let column: Vec<_> = (0..n).map(|i| posterior_covariance[i * n + j]).collect();
            let value = dot(row, &column)?;
            // Judge A*cov(x)=0 against its own terms. Scaling against the
            // original V can conceal an invalid tiny posterior under a huge V.
            let mut scale = 0.0;
            for (&a, &p) in row.iter().zip(&column) {
                let product = finite(a * p, "constrained covariance product")?;
                if a != 0.0 && p != 0.0 && product == 0.0 {
                    return Err(refuse(
                        "linear reconciliation: constrained covariance audit underflows",
                    ));
                }
                scale += product.abs();
            }
            audit(value, scale, "constrained covariance")?;
        }
    }
    Ok(LinearReconciliation {
        estimates,
        adjustments,
        posterior_covariance,
        adjustment_covariance,
        raw_residuals,
        reconciled_residuals,
        chi_square,
        degrees_of_freedom: m,
        normalized_adjustments,
    })
}

#[cfg(test)]
mod tests;
