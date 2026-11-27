//! Desurvey oracle tests (`BD-164`, PO-CORE-1: within 1 mm per 1,000 m of reference minimum-curvature results).
//!
//! Three references. Holes built from straight runs and circular arcs, where minimum curvature is exact, give
//! the true position at every depth. The textbook inclination-and-azimuth form of the method (as in Bourgoyne et al.,
//! *Applied Drilling Engineering*, SPE, 1986), written here independently with std trigonometry and an
//! arc-cosine dogleg, gives the station positions of seeded random 2 km holes. And a station added on the trace
//! must not move it.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use fmine_drill::desurvey::{CollarFrom, Direction, Point, Trace, desurvey};
use fmine_drill::records::{self, DrillError, Orientation, Survey, SurveyInstrument};
use fmine_evidence::RefusalCode;

#[allow(dead_code)]
#[path = "common/fixtures.rs"]
mod fixtures;

use fixtures::{collar, survey};

const HOLE: &str = "H1";

/// PO-CORE-1's tolerance at `depth_m` along the hole, plus 10 nm for rounding at UTM-sized coordinates.
fn tolerance_m(depth_m: f64) -> f64 {
    1e-3 * depth_m / 1000.0 + 1e-8
}

type V = [f64; 3];

fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: V, k: f64) -> V {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: V) -> V {
    scale(a, 1.0 / dot(a, a).sqrt())
}

/// East, north, up of an azimuth and dip in degrees.
fn direction(azimuth_deg: f64, dip_deg: f64) -> V {
    let (a, d) = (azimuth_deg.to_radians(), dip_deg.to_radians());
    [d.cos() * a.sin(), d.cos() * a.cos(), d.sin()]
}

/// Azimuth in [0, 360) and dip of a unit direction, degrees, with std trigonometry.
fn orientation(t: V) -> Orientation {
    let mut azimuth = t[0].atan2(t[1]).to_degrees();
    if azimuth < 0.0 {
        azimuth += 360.0;
    }
    if azimuth >= 360.0 {
        azimuth -= 360.0;
    }
    Orientation {
        azimuth_deg: azimuth,
        dip_deg: t[2].clamp(-1.0, 1.0).asin().to_degrees(),
        sigma_azimuth_deg: 0.5,
        sigma_dip_deg: 0.25,
    }
}

fn station(id: &str, depth_mm: u64, o: Orientation) -> Survey {
    Survey {
        orientation: Some(o),
        ..survey(id, HOLE, depth_mm)
    }
}

fn distance(p: Point, q: Point) -> f64 {
    let d = [
        p.easting_m - q.easting_m,
        p.northing_m - q.northing_m,
        p.elevation_m - q.elevation_m,
    ];
    dot(d, d).sqrt()
}

fn collar_point() -> Point {
    let c = collar(HOLE).actual.unwrap();
    Point {
        easting_m: c.easting_m,
        northing_m: c.northing_m,
        elevation_m: c.elevation_m,
    }
}

fn shifted(p: Point, offset: V) -> Point {
    Point {
        easting_m: p.easting_m + offset[0],
        northing_m: p.northing_m + offset[1],
        elevation_m: p.elevation_m + offset[2],
    }
}

/// A run of a hole: straight (`radius` infinite) or a circular arc turning towards `normal`.
#[derive(Clone, Copy)]
struct Run {
    from_m: f64,
    length_m: f64,
    start: V,
    tangent: V,
    normal: V,
    radius_m: f64,
}

impl Run {
    fn at(&self, s: f64) -> (V, V) {
        if self.radius_m.is_infinite() {
            return (add(self.start, scale(self.tangent, s)), self.tangent);
        }
        let (r, a) = (self.radius_m, s / self.radius_m);
        let point = add(
            self.start,
            add(
                scale(self.tangent, r * a.sin()),
                scale(self.normal, r * (1.0 - a.cos())),
            ),
        );
        (
            point,
            add(scale(self.tangent, a.cos()), scale(self.normal, a.sin())),
        )
    }
}

/// A tangent-continuous hole made of runs, as offsets from the collar.
struct Path {
    runs: Vec<Run>,
}

impl Path {
    fn new() -> Self {
        Self { runs: Vec::new() }
    }

