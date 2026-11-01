//! Universal `Claim<T>` epistemic envelope (`§8.1`), non-laundering laws `NL-1..NL-8` (`GATE-PLUMB-E1`),
//! confidentiality lattice (`INV-007`), `UuTerm` guard (`INV-015`), spatial support / additivity
//! guards (`INV-014`), and provenance DAG staleness propagation (`INV-013`).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use fmine_core::anchor::SourceAnchor;
use fmine_core::codec::Encoder;
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::color::{IntervalOp, ValidityDomain, compose};
use fmine_evidence::{
    Color, EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode, project_color,
};

/// Type alias for a `clm_...` content-addressed [`ObjectId`].
pub type ClaimId = ObjectId;

/// Uncertainty representation carried by a `Claim` (`BD-041`).
#[derive(Debug, Clone, PartialEq)]
pub enum Representation {
    /// Deterministic point value (`KK` or audited filing cell).
    Deterministic,
    /// Parametric Gaussian summary (`mean`, `std_dev >= 0`).
    Gaussian { mean: f64, std_dev: f64 },
    /// Quantile triplet (`p10 <= p50 <= p90`).
    Quantiles { p10: f64, p50: f64, p90: f64 },
    /// Conditional simulation / ensemble summary (`n` realizations).
    EnsembleSummary {
        n: u32,
        mean: f64,
        p10: f64,
        p50: f64,
        p90: f64,
    },
    /// Bounded interval `[lo, hi]`.
    Interval { lo: f64, hi: f64 },
    /// Unresolved placeholder (`-`, `N/A`, `#REF!`).
    Placeholder { token: String },
}

/// Physical support volume of a `Claim` (`BD-045`, `INV-014`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpatialSupport {
    /// Point or drill-core assay sample (`~dm^3`).
    PointCore,
    /// Downhole composite interval (`length_mm` in millimetres).
    Composite { length_mm: u32 },
    /// Selective Mining Unit (SMU) block (`dx_cm`, `dy_cm`, `dz_cm` in centimetres).
    SmuBlock { dx_cm: u32, dy_cm: u32, dz_cm: u32 },
    /// Underground stope or open-pit blast polygon volume.
    MiningPolygon,
    /// Plant feed lot or stockpile parcel.
    PlantLot,
    /// Site or corporate period aggregate (quarter / year).
    PeriodAggregate,
}

/// Additivity classification under linear aggregation (`BD-045`, `INV-014`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdditivityClass {
    /// Extensive quantity (`dry_mass`, `contained_metal`, `Money`) that sums directly.
    ExtensiveAdditive,
    /// Intensive ratio (`grade`, `recovery`, `AISC/oz`) requiring denominator weighting.
    IntensiveWeighted,
    /// Strictly non-additive (`pH`, `RMR`, `FactorOfSafety`, `log_permeability`).
    NonAdditive,
}

pub mod dag;
mod label;
pub mod reliance;

pub use label::{Confidentiality, LabelError, LabelRegistry, VentureId, WallId};

/// Explicit Unknown-Unknown (`UU`) structural tail-risk scenario (`BD-044`, `INV-015`).
#[derive(Debug, Clone, PartialEq)]
pub struct UuTerm {
    pub scenario_id: String,
    pub description: String,
    /// Estimated unhedged downside exposure in USD millions (`$M`).
    pub downside_exposure_usd_m: f64,
    /// Leading observable indicator or tripwire.
    pub trigger_indicator: String,
}

/// Universal epistemic envelope wrapping any value `T` (`§8.1`, `BD-040`).
///
/// A claim's colour is computed, never set by a caller (NL-1). No constructor takes one, and what it is computed
/// from is private:
///
/// ```compile_fail,E0616
/// fn forge(claim: &mut fmine_claim::Claim<f64>) {
///     claim.color_basis = None;
/// }
/// ```
///
/// `claim_id` is the BLAKE3 id of [`Claim::canonical_body`]: "append-only; supersede, never edit" (§8.1). A write
/// to a hashed field after construction leaves an id that names another body. [`Claim::check_id`] refuses such a
/// claim, and so does [`ClaimDag::insert`]. [`Claim::downgraded`] is the supersession path for evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct Claim<T> {
    pub claim_id: ClaimId,
    pub metric_key: String,
    pub value: T,
    pub representation: Representation,
    pub support: SpatialSupport,
    pub additivity: AdditivityClass,
    pub evidence: EvidenceClass,
    pub epistemic: EpistemicStatus,
    pub state: KnowledgeState,
    pub confidentiality: Confidentiality,
    pub uu_terms: Vec<UuTerm>,
    pub parents: Vec<ClaimId>,
    pub anchors: Vec<SourceAnchor>,
    pub supersedes: Option<ClaimId>,
    pub stale: bool,
    pub calibration_failed: bool,
    /// The anchor's colour on a leaf, the composition of the parents' colours on a derived claim.
    color_basis: Option<Color>,
    /// The value's canonical text, hashed into the id in place of `value`.
    value_tag: String,
}

