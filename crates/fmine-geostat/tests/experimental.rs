//! Experimental variogram tests (`BD-237`).
//!
//! A brute-force oracle, a double loop over every pair in index order, gives bit-identical lags to the grid
//! search for seeded point sets, directions, lag schemes and hole filters. A two-hole campaign runs the whole path:
//! assays composited by `fmine_drill::composite`, desurveyed holes, `from_composites`, then experimental variograms
//! checked against values worked by hand.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss
)]

use fmine_claim::{AdditivityClass, SpatialSupport};
use fmine_core::det;
use fmine_drill::composite::{Bounds, Domain, Input, Residual, Rule, Weighting, composite};
use fmine_drill::desurvey::desurvey;
use fmine_drill::join::Span;
use fmine_drill::records::{
    Collar, HoleType, Orientation, Position, PositionMethod, RunDirection, Survey, SurveyInstrument,
};
use fmine_geostat::GeostatError;
use fmine_geostat::experimental::{Direction, Lag, Lags, Pairs, experimental};
use fmine_geostat::samples::{Censored, Sample, SampleSet, from_composites};

const CRS: &str = "EPSG:32735";

struct Rng(u64);

impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1_u64 << 53) as f64
    }
}

#[derive(Default, Clone, Copy)]
struct Sums {
    n: u64,
    h: f64,
    sq: f64,
    head: f64,
    tail: f64,
}

/// Every pair (i < j) in index order; every lag tested, with no candidate pruning.
fn brute_force(
    points: &[Sample],
    directions: &[Direction],
    lags: &Lags,
    pairs: Pairs,
) -> (Vec<Vec<Lag>>, u64) {
    let count = lags.count as usize;
    let mut sums = vec![vec![Sums::default(); count]; directions.len()];
    let mut coincident = 0;
    let frames: Vec<[[f64; 3]; 3]> = directions.iter().map(Direction::frame).collect();
    for (i, a) in points.iter().enumerate() {
        for b in &points[i + 1..] {
            let keep = match pairs {
                Pairs::Any => true,
                Pairs::SameHole => a.hole == b.hole,
                Pairs::OtherHoles => a.hole != b.hole,
            };
            if !keep {
                continue;
            }
            let d = [b.east_m - a.east_m, b.north_m - a.north_m, b.up_m - a.up_m];
            let h2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            if h2 == 0.0 {
                coincident += 1;
                continue;
            }
            let h = h2.sqrt();
            for ((dir, f), row) in directions.iter().zip(&frames).zip(&mut sums) {
                let dot = |v: [f64; 3]| d[0] * v[0] + d[1] * v[1] + d[2] * v[2];
                let along = dot(f[0]);
                if dir.angle_tolerance_deg < 90.0 {
                    let cos =
                        det::sin_cos(dir.angle_tolerance_deg * std::f64::consts::PI / 180.0).1;
                    if along.abs() < cos * h
                        || dot(f[1]).abs() > dir.bandwidth_h_m
                        || dot(f[2]).abs() > dir.bandwidth_v_m
                    {
                        continue;
                    }
                }
                let (head, tail) = if along >= 0.0 {
                    (b.value, a.value)
                } else {
                    (a.value, b.value)
                };
                for (k, s) in row.iter_mut().enumerate() {
                    let centre = (k + 1) as f64 * lags.width_m;
                    if (h - centre).abs() <= lags.tolerance_m {
                        s.n += 1;
                        s.h += h;
                        s.sq += (head - tail) * (head - tail);
                        s.head += head;
                        s.tail += tail;
                    }
                }
            }
        }
    }
    let out = sums
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .filter(|(_, s)| s.n > 0)
                .map(|(k, s)| {
                    let n = s.n as f64;
                    Lag {
                        k: u32::try_from(k + 1).unwrap(),
                        pairs: s.n,
                        mean_distance_m: s.h / n,
                        gamma: s.sq / (2.0 * n),
                        head_mean: s.head / n,
                        tail_mean: s.tail / n,
                    }
                })
                .collect()
        })
        .collect();
    (out, coincident)
}

