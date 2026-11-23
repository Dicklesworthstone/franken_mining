//! DESIGN: nonlinear measurement likelihood with linear conserved quantities.
//! Adjusted coordinates are extensive states, never measured grades. Each
//! Gauss-Newton subproblem uses the analytic measurement Jacobian and the full
//! declared raw-error correlation. An l1 merit line search respects the physical
//! domain. No rank dropping, clipping or regularized fallback occurs.
//! Reference: Gill and Wong, Sequential quadratic programming methods, section 4,
//! https://www.ccom.ucsd.edu/~peg/papers/sqpReview.pdf
//! Determinism: DeterministicPerIsa

use super::{MAX_CONSTRAINTS, MAX_OBSERVATIONS, ScaledSpd, covariance_congruence, dot};
use fmine_evidence::{Refusal, RefusalCode};

type Result<T> = std::result::Result<T, Refusal>;
const KKT_TOLERANCE: f64 = 1e-9;
const MAX_BACKTRACKS: usize = 48;

/// Conditional stationary candidate, not a certified global nonlinear optimum.
/// Covariance is the local Gauss-Newton tangent approximation, not a nonlinear
/// posterior or a calibrated confidence region.
#[derive(Debug, Clone, PartialEq)]
pub struct NonlinearReconciliation {
    pub estimates: Vec<f64>,
    pub standardized_measurement_residuals: Vec<f64>,
    pub local_covariance: Vec<f64>,
    pub residuals: Vec<f64>,
    pub multipliers: Vec<f64>,
    /// Full squared Mahalanobis adjustment, without an exact chi-square claim.
    pub objective: f64,
    pub relative_stationarity: f64,
    pub relative_step: f64,
    pub iterations: usize,
    pub backtracks: usize,
}

/// Numerical coordinates only: the domain owner supplies units and sources.
pub struct NonlinearProblem<'a> {
    pub initial: &'a [f64],
    pub correlation: &'a [f64],
    pub observation_count: usize,
    pub constraints: &'a [f64],
    pub targets: &'a [f64],
    pub tolerances: &'a [f64],
    pub max_iterations: usize,
}

fn refuse(what: &str) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        format!("nonlinear reconciliation: {what}"),
    )
}

fn norm(values: &[f64]) -> f64 {
    values.iter().fold(0.0_f64, |a, b| a.max(b.abs()))
}

fn evaluate_checked(
    evaluate: &impl Fn(&[f64], &mut [f64], &mut [f64]) -> Result<()>,
    point: &[f64],
    observations: usize,
) -> Result<(Vec<f64>, Vec<f64>)> {
    let mut values = vec![f64::NAN; observations];
    let mut jacobian = vec![f64::NAN; observations * point.len()];
    evaluate(point, &mut values, &mut jacobian)?;
    if values.iter().chain(&jacobian).any(|v| !v.is_finite()) {
        return Err(refuse(
            "nonfinite or incomplete measurement residuals/Jacobian",
        ));
    }
    Ok((values, jacobian))
}

