//! Adversarial corpus for the exact predicates (`BD-193`, done-when "Adversarial predicate corpus").
//!
//! Every expected sign comes from an all-integer `i128` oracle over the same coordinates, so a failure indicts the
//! predicate (or the oracle), with no floating-point reference in the loop. The generated families are built to
//! defeat the floating-point filter, and each test asserts that they do:
//! - `orient2d`, Bezout family: large lattice points whose determinant is exactly −1, 0 or +1 while the products
//!   round, so stage A cannot decide (they resolve at the adaptive stage);
//! - `orient2d`, Kettner grid: half-ulp offsets whose differences round, so only stage D, the exact one, decides;
//! - `orient3d`, `incircle`, `insphere`: exactly coplanar, cocircular and cospherical points far from the origin,
//!   and their ±1 perturbations; the exact zeros force the exact stage.
//!
//! Each family also checks antisymmetry under one argument swap, counts how often the naive float determinant gets
//! the sign wrong (it must, at least once, or the family is too easy), and runs the Simulation-of-Simplicity forms
//! on its degenerate rows. The refusal rows check that non-finite input and overflow are refused, never signed.
//!
//! The literal rows `fsim-r033` to `fsim-r042` are frankensim's predicate rows
//! (`crates/fs-ivl/tests/fixtures/exact_corpus.txt` @ 20463525d); everything else is new. Each family prints one
//! JSON summary line. Green proves the stated rows and families only; it says nothing about subnormal coordinates,
//! which the predicates do not certify (CONTRACT.md §8).

use fmine_geom::exact::{
    Sign, Stage, incircle, incircle_with_stage, insphere, insphere_with_stage, orient2d,
    orient2d_sos, orient2d_with_stage, orient3d, orient3d_sos, orient3d_with_stage,
};

// ---------------------------------------------------------------------------
// Exact inputs and the integer oracle
// ---------------------------------------------------------------------------

/// `n · 2^-shift` as an `f64`, refusing (by panic, in this test) any `n` that does not convert exactly.
fn exact(n: i128, shift: u32) -> f64 {
    let x = n as f64;
    assert_eq!(x as i128, n, "{n} is not exactly representable");
    x * f64::from_bits(u64::from(1023 - shift) << 52)
}

fn p2(p: [i128; 2]) -> [f64; 2] {
    [exact(p[0], 0), exact(p[1], 0)]
}

fn p3(p: [i128; 3]) -> [f64; 3] {
    [exact(p[0], 0), exact(p[1], 0), exact(p[2], 0)]
}

fn sgn(x: i128) -> Sign {
    match x.cmp(&0) {
        core::cmp::Ordering::Less => Sign::Negative,
        core::cmp::Ordering::Equal => Sign::Zero,
        core::cmp::Ordering::Greater => Sign::Positive,
    }
}

fn fsgn(x: f64) -> Sign {
    if x > 0.0 {
        Sign::Positive
    } else if x < 0.0 {
        Sign::Negative
    } else {
        Sign::Zero
    }
}

fn o2d_i128(a: [i128; 2], b: [i128; 2], c: [i128; 2]) -> i128 {
    (a[0] - c[0]) * (b[1] - c[1]) - (a[1] - c[1]) * (b[0] - c[0])
}

fn o3d_i128(a: [i128; 3], b: [i128; 3], c: [i128; 3], d: [i128; 3]) -> i128 {
    let v = |p: [i128; 3]| [p[0] - d[0], p[1] - d[1], p[2] - d[2]];
    let (a, b, c) = (v(a), v(b), v(c));
    a[2] * (b[0] * c[1] - c[0] * b[1])
        + b[2] * (c[0] * a[1] - a[0] * c[1])
        + c[2] * (a[0] * b[1] - b[0] * a[1])
}

fn icc_i128(a: [i128; 2], b: [i128; 2], c: [i128; 2], d: [i128; 2]) -> i128 {
    let v = |p: [i128; 2]| {
        let (x, y) = (p[0] - d[0], p[1] - d[1]);
        (x, y, x * x + y * y)
    };
    let (a, b, c) = (v(a), v(b), v(c));
    a.2 * (b.0 * c.1 - c.0 * b.1) + b.2 * (c.0 * a.1 - a.0 * c.1) + c.2 * (a.0 * b.1 - b.0 * a.1)
}

