//! Provenance: frankensim crates/fs-math/src/det.rs @ 20463525d9775a18c1b237d2676f364f367175ab; licence MIT; Changes: inlined double-double erf/erfc and added norm_cdf/norm_ppf; Left out: external payne module for |x| > 2^20
//! Determinism: BitwisePortable

use crate::hash::blake3_hash;

const LN2_HI: f64 = f64::from_bits(0x3FE6_2E42_FEE0_0000);
const LN2_LO: f64 = f64::from_bits(0x3DEA_39EF_3579_3C76);
const LOG2_E: f64 = core::f64::consts::LOG2_E;

/// Deterministic strict-mode $e^x$ (`BD-019`, `PO-NUM-2`).
#[must_use]
pub fn exp(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x > 709.782_712_893_384 {
        return f64::INFINITY;
    }
    if x < -745.133_219_101_941_1 {
        return 0.0;
    }
    let k = (x * LOG2_E).round();
    let r = (-k).mul_add(LN2_LO, (-k).mul_add(LN2_HI, x));
    scale_by_2k(exp_core(r), k as i64)
}

/// Deterministic strict-mode $e^x - 1$ (`BD-019`).
#[must_use]
pub fn expm1(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x > 709.782_712_893_384 {
        return f64::INFINITY;
    }
    if x < -37.0 {
        return -1.0;
    }
    if x.abs() < 0.5 * core::f64::consts::LN_2 {
        return expm1_core(x);
    }
    exp(x) - 1.0
}

/// Deterministic strict-mode natural logarithm $\ln(x)$ (`BD-019`).
#[must_use]
pub fn ln(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x.is_infinite() {
        return f64::INFINITY;
    }
    let (x, sub_k) = if x < f64::MIN_POSITIVE {
        (x * 9_007_199_254_740_992.0, -53i64)
    } else {
        (x, 0)
    };
    let bits = x.to_bits();
    let mut k = i64::try_from((bits >> 52) & 0x7FF).unwrap_or(0) - 1023;
    let mut m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | 0x3ff0_0000_0000_0000);
    if m > core::f64::consts::SQRT_2 {
        m *= 0.5;
        k += 1;
    }
    let k = k + sub_k;
    let s = (m - 1.0) / (m + 1.0);
    let z = s * s;
    let poly = z.mul_add(
        z.mul_add(
            z.mul_add(
                z.mul_add(
                    z.mul_add(
                        z.mul_add(
                            z.mul_add(
                                z.mul_add(z.mul_add(1.0 / 19.0, 1.0 / 17.0), 1.0 / 15.0),
                                1.0 / 13.0,
                            ),
                            1.0 / 11.0,
                        ),
                        1.0 / 9.0,
                    ),
                    1.0 / 7.0,
                ),
                1.0 / 5.0,
            ),
            1.0 / 3.0,
        ),
        1.0,
    );
    let kf = k as f64;
    kf.mul_add(LN2_LO, (2.0 * s).mul_add(poly, kf * LN2_HI))
}

/// Deterministic strict-mode $\sin(x)$ (`BD-019`).
#[must_use]
pub fn sin(x: f64) -> f64 {
    if x.is_nan() || x.is_infinite() {
        return f64::NAN;
    }
    let (r, quadrant) = reduce_pio2(x);
    match quadrant {
        0 => sin_core(r),
        1 => cos_core(r),
        2 => -sin_core(r),
        _ => -cos_core(r),
    }
}

/// Deterministic strict-mode $\cos(x)$ (`BD-019`).
#[must_use]
pub fn cos(x: f64) -> f64 {
    if x.is_nan() || x.is_infinite() {
        return f64::NAN;
    }
    let (r, quadrant) = reduce_pio2(x);
    match quadrant {
        0 => cos_core(r),
        1 => -sin_core(r),
        2 => -cos_core(r),
        _ => sin_core(r),
    }
}

