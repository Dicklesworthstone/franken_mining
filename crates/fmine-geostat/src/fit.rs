//! Weighted least-squares fit of a nested variogram model (`BD-237`).
//!
//! Determinism: BitwisePortable (`+ − × ÷` and `sqrt`; `exp` and `ln` through `fmine_core::det`; the small dense
//! solves are `fmine_numerics::cholesky_solve`; every step runs in a fixed order).
//!
//! The objective is S = Σ w_j (γ̂_j − γ(h_j))² over every lag j of every experimental variogram, where h_j is the
//! lag vector of the lag's mean distance along its direction. The weight is w_j = N_j ([`Weights::Pairs`], gstat's
//! `fit.method = 1`) or N_j / |h_j|² ([`Weights::PairsOverDistanceSquared`], gstat's default `fit.method = 7`),
//! scaled so that the weights average 1 (which changes no minimiser).
//!
//! For fixed ranges γ is linear in the nugget and sills, so these are found exactly under the constraint that none
//! is negative. The solution is the best, over every subset of the coefficients, of the unconstrained least-squares
//! solution on that subset whose coefficients are all positive; the empty subset, all zero, is a candidate. The
//! free ranges are then found by Nelder–Mead over their logarithms, which is restarted from its best vertex until a
//! restart no longer improves it.
//!
//! The fit refuses (`FMINE-REF-202`) what the lags cannot determine:
//! - fewer lags than parameters;
//! - a free range of an anisotropic structure that the directions do not separate from its other free ranges. Along
//!   direction u a structure's scaled distance is |h| √(Σ_a (e_a·u)² / r_a²) over its axes e_a and ranges r_a, so
//!   each direction shows one combination of the 1/r_a²; the columns (e_a·u)² of the free axes must be independent;
//! - a fitted range at the edge of the search, a thousandth of the shortest lag or a thousand times the longest, on
//!   a structure with a positive sill.
//!
//! A structure the data do not need gets sill 0; its ranges then carry no information and are reported as found.

use fmine_core::det;
use fmine_numerics::cholesky_solve;

use crate::GeostatError;
use crate::experimental::Experimental;
use crate::model::{self, MAX_STRUCTURES, Model, Ranges, Rotation, Shape, Structure};

/// Most objective evaluations one fit may spend.
pub const MAX_EVALUATIONS: u32 = 200_000;

/// A fitted range lies between the shortest lag over this and the longest times it.
pub const RANGE_REACH: f64 = 1000.0;

const MAX_RESTARTS: u32 = 20;
/// Initial simplex step in log range (a factor of e^0.5 ≈ 1.65), and the step of each restart.
const INITIAL_STEP: f64 = 0.5;
const RESTART_STEP: f64 = 0.1;
/// Convergence: the simplex is this small in log range, and its objective spread this small relative to the
/// objective of the zero model.
const X_TOLERANCE: f64 = 1e-10;
const F_TOLERANCE: f64 = 1e-14;
/// A range within this factor of a search bound is at its edge.
const EDGE: f64 = 1.01;
/// A free axis's direction column whose part independent of the others is shorter than this is not determined.
const RANK_TOLERANCE: f64 = 1e-6;

const AXES: [&str; 3] = ["major", "semi-major", "minor"];

/// How each lag is weighted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weights {
    /// By its pair count.
    Pairs,
    /// By its pair count over its squared distance, which favours short lags.
    PairsOverDistanceSquared,
}

/// A range, m: fixed, or free from an initial value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Range {
    /// Held at this value.
    Fixed(f64),
    /// Fitted, starting here.
    Free(f64),
}

impl Range {
    const fn value(self) -> f64 {
        match self {
            Self::Fixed(v) | Self::Free(v) => v,
        }
    }
}

