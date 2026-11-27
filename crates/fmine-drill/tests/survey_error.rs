//! Survey-error propagation (`bd-zx57`'s done-when: covariance at each station and at any depth from the records'
//! sigmas with declared error terms; checked against the closed form for a straight hole, and against a Monte
//! Carlo of perturbed surveys within its sampling error).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp
)]

use fmine_drill::desurvey::{Direction, Point, desurvey};
use fmine_drill::records::{
    self, Collar, DrillError, Orientation, RunDirection, Survey, SurveyInstrument,
};
use fmine_drill::survey_error::{Angle, Covariance, ErrorModel, ErrorTerm, Propagation, propagate};

#[allow(dead_code)]
#[path = "common/fixtures.rs"]
mod fixtures;

use fixtures::{collar, survey};

const DEG: f64 = std::f64::consts::PI / 180.0;

#[allow(clippy::too_many_arguments)]
fn station(
    id: &str,
    hole: &str,
    depth_mm: u64,
    azimuth_deg: f64,
    dip_deg: f64,
    sigma: (f64, f64),
    instrument: SurveyInstrument,
    run: RunDirection,
) -> Survey {
    Survey {
        instrument,
        run,
        orientation: Some(Orientation {
            azimuth_deg,
            dip_deg,
            sigma_azimuth_deg: sigma.1,
            sigma_dip_deg: sigma.0,
        }),
        ..survey(id, hole, depth_mm)
    }
}

fn term(code: &str, angle: Angle, propagation: Propagation, variance_share: f64) -> ErrorTerm {
    ErrorTerm {
        code: code.to_string(),
        angle,
        propagation,
        variance_share,
    }
}

/// Dip 36% random and 64% systematic; azimuth all random.
fn mixed() -> ErrorModel {
    ErrorModel {
        terms: vec![
            term("DIP-R", Angle::Dip, Propagation::Random, 0.36),
            term("DIP-S", Angle::Dip, Propagation::Systematic, 0.64),
            term("AZ-R", Angle::Azimuth, Propagation::Random, 1.0),
        ],
    }
}

fn outer(u: [f64; 3], v: [f64; 3], k: f64) -> [[f64; 3]; 3] {
    let mut m = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = k * u[i] * v[j];
        }
    }
    m
}

fn plus(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut m = a;
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] += b[i][j];
        }
    }
    m
}

/// Each station's weight in the point at depth `d` (m) of a straight hole, to first order: half of each leg it
/// ends or starts, the run from the collar for the first station, and within a leg a fraction `f` of length `h`
/// gives `f·h·(2 − f)/2` to the upper station and `f²·h/2` to the lower.
fn weights(depths: &[f64], d: f64) -> Vec<f64> {
    let mut w = vec![0.0; depths.len()];
    if d < depths[0] {
        w[0] = d;
        return w;
    }
    w[0] = depths[0];
    let i = depths.iter().rposition(|&s| s <= d).unwrap();
    for k in 0..i {
        let h = depths[k + 1] - depths[k];
        w[k] += h / 2.0;
        w[k + 1] += h / 2.0;
    }
    let along = d - depths[i];
    if i + 1 == depths.len() {
        w[i] += along;
    } else {
        let h = depths[i + 1] - depths[i];
        let f = along / h;
        w[i] += f * h * (2.0 - f) / 2.0;
        w[i + 1] += f * f * h / 2.0;
    }
    w
}

/// The closed-form covariance of a straight hole at dip `dip` and azimuth `azimuth` (degrees), with station
/// weights `w`, the stations' runs, sigmas `(dip, azimuth)` in degrees, and collar sigmas `(h, v)`.
fn closed_form(
    model: &ErrorModel,
    w: &[f64],
    runs: &[usize],
    (azimuth, dip): (f64, f64),
    sigma: (f64, f64),
    (h, v): (f64, f64),
) -> [[f64; 3]; 3] {
    let (sd, cd) = (dip * DEG).sin_cos();
    let (sa, ca) = (azimuth * DEG).sin_cos();
    // ∂t/∂dip is a unit vector; ∂t/∂azimuth has length cos(dip).
    let u_dip = [-sd * sa, -sd * ca, cd];
    let u_az = [cd * ca, -cd * sa, 0.0];
    let random: f64 = w.iter().map(|x| x * x).sum();
    let n_runs = runs.iter().max().unwrap() + 1;
    let mut by_run = vec![0.0; n_runs];
    for (x, r) in w.iter().zip(runs) {
        by_run[*r] += x;
    }
    let systematic: f64 = by_run.iter().map(|x| x * x).sum();
    let mut m = [[h * h, 0.0, 0.0], [0.0, h * h, 0.0], [0.0, 0.0, v * v]];
    for t in &model.terms {
        let (u, s) = match t.angle {
            Angle::Dip => (u_dip, sigma.0 * DEG),
            Angle::Azimuth => (u_az, sigma.1 * DEG),
        };
        let w2 = match t.propagation {
            Propagation::Random => random,
            Propagation::Systematic => systematic,
        };
        m = plus(m, outer(u, u, t.variance_share * s * s * w2));
    }
    m
}