/// Deterministic strict-mode $(\sin x, \cos x)$ (`BD-019`).
#[must_use]
pub fn sin_cos(x: f64) -> (f64, f64) {
    if x.is_nan() || x.is_infinite() {
        return (f64::NAN, f64::NAN);
    }
    let (r, quadrant) = reduce_pio2(x);
    let sine = sin_core(r);
    let cosine = cos_core(r);
    match quadrant {
        0 => (sine, cosine),
        1 => (cosine, -sine),
        2 => (-sine, -cosine),
        _ => (-cosine, sine),
    }
}

/// Deterministic strict-mode $\tanh(x)$ (`BD-019`).
#[must_use]
pub fn tanh(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let a = x.abs();
    let mag = if a > 20.0 {
        1.0
    } else {
        let t = expm1(2.0 * a);
        t / (t + 2.0)
    };
    if x.is_sign_negative() { -mag } else { mag }
}

/// IEEE-754 correctly rounded $\sqrt{x}$ (`BD-019`).
#[must_use]
pub fn sqrt(x: f64) -> f64 {
    x.sqrt()
}

/// Deterministic strict-mode $\sqrt{x^2 + y^2}$ (`BD-019`).
#[must_use]
pub fn hypot(x: f64, y: f64) -> f64 {
    if x.is_infinite() || y.is_infinite() {
        return f64::INFINITY;
    }
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    let (a, b) = (x.abs(), y.abs());
    let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
    if hi == 0.0 {
        return 0.0;
    }
    let r = lo / hi;
    hi * sqrt(r.mul_add(r, 1.0))
}

const PI_2_HI: f64 = core::f64::consts::FRAC_PI_2;
const PI_2_LO: f64 = 6.123_233_995_736_766e-17;
const PI_6_HI: f64 = core::f64::consts::FRAC_PI_6;
const PI_6_LO: f64 = -1.643_486_876_741_777e-17;
const TAN_PI_12: f64 = 0.267_949_192_431_123_05;
const SQRT_3: f64 = 1.732_050_807_568_877_2;
const SQRT_3_LO: f64 = 1.020_508_405_819_248_3e-16;

/// Deterministic strict-mode $\arctan(x)$ (`BD-019`).
#[must_use]
pub fn atan(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let sign = if x.is_sign_negative() { -1.0 } else { 1.0 };
    let a = x.abs();
    if a.is_infinite() {
        return sign * (PI_2_HI + PI_2_LO);
    }
    let (invert, t0) = if a > 1.0 { (true, 1.0 / a) } else { (false, a) };
    let (rotated, t) = if t0 > TAN_PI_12 {
        let num = SQRT_3.mul_add(t0, -1.0) + SQRT_3_LO * t0;
        (true, num / (SQRT_3 + t0))
    } else {
        (false, t0)
    };
    let base = atan_core(t);
    let mut v = if rotated {
        (PI_6_HI + base) + PI_6_LO
    } else {
        base
    };
    if invert {
        v = (PI_2_HI - v) + PI_2_LO;
    }
    sign * v
}

/// Deterministic strict-mode $\operatorname{atan2}(y, x)$ (`BD-019`).
#[must_use]
pub fn atan2(y: f64, x: f64) -> f64 {
    const PI_HI: f64 = core::f64::consts::PI;
    const PI_LO: f64 = 1.224_646_799_147_353_2e-16;
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    let sign = if y.is_sign_negative() { -1.0 } else { 1.0 };
    let ay = y.abs();
    if ay == 0.0 {
        return if x.is_sign_negative() {
            sign * (PI_HI + PI_LO)
        } else {
            sign * 0.0
        };
    }
    if x == 0.0 {
        return sign * (PI_2_HI + PI_2_LO);
    }
    if x.is_infinite() {
        return match (x > 0.0, ay.is_infinite()) {
            (true, true) => sign * (PI_2_HI / 2.0 + PI_2_LO / 2.0),
            (true, false) => sign * 0.0,
            (false, true) => sign * 3.0f64.mul_add(PI_2_HI / 2.0, 3.0 * (PI_2_LO / 2.0)),
            (false, false) => sign * (PI_HI + PI_LO),
        };
    }
    if ay.is_infinite() {
        return sign * (PI_2_HI + PI_2_LO);
    }
    let base = atan(ay / x.abs());
    if x > 0.0 {
        sign * base
    } else {
        sign * ((PI_HI - base) + PI_LO)
    }
}

