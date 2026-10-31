//! The plan's epistemic types: evidence classes (`§7.1`), knowledge states (`§7.2`) and `EpistemicStatus` (`§8.1`),
//! with the orders NL-2 and NL-5 are stated over (`BD-038`).
//!
//! A class says how something is known and a state says what may be said about it now. Neither says who stands
//! behind it (NL-3). An issuer's published figure is `Assertion { source: Issuer }`, whatever its state: reading a
//! cell correctly is not evidence that the cell is true.

use std::cmp::Ordering;

use fmine_core::id::ObjectId;

/// Area-of-applicability status of a transferred claim, weakest first.
///
/// The plan names `Transferred{aoa}` without defining `aoa`. OPEN for the owner: these three values and their order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AoaStatus {
    /// The subject lies outside the analogue's area of applicability.
    Outside,
    /// Nobody has assessed it.
    NotAssessed,
    /// The subject lies inside it.
    Inside,
}

/// Who made an [`EvidenceClass::Assertion`].
///
/// When two sources meet, the alphabetically first survives (`Ord` here). OPEN for the owner: the rule is arbitrary
/// but total, and either source's statement stays reachable through provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssertionSource {
    /// An agent's proposal.
    Agent,
    /// An LLM or OCR extraction from a document.
    Extraction,
    /// The issuer's own published statement.
    Issuer,
    /// A third party's statement.
    Secondary,
}

/// How something is known (`§7.1`). A partial order under NL-2, so it has no `Ord`:
///
/// `Measured > Derived > ModelledCalibrated > {ModelledUncalibrated, Transferred, Elicited} > Assumed > Assertion`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceClass {
    /// QA/QC-passed measurement with a characterized error model.
    Measured,
    /// A deterministic function of other claims.
    Derived,
    /// Model output inside a domain where its forecasts have a scored track record.
    ModelledCalibrated,
    /// Model output without a usable track record.
    ModelledUncalibrated,
    /// Structured expert judgment, stored and scored.
    Elicited,
    /// Carried from an analogue.
    Transferred {
        /// Its area-of-applicability status.
        aoa: AoaStatus,
    },
    /// Stated without support.
    Assumed,
    /// Unverified statement: an imported claim, an extraction, an agent proposal.
    Assertion {
        /// Who made it.
        source: AssertionSource,
    },
}

impl EvidenceClass {
    /// An issuer's published figure (`§24.3`): whatever it says, the issuer said it.
    pub const ISSUER: Self = Self::Assertion {
        source: AssertionSource::Issuer,
    };

    /// Height in the NL-2 order. Equal ranks are not equal strength: the three classes at rank 2 are incomparable,
    /// so compare with [`Self::no_stronger_than`].
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Measured => 5,
            Self::Derived => 4,
            Self::ModelledCalibrated => 3,
            Self::ModelledUncalibrated | Self::Elicited | Self::Transferred { .. } => 2,
            Self::Assumed => 1,
            Self::Assertion { .. } => 0,
        }
    }

    /// `self` is no stronger than `other`. Two assertions compare by variant alone, because a source is not a
    /// strength; two transfers compare by [`AoaStatus`]. Named apart from `PartialOrd::le`, which is false for
    /// assertions from different sources, since those are not equal.
    #[must_use]
    pub fn no_stronger_than(self, other: Self) -> bool {
        match (self, other) {
            (Self::Transferred { aoa: a }, Self::Transferred { aoa: b }) => a <= b,
            (Self::Assertion { .. }, Self::Assertion { .. }) => true,
            _ => self == other || self.rank() < other.rank(),
        }
    }

    /// The greatest lower bound, which is NL-2's "weakest input class".
    ///
    /// NL-2 leaves two incomparable inputs undefined. Their lower bound is `Assumed`, so `Transferred ∧ Elicited` is
    /// `Assumed`: it never promotes, and it demotes two mid-tier inputs (plan erratum, CONTRACT §7).
    #[must_use]
    pub fn meet(self, other: Self) -> Self {
        match (self, other) {
            (Self::Transferred { aoa: a }, Self::Transferred { aoa: b }) => {
                Self::Transferred { aoa: a.min(b) }
            }
            (Self::Assertion { source: a }, Self::Assertion { source: b }) => {
                Self::Assertion { source: a.min(b) }
            }
            _ if self == other => self,
            _ if self.rank() < other.rank() => self,
            _ if other.rank() < self.rank() => other,
            _ => Self::Assumed,
        }
    }

    /// Stable `[variant, payload]` bytes for canonical encodings. Never reorder: claim ids hash them.
    #[must_use]
    pub const fn tag(self) -> [u8; 2] {
        match self {
            Self::Measured => [0, 0],
            Self::Derived => [1, 0],
            Self::ModelledCalibrated => [2, 0],
            Self::ModelledUncalibrated => [3, 0],
            Self::Elicited => [4, 0],
            Self::Transferred { aoa } => [5, aoa as u8],
            Self::Assumed => [6, 0],
            Self::Assertion { source } => [7, source as u8],
        }
    }
}