fn worst_relative(got: &Covariance, want: &[[f64; 3]; 3]) -> f64 {
    let scale = want[0][0] + want[1][1] + want[2][2];
    let mut worst = 0.0_f64;
    for i in 0..3 {
        for j in 0..3 {
            worst = worst.max((got.enu_m2[i][j] - want[i][j]).abs() / scale);
        }
    }
    worst
}

#[test]
fn test_a_straight_hole_has_its_closed_form_covariance_at_stations_and_between_them() {
    let (azimuth, dip, sigma) = (45.0, -60.0, (0.25, 0.5));
    let c = collar("S1");
    let collar_sigma = (0.05, 0.1);
    let mut worst = 0.0_f64;
    // Stations at 0, 30 … 300 m; the upper half gyro going in, the lower half coming out: two runs.
    let depths_mm: Vec<u64> = (0..=10).map(|k| k * 30_000).collect();
    let runs: Vec<usize> = depths_mm
        .iter()
        .map(|&d| usize::from(d > 150_000))
        .collect();
    let surveys: Vec<Survey> = depths_mm
        .iter()
        .zip(&runs)
        .map(|(&d, &r)| {
            let run = if r == 0 {
                RunDirection::In
            } else {
                RunDirection::Out
            };
            station(
                &format!("s{d}"),
                "S1",
                d,
                azimuth,
                dip,
                sigma,
                SurveyInstrument::Gyro,
                run,
            )
        })
        .collect();
    let depths: Vec<f64> = depths_mm.iter().map(|&d| d as f64 / 1000.0).collect();
    let probes = [
        0, 30_000, 45_000, 123_456, 150_000, 165_000, 299_999, 300_000, 350_000,
    ];
    for model in [ErrorModel::random(), ErrorModel::systematic(), mixed()] {
        let hole = propagate(&c, &surveys, &model).unwrap();
        assert_eq!(hole.at_stations().len(), depths_mm.len());
        for &d in &probes {
            let got = hole.covariance_at(d);
            assert_eq!(hole.checked_covariance_at(d).unwrap(), got);
            let want = closed_form(
                &model,
                &weights(&depths, d as f64 / 1000.0),
                &runs,
                (azimuth, dip),
                sigma,
                collar_sigma,
            );
            let r = worst_relative(&got.total, &want);
            assert!(r < 1e-9, "{model:?} at {d} mm: {got:?} vs {want:?} ({r:e})");
            worst = worst.max(r);
            // The terms and the collar add up to the total.
            let mut sum = got.collar.enu_m2;
            for t in &got.terms {
                sum = plus(sum, t.covariance.enu_m2);
            }
            assert!(worst_relative(&got.total, &sum) < 1e-15);
            // Along a straight hole the angles move nothing: only the collar's accuracy is left.
            let t = Direction::of(&Orientation {
                azimuth_deg: azimuth,
                dip_deg: dip,
                sigma_azimuth_deg: 0.0,
                sigma_dip_deg: 0.0,
            });
            let along = collar_sigma.0.powi(2) * (t.east * t.east + t.north * t.north)
                + collar_sigma.1.powi(2) * t.up * t.up;
            assert!(
                (got.total.variance_along(t) - along).abs() < 1e-9 * along,
                "{d}"
            );
            assert!((hole.checked_variance_along(d, t).unwrap() - along).abs() < 1e-9 * along);
            // An oblique linear projection exercises every covariance component under each model.
            let u = Direction {
                east: 0.3,
                north: -0.4,
                up: 0.5,
            };
            let want_variance = Covariance { enu_m2: want }.variance_along(u);
            let got_variance = hole.checked_variance_along(d, u).unwrap();
            assert!((got_variance - want_variance).abs() < 1e-9 * want_variance);
        }
        // At 300 m, all random: σ_dip·h·√(n − ½) across the hole, ten 30 m legs.
        if model == ErrorModel::random() {
            let lateral = 0.25 * DEG * 30.0 * 9.5_f64.sqrt();
            let p = hole.covariance_at(300_000);
            let (sd, cd) = (dip * DEG).sin_cos();
            let (sa, ca) = (azimuth * DEG).sin_cos();
            let across = Direction {
                east: -sd * sa,
                north: -sd * ca,
                up: cd,
            };
            let want = lateral * lateral
                + collar_sigma.0.powi(2) * (sd * sd)
                + collar_sigma.1.powi(2) * cd * cd;
            assert!((p.total.variance_along(across) - want).abs() < 1e-9 * want);
        }
    }

    // First station at 50 m: above it the hole runs straight along that station's direction.
    let late: Vec<Survey> = (1..=6)
        .map(|k| {
            station(
                &format!("l{k}"),
                "S1",
                k * 50_000,
                azimuth,
                dip,
                sigma,
                SurveyInstrument::Gyro,
                RunDirection::Out,
            )
        })
        .collect();
    let late_depths: Vec<f64> = (1..=6).map(|k| f64::from(k) * 50.0).collect();
    for model in [ErrorModel::random(), ErrorModel::systematic(), mixed()] {
        let hole = propagate(&c, &late, &model).unwrap();
        for d in [0, 20_000, 50_000, 75_000, 300_000, 320_000] {
            let want = closed_form(
                &model,
                &weights(&late_depths, d as f64 / 1000.0),
                &[0; 6],
                (azimuth, dip),
                sigma,
                collar_sigma,
            );
            let r = worst_relative(&hole.covariance_at(d).total, &want);
            assert!(r < 1e-9, "{model:?} at {d} mm: {r:e}");
            worst = worst.max(r);
        }
    }
    println!("straight holes: worst element off the closed form by {worst:e} of the trace");
}

