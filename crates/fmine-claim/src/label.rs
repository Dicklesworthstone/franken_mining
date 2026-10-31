//! Confidentiality labels (plan §31.1–31.2, INV-007): a join-semilattice, not a ranking.
//!
//! A label is a product of three parts: `internal`, a set of ventures and a set of walls. A non-empty set implies
//! `internal`. `Public` is the bottom, `Internal` is above it, and `Venture(V)` and `Mnpi(W)` are above that.
//! Labels with different ventures, or with different walls, are incomparable. Data joined from walls A and B carry
//! both walls, and only a reader cleared for both may see them. A wall never leaves a label once it has been joined,
//! so MNPI-derived data stay MNPI (INV-007).
//!
//! §31.1 writes `Venture(V) < Mnpi(Wall)` as one chain. The product form keeps a venture label and a wall label
//! apart (DESIGN, bd-lf4q). Whether the owner keeps it or wants the strict chain is an open decision.
//!
//! Labels have no `<`. The old tiers had a numeric rank, and rank comparison is gone:
//!
//! ```compile_fail,E0369
//! let _ = fmine_claim::Confidentiality::public() < fmine_claim::Confidentiality::internal();
//! ```

use std::collections::BTreeSet;
use std::fmt;

use fmine_core::codec::Encoder;
use fmine_evidence::RefusalCode;

/// A venture id from a [`LabelRegistry`]; no other constructor exists.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VentureId(String);

/// A wall id (`W-…`, the `mnpi_wall` caveat of §31.1) from a [`LabelRegistry`]; no other constructor exists.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WallId(String);

impl VentureId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl WallId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why an id was not admitted to, or found in, a [`LabelRegistry`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelError {
    /// Not ASCII letters, digits, `-`, `_` or `.`, empty, or a wall not starting `W-`.
    Malformed(String),
    /// Listed twice.
    Duplicate(String),
    /// Not in the registry.
    Unregistered(String),
}

/// The registered ventures and walls. The ids are data, loaded by the caller from the deployment's registry (site
/// registries live in the private overlay, §31.1); a label can name only an id held here.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LabelRegistry {
    ventures: BTreeSet<String>,
    walls: BTreeSet<String>,
}

impl LabelRegistry {
    /// Refuses a malformed or repeated id.
    pub fn new<'a>(
        ventures: impl IntoIterator<Item = &'a str>,
        walls: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, LabelError> {
        let well_formed = |id: &str| {
            !id.is_empty()
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        };
        let mut reg = Self::default();
        for v in ventures {
            if !well_formed(v) {
                return Err(LabelError::Malformed(format!("venture {v:?}")));
            }
            if !reg.ventures.insert(v.to_string()) {
                return Err(LabelError::Duplicate(format!("venture {v}")));
            }
        }
        for w in walls {
            if !well_formed(w) || !w.starts_with("W-") || w.len() == 2 {
                return Err(LabelError::Malformed(format!("wall {w:?}")));
            }
            if !reg.walls.insert(w.to_string()) {
                return Err(LabelError::Duplicate(format!("wall {w}")));
            }
        }
        Ok(reg)
    }

    pub fn venture(&self, id: &str) -> Result<VentureId, LabelError> {
        self.ventures
            .get(id)
            .map(|v| VentureId(v.clone()))
            .ok_or_else(|| LabelError::Unregistered(format!("venture {id}")))
    }

    pub fn wall(&self, id: &str) -> Result<WallId, LabelError> {
        self.walls
            .get(id)
            .map(|w| WallId(w.clone()))
            .ok_or_else(|| LabelError::Unregistered(format!("wall {id}")))
    }
}

/// A confidentiality label, or a reader's clearance: a reader may see `label` iff `label.leq(clearance)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Confidentiality {
    internal: bool,
    ventures: BTreeSet<VentureId>,
    walls: BTreeSet<WallId>,
}

impl Confidentiality {
    /// Publicly filed disclosure: the bottom.
    #[must_use]
    pub const fn public() -> Self {
        Self {
            internal: false,
            ventures: BTreeSet::new(),
            walls: BTreeSet::new(),
        }
    }

    /// Internal operational data, in no venture and behind no wall.
    #[must_use]
    pub const fn internal() -> Self {
        Self {
            internal: true,
            ventures: BTreeSet::new(),
            walls: BTreeSet::new(),
        }
    }

    /// One venture's data (§31.2).
    #[must_use]
    pub fn venture(v: VentureId) -> Self {
        Self {
            internal: true,
            ventures: BTreeSet::from([v]),
            walls: BTreeSet::new(),
        }
    }

    /// Embargoed data behind one wall (§31.1).
    #[must_use]
    pub fn mnpi(w: WallId) -> Self {
        Self {
            internal: true,
            ventures: BTreeSet::new(),
            walls: BTreeSet::from([w]),
        }
    }