/// A structure's ranges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RangeSpec {
    /// One range on every axis.
    Isotropic(Range),
    /// A range per axis of a declared rotation.
    Anisotropic {
        /// The structure's orientation, held fixed.
        rotation: Rotation,
        /// Along the major axis.
        major: Range,
        /// Along the semi-major axis.
        semi: Range,
        /// Along the minor axis.
        minor: Range,
    },
}

/// One structure to fit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StructureSpec {
    /// Its shape.
    pub shape: Shape,
    /// Its ranges.
    pub ranges: RangeSpec,
}

/// The model to fit.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    /// Whether it has a nugget.
    pub nugget: bool,
    /// Its structures, at most [`MAX_STRUCTURES`].
    pub structures: Vec<StructureSpec>,
    /// The lag weights.
    pub weights: Weights,
}

/// A fitted model.
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
    /// The model.
    pub model: Model,
    /// The weighted sum of squares it leaves, with the weights averaging 1.
    pub objective: f64,
    /// The lags it was fitted to.
    pub points: usize,
    /// Objective evaluations spent.
    pub evaluations: u32,
    /// Restarts of the range search.
    pub restarts: u32,
}

struct Point {
    lag: [f64; 3],
    gamma: f64,
    weight: f64,
}

/// A free range: its structure, and its axis (`None`: every axis of an isotropic structure).
#[derive(Clone, Copy)]
struct Free {
    structure: usize,
    axis: Option<usize>,
}

struct Problem<'a> {
    spec: &'a Spec,
    points: Vec<Point>,
    rotations: Vec<Rotation>,
    /// Each structure's ranges: the fixed values and the initial free ones.
    base: Vec<[f64; 3]>,
    free: Vec<Free>,
    lo: f64,
    hi: f64,
    /// The objective of the zero model.
    scale: f64,
}

impl Problem<'_> {
    /// The ranges at log ranges `x`; `None` when one is outside the search.
    fn ranges(&self, x: &[f64]) -> Option<Vec<[f64; 3]>> {
        let mut ranges = self.base.clone();
        for (f, xi) in self.free.iter().zip(x) {
            let v = det::exp(*xi);
            if !(v >= self.lo && v <= self.hi) {
                return None;
            }
            match f.axis {
                Some(a) => ranges[f.structure][a] = v,
                None => ranges[f.structure] = [v; 3],
            }
        }
        Some(ranges)
    }

    /// The nugget column (when fitted), then each structure's unit-sill values at the lags.
    fn columns(&self, ranges: &[[f64; 3]]) -> Vec<Vec<f64>> {
        let mut cols = Vec::with_capacity(self.spec.structures.len() + 1);
        if self.spec.nugget {
            cols.push(vec![1.0; self.points.len()]);
        }
        for ((s, rotation), r) in self.spec.structures.iter().zip(&self.rotations).zip(ranges) {
            let frame = model::frame(
                rotation,
                &Ranges {
                    major_m: r[0],
                    semi_m: r[1],
                    minor_m: r[2],
                },
            );
            cols.push(
                self.points
                    .iter()
                    .map(|p| s.shape.at(model::scaled(&frame, p.lag)))
                    .collect(),
            );
        }
        cols
    }

    fn objective(&self, x: &[f64]) -> f64 {
        self.ranges(x)
            .map_or(f64::INFINITY, |r| nnls(&self.columns(&r), &self.points).1)
    }
}

