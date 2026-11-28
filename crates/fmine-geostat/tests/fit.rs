//! Variogram fit tests (`BD-237`).
//!
//! Known answers: experimental variograms computed from a nested model at its own lags are fitted back to that
//! model, isotropic and rotated anisotropic. An independent oracle checks a fit to noisy values. For one
//! spherical structure and a nugget it enumerates the non-negative least-squares solutions of the two
//! coefficients in closed form, then scans the range on a fine grid. Refusals cover what the lags cannot
//! determine.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss
)]

use fmine_geostat::GeostatError;
use fmine_geostat::experimental::{Direction, Experimental, Lag};
use fmine_geostat::fit::{Range, RangeSpec, Spec, StructureSpec, Weights, fit};
use fmine_geostat::model::{Model, Ranges, Rotation, Shape, Structure};

fn direction(azimuth_deg: f64, dip_deg: f64) -> Direction {
    Direction {
        azimuth_deg,
        dip_deg,
        angle_tolerance_deg: 22.5,
        bandwidth_h_m: f64::INFINITY,
        bandwidth_v_m: f64::INFINITY,
    }
}

/// `model`'s γ along `direction` at lags 10 m, 20 m, … `count` × 10 m, as an experimental variogram whose pair
/// counts fall with distance. `noise(k)` is added to the k-th γ.
fn variogram(
    model: &Model,
    direction: Direction,
    count: u32,
    noise: impl Fn(u32) -> f64,
) -> Experimental {
    let u = direction.frame()[0];
    Experimental {
        direction,
        lags: (1..=count)
            .map(|k| {
                let h = 10.0 * f64::from(k);
                Lag {
                    k,
                    pairs: u64::from(2_000 - 30 * k),
                    mean_distance_m: h,
                    gamma: model.gamma([u[0] * h, u[1] * h, u[2] * h]) + noise(k),
                    head_mean: 0.0,
                    tail_mean: 0.0,
                }
            })
            .collect(),
    }
}

fn structure(shape: Shape, sill: f64, ranges: Ranges, rotation: Rotation) -> Structure {
    Structure {
        shape,
        sill,
        ranges,
        rotation,
    }
}

fn iso(range_m: f64) -> Ranges {
    Ranges {
        major_m: range_m,
        semi_m: range_m,
        minor_m: range_m,
    }
}

fn relative(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}

#[test]
fn test_a_nested_isotropic_model_is_recovered_from_its_own_variogram() {
    let truth = Model::new(
        0.15,
        vec![
            structure(Shape::Spherical, 0.6, iso(80.0), Rotation::NONE),
            structure(Shape::Exponential, 0.4, iso(450.0), Rotation::NONE),
        ],
    )
    .unwrap();
    let data = [variogram(&truth, Direction::OMNIDIRECTIONAL, 60, |_| 0.0)];
    for weights in [Weights::Pairs, Weights::PairsOverDistanceSquared] {
        let spec = Spec {
            nugget: true,
            structures: vec![
                StructureSpec {
                    shape: Shape::Spherical,
                    ranges: RangeSpec::Isotropic(Range::Free(40.0)),
                },
                StructureSpec {
                    shape: Shape::Exponential,
                    ranges: RangeSpec::Isotropic(Range::Free(900.0)),
                },
            ],
            weights,
        };
        let got = fit(&data, &spec).unwrap();
        let m = &got.model;
        assert!(relative(m.nugget(), 0.15) < 1e-7, "{weights:?}: {m:?}");
        let s = m.structures();
        assert!(relative(s[0].sill, 0.6) < 1e-7, "{weights:?}: {m:?}");
        assert!(relative(s[1].sill, 0.4) < 1e-7, "{weights:?}: {m:?}");
        assert!(
            relative(s[0].ranges.major_m, 80.0) < 1e-7,
            "{weights:?}: {m:?}"
        );
        assert!(
            relative(s[1].ranges.major_m, 450.0) < 1e-7,
            "{weights:?}: {m:?}"
        );
        assert_eq!(s[0].ranges, iso(s[0].ranges.major_m));
        assert!(got.objective < 1e-14, "{weights:?}: {}", got.objective);
        assert_eq!(got.points, 60);
        assert!(got.restarts >= 1);
    }
}