    /// The least label at least as high as both: componentwise or, union and union.
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        Self {
            internal: self.internal || other.internal,
            ventures: self.ventures.union(&other.ventures).cloned().collect(),
            walls: self.walls.union(&other.walls).cloned().collect(),
        }
    }

    /// The partial order: componentwise implication and subset.
    #[must_use]
    pub fn leq(&self, other: &Self) -> bool {
        (!self.internal || other.internal)
            && self.ventures.is_subset(&other.ventures)
            && self.walls.is_subset(&other.walls)
    }

    #[must_use]
    pub fn is_public(&self) -> bool {
        !self.internal
    }

    pub fn ventures(&self) -> impl Iterator<Item = &VentureId> {
        self.ventures.iter()
    }

    pub fn walls(&self) -> impl Iterator<Item = &WallId> {
        self.walls.iter()
    }

    /// The refusal for a reader holding `clearance`, or `None` when this label is within it. A wall the reader
    /// lacks is the MNPI wall (FMINE-REF-301), as is internal data read on a public clearance. A venture the reader
    /// lacks, with every wall held, is the venture wall (FMINE-REF-302, §31.2).
    #[must_use]
    pub fn refusal_over(&self, clearance: &Self) -> Option<RefusalCode> {
        if self.leq(clearance) {
            None
        } else if self.walls.is_subset(&clearance.walls) && (!self.internal || clearance.internal) {
            Some(RefusalCode::VentureWall302)
        } else {
            Some(RefusalCode::MnpiWall301)
        }
    }

    /// The claim-id bytes. Public and internal labels are the one byte 0 or 1, as the old tier byte was, so no
    /// existing claim id moves. Any other label is tag 5 (after the old tiers 2 to 4), then its ventures and walls.
    pub(crate) fn encode(&self, enc: &mut Encoder) {
        if self.ventures.is_empty() && self.walls.is_empty() {
            enc.put_u8(u8::from(self.internal));
            return;
        }
        enc.put_u8(5);
        enc.put_u64(self.ventures.len() as u64);
        for v in &self.ventures {
            enc.put_str(&v.0);
        }
        enc.put_u64(self.walls.len() as u64);
        for w in &self.walls {
            enc.put_str(&w.0);
        }
    }
}