/// The non-negative least-squares coefficients of `cols` against the points' γ, and the objective they leave.
fn nnls(cols: &[Vec<f64>], points: &[Point]) -> (Vec<f64>, f64) {
    let m = cols.len();
    let mut gram = vec![0.0; m * m];
    let mut rhs = vec![0.0; m];
    for a in 0..m {
        for b in 0..m {
            gram[a * m + b] = points
                .iter()
                .enumerate()
                .map(|(j, p)| p.weight * cols[a][j] * cols[b][j])
                .sum();
        }
        rhs[a] = points
            .iter()
            .enumerate()
            .map(|(j, p)| p.weight * cols[a][j] * p.gamma)
            .sum();
    }
    let objective = |c: &[f64]| -> f64 {
        points
            .iter()
            .enumerate()
            .map(|(j, p)| {
                let fit: f64 = cols.iter().zip(c).map(|(col, ci)| ci * col[j]).sum();
                let r = p.gamma - fit;
                p.weight * r * r
            })
            .sum()
    };
    let zero = vec![0.0; m];
    let mut best = (zero.clone(), objective(&zero));
    for mask in 1_u32..(1 << m) {
        let on: Vec<usize> = (0..m).filter(|i| mask & (1 << i) != 0).collect();
        let k = on.len();
        let mut g = vec![0.0; k * k];
        let mut b = vec![0.0; k];
        for (a, &ia) in on.iter().enumerate() {
            for (c, &ic) in on.iter().enumerate() {
                g[a * k + c] = gram[ia * m + ic];
            }
            b[a] = rhs[ia];
        }
        // A singular subset: some other subset spans the same fit.
        let Ok(solution) = cholesky_solve(&g, &b, k) else {
            continue;
        };
        if !solution.iter().all(|v| *v > 0.0) {
            continue;
        }
        let mut c = zero.clone();
        for (&i, v) in on.iter().zip(&solution) {
            c[i] = *v;
        }
        let s = objective(&c);
        if s < best.1 {
            best = (c, s);
        }
    }
    best
}

struct Search<'a, 'b> {
    problem: &'a Problem<'b>,
    evaluations: u32,
}

impl Search<'_, '_> {
    fn eval(&mut self, x: &[f64]) -> Result<f64, GeostatError> {
        if self.evaluations >= MAX_EVALUATIONS {
            return Err(GeostatError::NoConvergence {
                detail: format!(
                    "the range search did not converge in {MAX_EVALUATIONS} evaluations"
                ),
            });
        }
        self.evaluations += 1;
        Ok(self.problem.objective(x))
    }

    /// Nelder–Mead from `x0` with a simplex of side `step`: its best vertex once the simplex has collapsed.
    fn minimize(&mut self, x0: &[f64], step: f64) -> Result<(Vec<f64>, f64), GeostatError> {
        let n = x0.len();
        let mut simplex: Vec<(Vec<f64>, f64)> = Vec::with_capacity(n + 1);
        let f0 = self.eval(x0)?;
        simplex.push((x0.to_vec(), f0));
        for i in 0..n {
            let mut x = x0.to_vec();
            x[i] += step;
            let f = self.eval(&x)?;
            simplex.push((x, f));
        }
        let scale = self.problem.scale;
        loop {
            // Stable: ties keep their order.
            simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
            let (best, worst) = (simplex[0].1, simplex[n].1);
            let size = simplex[1..]
                .iter()
                .flat_map(|(x, _)| x.iter().zip(&simplex[0].0).map(|(a, b)| (a - b).abs()))
                .fold(0.0, f64::max);
            if worst - best <= F_TOLERANCE * (best.abs() + scale) && size <= X_TOLERANCE {
                return Ok(simplex.swap_remove(0));
            }
            let mut centroid = vec![0.0; n];
            for (x, _) in &simplex[..n] {
                for (c, xi) in centroid.iter_mut().zip(x) {
                    *c += xi;
                }
            }
            for c in &mut centroid {
                *c /= n as f64;
            }
            let far = simplex[n].0.clone();
            let along = |t: f64| -> Vec<f64> {
                centroid
                    .iter()
                    .zip(&far)
                    .map(|(c, w)| c + t * (c - w))
                    .collect()
            };
            let xr = along(1.0);
            let fr = self.eval(&xr)?;
            if fr < best {
                let xe = along(2.0);
                let fe = self.eval(&xe)?;
                simplex[n] = if fe < fr { (xe, fe) } else { (xr, fr) };
                continue;
            }
            if fr < simplex[n - 1].1 {
                simplex[n] = (xr, fr);
                continue;
            }
            let (xc, fc, accept) = if fr < worst {
                let xc = along(0.5);
                let fc = self.eval(&xc)?;
                (xc, fc, fc <= fr)
            } else {
                let xc = along(-0.5);
                let fc = self.eval(&xc)?;
                (xc, fc, fc < worst)
            };
            if accept {
                simplex[n] = (xc, fc);
                continue;
            }
            let keep = simplex[0].0.clone();
            for v in simplex.iter_mut().skip(1) {
                let x: Vec<f64> = keep
                    .iter()
                    .zip(&v.0)
                    .map(|(b, xi)| b + 0.5 * (xi - b))
                    .collect();
                let f = self.eval(&x)?;
                *v = (x, f);
            }
        }
    }
}

