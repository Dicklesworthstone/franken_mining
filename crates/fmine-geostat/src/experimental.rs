//! Directional experimental semivariograms (`BD-237`).
//!
//! Determinism: BitwisePortable (`+ − × ÷`, `sqrt` and `floor`; `sin_cos` through `fmine_core::det`). Pairs are
//! summed in (first sample, second sample) index order, the order of a scan over every pair.
//!
//! A pair of samples i < j, with lag vector d from i to j and distance h = |d|, counts in a direction with unit
//! vector u when:
//! - |d·u| ≥ h cos(angle tolerance), unless the tolerance is 90° (omnidirectional);
//! - its offsets from the direction line are within the bandwidths: |d·v| ≤ horizontal and |d·w| ≤ vertical, where
//!   v is horizontal and perpendicular to u, and w = u × v lies in the vertical plane through u. An
//!   omnidirectional variogram has no bandwidths;
//! - its holes match the [`Pairs`] scope.
//!
//! It counts in lag k (1 ≤ k ≤ count) when |h − k·width| ≤ tolerance. With a tolerance over half the width a pair
//! can count in two lags, as in GSLIB's `gamv`. Coincident samples (h = 0) count in no lag and are reported.
//! γ(k) = Σ (z_head − z_tail)² / (2 N(k)). The head is the sample ahead along u, and the head and tail means show a
//! proportional effect.

use fmine_core::det;

use crate::GeostatError;
use crate::model::Rotation;
use crate::samples::{Sample, SampleSet};

/// Most lags one variogram may have.
pub const MAX_LAGS: u32 = 1000;

/// Most directions one call may take.
pub const MAX_DIRECTIONS: usize = 64;

/// A grid cell index beyond this, as a multiple of the search reach, is refused: past it the cell arithmetic loses
/// the precision the neighbour search needs.
const MAX_CELLS_PER_AXIS: f64 = 1e9;

/// A direction and its tolerances.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Direction {
    /// Azimuth, clockwise from north, degrees.
    pub azimuth_deg: f64,
    /// Dip from horizontal, negative downward, degrees.
    pub dip_deg: f64,
    /// Half-angle of the cone of accepted pairs, degrees; 90 is omnidirectional.
    pub angle_tolerance_deg: f64,
    /// Largest horizontal offset from the direction line, m.
    pub bandwidth_h_m: f64,
    /// Largest vertical offset from the direction line, m.
    pub bandwidth_v_m: f64,
}

impl Direction {
    /// Every pair, whatever its direction.
    pub const OMNIDIRECTIONAL: Self = Self {
        azimuth_deg: 0.0,
        dip_deg: 0.0,
        angle_tolerance_deg: 90.0,
        bandwidth_h_m: f64::INFINITY,
        bandwidth_v_m: f64::INFINITY,
    };

    /// Whether every direction counts.
    #[must_use]
    pub fn is_omnidirectional(&self) -> bool {
        self.angle_tolerance_deg >= 90.0
    }

    /// The direction's unit vector u, then the horizontal v perpendicular to it and w = u × v: the axes of a
    /// [`Rotation`] with this azimuth and dip and no rake.
    #[must_use]
    pub fn frame(&self) -> [[f64; 3]; 3] {
        Rotation {
            azimuth_deg: self.azimuth_deg,
            dip_deg: self.dip_deg,
            rake_deg: 0.0,
        }
        .axes()
    }

    fn check(&self, field: &str) -> Result<(), GeostatError> {
        if !self.azimuth_deg.is_finite() {
            return Err(GeostatError::invalid(
                format!("{field}.azimuth_deg"),
                format!("{} is not finite", self.azimuth_deg),
            ));
        }
        if !(self.dip_deg.is_finite() && (-90.0..=90.0).contains(&self.dip_deg)) {
            return Err(GeostatError::invalid(
                format!("{field}.dip_deg"),
                format!("{} is not between -90 and 90", self.dip_deg),
            ));
        }
        if !(self.angle_tolerance_deg > 0.0 && self.angle_tolerance_deg <= 90.0) {
            return Err(GeostatError::invalid(
                format!("{field}.angle_tolerance_deg"),
                format!("{} is not above 0 and at most 90", self.angle_tolerance_deg),
            ));
        }
        for (name, band) in [
            ("bandwidth_h_m", self.bandwidth_h_m),
            ("bandwidth_v_m", self.bandwidth_v_m),
        ] {
            if band.is_nan() || band <= 0.0 {
                return Err(GeostatError::invalid(
                    format!("{field}.{name}"),
                    format!("{band} is not positive"),
                ));
            }
        }
        Ok(())
    }
}

/// The lag classes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lags {
    /// Number of lags, the k-th centred at k·width.
    pub count: u32,
    /// Lag width, m.
    pub width_m: f64,
    /// Lag tolerance, m: above 0 and at most the width.
    pub tolerance_m: f64,
}

