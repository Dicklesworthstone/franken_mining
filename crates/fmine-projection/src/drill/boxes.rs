//! The 3-D box that covers a logging interval's desurveyed trace (`BD-121`).
//!
//! Determinism: BitwisePortable (IEEE-754 addition, subtraction, multiplication, division and square roots).
//!
//! Boxes are axis-aligned in the hole's CRS: east, north and up, in metres, and bit-identical wherever the
//! `fmine_drill::desurvey` trace is.

use std::f64::consts::PI;

use fmine_drill::desurvey::{Direction, Point, Trace};

/// Rounding allowance added to every piece of a box, m. The trace's points are the collar plus offsets summed
/// near the origin, so their rounding error is about one ulp of the collar's coordinates (2 × 10⁻⁹ m at a
/// northing of 8.6 × 10⁶ m). 1 µm covers that many times over and is far below any logging resolution.
pub const ROUNDING_M: f64 = 1e-6;

/// An axis-aligned box: east, north and up, m. Each axis has `min ≤ max`, and both are finite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box3 {
    min: [f64; 3],
    max: [f64; 3],
}

impl Box3 {
    /// The box from `min` to `max`, or `None` when a bound is not finite or a `min` exceeds its `max`.
    #[must_use]
    pub fn new(min: [f64; 3], max: [f64; 3]) -> Option<Self> {
        let ok = (0..3).all(|k| min[k].is_finite() && max[k].is_finite() && min[k] <= max[k]);
        ok.then_some(Self { min, max })
    }

    /// The lower corner: east, north, up.
    #[must_use]
    pub const fn min(&self) -> [f64; 3] {
        self.min
    }

    /// The upper corner: east, north, up.
    #[must_use]
    pub const fn max(&self) -> [f64; 3] {
        self.max
    }

    /// Whether the two boxes share a point; touching faces count.
    #[must_use]
    pub fn meets(&self, other: &Self) -> bool {
        (0..3).all(|k| self.min[k] <= other.max[k] && other.min[k] <= self.max[k])
    }

    /// Whether `p` lies in the box, faces included.
    #[must_use]
    pub fn contains(&self, p: &Point) -> bool {
        let q = coords(p);
        (0..3).all(|k| self.min[k] <= q[k] && q[k] <= self.max[k])
    }

    fn of_point(p: &Point) -> Self {
        let q = coords(p);
        Self { min: q, max: q }
    }

    fn join(self, other: Self) -> Self {
        let mut out = self;
        for k in 0..3 {
            out.min[k] = out.min[k].min(other.min[k]);
            out.max[k] = out.max[k].max(other.max[k]);
        }
        out
    }

    fn widened(self, by: f64) -> Self {
        let mut out = self;
        for k in 0..3 {
            out.min[k] -= by;
            out.max[k] += by;
        }
        out
    }
}

/// The box covering `trace` between depths `from_mm` and `to_mm` along the hole.
///
/// The trace is cut at its stations. Above the first station and below the last it is straight, and between two
/// stations it is one circular arc (minimum curvature). A straight piece lies in the box of its two ends. An arc
/// of length ℓ that turns through φ strays from its chord by at most (ℓ/φ)(1 − cos(φ/2)) ≤ ℓφ/8. The unit
/// tangents d_a and d_b at its ends satisfy |d_a − d_b| = 2 sin(φ/2) ≥ 2φ/π, because sin x ≥ 2x/π on [0, π/2].
/// So each piece's box is its ends' box widened by ℓπ|d_a − d_b|/16 (zero for a straight piece), plus
/// [`ROUNDING_M`]. The interval's box joins its pieces' boxes.
#[must_use]
pub fn interval_box(trace: &Trace, from_mm: u64, to_mm: u64) -> Box3 {
    let (lo, hi) = (from_mm.min(to_mm), from_mm.max(to_mm));
    let mut cuts = vec![lo];
    cuts.extend(
        trace
            .stations()
            .iter()
            .map(|s| s.depth_mm)
            .filter(|d| *d > lo && *d < hi),
    );
    cuts.push(hi);
    let start = trace.at(lo);
    let mut out = Box3::of_point(&start.point).widened(ROUNDING_M);
    for pair in cuts.windows(2) {
        let &[a, b] = pair else { continue };
        let (pa, pb) = (trace.at(a), trace.at(b));
        let length_m = metres(b - a);
        let sagitta_m = length_m * PI * gap(&pa.direction, &pb.direction) / 16.0;
        let piece = Box3::of_point(&pa.point)
            .join(Box3::of_point(&pb.point))
            .widened(sagitta_m + ROUNDING_M);
        out = out.join(piece);
    }
    out
}

const fn coords(p: &Point) -> [f64; 3] {
    [p.easting_m, p.northing_m, p.elevation_m]
}

/// |a − b| for two unit directions.
fn gap(a: &Direction, b: &Direction) -> f64 {
    let (e, n, u) = (a.east - b.east, a.north - b.north, a.up - b.up);
    (e * e + n * n + u * u).sqrt()
}

