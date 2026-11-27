//! Survey-error propagation (`bd-zx57`, plan §12.2): the positional covariance of a desurveyed hole at each
//! station and at any depth, from the one-sigma accuracies its records state.
//! Determinism: BitwisePortable (IEEE-754 arithmetic with `fmine_core::det` trigonometry and square roots).
//!
//! The error model is a list of declared terms, after the form of the ISCWSA error model: each term takes a share
//! of one measured angle's stated variance and propagates it either at random, independently at every station,
//! or systematically, one error shared by every station of the same instrument and run. The shares of one angle
//! sum to 1, so a station's stated sigma is used whole and once. The collar's horizontal and vertical one-sigmas
//! are independent of the angles and of each other. Horizontal sigma is the standard deviation on each grid
//! axis, not a radial confidence radius. This is a declared first-order angular model, not a qualified ISCWSA
//! instrument model; large angular errors or nearly opposing directions can invalidate the linearization.
//!
//! The covariance is the linear propagation `Σ = Σ_collar + Σ_terms J·Σ_term·Jᵀ`, where `J` is the derivative of
//! the point with respect to each station's dip and azimuth. `J` comes from the minimum-curvature map that
//! [`desurvey`] uses, leg by leg: a station moves only the legs that touch it, the straight run from the collar
//! above the first station, and the run below the last. Each leg's derivative is a central difference of
//! `1e-5` rad on its offset from the station before it, never on CRS coordinates, so large eastings and
//! northings do not round it. Where ISCWSA weights each station by balanced-tangential legs, this uses the
//! minimum-curvature legs themselves; on a straight hole the two agree.

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::det;

use crate::desurvey::{
    CollarFrom, DEG, Direction, Trace, add, desurvey, metres, scale, step, sub, turn,
};
use crate::records::{Collar, DrillError, RunDirection, Survey, SurveyInstrument};

/// Central-difference step on each angle, rad.
const STEP_RAD: f64 = 1e-5;

/// The shares of one angle's variance sum to 1 within this.
const SHARE_TOLERANCE: f64 = 1e-9;

/// A measured angle of a survey station.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Angle {
    /// Dip, from the station's `sigma_dip_deg`.
    Dip,
    /// Azimuth, from the station's `sigma_azimuth_deg`.
    Azimuth,
}

impl Angle {
    const fn index(self) -> usize {
        match self {
            Self::Dip => 0,
            Self::Azimuth => 1,
        }
    }
}

/// How a term's error is shared between stations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Propagation {
    /// Independent at every station.
    Random,
    /// One error for all stations of one instrument and run, scaled by each station's sigma.
    Systematic,
}

/// One declared error term.
#[derive(Clone, Debug, PartialEq)]
pub struct ErrorTerm {
    /// The term's code, such as `AZ-S`; unique in its model.
    pub code: String,
    /// The angle it perturbs.
    pub angle: Angle,
    /// How it is shared between stations.
    pub propagation: Propagation,
    /// Its share of the angle's stated variance, in (0, 1].
    pub variance_share: f64,
}

/// The declared error terms of a hole's surveys.
#[derive(Clone, Debug, PartialEq)]
pub struct ErrorModel {
    /// The terms; each angle's shares sum to 1.
    pub terms: Vec<ErrorTerm>,
}

impl ErrorModel {
    /// Every station's dip and azimuth errors independent of every other station's: `DIP-R` and `AZ-R`.
    #[must_use]
    pub fn random() -> Self {
        Self::whole(Propagation::Random, "R")
    }

    /// One dip and one azimuth error per instrument run, shared by its stations: `DIP-S` and `AZ-S`.
    #[must_use]
    pub fn systematic() -> Self {
        Self::whole(Propagation::Systematic, "S")
    }

    fn whole(propagation: Propagation, suffix: &str) -> Self {
        let term = |angle, name: &str| ErrorTerm {
            code: format!("{name}-{suffix}"),
            angle,
            propagation,
            variance_share: 1.0,
        };
        Self {
            terms: vec![term(Angle::Dip, "DIP"), term(Angle::Azimuth, "AZ")],
        }
    }

    fn check(&self) -> Result<(), DrillError> {
        let field = "survey_error.model";
        let mut codes = BTreeSet::new();
        let mut shares = [0.0_f64; 2];
        let mut covered = [false; 2];
        for t in &self.terms {
            if t.code.trim().is_empty() || !codes.insert(t.code.as_str()) {
                return Err(DrillError::invalid(
                    field,
                    format!("term code '{}' is empty or repeated", t.code),
                ));
            }
            if !(t.variance_share > 0.0 && t.variance_share <= 1.0) {
                return Err(DrillError::invalid(
                    field,
                    format!(
                        "term '{}': share {} is not in (0, 1]",
                        t.code, t.variance_share
                    ),
                ));
            }
            shares[t.angle.index()] += t.variance_share;
            covered[t.angle.index()] = true;
        }
        for angle in [Angle::Dip, Angle::Azimuth] {
            let i = angle.index();
            if !covered[i] || (shares[i] - 1.0).abs() > SHARE_TOLERANCE {
                return Err(DrillError::invalid(
                    field,
                    format!(
                        "the {angle:?} terms' shares sum to {}, not 1: each station's stated sigma is used whole",
                        shares[i]
                    ),
                ));
            }
        }
        Ok(())
    }
}