    fn end(&self, first: V) -> (f64, V, V) {
        self.runs.last().map_or((0.0, [0.0; 3], first), |r| {
            let (p, t) = r.at(r.length_m);
            (r.from_m + r.length_m, p, t)
        })
    }

    fn push(mut self, first: V, length_mm: u64, toward: V, build_deg_per_30m: f64) -> Self {
        let (from_m, start, tangent) = self.end(unit(first));
        let normal = unit(add(toward, scale(tangent, -dot(toward, tangent))));
        let radius_m = if build_deg_per_30m == 0.0 {
            f64::INFINITY
        } else {
            30.0 / build_deg_per_30m.to_radians()
        };
        self.runs.push(Run {
            from_m,
            length_m: length_mm as f64 / 1000.0,
            start,
            tangent,
            normal,
            radius_m,
        });
        self
    }

    fn at(&self, depth_m: f64) -> (V, V) {
        let run = self
            .runs
            .iter()
            .rev()
            .find(|r| r.from_m <= depth_m)
            .unwrap_or(&self.runs[0]);
        run.at(depth_m - run.from_m)
    }

    /// Run boundaries, mm.
    fn joints_mm(&self) -> Vec<u64> {
        let mut at = vec![0];
        for r in &self.runs {
            at.push((((r.from_m + r.length_m) * 1000.0).round()) as u64);
        }
        at
    }

    fn length_mm(&self) -> u64 {
        *self.joints_mm().last().unwrap()
    }

    /// Stations at every joint and every `spacing_mm` between them, read from the path.
    fn surveys(&self, spacing_mm: u64) -> Vec<Survey> {
        let mut depths = self.joints_mm();
        let mut d = spacing_mm;
        while d < self.length_mm() {
            depths.push(d);
            d += spacing_mm;
        }
        depths.sort_unstable();
        depths.dedup();
        depths
            .into_iter()
            .map(|mm| {
                station(
                    &format!("S-{mm}"),
                    mm,
                    orientation(self.at(mm as f64 / 1000.0).1),
                )
            })
            .collect()
    }
}

/// The largest distance, over every 250 mm of the hole and every station, between the trace and the path, and
/// a check that each point is within the tolerance at its depth.
fn worst_error(trace: &Trace, path: &Path, what: &str) -> f64 {
    let origin = collar_point();
    let mut depths: Vec<u64> = (0..=path.length_mm() + 20_000).step_by(250).collect();
    depths.extend(trace.stations().iter().map(|s| s.depth_mm));
    let mut worst = 0.0_f64;
    for mm in depths {
        let depth_m = mm as f64 / 1000.0;
        let (offset, tangent) = if mm <= path.length_mm() {
            path.at(depth_m)
        } else {
            let (end, t) = path.at(path.length_mm() as f64 / 1000.0);
            (
                add(end, scale(t, depth_m - path.length_mm() as f64 / 1000.0)),
                t,
            )
        };
        let located = trace.at(mm);
        let err = distance(located.point, shifted(origin, offset));
        assert!(
            err <= tolerance_m(depth_m),
            "{what}: {err:e} m off at {depth_m} m (tolerance {:e} m)",
            tolerance_m(depth_m)
        );
        let d = located.direction;
        let turn = 1.0 - dot([d.east, d.north, d.up], tangent);
        assert!(
            turn < 1e-12,
            "{what}: direction off by 1 - cos = {turn:e} at {depth_m} m"
        );
        worst = worst.max(err);
    }
    worst
}

