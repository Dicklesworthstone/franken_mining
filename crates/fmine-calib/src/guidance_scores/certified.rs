//! Certified outward Clopper-Pearson endpoints for the explicitly iid Bernoulli reference model.
//! Determinism: Exact (integer binomial enclosures and dyadic root brackets).

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
    clopper_pearson_with_limit(k, n, Rat::new(9, 10).map_err(numerical)?, max_iterations)
}

/// Two-sided Clopper-Pearson interval at a declared coverage level, conditional on iid Bernoulli
/// trials. Endpoints are outward dyadic brackets, with the same certificate and resource bound as CP90.
/// Arithmetic that cannot certify the tail comparison refuses instead of using a floating approximation.
pub fn clopper_pearson(k: usize, n: usize, level: Rat) -> Result<Interval, Refusal> {
    clopper_pearson_with_limit(k, n, level, GRID_BITS as usize)
}

fn clopper_pearson_with_limit(
    k: usize,
    n: usize,
    level: Rat,
    max_iterations: usize,
) -> Result<Interval, Refusal> {
    let one = Rat::from_integer(1);
    if level.signum() <= 0 || exact(level.checked_cmp(one))? != std::cmp::Ordering::Less {
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
        let hi_cmp = exact(bound(tail_hi)?.checked_cmp(tail))?;
        let lo_cmp = exact(bound(tail_lo)?.checked_cmp(tail))?;
        if hi_cmp == std::cmp::Ordering::Less {
            lo = mid;
        } else if lo_cmp == std::cmp::Ordering::Greater {
            hi = mid;
        } else if lo_cmp == std::cmp::Ordering::Equal && hi_cmp == std::cmp::Ordering::Equal {
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
}