/// Millimetres along the hole to metres. Exact below 2⁵³ mm (9 × 10⁹ km).
#[allow(clippy::cast_precision_loss)]
fn metres(mm: u64) -> f64 {
    mm as f64 / 1000.0
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use fmine_drill::desurvey::desurvey;
    use fmine_drill::records::{
        Collar, HoleType, Orientation, Position, PositionMethod, RunDirection, Survey,
        SurveyInstrument,
    };

    fn collar(e: f64, n: f64, z: f64) -> Collar {
        Collar {
            hole_id: "H1".to_string(),
            hole_type: HoleType::Diamond,
            program: "test".to_string(),
            planned: None,
            actual: Some(Position {
                crs: "EPSG:32735".to_string(),
                easting_m: e,
                northing_m: n,
                elevation_m: z,
                method: PositionMethod::DifferentialGnss,
                sigma_horizontal_m: 0.05,
                sigma_vertical_m: 0.1,
            }),
        }
    }

    fn station(id: &str, depth_mm: u64, azimuth_deg: f64, dip_deg: f64) -> Survey {
        Survey {
            survey_id: id.to_string(),
            hole_id: "H1".to_string(),
            depth_mm,
            instrument: SurveyInstrument::Gyro,
            orientation: Some(Orientation {
                azimuth_deg,
                dip_deg,
                sigma_azimuth_deg: 0.5,
                sigma_dip_deg: 0.25,
            }),
            magnetic_interference: false,
            run: RunDirection::Out,
        }
    }

    /// A vertical hole: the box of 10 m to 12 m is the segment's box, widened only by the rounding allowance.
    #[test]
    fn test_a_vertical_interval_is_its_segment() {
        let trace = desurvey(
            &collar(421_000.0, 8_652_100.0, 1_300.0),
            &[station("S0", 0, 0.0, -90.0)],
        )
        .unwrap();
        let b = interval_box(&trace, 10_000, 12_000);
        let near = |x: f64, y: f64| (x - y).abs() <= 1e-9;
        assert!(
            near(b.min()[0], 421_000.0 - ROUNDING_M) && near(b.max()[0], 421_000.0 + ROUNDING_M)
        );
        assert!(
            near(b.min()[1], 8_652_100.0 - ROUNDING_M)
                && near(b.max()[1], 8_652_100.0 + ROUNDING_M)
        );
        assert!(near(b.min()[2], 1_288.0 - ROUNDING_M) && near(b.max()[2], 1_290.0 + ROUNDING_M));
        // The order of the depths does not matter.
        assert_eq!(interval_box(&trace, 12_000, 10_000), b);
    }

    /// A hole curving through 40° of azimuth and 25° of dip over three stations: every point of the trace, sampled
    /// each 5 mm, lies in its interval's box, including intervals that span stations and run past the last one.
    /// For logging-length intervals (up to 3 m) the box is within 2 cm of the samples' own box on every face; a
    /// long piece of arc is padded more, by design (ℓπ|d_a − d_b|/16).
    #[test]
    fn test_every_point_of_a_curved_trace_lies_in_its_box() {
        let trace = desurvey(
            &collar(421_000.4, 8_652_100.25, 1_312.5),
            &[
                station("S0", 0, 80.0, -75.0),
                station("S1", 30_000, 100.0, -62.0),
                station("S2", 60_000, 120.0, -50.0),
            ],
        )
        .unwrap();
        let mut checked = 0_u64;
        let intervals = [
            (0, 2_000, Some(0.02)),
            (28_500, 31_500, Some(0.02)),
            (44_000, 47_000, Some(0.02)),
            (59_000, 61_000, Some(0.02)),
            (5_000, 55_000, None),
            (59_000, 75_000, None),
            (0, 90_000, None),
        ];
        for (from, to, slack) in intervals {
            let b = interval_box(&trace, from, to);
            let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
            for d in (from..=to).step_by(5) {
                let p = trace.at(d).point;
                assert!(b.contains(&p), "{from}..{to} at {d}: {p:?} outside {b:?}");
                let q = coords(&p);
                for k in 0..3 {
                    lo[k] = lo[k].min(q[k]);
                    hi[k] = hi[k].max(q[k]);
                }
                checked += 1;
            }
            if let Some(slack) = slack {
                for k in 0..3 {
                    let (below, above) = (lo[k] - b.min()[k], b.max()[k] - hi[k]);
                    assert!(
                        below < slack && above < slack,
                        "{from}..{to} axis {k}: {below} {above}"
                    );
                }
            }
        }
        // 401 + 601 + 601 + 401 + 10 001 + 3 201 + 18 001 points.
        assert_eq!(checked, 33_207);
    }

    #[test]
    fn test_boxes_refuse_inverted_or_non_finite_bounds_and_meet_at_faces() {
        assert!(Box3::new([0.0; 3], [1.0, 1.0, -1.0]).is_none());
        assert!(Box3::new([f64::NAN, 0.0, 0.0], [1.0; 3]).is_none());
        assert!(Box3::new([0.0; 3], [f64::INFINITY, 1.0, 1.0]).is_none());
        let a = Box3::new([0.0; 3], [1.0; 3]).unwrap();
        let touching = Box3::new([1.0, 0.0, 0.0], [2.0, 1.0, 1.0]).unwrap();
        let apart = Box3::new([1.0 + 1e-9, 0.0, 0.0], [2.0, 1.0, 1.0]).unwrap();
        assert!(a.meets(&touching) && touching.meets(&a));
        assert!(!a.meets(&apart) && !apart.meets(&a));
    }
}
