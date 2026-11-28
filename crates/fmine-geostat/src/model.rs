//! Variogram models: a nugget plus nested structures with geometric anisotropy (`BD-237`).
//!
//! Determinism: BitwisePortable (`+ − × ÷` and `sqrt`; `exp` and `sin_cos` through `fmine_core::det`).
//!
//! A structure's shape is a function of the scaled distance h, which is 1 at its range:
//! - spherical: 1.5 h − 0.5 h³ below 1, and 1 from there on;
//! - exponential: 1 − exp(−3h). Its range is the practical range, where it reaches 95% of its sill;
//! - Gaussian: 1 − exp(−3h²), with a practical range in the same sense.
//!
//! These are GSLIB's definitions (`cova3`). Anisotropy is geometric. A rotation takes a lag into the structure's
//! frame, and each frame component is divided by that axis's range. The rotation is GSLIB's `setrot`: the major
//! axis points along `azimuth_deg` (clockwise from north) and `dip_deg` (negative downward), and `rake_deg` turns
//! the other two axes about it. With no rake the semi-major axis is horizontal, 90° anticlockwise from the major
//! axis in plan, and the minor axis completes a right-handed frame.

use fmine_core::det;

use crate::GeostatError;

/// Degrees to radians.
pub(crate) const DEG: f64 = std::f64::consts::PI / 180.0;

/// Most structures a model may nest.
pub const MAX_STRUCTURES: usize = 4;

/// The shape of a structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// Spherical: linear near the origin, and reaches its sill at its range.
    Spherical,
    /// Exponential, by its practical range.
    Exponential,
    /// Gaussian, by its practical range; parabolic near the origin.
    Gaussian,
}

impl Shape {
    /// Its value for a unit sill at scaled distance `h`, which is 1 at the range.
    #[must_use]
    pub fn at(self, h: f64) -> f64 {
        match self {
            Self::Spherical => {
                if h >= 1.0 {
                    1.0
                } else {
                    h * (1.5 - 0.5 * h * h)
                }
            }
            Self::Exponential => 1.0 - det::exp(-3.0 * h),
            Self::Gaussian => 1.0 - det::exp(-3.0 * h * h),
        }
    }
}

/// A structure's orientation, as GSLIB's `setrot` angles in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotation {
    /// Azimuth of the major axis, clockwise from north.
    pub azimuth_deg: f64,
    /// Dip of the major axis from horizontal, negative downward.
    pub dip_deg: f64,
    /// Rotation of the semi-major and minor axes about the major axis.
    pub rake_deg: f64,
}

impl Rotation {
    /// No rotation: the major axis points north, the semi-major axis west and the minor axis up.
    pub const NONE: Self = Self {
        azimuth_deg: 0.0,
        dip_deg: 0.0,
        rake_deg: 0.0,
    };

    /// The unit major, semi-major and minor axes, each as east, north and up components.
    #[must_use]
    pub fn axes(&self) -> [[f64; 3]; 3] {
        let (sin_a, cos_a) = det::sin_cos((90.0 - self.azimuth_deg) * DEG);
        let (sin_b, cos_b) = det::sin_cos(-self.dip_deg * DEG);
        let (sin_t, cos_t) = det::sin_cos(self.rake_deg * DEG);
        [
            [cos_b * cos_a, cos_b * sin_a, -sin_b],
            [
                -cos_t * sin_a + sin_t * sin_b * cos_a,
                cos_t * cos_a + sin_t * sin_b * sin_a,
                sin_t * cos_b,
            ],
            [
                sin_t * sin_a + cos_t * sin_b * cos_a,
                -sin_t * cos_a + cos_t * sin_b * sin_a,
                cos_t * cos_b,
            ],
        ]
    }

    fn check(&self, field: &str) -> Result<(), GeostatError> {
        for (name, angle) in [
            ("azimuth_deg", self.azimuth_deg),
            ("dip_deg", self.dip_deg),
            ("rake_deg", self.rake_deg),
        ] {
            if !angle.is_finite() {
                return Err(GeostatError::invalid(
                    format!("{field}.{name}"),
                    format!("{angle} is not a finite angle"),
                ));
            }
        }
        Ok(())
    }
}

/// A structure's ranges along its major, semi-major and minor axes, m.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ranges {
    /// Along the major axis.
    pub major_m: f64,
    /// Along the semi-major axis.
    pub semi_m: f64,
    /// Along the minor axis.
    pub minor_m: f64,
}