/// A positional covariance by east, north and up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Covariance {
    /// The symmetric matrix, m².
    pub enu_m2: [[f64; 3]; 3],
}

impl Covariance {
    /// No uncertainty.
    pub const ZERO: Self = Self {
        enu_m2: [[0.0; 3]; 3],
    };

    /// One-sigma east, north and up, m.
    #[must_use]
    pub fn sigma_m(&self) -> [f64; 3] {
        [
            det::sqrt(self.enu_m2[0][0]),
            det::sqrt(self.enu_m2[1][1]),
            det::sqrt(self.enu_m2[2][2]),
        ]
    }

    /// The variance along `direction`, m².
    #[must_use]
    pub fn variance_along(&self, direction: Direction) -> f64 {
        let v = direction.v();
        let mut sum = 0.0;
        for (i, row) in self.enu_m2.iter().enumerate() {
            for (j, m) in row.iter().enumerate() {
                sum += v[i] * m * v[j];
            }
        }
        sum
    }

    fn add_outer(&mut self, v: [f64; 3], weight: f64) {
        for (i, row) in self.enu_m2.iter_mut().enumerate() {
            for (j, m) in row.iter_mut().enumerate() {
                *m += weight * v[i] * v[j];
            }
        }
    }

    fn add(&mut self, other: &Self) {
        for (row, other_row) in self.enu_m2.iter_mut().zip(&other.enu_m2) {
            for (m, o) in row.iter_mut().zip(other_row) {
                *m += o;
            }
        }
    }
}

/// One term's part of a covariance.
#[derive(Clone, Debug, PartialEq)]
pub struct TermCovariance {
    /// The term's code.
    pub code: String,
    /// Its covariance.
    pub covariance: Covariance,
}

/// The covariance of the point at one depth.
#[derive(Clone, Debug, PartialEq)]
pub struct PositionCovariance {
    /// Depth along the hole, mm.
    pub depth_mm: u64,
    /// The collar's part and every term's.
    pub total: Covariance,
    /// The collar's part.
    pub collar: Covariance,
    /// Each term's part, in model order.
    pub terms: Vec<TermCovariance>,
}

/// Derivatives of a point with respect to a station's dip and azimuth, m/rad.
type Columns = [[f64; 3]; 2];

const NO_COLUMNS: Columns = [[0.0; 3]; 2];

#[derive(Clone, Debug, PartialEq)]
struct StationAngles {
    dip_rad: f64,
    azimuth_rad: f64,
    /// One-sigma dip and azimuth, rad.
    sigma_rad: [f64; 2],
    /// Its instrument run, for systematic terms.
    run: usize,
}

impl StationAngles {
    /// The direction with angle `angle` moved by `delta_rad`. Past a pole the formula carries on, so a vertical
    /// station's derivative is the true one.
    fn moved(&self, angle: usize, delta_rad: f64) -> Direction {
        let (dip, azimuth) = if angle == 0 {
            (self.dip_rad + delta_rad, self.azimuth_rad)
        } else {
            (self.dip_rad, self.azimuth_rad + delta_rad)
        };
        let (sin_dip, cos_dip) = det::sin_cos(dip);
        let (sin_az, cos_az) = det::sin_cos(azimuth);
        Direction {
            east: cos_dip * sin_az,
            north: cos_dip * cos_az,
            up: sin_dip,
        }
    }

    /// `∂f/∂(dip, azimuth)` of this station by central differences.
    fn derivative(&self, f: impl Fn(Direction) -> [f64; 3]) -> Columns {
        let mut out = NO_COLUMNS;
        for (angle, column) in out.iter_mut().enumerate() {
            let difference = sub(
                f(self.moved(angle, STEP_RAD)),
                f(self.moved(angle, -STEP_RAD)),
            );
            *column = scale(difference, 0.5 / STEP_RAD);
        }
        out
    }
}

fn plus(a: Columns, b: Columns) -> Columns {
    [add(a[0], b[0]), add(a[1], b[1])]
}

/// A desurveyed hole with its error model.
#[derive(Clone, Debug, PartialEq)]
pub struct UncertainTrace {
    trace: Trace,
    model: ErrorModel,
    collar: Covariance,
    stations: Vec<StationAngles>,
    runs: usize,
    /// Derivative of the run ending at each station (from the collar, for the first) by that station's angles.
    into: Vec<Columns>,
    /// Derivative of the leg starting at each station by its angles; none at the last.
    out_of: Vec<Columns>,
}