#[test]
fn test_holes_of_arcs_and_straight_runs_desurvey_to_their_true_positions() {
    let east = [1.0, 0.0, 0.0];
    let north = [0.0, 1.0, 0.0];
    let up = [0.0, 0.0, 1.0];
    let down = [0.0, 0.0, -1.0];
    let cases: Vec<(&str, Path, u64)> = vec![
        (
            "one arc, 1°/30 m, building up",
            Path::new().push(direction(87.5, -62.0), 1_500_000, up, 1.0),
            30_000,
        ),
        (
            "one arc, 0.5°/30 m, turning right, irregular stations",
            Path::new().push(direction(355.0, -70.0), 1_200_000, east, 0.5),
            47_300,
        ),
        (
            "one arc, 10°/30 m, through vertical",
            Path::new().push(direction(10.0, -80.0), 90_000, down, 10.0),
            9_100,
        ),
        (
            "straight, arc, arc in another plane, straight (S-shape)",
            Path::new()
                .push(direction(140.0, -75.0), 100_000, up, 0.0)
                .push([0.0; 3], 300_000, up, 3.0)
                .push([0.0; 3], 500_000, north, 2.0)
                .push([0.0; 3], 400_000, east, 0.0),
            30_000,
        ),
        (
            "near-vertical hole wandering across north",
            Path::new()
                .push(direction(2.0, -89.5), 400_000, scale(east, -1.0), 1.5)
                .push([0.0; 3], 400_000, east, 1.0),
            25_000,
        ),
    ];
    for (what, path, spacing) in cases {
        let trace = desurvey(&collar(HOLE), &path.surveys(spacing)).unwrap();
        assert_eq!(trace.collar_from(), CollarFrom::Actual);
        assert_eq!(trace.crs(), "EPSG:32735");
        let worst = worst_error(&trace, &path, what);
        eprintln!("{what}: worst {worst:e} m");
        assert!(worst < 1e-6, "{what}: worst {worst:e} m");
    }
}

/// Station positions by the textbook form: inclination `I` from vertical, `cos β = cos(I2 − I1) −
/// sin I1 sin I2 (1 − cos(A2 − A1))`, `RF = 2/β tan(β/2)`, and the north, east and vertical steps
/// `ΔMD/2 · (f(I1, A1) + f(I2, A2)) · RF`. Offsets east, north, up from the collar. With `ratio_factor` false,
/// `RF = 1`: the balanced-tangential method, the negative control.
fn textbook(surveys: &[Survey], ratio_factor: bool) -> Vec<V> {
    let mut out: Vec<V> = Vec::new();
    let mut prev: Option<(f64, f64, f64)> = None;
    for s in surveys {
        let o = s.orientation.unwrap();
        let md = s.depth_mm as f64 / 1000.0;
        let i2 = (90.0 + o.dip_deg).to_radians();
        let a2 = o.azimuth_deg.to_radians();
        let next = match (prev, out.last()) {
            (Some((md1, i1, a1)), Some(at)) => {
                let cos_beta = (i2 - i1).cos() - i1.sin() * i2.sin() * (1.0 - (a2 - a1).cos());
                let beta = cos_beta.clamp(-1.0, 1.0).acos();
                let rf = if beta < 1e-7 || !ratio_factor {
                    1.0
                } else {
                    2.0 / beta * (beta / 2.0).tan()
                };
                let half = (md - md1) / 2.0 * rf;
                [
                    at[0] + half * (i1.sin() * a1.sin() + i2.sin() * a2.sin()),
                    at[1] + half * (i1.sin() * a1.cos() + i2.sin() * a2.cos()),
                    at[2] - half * (i1.cos() + i2.cos()),
                ]
            }
            _ => [
                md * i2.sin() * a2.sin(),
                md * i2.sin() * a2.cos(),
                -md * i2.cos(),
            ],
        };
        out.push(next);
        prev = Some((md, i2, a2));
    }
    out
}

/// splitmix64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn between(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

fn wrap(azimuth: f64) -> f64 {
    let a = azimuth.rem_euclid(360.0);
    if a >= 360.0 { 0.0 } else { a }
}