fn bits(lags: &[Lag]) -> Vec<(u32, u64, [u64; 4])> {
    lags.iter()
        .map(|l| {
            (
                l.k,
                l.pairs,
                [
                    l.mean_distance_m.to_bits(),
                    l.gamma.to_bits(),
                    l.head_mean.to_bits(),
                    l.tail_mean.to_bits(),
                ],
            )
        })
        .collect()
}

#[test]
fn test_the_grid_search_gives_the_brute_force_lags_bit_for_bit() {
    let directions = [
        Direction::OMNIDIRECTIONAL,
        Direction {
            azimuth_deg: 0.0,
            dip_deg: 0.0,
            angle_tolerance_deg: 22.5,
            bandwidth_h_m: 20.0,
            bandwidth_v_m: 8.0,
        },
        Direction {
            azimuth_deg: 45.0,
            dip_deg: -30.0,
            angle_tolerance_deg: 15.0,
            bandwidth_h_m: f64::INFINITY,
            bandwidth_v_m: 5.0,
        },
        Direction {
            azimuth_deg: 0.0,
            dip_deg: -90.0,
            angle_tolerance_deg: 10.0,
            bandwidth_h_m: f64::INFINITY,
            bandwidth_v_m: f64::INFINITY,
        },
    ];
    let schemes = [
        Lags {
            count: 12,
            width_m: 10.0,
            tolerance_m: 5.0,
        },
        // Overlapping classes: a pair counts in two lags.
        Lags {
            count: 8,
            width_m: 15.0,
            tolerance_m: 12.0,
        },
        Lags {
            count: 40,
            width_m: 2.5,
            tolerance_m: 0.5,
        },
    ];
    for seed in 0..4_u64 {
        let mut rng = Rng(seed);
        // 450 m on every axis: at least four grid cells along each, so every neighbouring cell is searched.
        let mut points: Vec<Sample> = (0..1_500)
            .map(|i| Sample {
                east_m: 400_000.0 + 450.0 * rng.unit(),
                north_m: 8_640_000.0 + 450.0 * rng.unit(),
                up_m: 1_200.0 + 450.0 * rng.unit(),
                value: 0.01 * rng.unit() + if i % 3 == 0 { 0.02 } else { 0.0 },
                hole: u32::try_from(i % 9).unwrap(),
            })
            .collect();
        // Coincident points, one pair within a hole and one across holes.
        points.push(Sample {
            hole: 0,
            ..points[9]
        });
        points.push(Sample {
            hole: 5,
            ..points[9]
        });
        let holes = (0..9).map(|h| format!("H{h}")).collect();
        let set = SampleSet::new(CRS, SpatialSupport::PointCore, holes, points.clone()).unwrap();
        for lags in &schemes {
            for pairs in [Pairs::Any, Pairs::SameHole, Pairs::OtherHoles] {
                let got = experimental(&set, &directions, lags, pairs).unwrap();
                let (want, coincident) = brute_force(&points, &directions, lags, pairs);
                assert_eq!(
                    got.coincident_pairs, coincident,
                    "seed {seed} {lags:?} {pairs:?}"
                );
                assert!(want.iter().any(|w| !w.is_empty()));
                for (g, w) in got.variograms.iter().zip(&want) {
                    assert_eq!(
                        bits(&g.lags),
                        bits(w),
                        "seed {seed} {lags:?} {pairs:?} {:?}",
                        g.direction
                    );
                }
            }
        }
    }
}

fn collar(hole_id: &str, easting_m: f64) -> Collar {
    Collar {
        hole_id: hole_id.to_string(),
        hole_type: HoleType::Diamond,
        program: "geostat test".to_string(),
        planned: None,
        actual: Some(Position {
            crs: CRS.to_string(),
            easting_m,
            northing_m: 8_652_100.0,
            elevation_m: 1_300.0,
            method: PositionMethod::DifferentialGnss,
            sigma_horizontal_m: 0.05,
            sigma_vertical_m: 0.1,
        }),
    }
}

