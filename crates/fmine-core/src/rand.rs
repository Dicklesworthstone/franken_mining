//! Provenance: frankensim crates/fs-rand/src/philox.rs @ 20463525d9775a18c1b237d2676f364f367175ab; licence LicenseRef-MIT-OpenAI-Anthropic-Rider; Changes: added FrankenMining 6-tuple RngKey (seed_root, partition, domain, var_group, r, brick) per ADR-RNG-01 and Sobol/Owen 2D scrambling; Left out: philox4x32_10_batch
//! Determinism: BitwisePortable

use crate::det;
use crate::hash::blake3_hash;

const M0: u32 = 0xD251_1F53;
const M1: u32 = 0xCD9E_8D57;
const W0: u32 = 0x9E37_79B9;
const W1: u32 = 0xBB67_AE85;

#[inline]
fn mulhilo(a: u32, b: u32) -> (u32, u32) {
    let wide = u64::from(a) * u64::from(b);
    ((wide >> 32) as u32, wide as u32)
}

#[inline]
fn round(ctr: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    let (hi0, lo0) = mulhilo(M0, ctr[0]);
    let (hi1, lo1) = mulhilo(M1, ctr[2]);
    [hi1 ^ ctr[1] ^ key[0], lo1, hi0 ^ ctr[3] ^ key[1], lo0]
}

/// 10-round `Philox4x32` block function (`Random123` standard, `BD-020`).
#[must_use]
pub fn philox4x32_10(mut ctr: [u32; 4], mut key: [u32; 2]) -> [u32; 4] {
    for i in 0..10 {
        if i > 0 {
            key[0] = key[0].wrapping_add(W0);
            key[1] = key[1].wrapping_add(W1);
        }
        ctr = round(ctr, key);
    }
    ctr
}

/// Canonical 6-tuple stream key `(seed_root, partition, domain, var_group, realization, brick)` (`ADR-RNG-01`, `BD-020`, `BD-317`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RngKey {
    /// Root study seed.
    pub seed_root: u64,
    /// Spatial or logical partition ID (e.g. `"strata_bg"` hash for background CRN, `BD-317`).
    pub partition: u32,
    /// Geological / operational domain ID.
    pub domain: u32,
    /// Variable group ID (e.g. Au, Cu, S, density).
    pub var_group: u32,
    /// Realization index $r$.
    pub realization: u32,
    /// Voxel brick or entity ID.
    pub brick: u64,
}

impl RngKey {
    /// Fold the 6-tuple key into a 64-bit Philox key and upper 64-bit counter prefix via BLAKE3.
    #[must_use]
    pub fn to_philox_key_and_hi_ctr(&self) -> ([u32; 2], [u32; 2]) {
        let mut buf = [0u8; 32];
        buf[0..8].copy_from_slice(&self.seed_root.to_le_bytes());
        buf[8..12].copy_from_slice(&self.partition.to_le_bytes());
        buf[12..16].copy_from_slice(&self.domain.to_le_bytes());
        buf[16..20].copy_from_slice(&self.var_group.to_le_bytes());
        buf[20..24].copy_from_slice(&self.realization.to_le_bytes());
        buf[24..32].copy_from_slice(&self.brick.to_le_bytes());
        let d = blake3_hash(&buf);
        let b = d.as_bytes();
        let k0 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let k1 = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
        let c2 = u32::from_le_bytes([b[8], b[9], b[10], b[11]]);
        let c3 = u32::from_le_bytes([b[12], b[13], b[14], b[15]]);
        ([k0, k1], [c2, c3])
    }
}

/// Counter-based `Philox4x32-10` stream keyed by [`RngKey`] (`BD-020`).
#[derive(Clone, Debug)]
pub struct PhiloxStream {
    key: [u32; 2],
    hi_ctr: [u32; 2],
    index: u64,
}

impl PhiloxStream {
    /// Construct a new stream at counter index `0`.
    #[must_use]
    pub fn new(rng_key: RngKey) -> Self {
        let (key, hi_ctr) = rng_key.to_philox_key_and_hi_ctr();
        Self {
            key,
            hi_ctr,
            index: 0,
        }
    }

    /// Evaluate the 4x32 block at arbitrary draw index `idx` without mutating state.
    #[must_use]
    pub fn block_at(&self, idx: u64) -> [u32; 4] {
        let ctr = [
            idx as u32,
            (idx >> 32) as u32,
            self.hi_ctr[0],
            self.hi_ctr[1],
        ];
        philox4x32_10(ctr, self.key)
    }