impl PartialOrd for EvidenceClass {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (
            self == other,
            self.no_stronger_than(*other),
            other.no_stronger_than(*self),
        ) {
            (true, _, _) => Some(Ordering::Equal),
            (false, true, false) => Some(Ordering::Less),
            (false, false, true) => Some(Ordering::Greater),
            _ => None,
        }
    }
}

/// What the system may say about a subject now (`§7.2`). Renderers show the state instead of a number unless it is
/// `Known` or `Estimated` (G-EXEC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnowledgeState {
    /// Evidence suffices at the claim's support and validity.
    Known,
    /// Model-based, with stated uncertainty.
    Estimated,
    /// No admissible basis. Not "false".
    Unknown,
    /// Admissible sources disagree beyond tolerance.
    Conflicted,
    /// The basis is older than its validity window, or a dependency was superseded.
    Stale,
    /// The sensor or method could not have seen it. Not "no detection".
    NotObservable,
    /// Exists, but the viewer lacks the capability. Not "absent".
    Redacted,
    /// A flip margin is unbounded: the verdict can go either way.
    Indeterminate,
    /// The question does not apply to this subject.
    NotApplicable,
}

impl KnowledgeState {
    /// Precedence under conjunction. `NotApplicable` is lowest, which makes it the identity.
    const fn precedence(self) -> u8 {
        match self {
            Self::NotApplicable => 0,
            Self::Known => 1,
            Self::Estimated => 2,
            Self::Stale => 3,
            Self::Unknown => 4,
            Self::Indeterminate => 5,
            Self::Conflicted => 6,
            Self::NotObservable => 7,
            Self::Redacted => 8,
        }
    }

    /// NL-5 conjunction: the operand of higher precedence, so a state never upgrades.
    ///
    /// The plan gives only `unknown ∧ known = unknown` and that `redacted` and `not_observable` persist. The rest of
    /// the order is a design choice awaiting the owner, because it decides what renderers suppress (CONTRACT §7).
    #[must_use]
    pub const fn and(self, other: Self) -> Self {
        if self.precedence() >= other.precedence() {
            self
        } else {
            other
        }
    }

    /// Stable byte for canonical encodings. Never reorder: claim ids hash it.
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }
}

/// Identifier of a hypothesis set behind a structural unknown (plan line 713).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HypothesisSetId(pub ObjectId);

/// The kind of a known unknown, weakest knowledge last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KuKind {
    /// Irreducible variability.
    Aleatory,
    /// Reducible uncertainty about a parameter.
    Parametric,
    /// Uncertainty about which model holds.
    Structural {
        /// The hypotheses considered.
        set: HypothesisSetId,
    },
}

/// Whether the unknowns behind a claim are known to exist (`§8.1`), weakest knowledge last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EpistemicStatus {
    /// Nothing material is uncertain at this support.
    KnownKnown,
    /// Uncertainty of a stated kind.
    KnownUnknown(KuKind),
    /// A budget stands in for what was not modelled.
    UnmodelledBudget,
}

