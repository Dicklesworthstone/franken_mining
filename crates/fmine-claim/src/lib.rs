//! Universal `Claim<T>` epistemic envelope (`§8.1`), non-laundering laws `NL-1..NL-8` (`GATE-PLUMB-E1`),
//! confidentiality lattice (`INV-007`), `UuTerm` guard (`INV-015`), spatial support / additivity
//! guards (`INV-014`), and provenance DAG staleness propagation (`INV-013`).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use fmine_core::anchor::SourceAnchor;
use fmine_core::codec::Encoder;
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::{ClaimColor, EvidenceClass, KnowledgeState, Refusal, project_color};

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

/// Confidentiality clearance lattice (`BD-043`, `INV-007`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfidentialityTier {
    /// Publicly filed disclosure (SEC/SEDAR+, public ESIA).
    Public = 0,
    /// Internal operational engineering data.
    Internal = 1,
    /// Joint-venture partner restricted (`NGM`, `Pueblo Viejo`, `Kibali`, `Reko Diq`).
    JvPartner = 2,
    /// Sovereign/host-government restricted geological or security data.
    SovereignRestricted = 3,
    /// Board / M&A / litigation privileged.
    BoardEyesOnly = 4,
}

impl ConfidentialityTier {
    /// Least upper bound (`max`) of two confidentiality tiers (`INV-007`).
    #[must_use]
    pub const fn combine(self, other: Self) -> Self {
        if (self as u8) >= (other as u8) {
            self
        } else {
            other
        }
    }
}

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
#[derive(Debug, Clone, PartialEq)]
pub struct Claim<T> {
    pub claim_id: ClaimId,
    pub metric_key: String,
    pub value: T,
    pub representation: Representation,
    pub support: SpatialSupport,
    pub additivity: AdditivityClass,
    pub evidence: EvidenceClass,
    pub knowledge: KnowledgeState,
    pub confidentiality: ConfidentialityTier,
    pub uu_terms: Vec<UuTerm>,
    pub parents: Vec<ClaimId>,
    pub anchors: Vec<SourceAnchor>,
    pub supersedes: Option<ClaimId>,
    pub stale: bool,
    pub calibration_failed: bool,
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
        knowledge: KnowledgeState,
        confidentiality: ConfidentialityTier,
        uu_terms: Vec<UuTerm>,
        parents: Vec<ClaimId>,
        anchors: Vec<SourceAnchor>,
    ) -> Self {
        let metric_key = metric_key.into();
        let claim_id = compute_claim_id(
            &metric_key,
            canonical_value_tag,
            evidence,
            knowledge,
            confidentiality,
            &parents,
            &anchors,
        );
        Self {
            claim_id,
            metric_key,
            value,
            representation,
            support,
            additivity,
            evidence,
            knowledge,
            confidentiality,
            uu_terms,
            parents,
            anchors,
            supersedes: None,
            stale: false,
            calibration_failed: false,
        }
    }

    /// Projects the claim's executive traffic-light color (`Green`, `Amber`, `Red`)
    /// enforcing non-laundering laws `NL-4`, `NL-5`, `NL-7`, `NL-8`.
    #[must_use]
    pub fn color(&self) -> ClaimColor {
        let uu_material = self
            .uu_terms
            .iter()
            .any(|u| u.downside_exposure_usd_m > 0.0);
        project_color(
            self.evidence,
            self.knowledge,
            self.calibration_failed,
            self.stale,
            uu_material,
        )
    }

    /// Enforces `INV-007`: refuses export if `self.confidentiality > clearance` (`FMINE-REF-402`).
    pub fn verify_export_clearance(&self, clearance: ConfidentialityTier) -> Result<(), Refusal> {
        if self.confidentiality > clearance {
            return Err(Refusal::from_code(
                "FMINE-REF-402",
                format!(
                    "claim '{}' has confidentiality {:?} > clearance {:?}",
                    self.metric_key, self.confidentiality, clearance
                ),
            ));
        }
        Ok(())
    }

    /// Enforces `INV-015` (`G-UU`): refuses decision card emission if `knowledge != KnownKnown`
    /// and `uu_terms` is empty (`FMINE-REF-404`).
    pub fn verify_uu_slot_for_decision(&self) -> Result<(), Refusal> {
        if self.knowledge != KnowledgeState::KnownKnown && self.uu_terms.is_empty() {
            return Err(Refusal::from_code(
                "FMINE-REF-404",
                format!(
                    "claim '{}' has knowledge state {:?} but zero UuTerm scenarios",
                    self.metric_key, self.knowledge
                ),
            ));
        }
        Ok(())
    }

    /// Enforces `NL-4` (`FMINE-REF-401`): refuses if a caller demands `ClaimColor::Green`
    /// on a `Placeholder` or failed-calibration claim.
    pub fn require_decision_grade(&self) -> Result<ClaimColor, Refusal> {
        let c = self.color();
        if c == ClaimColor::Red {
            return Err(Refusal::from_code(
                "FMINE-REF-401",
                format!(
                    "claim '{}' is Red (evidence={:?}, calibration_failed={})",
                    self.metric_key, self.evidence, self.calibration_failed
                ),
            ));
        }
        Ok(c)
    }
}