fn isp_i128(a: [i128; 3], b: [i128; 3], c: [i128; 3], d: [i128; 3], e: [i128; 3]) -> i128 {
    let v = |p: [i128; 3]| {
        let (x, y, z) = (p[0] - e[0], p[1] - e[1], p[2] - e[2]);
        (x, y, z, x * x + y * y + z * z)
    };
    let (a, b, c, d) = (v(a), v(b), v(c), v(d));
    let m2 = |p: (i128, i128, i128, i128), q: (i128, i128, i128, i128)| p.0 * q.1 - q.0 * p.1;
    let (ab, bc, cd, da, ac, bd) = (m2(a, b), m2(b, c), m2(c, d), m2(d, a), m2(a, c), m2(b, d));
    let abc = a.2 * bc - b.2 * ac + c.2 * ab;
    let bcd = b.2 * cd - c.2 * bd + d.2 * bc;
    let cda = c.2 * da + d.2 * ac + a.2 * cd;
    let dab = d.2 * ab + a.2 * bd + b.2 * da;
    (d.3 * abc - c.3 * dab) + (b.3 * cda - a.3 * bcd)
}

// The naive float determinants: the same formulas, evaluated once in f64 with no error bound.

fn o2d_naive(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Sign {
    fsgn((a[0] - c[0]) * (b[1] - c[1]) - (a[1] - c[1]) * (b[0] - c[0]))
}

fn o3d_naive(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> Sign {
    let v = |p: [f64; 3]| [p[0] - d[0], p[1] - d[1], p[2] - d[2]];
    let (a, b, c) = (v(a), v(b), v(c));
    fsgn(
        a[2] * (b[0] * c[1] - c[0] * b[1])
            + b[2] * (c[0] * a[1] - a[0] * c[1])
            + c[2] * (a[0] * b[1] - b[0] * a[1]),
    )
}

fn icc_naive(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Sign {
    let v = |p: [f64; 2]| {
        let (x, y) = (p[0] - d[0], p[1] - d[1]);
        (x, y, x * x + y * y)
    };
    let (a, b, c) = (v(a), v(b), v(c));
    fsgn(
        a.2 * (b.0 * c.1 - c.0 * b.1)
            + b.2 * (c.0 * a.1 - a.0 * c.1)
            + c.2 * (a.0 * b.1 - b.0 * a.1),
    )
}

fn isp_naive(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3], e: [f64; 3]) -> Sign {
    let v = |p: [f64; 3]| {
        let (x, y, z) = (p[0] - e[0], p[1] - e[1], p[2] - e[2]);
        (x, y, z, x * x + y * y + z * z)
    };
    let (a, b, c, d) = (v(a), v(b), v(c), v(d));
    let m2 = |p: (f64, f64, f64, f64), q: (f64, f64, f64, f64)| p.0 * q.1 - q.0 * p.1;
    let (ab, bc, cd, da, ac, bd) = (m2(a, b), m2(b, c), m2(c, d), m2(d, a), m2(a, c), m2(b, d));
    let abc = a.2 * bc - b.2 * ac + c.2 * ab;
    let bcd = b.2 * cd - c.2 * bd + d.2 * bc;
    let cda = c.2 * da + d.2 * ac + a.2 * cd;
    let dab = d.2 * ab + a.2 * bd + b.2 * da;
    fsgn((d.3 * abc - c.3 * dab) + (b.3 * cda - a.3 * bcd))
}

// ---------------------------------------------------------------------------
// Family bookkeeping
// ---------------------------------------------------------------------------

struct LcgRng(u64);

impl LcgRng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 11
    }

    /// Uniform in `[lo, hi)`.
    fn range(&mut self, lo: i128, hi: i128) -> i128 {
        lo + i128::from(self.next()) % (hi - lo)
    }
}

#[derive(Default)]
struct Tally {
    rows: usize,
    zeros: usize,
    filtered: usize,
    adaptive: usize,
    exact: usize,
    naive_wrong: usize,
}

impl Tally {
    fn record(&mut self, got: (Sign, Stage), want: Sign, naive: Sign, row: &str) {
        assert_eq!(
            got.0, want,
            "{row}: predicate sign disagrees with the i128 oracle"
        );
        self.rows += 1;
        self.zeros += usize::from(want == Sign::Zero);
        match got.1 {
            Stage::Filtered => self.filtered += 1,
            Stage::Adaptive => self.adaptive += 1,
            Stage::Exact => self.exact += 1,
        }
        self.naive_wrong += usize::from(naive != want);
    }