#[test]
fn test_a_rotated_anisotropic_model_is_recovered_from_five_directions() {
    let rotation = Rotation {
        azimuth_deg: 35.0,
        dip_deg: -20.0,
        rake_deg: 15.0,
    };
    let ranges = Ranges {
        major_m: 240.0,
        semi_m: 120.0,
        minor_m: 40.0,
    };
    let truth = Model::new(
        0.1,
        vec![structure(Shape::Spherical, 1.0, ranges, rotation)],
    )
    .unwrap();
    let directions = [
        (0.0, 0.0),
        (60.0, 0.0),
        (120.0, 0.0),
        (35.0, -45.0),
        (215.0, -60.0),
    ];
    let data: Vec<Experimental> = directions
        .iter()
        .map(|&(az, dip)| variogram(&truth, direction(az, dip), 30, |_| 0.0))
        .collect();
    let spec = Spec {
        nugget: true,
        structures: vec![StructureSpec {
            shape: Shape::Spherical,
            ranges: RangeSpec::Anisotropic {
                rotation,
                major: Range::Free(150.0),
                semi: Range::Free(150.0),
                minor: Range::Free(150.0),
            },
        }],
        weights: Weights::Pairs,
    };
    let got = fit(&data, &spec).unwrap();
    let m = &got.model;
    let s = &m.structures()[0];
    assert!(relative(m.nugget(), 0.1) < 1e-7, "{m:?}");
    assert!(relative(s.sill, 1.0) < 1e-7, "{m:?}");
    assert!(relative(s.ranges.major_m, 240.0) < 1e-7, "{m:?}");
    assert!(relative(s.ranges.semi_m, 120.0) < 1e-7, "{m:?}");
    assert!(relative(s.ranges.minor_m, 40.0) < 1e-7, "{m:?}");
    assert_eq!(s.rotation, rotation);
    assert_eq!(got.points, 150);
}

#[test]
fn test_a_minor_range_no_direction_sees_is_refused_and_fits_once_fixed() {
    // Horizontal directions only: the vertical minor axis of an unrotated structure is never seen.
    let ranges = Ranges {
        major_m: 200.0,
        semi_m: 90.0,
        minor_m: 30.0,
    };
    let truth = Model::new(
        0.0,
        vec![structure(Shape::Spherical, 1.0, ranges, Rotation::NONE)],
    )
    .unwrap();
    let data: Vec<Experimental> = [0.0, 45.0, 90.0]
        .iter()
        .map(|&az| variogram(&truth, direction(az, 0.0), 30, |_| 0.0))
        .collect();
    let spec = |minor: Range| Spec {
        nugget: false,
        structures: vec![StructureSpec {
            shape: Shape::Spherical,
            ranges: RangeSpec::Anisotropic {
                rotation: Rotation::NONE,
                major: Range::Free(100.0),
                semi: Range::Free(100.0),
                minor,
            },
        }],
        weights: Weights::Pairs,
    };
    let refused = fit(&data, &spec(Range::Free(100.0))).unwrap_err();
    assert!(
        matches!(&refused, GeostatError::NotObservable { detail } if detail.contains("minor")),
        "{refused:?}"
    );
    assert_eq!(refused.to_refusal().code, "FMINE-REF-202");

    let got = fit(&data, &spec(Range::Fixed(30.0))).unwrap();
    let s = &got.model.structures()[0];
    assert!(relative(s.ranges.major_m, 200.0) < 1e-7, "{s:?}");
    assert!(relative(s.ranges.semi_m, 90.0) < 1e-7, "{s:?}");
    assert_eq!(s.ranges.minor_m, 30.0);
    assert!(relative(s.sill, 1.0) < 1e-7, "{s:?}");

    // One horizontal direction cannot separate the major and semi-major ranges either.
    let refused = fit(&data[..1], &spec(Range::Fixed(30.0))).unwrap_err();
    assert!(
        matches!(&refused, GeostatError::NotObservable { detail } if detail.contains("semi-major")),
        "{refused:?}"
    );
}

/// Deterministic noise in ±`amplitude` (SplitMix64).
fn noise(seed: u64, amplitude: f64) -> impl Fn(u32) -> f64 {
    move |k| {
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15_u64.wrapping_mul(u64::from(k) + 1));
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        let unit = ((z ^ (z >> 31)) >> 11) as f64 / (1_u64 << 53) as f64;
        amplitude * (2.0 * unit - 1.0)
    }
}

fn spherical(h: f64) -> f64 {
    if h >= 1.0 {
        1.0
    } else {
        1.5 * h - 0.5 * h * h * h
    }
}

