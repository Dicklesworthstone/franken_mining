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

struct KktEvaluation {
    measurement_residuals: Vec<f64>,
    objective: f64,
    proposal: Vec<f64>,
    covariance: Vec<f64>,
    residuals: Vec<f64>,
    multipliers: Vec<f64>,
    direction: Vec<f64>,
    quad_d: f64,
    relative_stationarity: f64,
    relative_step: f64,
}

#[allow(clippy::too_many_arguments)]
fn evaluate_kkt_from_raw(
    x: &[f64],
    measurement_residuals: Vec<f64>,
    jacobian: &[f64],
    factor: &ScaledSpd,
    constraints: &[f64],
    targets: &[f64],
    n: usize,
    m: usize,
    p: usize,
) -> Result<KktEvaluation> {
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
        .map(|(xi, d)| xi - d)
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
    // A true zero cross-covariance can leave ULP-level cancellation noise
    // in H^-1[i, j] when coupled Hessian blocks cancel analytically. Judge
    // symmetry against the marginal scale sqrt(H^-1[i,i] * H^-1[j,j]) (as
    // `partial.rs` does), not against a roundoff cross-term alone.
    for i in 0..n {
        let var_i = covariance[i * n + i];
        if !var_i.is_finite() || var_i <= 0.0 {
            return Err(refuse(
                "inverse measurement information has non-positive variance",
            ));
        }
        let sigma_i = var_i.sqrt();
        for j in i + 1..n {
            let var_j = covariance[j * n + j];
            if !var_j.is_finite() || var_j <= 0.0 {
                return Err(refuse(
                    "inverse measurement information has non-positive variance",
                ));
            }
            let marginal_scale = sigma_i * var_j.sqrt();
            super::audit(
                covariance[i * n + j] - covariance[j * n + i],
                (covariance[i * n + j].abs() + covariance[j * n + i].abs()).max(marginal_scale),
                "inverse measurement information symmetry",
            )?;
            let value = 0.5 * covariance[i * n + j] + 0.5 * covariance[j * n + i];
            covariance[i * n + j] = value;
            covariance[j * n + i] = value;
        }
    }
    let residuals = (0..m)
        .map(|r| Ok(dot(&constraints[r * n..(r + 1) * n], x)? - targets[r]))
        .collect::<Result<Vec<_>>>()?;
    // Solve the KKT step directly in delta coordinates without forming
    // (x - H^-1 g) - H^-1 A^T lam - x, preserving precision when ||d|| << ||x||.
    let raw_d = (0..m)
        .map(|r| Ok(residuals[r] - dot(&constraints[r * n..(r + 1) * n], &unconstrained_step)?))
        .collect::<Result<Vec<_>>>()?;
    let normal = covariance_congruence(&covariance, constraints, n, m)?;
    let normal_factor = ScaledSpd::new(&normal, m)?;
    let multipliers = normal_factor.solve(&raw_d)?;
    let dual = (0..n)
        .map(|i| {
            dot(
                &(0..m).map(|r| constraints[r * n + i]).collect::<Vec<_>>(),
                &multipliers,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let grad_plus_dual = gradient
        .iter()
        .zip(&dual)
        .map(|(g, d)| g + d)
        .collect::<Vec<_>>();
    let mut direction = hessian_factor
        .solve(&grad_plus_dual)?
        .into_iter()
        .map(|v| -v)
        .collect::<Vec<_>>();
    // One iterative feasibility projection ensures A(x + d) = b to machine precision.
    let feas_residual = (0..m)
        .map(|r| Ok(dot(&constraints[r * n..(r + 1) * n], &direction)? + residuals[r]))
        .collect::<Result<Vec<_>>>()?;
    let delta_lam = normal_factor.solve(&feas_residual)?;
    let corr_rhs = (0..n)
        .map(|i| {
            dot(
                &(0..m).map(|r| constraints[r * n + i]).collect::<Vec<_>>(),
                &delta_lam,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let corr = hessian_factor.solve(&corr_rhs)?;
    for (di, ci) in direction.iter_mut().zip(&corr) {
        *di -= *ci;
    }

    // Exact quadratic form d^T H d = (J d)^T R^-1 (J d) >= 0 avoids catastrophic
    // cancellation in g^T d when gradient and A^T lam cancel near the optimum.
    let jd = (0..p)
        .map(|r| dot(&jacobian[r * n..(r + 1) * n], &direction))
        .collect::<Result<Vec<_>>>()?;
    let quad_d = dot(&jd, &factor.solve(&jd)?)?;

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
            return Err(refuse(
                "information-scaled convergence audit exceeds numerical range",
            ));
        }
        let scale = 1.0_f64.max(g.abs()).max(d.abs());
        let error =
            (g / scale + d / scale).abs() / (1.0 / scale + (g / scale).abs() + (d / scale).abs());
        relative_stationarity = relative_stationarity.max(error);
        relative_step = relative_step.max(step.abs());
    }
    if !objective.is_finite()
        || objective < 0.0
        || !quad_d.is_finite()
        || quad_d < 0.0
        || !relative_step.is_finite()
        || !relative_stationarity.is_finite()
    {
        return Err(refuse("unrepresentable objective or stationarity audit"));
    }
    Ok(KktEvaluation {
        measurement_residuals,
        objective,
        proposal,
        covariance,
        residuals,
        multipliers,
        direction,
        quad_d,
        relative_stationarity,
        relative_step,
    })
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
    let (init_res, init_jac) = evaluate_checked(&evaluate, &x, p)?;
    let mut cur = evaluate_kkt_from_raw(
        &x,
        init_res,
        &init_jac,
        &factor,
        constraints,
        targets,
        n,
        m,
        p,
    )?;
    for iteration in 0..=max_iterations {
        let on_manifold = cur
            .residuals
            .iter()
            .zip(tolerances)
            .all(|(r, t)| r.abs() <= *t);
        if cur.relative_stationarity <= KKT_TOLERANCE
            && cur.relative_step <= KKT_TOLERANCE
            && on_manifold
        {
            let tangent =
                super::reconcile_linear(&cur.proposal, &cur.covariance, constraints, targets)?;
            return Ok(NonlinearReconciliation {
                estimates: x,
                standardized_measurement_residuals: cur.measurement_residuals,
                local_covariance: tangent.posterior_covariance,
                residuals: cur.residuals,
                multipliers: cur.multipliers,
                objective: cur.objective,
                relative_stationarity: cur.relative_stationarity,
                relative_step: cur.relative_step,
                iterations: iteration,
                backtracks,
            });
        }
        if iteration == max_iterations {
            return Err(refuse(
                "iteration budget exhausted before extensive closure and KKT convergence",
            ));
        }
        penalty = penalty.max(2.0 * norm(&cur.multipliers) + 1.0);
        let violation = cur.residuals.iter().map(|v| v.abs()).sum::<f64>();
        let merit = 0.5 * cur.objective + penalty * violation;
        // Exact l1 merit directional derivative:
        // g^T d - penalty * ||Ax - b||_1 = -d^T H d + lam^T (Ax - b) - penalty * ||Ax - b||_1.
        let slope = -cur.quad_d + dot(&cur.multipliers, &cur.residuals)? - penalty * violation;
        if !merit.is_finite() || !slope.is_finite() || slope >= 0.0 {
            return Err(refuse(
                "no resolved descent direction for the nonlinear merit function",
            ));
        }
        let cur_kkt = cur.relative_stationarity.max(cur.relative_step);
        let mut alpha = 1.0;
        let mut accepted: Option<(Vec<f64>, Option<KktEvaluation>)> = None;
        for bt in 0..MAX_BACKTRACKS {
            let candidate = x
                .iter()
                .zip(&cur.direction)
                .map(|(xi, d)| xi + alpha * d)
                .collect::<Vec<_>>();
            if candidate.iter().all(|v| v.is_finite()) && admissible(&candidate) {
                let (values, cand_jac) = evaluate_checked(&evaluate, &candidate, p)?;
                // Exact difference of squares (r_c - r)^T R^-1 (r_c + r) avoids
                // catastrophic subtraction when |delta_obj| << objective.
                let diff = values
                    .iter()
                    .zip(&cur.measurement_residuals)
                    .map(|(a, b)| a - b)
                    .collect::<Vec<_>>();
                let sum = values
                    .iter()
                    .zip(&cur.measurement_residuals)
                    .map(|(a, b)| a + b)
                    .collect::<Vec<_>>();
                let delta_obj = dot(&diff, &factor.solve(&sum)?)?;
                let next_residuals = (0..m)
                    .map(|r| Ok(dot(&constraints[r * n..(r + 1) * n], &candidate)? - targets[r]))
                    .collect::<Result<Vec<_>>>()?;
                let next_violation = next_residuals.iter().map(|v| v.abs()).sum::<f64>();
                let delta_merit = 0.5 * delta_obj + penalty * (next_violation - violation);
                if delta_merit.is_finite() && delta_merit <= 1e-4 * alpha * slope {
                    accepted = Some((candidate, None));
                    break;
                }
                // Near IEEE-754 f64 evaluation noise of 0.5 * r^T R^-1 r, the true
                // quadratic decrease -0.5 * d^T H d (~1e-16) of a full feasible
                // Gauss-Newton step can fall below roundoff in r(x). Accept alpha = 1
                // only when feasibility is maintained, objective does not increase
                // beyond roundoff, and the full KKT residual strictly contracts.
                let next_on_manifold = next_residuals
                    .iter()
                    .zip(tolerances)
                    .all(|(r, t)| r.abs() <= *t);
                if bt == 0
                    && on_manifold
                    && next_on_manifold
                    && cur_kkt <= 1e-4
                    && delta_obj.is_finite()
                    && 0.5 * delta_obj <= 1e-10 * (1.0 + cur.objective)
                    && let Ok(cand_kkt) = evaluate_kkt_from_raw(
                        &candidate,
                        values,
                        &cand_jac,
                        &factor,
                        constraints,
                        targets,
                        n,
                        m,
                        p,
                    )
                {
                    let next_kkt_metric =
                        cand_kkt.relative_stationarity.max(cand_kkt.relative_step);
                    if next_kkt_metric <= 0.5 * cur_kkt
                        || (cand_kkt.relative_stationarity <= KKT_TOLERANCE
                            && cand_kkt.relative_step <= KKT_TOLERANCE)
                    {
                        accepted = Some((candidate, Some(cand_kkt)));
                        break;
                    }
                }
            }
            backtracks += 1;
            alpha *= 0.5;
        }
        let (next_x, next_kkt) = accepted.ok_or_else(|| {
            refuse("physical-domain merit line search exhausted; no values were clamped")
        })?;
        x = next_x;
        cur = match next_kkt {
            Some(eval) => eval,
            None => {
                let (next_res, next_jac) = evaluate_checked(&evaluate, &x, p)?;
                evaluate_kkt_from_raw(
                    &x,
                    next_res,
                    &next_jac,
                    &factor,
                    constraints,
                    targets,
                    n,
                    m,
                    p,
                )?
            }
        };
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
            r.estimates
                .iter()
                .chain(&r.local_covariance)
                .chain(&r.standardized_measurement_residuals)
                .chain(&r.residuals)
                .chain(&r.multipliers)
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

    #[test]
    fn coupled_hessian_with_analytically_zero_cross_covariance_passes_symmetry_audit() {
        // Two-product concentrator structure with unweighed tails M_T = M_F - M_C:
        // H = J^T J couples M_C to M_F (via M_T) and M_F to m_F (via g_F = m_F / M_F),
        // while analytically H^-1[M_C, m_F] = 0. ULP cancellation in LDLT solves must
        // be audited against sqrt(H^-1[i,i] * H^-1[j,j]), not the ~1e-16 roundoff term.
        let r = reconcile_nonlinear(
            NonlinearProblem {
                initial: &[100_000.0, 3_000.0, 850.0, 765.0, 85.0],
                correlation: &[
                    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
                    0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
                ],
                observation_count: 5,
                constraints: &[0.0, 0.0, 1.0, -1.0, -1.0],
                targets: &[0.0],
                tolerances: &[1e-10],
                max_iterations: 64,
            },
            |x, r, j| {
                j.fill(0.0);
                let (mf, mc, emf, emc, emt) = (x[0], x[1], x[2], x[3], x[4]);
                let mt = mf - mc;
                let (smf, smc, sgf, sgc, sgt) = (1500.0, 15.0, 0.00017, 0.0013, 0.00002);
                r[0] = (mf - 100_000.0) / smf;
                j[0] = 1.0 / smf;
                r[1] = (mc - 3_000.0) / smc;
                j[5 + 1] = 1.0 / smc;
                r[2] = (emf / mf - 0.0085) / sgf;
                j[10] = -(emf / (mf * mf)) / sgf;
                j[10 + 2] = (1.0 / mf) / sgf;
                r[3] = (emc / mc - 0.255) / sgc;
                j[15 + 1] = -(emc / (mc * mc)) / sgc;
                j[15 + 3] = (1.0 / mc) / sgc;
                r[4] = (emt / mt - (85.0 / 97_000.0)) / sgt;
                j[20] = -(emt / (mt * mt)) / sgt;
                j[20 + 1] = (emt / (mt * mt)) / sgt;
                j[20 + 4] = (1.0 / mt) / sgt;
                Ok(())
            },
            |x| {
                let mt = x[0] - x[1];
                x[0] > 0.0 && x[1] > 0.0 && mt > 0.0 && x[2] > 0.0 && x[3] > 0.0 && x[4] > 0.0
            },
        )
        .expect("concentrator nonlinear balance converges");
        assert!(r.objective < 1e-18);
        assert!((r.estimates[0] - 100_000.0).abs() < 1e-6);
        assert!((r.estimates[2] - 850.0).abs() < 1e-9);
    }

    #[test]
    fn noisy_multi_element_bilinear_sqp_converges_past_objective_roundoff_floor() {
        // Two-node rougher-cleaner Cu (%) + Au (g/t) circuit with nonzero measurement
        // residuals (objective ~ O(5)). Near KKT convergence (rel_step ~ 1e-8), true
        // quadratic merit decrease -0.5 d^T H d (~1e-16) sits below f64 evaluation noise
        // in 0.5 ||r(x)||^2 (~1e-13); exact slope + KKT-contraction acceptance reaches
        // rel_stat <= 1e-9 and rel_step <= 1e-9 without weakening KKT_TOLERANCE.
        let ym = [1018.4, 39.82, 163.5];
        let sd_m = [15.0, 0.20, 3.2];
        let ycu = [0.00985, 0.0468, 0.00104, 0.2108, 0.00491];
        let sd_cu = [0.0002, 0.00092, 0.00003, 0.00105, 0.000125];
        let yau = [2.04e-6, 8.32e-6, 0.386e-6, 37.3e-6, 1.28e-6];
        let sd_au = [0.05e-6, 0.2125e-6, 0.01125e-6, 0.1875e-6, 0.03125e-6];
        // M_s = ME * [M_feed, M_final_conc, M_c_tail]
        let me = [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 1.0],
            [1.0, -1.0, -1.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let mut initial = vec![0.0; 13];
        initial[..3].copy_from_slice(&ym);
        let ms0: Vec<f64> = me
            .iter()
            .map(|row| row[0] * ym[0] + row[1] * ym[1] + row[2] * ym[2])
            .collect();
        for s in 0..5 {
            initial[3 + s] = ms0[s] * ycu[s];
            initial[8 + s] = ms0[s] * yau[s];
        }
        let mut constraints = vec![0.0; 4 * 13];
        let raw_rows = [
            (0usize, [3usize, 4, 5]),
            (1, [4, 6, 7]),
            (2, [8, 9, 10]),
            (3, [9, 11, 12]),
        ];
        for (r, cols) in raw_rows {
            let scale = initial[cols[0]].abs() + initial[cols[1]].abs() + initial[cols[2]].abs();
            constraints[r * 13 + cols[0]] = 1.0 / scale;
            constraints[r * 13 + cols[1]] = -1.0 / scale;
            constraints[r * 13 + cols[2]] = -1.0 / scale;
        }
        let mut correlation = vec![0.0; 13 * 13];
        for i in 0..13 {
            correlation[i * 13 + i] = 1.0;
        }
        let res = reconcile_nonlinear(
            NonlinearProblem {
                initial: &initial,
                correlation: &correlation,
                observation_count: 13,
                constraints: &constraints,
                targets: &[0.0; 4],
                tolerances: &[5e-11; 4],
                max_iterations: 64,
            },
            |x, r, j| {
                j.fill(0.0);
                for k in 0..3 {
                    r[k] = (x[k] - ym[k]) / sd_m[k];
                    j[k * 13 + k] = 1.0 / sd_m[k];
                }
                let ms: Vec<f64> = me
                    .iter()
                    .map(|row| row[0] * x[0] + row[1] * x[1] + row[2] * x[2])
                    .collect();
                for s in 0..5 {
                    let row = 3 + s;
                    r[row] = (x[row] / ms[s] - ycu[s]) / sd_cu[s];
                    let dg_dm = -(x[row] / (ms[s] * ms[s])) / sd_cu[s];
                    for k in 0..3 {
                        j[row * 13 + k] = dg_dm * me[s][k];
                    }
                    j[row * 13 + row] = (1.0 / ms[s]) / sd_cu[s];
                }
                for s in 0..5 {
                    let row = 8 + s;
                    r[row] = (x[row] / ms[s] - yau[s]) / sd_au[s];
                    let dg_dm = -(x[row] / (ms[s] * ms[s])) / sd_au[s];
                    for k in 0..3 {
                        j[row * 13 + k] = dg_dm * me[s][k];
                    }
                    j[row * 13 + row] = (1.0 / ms[s]) / sd_au[s];
                }
                Ok(())
            },
            |x| {
                for (s, row) in me.iter().enumerate() {
                    let m_s = row[0] * x[0] + row[1] * x[1] + row[2] * x[2];
                    if !(m_s > 0.0 && x[3 + s] > 0.0 && x[8 + s] > 0.0 && x[3 + s] + x[8 + s] < m_s)
                    {
                        return false;
                    }
                }
                true
            },
        )
        .expect("noisy multi-element rougher-cleaner SQP converges cleanly");
        assert!(res.relative_stationarity <= KKT_TOLERANCE);
        assert!(res.relative_step <= KKT_TOLERANCE);
        assert!(res.residuals.iter().all(|v| v.abs() <= 5e-11));
        assert!(res.objective > 0.5 && res.iterations <= 16);
    }
}
