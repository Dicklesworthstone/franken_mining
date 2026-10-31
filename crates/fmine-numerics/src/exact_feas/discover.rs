//! Discovery of a `Sum` family's rounding construction (bd-4wvl.4, BD-340.S4, part 1): which of `ComponentSum`
//! and `RoundedOnce` the printed footings of one family fit. A report, never a verdict: it reads finished
//! footings and changes no state.
//!
//! - **Statistic.** The number of totals that foot exactly: the printed total equals the sum of its printed
//!   summands. `ComponentSum` accepts exactly those. `RoundedOnce` accepts a footing whose residual is within
//!   the half-steps of its boxes, `2·|residual| ≤ k + 1` in units of the common step.
//! - **Null.** Under `RoundedOnce` the printed total is the true total rounded once, and the true total is the
//!   sum of the true summands. With `N` the sum of the k printed summands and eᵢ their rounding errors, the
//!   total prints `round(N − Σeᵢ)`, which is `N` exactly when `|Σeᵢ| < ½`. For eᵢ i.i.d. uniform on (−½, ½)
//!   that is p₀(k), computed exactly from the Irwin–Hall distribution ([`p0`]): p₀(1) = 1, p₀(2) = 3/4,
//!   p₀(3) = 2/3, p₀(4) = 115/192. A placeholder summand is an exact 0 and is not among the k.
//! - **Report.** The expected count of exact footings under `RoundedOnce`, Σ p₀(kᵢ), exactly; the upper and lower
//!   tails of the observed count under that null (independent footings, a Poisson-binomial law), as natural
//!   logarithms computed in `f64` through `fmine_core::det`.
//! - **Decision**, declared before any real-file run, with α = 10⁻⁶ ([`ALPHA`]) and ε = 1/10
//!   ([`MAX_ERROR_RATE`]). A construction may reject some footings, as issuer errors, at a rate up to ε. With n
//!   footings it rejects m, it is ruled out when P(Binomial(n, ε) ≥ m) < α. `RoundedOnce` is also ruled out
//!   when either tail of the exact count is below α. Then:
//!   - only `ComponentSum` fits: [`Discovered::ComponentSum`];
//!   - only `RoundedOnce` fits: [`Discovered::RoundedOnce`];
//!   - both or neither: [`Discovered::Ambiguous`], carrying [`NON_IDENTIFIABLE`] (FMINE-REF-104, exit 3). One
//!     total that misses by a unit is as much one issuer error as one rounding, so it identifies nothing.
//!
//! A footing with more than [`MAX_K`] summands is indeterminate (its p₀ is not computed) and is counted apart.
//! The tails take the footings as independent draws, so the caller passes each distinct footing once: a table
//! reprinted as a later release's comparative is the same draw, not a second one.
//!
//! Determinism: DeterministicPerIsa (p₀ and the expected count are exact rationals; the tails are `f64` through
//! `fmine_core::det`, in a fixed order).

use fmine_core::det;
use fmine_units::UnitsError;

use super::Rat;

/// Footings with more summands are indeterminate.
pub const MAX_K: u32 = 20;

/// The refusal an ambiguous discovery carries: no single construction is identified.
pub const NON_IDENTIFIABLE: &str = "FMINE-REF-104";

/// The tail probability below which a construction is ruled out.
pub const ALPHA: f64 = 1e-6;

/// The highest rate of footings a construction may reject as issuer errors and still fit.
pub const MAX_ERROR_RATE: f64 = 0.1;

/// p₀(k) = P(|e₁ + … + e_k| < ½) for eᵢ i.i.d. uniform on (−½, ½), exactly; `None` past [`MAX_K`].
///
/// With S + k/2 Irwin–Hall on [0, k], p₀(k) = F((k + 1)/2) − F((k − 1)/2), where
/// k!·F(x) = Σ_{0 ≤ j ≤ x} (−1)ʲ C(k, j)(x − j)ᵏ. Multiplying by 2ᵏ keeps every term an integer.
#[must_use]
pub fn p0(k: u32) -> Option<Rat> {
    if k > MAX_K {
        return None;
    }
    if k == 0 {
        return Some(Rat::from_integer(1));
    }
    let kk = i128::from(k);
    // Σ_{0 ≤ j, 2j ≤ c} (−1)ʲ C(k, j)(c − 2j)ᵏ, for c = k + 1 and c = k − 1.
    let sum = |c: i128| -> Option<i128> {
        let mut total: i128 = 0;
        let mut binom: i128 = 1;
        let mut j: i128 = 0;
        while 2 * j <= c {
            let term = binom.checked_mul((c - 2 * j).checked_pow(k)?)?;
            total = if j % 2 == 0 {
                total.checked_add(term)?
            } else {
                total.checked_sub(term)?
            };
            binom = binom.checked_mul(kk - j)? / (j + 1);
            j += 1;
        }
        Some(total)
    };
    let num = sum(kk + 1)?.checked_sub(sum(kk - 1)?)?;
    let factorial = (1..=kk).try_fold(1_i128, i128::checked_mul)?;
    let den = factorial.checked_mul(1_i128.checked_shl(k)?)?;
    Rat::new(num, den).ok()
}