/// `Public`, `Internal`, or the ventures and walls joined by `+`, such as `Venture(ven_a)+Mnpi(W-1)`.
impl fmt::Display for Confidentiality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ventures.is_empty() && self.walls.is_empty() {
            return f.write_str(if self.internal { "Internal" } else { "Public" });
        }
        let parts: Vec<String> = self
            .ventures
            .iter()
            .map(|v| format!("Venture({})", v.0))
            .chain(self.walls.iter().map(|w| format!("Mnpi({})", w.0)))
            .collect();
        f.write_str(&parts.join("+"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> LabelRegistry {
        LabelRegistry::new(["ven_a", "ven_b", "ven_c"], ["W-1", "W-2", "W-3"]).unwrap()
    }

    /// Every label over three ventures and three walls: public, and internal with each pair of subsets (65).
    fn universe() -> Vec<Confidentiality> {
        let reg = registry();
        let ventures: Vec<VentureId> = ["ven_a", "ven_b", "ven_c"]
            .iter()
            .map(|v| reg.venture(v).unwrap())
            .collect();
        let walls: Vec<WallId> = ["W-1", "W-2", "W-3"]
            .iter()
            .map(|w| reg.wall(w).unwrap())
            .collect();
        let mut out = vec![Confidentiality::public()];
        for vm in 0..8_usize {
            for wm in 0..8_usize {
                let mut label = Confidentiality::internal();
                for (_, v) in ventures
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| (vm >> i) & 1 == 1)
                {
                    label = label.join(&Confidentiality::venture(v.clone()));
                }
                for (_, w) in walls.iter().enumerate().filter(|(i, _)| (wm >> i) & 1 == 1) {
                    label = label.join(&Confidentiality::mnpi(w.clone()));
                }
                out.push(label);
            }
        }
        out
    }

    /// The semilattice laws on all 65³ triples (exhaustive, so no seed): join commutative, associative and
    /// idempotent; `leq` a partial order whose least upper bound is `join`; and `a ≤ b` exactly when `a ⊔ b = b`.
    #[test]
    fn test_join_semilattice_laws_exhaustive() {
        let all = universe();
        assert_eq!(all.len(), 65);
        let distinct: BTreeSet<String> = all.iter().map(ToString::to_string).collect();
        assert_eq!(distinct.len(), 65, "every label prints differently");
        let mut triples = 0_usize;
        for a in &all {
            assert_eq!(a.join(a), *a);
            assert!(a.leq(a));
            assert!(Confidentiality::public().leq(a));
            assert!((a.walls.is_empty() && a.ventures.is_empty()) || a.internal);
            assert_eq!(a.is_public(), *a == Confidentiality::public(), "{a}");
            for b in &all {
                let ab = a.join(b);
                assert_eq!(ab, b.join(a));
                assert!(a.leq(&ab) && b.leq(&ab));
                assert_eq!(a.leq(b), ab == *b);
                if a.leq(b) && b.leq(a) {
                    assert_eq!(a, b);
                }
                for c in &all {
                    assert_eq!(ab.join(c), a.join(&b.join(c)));
                    if a.leq(b) && b.leq(c) {
                        assert!(a.leq(c));
                    }
                    if a.leq(c) && b.leq(c) {
                        assert!(ab.leq(c), "{a} and {b} are below {c}, so their join is");
                    }
                    triples += 1;
                }
            }
        }
        assert_eq!(triples, 65 * 65 * 65);
    }

    #[test]
    fn test_distinct_walls_and_ventures_are_incomparable() {
        let reg = registry();
        let (w1, w2) = (reg.wall("W-1").unwrap(), reg.wall("W-2").unwrap());
        let (a, b) = (Confidentiality::mnpi(w1.clone()), Confidentiality::mnpi(w2));
        assert!(!a.leq(&b) && !b.leq(&a));
        let (va, vb) = (reg.venture("ven_a").unwrap(), reg.venture("ven_b").unwrap());
        let (x, y) = (
            Confidentiality::venture(va.clone()),
            Confidentiality::venture(vb),
        );
        assert!(!x.leq(&y) && !y.leq(&x));
        // A venture label and a wall label stay apart, both above Internal.
        assert!(!x.leq(&a) && !a.leq(&x));
        assert!(Confidentiality::internal().leq(&x) && Confidentiality::internal().leq(&a));
        assert!(!Confidentiality::internal().leq(&Confidentiality::public()));
        // The codes: a missing wall is 301, a missing venture alone 302, internal on a public clearance 301.
        let both = a.join(&b);
        assert_eq!(both.refusal_over(&a), Some(RefusalCode::MnpiWall301));
        assert_eq!(both.refusal_over(&both), None);
        assert_eq!(
            x.join(&y).refusal_over(&x),
            Some(RefusalCode::VentureWall302)
        );
        assert_eq!(x.join(&a).refusal_over(&x), Some(RefusalCode::MnpiWall301));
        let pub_ = Confidentiality::public();
        assert_eq!(
            Confidentiality::internal().refusal_over(&pub_),
            Some(RefusalCode::MnpiWall301)
        );
        assert_eq!(x.refusal_over(&pub_), Some(RefusalCode::MnpiWall301));
        assert_eq!(x.join(&a).to_string(), "Venture(ven_a)+Mnpi(W-1)");
        assert_eq!(w1.as_str(), "W-1");
        assert_eq!(va.as_str(), "ven_a");
    }

    #[test]
    fn test_registry_admits_only_well_formed_unique_ids() {
        let reg = registry();
        assert_eq!(
            reg.venture("ven_z"),
            Err(LabelError::Unregistered("venture ven_z".into()))
        );
        assert_eq!(
            reg.wall("W-9"),
            Err(LabelError::Unregistered("wall W-9".into()))
        );
        assert_eq!(
            reg.wall("ven_a"),
            Err(LabelError::Unregistered("wall ven_a".into()))
        );
        for (ventures, walls, want) in [
            (
                vec!["a b"],
                vec![],
                LabelError::Malformed("venture \"a b\"".into()),
            ),
            (
                vec![""],
                vec![],
                LabelError::Malformed("venture \"\"".into()),
            ),
            (
                vec!["v", "v"],
                vec![],
                LabelError::Duplicate("venture v".into()),
            ),
            (
                vec![],
                vec!["X-1"],
                LabelError::Malformed("wall \"X-1\"".into()),
            ),
            (
                vec![],
                vec!["W-"],
                LabelError::Malformed("wall \"W-\"".into()),
            ),
            (
                vec![],
                vec!["W-1", "W-1"],
                LabelError::Duplicate("wall W-1".into()),
            ),
        ] {
            assert_eq!(LabelRegistry::new(ventures, walls), Err(want));
        }
    }

    /// Public and internal encode as the old tier bytes 0 and 1; a set label is tagged 5 with its ids, so labels
    /// differing only in a venture or wall encode differently.
    #[test]
    fn test_encoding_keeps_old_bytes_and_separates_sets() {
        let bytes = |c: &Confidentiality| {
            let mut enc = Encoder::new_frame(*b"TEST", 1);
            c.encode(&mut enc);
            enc.finish()
        };
        let with = |b: u8| {
            let mut enc = Encoder::new_frame(*b"TEST", 1);
            enc.put_u8(b);
            enc.finish()
        };
        assert_eq!(bytes(&Confidentiality::public()), with(0));
        assert_eq!(bytes(&Confidentiality::internal()), with(1));
        // Any other label opens with tag 5, never an old tier byte: inside a claim id the next field follows, so a
        // label opening with 1 could read as Internal followed by other fields.
        let reg = registry();
        let label = Confidentiality::venture(reg.venture("ven_a").unwrap())
            .join(&Confidentiality::mnpi(reg.wall("W-2").unwrap()));
        let mut enc = Encoder::new_frame(*b"TEST", 1);
        enc.put_u8(5);
        enc.put_u64(1);
        enc.put_str("ven_a");
        enc.put_u64(1);
        enc.put_str("W-2");
        assert_eq!(bytes(&label), enc.finish());
        let all = universe();
        let encoded: BTreeSet<Vec<u8>> = all.iter().map(bytes).collect();
        assert_eq!(encoded.len(), all.len());
    }
}