    /// Draw the next `u64` and advance counter.
    pub fn next_u64(&mut self) -> u64 {
        let blk = self.block_at(self.index);
        self.index += 1;
        u64::from(blk[0]) | (u64::from(blk[1]) << 32)
    }

    /// Draw a uniform `f64` in $(0, 1)$ (53-bit mantissa, open interval so Box-Muller `ln` never hits 0).
    pub fn next_f64_open01(&mut self) -> f64 {
        let u = (self.next_u64() >> 11) | 1;
        (u as f64) * (1.0 / 9_007_199_254_740_992.0)
    }

    /// Draw a standard normal $\mathcal{N}(0, 1)$ via deterministic Box-Muller (`det::ln`, `det::cos`).
    pub fn next_normal(&mut self) -> f64 {
        let blk = self.block_at(self.index);
        self.index += 1;
        let u1_bits = ((u64::from(blk[0]) | (u64::from(blk[1]) << 32)) >> 11) | 1;
        let u2_bits = ((u64::from(blk[2]) | (u64::from(blk[3]) << 32)) >> 11) | 1;
        let u1 = (u1_bits as f64) * (1.0 / 9_007_199_254_740_992.0);
        let u2 = (u2_bits as f64) * (1.0 / 9_007_199_254_740_992.0);
        let r = det::sqrt(-2.0 * det::ln(u1));
        let theta = 2.0 * core::f64::consts::PI * u2;
        r * det::cos(theta)
    }
}

/// Owen-scrambled 2D Sobol point `(u, v) in [0, 1)^2` for index `i` and scramble seed `seed` (`BD-020`).
#[must_use]
pub fn sobol_owen_2d(i: u32, seed: u64) -> (f64, f64) {
    // Dimension 0: van der Corput base-2 radical inverse.
    let x0 = i.reverse_bits();
    // Dimension 1: Sobol direction numbers v_k = 1 / 2^k with primitive polynomial x + 1.
    let mut x1 = 0u32;
    let mut v = 0x8000_0000u32;
    let mut n = i;
    while n != 0 {
        if (n & 1) != 0 {
            x1 ^= v;
        }
        v ^= v >> 1;
        n >>= 1;
    }
    let s0 = laine_karras_hash(x0, seed as u32);
    let s1 = laine_karras_hash(x1, (seed >> 32) as u32 ^ 0x9E37_79B9);
    (
        f64::from(s0) * (1.0 / 4_294_967_296.0),
        f64::from(s1) * (1.0 / 4_294_967_296.0),
    )
}

/// Laine-Karras permutation hash for fast nested uniform Owen scrambling in reverse bit order.
#[inline]
fn laine_karras_hash(mut x: u32, seed: u32) -> u32 {
    x = x.reverse_bits();
    x = x.wrapping_add(seed);
    x ^= x.wrapping_mul(0x6C50_B47Cu32);
    x ^= x.wrapping_mul(0xB82F_1E52u32);
    x ^= x.wrapping_mul(0xC7AF_E638u32);
    x ^= x.wrapping_mul(0x8D22_F6E6u32);
    x.reverse_bits()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_philox4x32_known_answers() {
        let cases: [([u32; 4], [u32; 2], [u32; 4]); 3] = [
            (
                [0, 0, 0, 0],
                [0, 0],
                [0x6627_e8d5, 0xe169_c58d, 0xbc57_ac4c, 0x9b00_dbd8],
            ),
            (
                [0xffff_ffff; 4],
                [0xffff_ffff; 2],
                [0x408f_276d, 0x41c8_3b0e, 0xa20b_c7c6, 0x6d54_51fd],
            ),
            (
                [0x243f_6a88, 0x85a3_08d3, 0x1319_8a2e, 0x0370_7344],
                [0xa409_3822, 0x299f_31d0],
                [0xd16c_fe09, 0x94fd_cceb, 0x5001_e420, 0x2412_6ea1],
            ),
        ];
        for (ctr, key, want) in cases {
            assert_eq!(philox4x32_10(ctr, key), want);
        }
    }

    #[test]
    fn test_sobol_owen_stratification() {
        // 64 Owen-scrambled Sobol points in [0, 1)^2 must cover all four quadrants evenly (16 each).
        let mut quadrants = [0usize; 4];
        for i in 0..64 {
            let (u, v) = sobol_owen_2d(i, 42);
            let q = usize::from(u >= 0.5) * 2 + usize::from(v >= 0.5);
            quadrants[q] += 1;
        }
        assert_eq!(quadrants, [16, 16, 16, 16]);
    }
}