/// Verifies that two claims have identical `SpatialSupport` before direct comparison or combination (`FMINE-REF-102`).
pub fn verify_support_compatible(a: SpatialSupport, b: SpatialSupport) -> Result<(), Refusal> {
    if a != b {
        return Err(Refusal::from_code(
            "FMINE-REF-102",
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
        return Err(Refusal::from_code(
            "FMINE-REF-103",
            format!("unweighted aggregation on {additivity:?} in {context}"),
        ));
    }
    Ok(())
}

/// Composes two parent claims `left` and `right` into a derived `Claim<R>` using `NL-1..NL-8` (`BD-042`).
///
/// - `evidence = left.evidence.combine(right.evidence)` (weakest-link, never upgrades)
/// - `knowledge = left.knowledge.combine(right.knowledge)`
/// - `confidentiality = left.confidentiality.combine(right.confidentiality)` (`INV-007`)
/// - `stale = left.stale || right.stale` (`NL-7`)
/// - `calibration_failed = left.calibration_failed || right.calibration_failed` (`NL-8`)
/// - `uu_terms` = deduplicated union of `left.uu_terms` and `right.uu_terms` (`NL-5`)
/// - `parents` = `[left.claim_id, right.claim_id]`
pub fn compose_binary<A, B, R>(
    metric_key: impl Into<String>,
    left: &Claim<A>,
    right: &Claim<B>,
    value: R,
    canonical_value_tag: &str,
    representation: Representation,
    support: SpatialSupport,
    additivity: AdditivityClass,
) -> Claim<R> {
    let evidence = left.evidence.combine(right.evidence);
    let knowledge = left.knowledge.combine(right.knowledge);
    let confidentiality = left.confidentiality.combine(right.confidentiality);

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
        knowledge,
        confidentiality,
        uu_terms,
        parents,
        anchors,
    );
    derived.stale = left.stale || right.stale;
    derived.calibration_failed = left.calibration_failed || right.calibration_failed;
    derived
}

fn compute_claim_id(
    metric_key: &str,
    canonical_value_tag: &str,
    evidence: EvidenceClass,
    knowledge: KnowledgeState,
    confidentiality: ConfidentialityTier,
    parents: &[ClaimId],
    anchors: &[SourceAnchor],
) -> ClaimId {
    let mut enc = Encoder::new_frame(*b"CLM1", 1);
    enc.put_str(metric_key);
    enc.put_str(canonical_value_tag);
    enc.put_u8(evidence as u8);
    enc.put_u8(knowledge as u8);
    enc.put_u8(confidentiality as u8);
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
    ObjectId::from_canonical_bytes(PrefixKind::Claim, &enc.finish())
}

// --- Provenance DAG & Staleness Propagation (BD-046, INV-013) ---

/// Node metadata stored in `ClaimDag` for dependency tracking and staleness propagation (`BD-046`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagNode {
    pub claim_id: ClaimId,
    pub metric_key: String,
    pub parents: Vec<ClaimId>,
    pub stale: bool,
    pub superseded_by: Option<ClaimId>,
}

/// Directed acyclic provenance graph tracking parent -> child derivation edges (`BD-046`, `INV-013`).
#[derive(Debug, Default, Clone)]
pub struct ClaimDag {
    nodes: BTreeMap<ClaimId, DagNode>,
    children: BTreeMap<ClaimId, BTreeSet<ClaimId>>,
}