impl<T> Claim<T> {
    /// Constructs a new `Claim<T>` and deterministically computes its BLAKE3 `claim_id`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        metric_key: impl Into<String>,
        value: T,
        canonical_value_tag: &str,
        representation: Representation,
        support: SpatialSupport,
        additivity: AdditivityClass,
        evidence: EvidenceClass,
        epistemic: EpistemicStatus,
        state: KnowledgeState,
        confidentiality: Confidentiality,
        uu_terms: Vec<UuTerm>,
        parents: Vec<ClaimId>,
        anchors: Vec<SourceAnchor>,
    ) -> Self {
        let metric_key = metric_key.into();
        let claim_id = id_of_body(&encode_body(
            &metric_key,
            canonical_value_tag,
            evidence,
            epistemic,
            state,
            &confidentiality,
            &parents,
            &anchors,
        ));
        Self {
            claim_id,
            metric_key,
            value,
            representation,
            support,
            additivity,
            evidence,
            epistemic,
            state,
            confidentiality,
            uu_terms,
            parents,
            anchors,
            supersedes: None,
            stale: false,
            calibration_failed: false,
            color_basis: None,
            value_tag: canonical_value_tag.to_string(),
        }
    }

    /// The value's canonical text, as hashed into the id.
    #[must_use]
    pub fn value_tag(&self) -> &str {
        &self.value_tag
    }

    /// The body `claim_id` is the BLAKE3 id of (`§8.1`): the `CLM1` frame, schema 2, over the metric key, value tag,
    /// evidence class, epistemic status, knowledge state, label, parents and anchors, in that order. Its bytes are
    /// pinned by known answers from `scripts/oracle/claim_body_known_answers.py`.
    ///
    /// Not in the body yet: representation, support, additivity, UU terms and `supersedes`. Two claims that differ
    /// only there share an id. `value` enters only through its tag, so writing `value` alone goes undetected.
    /// Adding any of them changes every claim id, and with them the public lane's state root (CONTRACT §2).
    #[must_use]
    pub fn canonical_body(&self) -> Vec<u8> {
        encode_body(
            &self.metric_key,
            &self.value_tag,
            self.evidence,
            self.epistemic,
            self.state,
            &self.confidentiality,
            &self.parents,
            &self.anchors,
        )
    }

    /// Refuses with `FMINE-REF-507` when `claim_id` is not the id of the current body, because a hashed field was
    /// written after construction. A claim is superseded, never edited (§8.1).
    pub fn check_id(&self) -> Result<(), Refusal> {
        let body_id = id_of_body(&self.canonical_body());
        if body_id == self.claim_id {
            return Ok(());
        }
        Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            format!(
                "claim '{}': its id {} names another body (the current body's id is {body_id}); a claim is \
                 superseded, never edited",
                self.metric_key, self.claim_id
            ),
        ))
    }

    /// The supersession path for evidence (§8.1, NL-2). The class becomes `self`'s class met with `to`, so it can
    /// only fall. When it does not change, the result is `self` unchanged. Otherwise the result has its own id, and
    /// its `supersedes` names `self`.
    #[must_use]
    pub fn downgraded(&self, to: EvidenceClass) -> Self
    where
        T: Clone,
    {
        let evidence = self.evidence.meet(to);
        let mut next = self.clone();
        if evidence == self.evidence {
            return next;
        }
        next.evidence = evidence;
        next.supersedes = Some(self.claim_id);
        next.claim_id = id_of_body(&next.canonical_body());
        next
    }

    /// States the regime and dataset this claim is anchored to. They count only for a class `§7.1` lets stand on an
    /// anchor ([`project_color`]), so this can never raise a colour by itself.
    #[must_use]
    pub fn with_regime(mut self, regime: ValidityDomain, dataset: impl Into<String>) -> Self {
        self.color_basis = Some(Color::Validated {
            regime,
            dataset: dataset.into(),
        });
        self
    }

    /// The claim's colour (`§7.1`, NL-1), or a refusal when it has none to show: failed calibration (NL-8), or an
    /// `Assertion`, which is refused as a colour leaf until reviewed (`FMINE-REF-205`, the use a colour serves).
    /// A stale claim is at most `Estimated`.
    pub fn color(&self) -> Result<Color, Refusal> {
        let refused = |why: &str| {
            Refusal::new(
                RefusalCode::CalibrationRequired205,
                format!("claim '{}': {why}", self.metric_key),
            )
        };
        if self.calibration_failed {
            return Err(refused("calibration failed"));
        }
        let color = project_color(self.evidence, self.color_basis.as_ref())
            .ok_or_else(|| refused("an unreviewed assertion has no colour"))?;
        if self.stale && color.rank() > fmine_evidence::color::ColorRank::Estimated {
            return Ok(Color::Estimated {
                estimator: "state:stale".to_string(),
                dispersion: f64::INFINITY,
            });
        }
        Ok(color)
    }

    /// Enforces `INV-007`: a reader holding `clearance` may see this claim iff its label is within the clearance
    /// ([`Confidentiality::leq`]). Otherwise it refuses with [`Confidentiality::refusal_over`]'s code:
    /// `FMINE-REF-301` for a wall the reader lacks, or for internal data on a public clearance (§31.1), and
    /// `FMINE-REF-302` for a venture the reader lacks (§31.2).
    pub fn verify_export_clearance(&self, clearance: &Confidentiality) -> Result<(), Refusal> {
        match self.confidentiality.refusal_over(clearance) {
            None => Ok(()),
            Some(code) => Err(Refusal::new(
                code,
                format!(
                    "claim '{}' is labelled {}; the reader's clearance is {clearance}",
                    self.metric_key, self.confidentiality
                ),
            )),
        }
    }

    /// Enforces `INV-015` (`G-UU`): refuses decision card emission if `epistemic != KnownKnown`
    /// and `uu_terms` is empty (`FMINE-REF-204`, unbudgeted unknowns).
    pub fn verify_uu_slot_for_decision(&self) -> Result<(), Refusal> {
        if self.epistemic != EpistemicStatus::KnownKnown && self.uu_terms.is_empty() {
            return Err(Refusal::new(
                RefusalCode::UnbudgetedUnknowns204,
                format!(
                    "claim '{}' has epistemic status {:?} but zero UuTerm scenarios",
                    self.metric_key, self.epistemic
                ),
            ));
        }
        Ok(())
    }
}

/// Verifies that two claims have identical `SpatialSupport` before direct comparison or combination (`FMINE-REF-102`).
pub fn verify_support_compatible(a: SpatialSupport, b: SpatialSupport) -> Result<(), Refusal> {
    if a != b {
        return Err(Refusal::new(
            RefusalCode::SupportMismatch102,
            format!("cannot combine supports {a:?} and {b:?} without change-of-support model"),
        ));
    }
    Ok(())
}

/// Verifies that an unweighted sum/mean is only applied to `ExtensiveAdditive` claims (`FMINE-REF-103`).
pub fn verify_unweighted_additive(
    additivity: AdditivityClass,
    context: &str,
) -> Result<(), Refusal> {
    if additivity != AdditivityClass::ExtensiveAdditive {
        return Err(Refusal::new(
            RefusalCode::NonAdditiveAverage103,
            format!("unweighted aggregation on {additivity:?} in {context}"),
        ));
    }
    Ok(())
}

/// Composes two parent claims `left` and `right` into a derived `Claim<R>` using `NL-1..NL-8` (`BD-042`).
///
/// - `evidence = left.evidence.meet(right.evidence)` (NL-2, never upgrades)
/// - `epistemic = left.epistemic.and(right.epistemic)`, `state = left.state.and(right.state)` (NL-5)
/// - the colour composes both parents' colours under `op` (NL-1); with either refused it stays refused (NL-6)
/// - `confidentiality = left.confidentiality.join(&right.confidentiality)` (`INV-007`)
/// - `stale = left.stale || right.stale` (`NL-7`)
/// - `calibration_failed = left.calibration_failed || right.calibration_failed` (`NL-8`)
/// - `uu_terms` = deduplicated union of `left.uu_terms` and `right.uu_terms` (`NL-5`)
/// - `parents` = `[left.claim_id, right.claim_id]`
// Nine arguments, each a distinct part of the composed Claim; a parameter struct would only rename them.
#[allow(clippy::too_many_arguments)]
pub fn compose_binary<A, B, R>(
    metric_key: impl Into<String>,
    left: &Claim<A>,
    right: &Claim<B>,
    value: R,
    canonical_value_tag: &str,
    representation: Representation,
    support: SpatialSupport,
    additivity: AdditivityClass,
    op: IntervalOp,
) -> Claim<R> {
    let evidence = left.evidence.meet(right.evidence);
    let epistemic = left.epistemic.and(right.epistemic);
    let state = left.state.and(right.state);
    let confidentiality = left.confidentiality.join(&right.confidentiality);
    let color_basis = match (left.color(), right.color()) {
        (Ok(a), Ok(b)) => Some(compose(&a, &b, op)),
        _ => None,
    };

    let mut uu_terms = left.uu_terms.clone();
    for u in &right.uu_terms {
        if !uu_terms
            .iter()
            .any(|existing| existing.scenario_id == u.scenario_id)
        {
            uu_terms.push(u.clone());
        }
    }

    let parents = vec![left.claim_id, right.claim_id];
    let mut anchors = left.anchors.clone();
    for a in &right.anchors {
        if !anchors.contains(a) {
            anchors.push(a.clone());
        }
    }

    let mut derived = Claim::new(
        metric_key,
        value,
        canonical_value_tag,
        representation,
        support,
        additivity,
        evidence,
        epistemic,
        state,
        confidentiality,
        uu_terms,
        parents,
        anchors,
    );
    derived.color_basis = color_basis;
    derived.stale = left.stale || right.stale;
    derived.calibration_failed = left.calibration_failed || right.calibration_failed;
    derived
}

