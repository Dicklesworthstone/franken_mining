//! Determinism: BitwisePortable
//! Salted internal-review catch-rate estimator (exact Clopper–Pearson via transcendental-free
//! binomial-tail bisection) and BLAKE3 commitment/verification (`BD-357.S1` / `bd-kftl.1`,
//! plan WP-00).
//!
//! Safety & anti-leak boundary (`BD-357`):
//! - `SeedClass` deliberately has NO basis-switch variant: an issuer-attributed false figure is
//!   never placed before an issuer reader.
//! - `generate_os_nonce` reads 32 bytes from `/dev/urandom` (never Philox or any committed repo
//!   seed) so candidate seed placements cannot be brute-forced from repository state.
//! - `clopper_pearson` uses strictly `+`, `-`, `*`, `/` and comparisons (ZERO std transcendentals,
//!   so `xtask check-det` certifies `BitwisePortable`).

use std::collections::BTreeSet;
use std::io::Read;

use fmine_core::Digest32;
use fmine_core::hash::blake3_hash;
use fmine_evidence::{Refusal, RefusalCode};

/// Maximum `n` for which every binomial coefficient `C(n, k)` fits exactly in IEEE-754 `f64`
/// (`C(50, 25) = 126_410_606_437_752 < 2^53 = 9_007_199_254_740_992`).
pub const MAX_EXACT_CLOPPER_PEARSON_N: u32 = 50;

/// Fixed bisection iteration count (`2^-100 ≈ 7.9e-31`, well below `f64` precision).
const BISECTION_ITERATIONS: usize = 100;

/// Canonical domain-separation prefix for `ReviewKey` commitments (`BD-357.S1`).
const REVIEW_KEY_DOMAIN_PREFIX: &[u8] = b"fmine:review-key:v1\0";

/// Permitted internal-review seed classes (`BD-357.S1` / `BD-357.S2`).
///
/// Intentionally omits any basis-switch class so no issuer-attributed false number can ever be
/// synthesized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SeedClass {
    S1CloseCitesNonExercisingTest = 1,
    S2CloseCitesNonPassRecord = 2,
    S3MisCitedCell = 3,
    S4RefusedShownReproduced = 4,
    S5StaleShownCurrent = 5,
    S6WrongCauseClass = 6,
}

impl SeedClass {
    /// Stable single-byte canonical encoding for commitment hashing.
    #[must_use]
    pub const fn tag_byte(self) -> u8 {
        self as u8
    }
}

/// Exact integer power by repeated squaring using only `*` (no `.powi` / `.powf` / `.ln` / `.exp`).
fn pow_u32(mut base: f64, mut exp: u32) -> f64 {
    let mut acc = 1.0;
    while exp > 0 {
        if (exp & 1) == 1 {
            acc *= base;
        }
        exp >>= 1;
        if exp > 0 {
            base *= base;
        }
    }
    acc
}

/// Exact binomial coefficient `C(n, k)` as `f64` for `0 <= k <= n <= MAX_EXACT_CLOPPER_PEARSON_N`.
fn binom_coeff_exact_f64(n: u32, k: u32) -> f64 {
    let k = k.min(n - k);
    let mut acc: u128 = 1;
    let n128 = u128::from(n);
    for i in 0..u128::from(k) {
        acc = acc * (n128 - i) / (i + 1);
    }
    acc as f64
}

/// Binomial probability mass `P(X = k | n, p)` using only `+ - * /`.
fn binom_pmf(k: u32, n: u32, p: f64) -> f64 {
    binom_coeff_exact_f64(n, k) * pow_u32(p, k) * pow_u32(1.0 - p, n - k)
}

/// Upper binomial tail `P(X >= x | n, p)` (strictly increasing in `p` on `(0, 1)` for `1 <= x <= n`).
fn binom_upper_tail(x: u32, n: u32, p: f64) -> f64 {
    let mut sum = 0.0;
    for k in x..=n {
        sum += binom_pmf(k, n, p);
    }
    sum
}

/// Lower binomial tail `P(X <= x | n, p)` (strictly decreasing in `p` on `(0, 1)` for `0 <= x < n`).
fn binom_lower_tail(x: u32, n: u32, p: f64) -> f64 {
    let mut sum = 0.0;
    for k in 0..=x {
        sum += binom_pmf(k, n, p);
    }
    sum
}

