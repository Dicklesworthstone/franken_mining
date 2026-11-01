//! Provenance: frankensim crates/fs-ivl/src/expansion.rs @ 20463525d9775a18c1b237d2676f364f367175ab; licence LicenseRef-MIT-OpenAI-Anthropic-Rider; Changes: every assert! on a non-finite operand or component becomes a FMINE-REF-207 refusal returned through Result, the should_panic tests become refusal tests, and the module is crate-private; Left out: nothing
//! Determinism: BitwisePortable (pure `+ − ×` and `mul_add`).
//!
//! Floating-point expansion arithmetic (Shewchuk 1997, plan §6.4): EXACT
//! operations on numbers represented as sums of nonoverlapping f64
//! components, built on the error-free transformations in [`super::eft`].
//!
//! Representation: an expansion is a slice of NONZERO components in
//! increasing magnitude whose exact real sum is the value; the empty slice
//! is exactly zero (all constructors here zero-eliminate). For expansions
//! produced by these operations the components are nonoverlapping, so the
//! SIGN of the value is the sign of the largest (last) component — the
//! property the exact predicates stand on.
//!
//! For valid finite inputs whose exact result remains representable as a
//! finite expansion, every operation is error-free: the output's exact sum
//! equals the exact real-arithmetic result. That is bitwise-testable algebra,
//! not approximation. Arithmetic that leaves this domain is refused
//! (`FMINE-REF-207`) instead of emitting a non-finite pseudo-expansion.
//! [`two_diff`] and [`diff_expansion`] reject non-finite operands and finite
//! operands whose mathematical difference cannot be represented by a valid
//! finite expansion. This prevents a NaN-bearing pseudo-expansion from being
//! mistaken for exact zero by downstream certificate code. As a second
//! fail-closed boundary, [`expansion_sign`] rejects any non-finite component.

use fmine_evidence::{Refusal, RefusalCode};

use super::eft::{quick_two_sum, two_prod, two_sum};

/// The refusal for arithmetic that left the finite expansion domain.
fn left_domain(what: &str) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        format!("exact expansion arithmetic: {what}"),
    )
}

/// Exact difference: `(d, tail)` with `d = fl(a−b)` and `d + tail = a - b`
/// exactly (Shewchuk's Two-Diff, via [`two_sum`] on the negation).
///
/// Refuses (`FMINE-REF-207`) when either operand is non-finite or when the
/// rounded difference overflows. Neither case has a valid finite
/// floating-point expansion, and allowing the resulting NaNs through would
/// make [`expansion_sign`] fail open.
pub(crate) fn two_diff(a: f64, b: f64) -> Result<(f64, f64), Refusal> {
    if !(a.is_finite() && b.is_finite()) {
        return Err(left_domain(&format!(
            "exact difference requires finite operands: a={a}, b={b}"
        )));
    }
    let (d, tail) = two_sum(a, -b);
    if !(d.is_finite() && tail.is_finite()) {
        return Err(left_domain(&format!(
            "exact difference overflowed the finite expansion domain: a={a}, b={b}"
        )));
    }
    Ok((d, tail))
}

/// Push a component, eliminating zeros (empty output = exact zero).
fn push_nonzero(h: &mut Vec<f64>, x: f64) -> Result<(), Refusal> {
    if !x.is_finite() {
        return Err(left_domain(
            "expansion arithmetic left the finite representable domain",
        ));
    }
    if x != 0.0 {
        h.push(x);
    }
    Ok(())
}