/// xorshift64* from a splitmix64 seed, with Box–Muller normals.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Self((z ^ (z >> 31)).max(1))
    }

    fn uniform(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let x = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11;
        (x as f64 + 0.5) / (1_u64 << 53) as f64
    }

    fn normal(&mut self) -> f64 {
        let (u, v) = (self.uniform(), self.uniform());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

fn offset(p: Point, nominal: Point) -> [f64; 3] {
    [
        p.easting_m - nominal.easting_m,
        p.northing_m - nominal.northing_m,
        p.elevation_m - nominal.elevation_m,
    ]
}

/// The largest `|S − Σ|` over the matrix in standard errors of a sample covariance of `n` normal draws,
/// `√((Σᵢᵢ·Σⱼⱼ + Σᵢⱼ²)/n)`.
fn worst_z(sample: &[[f64; 3]; 3], model: &Covariance, n: usize) -> f64 {
    let m = &model.enu_m2;
    let mut worst = 0.0_f64;
    for i in 0..3 {
        for j in 0..3 {
            let se = ((m[i][i] * m[j][j] + m[i][j] * m[i][j]) / n as f64).sqrt();
            worst = worst.max((sample[i][j] - m[i][j]).abs() / se);
        }
    }
    worst
}

#[test]
fn test_a_curved_hole_matches_a_monte_carlo_of_perturbed_surveys() {
    const DRAWS: usize = 20_000;
    // 400 m, stations every 25 m, steepening from −55° to −80° and turning from 60° to 110°. The upper nine by
    // gyro going in (0.25°, 0.5°), the lower eight by magnetic multishot coming out (0.3°, 1.0°).
    let c = collar("C1");
    let surveys: Vec<Survey> = (0..=16_u32)
        .map(|k| {
            let f = f64::from(k) / 16.0;
            let (instrument, run, sigma) = if k <= 8 {
                (SurveyInstrument::Gyro, RunDirection::In, (0.25, 0.5))
            } else {
                (
                    SurveyInstrument::MagneticMultishot,
                    RunDirection::Out,
                    (0.3, 1.0),
                )
            };
            station(
                &format!("c{k}"),
                "C1",
                u64::from(k) * 25_000,
                60.0 + 50.0 * f,
                -55.0 - 25.0 * f,
                sigma,
                instrument,
                run,
            )
        })
        .collect();
    let model = ErrorModel {
        terms: vec![
            term("DIP-R", Angle::Dip, Propagation::Random, 0.5),
            term("DIP-S", Angle::Dip, Propagation::Systematic, 0.5),
            term("AZ-R", Angle::Azimuth, Propagation::Random, 0.25),
            term("AZ-S", Angle::Azimuth, Propagation::Systematic, 0.75),
        ],
    };
    let hole = propagate(&c, &surveys, &model).unwrap();
    let probes = [0_u64, 12_500, 100_000, 212_345, 400_000, 437_500];
    let nominal: Vec<Point> = probes.iter().map(|&d| hole.trace().at(d).point).collect();

    // Draw the errors as the model declares them and desurvey each perturbed hole with `desurvey`.
    let mut rng = Rng::new(0x2A57);
    let run_of = |k: usize| usize::from(k > 8);
    let mut sums = vec![[[0.0_f64; 3]; 3]; probes.len()];
    let mut means = vec![[0.0_f64; 3]; probes.len()];
    let mut draws: Vec<Vec<[f64; 3]>> = vec![Vec::with_capacity(DRAWS); probes.len()];
    for _ in 0..DRAWS {
        let shared = [[rng.normal(), rng.normal()], [rng.normal(), rng.normal()]];
        let perturbed: Vec<Survey> = surveys
            .iter()
            .enumerate()
            .map(|(k, s)| {
                let mut s = s.clone();
                let o = s.orientation.as_mut().unwrap();
                let sys = shared[run_of(k)];
                o.dip_deg +=
                    o.sigma_dip_deg * (0.5_f64.sqrt() * rng.normal() + 0.5_f64.sqrt() * sys[0]);
                o.azimuth_deg +=
                    o.sigma_azimuth_deg * (0.5 * rng.normal() + 0.75_f64.sqrt() * sys[1]);
                o.azimuth_deg = o.azimuth_deg.rem_euclid(360.0);
                s
            })
            .collect();
        let mut moved: Collar = c.clone();
        let p = moved.actual.as_mut().unwrap();
        p.easting_m += p.sigma_horizontal_m * rng.normal();
        p.northing_m += p.sigma_horizontal_m * rng.normal();
        p.elevation_m += p.sigma_vertical_m * rng.normal();
        let trace = desurvey(&moved, &perturbed).unwrap();
        for (k, &d) in probes.iter().enumerate() {
            let x = offset(trace.at(d).point, nominal[k]);
            for i in 0..3 {
                means[k][i] += x[i];
            }
            draws[k].push(x);
        }
    }
    for k in 0..probes.len() {
        for m in &mut means[k] {
            *m /= DRAWS as f64;
        }
        for x in &draws[k] {
            for i in 0..3 {
                for j in 0..3 {
                    sums[k][i][j] += (x[i] - means[k][i]) * (x[j] - means[k][j]);
                }
            }
        }
        for row in &mut sums[k] {
            for v in row.iter_mut() {
                *v /= (DRAWS - 1) as f64;
            }
        }
    }

    let control = propagate(&c, &surveys, &ErrorModel::random()).unwrap();
    let (mut worst, mut control_worst) = (0.0_f64, 0.0_f64);
    for (k, &d) in probes.iter().enumerate() {
        let z = worst_z(&sums[k], &hole.covariance_at(d).total, DRAWS);
        let zc = worst_z(&sums[k], &control.covariance_at(d).total, DRAWS);
        let sigma = hole.covariance_at(d).total.sigma_m();
        println!("{d:>7} mm: sigma ENU {sigma:.3?} m; worst z {z:.2}, all-random control {zc:.1}");
        assert!(
            z < 4.5,
            "at {d} mm the sample is {z:.2} standard errors off"
        );
        worst = worst.max(z);
        if d > 0 {
            control_worst = control_worst.max(zc);
        }
    }
    // The control, which takes the systematic shares as random, is far outside the sampling error.
    assert!(control_worst > 20.0, "{control_worst}");
    println!(
        "worst z {worst:.2} over {} elements; control {control_worst:.1}",
        probes.len() * 9
    );
}

#[test]
fn test_a_model_that_does_not_use_each_sigma_whole_is_refused() {
    let c = collar("H1");
    let surveys = [survey("a", "H1", 0), survey("b", "H1", 100_000)];
    let field = |e: DrillError| match e {
        DrillError::Invalid { field, .. } => field,
        other => panic!("{other:?}"),
    };
    let refused =
        |terms: Vec<ErrorTerm>| field(propagate(&c, &surveys, &ErrorModel { terms }).unwrap_err());
    let r = |code: &str, angle, share| term(code, angle, Propagation::Random, share);
    // An angle's shares that sum to 0.9, to 1.1, or are missing; a share of 0, NaN or 1.5; a blank or repeated code.
    for terms in [
        vec![r("D1", Angle::Dip, 0.9), r("A1", Angle::Azimuth, 1.0)],
        vec![
            r("D1", Angle::Dip, 0.6),
            r("D2", Angle::Dip, 0.5),
            r("A1", Angle::Azimuth, 1.0),
        ],
        vec![r("D1", Angle::Dip, 1.0)],
        vec![],
        vec![
            r("D1", Angle::Dip, 1.0),
            r("A0", Angle::Azimuth, 0.0),
            r("A1", Angle::Azimuth, 1.0),
        ],
        vec![r("D1", Angle::Dip, f64::NAN), r("A1", Angle::Azimuth, 1.0)],
        vec![r("D1", Angle::Dip, 1.5), r("A1", Angle::Azimuth, 1.0)],
        vec![r(" ", Angle::Dip, 1.0), r("A1", Angle::Azimuth, 1.0)],
        vec![r("X", Angle::Dip, 1.0), r("X", Angle::Azimuth, 1.0)],
    ] {
        assert_eq!(refused(terms.clone()), "survey_error.model", "{terms:?}");
    }
    // Shares of 0.7, 0.2 and 0.1 sum to 1 within rounding and are admitted.
    let ok = vec![
        r("D1", Angle::Dip, 0.7),
        term("D2", Angle::Dip, Propagation::Systematic, 0.2),
        r("D3", Angle::Dip, 0.1),
        r("A1", Angle::Azimuth, 1.0),
    ];
    assert!(propagate(&c, &surveys, &ErrorModel { terms: ok }).is_ok());
    // desurvey's own refusals pass through.
    assert_eq!(
        field(propagate(&c, &[], &ErrorModel::random()).unwrap_err()),
        "surveys"
    );
}

#[test]
fn test_checked_projection_preserves_a_rank_one_null_direction() {
    let mut c = collar("H1");
    let p = c.actual.as_mut().unwrap();
    p.sigma_horizontal_m = 0.0;
    p.sigma_vertical_m = 0.0;
    // Only azimuth is uncertain: the positional covariance is rank one. A matrix quadratic form in
    // its null direction can round below zero; projecting the original error vectors cannot.
    let s = station(
        "a",
        "H1",
        0,
        0.009_f64.to_degrees(),
        0.0,
        (0.0, 1.0),
        SurveyInstrument::Gyro,
        RunDirection::In,
    );
    let tangent = Direction::of(s.orientation.as_ref().unwrap());
    for model in [ErrorModel::random(), ErrorModel::systematic(), mixed()] {
        let hole = propagate(&c, std::slice::from_ref(&s), &model).unwrap();
        let variance = hole.checked_variance_along(100_000, tangent).unwrap();
        assert!((0.0..1e-18).contains(&variance), "{model:?}: {variance:e}");
        let across = Direction {
            east: tangent.north,
            north: -tangent.east,
            up: 0.0,
        };
        let want = (100.0 * DEG).powi(2);
        assert!((hole.checked_variance_along(100_000, across).unwrap() - want).abs() < 1e-9 * want);
        let invalid = Direction {
            east: f64::INFINITY,
            ..across
        };
        assert!(hole.checked_variance_along(100_000, invalid).is_err());
    }
}

#[test]
fn test_checked_covariance_refuses_sigma_and_extrapolation_overflow() {
    let mut c = collar("H1");
    let s = station(
        "a",
        "H1",
        0,
        0.0,
        0.0,
        (0.0, 1.0),
        SurveyInstrument::Gyro,
        RunDirection::In,
    );
    let across = Direction {
        east: 1.0,
        north: 0.0,
        up: 0.0,
    };
    c.actual.as_mut().unwrap().sigma_horizontal_m = 1e200;
    let hole = propagate(&c, std::slice::from_ref(&s), &ErrorModel::random()).unwrap();
    assert!(hole.checked_covariance_at(0).is_err());
    assert!(hole.checked_variance_along(0, across).is_err());

    let c = collar("H1");
    let mut s = s;
    s.orientation.as_mut().unwrap().sigma_azimuth_deg = 1e140;
    for model in [ErrorModel::random(), ErrorModel::systematic()] {
        let hole = propagate(&c, std::slice::from_ref(&s), &model).unwrap();
        assert!(hole.checked_covariance_at(100_000).is_ok());
        assert!(hole.checked_covariance_at(u64::MAX).is_err());
        assert!(hole.checked_variance_along(u64::MAX, across).is_err());
    }
}