impl EpistemicStatus {
    /// The weaker of the two. Two structural unknowns keep one hypothesis set, the greater id; the other stays
    /// reachable through provenance.
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        self.max(other)
    }

    /// Stable bytes for canonical encodings. Never reorder: claim ids hash them.
    #[must_use]
    pub const fn tag(self) -> [u8; 18] {
        let mut out = [0u8; 18];
        match self {
            Self::KnownKnown => {}
            Self::KnownUnknown(KuKind::Aleatory) => out[0] = 1,
            Self::KnownUnknown(KuKind::Parametric) => out[0] = 2,
            Self::KnownUnknown(KuKind::Structural { set }) => {
                out[0] = 3;
                out[1] = set.0.kind as u8;
                let mut i = 0;
                while i < 16 {
                    out[2 + i] = set.0.bytes[i];
                    i += 1;
                }
            }
            Self::UnmodelledBudget => out[0] = 4,
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::id::PrefixKind;
    use fmine_core::rand::{PhiloxStream, RngKey};
    use std::collections::BTreeSet;

    use AssertionSource as Src;
    use EvidenceClass as C;
    use KnowledgeState as K;

    const T: C = C::Transferred {
        aoa: AoaStatus::NotAssessed,
    };
    const A: C = C::Assertion {
        source: Src::Issuer,
    };
    /// The eight classes in the order of the tables below, payloads held fixed.
    const EIGHT: [C; 8] = [
        C::Measured,
        C::Derived,
        C::ModelledCalibrated,
        C::ModelledUncalibrated,
        C::Elicited,
        T,
        C::Assumed,
        A,
    ];
    /// The nine states, weakest knowledge last, written out independently of `precedence`.
    const NINE: [K; 9] = [
        K::NotApplicable,
        K::Known,
        K::Estimated,
        K::Stale,
        K::Unknown,
        K::Indeterminate,
        K::Conflicted,
        K::NotObservable,
        K::Redacted,
    ];

    /// Every class with every payload: 6 plain, 3 transfers, 4 assertions.
    fn every_class() -> Vec<C> {
        let mut all = vec![
            C::Measured,
            C::Derived,
            C::ModelledCalibrated,
            C::ModelledUncalibrated,
            C::Elicited,
            C::Assumed,
        ];
        for aoa in [
            AoaStatus::Outside,
            AoaStatus::NotAssessed,
            AoaStatus::Inside,
        ] {
            all.push(C::Transferred { aoa });
        }
        for source in [Src::Agent, Src::Extraction, Src::Issuer, Src::Secondary] {
            all.push(C::Assertion { source });
        }
        all
    }

    fn every_status() -> Vec<EpistemicStatus> {
        let set =
            |seed: &[u8]| HypothesisSetId(ObjectId::from_canonical_bytes(PrefixKind::Domain, seed));
        vec![
            EpistemicStatus::KnownKnown,
            EpistemicStatus::KnownUnknown(KuKind::Aleatory),
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            EpistemicStatus::KnownUnknown(KuKind::Structural { set: set(b"a") }),
            EpistemicStatus::KnownUnknown(KuKind::Structural { set: set(b"b") }),
            EpistemicStatus::UnmodelledBudget,
        ]
    }

    #[test]
    fn test_le_table_64_pairs() {
        #[rustfmt::skip]
        const LE: [[u8; 8]; 8] = [
            // a ≤ M  D MC MU  E  T Am As
            [1, 0, 0, 0, 0, 0, 0, 0], // Measured
            [1, 1, 0, 0, 0, 0, 0, 0], // Derived
            [1, 1, 1, 0, 0, 0, 0, 0], // ModelledCalibrated
            [1, 1, 1, 1, 0, 0, 0, 0], // ModelledUncalibrated
            [1, 1, 1, 0, 1, 0, 0, 0], // Elicited
            [1, 1, 1, 0, 0, 1, 0, 0], // Transferred
            [1, 1, 1, 1, 1, 1, 1, 0], // Assumed
            [1, 1, 1, 1, 1, 1, 1, 1], // Assertion
        ];
        for (i, a) in EIGHT.iter().enumerate() {
            for (j, b) in EIGHT.iter().enumerate() {
                assert_eq!(a.no_stronger_than(*b), LE[i][j] == 1, "{a:?} ≤ {b:?}");
            }
        }
    }

    #[test]
    fn test_meet_table_64_pairs() {
        #[rustfmt::skip]
        const MEET: [[usize; 8]; 8] = [
            [0, 1, 2, 3, 4, 5, 6, 7],
            [1, 1, 2, 3, 4, 5, 6, 7],
            [2, 2, 2, 3, 4, 5, 6, 7],
            [3, 3, 3, 3, 6, 6, 6, 7],
            [4, 4, 4, 6, 4, 6, 6, 7],
            [5, 5, 5, 6, 6, 5, 6, 7],
            [6, 6, 6, 6, 6, 6, 6, 7],
            [7, 7, 7, 7, 7, 7, 7, 7],
        ];
        for (i, a) in EIGHT.iter().enumerate() {
            for (j, b) in EIGHT.iter().enumerate() {
                assert_eq!(a.meet(*b), EIGHT[MEET[i][j]], "{a:?} ∧ {b:?}");
            }
        }
    }

    #[test]
    fn test_meet_transferred_elicited_is_assumed() {
        assert_eq!(T.meet(C::Elicited), C::Assumed);
        assert_eq!(C::Elicited.meet(T), C::Assumed);
    }

    #[test]
    fn test_rank2_classes_incomparable() {
        let rank2 = [C::ModelledUncalibrated, C::Elicited, T];
        for a in rank2 {
            for b in rank2 {
                if a != b {
                    assert!(!a.no_stronger_than(b), "{a:?} ≤ {b:?}");
                    assert_eq!(a.partial_cmp(&b), None);
                }
            }
        }
    }

    #[test]
    fn test_payloads_never_change_the_order_of_assertions_or_promote_transfers() {
        let (agent, issuer) = (
            C::Assertion { source: Src::Agent },
            C::Assertion {
                source: Src::Issuer,
            },
        );
        assert!(agent.no_stronger_than(issuer) && issuer.no_stronger_than(agent));
        assert_eq!(agent.partial_cmp(&issuer), None);
        assert_eq!(issuer.meet(agent), agent);

        let (inside, outside) = (
            C::Transferred {
                aoa: AoaStatus::Inside,
            },
            C::Transferred {
                aoa: AoaStatus::Outside,
            },
        );
        assert!(outside < inside);
        assert_eq!(inside.meet(T), T);
        assert_eq!(inside.meet(outside), outside);
    }

    #[test]
    fn test_partial_ord_agrees_with_eq_and_le() {
        for a in every_class() {
            for b in every_class() {
                assert_eq!(a == b, a.partial_cmp(&b) == Some(Ordering::Equal));
                assert_eq!(
                    a < b,
                    a.no_stronger_than(b) && !b.no_stronger_than(a),
                    "{a:?} < {b:?}"
                );
                assert_eq!(a.partial_cmp(&b).map(Ordering::reverse), b.partial_cmp(&a));
            }
        }
    }

    #[test]
    fn test_tags_are_distinct() {
        let classes: BTreeSet<_> = every_class().iter().map(|c| c.tag()).collect();
        assert_eq!(classes.len(), 13);
        let states: BTreeSet<_> = NINE.iter().map(|k| k.tag()).collect();
        assert_eq!(states.len(), 9);
        let statuses: BTreeSet<_> = every_status().iter().map(|s| s.tag()).collect();
        assert_eq!(statuses.len(), 6);
    }

    #[test]
    fn test_and_table_81_pairs() {
        for (i, a) in NINE.iter().enumerate() {
            for (j, b) in NINE.iter().enumerate() {
                assert_eq!(a.and(*b), NINE[i.max(j)], "{a:?} ∧ {b:?}");
            }
        }
        // The rows the plan states outright.
        assert_eq!(K::Unknown.and(K::Known), K::Unknown);
        for state in NINE {
            assert_eq!(K::Redacted.and(state), K::Redacted);
            if state != K::Redacted {
                assert_eq!(K::NotObservable.and(state), K::NotObservable);
            }
        }
    }

    #[test]
    fn test_not_applicable_is_and_identity() {
        for state in NINE {
            assert_eq!(K::NotApplicable.and(state), state);
            assert_eq!(state.and(K::NotApplicable), state);
        }
    }

    #[test]
    fn test_meet_commutative_associative_idempotent() {
        let all = every_class();
        for &a in &all {
            assert_eq!(a.meet(a), a);
            for &b in &all {
                assert_eq!(a.meet(b), b.meet(a), "{a:?}, {b:?}");
                for &c in &all {
                    assert_eq!(a.meet(b).meet(c), a.meet(b.meet(c)), "{a:?}, {b:?}, {c:?}");
                }
            }
        }
    }

    #[test]
    fn test_meet_le_both_operands() {
        for a in every_class() {
            for b in every_class() {
                let m = a.meet(b);
                assert!(
                    m.no_stronger_than(a) && m.no_stronger_than(b),
                    "{a:?} ∧ {b:?} = {m:?}"
                );
            }
        }
    }

    #[test]
    fn test_and_commutative_associative() {
        for a in NINE {
            assert_eq!(a.and(a), a);
            for b in NINE {
                assert_eq!(a.and(b), b.and(a));
                for c in NINE {
                    assert_eq!(a.and(b).and(c), a.and(b.and(c)));
                }
            }
        }
    }

    #[test]
    fn test_epistemic_and_is_a_commutative_associative_weakening() {
        let all = every_status();
        for &a in &all {
            assert_eq!(a.and(EpistemicStatus::KnownKnown), a);
            assert_eq!(
                a.and(EpistemicStatus::UnmodelledBudget),
                EpistemicStatus::UnmodelledBudget
            );
            for &b in &all {
                assert_eq!(a.and(b), b.and(a));
                assert!(a.and(b) >= a && a.and(b) >= b);
                for &c in &all {
                    assert_eq!(a.and(b).and(c), a.and(b.and(c)));
                }
            }
        }
    }

    /// PO-PLUMB-1 for NL-2 and NL-5. Each node takes one to three earlier parents, or is a random leaf; its class is
    /// the meet of theirs and its state the conjunction of theirs. No node may outrank any ancestor.
    #[test]
    fn test_random_dags_nl2_nl5_100k() {
        const CASES: usize = 100_000;
        let key = RngKey {
            seed_root: 0x0009_D0C5_0038,
            partition: 2,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        };
        let mut rng = PhiloxStream::new(key);
        let classes = every_class();
        let (mut max_depth, mut violations) = (0_u32, 0_u32);
        for _ in 0..CASES {
            let n = 2 + (rng.next_u64() % 15) as usize;
            let mut nodes: Vec<(C, K, u32, u32)> = Vec::with_capacity(n);
            for i in 0..n {
                let parents = if i == 0 { 0 } else { rng.next_u64() % 4 };
                if parents == 0 {
                    let class = classes[(rng.next_u64() % classes.len() as u64) as usize];
                    let state = NINE[(rng.next_u64() % 9) as usize];
                    nodes.push((class, state, 0, 0));
                    continue;
                }
                let p0 = (rng.next_u64() % i as u64) as usize;
                let (mut class, mut state, mut ancestors, mut depth) = nodes[p0];
                ancestors |= 1 << p0;
                for _ in 1..parents {
                    let p = (rng.next_u64() % i as u64) as usize;
                    class = class.meet(nodes[p].0);
                    state = state.and(nodes[p].1);
                    ancestors |= nodes[p].2 | (1 << p);
                    depth = depth.max(nodes[p].3);
                }
                nodes.push((class, state, ancestors, depth + 1));
                max_depth = max_depth.max(depth + 1);
            }
            for (class, state, ancestors, _) in &nodes {
                for (p, parent) in nodes.iter().enumerate() {
                    if ancestors & (1 << p) == 0 {
                        continue;
                    }
                    if !class.no_stronger_than(parent.0)
                        || state.precedence() < parent.1.precedence()
                    {
                        violations += 1;
                    }
                }
            }
        }
        println!(
            "{{\"test\":\"nl2_nl5_dags\",\"cases\":{CASES},\"rng_key\":\"{:016x}\",\"max_depth\":{max_depth},\
             \"violations\":{violations}}}",
            key.seed_root
        );
        assert_eq!(violations, 0);
        assert!(max_depth >= 4, "the DAGs must be more than a level deep");
    }
}