fn vertical(hole_id: &str) -> Survey {
    Survey {
        survey_id: format!("{hole_id}-S0"),
        hole_id: hole_id.to_string(),
        depth_mm: 0,
        instrument: SurveyInstrument::Gyro,
        orientation: Some(Orientation {
            azimuth_deg: 0.0,
            dip_deg: -90.0,
            sigma_azimuth_deg: 0.5,
            sigma_dip_deg: 0.25,
        }),
        magnetic_interference: false,
        run: RunDirection::Out,
    }
}

fn metre(from_m: u64, lo: f64, hi: f64) -> Input {
    Input {
        span: Span {
            from_mm: 1_000 * from_m,
            to_mm: 1_000 * (from_m + 1),
        },
        value: Bounds { lo, hi },
        mass_kg: None,
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-15 * b.abs().max(1e-3)
}

#[test]
fn test_composites_of_two_holes_give_the_variograms_worked_by_hand() {
    let rule = Rule {
        length_mm: 2_000,
        weighting: Weighting::Length,
        residual: Residual::Keep,
        additivity: AdditivityClass::IntensiveWeighted,
        input_support: SpatialSupport::PointCore,
    };
    let domain = |code: &str, from_m: u64, to_m: u64| Domain {
        code: code.to_string(),
        span: Span {
            from_mm: 1_000 * from_m,
            to_mm: 1_000 * to_m,
        },
    };
    // Hole A, oxide 0–8 m: composites 0.01, 0.02, 0.04, 0.07 at 1, 3, 5 and 7 m. Fresh rock 8–10 m is another
    // domain.
    let a_values = [0.01, 0.01, 0.02, 0.02, 0.04, 0.04, 0.07, 0.07, 0.5, 0.5];
    let a_inputs: Vec<Input> = (0..10)
        .map(|m| metre(m, a_values[m as usize], a_values[m as usize]))
        .collect();
    let a = composite(&[domain("OX", 0, 8), domain("FR", 8, 10)], &a_inputs, &rule).unwrap();
    // Hole B, 10 m east: 0–1 m below a 0.06 detection limit, so 0–2 m lies in [0.03, 0.06]; then 0.03 and 0.05;
    // 7–8 m was not sampled, so 6–8 m is partly sampled.
    let b_inputs = [
        metre(0, 0.0, 0.06),
        metre(1, 0.06, 0.06),
        metre(2, 0.03, 0.03),
        metre(3, 0.03, 0.03),
        metre(4, 0.05, 0.05),
        metre(5, 0.05, 0.05),
        metre(6, 0.05, 0.05),
    ];
    let b = composite(&[domain("OX", 0, 8)], &b_inputs, &rule).unwrap();
    let ta = desurvey(&collar("A", 421_000.0), &[vertical("A")]).unwrap();
    let tb = desurvey(&collar("B", 421_010.0), &[vertical("B")]).unwrap();
    let holes = [(&ta, &a), (&tb, &b)];

    let refused = from_composites(
        &holes,
        "OX",
        SpatialSupport::Composite { length_mm: 2_000 },
        Censored::Refuse,
    )
    .unwrap_err();
    assert!(
        matches!(refused, GeostatError::NotObservable { .. }),
        "{refused:?}"
    );
    let other = from_composites(
        &holes,
        "OX",
        SpatialSupport::Composite { length_mm: 1_000 },
        Censored::Midpoint,
    )
    .unwrap();
    assert_eq!((other.set.samples().len(), other.other_support), (0, 8));

    let got = from_composites(
        &holes,
        "OX",
        SpatialSupport::Composite { length_mm: 2_000 },
        Censored::Midpoint,
    )
    .unwrap();
    assert_eq!(
        (
            got.censored,
            got.partly_sampled,
            got.unsampled,
            got.other_support
        ),
        (1, 1, 0, 0)
    );
    let set = &got.set;
    assert_eq!(set.holes(), ["A", "B"]);
    let at: Vec<(u32, f64, f64, f64, f64)> = set
        .samples()
        .iter()
        .map(|s| (s.hole, s.east_m, s.north_m, s.up_m, s.value))
        .collect();
    let want = [
        (0, 421_000.0, 1_299.0, 0.01),
        (0, 421_000.0, 1_297.0, 0.02),
        (0, 421_000.0, 1_295.0, 0.04),
        (0, 421_000.0, 1_293.0, 0.07),
        (1, 421_010.0, 1_299.0, 0.045),
        (1, 421_010.0, 1_297.0, 0.03),
        (1, 421_010.0, 1_295.0, 0.05),
    ];
    assert_eq!(at.len(), want.len());
    for (g, w) in at.iter().zip(&want) {
        assert_eq!(g.0, w.0);
        assert!(
            (g.1 - w.1).abs() < 1e-9 && (g.2 - 8_652_100.0).abs() < 1e-9,
            "{g:?}"
        );
        assert!(
            (g.3 - w.2).abs() < 1e-9 && close(g.4, w.3),
            "{g:?} vs {w:?}"
        );
    }

    // Down the holes: lags of 2, 4 and 6 m.
    let down = Direction {
        azimuth_deg: 0.0,
        dip_deg: -90.0,
        angle_tolerance_deg: 10.0,
        bandwidth_h_m: f64::INFINITY,
        bandwidth_v_m: f64::INFINITY,
    };
    let lags = Lags {
        count: 3,
        width_m: 2.0,
        tolerance_m: 1.0,
    };
    let v = experimental(set, &[down], &lags, Pairs::SameHole).unwrap();
    let l = &v.variograms[0].lags;
    assert_eq!(
        l.iter().map(|l| (l.k, l.pairs)).collect::<Vec<_>>(),
        [(1, 5), (2, 3), (3, 1)]
    );
    // Lag 1: A gives differences 0.01, 0.02, 0.03 and B 0.015, 0.02. Head is the deeper sample.
    assert!(
        close(l[0].gamma, (1e-4 + 4e-4 + 9e-4 + 2.25e-4 + 4e-4) / 10.0),
        "{:?}",
        l[0]
    );
    assert!(
        close(l[0].head_mean, (0.02 + 0.04 + 0.07 + 0.03 + 0.05) / 5.0),
        "{:?}",
        l[0]
    );
    assert!(
        close(l[0].tail_mean, (0.01 + 0.02 + 0.04 + 0.045 + 0.03) / 5.0),
        "{:?}",
        l[0]
    );
    assert!((l[0].mean_distance_m - 2.0).abs() < 1e-9);
    // Lag 2: 0.03 and 0.05 in A, 0.005 in B. Lag 3: 0.06 in A.
    assert!(
        close(l[1].gamma, (9e-4 + 25e-4 + 0.25e-4) / 6.0),
        "{:?}",
        l[1]
    );
    assert!(close(l[2].gamma, 36e-4 / 2.0), "{:?}", l[2]);
    let across = experimental(set, &[down], &lags, Pairs::OtherHoles).unwrap();
    assert!(across.variograms[0].lags.is_empty());

    // Across the holes, east: only the pairs at one depth lie within 10° of horizontal (the next are 11.3° off).
    let east = Direction {
        azimuth_deg: 90.0,
        dip_deg: 0.0,
        angle_tolerance_deg: 10.0,
        bandwidth_h_m: f64::INFINITY,
        bandwidth_v_m: f64::INFINITY,
    };
    let ten = Lags {
        count: 1,
        width_m: 10.0,
        tolerance_m: 1.0,
    };
    let v = experimental(
        set,
        &[east, Direction::OMNIDIRECTIONAL],
        &ten,
        Pairs::OtherHoles,
    )
    .unwrap();
    let e = &v.variograms[0].lags;
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].pairs, 3);
    assert!(
        close(e[0].gamma, (12.25e-4 + 1e-4 + 1e-4) / 6.0),
        "{:?}",
        e[0]
    );
    assert!(
        close(e[0].head_mean, (0.045 + 0.03 + 0.05) / 3.0),
        "{:?}",
        e[0]
    );
    assert!(
        close(e[0].tail_mean, (0.01 + 0.02 + 0.04) / 3.0),
        "{:?}",
        e[0]
    );
    // Omnidirectionally every pair within 9 m to 11 m counts: 3 at one depth, 5 two metres apart (10.2 m) and 3 four
    // metres apart (10.8 m); the one six metres apart is 11.7 m away.
    assert_eq!(v.variograms[1].lags[0].pairs, 11);
}