    fn log(&self, family: &str) {
        println!(
            "{{\"suite\":\"fmine-geom/predicate_corpus\",\"family\":\"{family}\",\"rows\":{},\"zeros\":{},\
             \"filtered\":{},\"adaptive\":{},\"exact\":{},\"naive_wrong\":{}}}",
            self.rows, self.zeros, self.filtered, self.adaptive, self.exact, self.naive_wrong
        );
    }
}

/// `(g, s, t)` with `a·s + b·t = g = gcd(a, b)`.
fn ext_gcd(a: i128, b: i128) -> (i128, i128, i128) {
    if b == 0 {
        (a, 1, 0)
    } else {
        let (g, s, t) = ext_gcd(b, a % b);
        (g, t, s - (a / b) * t)
    }
}

/// A random coprime pair in `[lo, hi)` and its Bezout vector `w`, with `a·w[1] − b·w[0] = 1`.
fn coprime_with_bezout(rng: &mut LcgRng, lo: i128, hi: i128) -> ([i128; 2], [i128; 2]) {
    loop {
        let (a, b) = (rng.range(lo, hi), rng.range(lo, hi));
        let (g, s, t) = ext_gcd(a, b);
        if g == 1 {
            return ([a, b], [-t, s]);
        }
    }
}

/// Every integer point with `x² + y² = r2`, then with `x² + y² + z² = r2`.
fn circle_points(r2: i128) -> Vec<[i128; 2]> {
    let r = (0..).find(|r: &i128| r * r > r2).unwrap_or(0);
    let mut out = Vec::new();
    for x in -r..=r {
        for y in -r..=r {
            if x * x + y * y == r2 {
                out.push([x, y]);
            }
        }
    }
    out
}