/// Desurvey `collar` with its `surveys` and attach `model` (plan §12.2).
///
/// # Errors
/// [`DrillError::Invalid`] (`FMINE-REF-109`) for a model whose codes are empty or repeated, whose shares are not
/// in (0, 1], or whose shares of an angle do not sum to 1; [`desurvey`]'s refusals.
pub fn propagate(
    collar: &Collar,
    surveys: &[Survey],
    model: &ErrorModel,
) -> Result<UncertainTrace, DrillError> {
    model.check()?;
    let trace = desurvey(collar, surveys)?;
    let position = match trace.collar_from() {
        CollarFrom::Actual => collar.actual.as_ref(),
        CollarFrom::Planned => collar.planned.as_ref(),
    }
    .ok_or_else(|| DrillError::invalid("collar.actual", "a collar needs a position"))?;
    let (h, v) = (position.sigma_horizontal_m, position.sigma_vertical_m);
    let collar_covariance = Covariance {
        enu_m2: [[h * h, 0.0, 0.0], [0.0, h * h, 0.0], [0.0, 0.0, v * v]],
    };

    // desurvey keeps one oriented station per depth.
    let by_depth: BTreeMap<u64, &Survey> = surveys
        .iter()
        .filter(|s| s.orientation.is_some())
        .map(|s| (s.depth_mm, s))
        .collect();
    let mut runs: BTreeMap<(SurveyInstrument, RunDirection), usize> = BTreeMap::new();
    let mut stations = Vec::with_capacity(trace.stations().len());
    for station in trace.stations() {
        let survey = by_depth.get(&station.depth_mm);
        let Some((survey, o)) = survey.and_then(|s| s.orientation.as_ref().map(|o| (s, o))) else {
            return Err(DrillError::invalid(
                "surveys",
                format!("station at {} mm has no orientation", station.depth_mm),
            ));
        };
        let next = runs.len();
        let run = *runs.entry((survey.instrument, survey.run)).or_insert(next);
        stations.push(StationAngles {
            dip_rad: o.dip_deg * DEG,
            azimuth_rad: o.azimuth_deg * DEG,
            sigma_rad: [o.sigma_dip_deg * DEG, o.sigma_azimuth_deg * DEG],
            run,
        });
    }

    let st = trace.stations();
    let mut into = Vec::with_capacity(st.len());
    let mut out_of = vec![NO_COLUMNS; st.len()];
    for (k, angles) in stations.iter().enumerate() {
        into.push(match k.checked_sub(1) {
            None => {
                let run = metres(st[0].depth_mm);
                angles.derivative(|t| scale(t.v(), run))
            }
            Some(p) => {
                let (before, length) = (st[p].direction, metres(st[k].depth_mm - st[p].depth_mm));
                angles.derivative(|t| step(before, t, length))
            }
        });
        if let Some(next) = st.get(k + 1) {
            let (after, length) = (next.direction, metres(next.depth_mm - st[k].depth_mm));
            out_of[k] = angles.derivative(|t| step(t, after, length));
        }
    }
    Ok(UncertainTrace {
        trace,
        model: model.clone(),
        collar: collar_covariance,
        stations,
        runs: runs.len(),
        into,
        out_of,
    })
}

impl UncertainTrace {
    /// The desurveyed hole.
    #[must_use]
    pub const fn trace(&self) -> &Trace {
        &self.trace
    }

    /// The error model.
    #[must_use]
    pub const fn model(&self) -> &ErrorModel {
        &self.model
    }

    /// The covariance of [`Trace::at`]`(depth_mm)`.
    #[must_use]
    pub fn covariance_at(&self, depth_mm: u64) -> PositionCovariance {
        let jacobian = self.jacobian(depth_mm);
        let mut total = self.collar;
        let mut terms = Vec::with_capacity(self.model.terms.len());
        for term in &self.model.terms {
            let a = term.angle.index();
            let mut part = Covariance::ZERO;
            match term.propagation {
                Propagation::Random => {
                    for (s, columns) in self.stations.iter().zip(&jacobian) {
                        part.add_outer(scale(columns[a], s.sigma_rad[a]), term.variance_share);
                    }
                }
                Propagation::Systematic => {
                    let mut sums = vec![[0.0; 3]; self.runs];
                    for (s, columns) in self.stations.iter().zip(&jacobian) {
                        sums[s.run] = add(sums[s.run], scale(columns[a], s.sigma_rad[a]));
                    }
                    for v in sums {
                        part.add_outer(v, term.variance_share);
                    }
                }
            }
            total.add(&part);
            terms.push(TermCovariance {
                code: term.code.clone(),
                covariance: part,
            });
        }
        PositionCovariance {
            depth_mm,
            total,
            collar: self.collar,
            terms,
        }
    }