/// Exact sum of two expansions (Shewchuk's FAST-EXPANSION-SUM with zero
/// elimination). Inputs must be valid expansions (nonoverlapping,
/// increasing magnitude); the output is one too.
pub(crate) fn fast_expansion_sum_zeroelim(e: &[f64], f: &[f64]) -> Result<Vec<f64>, Refusal> {
    if e.is_empty() {
        return Ok(f.to_vec());
    }
    if f.is_empty() {
        return Ok(e.to_vec());
    }
    // Merge by increasing magnitude. `(fnow > enow) == (fnow > -enow)`
    // is Shewchuk's branchless |fnow| > |enow| test.
    let mut h = Vec::with_capacity(e.len() + f.len());
    let (mut ei, mut fi) = (0usize, 0usize);
    let smaller_is_e = |ei: usize, fi: usize| -> bool {
        // Shewchuk's branchless |f| > |e| test: true when e[ei] has the
        // smaller magnitude (take it first).
        (f[fi] > e[ei]) == (f[fi] > -e[ei])
    };
    let mut q = if smaller_is_e(ei, fi) {
        ei += 1;
        e[ei - 1]
    } else {
        fi += 1;
        f[fi - 1]
    };
    let mut first = true;
    while ei < e.len() && fi < f.len() {
        let now = if smaller_is_e(ei, fi) {
            ei += 1;
            e[ei - 1]
        } else {
            fi += 1;
            f[fi - 1]
        };
        let (qnew, hh) = if first {
            // The second-smallest component dominates the smallest.
            quick_two_sum(now, q)
        } else {
            two_sum(q, now)
        };
        first = false;
        q = qnew;
        push_nonzero(&mut h, hh)?;
    }
    for &now in &e[ei..] {
        let (qnew, hh) = two_sum(q, now);
        q = qnew;
        push_nonzero(&mut h, hh)?;
    }
    for &now in &f[fi..] {
        let (qnew, hh) = two_sum(q, now);
        q = qnew;
        push_nonzero(&mut h, hh)?;
    }
    push_nonzero(&mut h, q)?;
    Ok(h)
}

/// Exact product of an expansion by one f64 (Shewchuk's SCALE-EXPANSION
/// with zero elimination). FMA-backed: no Dekker splitting.
pub(crate) fn scale_expansion_zeroelim(e: &[f64], b: f64) -> Result<Vec<f64>, Refusal> {
    let Some((&e0, rest)) = e.split_first() else {
        return Ok(Vec::new());
    };
    let mut h = Vec::with_capacity(2 * e.len());
    let (mut q, hh) = two_prod(e0, b);
    push_nonzero(&mut h, hh)?;
    for &enow in rest {
        let (product1, product0) = two_prod(enow, b);
        let (sum, hh) = two_sum(q, product0);
        push_nonzero(&mut h, hh)?;
        // |product1| >= |sum| holds by Shewchuk's proof (Theorem 19).
        let (qnew, hh) = quick_two_sum(product1, sum);
        q = qnew;
        push_nonzero(&mut h, hh)?;
    }
    push_nonzero(&mut h, q)?;
    Ok(h)
}

/// Exact product of two expansions: distribute [`scale_expansion_zeroelim`]
/// over one operand's components and distill with pairwise exact sums.
/// Sizes grow as O(|a|·|b|) — this is the cold exact tail of the predicate
/// ladder, never the hot path.
pub(crate) fn expansion_product(a: &[f64], b: &[f64]) -> Result<Vec<f64>, Refusal> {
    match a.len() {
        0 => Ok(Vec::new()),
        1 => scale_expansion_zeroelim(b, a[0]),
        _ => {
            let mid = a.len() / 2;
            let lo = expansion_product(&a[..mid], b)?;
            let hi = expansion_product(&a[mid..], b)?;
            fast_expansion_sum_zeroelim(&lo, &hi)
        }
    }
}

/// Exact difference of two expansions (`a − b`).
pub(crate) fn expansion_diff(a: &[f64], b: &[f64]) -> Result<Vec<f64>, Refusal> {
    let neg: Vec<f64> = b.iter().map(|&x| -x).collect();
    fast_expansion_sum_zeroelim(a, &neg)
}

