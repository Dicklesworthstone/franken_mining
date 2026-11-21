//! Determinism: BitwisePortable
//! Reusable metamorphic law verification library (`BD-143`, plan §33.4, `PO-UNIT-1`, `PO-UNIT-3`).
//!
//! Shared across Testpit and verification crates (`fmine-gen`, `fmine-verify`, `xtask`):
//! - `assert_split_merge_lot_invariance`: splitting or merging dry lots leaves total dry mass,
//!   total contained metal, and mass-weighted grade invariant within exact tolerance (`PO-UNIT-3`);
//! - `assert_permutation_digest_invariance`: permuting input order leaves canonical sorted
//!   digests and closures bit-identical;
//! - `assert_exact_statutory_unit_roundtrips`: exact `ExactQty` round-trips across `g ↔ troy oz`,
//!   `g ↔ avdp lb`, and `g/t ↔ oz/st` with zero residual (`PO-UNIT-1`);
//! - `assert_zero_mass_identity`: adding a zero-mass lot leaves mass-weighted grade unchanged.

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};
use fmine_units::{
    Contained, Dry, DryMassGrade, ExactQty, Grade, Mass, Neutral, Qty, mass_weighted_mean,
};

/// One extensive dry lot `(dry_tonnes, grade_g_per_t)` for split/merge metamorphic checks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LotSpec {
    pub dry_tonnes: f64,
    pub grade_g_per_t: f64,
}

impl LotSpec {
    fn to_typed(self) -> DryMassGrade<Contained> {
        (
            Qty::<Mass, Dry, Neutral>::new(self.dry_tonnes),
            Grade::<Contained>::from_g_per_t(self.grade_g_per_t),
        )
    }
}

/// Verify `PO-UNIT-3`: splitting every lot in `lots` by fraction `split_frac ∈ (0, 1)` into two
/// sub-lots preserves total dry mass, total contained metal, and mass-weighted grade within `rel_tol`.
pub fn assert_split_merge_lot_invariance(
    lots: &[LotSpec],
    split_frac: f64,
    rel_tol: f64,
) -> Result<(), String> {
    if lots.is_empty() || !split_frac.is_finite() || split_frac <= 0.0 || split_frac >= 1.0 {
        return Err("invalid lots or split_frac".to_string());
    }
    let orig: Vec<DryMassGrade<Contained>> = lots.iter().map(|l| l.to_typed()).collect();
    let mut split = Vec::with_capacity(lots.len() * 2);
    for l in lots {
        split.push(
            LotSpec {
                dry_tonnes: l.dry_tonnes * split_frac,
                grade_g_per_t: l.grade_g_per_t,
            }
            .to_typed(),
        );
        split.push(
            LotSpec {
                dry_tonnes: l.dry_tonnes * (1.0 - split_frac),
                grade_g_per_t: l.grade_g_per_t,
            }
            .to_typed(),
        );
    }

    let (m_orig_q, g_orig) = mass_weighted_mean(&orig).map_err(|e| e.to_string())?;
    let (m_split_q, g_split) = mass_weighted_mean(&split).map_err(|e| e.to_string())?;

    let m_orig = m_orig_q.value();
    let m_split = m_split_q.value();
    let au_orig: f64 = lots.iter().map(|l| l.dry_tonnes * l.grade_g_per_t).sum();
    let au_split: f64 = split.iter().map(|(m, g)| m.value() * g.g_per_t()).sum();

    let rel = |a: f64, b: f64| (a - b).abs() / a.abs().max(1.0);
    if rel(m_orig, m_split) > rel_tol {
        return Err(format!(
            "dry mass drifted under split: {m_orig} vs {m_split}"
        ));
    }
    if rel(au_orig, au_split) > rel_tol {
        return Err(format!(
            "contained metal drifted under split: {au_orig} vs {au_split}"
        ));
    }
    if rel(g_orig.g_per_t(), g_split.g_per_t()) > rel_tol {
        return Err(format!(
            "mass-weighted grade drifted under split: {} vs {}",
            g_orig.g_per_t(),
            g_split.g_per_t()
        ));
    }

    // Zero-mass lot identity law (§33.4): appending a zero-mass lot changes nothing.
    let mut with_zero = orig;
    with_zero.push(
        LotSpec {
            dry_tonnes: 0.0,
            grade_g_per_t: 999.0,
        }
        .to_typed(),
    );
    let (_m_zero, g_zero) = mass_weighted_mean(&with_zero).map_err(|e| e.to_string())?;
    if rel(g_orig.g_per_t(), g_zero.g_per_t()) > rel_tol {
        return Err(format!(
            "zero-mass lot altered weighted grade: {} vs {}",
            g_orig.g_per_t(),
            g_zero.g_per_t()
        ));
    }
    Ok(())
}