/// Fit `spec` to `variograms`.
///
/// # Errors
/// - [`GeostatError::Invalid`]: no nugget and no structures, more than [`MAX_STRUCTURES`], a range that is not
///   positive and finite or a free one outside the search, a lag with no pairs or a non-finite value, or an
///   anisotropic structure fitted to an omnidirectional variogram.
/// - [`GeostatError::NotObservable`]: what the lags cannot determine (module documentation).
/// - [`GeostatError::NoConvergence`]: the range search did not converge in [`MAX_EVALUATIONS`] evaluations or
///   was still improving after its restarts.
pub fn fit(variograms: &[Experimental], spec: &Spec) -> Result<Fit, GeostatError> {
    let problem = problem(variograms, spec)?;
    let x0: Vec<f64> = problem
        .free
        .iter()
        .map(|f| det::ln(problem.base[f.structure][f.axis.unwrap_or(0)]))
        .collect();
    let mut search = Search {
        problem: &problem,
        evaluations: 0,
    };
    let (mut x, mut f) = if x0.is_empty() {
        (x0, 0.0)
    } else {
        search.minimize(&x0, INITIAL_STEP)?
    };
    let mut restarts = 0;
    while !x.is_empty() {
        if restarts == MAX_RESTARTS {
            return Err(GeostatError::NoConvergence {
                detail: format!(
                    "the range search was still improving after {MAX_RESTARTS} restarts"
                ),
            });
        }
        restarts += 1;
        let (xr, fr) = search.minimize(&x, RESTART_STEP)?;
        let improved = f - fr > F_TOLERANCE * (f.abs() + problem.scale);
        if fr < f {
            (x, f) = (xr, fr);
        }
        if !improved {
            break;
        }
    }
    let ranges = problem
        .ranges(&x)
        .ok_or_else(|| GeostatError::NoConvergence {
            detail: "the range search left its bounds".to_string(),
        })?;
    let (sills, objective) = nnls(&problem.columns(&ranges), &problem.points);
    let offset = usize::from(spec.nugget);
    for free in &problem.free {
        let sill = sills[offset + free.structure];
        let v = ranges[free.structure][free.axis.unwrap_or(0)];
        if sill > 0.0 && (v <= problem.lo * EDGE || v * EDGE >= problem.hi) {
            return Err(GeostatError::not_observable(format!(
                "structure {}'s {} range runs to the edge of the search ({v} m, against {} m to {} m)",
                free.structure,
                free.axis.map_or("isotropic", |a| AXES[a]),
                problem.lo,
                problem.hi
            )));
        }
    }
    let structures = spec
        .structures
        .iter()
        .zip(&problem.rotations)
        .zip(&ranges)
        .zip(&sills[offset..])
        .map(|(((s, rotation), r), sill)| Structure {
            shape: s.shape,
            sill: *sill,
            ranges: Ranges {
                major_m: r[0],
                semi_m: r[1],
                minor_m: r[2],
            },
            rotation: *rotation,
        })
        .collect();
    let nugget = if spec.nugget { sills[0] } else { 0.0 };
    Ok(Fit {
        model: Model::new(nugget, structures)?,
        objective,
        points: problem.points.len(),
        evaluations: search.evaluations,
        restarts,
    })
}

