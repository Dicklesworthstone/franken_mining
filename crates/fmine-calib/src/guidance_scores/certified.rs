//! Certified outward Clopper-Pearson endpoints for the explicitly iid Bernoulli reference model.
//! Determinism: Exact (integer binomial enclosures and dyadic root brackets).

use std::cmp::Ordering;
use std::fmt::Display;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::exact_feas::{Interval, Rat};

/// The certified binomial evaluator's resource bound; larger cohorts require a declared partition.
pub const MAX_CP_TRIALS: usize = 4_096;
const GRID_BITS: u32 = 40;
const GRID: u64 = 1_u64 << GRID_BITS;
const PROBABILITY_SCALE: u128 = 1_u128 << 80;

fn numerical(why: impl Display) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        format!("guidance scoring: {why}"),
    )
}

fn exact<T, E: Display>(value: Result<T, E>) -> Result<T, Refusal> {
    value.map_err(numerical)
}

fn closed(lo: Rat, hi: Rat) -> Interval {
    Interval {
        lo,
        hi,
        lo_strict: false,
        hi_strict: false,
    }
}

/// Two-sided CP90, conditional on iid Bernoulli trials. Each endpoint is outward-rounded by at most
/// `2^-40` from the exact binomial-model endpoint. No normal approximation or transcendental is used.
/// Zero trials return the vacuous interval; invalid counts and resource exhaustion refuse.
pub fn clopper_pearson_90(k: usize, n: usize) -> Result<Interval, Refusal> {
    clopper_pearson_90_with_limit(k, n, GRID_BITS as usize)
}

/// As [`clopper_pearson_90`], with an explicit iteration budget per endpoint. An unfinished bracket is
/// refused, never returned as a point estimate or silently widened result.
pub fn clopper_pearson_90_with_limit(
    k: usize,
    n: usize,
    max_iterations: usize,
) -> Result<Interval, Refusal> {
    clopper_pearson_with_limit(k, n, exact(Rat::new(9, 10))?, max_iterations)
}

/// Two-sided Clopper-Pearson at an explicitly supplied confidence level in `(0, 1)`. The level is
/// exact; in particular a frozen 95% rule cannot be silently evaluated as a 90% reference interval.
/// The same outward `2^-40` endpoint and resource guarantees as [`clopper_pearson_90`] apply.
pub fn clopper_pearson(k: usize, n: usize, level: Rat) -> Result<Interval, Refusal> {
    clopper_pearson_with_limit(k, n, level, GRID_BITS as usize)
}

/// As [`clopper_pearson`], with an explicit endpoint iteration budget.
pub fn clopper_pearson_with_limit(
    k: usize,
    n: usize,
    level: Rat,
    max_iterations: usize,
) -> Result<Interval, Refusal> {
    let one = Rat::from_integer(1);
    if level.signum() <= 0 || exact(level.checked_cmp(one))? != Ordering::Less {
        return Err(numerical(
            "confidence level must be strictly between zero and one",
        ));
    }
    let tail = exact(exact(one.checked_sub(level))?.checked_div(Rat::from_integer(2)))?;
    if k > n {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "guidance scoring: success count exceeds trial count",
        ));
    }
    if n > MAX_CP_TRIALS {
        return Err(numerical(
            "binomial trial count exceeds the certified evaluator's resource bound",
        ));
    }
    let lo = lower_endpoint(k, n, tail, max_iterations)?;
    let complement_lo = if n - k == k {
        lo
    } else {
        lower_endpoint(n - k, n, tail, max_iterations)?
    };
    let hi = GRID - complement_lo;
    Ok(closed(
        exact(Rat::new(i128::from(lo), i128::from(GRID)))?,
        exact(Rat::new(i128::from(hi), i128::from(GRID)))?,
    ))
}