impl Ranges {
    /// The same range along every axis.
    #[cfg(test)]
    pub(crate) const fn isotropic(range_m: f64) -> Self {
        Self {
            major_m: range_m,
            semi_m: range_m,
            minor_m: range_m,
        }
    }

    pub(crate) const fn as_array(&self) -> [f64; 3] {
        [self.major_m, self.semi_m, self.minor_m]
    }
}

/// One nested structure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Structure {
    /// Its shape.
    pub shape: Shape,
    /// Its sill contribution, in the value's units squared.
    pub sill: f64,
    /// Its ranges.
    pub ranges: Ranges,
    /// Its orientation.
    pub rotation: Rotation,
}

/// A variogram model: a nugget and up to [`MAX_STRUCTURES`] nested structures.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    nugget: f64,
    structures: Vec<Structure>,
    /// Per structure, its axes divided by their ranges: the matrix that takes a lag to its scaled frame.
    frames: Vec<[[f64; 3]; 3]>,
}

impl Model {
    /// A model of `nugget` and `structures`.
    ///
    /// # Errors
    /// [`GeostatError::Invalid`] for a negative or non-finite nugget or sill, a range that is not positive and
    /// finite, an angle that is not finite, or more than [`MAX_STRUCTURES`] structures.
    pub fn new(nugget: f64, structures: Vec<Structure>) -> Result<Self, GeostatError> {
        check_sill("model.nugget", nugget)?;
        if structures.len() > MAX_STRUCTURES {
            return Err(GeostatError::invalid(
                "model.structures",
                format!(
                    "{} structures; at most {MAX_STRUCTURES} are nested",
                    structures.len()
                ),
            ));
        }
        let mut frames = Vec::with_capacity(structures.len());
        for (k, s) in structures.iter().enumerate() {
            let field = format!("model.structures[{k}]");
            check_sill(&format!("{field}.sill"), s.sill)?;
            check_ranges(&field, &s.ranges)?;
            s.rotation.check(&format!("{field}.rotation"))?;
            frames.push(frame(&s.rotation, &s.ranges));
        }
        Ok(Self {
            nugget,
            structures,
            frames,
        })
    }

    /// The nugget.
    #[must_use]
    pub const fn nugget(&self) -> f64 {
        self.nugget
    }

    /// The structures, as given.
    #[must_use]
    pub fn structures(&self) -> &[Structure] {
        &self.structures
    }

    /// The total sill: the nugget plus every structure's sill.
    #[must_use]
    pub fn sill(&self) -> f64 {
        self.structures
            .iter()
            .fold(self.nugget, |total, s| total + s.sill)
    }

    /// γ at lag vector `lag` (east, north and up, m). It is 0 at the zero lag and takes the nugget at any other.
    #[must_use]
    pub fn gamma(&self, lag: [f64; 3]) -> f64 {
        if lag == [0.0; 3] {
            return 0.0;
        }
        let mut gamma = self.nugget;
        for (s, f) in self.structures.iter().zip(&self.frames) {
            gamma += s.sill * s.shape.at(scaled(f, lag));
        }
        gamma
    }

    /// The covariance at lag vector `lag`: the total sill less γ.
    #[must_use]
    pub fn covariance(&self, lag: [f64; 3]) -> f64 {
        self.sill() - self.gamma(lag)
    }
}

/// The matrix taking a lag to the scaled frame of a structure with `rotation` and `ranges`.
pub(crate) fn frame(rotation: &Rotation, ranges: &Ranges) -> [[f64; 3]; 3] {
    let axes = rotation.axes();
    let ranges = ranges.as_array();
    let mut out = [[0.0; 3]; 3];
    for (row, (axis, range)) in out.iter_mut().zip(axes.iter().zip(ranges)) {
        for (o, a) in row.iter_mut().zip(axis) {
            *o = a / range;
        }
    }
    out
}

/// The scaled distance of `lag` in `frame`.
pub(crate) fn scaled(frame: &[[f64; 3]; 3], lag: [f64; 3]) -> f64 {
    let mut sum = 0.0;
    for row in frame {
        let c = row[0] * lag[0] + row[1] * lag[1] + row[2] * lag[2];
        sum += c * c;
    }
    det::sqrt(sum)
}