    /// The declared model's covariance, refusing nonfinite arithmetic before it reaches a report.
    ///
    /// # Errors
    /// [`DrillError::Invalid`] for nonfinite covariance, including squared sigmas, extreme extrapolation,
    /// or an ill-conditioned perturbed arc whose derivative cannot be represented.
    pub fn checked_covariance_at(&self, depth_mm: u64) -> Result<PositionCovariance, DrillError> {
        let result = self.covariance_at(depth_mm);
        for covariance in std::iter::once(&result.collar)
            .chain(std::iter::once(&result.total))
            .chain(result.terms.iter().map(|term| &term.covariance))
        {
            if covariance
                .enu_m2
                .iter()
                .flatten()
                .any(|value| !value.is_finite())
            {
                return Err(DrillError::invalid(
                    "survey_error.covariance",
                    "declared model exceeds finite covariance arithmetic",
                ));
            }
        }
        Ok(result)
    }

    /// First-order positional variance along a finite direction under this declared error model.
    /// A unit direction gives square metres. Each independent error vector is projected before squaring,
    /// so a rank-deficient model's null direction cannot produce negative variance through matrix cancellation.
    ///
    /// # Errors
    /// [`DrillError::Invalid`] for a nonfinite direction, sensitivity, or variance. No clamping is used.
    pub fn checked_variance_along(
        &self,
        depth_mm: u64,
        direction: Direction,
    ) -> Result<f64, DrillError> {
        let u = direction.v();
        if u.iter().any(|value| !value.is_finite()) {
            return Err(DrillError::invalid(
                "survey_error.direction",
                "projection direction must be finite",
            ));
        }
        let mut variance = 0.0;
        for (axis, component) in u.iter().enumerate() {
            let projected = component * det::sqrt(self.collar.enu_m2[axis][axis]);
            variance += projected * projected;
        }
        let jacobian = self.jacobian(depth_mm);
        for term in &self.model.terms {
            let angle = term.angle.index();
            let root_share = det::sqrt(term.variance_share);
            let projected_square = |v: [f64; 3]| {
                let projection = root_share * (u[0] * v[0] + u[1] * v[1] + u[2] * v[2]);
                projection * projection
            };
            match term.propagation {
                Propagation::Random => {
                    for (station, columns) in self.stations.iter().zip(&jacobian) {
                        variance +=
                            projected_square(scale(columns[angle], station.sigma_rad[angle]));
                    }
                }
                Propagation::Systematic => {
                    let mut sums = vec![[0.0; 3]; self.runs];
                    for (station, columns) in self.stations.iter().zip(&jacobian) {
                        sums[station.run] = add(
                            sums[station.run],
                            scale(columns[angle], station.sigma_rad[angle]),
                        );
                    }
                    for vector in sums {
                        variance += projected_square(vector);
                    }
                }
            }
        }
        if !variance.is_finite() {
            return Err(DrillError::invalid(
                "survey_error.variance",
                "declared model exceeds finite projected variance arithmetic",
            ));
        }
        Ok(variance)
    }

    /// The covariance at every station, by depth.
    #[must_use]
    pub fn at_stations(&self) -> Vec<PositionCovariance> {
        self.trace
            .stations()
            .iter()
            .map(|s| self.covariance_at(s.depth_mm))
            .collect()
    }

    /// Derivatives of the point at `depth_mm` by each station's angles, following [`Trace::at`].
    fn jacobian(&self, depth_mm: u64) -> Vec<Columns> {
        let st = self.trace.stations();
        let mut j = vec![NO_COLUMNS; st.len()];
        let after = st.partition_point(|s| s.depth_mm <= depth_mm);
        let Some(i) = after.checked_sub(1) else {
            let run = metres(depth_mm);
            j[0] = self.stations[0].derivative(|t| scale(t.v(), run));
            return j;
        };
        for k in 0..i {
            j[k] = plus(self.into[k], self.out_of[k]);
        }
        j[i] = self.into[i];
        let along = metres(depth_mm - st[i].depth_mm);
        match st.get(i + 1) {
            None => {
                j[i] = plus(j[i], self.stations[i].derivative(|t| scale(t.v(), along)));
            }
            Some(to) => {
                let fraction = along / metres(to.depth_mm - st[i].depth_mm);
                let (from_direction, to_direction) = (st[i].direction, to.direction);
                j[i] = plus(
                    j[i],
                    self.stations[i]
                        .derivative(|t| step(t, turn(t, to_direction, fraction), along)),
                );
                j[i + 1] = self.stations[i + 1]
                    .derivative(|t| step(from_direction, turn(from_direction, t, fraction), along));
            }
        }
        j
    }
}