/// Return the lower grid neighbour of P(Binomial(n,p) >= k) = tail, certified by enclosures.
fn lower_endpoint(k: usize, n: usize, tail: Rat, max_iterations: usize) -> Result<u64, Refusal> {
    if k == 0 {
        return Ok(0);
    }
    let (mut lo, mut hi) = (0_u64, GRID);
    for _ in 0..max_iterations {
        if hi - lo == 1 {
            return Ok(lo);
        }
        let mid = lo + (hi - lo) / 2;
        let (tail_lo, tail_hi) = binomial_survival_bounds(k, n, mid);
        let bound = |v: u128| {
            exact(Rat::new(
                i128::try_from(v).map_err(numerical)?,
                i128::try_from(PROBABILITY_SCALE).map_err(numerical)?,
            ))
        };
        let high_sign = exact(bound(tail_hi)?.checked_cmp(tail))?;
        let low_sign = exact(bound(tail_lo)?.checked_cmp(tail))?;
        if high_sign == Ordering::Less {
            lo = mid;
        } else if low_sign == Ordering::Greater {
            hi = mid;
        } else if low_sign == Ordering::Equal && high_sign == Ordering::Equal {
            return Ok(mid);
        } else {
            return Err(numerical(
                "binomial tail enclosure cannot certify a root-bracket sign",
            ));
        }
    }
    if hi - lo == 1 {
        Ok(lo)
    } else {
        Err(numerical(format!(
            "binomial root did not converge within {max_iterations} iterations"
        )))
    }
}