#[test]
fn test_random_two_kilometre_holes_match_the_textbook_form() {
    let origin = collar_point();
    let mut rng = Rng(0x00F0_5EED_0164);
    let mut worst_per_km = 0.0_f64;
    let mut control_per_km = 0.0_f64;
    for hole in 0..60 {
        // Every third hole has doglegs of up to 20° between stations; the rest up to about 3°.
        let swing = if hole % 3 == 0 { 20.0 } else { 3.0 };
        let mut azimuth = rng.between(0.0, 360.0);
        let mut dip = rng.between(-90.0, -45.0);
        let spacing_mm = [30_000, 12_500, 50_000][hole % 3];
        let mut surveys = Vec::new();
        let mut depth = rng.between(0.0, 20.0).round() as u64 * 1000;
        while depth <= 2_000_000 {
            surveys.push(station(
                &format!("S-{depth}"),
                depth,
                Orientation {
                    azimuth_deg: azimuth,
                    dip_deg: dip,
                    sigma_azimuth_deg: 1.0,
                    sigma_dip_deg: 0.5,
                },
            ));
            azimuth = wrap(azimuth + rng.between(-swing, swing));
            dip += rng.between(-swing / 2.0, swing / 2.0);
            if dip < -90.0 {
                dip = -180.0 - dip;
                azimuth = wrap(azimuth + 180.0);
            }
            dip = dip.min(-5.0);
            depth += spacing_mm;
        }
        let trace = desurvey(&collar(HOLE), &surveys).unwrap();
        for (s, tangential) in trace.stations().iter().zip(textbook(&surveys, false)) {
            let depth_m = s.depth_mm as f64 / 1000.0;
            if depth_m > 0.0 {
                let err = distance(s.point, shifted(origin, tangential));
                control_per_km = control_per_km.max(err / depth_m * 1000.0);
            }
        }
        for (s, expected) in trace.stations().iter().zip(textbook(&surveys, true)) {
            let depth_m = s.depth_mm as f64 / 1000.0;
            let err = distance(s.point, shifted(origin, expected));
            assert!(
                err <= tolerance_m(depth_m),
                "hole {hole}: station at {depth_m} m is {err:e} m from the textbook"
            );
            if depth_m > 0.0 {
                worst_per_km = worst_per_km.max(err / depth_m * 1000.0);
            }
        }
    }
    eprintln!(
        "textbook: worst {worst_per_km:e} m per 1,000 m; balanced tangential (control) {control_per_km:e} m"
    );
    assert!(worst_per_km < 1e-6, "worst {worst_per_km:e} m per 1,000 m");
    assert!(
        control_per_km > 1e-3,
        "the comparison cannot tell RF = 1: {control_per_km:e}"
    );
}

#[test]
fn test_a_station_added_on_the_trace_moves_nothing_and_input_order_does_not_matter() {
    let path = Path::new()
        .push(direction(140.0, -75.0), 100_000, [0.0, 0.0, 1.0], 0.0)
        .push([0.0; 3], 300_000, [0.0, 0.0, 1.0], 4.0)
        .push([0.0; 3], 500_000, [0.0, 1.0, 0.0], 2.5);
    let surveys = path.surveys(60_000);
    let trace = desurvey(&collar(HOLE), &surveys).unwrap();

    let mut more = surveys.clone();
    for mm in [7_000_u64, 133_333, 250_001, 777_777, 950_000] {
        let o = trace.at(mm).direction.azimuth_dip_deg();
        more.push(station(
            &format!("X-{mm}"),
            mm,
            Orientation {
                azimuth_deg: o.0,
                dip_deg: o.1,
                sigma_azimuth_deg: 0.5,
                sigma_dip_deg: 0.25,
            },
        ));
    }
    more.reverse();
    let denser = desurvey(&collar(HOLE), &more).unwrap();
    for mm in (0..=1_000_000).step_by(1_000) {
        let err = distance(trace.at(mm).point, denser.at(mm).point);
        assert!(err < 1e-8, "{err:e} m at {mm} mm");
    }

    let mut shuffled = surveys.clone();
    shuffled.reverse();
    shuffled.swap(1, 4);
    assert_eq!(desurvey(&collar(HOLE), &shuffled).unwrap(), trace);
}