/// One decided footing: `k` numeric summands and `residual` = Σ printed summands − printed total, in units of
/// the step every operand shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Footing {
    pub k: u32,
    pub residual: i128,
}

/// What the footings of one family fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Discovered {
    ComponentSum,
    RoundedOnce,
    /// No single construction is identified (FMINE-REF-104).
    Ambiguous,
}

impl Discovered {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ComponentSum => "component_sum",
            Self::RoundedOnce => "rounded_once",
            Self::Ambiguous => "ambiguous",
        }
    }

    /// [`NON_IDENTIFIABLE`] for an ambiguous discovery, else `None`.
    #[must_use]
    pub const fn reason_code(self) -> Option<&'static str> {
        match self {
            Self::Ambiguous => Some(NON_IDENTIFIABLE),
            Self::ComponentSum | Self::RoundedOnce => None,
        }
    }
}

/// The construction report of one family.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstructionReport {
    /// Footings scored (at most [`MAX_K`] summands).
    pub n: usize,
    /// Scored footings whose residual is 0: what `ComponentSum` accepts.
    pub foot_exact: usize,
    /// Scored footings with `2·|residual| ≤ k + 1`: what `RoundedOnce` accepts.
    pub rounded_once_accepts: usize,
    /// Footings with more than [`MAX_K`] summands: indeterminate, not scored.
    pub indeterminate: usize,
    /// Σ p₀(kᵢ) over the scored footings: the expected exact count under `RoundedOnce`.
    pub expected_rounded_once: Rat,
    /// ln P(X ≥ `foot_exact`) under `RoundedOnce`.
    pub ln_upper_tail: f64,
    /// ln P(X ≤ `foot_exact`) under `RoundedOnce`.
    pub ln_lower_tail: f64,
    /// ln P(Binomial(n, ε) ≥ n − `foot_exact`): the footings `ComponentSum` rejects, as issuer errors.
    pub ln_component_sum_error_tail: f64,
    /// ln P(Binomial(n, ε) ≥ n − `rounded_once_accepts`): the footings `RoundedOnce` rejects.
    pub ln_rounded_once_error_tail: f64,
    pub discovered: Discovered,
}

/// ln(eᵃ + eᵇ), with −∞ as the log of 0.
fn ln_add(a: f64, b: f64) -> f64 {
    let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
    if lo == f64::NEG_INFINITY {
        return hi;
    }
    hi + det::ln(1.0 + det::exp(lo - hi))
}

/// ln P(X = j) for j = 0..=n, X the number of successes of independent trials with the given probabilities.
fn ln_poisson_binomial(ps: &[Rat]) -> Vec<f64> {
    let mut ln_pmf = vec![0.0_f64];
    for p in ps {
        let ln_p = det::ln(p.to_f64());
        let ln_q = det::ln(1.0 - p.to_f64());
        let mut next = vec![f64::NEG_INFINITY; ln_pmf.len() + 1];
        for (j, &l) in ln_pmf.iter().enumerate() {
            next[j] = ln_add(next[j], l + ln_q);
            next[j + 1] = ln_add(next[j + 1], l + ln_p);
        }
        ln_pmf = next;
    }
    ln_pmf
}

/// ln P(Binomial(n, [`MAX_ERROR_RATE`]) ≥ m); 0 for m = 0, −∞ for m > n.
fn ln_error_tail(n: usize, m: usize) -> f64 {
    let (ln_e, ln_f) = (det::ln(MAX_ERROR_RATE), det::ln(1.0 - MAX_ERROR_RATE));
    // ln j! for j = 0..=n.
    let mut ln_fact = vec![0.0_f64; n + 1];
    for j in 1..=n {
        ln_fact[j] = ln_fact[j - 1] + det::ln(j as f64);
    }
    (m..=n).fold(f64::NEG_INFINITY, |acc, j| {
        let ln_choose = ln_fact[n] - ln_fact[j] - ln_fact[n - j];
        ln_add(acc, ln_choose + j as f64 * ln_e + (n - j) as f64 * ln_f)
    })
}