fn problem<'a>(variograms: &[Experimental], spec: &'a Spec) -> Result<Problem<'a>, GeostatError> {
    if !spec.nugget && spec.structures.is_empty() {
        return Err(GeostatError::invalid("spec", "no nugget and no structures"));
    }
    if spec.structures.len() > MAX_STRUCTURES {
        return Err(GeostatError::invalid(
            "spec.structures",
            format!(
                "{} structures; at most {MAX_STRUCTURES}",
                spec.structures.len()
            ),
        ));
    }
    let mut points = Vec::new();
    let mut directions: Vec<[f64; 3]> = Vec::new();
    let mut omni = false;
    for (v, e) in variograms.iter().enumerate() {
        if e.lags.is_empty() {
            continue;
        }
        let u = e.direction.frame()[0];
        omni |= e.direction.is_omnidirectional();
        directions.push(u);
        for (l, lag) in e.lags.iter().enumerate() {
            let ok = lag.pairs > 0
                && lag.mean_distance_m.is_finite()
                && lag.mean_distance_m > 0.0
                && lag.gamma.is_finite()
                && lag.gamma >= 0.0;
            if !ok {
                return Err(GeostatError::invalid(
                    format!("variograms[{v}].lags[{l}]"),
                    format!(
                        "{} pairs, distance {} m, γ {}",
                        lag.pairs, lag.mean_distance_m, lag.gamma
                    ),
                ));
            }
            let h = lag.mean_distance_m;
            let n = lag.pairs as f64;
            points.push(Point {
                lag: [u[0] * h, u[1] * h, u[2] * h],
                gamma: lag.gamma,
                weight: match spec.weights {
                    Weights::Pairs => n,
                    Weights::PairsOverDistanceSquared => n / (h * h),
                },
            });
        }
    }
    let total: f64 = points.iter().map(|p| p.weight).sum();
    let mean = total / points.len().max(1) as f64;
    for p in &mut points {
        p.weight /= mean;
    }
    let (shortest, longest) = points.iter().fold((f64::INFINITY, 0.0_f64), |(lo, hi), p| {
        let h = det::sqrt(p.lag[0] * p.lag[0] + p.lag[1] * p.lag[1] + p.lag[2] * p.lag[2]);
        (lo.min(h), hi.max(h))
    });
    let (lo, hi) = (shortest / RANGE_REACH, longest * RANGE_REACH);

    let mut rotations = Vec::with_capacity(spec.structures.len());
    let mut base = Vec::with_capacity(spec.structures.len());
    let mut free = Vec::new();
    for (k, s) in spec.structures.iter().enumerate() {
        let field = format!("spec.structures[{k}]");
        let (rotation, axes): (Rotation, Vec<(Option<usize>, Range)>) = match s.ranges {
            RangeSpec::Isotropic(r) => (Rotation::NONE, vec![(None, r)]),
            RangeSpec::Anisotropic {
                rotation,
                major,
                semi,
                minor,
            } => {
                if omni {
                    return Err(GeostatError::invalid(
                        &field,
                        "an anisotropic structure fitted to an omnidirectional variogram, which has no direction",
                    ));
                }
                (
                    rotation,
                    vec![(Some(0), major), (Some(1), semi), (Some(2), minor)],
                )
            }
        };
        let mut r3 = [0.0; 3];
        for (axis, range) in &axes {
            let v = range.value();
            let name = axis.map_or("isotropic", |a| AXES[a]);
            if !(v.is_finite() && v > 0.0) {
                return Err(GeostatError::invalid(
                    format!("{field}.{name}"),
                    format!("{v} is not a positive, finite range"),
                ));
            }
            match axis {
                Some(a) => r3[*a] = v,
                None => r3 = [v; 3],
            }
            if let Range::Free(_) = range {
                if !(v > lo && v < hi) {
                    return Err(GeostatError::invalid(
                        format!("{field}.{name}"),
                        format!("initial range {v} m is outside the search, {lo} m to {hi} m"),
                    ));
                }
                free.push(Free {
                    structure: k,
                    axis: *axis,
                });
            }
        }
        if axes.len() == 3 {
            identifiable(k, &rotation, &axes, &directions)?;
        }
        rotations.push(rotation);
        base.push(r3);
    }
    let parameters = usize::from(spec.nugget) + spec.structures.len() + free.len();
    if points.len() < parameters {
        return Err(GeostatError::not_observable(format!(
            "{} lags for {parameters} parameters",
            points.len()
        )));
    }
    let scale = points.iter().map(|p| p.weight * p.gamma * p.gamma).sum();
    Ok(Problem {
        spec,
        points,
        rotations,
        base,
        free,
        lo,
        hi,
        scale,
    })
}