fn sphere_points(r2: i128) -> Vec<[i128; 3]> {
    let r = (0..).find(|r: &i128| r * r > r2).unwrap_or(0);
    let mut out = Vec::new();
    for x in -r..=r {
        for y in -r..=r {
            for z in -r..=r {
                if x * x + y * y + z * z == r2 {
                    out.push([x, y, z]);
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Literal rows: hand-derived signs, each also checked against the oracle
// ---------------------------------------------------------------------------

#[test]
fn literal_rows_match_hand_derivation_and_oracle() {
    // (id, [a, b, c], hand-derived sign of the orient2d determinant)
    let o2: [(&str, [[i128; 2]; 3], i128); 5] = [
        ("fsim-r033 ccw", [[0, 0], [1, 0], [0, 1]], 1),
        ("fsim-r034 cw", [[0, 0], [0, 1], [1, 0]], -1),
        ("fsim-r035 collinear", [[0, 0], [2, 1], [4, 2]], 0),
        (
            "fsim-r036 large collinear",
            [[1048576, 1048576], [2097152, 2097153], [3145728, 3145730]],
            0,
        ),
        (
            "fsim-r037 one off the line",
            [[1048576, 1048576], [2097152, 2097153], [3145728, 3145731]],
            1,
        ),
    ];
    for (id, [a, b, c], hand) in o2 {
        assert_eq!(
            sgn(o2d_i128(a, b, c)),
            sgn(hand),
            "{id}: hand derivation vs oracle"
        );
        assert_eq!(orient2d(p2(a), p2(b), p2(c)).expect(id), sgn(hand), "{id}");
    }
    // orient3d: Positive iff d lies below the plane of a, b, c seen counterclockwise from above.
    let o3: [(&str, [[i128; 3]; 4], i128); 3] = [
        (
            "fsim-r038 d above",
            [[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1]],
            -1,
        ),
        (
            "fsim-r039 coplanar",
            [[0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 1, 0]],
            0,
        ),
        ("d below", [[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, -1]], 1),
    ];
    for (id, [a, b, c, d], hand) in o3 {
        assert_eq!(
            sgn(o3d_i128(a, b, c, d)),
            sgn(hand),
            "{id}: hand derivation vs oracle"
        );
        assert_eq!(
            orient3d(p3(a), p3(b), p3(c), p3(d)).expect(id),
            sgn(hand),
            "{id}"
        );
    }
    // incircle on the counterclockwise triangle (0,0), (2,0), (0,2): Positive iff d is inside its circle.
    let icc: [(&str, [i128; 2], i128); 3] = [
        ("fsim-r040 inside", [1, 1], 1),
        ("fsim-r041 on", [2, 2], 0),
        ("fsim-r042 outside", [3, 3], -1),
    ];
    for (id, d, hand) in icc {
        let (a, b, c) = ([0, 0], [2, 0], [0, 2]);
        assert_eq!(
            sgn(icc_i128(a, b, c, d)),
            sgn(hand),
            "{id}: hand derivation vs oracle"
        );
        assert_eq!(
            incircle(p2(a), p2(b), p2(c), p2(d)).expect(id),
            sgn(hand),
            "{id}"
        );
    }
    // insphere on the positively oriented tetrahedron on x² + y² + z² = 9.
    let (a, b, c, d) = ([3, 0, 0], [0, 3, 0], [0, 0, 3], [-3, 0, 0]);
    assert_eq!(
        sgn(o3d_i128(a, b, c, d)),
        Sign::Positive,
        "insphere base orientation"
    );
    let isp: [(&str, [i128; 3], i128); 3] = [
        ("centre inside", [0, 0, 0], 1),
        ("on the sphere", [0, -3, 0], 0),
        ("outside", [0, 0, -4], -1),
    ];
    for (id, e, hand) in isp {
        assert_eq!(
            sgn(isp_i128(a, b, c, d, e)),
            sgn(hand),
            "{id}: hand derivation vs oracle"
        );
        assert_eq!(
            insphere(p3(a), p3(b), p3(c), p3(d), p3(e)).expect(id),
            sgn(hand),
            "{id}"
        );
    }
}

// ---------------------------------------------------------------------------
// Generated adversarial families
// ---------------------------------------------------------------------------

#[test]
fn orient2d_bezout_family_defeats_the_filter() {
    let mut rng = LcgRng(0x0B5E_2D00_0000_0001);
    let mut tally = Tally::default();
    for round in 0..300 {
        // det(v, m·v + e·w) = e·det(v, w) = e, while v ~ 2^27 makes every product round.
        let (v, w) = coprime_with_bezout(&mut rng, 1 << 27, 1 << 28);
        let base = [
            rng.range(-(1 << 50), 1 << 50),
            rng.range(-(1 << 50), 1 << 50),
        ];
        let m = rng.range(2, 6);
        let e = rng.range(-1, 2);
        let a = base;
        let b = [base[0] + v[0], base[1] + v[1]];
        let c = [base[0] + m * v[0] + e * w[0], base[1] + m * v[1] + e * w[1]];
        let want = sgn(o2d_i128(a, b, c));
        assert_eq!(want, sgn(e), "construction premise, round {round}");
        let (fa, fb, fc) = (p2(a), p2(b), p2(c));
        let row = format!("orient2d bezout round {round}");
        tally.record(
            orient2d_with_stage(fa, fb, fc).expect(&row),
            want,
            o2d_naive(fa, fb, fc),
            &row,
        );
        assert_eq!(
            orient2d(fb, fa, fc).expect(&row),
            want.flip(),
            "{row}: antisymmetry"
        );
        if want == Sign::Zero {
            let s = orient2d_sos(fa, fb, fc, 0, 1, 2).expect(&row);
            assert_ne!(s, Sign::Zero, "{row}: SoS must be total");
            assert_eq!(
                orient2d_sos(fb, fa, fc, 1, 0, 2).expect(&row),
                s.flip(),
                "{row}: SoS antisymmetry"
            );
        }
    }
    tally.log("orient2d_bezout");
    assert!(
        tally.zeros > 0 && tally.zeros < tally.rows,
        "family needs zeros and non-zeros"
    );
    assert!(
        tally.adaptive + tally.exact > tally.rows / 2,
        "the filter decided too often: weak family"
    );
    assert!(
        tally.naive_wrong > 0,
        "naive float never failed: weak family"
    );
}

#[test]
fn orient2d_kettner_grid_needs_the_exact_stage() {
    // p walks a half-ulp lattice around (0.5, 0.5); q = (12, 12) and r = (24, 24). In units of 2⁻⁵⁴ every
    // coordinate is an integer, which is the oracle's input.
    let mut tally = Tally::default();
    let (q, r) = ([12i128 << 54, 12 << 54], [24i128 << 54, 24 << 54]);
    for i in 0..17i128 {
        for k in 0..17i128 {
            let p = [(1 << 53) + 2 * i, (1 << 53) + 2 * k];
            let fp = [exact(p[0], 54), exact(p[1], 54)];
            let (fq, fr) = (
                [exact(q[0], 54), exact(q[1], 54)],
                [exact(r[0], 54), exact(r[1], 54)],
            );
            let row = format!("kettner i={i} k={k}");
            let want = sgn(o2d_i128(p, q, r));
            tally.record(
                orient2d_with_stage(fp, fq, fr).expect(&row),
                want,
                o2d_naive(fp, fq, fr),
                &row,
            );
            assert_eq!(
                orient2d(fq, fp, fr).expect(&row),
                want.flip(),
                "{row}: antisymmetry"
            );
        }
    }
    tally.log("orient2d_kettner");
    assert!(tally.zeros > 0, "the diagonal cells are exactly collinear");
    assert!(tally.exact > 0, "no row reached stage D: weak family");
    assert!(
        tally.naive_wrong > 0,
        "naive float never failed: weak family"
    );
}

#[test]
fn orient3d_coplanar_family_needs_the_exact_stage() {
    let mut rng = LcgRng(0x0B5E_3D00_0000_0002);
    let mut tally = Tally::default();
    for round in 0..300 {
        // Even rounds: u, v with u_x·v_y − u_y·v_x = 1 and d = a + u + v + e·ẑ, so the determinant is e and every
        // 2×2 minor is tiny while the permanent is about 2⁶³: the filter cannot decide, though naive float can.
        // Odd rounds: generic u, v and d = a + s·u + t·v + e·ẑ, so the minors are about 2⁴¹ and the exact zeros
        // come from cancellation that naive float gets wrong.
        let e = rng.range(-1, 2);
        let (u, v, s, t) = if round % 2 == 0 {
            let (uxy, vxy) = coprime_with_bezout(&mut rng, 1 << 20, 1 << 21);
            let u = [uxy[0], uxy[1], rng.range(-(1 << 20), 1 << 20)];
            let v = [vxy[0], vxy[1], rng.range(-(1 << 20), 1 << 20)];
            (u, v, 1, 1)
        } else {
            let mut r = || rng.range(-(1 << 20), 1 << 20);
            let (u, v) = ([r(), r(), r()], [r(), r(), r()]);
            (u, v, rng.range(-3, 4), rng.range(2, 5))
        };
        let base = [
            rng.range(-(1 << 35), 1 << 35),
            rng.range(-(1 << 35), 1 << 35),
            rng.range(-(1 << 35), 1 << 35),
        ];
        let add = |x: [i128; 3], y: [i128; 3]| [x[0] + y[0], x[1] + y[1], x[2] + y[2]];
        let scale = |k: i128, x: [i128; 3]| [k * x[0], k * x[1], k * x[2]];
        let (a, b, c) = (base, add(base, u), add(base, v));
        let d = add(add(add(base, scale(s, u)), scale(t, v)), [0, 0, e]);
        let want = sgn(o3d_i128(a, b, c, d));
        let normal_z = u[0] * v[1] - u[1] * v[0];
        assert_eq!(
            want == Sign::Zero,
            e == 0 || normal_z == 0,
            "construction premise, round {round}"
        );
        let (fa, fb, fc, fd) = (p3(a), p3(b), p3(c), p3(d));
        let row = format!("orient3d coplanar round {round}");
        tally.record(
            orient3d_with_stage(fa, fb, fc, fd).expect(&row),
            want,
            o3d_naive(fa, fb, fc, fd),
            &row,
        );
        assert_eq!(
            orient3d(fb, fa, fc, fd).expect(&row),
            want.flip(),
            "{row}: antisymmetry"
        );
        if want == Sign::Zero {
            let s = orient3d_sos(fa, fb, fc, fd, [0, 1, 2, 3]).expect(&row);
            assert_ne!(s, Sign::Zero, "{row}: SoS must be total");
            assert_eq!(
                orient3d_sos(fb, fa, fc, fd, [1, 0, 2, 3]).expect(&row),
                s.flip(),
                "{row}: SoS antisymmetry"
            );
        }
    }
    tally.log("orient3d_coplanar");
    assert!(
        tally.zeros > 0 && tally.zeros < tally.rows,
        "family needs zeros and non-zeros"
    );
    assert!(
        tally.exact > tally.rows / 2,
        "the filter decided too often: weak family"
    );
    assert!(
        tally.naive_wrong > 0,
        "naive float never failed: weak family"
    );
}

#[test]
fn incircle_cocircular_family_needs_the_exact_stage() {
    // 5525 = 5²·13·17 has 48 lattice points on x² + y² = 5525. Scaled by an odd k near 2²¹ the radius is about 2²⁸
    // and the lifted squares round (a power-of-two scale would keep every product exact).
    let ring = circle_points(5525);
    assert!(
        ring.len() >= 24,
        "too few lattice points on the circle: {}",
        ring.len()
    );
    let mut rng = LcgRng(0x0B5E_1CC0_0000_0003);
    let mut tally = Tally::default();
    for round in 0..300 {
        let mut pick = [0usize; 4];
        for slot in 0..4 {
            pick[slot] = loop {
                let i = rng.range(0, ring.len() as i128) as usize;
                if !pick[..slot].contains(&i) {
                    break i;
                }
            };
        }
        let offset = [
            rng.range(-(1 << 40), 1 << 40),
            rng.range(-(1 << 40), 1 << 40),
        ];
        let k = rng.range(1 << 20, 1 << 21) * 2 + 1;
        let place = |p: [i128; 2]| [offset[0] + k * p[0], offset[1] + k * p[1]];
        let (a, b, c) = (
            place(ring[pick[0]]),
            place(ring[pick[1]]),
            place(ring[pick[2]]),
        );
        let mut d = place(ring[pick[3]]);
        d[0] += rng.range(-1, 2);
        let want = sgn(icc_i128(a, b, c, d));
        let (fa, fb, fc, fd) = (p2(a), p2(b), p2(c), p2(d));
        let row = format!("incircle cocircular round {round}");
        tally.record(
            incircle_with_stage(fa, fb, fc, fd).expect(&row),
            want,
            icc_naive(fa, fb, fc, fd),
            &row,
        );
        assert_eq!(
            incircle(fb, fa, fc, fd).expect(&row),
            want.flip(),
            "{row}: antisymmetry"
        );
    }
    tally.log("incircle_cocircular");
    assert!(
        tally.zeros > 0 && tally.zeros < tally.rows,
        "family needs zeros and non-zeros"
    );
    assert!(
        tally.exact >= tally.zeros,
        "an exact zero was decided by the filter"
    );
    assert!(
        tally.naive_wrong > 0,
        "naive float never failed: weak family"
    );
}

#[test]
fn insphere_cospherical_family_needs_the_exact_stage() {
    // x² + y² + z² = 81 has 102 lattice points. Scaled by an odd k near 2¹⁶ the radius is about 2²⁰ and the
    // degree-5 terms round.
    let shell = sphere_points(81);
    assert!(
        shell.len() >= 30,
        "too few lattice points on the sphere: {}",
        shell.len()
    );
    let mut rng = LcgRng(0x0B5E_15F0_0000_0004);
    let mut tally = Tally::default();
    let mut round = 0;
    while tally.rows < 200 {
        round += 1;
        let mut pick = [0usize; 5];
        for slot in 0..5 {
            pick[slot] = loop {
                let i = rng.range(0, shell.len() as i128) as usize;
                if !pick[..slot].contains(&i) {
                    break i;
                }
            };
        }
        let offset = [
            rng.range(-(1 << 30), 1 << 30),
            rng.range(-(1 << 30), 1 << 30),
            rng.range(-(1 << 30), 1 << 30),
        ];
        let k = rng.range(1 << 15, 1 << 16) * 2 + 1;
        let place = |p: [i128; 3]| {
            [
                offset[0] + k * p[0],
                offset[1] + k * p[1],
                offset[2] + k * p[2],
            ]
        };
        let pts: Vec<[i128; 3]> = pick.iter().map(|&i| place(shell[i])).collect();
        let (a, b, c, d) = (pts[0], pts[1], pts[2], pts[3]);
        if o3d_i128(a, b, c, d) == 0 {
            continue; // four coplanar points span no sphere
        }
        let mut e = pts[4];
        e[1] += rng.range(-1, 2);
        let want = sgn(isp_i128(a, b, c, d, e));
        let (fa, fb, fc, fd, fe) = (p3(a), p3(b), p3(c), p3(d), p3(e));
        let row = format!("insphere cospherical round {round}");
        tally.record(
            insphere_with_stage(fa, fb, fc, fd, fe).expect(&row),
            want,
            isp_naive(fa, fb, fc, fd, fe),
            &row,
        );
        assert_eq!(
            insphere(fb, fa, fc, fd, fe).expect(&row),
            want.flip(),
            "{row}: antisymmetry"
        );
    }
    tally.log("insphere_cospherical");
    assert!(
        tally.zeros > 0 && tally.zeros < tally.rows,
        "family needs zeros and non-zeros"
    );
    assert!(
        tally.exact >= tally.zeros,
        "an exact zero was decided by the filter"
    );
    assert!(
        tally.naive_wrong > 0,
        "naive float never failed: weak family"
    );
}

// ---------------------------------------------------------------------------
// Refusal rows
// ---------------------------------------------------------------------------

#[test]
fn refusal_rows_are_refused_with_their_codes() {
    let nan = f64::NAN;
    let inf = f64::INFINITY;
    let big = 1e300;
    let m = 1e100;
    let o2 = [0.0, 0.0];
    let o3 = [0.0; 3];
    let rows: Vec<(&str, Result<Sign, fmine_evidence::Refusal>, &str)> = vec![
        (
            "orient2d NaN",
            orient2d([nan, 0.0], [1.0, 0.0], [0.0, 1.0]),
            "FMINE-REF-109",
        ),
        (
            "orient2d -inf",
            orient2d(o2, [-inf, 0.0], [0.0, 1.0]),
            "FMINE-REF-109",
        ),
        (
            "orient2d product overflow",
            orient2d([big, -big], [-big, big], [-big, -big]),
            "FMINE-REF-207",
        ),
        (
            "orient2d difference overflow",
            orient2d([f64::MAX, 0.0], [0.0, 1.0], [-f64::MAX, 0.0]),
            "FMINE-REF-207",
        ),
        (
            "orient3d inf",
            orient3d(o3, [inf, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            "FMINE-REF-109",
        ),
        (
            "orient3d overflow",
            orient3d(
                [big, 0.0, 0.0],
                [0.0, big, 0.0],
                [0.0, 0.0, big],
                [-big, -big, -big],
            ),
            "FMINE-REF-207",
        ),
        (
            "incircle NaN",
            incircle(o2, [1.0, 0.0], [0.0, 1.0], [nan, nan]),
            "FMINE-REF-109",
        ),
        (
            "incircle overflow",
            incircle([m, 0.0], [0.0, m], [-m, 0.0], [0.0, -m * 0.5]),
            "FMINE-REF-207",
        ),
        (
            "insphere NaN",
            insphere(
                o3,
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [nan, 0.0, 0.0],
            ),
            "FMINE-REF-109",
        ),
        (
            "insphere overflow",
            insphere(
                [m, 0.0, 0.0],
                [0.0, m, 0.0],
                [0.0, 0.0, m],
                [-m, 0.0, 0.0],
                [0.0, -m * 0.5, 0.0],
            ),
            "FMINE-REF-207",
        ),
        (
            "orient2d_sos repeated index",
            orient2d_sos(o2, [1.0, 1.0], [2.0, 2.0], 7, 3, 7),
            "FMINE-REF-109",
        ),
        (
            "orient3d_sos repeated index",
            orient3d_sos(o3, o3, o3, o3, [0, 1, 1, 2]),
            "FMINE-REF-109",
        ),
    ];
    for (id, result, code) in rows {
        match result {
            Err(refusal) => {
                assert_eq!(refusal.code, code, "{id}: {refusal:?}");
                println!(
                    "{{\"suite\":\"fmine-geom/predicate_corpus\",\"case\":\"refusal\",\"row\":\"{id}\",\
                     \"code\":\"{code}\"}}"
                );
            }
            Ok(sign) => panic!("{id}: must be refused, got {sign:?}"),
        }
    }
}