/// Exact two-sided `(1 - alpha)` Clopper–Pearson confidence interval for `x` catches out of `n`
/// trials (`BD-357.S1`).
///
/// Computes `[B(alpha/2; x, n - x + 1), B(1 - alpha/2; x + 1, n - x)]` by exact binomial-tail
/// inversion using 100 iterations of bisection on `[0, 1]` with strictly `+ - * /` and comparisons.
///
/// Edge conventions:
/// - `x == 0` yields an exact `0.0` lower bound;
/// - `x == n` yields an exact `1.0` upper bound.
///
/// Refuses (never clamps) with `FMINE-REF-109` when `n == 0`, `x > n`, `n > MAX_EXACT_CLOPPER_PEARSON_N`,
/// or `alpha` is non-finite or not strictly in `(0, 1)`.
pub fn clopper_pearson(x: u32, n: u32, alpha: f64) -> Result<(f64, f64), Refusal> {
    if n == 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "clopper_pearson: trial count n must be >= 1, got 0",
        ));
    }
    if x > n {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("clopper_pearson: catches x={x} exceeds trial count n={n}"),
        ));
    }
    if n > MAX_EXACT_CLOPPER_PEARSON_N {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "clopper_pearson: n={n} exceeds exact f64 binomial coefficient bound \
                 MAX_EXACT_CLOPPER_PEARSON_N={MAX_EXACT_CLOPPER_PEARSON_N}"
            ),
        ));
    }
    if !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("clopper_pearson: alpha must be finite and strictly in (0, 1), got {alpha}"),
        ));
    }

    let half_alpha = 0.5 * alpha;
    if half_alpha <= 0.0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("clopper_pearson: alpha/2 underflowed to 0 for alpha={alpha}"),
        ));
    }

    let lower = if x == 0 {
        0.0
    } else {
        let mut lo = 0.0_f64;
        let mut hi = 1.0_f64;
        for _ in 0..BISECTION_ITERATIONS {
            let mid = 0.5 * (lo + hi);
            if binom_upper_tail(x, n, mid) < half_alpha {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    };

    let upper = if x == n {
        1.0
    } else {
        let mut lo = 0.0_f64;
        let mut hi = 1.0_f64;
        for _ in 0..BISECTION_ITERATIONS {
            let mid = 0.5 * (lo + hi);
            if binom_lower_tail(x, n, mid) > half_alpha {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    };

    Ok((lower, upper))
}

/// Sealed answer key for a salted internal review round (`BD-357.S1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewKey {
    pub round_id: String,
    pub nonce: [u8; 32],
    pub items: Vec<(String, SeedClass)>,
}

impl ReviewKey {
    /// Validate invariants before serialization or commitment:
    /// - `round_id` non-empty;
    /// - `nonce` not all-zero;
    /// - every `item_id` non-empty and unique.
    pub fn validate(&self) -> Result<(), Refusal> {
        if self.round_id.trim().is_empty() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "ReviewKey: round_id must be non-empty",
            ));
        }
        if self.nonce == [0u8; 32] {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "ReviewKey: all-zero nonce is refused; draw 32 bytes from OS entropy",
            ));
        }
        let mut seen = BTreeSet::new();
        for (item_id, _) in &self.items {
            if item_id.trim().is_empty() {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    "ReviewKey: item_id must be non-empty",
                ));
            }
            if !seen.insert(item_id.as_str()) {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("ReviewKey: duplicate item_id '{item_id}'"),
                ));
            }
        }
        Ok(())
    }

    /// Encode canonical bytes independent of `self.items` input ordering (`BD-357.S1`):
    /// `"fmine:review-key:v1\0" || u32_le(round_id.len()) || round_id || nonce(32) || u32_le(items.len()) || sorted(u32_le(id.len()) || id || class_byte)`.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, Refusal> {
        self.validate()?;

        let round_len = u32::try_from(self.round_id.len()).map_err(|_| {
            Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "ReviewKey: round_id length exceeds u32::MAX",
            )
        })?;
        let item_count = u32::try_from(self.items.len()).map_err(|_| {
            Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "ReviewKey: item count exceeds u32::MAX",
            )
        })?;

        let mut sorted_items: Vec<(&str, SeedClass)> = self
            .items
            .iter()
            .map(|(id, cls)| (id.as_str(), *cls))
            .collect();
        sorted_items.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

        let mut out = Vec::new();
        out.extend_from_slice(REVIEW_KEY_DOMAIN_PREFIX);
        out.extend_from_slice(&round_len.to_le_bytes());
        out.extend_from_slice(self.round_id.as_bytes());
        out.extend_from_slice(&self.nonce);
        out.extend_from_slice(&item_count.to_le_bytes());
        for (item_id, cls) in sorted_items {
            let id_len = u32::try_from(item_id.len()).map_err(|_| {
                Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    "ReviewKey: item_id length exceeds u32::MAX",
                )
            })?;
            out.extend_from_slice(&id_len.to_le_bytes());
            out.extend_from_slice(item_id.as_bytes());
            out.push(cls.tag_byte());
        }
        Ok(out)
    }
}