impl Lags {
    fn check(&self) -> Result<(), GeostatError> {
        if self.count == 0 || self.count > MAX_LAGS {
            return Err(GeostatError::invalid(
                "lags.count",
                format!("{} lags; 1 to {MAX_LAGS}", self.count),
            ));
        }
        if !(self.width_m.is_finite() && self.width_m > 0.0) {
            return Err(GeostatError::invalid(
                "lags.width_m",
                format!("{} is not a positive, finite width", self.width_m),
            ));
        }
        if !(self.tolerance_m > 0.0 && self.tolerance_m <= self.width_m) {
            return Err(GeostatError::invalid(
                "lags.tolerance_m",
                format!(
                    "{} is not above 0 and at most the width {}",
                    self.tolerance_m, self.width_m
                ),
            ));
        }
        Ok(())
    }

    /// The farthest a counted pair can be apart.
    fn reach(&self) -> f64 {
        f64::from(self.count) * self.width_m + self.tolerance_m
    }

    /// The lags a pair `h` apart may count in: a superset of those with |h − k·width| ≤ tolerance.
    fn candidates(&self, h: f64) -> (u32, u32) {
        let lo = ((h - self.tolerance_m) / self.width_m).floor() - 1.0;
        let hi = ((h + self.tolerance_m) / self.width_m).floor() + 1.0;
        let count = f64::from(self.count);
        // Both are clamped into 1..=count first, so the casts are exact.
        (lo.clamp(1.0, count) as u32, hi.clamp(0.0, count) as u32)
    }

    fn holds(&self, k: u32, h: f64) -> bool {
        (h - f64::from(k) * self.width_m).abs() <= self.tolerance_m
    }
}

/// Which pairs of holes count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pairs {
    /// Every pair.
    Any,
    /// Pairs within one hole: the downhole variogram, which shows the nugget at short lags.
    SameHole,
    /// Pairs from two different holes.
    OtherHoles,
}

impl Pairs {
    const fn admits(self, a: &Sample, b: &Sample) -> bool {
        match self {
            Self::Any => true,
            Self::SameHole => a.hole == b.hole,
            Self::OtherHoles => a.hole != b.hole,
        }
    }
}

/// One lag of an experimental variogram.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lag {
    /// Its index, centred at k·width.
    pub k: u32,
    /// Pairs counted in it.
    pub pairs: u64,
    /// Their mean distance, m.
    pub mean_distance_m: f64,
    /// The semivariogram value.
    pub gamma: f64,
    /// Mean value of the head samples.
    pub head_mean: f64,
    /// Mean value of the tail samples.
    pub tail_mean: f64,
}

/// The experimental variogram of one direction: its lags that hold at least one pair, by k.
#[derive(Clone, Debug, PartialEq)]
pub struct Experimental {
    /// The direction.
    pub direction: Direction,
    /// Its lags with pairs.
    pub lags: Vec<Lag>,
}

/// The experimental variograms of one sample set.
#[derive(Clone, Debug, PartialEq)]
pub struct ExperimentalSet {
    /// One per direction, in the order given.
    pub variograms: Vec<Experimental>,
    /// The lag classes.
    pub lags: Lags,
    /// The pair scope.
    pub pairs: Pairs,
    /// Pairs at the same point, within the scope, counted in no lag.
    pub coincident_pairs: u64,
}

#[derive(Clone, Copy, Default)]
struct Sums {
    n: u64,
    h: f64,
    sq: f64,
    head: f64,
    tail: f64,
}

struct Test {
    frame: [[f64; 3]; 3],
    cos_tolerance: f64,
    omni: bool,
    band_h: f64,
    band_v: f64,
}