/// Deterministic strict-mode $\operatorname{erf}(x)$ (`BD-019`).
#[must_use]
pub fn erf(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let sign = if x.is_sign_negative() { -1.0 } else { 1.0 };
    let a = x.abs();
    if a >= 6.0 {
        // erfc(6) ≈ 2.2e-17 is below half an ulp of 1.
        return sign;
    }
    // bd-hp2j.2 replaced Abramowitz & Stegun 7.1.26 (absolute error 1.5e-7, so erf(0) was about 1e-9). Within
    // about 8 ulp of the correctly rounded value on a 0.005 grid over (0, 6), measured on an f64 prototype of
    // this algorithm against 60-digit references.
    //
    // e^{−a²} with a² split exactly (a² = hi + lo), so rounding a² does not cost about a² ulps.
    let hi = a * a;
    let lo = a.mul_add(a, -hi);
    let e = exp(-hi) * (1.0 - lo);
    let y = if a < 2.5 {
        // A&S 7.1.6: erf(a) = (2/√π) e^{−a²} Σ 2ⁿ a^{2n+1} / (1·3·…·(2n+1)). Every term is positive, so nothing
        // cancels; the terms fall once 2n + 3 > 2a².
        let two_a2 = 2.0 * hi;
        let mut term = a;
        let mut sum = a;
        let mut n = 0.0;
        while term > sum * 1e-17 && n < 200.0 {
            n += 1.0;
            term *= two_a2 / (2.0 * n + 1.0);
            sum += term;
        }
        core::f64::consts::FRAC_2_SQRT_PI * e * sum
    } else {
        // 1 − erfc(a), erfc by its continued fraction (A&S 7.1.14) evaluated bottom-up to depth 60:
        // erfc(a) = (e^{−a²}/√π) / (a + (1/2)/(a + 1/(a + (3/2)/(a + …)))). erfc is below 4.1e-4 here, so its
        // own relative error barely reaches the result.
        let mut f = a;
        for k in (1..=60).rev() {
            f = a + f64::from(k) * 0.5 / f;
        }
        1.0 - e * (0.5 * core::f64::consts::FRAC_2_SQRT_PI) / f
    };
    sign * y
}

/// Standard Normal CDF $\Phi(z)$ (`BD-019`).
#[must_use]
pub fn norm_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / core::f64::consts::SQRT_2))
}