/// DESIGN: q[j] encloses P(X >= j) after each trial. Floor/ceiling after each convex combination preserves
/// the enclosure inductively. Values stay in [0, 2^80]; multiplying by grid weights and summing never
/// exceeds 2^120, so all u128 operations below are exact. The accumulated enclosure width is at most n/2^80.
fn binomial_survival_bounds(k: usize, n: usize, p: u64) -> (u128, u128) {
    let p = u128::from(p);
    let q = u128::from(GRID) - p;
    let mask = u128::from(GRID) - 1;
    let floor = |a: u128| a >> GRID_BITS;
    let ceil = |a: u128| floor(a) + u128::from(a & mask != 0);
    if k == n {
        let (mut lo, mut hi) = (PROBABILITY_SCALE, PROBABILITY_SCALE);
        for _ in 0..n {
            lo = floor(p * lo);
            hi = ceil(p * hi);
        }
        return (lo, hi);
    }
    let mut lo = vec![0_u128; k + 1];
    let mut hi = vec![0_u128; k + 1];
    lo[0] = PROBABILITY_SCALE;
    hi[0] = PROBABILITY_SCALE;
    for trial in 1..=n {
        for j in (1..=k.min(trial)).rev() {
            lo[j] = floor(q * lo[j] + p * lo[j - 1]);
            hi[j] = ceil(q * hi[j] + p * hi[j - 1]);
        }
    }
    (lo[k], hi[k])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    const ZERO: Rat = Rat::from_integer(0);
    const ONE: Rat = Rat::from_integer(1);

    fn rat(n: i128, d: i128) -> Rat {
        Rat::new(n, d).unwrap()
    }

    #[test]
    fn test_clopper_pearson_independent_combinatorial_known_answers() {
        // FACT: independent 75-digit Decimal summation of sum(comb(n,j)*p^j*(1-p)^(n-j)),
        // with 260 bisections at tail 1/20. These 24-place oracle values are rounded, not exact roots.
        let cases = [
            (0, 10, "0", "0.258865550893052282678100"),
            (10, 10, "0.741134449106947717321900", "1"),
            (
                1,
                10,
                "0.005116196891823701134742",
                "0.394163302436504781716506",
            ),
            (
                5,
                10,
                "0.222441101008129078568625",
                "0.777558898991870921431375",
            ),
            (
                8,
                10,
                "0.493098698936797592827521",
                "0.963228562112534915361254",
            ),
            (
                18,
                20,
                "0.717381475114139092962780",
                "0.981934796914581377470397",
            ),
            (
                50,
                100,
                "0.413621714630911759724540",
                "0.586378285369088240275460",
            ),
        ];
        let grid_step = rat(1, i128::from(GRID));
        let oracle_half_step = rat(1, 2_000_000_000_000_000_000_000_000);
        for (k, n, l, u) in cases {
            let result = clopper_pearson_90(k, n).unwrap();
            let l = Rat::from_decimal_str(l).unwrap();
            let u = Rat::from_decimal_str(u).unwrap();
            let l_outer = if l == ZERO {
                ZERO
            } else {
                l.checked_sub(oracle_half_step).unwrap()
            };
            let u_outer = if u == ONE {
                ONE
            } else {
                u.checked_add(oracle_half_step).unwrap()
            };
            assert!(
                result.lo.checked_cmp(l_outer).unwrap() != Ordering::Greater,
                "k={k}, n={n}"
            );
            assert!(
                result.hi.checked_cmp(u_outer).unwrap() != Ordering::Less,
                "k={k}, n={n}"
            );
            assert!(
                l.checked_sub(result.lo)
                    .unwrap()
                    .checked_cmp(grid_step)
                    .unwrap()
                    != Ordering::Greater
            );
            assert!(
                result
                    .hi
                    .checked_sub(u)
                    .unwrap()
                    .checked_cmp(grid_step)
                    .unwrap()
                    != Ordering::Greater
            );
        }
    }

    #[test]
    fn test_clopper_pearson_closed_form_edges() {
        let expected = fmine_core::det::exp(fmine_core::det::ln(0.05) / 10.0);
        let upper = clopper_pearson_90(0, 10).unwrap();
        let lower = clopper_pearson_90(10, 10).unwrap();
        assert_eq!(upper.lo, ZERO);
        assert_eq!(lower.hi, ONE);
        assert!((lower.lo.num() as f64 / lower.lo.den() as f64 - expected).abs() < 1e-12);
        assert_eq!(upper.hi, ONE.checked_sub(lower.lo).unwrap());
    }

    #[test]
    fn test_clopper_pearson_refuses_on_nonconvergence() {
        assert!(clopper_pearson_90_with_limit(5, 10, 1).is_err());
        assert!(clopper_pearson_90_with_limit(0, 10, 0).is_err());
        assert!(clopper_pearson_90(11, 10).is_err());
        assert!(clopper_pearson_90(1, MAX_CP_TRIALS + 1).is_err());
        assert_eq!(clopper_pearson_90(0, 0).unwrap(), closed(ZERO, ONE));
    }

    #[test]
    fn test_binomial_enclosure_against_exact_small_polynomials() {
        for n in 1..=8 {
            for k in 1..=n {
                for eighths in 1..8_u64 {
                    let p = rat(i128::from(eighths), 8);
                    let q = ONE.checked_sub(p).unwrap();
                    let pow = |x: Rat, e: usize| (0..e).fold(ONE, |a, _| a.checked_mul(x).unwrap());
                    let mut total = ZERO;
                    let mut choose = 1_i128;
                    for j in 0..=n {
                        if j >= k {
                            let term = Rat::from_integer(choose)
                                .checked_mul(pow(p, j))
                                .unwrap()
                                .checked_mul(pow(q, n - j))
                                .unwrap();
                            total = total.checked_add(term).unwrap();
                        }
                        if j < n {
                            choose = choose * (n - j) as i128 / (j + 1) as i128;
                        }
                    }
                    let (lo, hi) = binomial_survival_bounds(k, n, eighths * GRID / 8);
                    let lo = rat(lo as i128, PROBABILITY_SCALE as i128);
                    let hi = rat(hi as i128, PROBABILITY_SCALE as i128);
                    assert!(lo.checked_cmp(total).unwrap() != Ordering::Greater);
                    assert!(hi.checked_cmp(total).unwrap() != Ordering::Less);
                }
            }
        }
    }

    #[test]
    fn test_clopper_pearson_monotone_and_symmetric() {
        for n in 1..=20 {
            let mut last = closed(ZERO, ZERO);
            for k in 0..=n {
                let current = clopper_pearson_90(k, n).unwrap();
                let mirror = clopper_pearson_90(n - k, n).unwrap();
                assert_eq!(current.lo, ONE.checked_sub(mirror.hi).unwrap());
                assert_eq!(current.hi, ONE.checked_sub(mirror.lo).unwrap());
                assert!(current.lo.checked_cmp(last.lo).unwrap() != Ordering::Less);
                assert!(current.hi.checked_cmp(last.hi).unwrap() != Ordering::Less);
                last = current;
            }
        }
    }

    #[test]
    fn declared_cp95_has_certified_closed_form_edges_and_contains_cp90() {
        let level = rat(19, 20);
        // With one success in one trial, the exact lower endpoint is alpha/2 = 1/40.
        let all = clopper_pearson(1, 1, level).unwrap();
        let step = rat(1, i128::from(GRID));
        assert_eq!(all.hi, ONE);
        assert!(all.lo.checked_cmp(rat(1, 40)).unwrap() != Ordering::Greater);
        assert!(
            all.lo
                .checked_add(step)
                .unwrap()
                .checked_cmp(rat(1, 40))
                .unwrap()
                != Ordering::Less
        );
        let p90 = clopper_pearson_90(3, 10).unwrap();
        let p95 = clopper_pearson(3, 10, level).unwrap();
        assert!(p95.lo.checked_cmp(p90.lo).unwrap() == Ordering::Less);
        assert!(p95.hi.checked_cmp(p90.hi).unwrap() == Ordering::Greater);
        let dyadic = clopper_pearson(1, 1, rat(1, 2)).unwrap();
        assert_eq!(dyadic.lo, rat(1, 4));
        assert_eq!(dyadic.hi, ONE);
        assert!(clopper_pearson(0, 0, ZERO).is_err());
        assert!(clopper_pearson(0, 0, ONE).is_err());
    }

    #[test]
    fn test_frozen_95_percent_level_against_independent_binomial_sum() {
        // FACT: 90-digit Python Decimal direct sum(comb(n,j)*p^j*(1-p)^(n-j)), 300 bisections
        // at tail 1/40. These values are rounded to 24 decimal places, independently of this evaluator.
        let cases = [
            (0, 10, "0", "0.308497107818760823906668"),
            (10, 10, "0.691502892181239176093332", "1"),
            (
                1,
                10,
                "0.002528578544461784502220",
                "0.445016117028195420263300",
            ),
            (
                5,
                10,
                "0.187086028447398531644798",
                "0.812913971552601468355202",
            ),
            (
                8,
                10,
                "0.443904537692358458221792",
                "0.974789273673166629948867",
            ),
            (
                1,
                2,
                "0.012579117093425049128077",
                "0.987420882906574950871923",
            ),
            (
                50,
                100,
                "0.398321129503300979436390",
                "0.601678870496699020563610",
            ),
        ];
        let step = rat(1, i128::from(GRID));
        let rounding = rat(1, 2_000_000_000_000_000_000_000_000);
        for (k, n, lower, upper) in cases {
            let actual = clopper_pearson(k, n, rat(19, 20)).unwrap();
            let lower = Rat::from_decimal_str(lower).unwrap();
            let upper = Rat::from_decimal_str(upper).unwrap();
            let lower_outer = if lower == ZERO {
                ZERO
            } else {
                lower.checked_sub(rounding).unwrap()
            };
            let upper_outer = if upper == ONE {
                ONE
            } else {
                upper.checked_add(rounding).unwrap()
            };
            assert!(
                actual.lo.checked_cmp(lower_outer).unwrap() != Ordering::Greater,
                "{k}/{n}"
            );
            assert!(
                actual.hi.checked_cmp(upper_outer).unwrap() != Ordering::Less,
                "{k}/{n}"
            );
            assert!(
                lower
                    .checked_sub(actual.lo)
                    .unwrap()
                    .checked_cmp(step)
                    .unwrap()
                    != Ordering::Greater
            );
            assert!(
                actual
                    .hi
                    .checked_sub(upper)
                    .unwrap()
                    .checked_cmp(step)
                    .unwrap()
                    != Ordering::Greater
            );
        }
    }

    #[test]
    fn test_90_percent_grid_endpoints_are_unchanged_and_general_levels_refuse_bad_inputs() {
        // The previous CP90 oracle roots rounded outward on the 2^40 grid, independent of the new level API.
        for (k, n, lo, hi) in [
            (0, 10, 0, 284_625_683_238_i128),
            (10, 10, 814_885_944_538, 1_099_511_627_776),
            (1, 10, 5_625_317_972, 433_387_134_272),
            (5, 10, 244_576_577_053, 854_935_050_723),
            (8, 10, 542_167_753_122, 1_059_081_004_249),
        ] {
            let expected = closed(rat(lo, i128::from(GRID)), rat(hi, i128::from(GRID)));
            assert_eq!(clopper_pearson_90(k, n).unwrap(), expected);
            assert_eq!(clopper_pearson(k, n, rat(9, 10)).unwrap(), expected);
        }
        // At 75%, a single success has a dyadic lower endpoint exactly 1/8; equality must certify it.
        assert_eq!(
            clopper_pearson(1, 1, rat(3, 4)).unwrap(),
            closed(rat(1, 8), ONE)
        );
        assert_eq!(
            clopper_pearson(0, 0, rat(19, 20)).unwrap(),
            closed(ZERO, ONE)
        );
        for level in [ZERO, ONE, rat(-1, 2), rat(3, 2)] {
            assert!(clopper_pearson(1, 2, level).is_err());
        }
        assert!(clopper_pearson_with_limit(1, 2, rat(19, 20), 0).is_err());
    }
}