/// Compute the BLAKE3 commitment digest for a validated [`ReviewKey`] (`BD-357.S1`).
pub fn commit(key: &ReviewKey) -> Result<Digest32, Refusal> {
    let bytes = key.canonical_bytes()?;
    Ok(blake3_hash(&bytes))
}

/// Verify a [`ReviewKey`] against a previously published BLAKE3 commitment digest (`BD-357.S1`).
///
/// Refuses with `FMINE-REF-505` (`DigestMismatch505`) if the digest differs, or `FMINE-REF-109` if
/// the key itself is malformed (e.g. all-zero nonce).
pub fn verify(key: &ReviewKey, commitment: &Digest32) -> Result<(), Refusal> {
    let computed = commit(key)?;
    let diff = computed
        .as_bytes()
        .iter()
        .zip(commitment.as_bytes().iter())
        .fold(0u8, |acc, (&a, &b)| acc | (a ^ b));
    if diff == 0 {
        Ok(())
    } else {
        Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "ReviewKey commitment mismatch for round '{}': expected {}, computed {}",
                key.round_id,
                commitment.to_hex(),
                computed.to_hex()
            ),
        ))
    }
}

/// Draw a fresh 32-byte commitment nonce from OS entropy (`/dev/urandom`).
///
/// Never uses `fmine_core::rand::PhiloxStream` or any committed repository seed (`BD-357.S1`).
pub fn generate_os_nonce() -> Result<[u8; 32], Refusal> {
    let mut file = std::fs::File::open("/dev/urandom").map_err(|e| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("unable to open /dev/urandom for review commitment nonce: {e}"),
        )
    })?;
    let mut nonce = [0u8; 32];
    file.read_exact(&mut nonce).map_err(|e| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("unable to read 32 bytes from /dev/urandom: {e}"),
        )
    })?;
    if nonce == [0u8; 32] {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "OS entropy source returned an all-zero nonce; refusing to commit",
        ));
    }
    Ok(nonce)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::rand::{PhiloxStream, RngKey};

    fn rng_key_hex(k: RngKey) -> String {
        let mut buf = [0u8; 32];
        buf[0..8].copy_from_slice(&k.seed_root.to_le_bytes());
        buf[8..12].copy_from_slice(&k.partition.to_le_bytes());
        buf[12..16].copy_from_slice(&k.domain.to_le_bytes());
        buf[16..20].copy_from_slice(&k.var_group.to_le_bytes());
        buf[20..24].copy_from_slice(&k.realization.to_le_bytes());
        buf[24..32].copy_from_slice(&k.brick.to_le_bytes());
        let mut s = String::with_capacity(64);
        for b in buf {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }

    #[test]
    fn test_clopper_pearson_known_answers() {
        // Known answers from BD-357.S1 at alpha = 0.05 (exact to 1e-4).
        let cases: &[(u32, u32, f64, f64)] = &[
            (0, 5, 0.0, 0.5218),
            (2, 5, 0.0527, 0.8534),
            (3, 5, 0.1466, 0.9473),
            (5, 5, 0.4782, 1.0),
            (0, 10, 0.0, 0.3085),
            (8, 10, 0.4439, 0.9748),
            (10, 10, 0.6915, 1.0),
        ];
        for &(x, n, exp_lo, exp_hi) in cases {
            let (lo, hi) = clopper_pearson(x, n, 0.05).unwrap();
            assert!(
                (lo - exp_lo).abs() <= 1e-4,
                "x={x}, n={n}: lower {lo} != expected {exp_lo}"
            );
            assert!(
                (hi - exp_hi).abs() <= 1e-4,
                "x={x}, n={n}: upper {hi} != expected {exp_hi}"
            );
            assert!(lo <= hi, "interval inverted for {x}/{n}");

            // Exact symmetry: lo(x, n) == 1 - hi(n - x, n).
            let (sym_lo, sym_hi) = clopper_pearson(n - x, n, 0.05).unwrap();
            assert!((lo - (1.0 - sym_hi)).abs() < 1e-12);
            assert!((hi - (1.0 - sym_lo)).abs() < 1e-12);
        }
    }

    #[test]
    fn test_clopper_pearson_edges_exact() {
        for n in [1_u32, 5, 10, 40, MAX_EXACT_CLOPPER_PEARSON_N] {
            let (lo0, hi0) = clopper_pearson(0, n, 0.05).unwrap();
            assert_eq!(lo0.to_bits(), 0.0_f64.to_bits(), "x=0 must yield exact 0.0");
            assert!(hi0 > 0.0 && hi0 < 1.0);

            let (lon, hin) = clopper_pearson(n, n, 0.05).unwrap();
            assert_eq!(hin.to_bits(), 1.0_f64.to_bits(), "x=n must yield exact 1.0");
            assert!(lon > 0.0 && lon < 1.0);
        }
    }

    #[test]
    fn test_clopper_pearson_refuses_bad_inputs() {
        let bad_cases: &[(u32, u32, f64)] = &[
            (0, 0, 0.05),
            (6, 5, 0.05),
            (0, MAX_EXACT_CLOPPER_PEARSON_N + 1, 0.05),
            (2, 5, 0.0),
            (2, 5, 1.0),
            (2, 5, -0.05),
            (2, 5, 1.05),
            (2, 5, f64::NAN),
            (2, 5, f64::INFINITY),
            (2, 5, f64::NEG_INFINITY),
        ];
        for &(x, n, alpha) in bad_cases {
            let err = clopper_pearson(x, n, alpha).unwrap_err();
            assert_eq!(
                err.code,
                RefusalCode::OutsidePhysicalDomain109.spec().code,
                "bad input ({x}, {n}, {alpha}) must refuse with FMINE-REF-109"
            );
        }
    }

    #[test]
    fn test_clopper_pearson_exact_coverage_ge_95() {
        let mut min_cov = 1.0_f64;
        let mut cov_10_05 = 0.0_f64;
        for n in [5_u32, 10] {
            let intervals: Vec<(f64, f64)> = (0..=n)
                .map(|x| clopper_pearson(x, n, 0.05).unwrap())
                .collect();
            for p in [0.2_f64, 0.5, 0.9] {
                let mut coverage = 0.0_f64;
                for (x, &(lo, hi)) in intervals.iter().enumerate() {
                    if lo <= p && p <= hi {
                        coverage += binom_pmf(x as u32, n, p);
                    }
                }
                assert!(
                    coverage >= 0.95,
                    "exact Clopper-Pearson coverage {coverage} < 0.95 for n={n}, p={p}"
                );
                if coverage < min_cov {
                    min_cov = coverage;
                }
                if n == 10 && (p - 0.5).abs() < 1e-12 {
                    cov_10_05 = coverage;
                }
            }
        }
        // BD-357.S1 acceptance criterion: computed minimum is 0.9785 at n=10, p=0.5.
        assert!(
            (cov_10_05 - 0.9785).abs() <= 1e-4,
            "expected exact coverage ~0.9785 at n=10, p=0.5, got {cov_10_05}"
        );
        assert!((min_cov - cov_10_05).abs() < 1e-12);
    }

    #[test]
    fn test_clopper_pearson_simulated_coverage_10k() {
        const ROUNDS: u32 = 10_000;
        let mut min_coverage = 1.0_f64;
        let mut logs = Vec::new();

        for (n_idx, &n) in [5_u32, 10].iter().enumerate() {
            let intervals: Vec<(f64, f64)> = (0..=n)
                .map(|x| clopper_pearson(x, n, 0.05).unwrap())
                .collect();
            for (p_idx, &p) in [0.2_f64, 0.5, 0.9].iter().enumerate() {
                let key = RngKey {
                    seed_root: 0xBD35_7510_0000_0001,
                    partition: 357,
                    domain: n,
                    var_group: (p_idx as u32) + 1,
                    realization: n_idx as u32,
                    brick: 0,
                };
                let mut stream = PhiloxStream::new(key);
                let mut covered = 0_u32;
                for _ in 0..ROUNDS {
                    let mut x = 0_u32;
                    for _ in 0..n {
                        if stream.next_f64_open01() < p {
                            x += 1;
                        }
                    }
                    let (lo, hi) = intervals[x as usize];
                    if lo <= p && p <= hi {
                        covered += 1;
                    }
                }
                let coverage = f64::from(covered) / f64::from(ROUNDS);
                let line = format!(
                    "{{\"p\":{p},\"n\":{n},\"rounds\":{ROUNDS},\"covered\":{covered},\
                     \"coverage\":{coverage:.6},\"key\":\"{}\"}}",
                    rng_key_hex(key)
                );
                logs.push(line);
                assert!(
                    coverage >= 0.95,
                    "simulated coverage {coverage} < 0.95 for n={n}, p={p}"
                );
                if coverage < min_coverage {
                    min_coverage = coverage;
                }
            }
        }
        let summary = format!("{{\"min_coverage\":{min_coverage:.6},\"threshold\":0.95}}");
        assert_eq!(logs.len(), 6);
        assert!(summary.contains("\"threshold\":0.95"));
        assert!(min_coverage >= 0.95);
    }

    fn sample_key() -> ReviewKey {
        ReviewKey {
            round_id: "round-1-bead-closes".to_string(),
            nonce: [
                0x19, 0xa2, 0x3b, 0x4c, 0x5d, 0x6e, 0x7f, 0x80, 0x91, 0xa2, 0xb3, 0xc4, 0xd5, 0xe6,
                0xf7, 0x08, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                0xdd, 0xee, 0xff, 0x42,
            ],
            items: vec![
                (
                    "item-007".to_string(),
                    SeedClass::S1CloseCitesNonExercisingTest,
                ),
                ("item-019".to_string(), SeedClass::S3MisCitedCell),
                ("item-033".to_string(), SeedClass::S5StaleShownCurrent),
            ],
        }
    }

    #[test]
    fn test_review_key_commit_verify_roundtrip() {
        let key = sample_key();
        let digest = commit(&key).unwrap();
        verify(&key, &digest).unwrap();

        // Also exercise real OS entropy nonce generation.
        let os_nonce = generate_os_nonce().unwrap();
        assert_ne!(os_nonce, [0u8; 32]);
        let mut os_key = key;
        os_key.nonce = os_nonce;
        let os_digest = commit(&os_key).unwrap();
        verify(&os_key, &os_digest).unwrap();
    }

    #[test]
    fn test_review_key_tamper_fails() {
        let key = sample_key();
        let digest = commit(&key).unwrap();

        // 1. Tamper seed class.
        let mut bad_class = key.clone();
        bad_class.items[0].1 = SeedClass::S2CloseCitesNonPassRecord;
        assert_eq!(
            verify(&bad_class, &digest).unwrap_err().code,
            RefusalCode::DigestMismatch505.spec().code
        );

        // 2. Tamper item id.
        let mut bad_id = key.clone();
        bad_id.items[1].0 = "item-020".to_string();
        assert_eq!(
            verify(&bad_id, &digest).unwrap_err().code,
            RefusalCode::DigestMismatch505.spec().code
        );

        // 3. Flip a single bit in nonce.
        let mut bad_nonce = key.clone();
        bad_nonce.nonce[31] ^= 0x01;
        assert_eq!(
            verify(&bad_nonce, &digest).unwrap_err().code,
            RefusalCode::DigestMismatch505.spec().code
        );

        // 4. Tamper round_id.
        let mut bad_round = key;
        bad_round.round_id = "round-2-m0-packet".to_string();
        assert_eq!(
            verify(&bad_round, &digest).unwrap_err().code,
            RefusalCode::DigestMismatch505.spec().code
        );
    }

    #[test]
    fn test_review_key_zero_nonce_refused() {
        let mut key = sample_key();
        key.nonce = [0u8; 32];
        assert_eq!(
            commit(&key).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );

        // Empty round_id and duplicate item_id also refused.
        let mut empty_round = sample_key();
        empty_round.round_id = " ".to_string();
        assert_eq!(
            commit(&empty_round).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );

        let mut dup_item = sample_key();
        dup_item
            .items
            .push(("item-007".to_string(), SeedClass::S6WrongCauseClass));
        assert_eq!(
            commit(&dup_item).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );
    }

    #[test]
    fn test_canonical_bytes_independent_of_item_order() {
        let k1 = sample_key();
        let mut k2 = sample_key();
        k2.items.reverse();
        let mut k3 = sample_key();
        k3.items.rotate_left(1);

        let b1 = k1.canonical_bytes().unwrap();
        let b2 = k2.canonical_bytes().unwrap();
        let b3 = k3.canonical_bytes().unwrap();
        assert_eq!(b1, b2);
        assert_eq!(b2, b3);
        assert_eq!(commit(&k1).unwrap(), commit(&k2).unwrap());
        assert_eq!(commit(&k2).unwrap(), commit(&k3).unwrap());
    }
}