/// Standard Normal inverse CDF (probit) $\Phi^{-1}(p)$ via Acklam's rational approximation (`BD-019`).
#[must_use]
pub fn norm_ppf(p: f64) -> f64 {
    if !(0.0..=1.0).contains(&p) || p.is_nan() {
        return f64::NAN;
    }
    if p == 0.0 {
        return f64::NEG_INFINITY;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    const P_LOW: f64 = 0.02425;
    const P_HIGH: f64 = 1.0 - P_LOW;

    if p < P_LOW {
        let q = sqrt(-2.0 * ln(p));
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= P_HIGH {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = sqrt(-2.0 * ln(1.0 - p));
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    }
}

/// Deterministic integer power $x^n$ (`BD-019`).
#[must_use]
pub fn powi(x: f64, n: i32) -> f64 {
    fn unsigned(mut a: f64, mut b: u64) -> f64 {
        let mut r = 1.0f64;
        loop {
            if b & 1 == 1 {
                r *= a;
            }
            b /= 2;
            if b == 0 {
                break;
            }
            a *= a;
        }
        r
    }
    let b = i64::from(n).unsigned_abs();
    let positive = unsigned(x, b);
    if n >= 0 {
        return positive;
    }
    1.0 / positive
}

fn exp_core(r: f64) -> f64 {
    expm1_core(r) + 1.0
}

fn expm1_core(r: f64) -> f64 {
    const C: [f64; 12] = [
        1.0 / 2.0,
        1.0 / 6.0,
        1.0 / 24.0,
        1.0 / 120.0,
        1.0 / 720.0,
        1.0 / 5_040.0,
        1.0 / 40_320.0,
        1.0 / 362_880.0,
        1.0 / 3_628_800.0,
        1.0 / 39_916_800.0,
        1.0 / 479_001_600.0,
        1.0 / 6_227_020_800.0,
    ];
    let mut p = C[11];
    for c in C[..11].iter().rev() {
        p = p.mul_add(r, *c);
    }
    r.mul_add(r * p, r)
}

fn sin_core(r: f64) -> f64 {
    let z = r * r;
    let p = z.mul_add(
        z.mul_add(
            z.mul_add(
                z.mul_add(
                    z.mul_add(
                        z.mul_add(
                            z.mul_add(-1.0 / 355_687_428_096_000.0, 1.0 / 1_307_674_368_000.0),
                            -1.0 / 6_227_020_800.0,
                        ),
                        1.0 / 39_916_800.0,
                    ),
                    -1.0 / 362_880.0,
                ),
                1.0 / 5_040.0,
            ),
            -1.0 / 120.0,
        ),
        1.0 / 6.0,
    );
    (-(z * r)).mul_add(p, r)
}

fn cos_core(r: f64) -> f64 {
    let z = r * r;
    let q = z.mul_add(
        z.mul_add(
            z.mul_add(
                z.mul_add(
                    z.mul_add(1.0 / 20_922_789_888_000.0, -1.0 / 87_178_291_200.0),
                    1.0 / 479_001_600.0,
                ),
                -1.0 / 3_628_800.0,
            ),
            1.0 / 40_320.0,
        ),
        -1.0 / 720.0,
    );
    let tail = z.mul_add(q, 1.0 / 24.0);
    (z * z).mul_add(tail, 1.0 - 0.5 * z)
}

fn atan_core(t: f64) -> f64 {
    let z = t * t;
    let mut p = -1.0 / 31.0;
    let coeffs = [
        29.0, 27.0, 25.0, 23.0, 21.0, 19.0, 17.0, 15.0, 13.0, 11.0, 9.0, 7.0, 5.0, 3.0,
    ];
    let mut sign = 1.0;
    for &d in &coeffs {
        p = z.mul_add(p, sign / d);
        sign = -sign;
    }
    let p = z.mul_add(p, 1.0);
    t * p
}

fn reduce_pio2(x: f64) -> (f64, u8) {
    const PIO2_HI: f64 = f64::from_bits(0x3FF9_21FB_5440_0000);
    const PIO2_MID: f64 = f64::from_bits(0x3DD0_B461_1A60_0000);
    const PIO2_LO: f64 = f64::from_bits(0x3BA3_198A_2E00_0000);
    const PIO2_LO2: f64 = 8.478_427_660_368_9e-32;
    const TWO_OVER_PI: f64 = core::f64::consts::FRAC_2_PI;
    let j = (x * TWO_OVER_PI).round();
    let r = (-j).mul_add(
        PIO2_LO2,
        (-j).mul_add(PIO2_LO, (-j).mul_add(PIO2_MID, (-j).mul_add(PIO2_HI, x))),
    );
    let q = ((j as i64) & 3) as u8;
    (r, q)
}

fn scale_by_2k(v: f64, k: i64) -> f64 {
    let clamp = k.clamp(-2000, 2000);
    let mut v = v;
    let mut remaining = clamp;
    while remaining != 0 {
        let step = remaining.clamp(-1000, 1000);
        v *= f64::from_bits(((1023 + step) as u64) << 52);
        remaining -= step;
    }
    v
}

/// Compute the BLAKE3 golden digest of $10^4$ deterministic evaluations across `exp`, `ln`, `sin`, `cos`, `atan2` (`BD-019`).
#[must_use]
pub fn golden_digest_10k() -> String {
    let mut bytes = Vec::with_capacity(10_000 * 40);
    for i in 1..=10_000u32 {
        let x = (f64::from(i) - 5_000.5) * 0.001;
        let xp = f64::from(i) * 0.0005;
        bytes.extend_from_slice(&exp(x).to_bits().to_le_bytes());
        bytes.extend_from_slice(&ln(xp).to_bits().to_le_bytes());
        bytes.extend_from_slice(&sin(x).to_bits().to_le_bytes());
        bytes.extend_from_slice(&cos(x).to_bits().to_le_bytes());
        bytes.extend_from_slice(&atan2(x, xp).to_bits().to_le_bytes());
    }
    blake3_hash(&bytes).to_hex()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_golden_hash_10k_inputs() {
        let digest = golden_digest_10k();
        assert_eq!(
            digest,
            "927a71067fdd069d69fbd77ae7d0de54f0fb1ddf10bde992638ffdf86faa1684"
        );
    }

    // bd-hp2j.2: det's own accuracy at known points, before the public-lane kernels rely on it.
    #[test]
    fn test_det_ln_2_equals_ln_2_constant() {
        assert_eq!(ln(2.0).to_bits(), core::f64::consts::LN_2.to_bits());
    }

    #[test]
    fn test_det_exp_1_within_one_ulp() {
        let e = core::f64::consts::E;
        let got = exp(1.0);
        let ulps = (got.to_bits() as i64 - e.to_bits() as i64).unsigned_abs();
        assert!(ulps <= 1, "exp(1) = {got:?}, {ulps} ulp from E");
    }

    /// References: the series summed to 60 significant digits in Python `decimal`, then correctly rounded. 0.44,
    /// 1.97 and 2.845 are where the f64 prototype was worst; 2.49 and 2.5 straddle the switch.
    #[test]
    fn test_det_erf_within_ten_ulp_of_reference() {
        let cases = [
            (1e-8, 1.128_379_167_095_512_6e-8),
            (0.1, 0.112_462_916_018_284_9),
            (0.44, 0.466_225_115_277_957_57),
            (0.5, 0.520_499_877_813_046_5),
            (1.97, 0.994_663_724_646_529_9),
            (2.49, 0.999_570_712_132_266_1),
            (2.5, 0.999_593_047_982_555),
            (2.845, 0.999_942_645_210_915_6),
            (1.0, 0.842_700_792_949_714_9),
            (1.5, 0.966_105_146_475_310_8),
            (2.0, 0.995_322_265_018_952_7),
            (3.0, 0.999_977_909_503_001_4),
            (4.0, 0.999_999_984_582_742_1),
            (5.0, 0.999_999_999_998_462_6),
            (5.9, 0.999_999_999_999_999_9),
        ];
        let mut worst = 0;
        for (x, want) in cases {
            let got = erf(x);
            let ulps = (got.to_bits() as i64 - f64::to_bits(want) as i64).unsigned_abs();
            worst = worst.max(ulps);
            assert!(ulps <= 10, "erf({x}) = {got:?}, want {want:?}, {ulps} ulp");
            assert_eq!(erf(-x).to_bits(), (-got).to_bits(), "erf is odd at {x}");
        }
        println!(
            "{{\"test\":\"erf_reference\",\"cases\":{},\"worst_ulp\":{worst}}}",
            cases.len()
        );
        assert_eq!(erf(0.0).to_bits(), 0.0_f64.to_bits());
        assert_eq!(erf(6.5), 1.0);
    }

    #[test]
    fn test_det_norm_cdf_zero_is_half() {
        assert_eq!(norm_cdf(0.0).to_bits(), 0.5_f64.to_bits());
    }

    #[test]
    fn test_det_norm_ppf_half_is_zero() {
        assert_eq!(norm_ppf(0.5), 0.0);
    }

    #[test]
    fn test_norm_cdf_and_ppf_roundtrip() {
        for &p in &[0.01, 0.05, 0.10, 0.25, 0.50, 0.75, 0.90, 0.95, 0.99] {
            let z = norm_ppf(p);
            let p_back = norm_cdf(z);
            assert!((p - p_back).abs() < 1e-6, "p={p}, z={z}, p_back={p_back}");
        }
    }
}