/// Refuse a free axis of structure `k` that `directions` do not separate from its other free axes.
fn identifiable(
    k: usize,
    rotation: &Rotation,
    axes: &[(Option<usize>, Range)],
    directions: &[[f64; 3]],
) -> Result<(), GeostatError> {
    let frame = rotation.axes();
    let mut basis: Vec<Vec<f64>> = Vec::new();
    for (axis, range) in axes {
        let (Some(a), Range::Free(_)) = (axis, range) else {
            continue;
        };
        let mut column: Vec<f64> = directions
            .iter()
            .map(|u| {
                let c = frame[*a][0] * u[0] + frame[*a][1] * u[1] + frame[*a][2] * u[2];
                c * c
            })
            .collect();
        for q in &basis {
            let dot: f64 = q.iter().zip(&column).map(|(x, y)| x * y).sum();
            for (c, qi) in column.iter_mut().zip(q) {
                *c -= dot * qi;
            }
        }
        let norm = det::sqrt(column.iter().map(|c| c * c).sum());
        if norm < RANK_TOLERANCE {
            return Err(GeostatError::not_observable(format!(
                "the directions do not determine structure {k}'s {} range: fix it, or add a direction along it",
                AXES[*a]
            )));
        }
        basis.push(column.iter().map(|c| c / norm).collect());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(gamma: f64) -> Point {
        Point {
            lag: [1.0, 0.0, 0.0],
            gamma,
            weight: 1.0,
        }
    }

    #[test]
    fn test_nnls_is_least_squares_when_the_solution_is_positive_and_clamps_when_not() {
        // γ = 2 + 3x exactly: both coefficients positive, objective zero.
        let xs = [0.1, 0.4, 0.5, 0.9];
        let cols = vec![vec![1.0; 4], xs.to_vec()];
        let points: Vec<Point> = xs.iter().map(|x| point(2.0 + 3.0 * x)).collect();
        let (c, s) = nnls(&cols, &points);
        assert!(
            (c[0] - 2.0).abs() < 1e-12 && (c[1] - 3.0).abs() < 1e-12,
            "{c:?}"
        );
        assert!(s < 1e-24, "{s}");
        // γ = −1 + 2x: the unconstrained intercept is negative, so it is 0 and the slope is the fit through the
        // origin, Σxγ / Σx².
        let points: Vec<Point> = xs.iter().map(|x| point(-1.0 + 2.0 * x)).collect();
        let (c, _) = nnls(&cols, &points);
        let sxy: f64 = xs.iter().map(|x| x * (-1.0 + 2.0 * x)).sum();
        let sxx: f64 = xs.iter().map(|x| x * x).sum();
        assert_eq!(c[0], 0.0);
        assert!((c[1] - sxy / sxx).abs() < 1e-12, "{c:?}");
        // γ = −1: every coefficient is 0.
        let points: Vec<Point> = xs.iter().map(|_| point(-1.0)).collect();
        assert_eq!(nnls(&cols, &points).0, vec![0.0, 0.0]);
    }
}