/// The construction report for one family's decided footings. An overflow in the expected count (it cannot
/// occur below [`MAX_K`] summands per footing and 10⁶ footings, but is checked) is an error.
pub fn discover_construction(footings: &[Footing]) -> Result<ConstructionReport, UnitsError> {
    let mut ps = Vec::new();
    let (mut foot_exact, mut rounded_once_accepts, mut indeterminate) = (0, 0, 0);
    let mut expected = Rat::from_integer(0);
    for f in footings {
        let Some(p) = p0(f.k) else {
            indeterminate += 1;
            continue;
        };
        expected = expected.checked_add(p)?;
        ps.push(p);
        if f.residual == 0 {
            foot_exact += 1;
        }
        let twice = f.residual.checked_abs().and_then(|a| a.checked_mul(2));
        if twice.is_some_and(|t| t <= i128::from(f.k) + 1) {
            rounded_once_accepts += 1;
        }
    }
    let ln_pmf = ln_poisson_binomial(&ps);
    let fold = |range: &[f64]| {
        range
            .iter()
            .fold(f64::NEG_INFINITY, |acc, &l| ln_add(acc, l))
    };
    let ln_upper_tail = fold(&ln_pmf[foot_exact..]);
    let ln_lower_tail = fold(&ln_pmf[..=foot_exact]);
    let n = ps.len();
    let ln_component_sum_error_tail = ln_error_tail(n, n - foot_exact);
    let ln_rounded_once_error_tail = ln_error_tail(n, n - rounded_once_accepts);
    let ln_alpha = det::ln(ALPHA);
    let component_sum_fits = ln_component_sum_error_tail >= ln_alpha;
    let rounded_once_fits = ln_rounded_once_error_tail >= ln_alpha
        && ln_upper_tail >= ln_alpha
        && ln_lower_tail >= ln_alpha;
    let discovered = match (n > 0, component_sum_fits, rounded_once_fits) {
        (true, true, false) => Discovered::ComponentSum,
        (true, false, true) => Discovered::RoundedOnce,
        _ => Discovered::Ambiguous,
    };
    Ok(ConstructionReport {
        n,
        foot_exact,
        rounded_once_accepts,
        indeterminate,
        expected_rounded_once: expected,
        ln_upper_tail,
        ln_lower_tail,
        ln_component_sum_error_tail,
        ln_rounded_once_error_tail,
        discovered,
    })
}

#[cfg(test)]
mod tests {
    use fmine_core::rand::{PhiloxStream, RngKey};

    use super::*;

    fn rat(n: i128, d: i128) -> Rat {
        Rat::new(n, d).expect("rational")
    }

    #[test]
    fn test_p0_exact_values() {
        assert_eq!(p0(0), Some(rat(1, 1)));
        assert_eq!(p0(1), Some(rat(1, 1)));
        assert_eq!(p0(2), Some(rat(3, 4)));
        assert_eq!(p0(3), Some(rat(2, 3)));
        assert_eq!(p0(4), Some(rat(115, 192)));
        assert_eq!(p0(MAX_K + 1), None);
        // Every p₀ up to MAX_K is a probability that falls with k (the sum spreads out).
        let mut last = rat(1, 1);
        for k in 2..=MAX_K {
            let p = p0(k).expect("within MAX_K");
            assert!(p.signum() > 0, "p0({k}) > 0");
            assert_eq!(p.checked_cmp(last), Ok(std::cmp::Ordering::Less), "p0({k})");
            last = p;
        }
        // Σ p₀ over every k up to MAX_K, fifty times over (about the real ledger's size): the exact sum does not
        // overflow, since its denominator stays the lcm of the p₀ denominators.
        let mixed: Vec<Footing> = (0..50)
            .flat_map(|_| (0..=MAX_K).map(|k| Footing { k, residual: 0 }))
            .collect();
        let r = discover_construction(&mixed).expect("no overflow");
        assert_eq!(r.n, 1_050);
    }

    /// 10⁵ seeded Philox draws of k uniforms per k: the share with |Σ| < ½ is within 4σ of p₀(k).
    #[test]
    fn prop_p0_matches_seeded_monte_carlo() {
        let draws = 100_000_u32;
        for k in [1_u32, 2, 3, 4, 6, 9, 13, 20] {
            let mut rng = PhiloxStream::new(RngKey {
                seed_root: 0x4d30_4a53,
                partition: 4,
                domain: 4,
                var_group: k,
                realization: 0,
                brick: 0,
            });
            let hits = (0..draws)
                .filter(|_| {
                    let s: f64 = (0..k).map(|_| rng.next_f64_open01() - 0.5).sum();
                    s.abs() < 0.5
                })
                .count();
            let p = p0(k).expect("within MAX_K").to_f64();
            let observed = hits as f64 / f64::from(draws);
            let sigma = (p * (1.0 - p) / f64::from(draws)).sqrt().max(1e-9);
            assert!(
                (observed - p).abs() <= 4.0 * sigma + 1e-12,
                "k {k}: observed {observed}, p0 {p}"
            );
        }
    }