// Eight arguments, each a hashed field of the claim; a parameter struct would only rename them.
#[allow(clippy::too_many_arguments)]
fn encode_body(
    metric_key: &str,
    canonical_value_tag: &str,
    evidence: EvidenceClass,
    epistemic: EpistemicStatus,
    state: KnowledgeState,
    confidentiality: &Confidentiality,
    parents: &[ClaimId],
    anchors: &[SourceAnchor],
) -> Vec<u8> {
    let mut enc = Encoder::new_frame(*b"CLM1", 2);
    enc.put_str(metric_key);
    enc.put_str(canonical_value_tag);
    enc.put_fixed(&evidence.tag());
    enc.put_fixed(&epistemic.tag());
    enc.put_u8(state.tag());
    confidentiality.encode(&mut enc);
    enc.put_u64(parents.len() as u64);
    for p in parents {
        enc.put_fixed(&p.bytes);
    }
    enc.put_u64(anchors.len() as u64);
    for a in anchors {
        enc.put_str(&a.artifact_path);
        enc.put_str(&a.locator);
        enc.put_fixed(&a.artifact_sha256);
    }
    enc.finish()
}

fn id_of_body(body: &[u8]) -> ClaimId {
    ObjectId::from_canonical_bytes(PrefixKind::Claim, body)
}

// --- Provenance DAG & Staleness Propagation (BD-046, INV-013) ---

/// Node metadata stored in `ClaimDag` for dependency tracking and staleness propagation (`BD-046`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagNode {
    pub claim_id: ClaimId,
    pub metric_key: String,
    pub evidence: EvidenceClass,
    pub parents: Vec<ClaimId>,
    pub stale: bool,
    pub superseded_by: Option<ClaimId>,
}

/// Directed acyclic provenance graph tracking parent -> child derivation edges (`BD-046`, `INV-013`).
#[derive(Debug, Default, Clone)]
pub struct ClaimDag {
    nodes: BTreeMap<ClaimId, DagNode>,
    children: BTreeMap<ClaimId, BTreeSet<ClaimId>>,
    /// DESIGN (INV-013): invalidation facts survive replay, including when the affected body has not arrived.
    invalidations: BTreeMap<ClaimId, Option<ClaimId>>,
}

