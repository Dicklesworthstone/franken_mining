//! Minimum-curvature desurvey (`BD-164`, plan §12.2, PO-CORE-1): a collar and its survey stations become a 3-D
//! trace, and any depth along the hole becomes a point in the collar's CRS.
//! Determinism: BitwisePortable (IEEE-754 arithmetic with `fmine_core::det` trigonometry and square roots).
//!
//! Between two stations the hole is the circular arc that leaves the first station along its direction and
//! reaches the second along its direction (minimum curvature). With unit directions `t1` and `t2`, dogleg `β` and
//! along-hole length `L`, the step is `L/2 · tan(β/2)/(β/2) · (t1 + t2)`. The dogleg is taken as
//! `2·atan2(|t2 − t1|, |t2 + t1|)`, which keeps its precision for small and large angles. A depth between
//! stations lies on that arc: its direction turns from `t1` to `t2` in proportion to depth, and its point is the
//! minimum-curvature step from the first station to that direction.
//!
//! Above the first station the hole is taken as straight from the collar along the first station's direction,
//! and below the last station as straight along the last one's. Those points are marked
//! [`Located::extrapolated`]. A hole surveyed at one station is straight along it.
//!
//! Azimuths are read as bearings from the collar CRS's grid north. Conversion from magnetic or true north is the
//! importer's (plan §12.1).

use fmine_core::det;

use crate::records::{
    Collar, DrillError, Orientation, Survey, SurveyInstrument, validate_collar, validate_survey,
};

const DEG: f64 = core::f64::consts::PI / 180.0;

/// Below this half-dogleg (rad), `tan(h)/h` is taken from its series.
const SMALL_HALF_DOGLEG: f64 = 1e-4;

/// Consecutive stations whose directions sum to less than this are antiparallel: the arc is undefined.
const ANTIPARALLEL: f64 = 1e-6;

/// A point in the collar's CRS, m.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// Easting, m.
    pub easting_m: f64,
    /// Northing, m.
    pub northing_m: f64,
    /// Elevation, m.
    pub elevation_m: f64,
}

/// A unit direction down the hole, by east, north and up components.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Direction {
    /// East component.
    pub east: f64,
    /// North component.
    pub north: f64,
    /// Up component; negative down the hole.
    pub up: f64,
}

impl Direction {
    /// The direction of a survey orientation (dip from horizontal, negative downward).
    #[must_use]
    pub fn of(orientation: &Orientation) -> Self {
        let (sin_dip, cos_dip) = det::sin_cos(orientation.dip_deg * DEG);
        let (sin_az, cos_az) = det::sin_cos(orientation.azimuth_deg * DEG);
        Self {
            east: cos_dip * sin_az,
            north: cos_dip * cos_az,
            up: sin_dip,
        }
    }

    /// Azimuth in [0, 360) and dip in [−90, 90], degrees. A vertical direction has azimuth 0.
    #[must_use]
    pub fn azimuth_dip_deg(&self) -> (f64, f64) {
        let horizontal = det::hypot(self.east, self.north);
        let dip = det::atan2(self.up, horizontal) / DEG;
        let mut azimuth = det::atan2(self.east, self.north) / DEG;
        if azimuth < 0.0 {
            azimuth += 360.0;
        }
        if azimuth >= 360.0 {
            azimuth = 0.0;
        }
        (azimuth, dip)
    }

    const fn v(self) -> [f64; 3] {
        [self.east, self.north, self.up]
    }

    fn from_v(v: [f64; 3]) -> Self {
        let n = norm(v);
        Self {
            east: v[0] / n,
            north: v[1] / n,
            up: v[2] / n,
        }
    }
}

/// Which collar position a trace starts from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollarFrom {
    /// The surveyed actual position.
    Actual,
    /// The planned position: the collar has no surveyed one.
    Planned,
}

/// One survey station on a trace.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceStation {
    /// The station's survey id.
    pub survey_id: String,
    /// Depth along the hole, mm.
    pub depth_mm: u64,
    /// Its direction.
    pub direction: Direction,
    /// Its point.
    pub point: Point,
    /// A magnetic reading with interference flagged, awaiting a gyro check ([`Survey::needs_gyro_check`]).
    pub suspect: bool,
}

/// A point along the hole.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Located {
    /// The point.
    pub point: Point,
    /// The hole's direction there.
    pub direction: Direction,
    /// Above the first station or below the last: a measured direction extended, not a point between stations.
    pub extrapolated: bool,
}