impl ClaimDag {
    /// Creates an empty `ClaimDag`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a `Claim<T>` in the provenance DAG.
    pub fn insert<T>(&mut self, claim: &Claim<T>) {
        let mut stale = claim.stale;
        for &p in &claim.parents {
            self.children.entry(p).or_default().insert(claim.claim_id);
            if self.nodes.get(&p).is_some_and(|pn| pn.stale) {
                stale = true;
            }
        }
        self.nodes.insert(
            claim.claim_id,
            DagNode {
                claim_id: claim.claim_id,
                metric_key: claim.metric_key.clone(),
                parents: claim.parents.clone(),
                stale,
                superseded_by: None,
            },
        );
    }

    /// Marks `root_id` and all transitive downstream dependents as `stale = true` (`INV-013`).
    /// Returns the deterministic sorted list of all affected `ClaimId`s.
    pub fn invalidate_and_propagate(
        &mut self,
        root_id: ClaimId,
        superseded_by: Option<ClaimId>,
    ) -> Vec<ClaimId> {
        let mut affected = BTreeSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(root_id);

        if let Some(node) = self.nodes.get_mut(&root_id) {
            node.superseded_by = superseded_by;
        }

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

    /// Returns whether `claim_id` is currently marked stale in the DAG.
    #[must_use]
    pub fn is_stale(&self, claim_id: ClaimId) -> bool {
        self.nodes.get(&claim_id).is_some_and(|n| n.stale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_anchor() -> SourceAnchor {
        SourceAnchor::new("synthetic/aisc.xlsx", [0x11; 32], "Synthetic AISC!L14")
    }

    #[test]
    fn test_nl1_to_nl8_non_laundering_composition() {
        let c_exact = Claim::new(
            "tonnes_mined",
            1_000.0_f64,
            "1000.0",
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Exact,
            KnowledgeState::KnownKnown,
            ConfidentialityTier::Public,
            vec![],
            vec![],
            vec![sample_anchor()],
        );
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
            KnowledgeState::KnownUnknown,
            ConfidentialityTier::JvPartner,
            vec![uu],
            vec![],
            vec![sample_anchor()],
        );

        let derived = compose_binary(
            "recovered_metal",
            &c_exact,
            &c_assumed,
            850.0_f64,
            "850.0",
            Representation::Interval {
                lo: 780.0,
                hi: 900.0,
            },
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
        );

        // Weakest-link EvidenceClass: Exact ∧ Assumed = Assumed (NL-6).
        assert_eq!(derived.evidence, EvidenceClass::Assumed);
        // Weakest-link KnowledgeState: KK ∧ KU = KU.
        assert_eq!(derived.knowledge, KnowledgeState::KnownUnknown);
        // Confidentiality monotonically increases: max(Public, JvPartner) = JvPartner (INV-007).
        assert_eq!(derived.confidentiality, ConfidentialityTier::JvPartner);
        // UuTerm preserved (NL-5).
        assert_eq!(derived.uu_terms.len(), 1);
        // Color is Amber (never Green when Assumed or UU present).
        assert_eq!(derived.color(), ClaimColor::Amber);
    }

    #[test]
    fn test_confidentiality_lattice_and_export_guard() {
        let claim = Claim::new(
            "sovereign_royalty_model",
            42.0_f64,
            "42.0",
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Calibrated,
            KnowledgeState::KnownKnown,
            ConfidentialityTier::SovereignRestricted,
            vec![],
            vec![],
            vec![sample_anchor()],
        );
        let err = claim
            .verify_export_clearance(ConfidentialityTier::Public)
            .expect_err("must block public export of SovereignRestricted claim");
        assert_eq!(err.code, "FMINE-REF-402");
        assert!(
            claim
                .verify_export_clearance(ConfidentialityTier::BoardEyesOnly)
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
        assert_eq!(err_sup.code, "FMINE-REF-102");

        let err_add =
            verify_unweighted_additive(AdditivityClass::IntensiveWeighted, "bench mean grade")
                .expect_err("must reject unweighted average of intensive grade");
        assert_eq!(err_add.code, "FMINE-REF-103");
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
            EvidenceClass::Exact,
            KnowledgeState::KnownKnown,
            ConfidentialityTier::Internal,
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
        );

        dag.insert(&c1);
        dag.insert(&c2);
        dag.insert(&c3);
        assert!(!dag.is_stale(c3.claim_id));

        let affected = dag.invalidate_and_propagate(c1.claim_id, None);
        assert_eq!(affected.len(), 3);
        assert!(dag.is_stale(c1.claim_id));
        assert!(dag.is_stale(c2.claim_id));
        assert!(dag.is_stale(c3.claim_id));
    }
}