/// The oracle: for range `a`, the best non-negative nugget and sill of γ = c₀ + c₁ sph(h / a) and the objective
/// they leave, with weights `w` (already averaging 1). The two-coefficient problem has four candidate supports;
/// each is solved in closed form.
fn oracle(points: &[(f64, f64, f64)], a: f64) -> (f64, f64, f64) {
    let s = |c0: f64, c1: f64| -> f64 {
        points
            .iter()
            .map(|&(h, g, w)| {
                let r = g - c0 - c1 * spherical(h / a);
                w * r * r
            })
            .sum()
    };
    let (mut sw, mut sf, mut sff, mut sg, mut sfg) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(h, g, w) in points {
        let f = spherical(h / a);
        sw += w;
        sf += w * f;
        sff += w * f * f;
        sg += w * g;
        sfg += w * f * g;
    }
    let mut best = (0.0, 0.0, s(0.0, 0.0));
    let det = sw * sff - sf * sf;
    let both = ((sff * sg - sf * sfg) / det, (sw * sfg - sf * sg) / det);
    for (c0, c1) in [both, (sg / sw, 0.0), (0.0, sfg / sff)] {
        if c0 >= 0.0 && c1 >= 0.0 {
            let v = s(c0, c1);
            if v < best.2 {
                best = (c0, c1, v);
            }
        }
    }
    best
}

#[test]
fn test_a_noisy_fit_is_the_global_least_squares_optimum_an_oracle_finds() {
    let truth = Model::new(
        0.2,
        vec![structure(Shape::Spherical, 0.8, iso(130.0), Rotation::NONE)],
    )
    .unwrap();
    let data = [variogram(
        &truth,
        Direction::OMNIDIRECTIONAL,
        40,
        noise(7, 0.04),
    )];
    for weights in [Weights::Pairs, Weights::PairsOverDistanceSquared] {
        let spec = Spec {
            nugget: true,
            structures: vec![StructureSpec {
                shape: Shape::Spherical,
                ranges: RangeSpec::Isotropic(Range::Free(60.0)),
            }],
            weights,
        };
        let got = fit(&data, &spec).unwrap();
        let mut points: Vec<(f64, f64, f64)> = data[0]
            .lags
            .iter()
            .map(|l| {
                let n = l.pairs as f64;
                let h = l.mean_distance_m;
                let w = match weights {
                    Weights::Pairs => n,
                    Weights::PairsOverDistanceSquared => n / (h * h),
                };
                (h, l.gamma, w)
            })
            .collect();
        let mean = points.iter().map(|p| p.2).sum::<f64>() / points.len() as f64;
        for p in &mut points {
            p.2 /= mean;
        }
        let a = got.model.structures()[0].ranges.major_m;
        let (c0, c1, at_fit) = oracle(&points, a);
        assert!(
            relative(got.objective, at_fit) < 1e-9,
            "{weights:?}: {} vs {at_fit}",
            got.objective
        );
        assert!(relative(got.model.nugget(), c0) < 1e-9, "{weights:?}: {c0}");
        assert!(
            relative(got.model.structures()[0].sill, c1) < 1e-9,
            "{weights:?}: {c1}"
        );
        // Global: no range on a grid 1 m to 1000 m at 0.05% steps fits better.
        let mut r = 1.0;
        while r < 1000.0 {
            let (_, _, v) = oracle(&points, r);
            assert!(
                v >= at_fit * (1.0 - 1e-9),
                "{weights:?}: range {r} m gives {v} < {at_fit} at {a} m"
            );
            r *= 1.0005;
        }
        // Local: a relative step of 10⁻⁶ either way does not improve it.
        for step in [1.0 - 1e-6, 1.0 + 1e-6] {
            let (_, _, v) = oracle(&points, a * step);
            assert!(v >= at_fit * (1.0 - 1e-12), "{weights:?}: {v} < {at_fit}");
        }
        // The noisy fit stays near the truth.
        assert!(relative(a, 130.0) < 0.1, "{weights:?}: {a}");
    }
}

#[test]
fn test_a_negative_nugget_is_held_at_zero_where_the_kkt_conditions_hold() {
    // γ = sph(h / 150) − 0.05, floored at 0: the unconstrained nugget is negative.
    let truth = Model::new(
        0.0,
        vec![structure(Shape::Spherical, 1.0, iso(150.0), Rotation::NONE)],
    )
    .unwrap();
    let mut data = variogram(&truth, Direction::OMNIDIRECTIONAL, 30, |_| 0.0);
    for l in &mut data.lags {
        l.gamma = (l.gamma - 0.05).max(0.0);
    }
    let spec = Spec {
        nugget: true,
        structures: vec![StructureSpec {
            shape: Shape::Spherical,
            ranges: RangeSpec::Isotropic(Range::Free(100.0)),
        }],
        weights: Weights::Pairs,
    };
    let got = fit(std::slice::from_ref(&data), &spec).unwrap();
    assert_eq!(got.model.nugget(), 0.0);
    let s = got.model.structures()[0];
    assert!(s.sill > 0.0);
    // KKT at fixed range: the residuals r_j = γ̂_j − γ(h_j) are orthogonal to the sill's column (its coefficient
    // is free), and Σ w_j r_j ≤ 0 (raising the nugget from 0 would not lower S).
    let (mut nugget_gradient, mut sill_gradient, mut scale) = (0.0, 0.0, 0.0);
    for l in &data.lags {
        let w = l.pairs as f64;
        let f = spherical(l.mean_distance_m / s.ranges.major_m);
        let r = l.gamma - s.sill * f;
        nugget_gradient += w * r;
        sill_gradient += w * r * f;
        scale += w * l.gamma;
    }
    assert!(nugget_gradient < 0.0, "{nugget_gradient}");
    assert!(sill_gradient.abs() < 1e-12 * scale, "{sill_gradient}");
}