/// A desurveyed hole.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    hole_id: String,
    crs: String,
    collar: Point,
    collar_from: CollarFrom,
    stations: Vec<TraceStation>,
    /// Offsets of the stations from the collar (east, north, up), m: positions are summed here and the collar
    /// added once, so that large CRS coordinates do not round the steps.
    offsets: Vec<[f64; 3]>,
}

/// Desurvey `collar` with its `surveys` by minimum curvature.
///
/// # Errors
/// [`DrillError::Invalid`] for a record that fails its own checks, a station of another hole, a hole with no
/// surveyed station (a hole declared unsurveyed is not taken as straight), a declared-unsurveyed hole that has
/// surveyed stations, two stations at one depth (keep one run per depth), or consecutive stations pointing in
/// opposite directions. [`DrillError::CrsMismatch`] as the collar's own check gives it.
pub fn desurvey(collar: &Collar, surveys: &[Survey]) -> Result<Trace, DrillError> {
    validate_collar(collar)?;
    let (position, collar_from) = match (&collar.actual, &collar.planned) {
        (Some(p), _) => (p, CollarFrom::Actual),
        (None, Some(p)) => (p, CollarFrom::Planned),
        (None, None) => {
            return Err(DrillError::invalid(
                "collar.actual",
                "a collar needs a position",
            ));
        }
    };
    let hole = collar.hole_id.as_str();
    let mut declared_unsurveyed = false;
    let mut oriented: Vec<(&Survey, &Orientation)> = Vec::with_capacity(surveys.len());
    for s in surveys {
        validate_survey(s)?;
        if s.hole_id != hole {
            return Err(DrillError::invalid(
                "survey.hole_id",
                format!(
                    "station '{}' is on hole '{}', not '{hole}'",
                    s.survey_id, s.hole_id
                ),
            ));
        }
        match (s.instrument, &s.orientation) {
            (SurveyInstrument::None, _) | (_, None) => declared_unsurveyed = true,
            (_, Some(o)) => oriented.push((s, o)),
        }
    }
    if oriented.is_empty() {
        return Err(DrillError::invalid(
            "surveys",
            format!(
                "hole '{hole}' has no surveyed station; an unsurveyed hole is not taken as straight"
            ),
        ));
    }
    if declared_unsurveyed {
        return Err(DrillError::invalid(
            "surveys",
            format!("hole '{hole}' is declared unsurveyed and has surveyed stations"),
        ));
    }
    oriented.sort_by_key(|(s, _)| s.depth_mm);
    for pair in oriented.windows(2) {
        if let [(a, _), (b, _)] = pair
            && a.depth_mm == b.depth_mm
        {
            return Err(DrillError::invalid(
                "survey.depth_mm",
                format!(
                    "stations '{}' and '{}' are both at {} mm; keep one run per depth",
                    a.survey_id, b.survey_id, a.depth_mm
                ),
            ));
        }
    }

    let mut stations: Vec<TraceStation> = Vec::with_capacity(oriented.len());
    let mut offsets: Vec<[f64; 3]> = Vec::with_capacity(oriented.len());
    let collar_point = Point {
        easting_m: position.easting_m,
        northing_m: position.northing_m,
        elevation_m: position.elevation_m,
    };
    for (s, o) in oriented {
        let direction = Direction::of(o);
        let offset = match (stations.last(), offsets.last()) {
            (Some(prev), Some(at)) => {
                if norm(add(prev.direction.v(), direction.v())) < ANTIPARALLEL {
                    return Err(DrillError::invalid(
                        "survey.orientation",
                        format!(
                            "stations '{}' and '{}' point in opposite directions; no arc joins them",
                            prev.survey_id, s.survey_id
                        ),
                    ));
                }
                add(
                    *at,
                    step(
                        prev.direction,
                        direction,
                        metres(s.depth_mm - prev.depth_mm),
                    ),
                )
            }
            _ => scale(direction.v(), metres(s.depth_mm)),
        };
        stations.push(TraceStation {
            survey_id: s.survey_id.clone(),
            depth_mm: s.depth_mm,
            direction,
            point: shifted(collar_point, offset),
            suspect: s.needs_gyro_check(),
        });
        offsets.push(offset);
    }
    Ok(Trace {
        hole_id: hole.to_string(),
        crs: position.crs.clone(),
        collar: collar_point,
        collar_from,
        stations,
        offsets,
    })
}

