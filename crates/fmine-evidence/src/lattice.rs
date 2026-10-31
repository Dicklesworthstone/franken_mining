//! Original FrankenMining code (not lifted). Upstream analogue: frankensim crates/fs-evidence/src/lib.rs @ 20463525d (not copied).
//! Evidence classes and knowledge states (`§7.1–§7.3`, `BD-037`, `BD-038`).

/// Epistemic evidence class ordered from strongest (`Exact`) to weakest (`Placeholder`).
///
/// Under weakest-link composition (`combine`), the resulting `EvidenceClass` is the minimum
/// (weakest) of the inputs (`NL-1..NL-6`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceClass {
    /// Unvalidated placeholder, missing datum (`-`, `N/A`), or failed calibration.
    Placeholder = 0,
    /// Uncalibrated assumption, price deck, or management judgment without backtest.
    Assumed = 1,
    /// Historical empirical baseline without active out-of-sample calibration.
    Historical = 2,
    /// Out-of-sample calibrated model or reconciled measurement passing `fmine-calib` gates.
    Calibrated = 3,
    /// Exact statutory identity, audited financial ledger cell, or mathematical constant.
    Exact = 4,
}

impl EvidenceClass {
    /// All 5 variants in ascending order of strength.
    pub const ALL: [Self; 5] = [
        Self::Placeholder,
        Self::Assumed,
        Self::Historical,
        Self::Calibrated,
        Self::Exact,
    ];

    /// Numeric strength rank (`0 = Placeholder` .. `4 = Exact`).
    #[must_use]
    pub const fn rank(self) -> u8 {
        self as u8
    }

    /// Weakest-link meet operator (`a ∧ b = min(a, b)`).
    #[must_use]
    pub const fn combine(self, other: Self) -> Self {
        if (self as u8) <= (other as u8) {
            self
        } else {
            other
        }
    }
}

impl PartialOrd for EvidenceClass {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EvidenceClass {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

/// Rumsfeldian knowledge partition (`§7.2`, `BD-038`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnowledgeState {
    /// Deterministic or tight-tolerance direct observation (`KK`).
    KnownKnown = 2,
    /// Quantified parametric or spatial uncertainty with calibrated distribution (`KU`).
    KnownUnknown = 1,
    /// Unmodeled regime shift, structural surprise, or tail-exposure term (`UU`).
    UnknownUnknown = 0,
}

impl KnowledgeState {
    /// All 3 variants.
    pub const ALL: [Self; 3] = [Self::UnknownUnknown, Self::KnownUnknown, Self::KnownKnown];

    /// Weakest-link composition (`KK ∧ KU = KU`, any `UU` yields `UU`).
    #[must_use]
    pub const fn combine(self, other: Self) -> Self {
        if (self as u8) <= (other as u8) {
            self
        } else {
            other
        }
    }
}

/// Executive traffic-light color projected from `(EvidenceClass, KnowledgeState, cal_ok, stale, uu_material)` (`§7.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClaimColor {
    /// Decision-grade (`Exact` or `Calibrated`, not stale, no material `UU`).
    Green = 2,
    /// Caution (`Historical`, `Assumed`, stale dependency, or non-zero `UU` exposure).
    Amber = 1,
    /// Blocked (`Placeholder`, failed calibration gate, or violated hard constraint).
    Red = 0,
}

impl ClaimColor {
    /// Weakest-link color combination (`Green ∧ Amber = Amber`, any `Red` yields `Red`).
    #[must_use]
    pub const fn combine(self, other: Self) -> Self {
        if (self as u8) <= (other as u8) {
            self
        } else {
            other
        }
    }
}

/// Projects `(EvidenceClass, KnowledgeState, calibration_failed, stale, uu_material)` into `ClaimColor` (`NL-4..NL-8`).
#[must_use]
pub const fn project_color(
    evidence: EvidenceClass,
    knowledge: KnowledgeState,
    calibration_failed: bool,
    stale: bool,
    uu_material: bool,
) -> ClaimColor {
    if matches!(evidence, EvidenceClass::Placeholder) || calibration_failed {
        return ClaimColor::Red;
    }
    if stale
        || uu_material
        || matches!(knowledge, KnowledgeState::UnknownUnknown)
        || matches!(evidence, EvidenceClass::Assumed | EvidenceClass::Historical)
    {
        return ClaimColor::Amber;
    }
    ClaimColor::Green
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lattice_laws_commutative_associative_idempotent() {
        for &a in &EvidenceClass::ALL {
            // Identity law: a ∧ Exact = a
            assert_eq!(a.combine(EvidenceClass::Exact), a);
            assert_eq!(EvidenceClass::Exact.combine(a), a);
            // Annihilator law: a ∧ Placeholder = Placeholder
            assert_eq!(
                a.combine(EvidenceClass::Placeholder),
                EvidenceClass::Placeholder
            );
            // Idempotence: a ∧ a = a
            assert_eq!(a.combine(a), a);

            for &b in &EvidenceClass::ALL {
                // Commutativity: a ∧ b = b ∧ a
                assert_eq!(a.combine(b), b.combine(a));
                // Monotonicity: (a ∧ b) <= a and (a ∧ b) <= b
                let ab = a.combine(b);
                assert!(ab <= a && ab <= b);

                for &c in &EvidenceClass::ALL {
                    // Associativity: (a ∧ b) ∧ c = a ∧ (b ∧ c)
                    assert_eq!(a.combine(b).combine(c), a.combine(b.combine(c)));
                }
            }
        }

        for &ka in &KnowledgeState::ALL {
            assert_eq!(ka.combine(ka), ka);
            assert_eq!(ka.combine(KnowledgeState::KnownKnown), ka);
            for &kb in &KnowledgeState::ALL {
                assert_eq!(ka.combine(kb), kb.combine(ka));
                for &kc in &KnowledgeState::ALL {
                    assert_eq!(ka.combine(kb).combine(kc), ka.combine(kb.combine(kc)));
                }
            }
        }
    }
}