impl ClaimDag {
    /// Creates an empty `ClaimDag`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a `Claim<T>` in the provenance DAG. Refuses (`FMINE-REF-507`, [`Claim::check_id`]) a claim whose id
    /// does not name its body, or which supersedes itself. Replaying a body cannot clear staleness. A changed
    /// identity's `supersedes` invalidates the old version and its dependants, without making the new version
    /// depend on the old one.
    pub fn insert<T>(&mut self, claim: &Claim<T>) -> Result<(), Refusal> {
        claim.check_id()?;
        if claim.supersedes == Some(claim.claim_id) {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                format!(
                    "claim '{}' cannot supersede its own identity",
                    claim.metric_key
                ),
            ));
        }
        let mut stale =
            claim.stale || claim.state == KnowledgeState::Stale || self.is_stale(claim.claim_id);
        for &p in &claim.parents {
            self.children.entry(p).or_default().insert(claim.claim_id);
            if self.is_stale(p) {
                stale = true;
            }
        }
        self.nodes.insert(
            claim.claim_id,
            DagNode {
                claim_id: claim.claim_id,
                metric_key: claim.metric_key.clone(),
                evidence: claim.evidence,
                parents: claim.parents.clone(),
                stale,
                superseded_by: self.invalidations.get(&claim.claim_id).copied().flatten(),
            },
        );
        if stale {
            self.propagate_staleness(claim.claim_id);
        }
        if let Some(previous) = claim.supersedes {
            self.invalidate_and_propagate(previous, Some(claim.claim_id));
        }
        Ok(())
    }

    /// G-DISC (`§8.3`), its evidence clauses: a disclosure-facing export needs a class in {Measured, Derived,
    /// ModelledCalibrated} and no `Assertion` or `Assumed` anywhere in its provenance. A parent this DAG does not hold
    /// proves nothing, so it refuses too. The body must still match its id (`FMINE-REF-507`), and both this claim
    /// and its ancestors must be current (`FMINE-REF-202`, INV-013). The supplied root need not be inserted.
    ///
    /// §8.4 has no G-DISC code, so `FMINE-REF-304` is the nearest: the clause this cannot yet check is the attested
    /// record. Passing is therefore necessary, not sufficient: the RefClass and attestation clauses wait on fields
    /// `Claim` does not have. FrankenMining's own public report is not a disclosure and does not call this.
    pub fn guard_disclosure_export<T>(&self, claim: &Claim<T>) -> Result<(), Refusal> {
        claim.check_id()?;
        let stale = |id: ClaimId| {
            Refusal::new(
                RefusalCode::NotObservable202,
                format!(
                    "G-DISC on '{}': claim {id} is stale; recompute from current dependencies",
                    claim.metric_key
                ),
            )
        };
        if claim.stale || claim.state == KnowledgeState::Stale || self.is_stale(claim.claim_id) {
            return Err(stale(claim.claim_id));
        }
        let refuse = |why: String| {
            Err(Refusal::new(
                RefusalCode::MissingHumanToken304,
                format!("G-DISC on '{}': {why}", claim.metric_key),
            ))
        };
        if !matches!(
            claim.evidence,
            EvidenceClass::Measured | EvidenceClass::Derived | EvidenceClass::ModelledCalibrated
        ) {
            return refuse(format!("class {:?}", claim.evidence));
        }
        let mut seen = BTreeSet::new();
        let mut queue: VecDeque<ClaimId> = claim.parents.iter().copied().collect();
        while let Some(id) = queue.pop_front() {
            if !seen.insert(id) {
                continue;
            }
            if self.is_stale(id) {
                return Err(stale(id));
            }
            let Some(node) = self.nodes.get(&id) else {
                return refuse(format!("parent {id} is not in this provenance graph"));
            };
            if matches!(
                node.evidence,
                EvidenceClass::Assumed | EvidenceClass::Assertion { .. }
            ) {
                return refuse(format!(
                    "'{}' in its provenance is {:?}",
                    node.metric_key, node.evidence
                ));
            }
            queue.extend(node.parents.iter().copied());
        }
        Ok(())
    }

    /// Marks `root_id` and all transitive downstream dependents as `stale = true` (`INV-013`).
    /// Returns the deterministic sorted list of all affected `ClaimId`s, including an absent root. Its
    /// invalidation remains effective when the body or a dependant arrives later. A repeated invalidation with
    /// no replacement does not erase a recorded supersession.
    pub fn invalidate_and_propagate(
        &mut self,
        root_id: ClaimId,
        superseded_by: Option<ClaimId>,
    ) -> Vec<ClaimId> {
        let replacement = self.invalidations.entry(root_id).or_default();
        if superseded_by.is_some() {
            *replacement = superseded_by;
        }
        if let Some(node) = self.nodes.get_mut(&root_id) {
            node.superseded_by = *replacement;
        }
        self.propagate_staleness(root_id)
    }

    fn propagate_staleness(&mut self, root_id: ClaimId) -> Vec<ClaimId> {
        let mut affected = BTreeSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(root_id);

        while let Some(curr) = queue.pop_front() {
            if !affected.insert(curr) {
                continue;
            }
            if let Some(node) = self.nodes.get_mut(&curr) {
                node.stale = true;
            }
            if let Some(kids) = self.children.get(&curr) {
                for &k in kids {
                    queue.push_back(k);
                }
            }
        }
        affected.into_iter().collect()
    }

    /// Returns whether `claim_id` is stale from an invalidation or a known dependency, even before its body arrives.
    #[must_use]
    pub fn is_stale(&self, claim_id: ClaimId) -> bool {
        self.invalidations.contains_key(&claim_id)
            || self.nodes.get(&claim_id).is_some_and(|n| n.stale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::rand::{PhiloxStream, RngKey};
    use fmine_evidence::color::ColorRank;
    use fmine_evidence::{AoaStatus, AssertionSource, KuKind};

    fn sample_anchor() -> SourceAnchor {
        SourceAnchor::new("synthetic/aisc.xlsx", [0x11; 32], "Synthetic AISC!L14")
    }

    fn regime() -> ValidityDomain {
        ValidityDomain::unconstrained().with("feed_grade", 1.0, 3.0)
    }

    /// A claim stating `class` itself and naming `parents`, whatever they are. Nothing in `Claim::new` checks that the
    /// two agree, which is why G-DISC walks the provenance.
    fn stating(key: &str, class: EvidenceClass, parents: Vec<ClaimId>) -> Claim<f64> {
        Claim::new(
            key,
            2.1,
            "2.1",
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            class,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::internal(),
            vec![],
            parents,
            vec![sample_anchor()],
        )
    }

    fn leaf(key: &str, class: EvidenceClass) -> Claim<f64> {
        stating(key, class, vec![])
    }

    fn join(key: &str, left: &Claim<f64>, right: &Claim<f64>) -> Claim<f64> {
        compose_binary(
            key,
            left,
            right,
            4.2,
            "4.2",
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            IntervalOp::Add,
        )
    }

    #[test]
    fn test_nl1_to_nl8_non_laundering_composition() {
        let reg = LabelRegistry::new(["ven_a"], []).unwrap();
        let ven_a = Confidentiality::venture(reg.venture("ven_a").unwrap());
        let c_measured = Claim::new(
            "tonnes_mined",
            1_000.0_f64,
            "1000.0",
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Measured,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![sample_anchor()],
        )
        .with_regime(regime(), "qaqc-2026");
        let uu = UuTerm {
            scenario_id: "UU-MET-01".to_string(),
            description: "Preg-robbing carbonaceous lens".to_string(),
            downside_exposure_usd_m: 12.5,
            trigger_indicator: "CIL tail solution grade > 0.15 g/t".to_string(),
        };
        let c_assumed = Claim::new(
            "recovery",
            0.85_f64,
            "0.85",
            Representation::Interval { lo: 0.78, hi: 0.90 },
            SpatialSupport::PeriodAggregate,
            AdditivityClass::IntensiveWeighted,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            ven_a.clone(),
            vec![uu],
            vec![],
            vec![sample_anchor()],
        );

        let derived = compose_binary(
            "recovered_metal",
            &c_measured,
            &c_assumed,
            850.0_f64,
            "850.0",
            Representation::Interval {
                lo: 780.0,
                hi: 900.0,
            },
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            IntervalOp::Mul,
        );

        // Weakest input class: Measured ∧ Assumed = Assumed (NL-2).
        assert_eq!(derived.evidence, EvidenceClass::Assumed);
        // Conjunction of states and of statuses (NL-5).
        assert_eq!(derived.state, KnowledgeState::Estimated);
        assert_eq!(
            derived.epistemic,
            EpistemicStatus::KnownUnknown(KuKind::Parametric)
        );
        // Confidentiality monotonically increases: Public ⊔ Venture(ven_a) = Venture(ven_a) (INV-007).
        assert_eq!(derived.confidentiality, ven_a);
        // UuTerm preserved (NL-5).
        assert_eq!(derived.uu_terms.len(), 1);
        // The Validated parent does not lift the Estimated one (NL-1).
        assert_eq!(
            c_measured.color().map(|c| c.rank()),
            Ok(ColorRank::Validated)
        );
        assert_eq!(derived.color().map(|c| c.rank()), Ok(ColorRank::Estimated));
    }

    #[test]
    fn test_with_regime_keeps_a_validated_basis_only_for_a_class_that_may_stand_on_one() {
        let stands = leaf("stands", EvidenceClass::Measured).with_regime(regime(), "qaqc-2026");
        assert_eq!(stands.color().map(|c| c.rank()), Ok(ColorRank::Validated));

        // Bare, a Measured claim has nothing to stand on yet.
        let bare = leaf("bare", EvidenceClass::Measured);
        assert_eq!(bare.color().map(|c| c.rank()), Ok(ColorRank::Estimated));

        // Planted: a caller supplies a regime for classes §7.1 keeps at Estimated. The colour must not rise.
        for class in [
            EvidenceClass::ModelledUncalibrated,
            EvidenceClass::Elicited,
            EvidenceClass::Assumed,
            EvidenceClass::Transferred {
                aoa: AoaStatus::Outside,
            },
        ] {
            let c = leaf("planted", class).with_regime(regime(), "qaqc-2026");
            assert_eq!(
                c.color().map(|c| c.rank()),
                Ok(ColorRank::Estimated),
                "{class:?}"
            );
        }
    }

    #[test]
    fn test_assertion_failed_calibration_and_staleness_never_show_a_strong_colour() {
        // An assertion has no colour, with or without a regime, and passes that refusal on (NL-6).
        let issuer = leaf("issuer", EvidenceClass::ISSUER).with_regime(regime(), "report-2025");
        let err = issuer.color().expect_err("assertion is no colour leaf");
        assert_eq!((err.code, err.exit_code), ("FMINE-REF-205", 3));
        let measured = leaf("measured", EvidenceClass::Measured).with_regime(regime(), "qaqc");
        assert!(join("child", &measured, &issuer).color().is_err());

        // NL-8: failed calibration refuses, whatever the class, and stays refused downstream.
        let mut failed = leaf("failed", EvidenceClass::Measured).with_regime(regime(), "qaqc");
        failed.calibration_failed = true;
        assert_eq!(failed.color().map_err(|e| e.code), Err("FMINE-REF-205"));
        assert!(join("child2", &failed, &measured).color().is_err());

        // NL-7: a stale claim keeps a colour, at most Estimated, and so does what it feeds.
        let mut stale = measured.clone();
        stale.stale = true;
        assert_eq!(stale.color().map(|c| c.rank()), Ok(ColorRank::Estimated));
        let child = join("child3", &stale, &measured);
        assert_eq!(child.color().map(|c| c.rank()), Ok(ColorRank::Estimated));
    }

    #[test]
    fn test_g_disc_refuses_assertion_in_provenance() {
        // Planted: `Derived` is stated over an issuer's figure. The class alone would pass.
        let root = leaf("issuer_root", EvidenceClass::ISSUER);
        let mid = stating("mid", EvidenceClass::Derived, vec![root.claim_id]);
        let top = stating("top", EvidenceClass::Derived, vec![mid.claim_id]);
        let mut dag = ClaimDag::new();
        for claim in [&root, &mid, &top] {
            assert_eq!(dag.insert(claim), Ok(()));
        }
        let err = dag
            .guard_disclosure_export(&top)
            .expect_err("an assertion two levels up must block the export");
        assert_eq!((err.code, err.exit_code), ("FMINE-REF-304", 7));
        assert!(err.what.contains("issuer_root"), "{}", err.what);
    }

    #[test]
    fn test_g_disc_refuses_assumed_in_provenance() {
        let root = leaf("assumed_root", EvidenceClass::Assumed);
        let top = stating("top", EvidenceClass::Measured, vec![root.claim_id]);
        let mut dag = ClaimDag::new();
        assert_eq!(dag.insert(&root), Ok(()));
        assert_eq!(dag.insert(&top), Ok(()));
        let err = dag
            .guard_disclosure_export(&top)
            .expect_err("an assumption in the provenance must block the export");
        assert!(err.what.contains("assumed_root"), "{}", err.what);
    }

    #[test]
    fn test_g_disc_refuses_a_weak_class_and_a_parent_it_cannot_see() {
        let dag = ClaimDag::new();
        for class in [
            EvidenceClass::ModelledUncalibrated,
            EvidenceClass::Elicited,
            EvidenceClass::Transferred {
                aoa: AoaStatus::Inside,
            },
            EvidenceClass::Assumed,
            EvidenceClass::Assertion {
                source: AssertionSource::Extraction,
            },
        ] {
            assert!(
                dag.guard_disclosure_export(&leaf("weak", class)).is_err(),
                "{class:?}"
            );
        }
        let missing = leaf("missing", EvidenceClass::Measured);
        let orphan = stating("orphan", EvidenceClass::Derived, vec![missing.claim_id]);
        assert!(dag.guard_disclosure_export(&orphan).is_err());
    }

    #[test]
    fn test_g_disc_accepts_measured_derived_chain() {
        let a = leaf("a", EvidenceClass::Measured);
        let b = leaf("b", EvidenceClass::Derived);
        let c = leaf("c", EvidenceClass::ModelledCalibrated);
        let ab = join("ab", &a, &b);
        let top = join("top", &ab, &c);
        assert_eq!(top.evidence, EvidenceClass::ModelledCalibrated);
        let mut dag = ClaimDag::new();
        for claim in [&a, &b, &c, &ab] {
            assert_eq!(dag.insert(claim), Ok(()));
        }
        // FACT: a fresh projection can be checked before it is stored; its ancestors must already be known.
        assert_eq!(dag.guard_disclosure_export(&top), Ok(()));
        assert_eq!(dag.insert(&top), Ok(()));
        assert_eq!(dag.guard_disclosure_export(&top), Ok(()));
    }

    #[test]
    fn test_g_disc_refuses_stale_claims_and_ancestors_before_root_insertion() {
        let root = leaf("current_deck", EvidenceClass::Measured);
        let mid = stating("cost", EvidenceClass::Derived, vec![root.claim_id]);
        let top = stating("projection", EvidenceClass::Derived, vec![mid.claim_id]);
        let mut dag = ClaimDag::new();
        dag.insert(&root).unwrap();
        dag.insert(&mid).unwrap();
        assert_eq!(dag.guard_disclosure_export(&top), Ok(()));
        dag.invalidate_and_propagate(root.claim_id, None);
        let err = dag.guard_disclosure_export(&top).unwrap_err();
        assert_eq!((err.code, err.exit_code), ("FMINE-REF-202", 3));
        assert!(err.what.contains(&mid.claim_id.to_string()));

        let mut stale = leaf("uninserted_stale", EvidenceClass::Measured);
        stale.stale = true;
        assert_eq!(
            ClaimDag::new()
                .guard_disclosure_export(&stale)
                .unwrap_err()
                .code,
            "FMINE-REF-202"
        );
        let mut absent = ClaimDag::new();
        absent.invalidate_and_propagate(root.claim_id, None);
        assert_eq!(
            absent.guard_disclosure_export(&root).unwrap_err().code,
            "FMINE-REF-202"
        );
    }

    /// PO-PLUMB-1 through `Claim` itself: random trees of 2 to 6 leaves, each pair joined by `compose_binary`. A node
    /// never outranks either parent in class, state or colour, and is refused exactly when some leaf under it is.
    #[test]
    fn test_random_claim_dags_nl1_nl2_nl5_100k() {
        const CASES: usize = 100_000;
        let key = RngKey {
            seed_root: 0x0009_D0C5_0040,
            partition: 2,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        };
        let mut rng = PhiloxStream::new(key);
        let classes = [
            EvidenceClass::Measured,
            EvidenceClass::Derived,
            EvidenceClass::ModelledCalibrated,
            EvidenceClass::ModelledUncalibrated,
            EvidenceClass::Elicited,
            EvidenceClass::Transferred {
                aoa: AoaStatus::Inside,
            },
            EvidenceClass::Transferred {
                aoa: AoaStatus::NotAssessed,
            },
            EvidenceClass::Assumed,
            EvidenceClass::ISSUER,
        ];
        let states = [
            KnowledgeState::Known,
            KnowledgeState::Estimated,
            KnowledgeState::Unknown,
            KnowledgeState::Conflicted,
            KnowledgeState::Redacted,
        ];
        let (mut violations, mut refused, mut validated) = (0_u32, 0_u32, 0_u32);
        for _ in 0..CASES {
            let leaves = 2 + (rng.next_u64() % 5) as usize;
            let mut pool: Vec<(Claim<f64>, Option<ColorRank>)> = (0..leaves)
                .map(|i| {
                    let class = classes[(rng.next_u64() % classes.len() as u64) as usize];
                    let mut claim = leaf(if i % 2 == 0 { "even" } else { "odd" }, class);
                    claim.state = states[(rng.next_u64() % states.len() as u64) as usize];
                    if rng.next_u64().is_multiple_of(2) {
                        claim = claim.with_regime(regime(), "qaqc");
                    }
                    let weakest = claim.color().ok().map(|c| c.rank());
                    (claim, weakest)
                })
                .collect();
            while pool.len() > 1 {
                let (l, l_weakest) =
                    pool.swap_remove((rng.next_u64() % pool.len() as u64) as usize);
                let (r, r_weakest) =
                    pool.swap_remove((rng.next_u64() % pool.len() as u64) as usize);
                let child = join("node", &l, &r);
                let weakest = l_weakest.zip(r_weakest).map(|(a, b)| a.min(b));
                let color = child.color().ok().map(|c| c.rank());
                let colour_holds = match (color, weakest) {
                    (None, None) => true,
                    (Some(c), Some(w)) => c <= w,
                    _ => false,
                };
                let class_holds = child.evidence.no_stronger_than(l.evidence)
                    && child.evidence.no_stronger_than(r.evidence);
                let state_holds = child.state.and(l.state) == child.state
                    && child.state.and(r.state) == child.state;
                if !(colour_holds && class_holds && state_holds) {
                    violations += 1;
                }
                match color {
                    None => refused += 1,
                    Some(ColorRank::Validated) => validated += 1,
                    Some(_) => {}
                }
                pool.push((child, weakest));
            }
        }
        println!(
            "{{\"test\":\"claim_dags_nl1_nl2_nl5\",\"cases\":{CASES},\"rng_key\":\"{:016x}\",\"violations\":{violations},\
             \"refused\":{refused},\"validated\":{validated}}}",
            key.seed_root
        );
        assert_eq!(violations, 0);
        // A run with no refused or Validated node would have proved nothing about either.
        assert!(refused > 0 && validated > 0);
    }

    fn labelled(key: &str, confidentiality: Confidentiality) -> Claim<f64> {
        Claim::new(
            key,
            42.0,
            "42.0",
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::ModelledCalibrated,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            confidentiality,
            vec![],
            vec![],
            vec![sample_anchor()],
        )
    }

    /// One export attempt, guarded and logged: the source labels, their join, the clearance, whether the join sits
    /// under it, and the decision.
    fn export_attempt(
        case: &str,
        sources: &[&Claim<f64>],
        derived: &Claim<f64>,
        clearance: &Confidentiality,
    ) -> Result<(), Refusal> {
        let verdict = derived.verify_export_clearance(clearance);
        let labels: Vec<String> = sources
            .iter()
            .map(|c| format!("\"{}\"", c.confidentiality))
            .collect();
        let code = verdict
            .as_ref()
            .err()
            .map_or("null".to_string(), |r| format!("\"{}\"", r.code));
        println!(
            "{{\"test\":\"export_guard\",\"case\":\"{case}\",\"sources\":[{}],\"join\":\"{}\",\
             \"clearance\":\"{clearance}\",\"leq\":{},\"decision\":\"{}\",\"code\":{code}}}",
            labels.join(","),
            derived.confidentiality,
            derived.confidentiality.leq(clearance),
            if verdict.is_ok() { "allow" } else { "refuse" },
        );
        verdict
    }

    #[test]
    fn test_confidentiality_lattice_and_export_guard() {
        let reg = LabelRegistry::new([], ["W-A", "W-B"]).unwrap();
        let wall_a = Confidentiality::mnpi(reg.wall("W-A").unwrap());
        let wall_b = Confidentiality::mnpi(reg.wall("W-B").unwrap());
        let a = labelled("royalty_model", wall_a.clone());
        let b = labelled("offtake_terms", wall_b.clone());
        let both = join("royalty_on_offtake", &a, &b);
        assert_eq!(both.confidentiality, wall_a.join(&wall_b));

        // A reader cleared behind wall A alone must not see a result that also carries wall B's facts.
        let err = export_attempt("two walls, cleared for one", &[&a, &b], &both, &wall_a)
            .expect_err("the second wall must refuse");
        // An authorization refusal (3xx, exit 7), never 402 (unknown PAG, a dump-safety code).
        assert_eq!((err.code, err.exit_code), ("FMINE-REF-301", 7));
        let public = export_attempt("wall to public", &[&a], &a, &Confidentiality::public())
            .expect_err("a walled claim must not go public");
        assert_eq!(public.code, "FMINE-REF-301");
        // Internal data to a public reader is the same refusal: no wall is involved, but nothing private may leak.
        let internal = labelled("site_cost", Confidentiality::internal());
        let leak = export_attempt(
            "internal to public",
            &[&internal],
            &internal,
            &Confidentiality::public(),
        )
        .expect_err("internal data must not go public");
        assert_eq!(leak.code, "FMINE-REF-301");

        assert!(
            export_attempt(
                "two walls, cleared for both",
                &[&a, &b],
                &both,
                &both.confidentiality
            )
            .is_ok()
        );
        assert!(export_attempt("one wall, cleared for it", &[&a], &a, &wall_a).is_ok());
    }

    #[test]
    fn test_two_venture_composition_refuses_a_single_venture_reader() {
        let reg = LabelRegistry::new(["ven_north", "ven_south"], []).unwrap();
        let north = Confidentiality::venture(reg.venture("ven_north").unwrap());
        let south = Confidentiality::venture(reg.venture("ven_south").unwrap());
        let north_grade = labelled("north_grade", north.clone());
        let north_tonnes = labelled("north_tonnes", north.clone());
        let south_tonnes = labelled("south_tonnes", south.clone());

        // Within one venture the composed label stays that venture's, and its partner may read it.
        let north_metal = join("north_metal", &north_grade, &north_tonnes);
        assert_eq!(north_metal.confidentiality, north);
        assert!(
            export_attempt(
                "one venture",
                &[&north_grade, &north_tonnes],
                &north_metal,
                &north
            )
            .is_ok()
        );

        // Across ventures the label joins both, and neither partner alone may read the result.
        let pooled = join("pooled_tonnes", &north_tonnes, &south_tonnes);
        assert_eq!(pooled.confidentiality, north.join(&south));
        for (case, clearance) in [("north reader", &north), ("south reader", &south)] {
            let err = export_attempt(case, &[&north_tonnes, &south_tonnes], &pooled, clearance)
                .expect_err("a single-venture reader must be refused the pooled result");
            assert_eq!((err.code, err.exit_code), ("FMINE-REF-302", 7));
        }
        // Group-internal staff cleared for both ventures may read it; bare internal clearance may not.
        let group = north.join(&south);
        assert!(
            export_attempt(
                "both ventures",
                &[&north_tonnes, &south_tonnes],
                &pooled,
                &group
            )
            .is_ok()
        );
        let err = export_attempt(
            "internal only",
            &[&north_tonnes, &south_tonnes],
            &pooled,
            &Confidentiality::internal(),
        )
        .expect_err("internal clearance names no venture");
        assert_eq!(err.code, "FMINE-REF-302");

        // One wall: the north venture's tonnes joined with a walled fact carry the wall, so the north partner, who is
        // not behind it, is refused at the wall (301) even though the venture matches.
        let walls = LabelRegistry::new([], ["W-EMBARGO"]).unwrap();
        let embargo = Confidentiality::mnpi(walls.wall("W-EMBARGO").unwrap());
        let embargoed = labelled("embargoed_plan", embargo.clone());
        let tainted = join("north_tonnes_vs_plan", &north_tonnes, &embargoed);
        assert_eq!(tainted.confidentiality, north.join(&embargo));
        let err = export_attempt(
            "across the wall",
            &[&north_tonnes, &embargoed],
            &tainted,
            &north,
        )
        .expect_err("the wall must refuse a reader outside it");
        assert_eq!((err.code, err.exit_code), ("FMINE-REF-301", 7));
        let inside = north.join(&embargo);
        assert!(
            export_attempt(
                "inside the wall",
                &[&north_tonnes, &embargoed],
                &tainted,
                &inside
            )
            .is_ok()
        );
    }

    #[test]
    fn test_support_and_additivity_guards() {
        let err_sup = verify_support_compatible(
            SpatialSupport::PointCore,
            SpatialSupport::SmuBlock {
                dx_cm: 1000,
                dy_cm: 1000,
                dz_cm: 500,
            },
        )
        .expect_err("must reject point vs SMU block support mismatch");
        assert_eq!((err_sup.code, err_sup.exit_code), ("FMINE-REF-102", 3));

        let err_add =
            verify_unweighted_additive(AdditivityClass::IntensiveWeighted, "bench mean grade")
                .expect_err("must reject unweighted average of intensive grade");
        assert_eq!((err_add.code, err_add.exit_code), ("FMINE-REF-103", 3));
    }

    #[test]
    fn test_claim_dag_staleness_propagation() {
        let mut dag = ClaimDag::new();
        let c1 = Claim::new(
            "assay_root",
            2.1_f64,
            "2.1",
            Representation::Deterministic,
            SpatialSupport::PointCore,
            AdditivityClass::IntensiveWeighted,
            EvidenceClass::Measured,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::internal(),
            vec![],
            vec![],
            vec![sample_anchor()],
        );
        let c2 = compose_binary(
            "composite_mid",
            &c1,
            &c1,
            2.1_f64,
            "2.1",
            Representation::Deterministic,
            SpatialSupport::Composite { length_mm: 2000 },
            AdditivityClass::IntensiveWeighted,
            IntervalOp::Hull,
        );
        let c3 = compose_binary(
            "block_leaf",
            &c2,
            &c2,
            2.1_f64,
            "2.1",
            Representation::Deterministic,
            SpatialSupport::SmuBlock {
                dx_cm: 1000,
                dy_cm: 1000,
                dz_cm: 500,
            },
            AdditivityClass::IntensiveWeighted,
            IntervalOp::Hull,
        );

        for claim in [&c1, &c2, &c3] {
            assert_eq!(dag.insert(claim), Ok(()));
        }
        assert!(!dag.is_stale(c3.claim_id));

        let affected = dag.invalidate_and_propagate(c1.claim_id, None);
        assert_eq!(affected.len(), 3);
        assert!(dag.is_stale(c1.claim_id));
        assert!(dag.is_stale(c2.claim_id));
        assert!(dag.is_stale(c3.claim_id));
    }

    #[test]
    fn test_claim_dag_invalidation_is_monotone_across_arrival_orders_and_replay() {
        let root = leaf("price_deck_v1", EvidenceClass::Measured);
        let mid = stating("metric_v1", EvidenceClass::Derived, vec![root.claim_id]);
        let top = stating("report_v1", EvidenceClass::Derived, vec![mid.claim_id]);
        let replacement = leaf("price_deck_v2", EvidenceClass::Measured);
        let claims = [&root, &mid, &top];
        let orders = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        for order in orders {
            for invalidation_step in 0..=claims.len() {
                let mut dag = ClaimDag::new();
                for step in 0..=claims.len() {
                    if step == invalidation_step {
                        dag.invalidate_and_propagate(root.claim_id, Some(replacement.claim_id));
                        assert!(dag.is_stale(root.claim_id));
                        dag.invalidate_and_propagate(root.claim_id, None);
                    }
                    if let Some(&index) = order.get(step) {
                        dag.insert(claims[index]).unwrap();
                    }
                }
                for claim in claims.into_iter().rev() {
                    assert!(
                        dag.is_stale(claim.claim_id),
                        "order={order:?}, step={invalidation_step}"
                    );
                    dag.insert(claim).unwrap();
                    assert!(
                        dag.is_stale(claim.claim_id),
                        "replay cannot revive a stale identity"
                    );
                }
                assert_eq!(
                    dag.nodes[&root.claim_id].superseded_by,
                    Some(replacement.claim_id)
                );
                assert_eq!(
                    dag.guard_disclosure_export(&top).unwrap_err().code,
                    "FMINE-REF-202"
                );
                dag.insert(&replacement).unwrap();
                assert!(!dag.is_stale(replacement.claim_id));
                assert_eq!(dag.guard_disclosure_export(&replacement), Ok(()));
            }
        }
    }

    #[test]
    fn test_claim_dag_late_stale_parent_invalidates_existing_descendants() {
        let mut root = leaf("late_deck", EvidenceClass::Measured);
        let mid = stating("late_metric", EvidenceClass::Derived, vec![root.claim_id]);
        let top = stating("late_report", EvidenceClass::Derived, vec![mid.claim_id]);
        let mut dag = ClaimDag::new();
        dag.insert(&top).unwrap();
        dag.insert(&mid).unwrap();
        assert!(!dag.is_stale(top.claim_id));
        root.stale = true;
        dag.insert(&root).unwrap();
        for claim in [&root, &mid, &top] {
            assert!(dag.is_stale(claim.claim_id));
        }
        root.stale = false;
        dag.insert(&root).unwrap();
        assert!(dag.is_stale(root.claim_id));
        assert!(dag.is_stale(top.claim_id));

        let stale_state = Claim::new(
            "stale_state",
            (),
            "stale",
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Measured,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Stale,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![],
        );
        assert_eq!(
            dag.guard_disclosure_export(&stale_state).unwrap_err().code,
            "FMINE-REF-202"
        );
        dag.insert(&stale_state).unwrap();
        assert!(dag.is_stale(stale_state.claim_id));
    }

    #[test]
    fn test_claim_dag_supersession_invalidates_old_dependants_without_staling_new_version() {
        let old = leaf("calibrated_grade", EvidenceClass::ModelledCalibrated);
        let child = stating("old_estimate", EvidenceClass::Derived, vec![old.claim_id]);
        let replacement = old.downgraded(EvidenceClass::ModelledUncalibrated);
        for replacement_first in [false, true] {
            let mut dag = ClaimDag::new();
            if replacement_first {
                dag.insert(&replacement).unwrap();
            }
            dag.insert(&child).unwrap();
            dag.insert(&old).unwrap();
            dag.insert(&replacement).unwrap();
            assert!(dag.is_stale(old.claim_id));
            assert!(dag.is_stale(child.claim_id));
            assert!(!dag.is_stale(replacement.claim_id));
            assert_eq!(
                dag.nodes[&old.claim_id].superseded_by,
                Some(replacement.claim_id)
            );
            let refreshed = stating(
                "new_estimate",
                EvidenceClass::Derived,
                vec![replacement.claim_id],
            );
            dag.insert(&refreshed).unwrap();
            assert!(!dag.is_stale(refreshed.claim_id));
            dag.insert(&old).unwrap();
            assert!(dag.is_stale(old.claim_id));
        }
    }

    #[test]
    fn test_claim_dag_rejects_self_supersession_without_mutation() {
        let root = leaf("root", EvidenceClass::Measured);
        let mut child = stating(
            "self_superseding",
            EvidenceClass::Derived,
            vec![root.claim_id],
        );
        child.supersedes = Some(child.claim_id);
        let mut dag = ClaimDag::new();
        dag.insert(&root).unwrap();
        let before = dag.clone();
        assert_eq!(dag.insert(&child).unwrap_err().code, "FMINE-REF-507");
        assert_eq!(dag.nodes, before.nodes);
        assert_eq!(dag.children, before.children);
        assert_eq!(dag.invalidations, before.invalidations);
    }

    /// The golden pair: an issuer's figure (public, one anchor) and a derived claim (venture joined with a wall, two
    /// parents, no anchor).
    fn golden_claims() -> (Claim<f64>, Claim<f64>) {
        let a = Claim::new(
            "aisc_usd_per_oz",
            1234.5,
            "1234.5",
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::IntensiveWeighted,
            EvidenceClass::ISSUER,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![sample_anchor()],
        );
        let reg = LabelRegistry::new(["ven_a"], ["W-1"]).unwrap();
        let label = Confidentiality::venture(reg.venture("ven_a").unwrap())
            .join(&Confidentiality::mnpi(reg.wall("W-1").unwrap()));
        let parents = [
            "clm_00112233445566778899aabbccddeeff",
            "clm_ffeeddccbbaa99887766554433221100",
        ]
        .map(|p| p.parse::<ClaimId>().unwrap());
        let b = Claim::new(
            "net_aisc_usd_m",
            -12.25,
            "-12.25",
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Derived,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            label,
            vec![],
            parents.to_vec(),
            vec![],
        );
        (a, b)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// BD-040: canonical bytes and ids, hand-built from the codec and BLAKE3 specs by
    /// `scripts/oracle/claim_body_known_answers.py` (its BLAKE3 port passes the official vectors first).
    #[test]
    fn test_canonical_body_and_claim_id_golden() {
        let (a, b) = golden_claims();
        assert_eq!(
            hex(&a.canonical_body()),
            "464d4e45434c4d3102000f000000616973635f7573645f7065725f6f7a06000000313233342e350702000000000000000000\
             0000000000000000000000000000000000000001000000000000001300000073796e7468657469632f616973632e786c7378\
             1200000053796e7468657469632041495343214c3134111111111111111111111111111111111111111111111111111111111\
             1111111b16cfd9a"
        );
        assert_eq!(
            a.claim_id.to_string(),
            "clm_581c1e44b9d7b72401cca56714637608"
        );
        assert_eq!(
            hex(&b.canonical_body()),
            "464d4e45434c4d3102000e0000006e65745f616973635f7573645f6d060000002d31322e3235010002000000000000000000\
             0000000000000000010501000000000000000500000076656e5f61010000000000000003000000572d310200000000000000\
             00112233445566778899aabbccddeeffffeeddccbbaa998877665544332211000000000000000000316f7b91"
        );
        assert_eq!(
            b.claim_id.to_string(),
            "clm_24ca9069e4b7290dc6790b4ef151c3c9"
        );
        assert_eq!((a.check_id(), b.check_id()), (Ok(()), Ok(())));
        assert_eq!(a.value_tag(), "1234.5");
    }

    /// A write to each hashed field after construction is refused by `check_id` and by `ClaimDag::insert`, and the
    /// DAG is left unchanged. Writes to the fields outside the body are not, which is the documented limit.
    #[test]
    fn test_edited_claim_is_refused_and_unhashed_fields_are_not() {
        type Edit = (&'static str, fn(&mut Claim<f64>));
        let (a, b) = golden_claims();
        let edits: [Edit; 7] = [
            ("metric_key", |c| c.metric_key.push('x')),
            ("evidence raised", |c| c.evidence = EvidenceClass::Measured),
            ("epistemic", |c| {
                c.epistemic = EpistemicStatus::UnmodelledBudget
            }),
            ("state", |c| c.state = KnowledgeState::Stale),
            ("label", |c| c.confidentiality = Confidentiality::internal()),
            ("parents", |c| c.parents.push(c.claim_id)),
            ("anchors", |c| c.anchors.clear()),
        ];
        for (what, edit) in edits {
            let mut edited = a.clone();
            edit(&mut edited);
            let err = edited.check_id().expect_err(what);
            assert_eq!((err.code, err.exit_code), ("FMINE-REF-507", 6), "{what}");
            assert!(
                err.what.contains("aisc_usd_per_oz") || what == "metric_key",
                "{}",
                err.what
            );
            let mut dag = ClaimDag::new();
            assert!(dag.insert(&edited).is_err(), "{what}");
            assert_eq!(
                dag.guard_disclosure_export(&edited).unwrap_err().code,
                "FMINE-REF-507",
                "{what}"
            );
            // Nothing was stored: a child of the refused claim finds its parent missing.
            let child = stating("child", EvidenceClass::Measured, vec![edited.claim_id]);
            assert_eq!(dag.insert(&child), Ok(()));
            let missing = dag.guard_disclosure_export(&child).expect_err(what);
            assert!(
                missing.what.contains("is not in this provenance graph"),
                "{what}: {}",
                missing.what
            );
        }
        let mut status = a.clone();
        status.stale = true;
        status.calibration_failed = true;
        status.supersedes = Some(b.claim_id);
        status.support = SpatialSupport::PointCore;
        assert_eq!(status.check_id(), Ok(()));
    }

    /// `downgraded` supersedes with a new id when the class falls, and returns the claim unchanged when the meet
    /// leaves the class where it was: it can never raise one.
    #[test]
    fn test_downgraded_supersedes_and_never_raises() {
        let calibrated = leaf("grade", EvidenceClass::ModelledCalibrated);
        let lowered = calibrated.downgraded(EvidenceClass::ModelledUncalibrated);
        assert_eq!(lowered.evidence, EvidenceClass::ModelledUncalibrated);
        assert_eq!(lowered.supersedes, Some(calibrated.claim_id));
        assert_ne!(lowered.claim_id, calibrated.claim_id);
        assert_eq!(lowered.check_id(), Ok(()));
        let mut dag = ClaimDag::new();
        assert_eq!(dag.insert(&lowered), Ok(()));

        let (issuer, _) = golden_claims();
        for to in [
            EvidenceClass::Measured,
            EvidenceClass::Derived,
            EvidenceClass::ModelledCalibrated,
        ] {
            assert_eq!(
                issuer.downgraded(to),
                issuer,
                "{to:?} must not raise an assertion"
            );
            assert_eq!(lowered.downgraded(to), lowered, "{to:?}");
        }
        // Incomparable classes meet below both (NL-2): Elicited met with Uncalibrated is Assumed.
        let elicited = leaf("judgement", EvidenceClass::Elicited);
        assert_eq!(
            elicited
                .downgraded(EvidenceClass::ModelledUncalibrated)
                .evidence,
            EvidenceClass::Assumed
        );
    }
}