/// The experimental variograms of `samples` in each of `directions`.
///
/// # Errors
/// [`GeostatError::Invalid`] for lags or a direction outside their domains, no directions or more than
/// [`MAX_DIRECTIONS`], or points spread over more than 10⁹ times the farthest lag.
pub fn experimental(
    samples: &SampleSet,
    directions: &[Direction],
    lags: &Lags,
    pairs: Pairs,
) -> Result<ExperimentalSet, GeostatError> {
    lags.check()?;
    if directions.is_empty() || directions.len() > MAX_DIRECTIONS {
        return Err(GeostatError::invalid(
            "directions",
            format!("{} directions; 1 to {MAX_DIRECTIONS}", directions.len()),
        ));
    }
    for (i, d) in directions.iter().enumerate() {
        d.check(&format!("directions[{i}]"))?;
    }
    let tests: Vec<Test> = directions
        .iter()
        .map(|d| Test {
            frame: d.frame(),
            cos_tolerance: det::sin_cos(d.angle_tolerance_deg * crate::model::DEG).1,
            omni: d.is_omnidirectional(),
            band_h: d.bandwidth_h_m,
            band_v: d.bandwidth_v_m,
        })
        .collect();
    let points = samples.samples();
    let grid = Grid::new(points, lags.reach())?;
    let count = lags.count as usize;
    let mut sums = vec![Sums::default(); tests.len() * count];
    let mut coincident_pairs = 0u64;
    let mut near = Vec::new();
    for (i, a) in points.iter().enumerate() {
        grid.after(i, &mut near);
        for &j in &near {
            let b = &points[j as usize];
            if !pairs.admits(a, b) {
                continue;
            }
            let d = [b.east_m - a.east_m, b.north_m - a.north_m, b.up_m - a.up_m];
            let h2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            if h2 == 0.0 {
                coincident_pairs += 1;
                continue;
            }
            let h = det::sqrt(h2);
            let (k_lo, k_hi) = lags.candidates(h);
            if k_lo > k_hi {
                continue;
            }
            for (t, test) in tests.iter().enumerate() {
                let along = dot(d, test.frame[0]);
                if !test.omni
                    && (along.abs() < test.cos_tolerance * h
                        || dot(d, test.frame[1]).abs() > test.band_h
                        || dot(d, test.frame[2]).abs() > test.band_v)
                {
                    continue;
                }
                let (head, tail) = if along >= 0.0 {
                    (b.value, a.value)
                } else {
                    (a.value, b.value)
                };
                let diff = head - tail;
                for k in k_lo..=k_hi {
                    if lags.holds(k, h) {
                        let s = &mut sums[t * count + (k as usize - 1)];
                        s.n += 1;
                        s.h += h;
                        s.sq += diff * diff;
                        s.head += head;
                        s.tail += tail;
                    }
                }
            }
        }
    }
    let variograms = directions
        .iter()
        .zip(sums.chunks(count))
        .map(|(direction, row)| Experimental {
            direction: *direction,
            lags: (1..=lags.count)
                .zip(row)
                .filter(|(_, s)| s.n > 0)
                .map(|(k, s)| {
                    let n = s.n as f64;
                    Lag {
                        k,
                        pairs: s.n,
                        mean_distance_m: s.h / n,
                        gamma: s.sq / (2.0 * n),
                        head_mean: s.head / n,
                        tail_mean: s.tail / n,
                    }
                })
                .collect(),
        })
        .collect();
    Ok(ExperimentalSet {
        variograms,
        lags: *lags,
        pairs,
        coincident_pairs,
    })
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Samples by grid cell. A cell is a little wider than the reach, so a pair within reach lies in the same or
/// adjacent cells along every axis.
struct Grid {
    /// (cell, sample index), sorted.
    cells: Vec<([i64; 3], u32)>,
    keys: Vec<[i64; 3]>,
}

impl Grid {
    fn new(points: &[Sample], reach: f64) -> Result<Self, GeostatError> {
        let mut origin = [f64::INFINITY; 3];
        for p in points {
            for (o, c) in origin.iter_mut().zip([p.east_m, p.north_m, p.up_m]) {
                *o = o.min(c);
            }
        }
        let cell = reach * (1.0 + 1e-6);
        let mut keys = Vec::with_capacity(points.len());
        for p in points {
            let mut key = [0i64; 3];
            for ((k, o), c) in key
                .iter_mut()
                .zip(origin)
                .zip([p.east_m, p.north_m, p.up_m])
            {
                let index = ((c - o) / cell).floor();
                if index > MAX_CELLS_PER_AXIS {
                    return Err(GeostatError::invalid(
                        "samples",
                        format!(
                            "points span more than {MAX_CELLS_PER_AXIS} times the farthest lag {reach} m"
                        ),
                    ));
                }
                // 0 ≤ index ≤ 10⁹: the cast is exact.
                *k = index as i64;
            }
            keys.push(key);
        }
        let mut cells: Vec<([i64; 3], u32)> = keys
            .iter()
            .enumerate()
            .map(|(i, k)| (*k, i as u32))
            .collect();
        cells.sort_unstable();
        Ok(Self { cells, keys })
    }

    /// The samples after sample `i` in its own and the 26 adjacent cells, ascending.
    fn after(&self, i: usize, out: &mut Vec<u32>) {
        out.clear();
        let key = self.keys[i];
        let i = i as u32;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let cell = [key[0] + dx, key[1] + dy, key[2] + dz];
                    let from = self.cells.partition_point(|e| (e.0, e.1) <= (cell, i));
                    let to = self.cells.partition_point(|e| e.0 <= cell);
                    if from < to {
                        out.extend(self.cells[from..to].iter().map(|e| e.1));
                    }
                }
            }
        }
        out.sort_unstable();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::SpatialSupport;

    fn set(points: &[([f64; 3], f64, u32)], holes: usize) -> SampleSet {
        SampleSet::new(
            "LOCAL",
            SpatialSupport::Composite { length_mm: 2000 },
            (0..holes).map(|h| format!("H{h}")).collect(),
            points
                .iter()
                .map(|&(p, value, hole)| Sample {
                    east_m: p[0],
                    north_m: p[1],
                    up_m: p[2],
                    value,
                    hole,
                })
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn test_candidate_lags_cover_every_lag_a_distance_holds() {
        let lags = Lags {
            count: 12,
            width_m: 2.5,
            tolerance_m: 2.0,
        };
        for step in 0..4000 {
            let h = f64::from(step) * 0.0125;
            let (lo, hi) = lags.candidates(h);
            for k in 1..=lags.count {
                if lags.holds(k, h) {
                    assert!(lo <= k && k <= hi, "h {h}: lag {k} outside {lo}..={hi}");
                }
            }
        }
    }

    #[test]
    fn test_the_grid_finds_every_later_sample_within_reach() {
        let mut points = Vec::new();
        let mut s: u64 = 0x2545_f491_4f6c_dd1d;
        for _ in 0..400 {
            let mut p = [0.0; 3];
            for c in &mut p {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                *c = (s >> 11) as f64 / (1u64 << 53) as f64 * 100.0;
            }
            points.push((p, 0.0, 0));
        }
        let samples = set(&points, 1);
        let reach = 9.0;
        let grid = Grid::new(samples.samples(), reach).unwrap();
        let mut near = Vec::new();
        for (i, a) in samples.samples().iter().enumerate() {
            grid.after(i, &mut near);
            let mut want = Vec::new();
            for (j, b) in samples.samples().iter().enumerate().skip(i + 1) {
                let d = [b.east_m - a.east_m, b.north_m - a.north_m, b.up_m - a.up_m];
                if (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() <= reach {
                    want.push(j as u32);
                }
            }
            assert!(near.windows(2).all(|w| w[0] < w[1]));
            assert!(near.iter().all(|&j| j as usize > i));
            for j in &want {
                assert!(near.binary_search(j).is_ok(), "sample {i}: {j} missing");
            }
        }
    }

    #[test]
    fn test_lags_and_directions_outside_their_domains_are_refused() {
        let samples = set(&[([0.0; 3], 1.0, 0), ([1.0, 0.0, 0.0], 2.0, 0)], 1);
        let ok = Lags {
            count: 4,
            width_m: 1.0,
            tolerance_m: 0.5,
        };
        let bad_lags = [
            (Lags { count: 0, ..ok }, "lags.count"),
            (Lags { count: 1001, ..ok }, "lags.count"),
            (Lags { width_m: 0.0, ..ok }, "lags.width_m"),
            (
                Lags {
                    tolerance_m: 1.5,
                    ..ok
                },
                "lags.tolerance_m",
            ),
            (
                Lags {
                    tolerance_m: f64::NAN,
                    ..ok
                },
                "lags.tolerance_m",
            ),
        ];
        for (lags, field) in bad_lags {
            match experimental(&samples, &[Direction::OMNIDIRECTIONAL], &lags, Pairs::Any) {
                Err(GeostatError::Invalid { field: f, .. }) => assert_eq!(f, field),
                other => panic!("{field}: {other:?}"),
            }
        }
        let d = Direction::OMNIDIRECTIONAL;
        let bad_directions = [
            (
                Direction {
                    dip_deg: -91.0,
                    ..d
                },
                "directions[0].dip_deg",
            ),
            (
                Direction {
                    azimuth_deg: f64::INFINITY,
                    ..d
                },
                "directions[0].azimuth_deg",
            ),
            (
                Direction {
                    angle_tolerance_deg: 0.0,
                    ..d
                },
                "directions[0].angle_tolerance_deg",
            ),
            (
                Direction {
                    angle_tolerance_deg: 91.0,
                    ..d
                },
                "directions[0].angle_tolerance_deg",
            ),
            (
                Direction {
                    bandwidth_h_m: 0.0,
                    ..d
                },
                "directions[0].bandwidth_h_m",
            ),
            (
                Direction {
                    bandwidth_v_m: f64::NAN,
                    ..d
                },
                "directions[0].bandwidth_v_m",
            ),
        ];
        for (direction, field) in bad_directions {
            match experimental(&samples, &[direction], &ok, Pairs::Any) {
                Err(GeostatError::Invalid { field: f, .. }) => assert_eq!(f, field),
                other => panic!("{field}: {other:?}"),
            }
        }
        match experimental(&samples, &[], &ok, Pairs::Any) {
            Err(GeostatError::Invalid { field, .. }) => assert_eq!(field, "directions"),
            other => panic!("{other:?}"),
        }
    }
}