#[test]
fn test_beyond_the_stations_the_hole_runs_straight_and_says_so() {
    let at_50 = station("S-1", 50_000, orientation(direction(90.0, -60.0)));
    let trace = desurvey(&collar(HOLE), std::slice::from_ref(&at_50)).unwrap();
    let origin = collar_point();
    let t = Direction::of(&at_50.orientation.unwrap());
    for (mm, extrapolated) in [
        (0, true),
        (20_000, true),
        (50_000, false),
        (1_000_000, true),
    ] {
        let located = trace.at(mm);
        let d = mm as f64 / 1000.0;
        let expected = shifted(origin, [t.east * d, t.north * d, t.up * d]);
        assert!(distance(located.point, expected) < 1e-8, "{mm} mm");
        assert_eq!(located.extrapolated, extrapolated, "{mm} mm");
    }
    assert_eq!(trace.at(0).point, origin);

    let vertical = station("S-1", 0, orientation([0.0, 0.0, -1.0]));
    let trace = desurvey(&collar(HOLE), &[vertical]).unwrap();
    let bottom = trace.at(1_000_000).point;
    assert!((bottom.elevation_m - (origin.elevation_m - 1000.0)).abs() < 1e-9);
    assert!(
        distance(
            Point {
                elevation_m: 0.0,
                ..bottom
            },
            Point {
                elevation_m: 0.0,
                ..origin
            }
        ) < 1e-9
    );

    // Between stations nothing is extrapolated; past the last one it is.
    let path = Path::new().push(direction(30.0, -55.0), 300_000, [0.0, 0.0, 1.0], 2.0);
    let trace = desurvey(&collar(HOLE), &path.surveys(100_000)).unwrap();
    assert!(!trace.at(150_000).extrapolated);
    assert!(!trace.at(300_000).extrapolated);
    assert!(trace.at(300_001).extrapolated);
}

#[test]
fn test_a_collar_with_only_a_planned_position_and_suspect_stations_are_reported() {
    let mut c = collar(HOLE);
    let planned = c.planned.clone().unwrap();
    c.actual = None;
    let mut magnetic = station("S-2", 30_000, orientation(direction(88.0, -61.0)));
    magnetic.instrument = SurveyInstrument::MagneticMultishot;
    magnetic.magnetic_interference = true;
    let trace = desurvey(&c, &[survey("S-1", HOLE, 0), magnetic]).unwrap();
    assert_eq!(trace.collar_from(), CollarFrom::Planned);
    assert_eq!(trace.collar().easting_m, planned.easting_m);
    let suspect: Vec<bool> = trace.stations().iter().map(|s| s.suspect).collect();
    assert_eq!(suspect, [false, true]);
}

fn refused(surveys: &[Survey]) -> DrillError {
    desurvey(&collar(HOLE), surveys).unwrap_err()
}

#[test]
fn test_holes_that_cannot_be_desurveyed_are_refused() {
    let unsurveyed = Survey {
        instrument: SurveyInstrument::None,
        orientation: None,
        ..survey("S-0", HOLE, 0)
    };
    let mut other_run = survey("S-2b", HOLE, 30_000);
    other_run.run = records::RunDirection::In;
    let mut out_of_range = survey("S-9", HOLE, 90_000);
    out_of_range.orientation.as_mut().unwrap().azimuth_deg = 360.0;
    let cases: Vec<(&str, Vec<Survey>, &str)> = vec![
        ("no stations", vec![], "surveys"),
        ("declared unsurveyed", vec![unsurveyed.clone()], "surveys"),
        (
            "declared unsurveyed with a surveyed station",
            vec![unsurveyed, survey("S-1", HOLE, 10_000)],
            "surveys",
        ),
        (
            "a station of another hole",
            vec![survey("S-1", HOLE, 0), survey("S-2", "H2", 30_000)],
            "survey.hole_id",
        ),
        (
            "two runs at one depth",
            vec![survey("S-2", HOLE, 30_000), other_run],
            "survey.depth_mm",
        ),
        (
            "opposite directions",
            vec![
                station("S-1", 0, orientation(direction(90.0, -60.0))),
                station("S-2", 30_000, orientation(direction(270.0, 60.0))),
            ],
            "survey.orientation",
        ),
        (
            "an azimuth of 360",
            vec![out_of_range],
            "survey.azimuth_deg",
        ),
    ];
    for (what, surveys, field) in cases {
        let err = refused(&surveys);
        assert!(
            matches!(&err, DrillError::Invalid { field: f, .. } if f == field),
            "{what}: {err:?}"
        );
        assert_eq!(
            err.to_refusal().code,
            RefusalCode::OutsidePhysicalDomain109.spec().code,
            "{what}"
        );
    }
    let mut no_position = collar(HOLE);
    no_position.actual = None;
    no_position.planned = None;
    assert!(desurvey(&no_position, &[survey("S-1", HOLE, 0)]).is_err());
}