impl Trace {
    /// The hole.
    #[must_use]
    pub fn hole_id(&self) -> &str {
        &self.hole_id
    }

    /// The CRS of every point, the collar's.
    #[must_use]
    pub fn crs(&self) -> &str {
        &self.crs
    }

    /// The collar point.
    #[must_use]
    pub const fn collar(&self) -> Point {
        self.collar
    }

    /// Which collar position the trace starts from.
    #[must_use]
    pub const fn collar_from(&self) -> CollarFrom {
        self.collar_from
    }

    /// The stations, by depth.
    #[must_use]
    pub fn stations(&self) -> &[TraceStation] {
        &self.stations
    }

    /// The point and direction at `depth_mm` along the hole.
    #[must_use]
    pub fn at(&self, depth_mm: u64) -> Located {
        let after = self.stations.partition_point(|s| s.depth_mm <= depth_mm);
        let (offset, direction, extrapolated) = match after.checked_sub(1) {
            None => {
                // Above the first station: straight from the collar along its direction.
                let first = &self.stations[0];
                (
                    scale(first.direction.v(), metres(depth_mm)),
                    first.direction,
                    true,
                )
            }
            Some(i) => {
                let from = &self.stations[i];
                let along = metres(depth_mm - from.depth_mm);
                match self.stations.get(i + 1) {
                    None => (
                        add(self.offsets[i], scale(from.direction.v(), along)),
                        from.direction,
                        depth_mm > from.depth_mm,
                    ),
                    Some(to) => {
                        let fraction = along / metres(to.depth_mm - from.depth_mm);
                        let direction = turn(from.direction, to.direction, fraction);
                        (
                            add(self.offsets[i], step(from.direction, direction, along)),
                            direction,
                            false,
                        )
                    }
                }
            }
        };
        Located {
            point: shifted(self.collar, offset),
            direction,
            extrapolated,
        }
    }
}

/// Millimetres to metres. Depths below 2^53 mm are exact.
#[allow(clippy::cast_precision_loss)]
fn metres(mm: u64) -> f64 {
    mm as f64 / 1000.0
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn norm(a: [f64; 3]) -> f64 {
    det::sqrt(a[0] * a[0] + a[1] * a[1] + a[2] * a[2])
}

fn shifted(p: Point, offset: [f64; 3]) -> Point {
    Point {
        easting_m: p.easting_m + offset[0],
        northing_m: p.northing_m + offset[1],
        elevation_m: p.elevation_m + offset[2],
    }
}

/// Half the dogleg between `t1` and `t2`, rad, with `|t1 + t2|` and `|t2 − t1|`.
fn half_dogleg(t1: Direction, t2: Direction) -> (f64, f64, f64) {
    let sum = norm(add(t1.v(), t2.v()));
    let diff = norm(sub(t2.v(), t1.v()));
    (det::atan2(diff, sum), sum, diff)
}

/// The minimum-curvature step over `length_m` from direction `t1` to direction `t2`.
fn step(t1: Direction, t2: Direction, length_m: f64) -> [f64; 3] {
    let (half, sum, diff) = half_dogleg(t1, t2);
    // tan(h)/h, the ratio factor; tan(h) = |t2 − t1| / |t2 + t1|.
    let ratio = if half < SMALL_HALF_DOGLEG {
        let h2 = half * half;
        1.0 + h2 / 3.0 + 2.0 * h2 * h2 / 15.0
    } else {
        diff / sum / half
    };
    scale(add(t1.v(), t2.v()), length_m / 2.0 * ratio)
}

/// The direction a `fraction` of the way along the arc from `t1` to `t2` (spherical interpolation).
fn turn(t1: Direction, t2: Direction, fraction: f64) -> Direction {
    let (half, _, _) = half_dogleg(t1, t2);
    let dogleg = 2.0 * half;
    if dogleg < SMALL_HALF_DOGLEG {
        return Direction::from_v(add(scale(t1.v(), 1.0 - fraction), scale(t2.v(), fraction)));
    }
    let s = det::sin(dogleg);
    let a = det::sin((1.0 - fraction) * dogleg) / s;
    let b = det::sin(fraction * dogleg) / s;
    Direction::from_v(add(scale(t1.v(), a), scale(t2.v(), b)))
}