#[test]
fn test_fits_the_lags_cannot_support_are_refused() {
    let truth = Model::new(
        0.0,
        vec![structure(Shape::Spherical, 1.0, iso(100.0), Rotation::NONE)],
    )
    .unwrap();
    let omni = [variogram(&truth, Direction::OMNIDIRECTIONAL, 20, |_| 0.0)];
    let one = |ranges: RangeSpec| Spec {
        nugget: true,
        structures: vec![StructureSpec {
            shape: Shape::Spherical,
            ranges,
        }],
        weights: Weights::Pairs,
    };
    let code = |e: &GeostatError| e.to_refusal().code;

    // An anisotropic structure on an omnidirectional variogram.
    let e = fit(
        &omni,
        &one(RangeSpec::Anisotropic {
            rotation: Rotation::NONE,
            major: Range::Free(100.0),
            semi: Range::Fixed(100.0),
            minor: Range::Fixed(100.0),
        }),
    )
    .unwrap_err();
    assert_eq!(code(&e), "FMINE-REF-109", "{e:?}");

    // Fewer lags than parameters: nugget, sill and range from two lags.
    let short = [Experimental {
        direction: Direction::OMNIDIRECTIONAL,
        lags: omni[0].lags[..2].to_vec(),
    }];
    let e = fit(&short, &one(RangeSpec::Isotropic(Range::Free(50.0)))).unwrap_err();
    assert_eq!(code(&e), "FMINE-REF-202", "{e:?}");

    // No lags at all.
    let empty = [Experimental {
        direction: Direction::OMNIDIRECTIONAL,
        lags: Vec::new(),
    }];
    let e = fit(&empty, &one(RangeSpec::Isotropic(Range::Free(50.0)))).unwrap_err();
    assert!(
        matches!(&e, GeostatError::NotObservable { detail } if detail.contains("no lags")),
        "{e:?}"
    );

    // A variogram still rising linearly at 200 m: a spherical range runs to the edge of the search.
    let linear = [Experimental {
        direction: Direction::OMNIDIRECTIONAL,
        lags: omni[0]
            .lags
            .iter()
            .map(|l| Lag {
                gamma: 0.001 * l.mean_distance_m,
                ..*l
            })
            .collect(),
    }];
    let e = fit(&linear, &one(RangeSpec::Isotropic(Range::Free(50.0)))).unwrap_err();
    assert!(
        matches!(&e, GeostatError::NotObservable { detail } if detail.contains("edge of the search")),
        "{e:?}"
    );

    // Domains: no structures and no nugget, a range that is not positive, a free range outside the search.
    let none = Spec {
        nugget: false,
        structures: Vec::new(),
        weights: Weights::Pairs,
    };
    for spec in [
        none,
        one(RangeSpec::Isotropic(Range::Fixed(0.0))),
        one(RangeSpec::Isotropic(Range::Free(1e9))),
    ] {
        let e = fit(&omni, &spec).unwrap_err();
        assert_eq!(code(&e), "FMINE-REF-109", "{spec:?}: {e:?}");
    }

    // A lag with no pairs.
    let mut empty = omni.clone();
    empty[0].lags[3].pairs = 0;
    let e = fit(&empty, &one(RangeSpec::Isotropic(Range::Free(50.0)))).unwrap_err();
    assert_eq!(code(&e), "FMINE-REF-109", "{e:?}");

    // Fixed ranges need no search: the sills are the exact non-negative least-squares solution.
    let got = fit(&omni, &one(RangeSpec::Isotropic(Range::Fixed(100.0)))).unwrap();
    assert_eq!(got.evaluations, 0);
    assert!(got.model.nugget().abs() < 1e-12, "{:?}", got.model);
    assert!(relative(got.model.structures()[0].sill, 1.0) < 1e-12);
}