pub(crate) fn check_sill(field: &str, sill: f64) -> Result<(), GeostatError> {
    if sill.is_finite() && sill >= 0.0 {
        Ok(())
    } else {
        Err(GeostatError::invalid(
            field,
            format!("{sill} is not a finite, non-negative sill"),
        ))
    }
}

fn check_ranges(field: &str, ranges: &Ranges) -> Result<(), GeostatError> {
    for (name, range) in [
        ("major_m", ranges.major_m),
        ("semi_m", ranges.semi_m),
        ("minor_m", ranges.minor_m),
    ] {
        if !(range.is_finite() && range > 0.0) {
            return Err(GeostatError::invalid(
                format!("{field}.ranges.{name}"),
                format!("{range} is not a positive, finite range"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * b.abs().max(1.0)
    }

    fn spherical(sill: f64, ranges: Ranges, rotation: Rotation) -> Structure {
        Structure {
            shape: Shape::Spherical,
            sill,
            ranges,
            rotation,
        }
    }

    fn at(axis: [f64; 3], length: f64) -> [f64; 3] {
        [axis[0] * length, axis[1] * length, axis[2] * length]
    }

    #[test]
    fn test_structures_take_their_textbook_values() {
        assert_eq!(Shape::Spherical.at(0.0), 0.0);
        assert_eq!(Shape::Spherical.at(0.5), 0.6875);
        assert_eq!(Shape::Spherical.at(1.0), 1.0);
        assert_eq!(Shape::Spherical.at(7.0), 1.0);
        // 1 − e⁻³ and 1 − e^(−3/4).
        assert!(close(
            Shape::Exponential.at(1.0),
            0.950_212_931_632_136,
            1e-15
        ));
        assert!(close(Shape::Gaussian.at(1.0), 0.950_212_931_632_136, 1e-15));
        assert!(close(
            Shape::Exponential.at(0.5),
            0.776_869_839_851_570_2,
            1e-15
        ));
        assert!(close(
            Shape::Gaussian.at(0.5),
            0.527_633_447_258_985_3,
            1e-15
        ));

        let model = Model::new(
            0.25,
            vec![spherical(2.0, Ranges::isotropic(100.0), Rotation::NONE)],
        )
        .unwrap();
        assert_eq!(model.gamma([0.0, 0.0, 0.0]), 0.0);
        assert_eq!(model.covariance([0.0, 0.0, 0.0]), 2.25);
        assert!(close(model.gamma([0.0, 1e-9, 0.0]), 0.25 + 3e-11, 1e-15));
        assert!(close(
            model.gamma([0.0, 50.0, 0.0]),
            0.25 + 2.0 * 0.6875,
            1e-15
        ));
        assert_eq!(model.gamma([0.0, 0.0, 400.0]), 2.25);
        assert_eq!(model.covariance([0.0, 0.0, 400.0]), 0.0);
    }

    #[test]
    fn test_rotations_follow_gslib_setrot() {
        // Azimuth 90 puts the major axis east, the semi-major axis north and the minor axis up.
        let east = Rotation {
            azimuth_deg: 90.0,
            dip_deg: 0.0,
            rake_deg: 0.0,
        }
        .axes();
        assert_eq!(east, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

        // Azimuth 0 and dip −30: the major axis points north and 30° down, and the minor axis leans north.
        let axes = Rotation {
            azimuth_deg: 0.0,
            dip_deg: -30.0,
            rake_deg: 0.0,
        }
        .axes();
        let expected = [
            [0.0, 0.75_f64.sqrt(), -0.5],
            [-1.0, 0.0, 0.0],
            [0.0, 0.5, 0.75_f64.sqrt()],
        ];
        for (row, want) in axes.iter().zip(expected) {
            for (a, b) in row.iter().zip(want) {
                assert!((a - b).abs() < 1e-15, "{axes:?}");
            }
        }

        // Rake 90 about a horizontal north axis turns the semi-major axis from west to up.
        let raked = Rotation {
            azimuth_deg: 0.0,
            dip_deg: 0.0,
            rake_deg: 90.0,
        }
        .axes();
        for (a, b) in raked[1].iter().zip([0.0, 0.0, 1.0]) {
            assert!((a - b).abs() < 1e-15, "{raked:?}");
        }
    }

    #[test]
    fn test_each_range_lies_on_its_axis_for_any_rotation() {
        let rotation = Rotation {
            azimuth_deg: 30.0,
            dip_deg: -40.0,
            rake_deg: 20.0,
        };
        let axes = rotation.axes();
        for (i, a) in axes.iter().enumerate() {
            for (j, b) in axes.iter().enumerate() {
                let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
                let want = if i == j { 1.0 } else { 0.0 };
                assert!((dot - want).abs() < 1e-15, "axes {i} and {j}: {dot}");
            }
        }
        let ranges = Ranges {
            major_m: 150.0,
            semi_m: 60.0,
            minor_m: 20.0,
        };
        let model = Model::new(0.0, vec![spherical(1.0, ranges, rotation)]).unwrap();
        for (axis, range) in axes.iter().zip(ranges.as_array()) {
            assert!(close(model.gamma(at(*axis, 0.5 * range)), 0.6875, 1e-14));
            assert!(close(model.gamma(at(*axis, range)), 1.0, 1e-14));
            assert!(close(model.gamma(at(*axis, -0.5 * range)), 0.6875, 1e-14));
        }
        // Halfway along the major axis and halfway along the minor axis: scaled distance √0.5.
        let lag = [
            axes[0][0] * 75.0 + axes[2][0] * 10.0,
            axes[0][1] * 75.0 + axes[2][1] * 10.0,
            axes[0][2] * 75.0 + axes[2][2] * 10.0,
        ];
        let h = 0.5_f64.sqrt();
        assert!(close(model.gamma(lag), h * (1.5 - 0.5 * h * h), 1e-14));
    }

    #[test]
    fn test_a_nested_anisotropic_covariance_is_positive_definite() {
        let model = Model::new(
            0.1,
            vec![
                spherical(
                    0.6,
                    Ranges {
                        major_m: 120.0,
                        semi_m: 50.0,
                        minor_m: 15.0,
                    },
                    Rotation {
                        azimuth_deg: 45.0,
                        dip_deg: -20.0,
                        rake_deg: 10.0,
                    },
                ),
                Structure {
                    shape: Shape::Exponential,
                    sill: 0.3,
                    ranges: Ranges::isotropic(300.0),
                    rotation: Rotation::NONE,
                },
            ],
        )
        .unwrap();
        // 60 points on a fixed pseudo-random pattern in a 200 m cube.
        let mut points = Vec::new();
        let mut s: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..60 {
            let mut p = [0.0; 3];
            for c in &mut p {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                *c = (s >> 11) as f64 / (1u64 << 53) as f64 * 200.0;
            }
            points.push(p);
        }
        let n = points.len();
        let mut c = vec![0.0; n * n];
        for (i, a) in points.iter().enumerate() {
            for (j, b) in points.iter().enumerate() {
                c[i * n + j] = model.covariance([b[0] - a[0], b[1] - a[1], b[2] - a[2]]);
            }
        }
        let x = fmine_numerics::cholesky_solve(&c, &vec![1.0; n], n).unwrap();
        assert!(x.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_models_refuse_parameters_outside_their_domain() {
        let ok = spherical(1.0, Ranges::isotropic(10.0), Rotation::NONE);
        let cases: Vec<(f64, Vec<Structure>, &str)> = vec![
            (-0.1, vec![ok], "model.nugget"),
            (f64::NAN, vec![ok], "model.nugget"),
            (
                0.0,
                vec![Structure { sill: -1.0, ..ok }],
                "model.structures[0].sill",
            ),
            (
                0.0,
                vec![
                    ok,
                    Structure {
                        ranges: Ranges {
                            semi_m: 0.0,
                            ..ok.ranges
                        },
                        ..ok
                    },
                ],
                "model.structures[1].ranges.semi_m",
            ),
            (
                0.0,
                vec![Structure {
                    ranges: Ranges::isotropic(f64::INFINITY),
                    ..ok
                }],
                "model.structures[0].ranges.major_m",
            ),
            (
                0.0,
                vec![Structure {
                    rotation: Rotation {
                        dip_deg: f64::NAN,
                        ..Rotation::NONE
                    },
                    ..ok
                }],
                "model.structures[0].rotation.dip_deg",
            ),
            (0.0, vec![ok; 5], "model.structures"),
        ];
        for (nugget, structures, field) in cases {
            match Model::new(nugget, structures) {
                Err(GeostatError::Invalid { field: f, .. }) => assert_eq!(f, field),
                other => panic!("{field}: {other:?}"),
            }
        }
    }
}