/// The sign of an expansion's exact value: the sign of its largest
/// component (valid because components are nonoverlapping), 0 when empty.
///
/// Refuses (`FMINE-REF-207`) when any component is non-finite. Such a slice
/// is not a valid expansion; in particular, NaN must not be interpreted as
/// exact zero.
pub(crate) fn expansion_sign(e: &[f64]) -> Result<i32, Refusal> {
    if !e.iter().all(|x| x.is_finite()) {
        return Err(left_domain(&format!(
            "expansion sign requires finite components: {e:?}"
        )));
    }
    Ok(match e.last() {
        Some(&x) if x > 0.0 => 1,
        Some(&x) if x < 0.0 => -1,
        _ => 0,
    })
}

/// Approximate value (plain left-to-right sum; adequate for error-bound
/// comparisons in the adaptive stages).
#[must_use]
pub(crate) fn estimate(e: &[f64]) -> f64 {
    e.iter().sum()
}

/// The exact 2×2 determinant `a·b − c·d` as an expansion of ≤ 4 components
/// (Shewchuk's Two-Product difference, the workhorse minor).
pub(crate) fn prod_diff(a: f64, b: f64, c: f64, d: f64) -> Result<Vec<f64>, Refusal> {
    let (p1, p0) = two_prod(a, b);
    let (q1, q0) = two_prod(c, d);
    fast_expansion_sum_zeroelim(&nonzero2(p0, p1)?, &nonzero2(-q0, -q1)?)
}

/// Two-component expansion `(lo, hi)` with zeros eliminated.
fn nonzero2(lo: f64, hi: f64) -> Result<Vec<f64>, Refusal> {
    let mut v = Vec::with_capacity(2);
    push_nonzero(&mut v, lo)?;
    push_nonzero(&mut v, hi)?;
    Ok(v)
}