/// Minimize r(x)'R^-1 r(x) subject to the linear extensive balances A x=b.
/// The callback fills standardized raw residuals h(x)-y and their row-major
/// analytic Jacobian. Physical admissibility includes cross-coordinate bounds,
/// such as elemental mass <= dry mass; invalid trials are never evaluated.
/// At most 128 iterations and 48 halvings per step are allowed. Boundary optima
/// needing active inequality multipliers, unresolved rank and exhausted work
/// refuse. No active-set or global-optimum fallback is supplied.
pub fn reconcile_nonlinear(
    problem: NonlinearProblem<'_>,
    evaluate: impl Fn(&[f64], &mut [f64], &mut [f64]) -> Result<()>,
    admissible: impl Fn(&[f64]) -> bool,
) -> Result<NonlinearReconciliation> {
    let NonlinearProblem {
        initial,
        correlation,
        observation_count: p,
        constraints,
        targets,
        tolerances,
        max_iterations,
    } = problem;
    let n = initial.len();
    let m = targets.len();
    if !(1..=MAX_OBSERVATIONS).contains(&n)
        || !(n..=MAX_OBSERVATIONS).contains(&p)
        || !(1..=MAX_CONSTRAINTS.min(n)).contains(&m)
        || correlation.len() != p * p
        || constraints.len() != m * n
        || tolerances.len() != m
        || !(1..=128).contains(&max_iterations)
        || tolerances.iter().any(|t| !t.is_finite() || *t <= 0.0)
        || initial
            .iter()
            .chain(constraints)
            .chain(targets)
            .any(|x| !x.is_finite())
        || !admissible(initial)
        || (0..p).any(|i| correlation[i * p + i] != 1.0)
    {
        return Err(refuse(
            "invalid dimensions, physical initial state, tolerances, correlation or iteration budget",
        ));
    }
    let factor = ScaledSpd::new(correlation, p)?;
    let mut x = initial.to_vec();
    let mut penalty = 1.0_f64;
    let mut backtracks = 0;
    for iteration in 0..=max_iterations {
        let (measurement_residuals, jacobian) = evaluate_checked(&evaluate, &x, p)?;
        let weighted = factor.solve(&measurement_residuals)?;
        let objective = dot(&measurement_residuals, &weighted)?;
        let columns = (0..n)
            .map(|i| (0..p).map(|r| jacobian[r * n + i]).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let weighted_columns = columns
            .iter()
            .map(|c| factor.solve(c))
            .collect::<Result<Vec<_>>>()?;
        let gradient = columns
            .iter()
            .map(|c| dot(c, &weighted))
            .collect::<Result<Vec<_>>>()?;
        let mut hessian = vec![0.0; n * n];
        for i in 0..n {
            for j in i..n {
                let value = dot(&columns[i], &weighted_columns[j])?;
                hessian[i * n + j] = value;
                hessian[j * n + i] = value;
            }
        }
        let hessian_factor = ScaledSpd::new(&hessian, n)?;
        let unconstrained_step = hessian_factor.solve(&gradient)?;
        let proposal = x
            .iter()
            .zip(&unconstrained_step)
            .map(|(x, d)| x - d)
            .collect::<Vec<_>>();
        let mut covariance = vec![0.0; n * n];
        for j in 0..n {
            let mut unit = vec![0.0; n];
            unit[j] = 1.0;
            let column = hessian_factor.solve(&unit)?;
            for i in 0..n {
                covariance[i * n + j] = column[i];
            }
        }
        for i in 0..n {
            for j in i + 1..n {
                super::audit(
                    covariance[i * n + j] - covariance[j * n + i],
                    covariance[i * n + j].abs() + covariance[j * n + i].abs(),
                    "inverse measurement information symmetry",
                )?;
                let value = 0.5 * covariance[i * n + j] + 0.5 * covariance[j * n + i];
                covariance[i * n + j] = value;
                covariance[j * n + i] = value;
            }
        }
        let tangent = super::reconcile_linear(&proposal, &covariance, constraints, targets)?;
        let direction = tangent
            .estimates
            .iter()
            .zip(&x)
            .map(|(next, now)| next - now)
            .collect::<Vec<_>>();
        let normal = covariance_congruence(&covariance, constraints, n, m)?;
        let multipliers = ScaledSpd::new(&normal, m)?.solve(&tangent.raw_residuals)?;
        let dual = (0..n)
            .map(|i| {
                dot(
                    &(0..m).map(|r| constraints[r * n + i]).collect::<Vec<_>>(),
                    &multipliers,
                )
            })
            .collect::<Result<Vec<_>>>()?;

        // Coordinate-wise information scales prevent a large carrier mass from
        // hiding an unresolved trace-element step or stationarity component.
        let mut relative_stationarity = 0.0_f64;
        let mut relative_step = 0.0_f64;
        for i in 0..n {
            let information_scale = hessian[i * n + i].sqrt();
            let g = gradient[i] / information_scale;
            let d = dual[i] / information_scale;
            let step = direction[i] * information_scale;
            if !g.is_finite() || !d.is_finite() || !step.is_finite() {
                return Err(refuse("information-scaled convergence audit exceeds numerical range"));
            }
            let scale = 1.0_f64.max(g.abs()).max(d.abs());
            let error = (g / scale + d / scale).abs()
                / (1.0 / scale + (g / scale).abs() + (d / scale).abs());
            relative_stationarity = relative_stationarity.max(error);
            relative_step = relative_step.max(step.abs());
        }
        let residuals = (0..m)
            .map(|r| Ok(dot(&constraints[r * n..(r + 1) * n], &x)? - targets[r]))
            .collect::<Result<Vec<_>>>()?;
        if !objective.is_finite()
            || objective < 0.0
            || !relative_step.is_finite()
            || !relative_stationarity.is_finite()
        {
            return Err(refuse("unrepresentable objective or stationarity audit"));
        }
        if relative_stationarity <= KKT_TOLERANCE
            && relative_step <= KKT_TOLERANCE
            && residuals
                .iter()
                .zip(tolerances)
                .all(|(r, t)| r.abs() <= *t)
        {
            return Ok(NonlinearReconciliation {
                estimates: x,
                standardized_measurement_residuals: measurement_residuals,
                local_covariance: tangent.posterior_covariance,
                residuals,
                multipliers,
                objective,
                relative_stationarity,
                relative_step,
                iterations: iteration,
                backtracks,
            });
        }
        if iteration == max_iterations {
            return Err(refuse(
                "iteration budget exhausted before extensive closure and KKT convergence",
            ));
        }
        penalty = penalty.max(2.0 * norm(&multipliers) + 1.0);
        let violation = residuals.iter().map(|v| v.abs()).sum::<f64>();
        let merit = 0.5 * objective + penalty * violation;
        let slope = dot(&gradient, &direction)? - penalty * violation;
        if !merit.is_finite() || !slope.is_finite() || slope >= 0.0 {
            return Err(refuse(
                "no resolved descent direction for the nonlinear merit function",
            ));
        }
        let mut alpha = 1.0;
        let mut accepted = None;
        for _ in 0..MAX_BACKTRACKS {
            let candidate = x
                .iter()
                .zip(&direction)
                .map(|(x, d)| x + alpha * d)
                .collect::<Vec<_>>();
            if candidate.iter().all(|v| v.is_finite()) && admissible(&candidate) {
                let (values, _) = evaluate_checked(&evaluate, &candidate, p)?;
                let quadratic = dot(&values, &factor.solve(&values)?)?;
                let balance = (0..m)
                    .map(|r| {
                        Ok((dot(&constraints[r * n..(r + 1) * n], &candidate)? - targets[r]).abs())
                    })
                    .collect::<Result<Vec<_>>>()?;
                let next_merit = 0.5 * quadratic + penalty * balance.iter().sum::<f64>();
                if next_merit.is_finite() && next_merit <= merit + 1e-4 * alpha * slope {
                    accepted = Some(candidate);
                    break;
                }
            }
            backtracks += 1;
            alpha *= 0.5;
        }
        x = accepted.ok_or_else(|| {
            refuse("physical-domain merit line search exhausted; no values were clamped")
        })?;
    }
    Err(refuse("unreachable iteration state"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measurement(x: &[f64], r: &mut [f64], j: &mut [f64]) -> Result<()> {
        r[0] = x[0] * x[0] - 1.0;
        r[1] = x[1] * x[1] - 9.0;
        j.copy_from_slice(&[2.0 * x[0], 0.0, 0.0, 2.0 * x[1]]);
        Ok(())
    }

    #[test]
    fn nonlinear_likelihood_recovers_independent_known_answer_and_tangent_covariance() {
        // x=y>0: (x²-1)²+(x²-9)² = 2(x²-5)²+32.
        // Thus x=y=sqrt(5), objective 32. H=20I gives covariance 1/40 in every entry.
        let r = reconcile_nonlinear(
            NonlinearProblem {
                initial: &[1.0, 3.0],
                correlation: &[1.0, 0.0, 0.0, 1.0],
                observation_count: 2,
                constraints: &[1.0, -1.0],
                targets: &[0.0],
                tolerances: &[1e-10],
                max_iterations: 64,
            },
            measurement,
            |x| x.iter().all(|v| *v > 0.0),
        )
        .unwrap();
        for x in &r.estimates {
            assert!((x - 5.0_f64.sqrt()).abs() < 1e-9);
        }
        assert!((r.objective - 32.0).abs() < 1e-9);
        for actual in &r.local_covariance {
            assert!((actual - 0.025).abs() < 1e-10);
        }
        assert!(r.relative_stationarity < 1e-9 && r.relative_step < 1e-9);
        // Actual Lagrangian stationarity at the accepted point, independent of
        // the implementation's scaled convergence statistic.
        let g0 = 2.0 * r.estimates[0] * (r.estimates[0] * r.estimates[0] - 1.0);
        let g1 = 2.0 * r.estimates[1] * (r.estimates[1] * r.estimates[1] - 9.0);
        assert!((g0 + r.multipliers[0]).abs() < 1e-7);
        assert!((g1 - r.multipliers[0]).abs() < 1e-7);
        assert!(
            r.estimates.iter().chain(&r.local_covariance)
                .chain(&r.standardized_measurement_residuals)
                .chain(&r.residuals).chain(&r.multipliers)
                .all(|v| v.is_finite())
        );
        assert!(r.iterations > 1);
    }

    #[test]
    fn full_correlation_changes_joint_objective_and_row_scaling_preserves_fit() {
        let correlation = [1.0, 0.5, 0.5, 1.0];
        let run = |scale: f64| {
            reconcile_nonlinear(
                NonlinearProblem {
                    initial: &[1.0, 3.0],
                    correlation: &correlation,
                    observation_count: 2,
                    constraints: &[scale, -scale],
                    targets: &[0.0],
                    tolerances: &[1e-10 * scale],
                    max_iterations: 64,
                },
                measurement,
                |x| x.iter().all(|v| *v > 0.0),
            )
            .unwrap()
        };
        let a = run(1.0);
        let b = run(1e8);
        assert!((a.objective - 64.0).abs() < 1e-9);
        // H^-1 = R/20 at sqrt(5); on x=y the common variance is
        // (1 + rho)/40 = 3/80, derived directly from the joint likelihood.
        for covariance in a.local_covariance.iter().chain(&b.local_covariance) {
            assert!((covariance - 0.0375).abs() < 1e-10);
        }
        assert!((a.multipliers[0] - 1e8 * b.multipliers[0]).abs() < 1e-7);
        for (x, y) in a.estimates.iter().zip(&b.estimates) {
            assert!((x - y).abs() < 1e-9);
        }
    }

    #[test]
    fn infeasible_domain_singular_covariance_and_work_exhaustion_refuse() {
        let run = |budget, target| {
            reconcile_nonlinear(
                NonlinearProblem {
                    initial: &[1.0, 3.0],
                    correlation: &[1.0, 0.0, 0.0, 1.0],
                    observation_count: 2,
                    constraints: &[1.0, 1.0],
                    targets: &[target],
                    tolerances: &[1e-10],
                    max_iterations: budget,
                },
                measurement,
                |x| x.iter().all(|v| *v > 0.0),
            )
        };
        assert!(run(64, -1.0).is_err());
        assert!(run(1, 2.0).is_err());
        assert!(
            reconcile_nonlinear(
                NonlinearProblem {
                    initial: &[1.0, 3.0],
                    correlation: &[1.0, 1.0, 1.0, 1.0],
                    observation_count: 2,
                    constraints: &[1.0, -1.0],
                    targets: &[0.0],
                    tolerances: &[1e-10],
                    max_iterations: 64,
                },
                measurement,
                |_| true,
            )
            .is_err()
        );
    }

    #[test]
    fn active_inequality_boundary_requires_refusal_instead_of_clamping() {
        // The unconstrained equality optimum is (-1,2); the physical optimum
        // under x,y>=0, x+y=1 is (0,1). Its active inequality multiplier is
        // essential, so this equality-only kernel must not certify it.
        let result = reconcile_nonlinear(
            NonlinearProblem {
                initial: &[0.5, 0.5],
                correlation: &[1.0, 0.0, 0.0, 1.0],
                observation_count: 2,
                constraints: &[1.0, 1.0],
                targets: &[1.0],
                tolerances: &[1e-10],
                max_iterations: 64,
            },
            |x, r, j| {
                r.copy_from_slice(&[x[0] + 1.0, x[1] - 2.0]);
                j.copy_from_slice(&[1.0, 0.0, 0.0, 1.0]);
                Ok(())
            },
            |x| x.iter().all(|v| *v >= 0.0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn incomplete_or_nonfinite_measurement_callbacks_refuse() {
        let result = reconcile_nonlinear(
            NonlinearProblem {
                initial: &[1.0, 3.0],
                correlation: &[1.0, 0.0, 0.0, 1.0],
                observation_count: 2,
                constraints: &[1.0, -1.0],
                targets: &[0.0],
                tolerances: &[1e-10],
                max_iterations: 64,
            },
            |_, r, _| {
                r.copy_from_slice(&[0.0, f64::INFINITY]);
                Ok(())
            },
            |_| true,
        );
        assert!(result.is_err());
    }

    #[test]
    fn information_scaled_convergence_survives_twelve_orders_of_coordinate_scale() {
        let scales = [1e6, 1e-6];
        let r = reconcile_nonlinear(
            NonlinearProblem {
                initial: &[scales[0], 3.0 * scales[1]],
                correlation: &[1.0, 0.0, 0.0, 1.0],
                observation_count: 2,
                constraints: &[1.0 / scales[0], -1.0 / scales[1]],
                targets: &[0.0],
                tolerances: &[1e-10],
                max_iterations: 64,
            },
            |x, r, j| {
                let physical = [x[0] / scales[0], x[1] / scales[1]];
                measurement(&physical, r, j)?;
                j[0] /= scales[0];
                j[3] /= scales[1];
                Ok(())
            },
            |x| x.iter().all(|v| *v > 0.0),
        )
        .unwrap();
        for (x, s) in r.estimates.iter().zip(scales) {
            assert!((x / s - 5.0_f64.sqrt()).abs() < 1e-9);
        }
        assert!((r.objective - 32.0).abs() < 1e-9);
    }
}