    #[test]
    fn test_poisson_binomial_tails_known_answers() {
        // Two fair trials: P(X ≥ 1) = 3/4, P(X ≥ 2) = 1/4, P(X ≤ 0) = 1/4.
        let half = rat(1, 2);
        let ln = ln_poisson_binomial(&[half, half]);
        let p: Vec<f64> = ln.iter().map(|&l| det::exp(l)).collect();
        for (got, want) in p.iter().zip([0.25, 0.5, 0.25]) {
            assert!((got - want).abs() < 1e-12, "{p:?}");
        }
        // 400 footings of two summands, all exact: P(X ≥ 400) = (3/4)^400, ln = 400·ln(3/4).
        let footings = vec![Footing { k: 2, residual: 0 }; 400];
        let r = discover_construction(&footings).expect("report");
        let want = 400.0 * det::ln(0.75);
        assert!(
            (r.ln_upper_tail - want).abs() < 1e-9 * want.abs(),
            "{}",
            r.ln_upper_tail
        );
        assert!(
            r.ln_lower_tail.abs() < 1e-9,
            "P(X ≤ n) = 1: {}",
            r.ln_lower_tail
        );
        assert_eq!(r.expected_rounded_once, rat(300, 1));
        assert_eq!(r.discovered, Discovered::ComponentSum);
    }

    /// Synthetic footings drawn under each construction are discovered as it.
    #[test]
    fn test_construction_discovery_synthetic_component_sum_and_rounded_once() {
        // ComponentSum: totals are the sum of the printed lines, so every residual is 0 (k from 2 to 8).
        let component: Vec<Footing> = (0..300)
            .map(|i| Footing {
                k: 2 + i % 7,
                residual: 0,
            })
            .collect();
        let r = discover_construction(&component).expect("report");
        assert_eq!((r.n, r.foot_exact, r.rounded_once_accepts), (300, 300, 300));
        assert_eq!(r.discovered, Discovered::ComponentSum);

        // RoundedOnce: true lines with fractional parts, each line and the total rounded once.
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 0x4d30_4a53,
            partition: 4,
            domain: 5,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let rounded: Vec<Footing> = (0..300_u32)
            .map(|i| {
                let k = 2 + i % 7;
                let lines: Vec<f64> = (0..k).map(|_| rng.next_f64_open01() * 100.0).collect();
                let printed: i64 = lines.iter().map(|x| x.round() as i64).sum();
                let total = lines.iter().sum::<f64>().round() as i64;
                Footing {
                    k,
                    residual: i128::from(printed - total),
                }
            })
            .collect();
        let r = discover_construction(&rounded).expect("report");
        assert_eq!(
            r.rounded_once_accepts, 300,
            "every rounding-once residual is within the half-steps"
        );
        assert!(r.foot_exact < 300);
        assert_eq!(r.discovered, Discovered::RoundedOnce, "{r:?}");
    }

    #[test]
    fn test_ambiguous_discovery_refuses_ref_104() {
        // Few footings, all exact: both constructions accept all and three exact two-line totals are likely
        // under rounding once (27/64).
        let few = vec![Footing { k: 2, residual: 0 }; 3];
        let r = discover_construction(&few).expect("report");
        assert_eq!(r.discovered, Discovered::Ambiguous);
        assert_eq!(r.discovered.reason_code(), Some("FMINE-REF-104"));
        assert_eq!(Discovered::ComponentSum.reason_code(), None);
        assert_eq!(Discovered::RoundedOnce.reason_code(), None);
        // One three-line total that misses by a unit: one issuer error or one rounding, so nothing is found.
        let one = discover_construction(&[Footing { k: 3, residual: 1 }]).expect("report");
        assert_eq!((one.foot_exact, one.rounded_once_accepts), (0, 1));
        assert!((one.ln_component_sum_error_tail - det::ln(0.1)).abs() < 1e-12);
        assert_eq!(one.discovered, Discovered::Ambiguous, "{one:?}");
        // Exact footings far rarer than rounding once predicts: neither fits.
        let rare: Vec<Footing> = (0..200)
            .map(|i| Footing {
                k: 2,
                residual: i128::from(i % 50 != 0),
            })
            .collect();
        let r = discover_construction(&rare).expect("report");
        assert_eq!(r.foot_exact, 4);
        assert_eq!(r.discovered, Discovered::Ambiguous, "{r:?}");
        // Nothing scored: past MAX_K a footing is indeterminate.
        let r = discover_construction(&[Footing { k: 21, residual: 0 }]).expect("report");
        assert_eq!(
            (r.n, r.indeterminate, r.discovered),
            (0, 1, Discovered::Ambiguous)
        );
    }
}