/// An exact coordinate difference as a ≤ 2-component expansion.
pub(crate) fn diff_expansion(a: f64, b: f64) -> Result<Vec<f64>, Refusal> {
    let (d, tail) = two_diff(a, b)?;
    nonzero2(tail, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The batteries feed finite, in-range operands; a refusal there is a test failure.
    fn diff_expansion(a: f64, b: f64) -> Vec<f64> {
        super::diff_expansion(a, b).expect("in-range difference")
    }
    fn fast_expansion_sum_zeroelim(e: &[f64], f: &[f64]) -> Vec<f64> {
        super::fast_expansion_sum_zeroelim(e, f).expect("in-range sum")
    }
    fn scale_expansion_zeroelim(e: &[f64], b: f64) -> Vec<f64> {
        super::scale_expansion_zeroelim(e, b).expect("in-range scale")
    }
    fn expansion_product(a: &[f64], b: &[f64]) -> Vec<f64> {
        super::expansion_product(a, b).expect("in-range product")
    }
    fn expansion_diff(a: &[f64], b: &[f64]) -> Vec<f64> {
        super::expansion_diff(a, b).expect("in-range difference")
    }
    fn expansion_sign(e: &[f64]) -> i32 {
        super::expansion_sign(e).expect("finite components")
    }
    fn prod_diff(a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
        super::prod_diff(a, b, c, d).expect("in-range minor")
    }

    fn lcg(seed: &mut u64) -> f64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((*seed >> 11) as f64) / (1u64 << 53) as f64 - 0.5
    }

    /// The exactness law in its native form: an EXACT residual. If op
    /// results are error-free, `result − inputs` distills to the empty
    /// expansion — no tolerance, no oracle precision ceiling.
    fn assert_exactly_zero(e: &[f64], law: &str) {
        assert_eq!(expansion_sign(e), 0, "{law}: nonzero residual {e:?}");
        assert!(
            e.is_empty(),
            "{law}: zero-sign residual with components {e:?}"
        );
    }

    fn assert_refused_207(result: Result<impl core::fmt::Debug, Refusal>, wanted: &str) {
        let refusal = result.expect_err("must refuse");
        assert_eq!(refusal.code, "FMINE-REF-207", "{refusal:?}");
        assert!(refusal.what.contains(wanted), "{refusal:?}");
    }

    #[test]
    fn sum_scale_product_satisfy_exact_residual_laws() {
        let mut seed = 0x5EED_E4BA_0000_0001u64;
        for round in 0..500 {
            // Wildly mixed magnitudes force nontrivial tails everywhere.
            let a = lcg(&mut seed);
            let b = lcg(&mut seed) * 1e-17;
            let c = lcg(&mut seed);
            let d = lcg(&mut seed) * 1e17;
            let s = lcg(&mut seed) * 1e5;
            let ea = diff_expansion(a, b);
            let eb = diff_expansion(c, d);
            // Sum law: (ea + eb) − ea − eb == 0 exactly.
            let sum = fast_expansion_sum_zeroelim(&ea, &eb);
            let residual = expansion_diff(&expansion_diff(&sum, &ea), &eb);
            assert_exactly_zero(&residual, &format!("sum law, round {round}"));
            // Scale ≡ product-by-singleton, exactly.
            let scaled = scale_expansion_zeroelim(&ea, s);
            let prod = expansion_product(&ea, &[s]);
            assert_exactly_zero(
                &expansion_diff(&scaled, &prod),
                &format!("scale/product law, round {round}"),
            );
            // Distributivity: (ea + eb)·ec == ea·ec + eb·ec, exactly.
            let ec = diff_expansion(s, a);
            let lhs = expansion_product(&sum, &ec);
            let rhs = fast_expansion_sum_zeroelim(
                &expansion_product(&ea, &ec),
                &expansion_product(&eb, &ec),
            );
            assert_exactly_zero(
                &expansion_diff(&lhs, &rhs),
                &format!("distributivity, round {round}"),
            );
        }
    }

    #[test]
    fn prod_diff_catches_catastrophic_cancellation() {
        let (a, b) = (1.0 + f64::EPSILON, 1.0 - f64::EPSILON);
        // ab − ba is exactly zero.
        assert_eq!(expansion_sign(&prod_diff(a, b, b, a)), 0);
        // Exact a·b = 1 − ε², but fl(a·b) = 1.0: the difference is the
        // 2⁻¹⁰⁴ the naive float determinant silently throws away.
        let fl_ab = a * b;
        assert_eq!(
            fl_ab.to_bits(),
            1.0f64.to_bits(),
            "premise: float product rounds to 1"
        );
        assert_eq!(expansion_sign(&prod_diff(a, b, 1.0, fl_ab)), -1);
        assert_eq!(expansion_sign(&prod_diff(1.0, fl_ab, a, b)), 1);
    }

    #[test]
    fn empty_expansion_is_exact_zero() {
        assert_eq!(expansion_sign(&[]), 0);
        assert!(fast_expansion_sum_zeroelim(&[], &[]).is_empty());
        assert!(scale_expansion_zeroelim(&[], 7.0).is_empty());
        assert!(expansion_product(&[], &[1.0]).is_empty());
        let x = diff_expansion(3.5, 3.5);
        assert!(x.is_empty(), "a − a is exactly zero: {x:?}");
    }

    #[test]
    fn coordinate_difference_rejects_non_finite_operands() {
        assert_refused_207(
            super::diff_expansion(f64::INFINITY, f64::INFINITY),
            "exact difference requires finite operands",
        );
        assert_refused_207(
            super::two_diff(1.0, f64::NAN),
            "exact difference requires finite operands",
        );
    }

    #[test]
    fn coordinate_difference_rejects_overflowing_finite_operands() {
        assert_refused_207(
            super::diff_expansion(f64::MAX, -f64::MAX),
            "exact difference overflowed the finite expansion domain",
        );
    }

    #[test]
    fn sign_rejects_nan_instead_of_misclassifying_it_as_zero() {
        assert_refused_207(
            super::expansion_sign(&[f64::NAN]),
            "expansion sign requires finite components",
        );
    }

    #[test]
    fn scaled_expansion_rejects_overflow_instead_of_emitting_nan() {
        assert_refused_207(
            super::scale_expansion_zeroelim(&[f64::MAX], 2.0),
            "expansion arithmetic left the finite representable domain",
        );
        assert_refused_207(
            super::expansion_product(&[f64::MAX], &[f64::MAX]),
            "expansion arithmetic left the finite representable domain",
        );
    }
}