/// Compute a canonical order-invariant SHA-256 digest over `(key, value)` records.
#[must_use]
pub fn canonical_set_digest(items: &[(&str, &[u8])]) -> Digest32 {
    let mut sorted: Vec<(&str, &[u8])> = items.to_vec();
    sorted.sort_unstable();
    let mut enc = Encoder::new_frame(*b"META", 1);
    enc.put_u32(sorted.len() as u32);
    for (k, v) in sorted {
        enc.put_str(k);
        enc.put_bytes(v);
    }
    sha256(&enc.finish())
}

/// Verify permutation invariance: reversing and rotating `items` yields the exact same digest.
pub fn assert_permutation_digest_invariance(items: &[(&str, &[u8])]) -> Result<(), String> {
    let base = canonical_set_digest(items);
    let mut rev: Vec<(&str, &[u8])> = items.to_vec();
    rev.reverse();
    if canonical_set_digest(&rev) != base {
        return Err("reverse permutation changed canonical set digest".to_string());
    }
    if items.len() > 1 {
        let mut rot = items.to_vec();
        rot.rotate_left(1);
        if canonical_set_digest(&rot) != base {
            return Err("cyclic rotation changed canonical set digest".to_string());
        }
    }
    Ok(())
}

/// Verify exact statutory unit round-trips (`g ↔ troy oz`, `g ↔ avdp lb`, `g/t ↔ oz/st`, `PO-UNIT-1`)
/// with strict zero residual in `ExactQty`.
pub fn assert_exact_statutory_unit_roundtrips(values: &[ExactQty]) -> Result<(), String> {
    for &v in values {
        let oz_back =
            ExactQty::grams_to_troy_oz(ExactQty::troy_oz_to_grams(v).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if oz_back != v {
            return Err(format!("troy oz round-trip failed: {v:?} -> {oz_back:?}"));
        }
        let lb_back =
            ExactQty::grams_to_avdp_lb(ExactQty::avdp_lb_to_grams(v).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if lb_back != v {
            return Err(format!("avdp lb round-trip failed: {v:?} -> {lb_back:?}"));
        }
        let grade_back = ExactQty::g_per_t_to_oz_per_st(
            ExactQty::oz_per_st_to_g_per_t(v).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if grade_back != v {
            return Err(format!(
                "oz/st <-> g/t round-trip failed: {v:?} -> {grade_back:?}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metamorphic_library_split_merge_permutation_and_unit_roundtrips() {
        let lots = [
            LotSpec {
                dry_tonnes: 12_500.0,
                grade_g_per_t: 1.85,
            },
            LotSpec {
                dry_tonnes: 7_250.0,
                grade_g_per_t: 4.10,
            },
            LotSpec {
                dry_tonnes: 30_000.0,
                grade_g_per_t: 0.72,
            },
        ];
        assert_split_merge_lot_invariance(&lots, 0.37, 1e-12).expect("split/merge invariant");

        let pairs: &[(&str, &[u8])] = &[
            ("wb_q1", b"cells-q1"),
            ("wb_q2", b"cells-q2"),
            ("reg", b"barrick.toml"),
        ];
        assert_permutation_digest_invariance(pairs).expect("permutation invariant");

        let samples: Vec<ExactQty> = (1..=50)
            .map(|i| ExactQty::new(i * 17, 100).unwrap())
            .collect();
        assert_exact_statutory_unit_roundtrips(&samples).expect("statutory round-trips exact");
    }
}
