//! Universal `Claim<T>` epistemic envelope (`§8.1`), non-laundering laws `NL-1..NL-8` (`GATE-PLUMB-E1`),
//! confidentiality lattice (`INV-007`), `UuTerm` guard (`INV-015`), spatial support / additivity
//! guards (`INV-014`), and provenance DAG staleness propagation (`INV-013`).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use fmine_core::anchor::SourceAnchor;
use fmine_core::codec::Encoder;
use fmine_core::hash::Digest32;
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::color::{IntervalOp, ValidityDomain, compose};
use fmine_evidence::{
    AoaStatus, Color, EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode,
    project_color,
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
pub mod forecast_journal;
pub mod inspection;
mod label;
pub mod origin;
pub mod refclass;
pub mod reliance;
pub mod render;
pub mod representation;

pub use label::{Confidentiality, LabelError, LabelRegistry, VentureId, WallId};
pub use refclass::{
    EpsilonContamination, RefClassId, RefClassRecord, RefClassRegistry, RefClassStatus,
};

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

/// Orthogonal authority metadata for a [`Claim`] (`§7.3`, `NL-3`, `BD-042`).
///
/// Signatures and attestations live strictly on `authority`; they NEVER upgrade `evidence`,
/// `color`, `epistemic`, `state`, or `calibration_score`. Roles use institutional titles rather
/// than personal names to keep personal data out of the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimAuthority {
    /// Produced algorithmically without human sign-off (default).
    Computed { module: String, version: String },
    /// Structured expert judgment elicited under a named protocol (e.g. `IDEA`).
    Elicited {
        expert_role: String,
        protocol: String,
    },
    /// Signed by a human authority role (`CP`, `QP`, `EoR`, `controller`, `ITRB`).
    /// Strictly orthogonal to epistemic strength (`NL-3`).
    Attested {
        role: String,
        sig_digest: Digest32,
        scope: String,
    },
}

impl Default for ClaimAuthority {
    fn default() -> Self {
        Self::Computed {
            module: "fmine-claim".to_string(),
            version: "0.1.0".to_string(),
        }
    }
}

/// Declared deterministic fallback linkage for `NL-6` / `NL-8` / `PO-PLUMB-15`.
///
/// Refusals remain absorbing unless an active, named fallback mechanism (e.g. MapMark4-degenerate
/// mode or Manski bounds) is explicitly declared and recorded on the derived claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredFallback {
    pub fallback_id: String,
    pub mechanism: String,
    pub fallback_active: bool,
}

/// Computed family calibration score (`§7.3`, `NL-8`, `BD-042`).
///
/// Never hand-entered: fields are private so callers can obtain a `CalibrationScore` ONLY through
/// [`CalibrationScore::compute`], which evaluates `1 − min(1, max(|cov80 − 0.8| / 0.2, ks_pit))`
/// and forces `0.0` whenever `calibrated == false`.
///
/// ```compile_fail,E0451
/// let _forged = fmine_claim::CalibrationScore {
///     score: 0.99,
///     cov80: 0.80,
///     ks_pit: 0.01,
///     calibrated: true,
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationScore {
    score: f64,
    cov80: f64,
    ks_pit: f64,
    calibrated: bool,
}

impl CalibrationScore {
    /// Compute the `NL-8` calibration score from empirical 80% coverage (`cov80 ∈ [0, 1]`) and
    /// two-sided PIT Kolmogorov–Smirnov distance (`ks_pit ∈ [0, 1]`).
    ///
    /// Returns `score = 0.0` strictly when `calibrated == false` (`Uncalibrated`). Refuses any
    /// non-finite or out-of-`[0, 1]` input with `FMINE-REF-109` (no silent clamping).
    pub fn compute(calibrated: bool, cov80: f64, ks_pit: f64) -> Result<Self, Refusal> {
        if !cov80.is_finite() || !(0.0..=1.0).contains(&cov80) {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("NL-8 calibration_score: cov80 ({cov80}) must be finite and in [0, 1]"),
            ));
        }
        if !ks_pit.is_finite() || !(0.0..=1.0).contains(&ks_pit) {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("NL-8 calibration_score: ks_pit ({ks_pit}) must be finite and in [0, 1]"),
            ));
        }
        let score = if calibrated {
            let cov_err = (cov80 - 0.8).abs() / 0.2;
            let penalty = cov_err.max(ks_pit).min(1.0);
            (1.0 - penalty).max(0.0)
        } else {
            0.0
        };
        Ok(Self {
            score,
            cov80,
            ks_pit,
            calibrated,
        })
    }

    /// Computed score in `[0.0, 1.0]` (`0.0` whenever uncalibrated).
    #[must_use]
    pub const fn score(&self) -> f64 {
        self.score
    }

    /// Empirical 80% coverage supplied to [`Self::compute`].
    #[must_use]
    pub const fn cov80(&self) -> f64 {
        self.cov80
    }

    /// Two-sided PIT KS distance supplied to [`Self::compute`].
    #[must_use]
    pub const fn ks_pit(&self) -> f64 {
        self.ks_pit
    }

    /// Whether the underlying claim family holds a passing calibration status.
    #[must_use]
    pub const fn is_calibrated(&self) -> bool {
        self.calibrated
    }
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
    /// Private so the label can only rise after construction (`INV-007`): read it with
    /// [`Claim::confidentiality`], raise it with [`Claim::taint`].
    confidentiality: Confidentiality,
    pub uu_terms: Vec<UuTerm>,
    pub parents: Vec<ClaimId>,
    pub anchors: Vec<SourceAnchor>,
    pub supersedes: Option<ClaimId>,
    pub stale: bool,
    pub calibration_failed: bool,
    /// Orthogonal authority/attestation metadata (`NL-3`). Unhashed so pinned public claim IDs stay invariant.
    authority: ClaimAuthority,
    /// Computed `NL-8` calibration score record (private so callers cannot hand-enter a score).
    calibration_score: Option<CalibrationScore>,
    /// Declared deterministic fallback recorded when `NL-6` fallback mode was engaged.
    fallback: Option<DeclaredFallback>,
    /// Checked Huber ε-contamination attachment (`§9.3`, `GATE-PLUMB-E4`, `BD-050`).
    /// Unhashed outside `CLM1` schema 2 so pinned M0 public-lane claim IDs stay invariant.
    epsilon_contamination: EpsilonContamination,
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
            authority: ClaimAuthority::default(),
            calibration_score: None,
            fallback: None,
            epsilon_contamination: EpsilonContamination::none(),
            color_basis: None,
            value_tag: canonical_value_tag.to_string(),
        }
    }

    /// The claim's confidentiality label (§31.1).
    #[must_use]
    pub fn confidentiality(&self) -> &Confidentiality {
        &self.confidentiality
    }

    /// Raises the confidentiality label (`INV-007`, §31.1): the label becomes its join with `by`, so it can only
    /// rise. When it does not change, nothing does. Otherwise the claim takes a new id, because the label is hashed
    /// into the body, and `supersedes` names the id it had (§8.1: a claim is superseded, never edited).
    pub fn taint(&mut self, by: &Confidentiality) {
        let label = self.confidentiality.join(by);
        if label == self.confidentiality {
            return;
        }
        self.confidentiality = label;
        self.supersedes = Some(self.claim_id);
        self.claim_id = id_of_body(&self.canonical_body());
    }

    /// Returns the orthogonal authority/attestation metadata (`NL-3`).
    #[must_use]
    pub fn authority(&self) -> &ClaimAuthority {
        &self.authority
    }

    /// Sets orthogonal authority metadata (`NL-3`). Does not alter `evidence`, `color`, `epistemic`,
    /// `state`, `confidentiality`, `calibration_failed`, or `canonical_body`.
    #[must_use]
    pub fn with_authority(mut self, authority: ClaimAuthority) -> Self {
        self.authority = authority;
        self
    }

    /// Attaches an institutional attestation signature (`CP`, `QP`, `EoR`, `controller`, `ITRB`)
    /// strictly on `authority` (`NL-3`). Refuses blank `role`/`scope` or all-zero `sig_digest`
    /// with `FMINE-REF-304`. Never upgrades `evidence`, `color`, `epistemic`, `state`, or
    /// `calibration_score`.
    pub fn attest(
        &self,
        role: impl Into<String>,
        sig_digest: Digest32,
        scope: impl Into<String>,
    ) -> Result<Self, Refusal>
    where
        T: Clone,
    {
        let role = role.into();
        let scope = scope.into();
        if role.trim().is_empty() || scope.trim().is_empty() || sig_digest == Digest32([0u8; 32]) {
            return Err(Refusal::new(
                RefusalCode::MissingHumanToken304,
                "NL-3 attestation requires non-empty role, scope, and non-zero signature digest",
            ));
        }
        let mut next = self.clone();
        next.authority = ClaimAuthority::Attested {
            role,
            sig_digest,
            scope,
        };
        Ok(next)
    }

    /// Attaches a computed [`CalibrationScore`] to a parent-free claim (`NL-8`).
    ///
    /// Derived claims (`!parents.is_empty()`) ignore external calibration scores to enforce `NL-4`
    /// (calibration is never inherited across derivation without a new family's own resolved record).
    #[must_use]
    pub fn with_calibration_score(mut self, score: CalibrationScore) -> Self {
        if self.parents.is_empty() {
            self.calibration_score = Some(score);
        }
        self
    }

    /// Returns the claim's computed `NL-8` calibration score in `[0.0, 1.0]`.
    ///
    /// Strictly returns `0.0` when the claim has no score, is uncalibrated, failed calibration, is
    /// stale, has an evidence class below `ModelledCalibrated`, or has an active fallback.
    #[must_use]
    pub fn calibration_score(&self) -> f64 {
        if self.calibration_failed
            || self.stale
            || self.fallback_active()
            || !matches!(
                self.evidence,
                EvidenceClass::Measured
                    | EvidenceClass::Derived
                    | EvidenceClass::ModelledCalibrated
            )
        {
            return 0.0;
        }
        self.calibration_score.map_or(0.0, |record| record.score())
    }

    /// Returns the declared fallback record if a fallback was attached (`NL-6`, `PO-PLUMB-15`).
    #[must_use]
    pub fn fallback(&self) -> Option<&DeclaredFallback> {
        self.fallback.as_ref()
    }

    /// Mirrors `FallbackPolicy::should_fallback` (`NL-8`): true iff an active declared fallback was
    /// engaged on this claim or propagated from an input.
    #[must_use]
    pub fn fallback_active(&self) -> bool {
        self.fallback.as_ref().is_some_and(|f| f.fallback_active)
    }

    /// Returns the checked Huber ε-contamination attachment (`§9.3`, `GATE-PLUMB-E4`).
    #[must_use]
    pub fn epsilon_contamination(&self) -> &EpsilonContamination {
        &self.epsilon_contamination
    }

    /// Attaches a pre-checked [`EpsilonContamination`] (`§9.3`, `GATE-PLUMB-E4`).
    #[must_use]
    pub fn with_epsilon_contamination(mut self, contamination: EpsilonContamination) -> Self {
        self.epsilon_contamination = contamination;
        self
    }

    /// Attaches a checked ε-contamination weight, refusing `epsilon_bps > 0` without a
    /// [`RefClassRecord`] (`FMINE-REF-205`) or `epsilon_bps > 10_000` (`FMINE-REF-109`).
    pub fn with_checked_epsilon(
        mut self,
        ref_class: Option<RefClassRecord>,
        epsilon_bps: u16,
    ) -> Result<Self, Refusal> {
        self.epsilon_contamination = EpsilonContamination::try_new(ref_class, epsilon_bps)?;
        Ok(self)
    }

    /// Attaches a registered `REFCLASS-` row from [`RefClassRegistry`] (`GATE-PLUMB-E4`).
    /// Refuses unknown IDs with `FMINE-REF-201` and out-of-range overrides with `FMINE-REF-109`.
    pub fn with_ref_class(
        mut self,
        registry: &RefClassRegistry,
        raw_id: &str,
        override_epsilon_bps: Option<u16>,
    ) -> Result<Self, Refusal> {
        self.epsilon_contamination =
            EpsilonContamination::from_registry(registry, raw_id, override_epsilon_bps)?;
        Ok(self)
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

    /// States the regime and dataset of a source claim. The evidence class still caps the colour
    /// ([`project_color`]). A claim with parents keeps its existing basis so a source anchor cannot
    /// replace the colour, dataset lineage, or validity intersection inherited from its inputs.
    #[must_use]
    pub fn with_regime(mut self, regime: ValidityDomain, dataset: impl Into<String>) -> Self {
        if self.parents.is_empty() {
            self.color_basis = Some(Color::Validated {
                regime,
                dataset: dataset.into(),
            });
        }
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
        let color = project_color(self.evidence, self.color_basis.as_ref()).ok_or_else(|| {
            if matches!(self.evidence, EvidenceClass::Assertion { .. }) {
                refused("an unreviewed assertion has no colour")
            } else {
                refused("invalid colour basis: check the enclosure, calibration domain, dataset or spread")
            }
        })?;
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
/// - evidence is the parents' meet (NL-2), with model calibration removed and an in-AOA transfer
///   marked `NotAssessed`: this new family has no own calibration or applicability record (NL-4)
/// - `authority` resets to `ClaimAuthority::default()` (`Computed`) — parent attestations never
///   transfer automatically to a new derived claim (NL-3)
/// - `epistemic = left.epistemic.and(right.epistemic)`, `state = left.state.and(right.state)` (NL-5)
/// - the colour composes both parents' colours under `op` (NL-1); with either refused it stays refused (NL-6)
/// - `confidentiality = left.confidentiality.join(&right.confidentiality)` (`NL-7`, `INV-007`)
/// - `stale = left.stale || right.stale` (`INV-013`)
/// - `calibration_failed = left.calibration_failed || right.calibration_failed` and `calibration_score = None` (`NL-4`/`NL-8`)
/// - `fallback` propagates if either parent already ran under an active declared fallback (`NL-6`/`NL-8`)
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
    let evidence = match left.evidence.meet(right.evidence) {
        EvidenceClass::ModelledCalibrated => EvidenceClass::ModelledUncalibrated,
        EvidenceClass::Transferred {
            aoa: AoaStatus::Inside,
        } => EvidenceClass::Transferred {
            aoa: AoaStatus::NotAssessed,
        },
        other => other,
    };
    let epistemic = left.epistemic.and(right.epistemic);
    let state = left.state.and(right.state);
    let confidentiality = left.confidentiality.join(&right.confidentiality);
    let left_color = left.color();
    let right_color = right.color();
    let invalid_calibration = (left_color.is_err()
        && !matches!(left.evidence, EvidenceClass::Assertion { .. }))
        || (right_color.is_err() && !matches!(right.evidence, EvidenceClass::Assertion { .. }));
    let color_basis = left_color
        .ok()
        .zip(right_color.ok())
        .map(|(a, b)| compose(&a, &b, op));

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
    derived.calibration_failed =
        left.calibration_failed || right.calibration_failed || invalid_calibration;
    derived.fallback = left
        .fallback
        .as_ref()
        .filter(|f| f.fallback_active)
        .or_else(|| right.fallback.as_ref().filter(|f| f.fallback_active))
        .cloned();
    // GATE-PLUMB-E4: propagate Unverified reference-class taint so composition never launders it.
    if left.epsilon_contamination.status() == Some(RefClassStatus::Unverified) {
        derived.epsilon_contamination = left.epsilon_contamination.clone();
    } else if right.epsilon_contamination.status() == Some(RefClassStatus::Unverified) {
        derived.epsilon_contamination = right.epsilon_contamination.clone();
    }
    derived
}

/// Composes two parent claims with an explicit [`DeclaredFallback`] (`NL-6`, `NL-8`, `PO-PLUMB-15`).
///
/// - When neither parent is refused (`left.color().is_ok() && right.color().is_ok()`), this behaves
///   identically to [`compose_binary`] without activating the fallback.
/// - When either parent is refused (`color().is_err()`), the refusal remains absorbing unless
///   `fallback.fallback_active` is `true` and both `fallback_id` and `mechanism` are non-empty;
///   otherwise returns `Err(FMINE-REF-205)`.
/// - When engaged, the derived claim is bounded strictly to `EvidenceClass::Assumed` (never
///   upgraded above the weakest non-assertion class), `KnowledgeState` no stronger than `Estimated`
///   (`state.and(KnowledgeState::Estimated)`), `Color::Estimated` labelled `fallback:<id>`,
///   `calibration_score() == 0.0`, full confidentiality join (`NL-7`), and `uu_terms` union.
#[allow(clippy::too_many_arguments)]
pub fn compose_binary_with_fallback<A, B, R>(
    metric_key: impl Into<String>,
    left: &Claim<A>,
    right: &Claim<B>,
    value: R,
    canonical_value_tag: &str,
    representation: Representation,
    support: SpatialSupport,
    additivity: AdditivityClass,
    op: IntervalOp,
    fallback: &DeclaredFallback,
) -> Result<Claim<R>, Refusal> {
    let metric_key = metric_key.into();
    let normal = compose_binary(
        metric_key.clone(),
        left,
        right,
        value,
        canonical_value_tag,
        representation.clone(),
        support,
        additivity,
        op,
    );
    if normal.color().is_ok() {
        return Ok(normal);
    }
    if !fallback.fallback_active
        || fallback.fallback_id.trim().is_empty()
        || fallback.mechanism.trim().is_empty()
    {
        return Err(Refusal::new(
            RefusalCode::CalibrationRequired205,
            format!(
                "NL-6 on '{metric_key}': refused parent cannot yield a value without an active declared fallback"
            ),
        ));
    }
    let fallback_state = normal.state.and(KnowledgeState::Estimated);
    let mut degraded = Claim::new(
        metric_key,
        normal.value,
        canonical_value_tag,
        representation,
        support,
        additivity,
        EvidenceClass::Assumed,
        normal.epistemic,
        fallback_state,
        normal.confidentiality,
        normal.uu_terms,
        normal.parents,
        normal.anchors,
    );
    degraded.stale = normal.stale;
    degraded.calibration_failed = false;
    degraded.fallback = Some(fallback.clone());
    degraded.epsilon_contamination = normal.epsilon_contamination;
    degraded.color_basis = Some(Color::Estimated {
        estimator: format!("fallback:{}", fallback.fallback_id.trim()),
        dispersion: f64::INFINITY,
    });
    Ok(degraded)
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
    /// Verification status of any attached `REFCLASS-` record (`GATE-PLUMB-E4`).
    pub ref_class_status: Option<RefClassStatus>,
    /// Contamination weight `ε` in basis points (`0..=10_000`, `GATE-PLUMB-E4`).
    pub epsilon_bps: u16,
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
                ref_class_status: claim.epsilon_contamination.status(),
                epsilon_bps: claim.epsilon_contamination.epsilon_bps(),
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

    /// G-DISC (`§8.3`, `GATE-PLUMB-E4`), its evidence and reference-class clauses: a disclosure-facing export needs
    /// a class in {Measured, Derived, ModelledCalibrated}, no `Assertion` or `Assumed` anywhere in its provenance,
    /// no unbacked `epsilon_bps > 0` (`FMINE-REF-205`), and no `RefClassStatus::Unverified` on itself or any
    /// ancestor (`FMINE-REF-304`). A parent this DAG does not hold proves nothing, so it refuses too. The body must
    /// still match its id (`FMINE-REF-507`), and both this claim and its ancestors must be current (`FMINE-REF-202`,
    /// INV-013). The supplied root need not be inserted.
    ///
    /// §8.4 has no G-DISC code, so `FMINE-REF-304` is the nearest: passing is still necessary, not sufficient,
    /// because regulatory disclosure sign-off requires human authority attestation (`ClaimAuthority::Attested` is
    /// checked by downstream disclosure sign-off gates). FrankenMining's own public report is not a disclosure and
    /// does not call this.
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
        if claim.epsilon_contamination.epsilon_bps() > 0
            && claim.epsilon_contamination.ref_class().is_none()
        {
            return Err(Refusal::new(
                RefusalCode::CalibrationRequired205,
                format!(
                    "G-DISC on '{}': non-zero epsilon without registered REFCLASS- (GATE-PLUMB-E4)",
                    claim.metric_key
                ),
            ));
        }
        if let Some(rc) = claim.epsilon_contamination.ref_class()
            && rc.status == RefClassStatus::Unverified
        {
            return refuse(format!(
                "unverified reference class '{}' (GATE-PLUMB-E4)",
                rc.id
            ));
        }
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
            if node.epsilon_bps > 0 && node.ref_class_status.is_none() {
                return Err(Refusal::new(
                    RefusalCode::CalibrationRequired205,
                    format!(
                        "G-DISC on '{}': ancestor '{}' carries non-zero epsilon without REFCLASS- (GATE-PLUMB-E4)",
                        claim.metric_key, node.metric_key
                    ),
                ));
            }
            if node.ref_class_status == Some(RefClassStatus::Unverified) {
                return refuse(format!(
                    "'{}' in its provenance uses an unverified reference class (GATE-PLUMB-E4)",
                    node.metric_key
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
    use fmine_evidence::{AssertionSource, KuKind};

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

    /// BD-044 (`G-UU`, `INV-015`): decision-facing non-KK claims without a `UuTerm` are refused
    /// with `FMINE-REF-204`, whereas claims carrying an explicit `UuTerm` or `KnownKnown` pass.
    #[test]
    fn test_g_uu_decision_facing_claim_without_uu_refused() {
        let missing_uu = Claim::new(
            "tailings_freeboard_margin",
            1.4_f64,
            "1.4",
            Representation::Interval { lo: 1.1, hi: 1.7 },
            SpatialSupport::MiningPolygon,
            AdditivityClass::NonAdditive,
            EvidenceClass::ModelledCalibrated,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::internal(),
            vec![],
            vec![],
            vec![sample_anchor()],
        );
        let refusal = missing_uu
            .verify_uu_slot_for_decision()
            .expect_err("decision-facing claim without UU must refuse");
        assert_eq!(refusal.code, "FMINE-REF-204");
        assert_eq!(refusal.exit_code, 3);

        let with_uu = Claim::new(
            "tailings_freeboard_margin",
            1.4_f64,
            "1.4",
            Representation::Interval { lo: 1.1, hi: 1.7 },
            SpatialSupport::MiningPolygon,
            AdditivityClass::NonAdditive,
            EvidenceClass::ModelledCalibrated,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::internal(),
            vec![UuTerm {
                scenario_id: "UU-TSF-01".to_string(),
                description: "PMP storm exceeding 1:10000 gauge history".to_string(),
                downside_exposure_usd_m: 150.0,
                trigger_indicator: "72h catchment rainfall > 240 mm".to_string(),
            }],
            vec![],
            vec![sample_anchor()],
        );
        assert!(with_uu.verify_uu_slot_for_decision().is_ok());

        let kk = leaf("statutory_mass_kk", EvidenceClass::Measured);
        assert!(kk.verify_uu_slot_for_decision().is_ok());
    }

    #[test]
    fn test_with_regime_keeps_a_validated_basis_only_for_a_class_that_may_stand_on_one() {
        let stands = leaf("stands", EvidenceClass::Measured).with_regime(regime(), "qaqc-2026");
        assert_eq!(stands.color().map(|c| c.rank()), Ok(ColorRank::Validated));
        let invalid = leaf("empty-regime", EvidenceClass::Measured)
            .with_regime(ValidityDomain::unconstrained(), "qaqc-2026");
        assert_eq!(invalid.color().unwrap_err().code, "FMINE-REF-205");
        assert_eq!(
            join("child-invalid", &stands, &invalid)
                .color()
                .unwrap_err()
                .code,
            "FMINE-REF-205"
        );

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
    fn test_regime_attachment_preserves_parent_linked_evidence() {
        let reference = |lo, hi| ValidityDomain::unconstrained().with("feed_grade", lo, hi);
        let left =
            leaf("left", EvidenceClass::Measured).with_regime(reference(1.0, 3.0), "assay-a");
        let right =
            leaf("right", EvidenceClass::Derived).with_regime(reference(2.0, 4.0), "assay-b");
        let unanchored = leaf("unanchored", EvidenceClass::Measured);
        let disjoint =
            leaf("disjoint", EvidenceClass::Measured).with_regime(reference(5.0, 6.0), "assay-c");
        let overlap = join("overlap", &left, &right);
        let Color::Validated { regime, dataset } = overlap.color().unwrap() else {
            panic!("overlapping source regimes should validate");
        };
        assert_eq!(regime.bounds()["feed_grade"], (2.0, 3.0));
        assert!(dataset.contains("assay-a") && dataset.contains("assay-b"));
        let weak = join("weak", &left, &unanchored);
        let no_overlap = join("no-overlap", &left, &disjoint);
        let declared = stating("parent-linked", EvidenceClass::Derived, vec![left.claim_id]);
        for claim in [&weak, &no_overlap, &declared] {
            assert_eq!(claim.color().unwrap().rank(), ColorRank::Estimated);
        }
        for claim in [weak, no_overlap, overlap, declared] {
            let anchored = claim.clone().with_regime(reference(0.0, 10.0), "new-label");
            assert_eq!(anchored, claim, "{}", claim.metric_key);
            assert_eq!(
                join("next", &anchored, &left).color(),
                join("next", &claim, &left).color()
            );
        }
    }

    #[test]
    fn test_nl4_model_and_transfer_calibration_do_not_cross_derivation_or_support() {
        let transferred = EvidenceClass::Transferred {
            aoa: AoaStatus::Inside,
        };
        let unassessed = EvidenceClass::Transferred {
            aoa: AoaStatus::NotAssessed,
        };
        let outside = EvidenceClass::Transferred {
            aoa: AoaStatus::Outside,
        };
        let cases = [
            (
                EvidenceClass::ModelledCalibrated,
                EvidenceClass::ModelledCalibrated,
                EvidenceClass::ModelledUncalibrated,
            ),
            (
                EvidenceClass::ModelledCalibrated,
                EvidenceClass::Measured,
                EvidenceClass::ModelledUncalibrated,
            ),
            (
                EvidenceClass::ModelledCalibrated,
                EvidenceClass::Derived,
                EvidenceClass::ModelledUncalibrated,
            ),
            (transferred, transferred, unassessed),
            (transferred, EvidenceClass::Measured, unassessed),
            (transferred, EvidenceClass::Derived, unassessed),
            (transferred, EvidenceClass::ModelledCalibrated, unassessed),
            (unassessed, EvidenceClass::Measured, unassessed),
            (outside, EvidenceClass::Measured, outside),
        ];
        let supports = [
            SpatialSupport::PointCore,
            SpatialSupport::Composite { length_mm: 1000 },
            SpatialSupport::SmuBlock {
                dx_cm: 100,
                dy_cm: 100,
                dz_cm: 100,
            },
            SpatialSupport::MiningPolygon,
            SpatialSupport::PlantLot,
            SpatialSupport::PeriodAggregate,
        ];
        let measured = leaf("measured", EvidenceClass::Measured).with_regime(regime(), "qaqc");
        for (a, b, expected) in cases {
            for (left_class, right_class) in [(a, b), (b, a)] {
                let left = leaf("left", left_class).with_regime(regime(), "parent-left");
                let right = leaf("right", right_class).with_regime(regime(), "parent-right");
                let mut dag = ClaimDag::new();
                dag.insert(&left).unwrap();
                dag.insert(&right).unwrap();
                for support in supports {
                    let child = compose_binary(
                        "derived-family",
                        &left,
                        &right,
                        4.2,
                        "4.2",
                        Representation::Deterministic,
                        support,
                        AdditivityClass::ExtensiveAdditive,
                        IntervalOp::Add,
                    );
                    assert_eq!(child.evidence, expected, "{left_class:?}, {right_class:?}");
                    assert!(child.evidence.no_stronger_than(left_class));
                    assert!(child.evidence.no_stronger_than(right_class));
                    assert_eq!(child.color().map(|c| c.rank()), Ok(ColorRank::Estimated));
                    assert!(!child.calibration_failed, "uncalibrated is not failed");
                    assert_eq!(child.check_id(), Ok(()));
                    assert_eq!(child.parents, vec![left.claim_id, right.claim_id]);
                    assert_eq!(child.anchors, vec![sample_anchor()]);
                    assert_eq!(
                        dag.guard_disclosure_export(&child).unwrap_err().code,
                        "FMINE-REF-304"
                    );
                    // A dataset name cannot substitute for this family's resolved outcomes or AOA.
                    let reanchored = child.with_regime(regime(), "parent-left");
                    assert_eq!(
                        reanchored.color().map(|c| c.rank()),
                        Ok(ColorRank::Estimated)
                    );
                    assert_eq!(
                        join("next-family", &reanchored, &measured).evidence,
                        expected
                    );
                }
            }
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
        let a = leaf("a", EvidenceClass::Measured).with_regime(regime(), "qaqc-a");
        let b = leaf("b", EvidenceClass::Derived).with_regime(regime(), "qaqc-b");
        let c = leaf("c", EvidenceClass::Measured).with_regime(regime(), "qaqc-c");
        let ab = join("ab", &a, &b);
        let top = join("top", &ab, &c);
        assert_eq!(top.evidence, EvidenceClass::Derived);
        assert_eq!(top.color().map(|c| c.rank()), Ok(ColorRank::Validated));
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
    /// never outranks either parent in class, state or colour, never inherits calibration (NL-4), and is refused
    /// exactly when some leaf under it is.
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
                let calibration_holds = !matches!(
                    child.evidence,
                    EvidenceClass::ModelledCalibrated
                        | EvidenceClass::Transferred {
                            aoa: AoaStatus::Inside
                        }
                );
                if !(colour_holds && class_holds && state_holds && calibration_holds) {
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

    /// `taint` raises a label and reseals the id, and a lower or equal label changes nothing (INV-007).
    #[test]
    fn test_taint_only_raises_and_reseals_the_id() {
        let reg = LabelRegistry::new(["JV-A"], ["W-X"]).unwrap();
        let venture = Confidentiality::venture(reg.venture("JV-A").unwrap());
        let wall = Confidentiality::mnpi(reg.wall("W-X").unwrap());
        let mut claim = labelled("haulage_cost", Confidentiality::internal());
        let first = claim.claim_id;
        for lower in [Confidentiality::public(), Confidentiality::internal()] {
            claim.taint(&lower);
            assert_eq!(*claim.confidentiality(), Confidentiality::internal());
            assert_eq!((claim.claim_id, claim.supersedes), (first, None));
        }
        claim.taint(&venture);
        assert_eq!(*claim.confidentiality(), venture);
        assert_eq!(claim.supersedes, Some(first));
        assert_ne!(claim.claim_id, first);
        claim.check_id().unwrap();
        let second = claim.claim_id;
        claim.taint(&wall);
        assert_eq!(*claim.confidentiality(), venture.join(&wall));
        assert_eq!(claim.supersedes, Some(second));
        claim.check_id().unwrap();
        // Tainting with the venture again, or with public, is a no-op: the label is already above both.
        let third = claim.claim_id;
        claim.taint(&venture);
        claim.taint(&Confidentiality::public());
        assert_eq!((claim.claim_id, claim.supersedes), (third, Some(second)));
        assert!(claim.verify_export_clearance(&venture).is_err());
        assert!(claim.verify_export_clearance(&venture.join(&wall)).is_ok());
    }

    /// INV-007 over seeded random composition DAGs: leaves carry random labels built from two ventures and two walls,
    /// pairs are joined by `compose_binary` until one root is left, and some nodes are tainted on the way. Each node's
    /// label is exactly its parents' join, so it is never below either parent; a taint never lowers a label; and
    /// the root carries the join of every leaf and every taint. A reader cleared for the root's label reads it, and
    /// a reader missing any one of its ventures or walls is refused.
    #[test]
    fn test_random_claim_dags_never_lower_a_label_inv_007() {
        const CASES: usize = 20_000;
        let reg = LabelRegistry::new(["JV-A", "JV-B"], ["W-X", "W-Y"]).unwrap();
        let atoms = [
            Confidentiality::public(),
            Confidentiality::internal(),
            Confidentiality::venture(reg.venture("JV-A").unwrap()),
            Confidentiality::venture(reg.venture("JV-B").unwrap()),
            Confidentiality::mnpi(reg.wall("W-X").unwrap()),
            Confidentiality::mnpi(reg.wall("W-Y").unwrap()),
        ];
        let key = RngKey {
            seed_root: 0x0009_D0C5_0043,
            partition: 7,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        };
        let mut rng = PhiloxStream::new(key);
        let pick =
            |rng: &mut PhiloxStream| atoms[(rng.next_u64() % atoms.len() as u64) as usize].clone();
        let (mut nodes, mut raised, mut unchanged, mut refused) = (0_u64, 0_u64, 0_u64, 0_u64);
        let mut labels_seen = BTreeSet::new();
        for case in 0..CASES {
            let leaves = 2 + (rng.next_u64() % 7) as usize;
            let mut expected = Confidentiality::public();
            let mut pool: Vec<Claim<f64>> = (0..leaves)
                .map(|i| {
                    let mut label = pick(&mut rng);
                    if rng.next_u64().is_multiple_of(3) {
                        label = label.join(&pick(&mut rng));
                    }
                    expected = expected.join(&label);
                    labelled(&format!("leaf_{case}_{i}"), label)
                })
                .collect();
            while pool.len() > 1 {
                let l = pool.swap_remove((rng.next_u64() % pool.len() as u64) as usize);
                let r = pool.swap_remove((rng.next_u64() % pool.len() as u64) as usize);
                let mut child = join("node", &l, &r);
                nodes += 1;
                assert_eq!(
                    *child.confidentiality(),
                    l.confidentiality().join(r.confidentiality())
                );
                assert!(l.confidentiality().leq(child.confidentiality()));
                assert!(r.confidentiality().leq(child.confidentiality()));
                if rng.next_u64().is_multiple_of(4) {
                    let by = pick(&mut rng);
                    let (before, id) = (child.confidentiality().clone(), child.claim_id);
                    child.taint(&by);
                    assert_eq!(*child.confidentiality(), before.join(&by));
                    assert!(before.leq(child.confidentiality()));
                    if *child.confidentiality() == before {
                        assert_eq!(child.claim_id, id);
                        unchanged += 1;
                    } else {
                        assert_eq!(child.supersedes, Some(id));
                        raised += 1;
                    }
                    child.check_id().unwrap();
                    expected = expected.join(&by);
                }
                pool.push(child);
            }
            let root = pool.pop().unwrap();
            assert_eq!(*root.confidentiality(), expected, "case {case}");
            assert!(root.verify_export_clearance(&expected).is_ok());
            labels_seen.insert(expected.to_string());
            let ventures: Vec<VentureId> = expected.ventures().cloned().collect();
            let walls: Vec<WallId> = expected.walls().cloned().collect();
            for skip in 0..ventures.len() + walls.len() {
                let mut clearance = Confidentiality::internal();
                let kept_ventures = ventures.iter().enumerate().filter(|(k, _)| *k != skip);
                for (_, v) in kept_ventures {
                    clearance = clearance.join(&Confidentiality::venture(v.clone()));
                }
                let kept_walls = walls
                    .iter()
                    .enumerate()
                    .filter(|(k, _)| ventures.len() + *k != skip);
                for (_, w) in kept_walls {
                    clearance = clearance.join(&Confidentiality::mnpi(w.clone()));
                }
                assert!(
                    root.verify_export_clearance(&clearance).is_err(),
                    "case {case}"
                );
                refused += 1;
            }
            if !expected.is_public() {
                assert!(
                    root.verify_export_clearance(&Confidentiality::public())
                        .is_err()
                );
                refused += 1;
            }
        }
        println!(
            "{{\"test\":\"inv_007_random_dags\",\"cases\":{CASES},\"nodes\":{nodes},\"taint_raised\":{raised},\
             \"taint_unchanged\":{unchanged},\"refused_readers\":{refused},\"distinct_root_labels\":{}}}",
            labels_seen.len()
        );
        assert!(raised > 0 && unchanged > 0 && refused > 0);
        // Every combination of the two ventures and two walls, plus internal alone, reaches some root.
        assert_eq!(labels_seen.len(), 17);
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

    /// NL-3 (`§7.3`): institutional attestation changes `authority` ONLY and NEVER upgrades
    /// `evidence`, `epistemic`, `state`, `color`, `confidentiality`, `calibration_score`, or
    /// `canonical_body`; blank metadata or zero signature digest refuses with `FMINE-REF-304`.
    #[test]
    fn test_nl3_attestation_is_strictly_orthogonal() {
        let sig = Digest32([0x5A; 32]);
        for class in [
            EvidenceClass::ISSUER,
            EvidenceClass::Assumed,
            EvidenceClass::Elicited,
            EvidenceClass::ModelledUncalibrated,
            EvidenceClass::ModelledCalibrated,
            EvidenceClass::Measured,
        ] {
            let base = leaf("qp_target", class);
            let attested = base
                .attest("QP", sig, "NI-43-101 Table 14-1")
                .expect("valid attestation");
            assert_eq!(
                attested.authority(),
                &ClaimAuthority::Attested {
                    role: "QP".to_string(),
                    sig_digest: sig,
                    scope: "NI-43-101 Table 14-1".to_string(),
                }
            );
            assert_eq!(attested.evidence, base.evidence);
            assert_eq!(attested.epistemic, base.epistemic);
            assert_eq!(attested.state, base.state);
            assert_eq!(attested.color(), base.color());
            assert_eq!(attested.confidentiality, base.confidentiality);
            assert_eq!(attested.calibration_failed, base.calibration_failed);
            assert_eq!(attested.calibration_score(), base.calibration_score());
            assert_eq!(attested.canonical_body(), base.canonical_body());
            assert_eq!(attested.claim_id, base.claim_id);
        }

        // Composing two attested parents resets authority to Computed (parent sign-off never auto-transfers).
        let a = leaf("a", EvidenceClass::Measured)
            .attest("CP", sig, "JORC")
            .unwrap();
        let b = leaf("b", EvidenceClass::Assumed)
            .attest("EoR", sig, "GISTM")
            .unwrap();
        let child = join("ab", &a, &b);
        assert_eq!(child.authority(), &ClaimAuthority::default());
        assert_eq!(child.evidence, EvidenceClass::Assumed);

        // Blank role/scope or zero digest refuses cleanly with FMINE-REF-304.
        assert_eq!(
            a.attest("", sig, "scope").unwrap_err().code,
            "FMINE-REF-304"
        );
        assert_eq!(a.attest("QP", sig, " ").unwrap_err().code, "FMINE-REF-304");
        assert_eq!(
            a.attest("QP", Digest32([0u8; 32]), "scope")
                .unwrap_err()
                .code,
            "FMINE-REF-304"
        );
    }

    /// NL-6 & PO-PLUMB-15: refusals stay absorbing unless an active declared fallback is supplied,
    /// and an active fallback never launders colour above `Estimated` or drops confidentiality/UU.
    #[test]
    fn test_nl6_refusal_absorption_and_declared_fallback_bounds() {
        let reg = LabelRegistry::new(["ven_a"], ["W-MNPI"]).unwrap();
        let mnpi = Confidentiality::mnpi(reg.wall("W-MNPI").unwrap());
        let ven = Confidentiality::venture(reg.venture("ven_a").unwrap());

        let valid_parent = Claim::new(
            "measured_mass",
            100.0_f64,
            "100.0",
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Measured,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            ven.clone(),
            vec![],
            vec![],
            vec![sample_anchor()],
        )
        .with_regime(regime(), "qaqc-2026");

        let refused_parent = Claim::new(
            "issuer_grade",
            1.5_f64,
            "1.5",
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::IntensiveWeighted,
            EvidenceClass::ISSUER,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Unknown,
            mnpi.clone(),
            vec![UuTerm {
                scenario_id: "UU-FALLBACK-1".to_string(),
                description: "Unverified assay split".to_string(),
                downside_exposure_usd_m: 10.0,
                trigger_indicator: "Lab check variance > 15%".to_string(),
            }],
            vec![],
            vec![sample_anchor()],
        );

        // Without fallback, refusal is strictly absorbing.
        let bare_child = join("no_fallback", &valid_parent, &refused_parent);
        assert_eq!(bare_child.color().unwrap_err().code, "FMINE-REF-205");
        assert!(!bare_child.fallback_active());

        // Inactive or blank fallback MUST still refuse.
        let inactive = DeclaredFallback {
            fallback_id: "mapmark4-degenerate-n0".to_string(),
            mechanism: "USGS MapMark4-degenerate bounds".to_string(),
            fallback_active: false,
        };
        let err = compose_binary_with_fallback(
            "inactive_fb",
            &valid_parent,
            &refused_parent,
            150.0_f64,
            "150.0",
            Representation::Interval {
                lo: 80.0,
                hi: 220.0,
            },
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            IntervalOp::Mul,
            &inactive,
        )
        .expect_err("inactive fallback must refuse");
        assert_eq!(err.code, "FMINE-REF-205");

        let blank_id = DeclaredFallback {
            fallback_id: " ".to_string(),
            mechanism: "Manski bounds".to_string(),
            fallback_active: true,
        };
        assert_eq!(
            compose_binary_with_fallback(
                "blank_fb",
                &valid_parent,
                &refused_parent,
                150.0_f64,
                "150.0",
                Representation::Interval {
                    lo: 80.0,
                    hi: 220.0
                },
                SpatialSupport::PeriodAggregate,
                AdditivityClass::ExtensiveAdditive,
                IntervalOp::Mul,
                &blank_id,
            )
            .unwrap_err()
            .code,
            "FMINE-REF-205"
        );

        // Active declared fallback recovers an Estimated-only claim with full taint & UU union.
        let active = DeclaredFallback {
            fallback_id: "mapmark4-degenerate-n0".to_string(),
            mechanism: "USGS MapMark4-degenerate bounds".to_string(),
            fallback_active: true,
        };
        let degraded = compose_binary_with_fallback(
            "active_fb",
            &valid_parent,
            &refused_parent,
            150.0_f64,
            "150.0",
            Representation::Interval {
                lo: 80.0,
                hi: 220.0,
            },
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            IntervalOp::Mul,
            &active,
        )
        .expect("active declared fallback produces bounded Estimated claim");
        assert!(degraded.fallback_active());
        assert_eq!(degraded.evidence, EvidenceClass::Assumed);
        assert_eq!(degraded.state, KnowledgeState::Unknown);
        assert_eq!(degraded.color().unwrap().rank(), ColorRank::Estimated);
        assert_eq!(degraded.calibration_score(), 0.0);
        assert_eq!(degraded.confidentiality, ven.join(&mnpi));
        assert_eq!(degraded.uu_terms.len(), 1);
        // Downstream composition preserves fallback_active flag.
        let downstream = join("downstream", &degraded, &valid_parent);
        assert!(downstream.fallback_active());
        assert_eq!(downstream.color().unwrap().rank(), ColorRank::Estimated);
        assert_eq!(downstream.calibration_score(), 0.0);
    }

    /// NL-8 (`§7.3`): `CalibrationScore::compute` matches `1 - min(1, max(|cov80 - 0.8|/0.2, ks_pit))`,
    /// forces `0.0` whenever uncalibrated, refuses out-of-domain inputs without clamping, and never
    /// transfers to derived claims (`NL-4`).
    #[test]
    fn test_nl8_calibration_score_formula_and_refusals() {
        let perfect = CalibrationScore::compute(true, 0.80, 0.0).unwrap();
        assert!((perfect.score() - 1.0).abs() < 1e-15);

        let cov_limited = CalibrationScore::compute(true, 0.70, 0.05).unwrap();
        // |0.70 - 0.80| / 0.20 = 0.50 > 0.05 => 1 - 0.50 = 0.50
        assert!((cov_limited.score() - 0.50).abs() < 1e-12);

        let ks_limited = CalibrationScore::compute(true, 0.81, 0.35).unwrap();
        // |0.81 - 0.80| / 0.20 = 0.05 < 0.35 => 1 - 0.35 = 0.65
        assert!((ks_limited.score() - 0.65).abs() < 1e-12);

        let capped = CalibrationScore::compute(true, 0.50, 0.10).unwrap();
        // |0.50 - 0.80| / 0.20 = 1.50 -> capped at 1.0 => score = 0.0
        assert_eq!(capped.score(), 0.0);

        // Uncalibrated MUST yield 0.0 even with ideal empirical coverage and KS = 0.
        let uncal = CalibrationScore::compute(false, 0.80, 0.0).unwrap();
        assert_eq!(uncal.score(), 0.0);
        assert!(!uncal.is_calibrated());

        // Non-finite or out-of-[0,1] inputs refuse cleanly with FMINE-REF-109.
        for (cov, ks) in [
            (-0.01, 0.0),
            (1.01, 0.0),
            (f64::NAN, 0.0),
            (0.80, -0.01),
            (0.80, 1.01),
            (0.80, f64::INFINITY),
        ] {
            assert_eq!(
                CalibrationScore::compute(true, cov, ks).unwrap_err().code,
                "FMINE-REF-109",
                "cov={cov}, ks={ks}"
            );
        }

        // Leaf ModelledCalibrated claim carries score; derived claim MUST NOT inherit it (NL-4 + NL-8).
        let model_a =
            leaf("model_a", EvidenceClass::ModelledCalibrated).with_calibration_score(ks_limited);
        let model_b = leaf("model_b", EvidenceClass::Measured).with_calibration_score(perfect);
        assert!((model_a.calibration_score() - 0.65).abs() < 1e-12);
        assert!((model_b.calibration_score() - 1.0).abs() < 1e-15);
        let derived = join("derived_ab", &model_a, &model_b);
        assert_eq!(derived.calibration_score(), 0.0);
        // Attempting to attach a score to a derived claim is ignored (NL-4).
        let attempted = derived.with_calibration_score(perfect);
        assert_eq!(attempted.calibration_score(), 0.0);
        // Failed calibration or staleness zeroes the score.
        let mut stale_a = model_a.clone();
        stale_a.stale = true;
        assert_eq!(stale_a.calibration_score(), 0.0);
        let mut failed_a = model_a;
        failed_a.calibration_failed = true;
        assert_eq!(failed_a.calibration_score(), 0.0);
    }

    /// Full `GATE-PLUMB-E1` / `PO-PLUMB-1` property gate: 100,000 seeded Philox provenance DAGs
    /// (including shared-ancestor diamonds) verifying ALL EIGHT non-laundering laws `NL-1..NL-8`:
    /// - `NL-1`: derived colour never exceeds weakest input colour;
    /// - `NL-2`: derived evidence class is no stronger than either parent;
    /// - `NL-3`: attestation changes `authority` only (zero impact on evidence/color/state/score);
    /// - `NL-4`: model calibration & in-AOA transfer are never inherited across derivation;
    /// - `NL-5`: knowledge states never upgrade across all 9 variants (`NotObservable`, `Redacted`, `Stale`, etc.);
    /// - `NL-6`: refusals are absorbing without fallback, and declared fallback never exceeds `Estimated`/`Assumed`;
    /// - `NL-7`: confidentiality is a monotone join (`left <= child && right <= child`), enforced by export clearance;
    /// - `NL-8`: `calibration_score` is strictly `0.0` on uncalibrated/derived claims and bounded in `[0, 1]`.
    #[test]
    fn test_gate_plumb_e1_nl1_to_nl8_100k_dags() {
        const CASES: usize = 100_000;
        let reg = LabelRegistry::new(["ven_a", "ven_b"], ["W-1", "W-2"]).unwrap();
        let labels = [
            Confidentiality::public(),
            Confidentiality::internal(),
            Confidentiality::venture(reg.venture("ven_a").unwrap()),
            Confidentiality::venture(reg.venture("ven_b").unwrap()),
            Confidentiality::mnpi(reg.wall("W-1").unwrap()),
            Confidentiality::mnpi(reg.wall("W-2").unwrap()),
            Confidentiality::venture(reg.venture("ven_a").unwrap())
                .join(&Confidentiality::mnpi(reg.wall("W-1").unwrap())),
            Confidentiality::venture(reg.venture("ven_a").unwrap())
                .join(&Confidentiality::venture(reg.venture("ven_b").unwrap()))
                .join(&Confidentiality::mnpi(reg.wall("W-2").unwrap())),
        ];
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
            KnowledgeState::Stale,
            KnowledgeState::NotObservable,
            KnowledgeState::Redacted,
            KnowledgeState::Indeterminate,
            KnowledgeState::NotApplicable,
        ];
        let key = RngKey {
            seed_root: 0x000E_1000_0042,
            partition: 8,
            domain: 1,
            var_group: 0,
            realization: 0,
            brick: 0,
        };
        let mut rng = PhiloxStream::new(key);
        let fallback = DeclaredFallback {
            fallback_id: "mapmark4-degenerate-n0".to_string(),
            mechanism: "declared-degenerate-bounds".to_string(),
            fallback_active: true,
        };
        let sig = Digest32([0x77; 32]);
        let good_score = CalibrationScore::compute(true, 0.80, 0.04).unwrap();

        let mut violations = 0_u64;
        let mut validated = 0_u64;
        let mut refused = 0_u64;
        let mut fallback_engaged = 0_u64;
        let mut attested_checked = 0_u64;
        let mut mnpi_tainted = 0_u64;
        let mut diamond_edges = 0_u64;

        for case in 0..CASES {
            let n_leaves = 2 + (rng.next_u64() % 4) as usize;
            let mut nodes: Vec<(Claim<f64>, Option<ColorRank>)> = Vec::with_capacity(n_leaves + 4);
            for i in 0..n_leaves {
                let class = classes[(rng.next_u64() % classes.len() as u64) as usize];
                let state = states[(rng.next_u64() % states.len() as u64) as usize];
                let conf = labels[(rng.next_u64() % labels.len() as u64) as usize].clone();
                let mut claim = Claim::new(
                    if i % 2 == 0 { "leaf_a" } else { "leaf_b" },
                    1.0 + i as f64,
                    "1.0",
                    Representation::Deterministic,
                    SpatialSupport::PeriodAggregate,
                    AdditivityClass::ExtensiveAdditive,
                    class,
                    EpistemicStatus::KnownKnown,
                    state,
                    conf,
                    vec![],
                    vec![],
                    vec![sample_anchor()],
                );
                if rng.next_u64().is_multiple_of(2) {
                    claim = claim.with_regime(regime(), "qaqc-2026");
                }
                if rng.next_u64().is_multiple_of(3) {
                    claim = claim.with_calibration_score(good_score);
                }
                if rng.next_u64().is_multiple_of(11) {
                    claim.calibration_failed = true;
                }
                if rng.next_u64().is_multiple_of(5) {
                    let before_ev = claim.evidence;
                    let before_col = claim.color();
                    let before_state = claim.state;
                    let before_score = claim.calibration_score();
                    let before_id = claim.claim_id;
                    claim = claim.attest("QP", sig, "scope-e1").unwrap();
                    attested_checked += 1;
                    // NL-3: attestation must change authority only.
                    if claim.evidence != before_ev
                        || claim.color() != before_col
                        || claim.state != before_state
                        || claim.calibration_score().to_bits() != before_score.to_bits()
                        || claim.claim_id != before_id
                    {
                        violations += 1;
                    }
                }
                let rank = claim.color().ok().map(|c| c.rank());
                nodes.push((claim, rank));
            }

            // Build 2..=4 derived DAG nodes allowing shared ancestors (diamonds) by sampling from all prior nodes.
            let n_derived = 2 + (rng.next_u64() % 3) as usize;
            let mut used_indices = BTreeSet::new();
            for step in 0..n_derived {
                let li = (rng.next_u64() % nodes.len() as u64) as usize;
                let ri = (rng.next_u64() % nodes.len() as u64) as usize;
                if !used_indices.insert(li) || !used_indices.insert(ri) {
                    diamond_edges += 1;
                }
                let (l, l_rank) = &nodes[li];
                let (r, r_rank) = &nodes[ri];

                let child = compose_binary(
                    if step == 0 { "mid" } else { "top" },
                    l,
                    r,
                    2.5,
                    "2.5",
                    Representation::Deterministic,
                    SpatialSupport::PeriodAggregate,
                    AdditivityClass::ExtensiveAdditive,
                    IntervalOp::Add,
                );
                let expected_bound = l_rank.zip(*r_rank).map(|(a, b)| a.min(b));
                let child_rank = child.color().ok().map(|c| c.rank());

                // NL-1 & NL-6 (no fallback): colour never exceeds weakest parent and refusals absorb.
                let nl1_nl6 = match (child_rank, expected_bound) {
                    (None, None) => true,
                    (Some(c), Some(b)) => c <= b,
                    _ => false,
                };
                // NL-2: evidence class never upgrades.
                let nl2 = child.evidence.no_stronger_than(l.evidence)
                    && child.evidence.no_stronger_than(r.evidence);
                // NL-3: derived authority resets to Computed.
                let nl3 = child.authority() == &ClaimAuthority::default();
                // NL-4: model calibration & in-AOA transfer are never inherited.
                let nl4 = !matches!(
                    child.evidence,
                    EvidenceClass::ModelledCalibrated
                        | EvidenceClass::Transferred {
                            aoa: AoaStatus::Inside
                        }
                );
                // NL-5: knowledge state never upgrades (including Stale, NotObservable, Redacted, etc.).
                let nl5 = child.state.and(l.state) == child.state
                    && child.state.and(r.state) == child.state;
                // NL-7: confidentiality is monotone join and public export refuses any non-public child.
                let nl7 = l.confidentiality.leq(&child.confidentiality)
                    && r.confidentiality.leq(&child.confidentiality)
                    && (child.confidentiality.is_public()
                        || child
                            .verify_export_clearance(&Confidentiality::public())
                            .is_err());
                // NL-8: derived claim never inherits parent calibration_score (must be 0.0).
                let nl8 = child.calibration_score() == 0.0;

                if !(nl1_nl6 && nl2 && nl3 && nl4 && nl5 && nl7 && nl8) {
                    violations += 1;
                }

                if child.confidentiality.walls().next().is_some() {
                    mnpi_tainted += 1;
                }
                match child_rank {
                    None => {
                        refused += 1;
                        // Exercise NL-6 declared fallback on a subset of refused pairs.
                        if case % 5 == 0 {
                            let fb = compose_binary_with_fallback(
                                "fb_node",
                                l,
                                r,
                                2.5,
                                "2.5",
                                Representation::Interval { lo: 1.0, hi: 4.0 },
                                SpatialSupport::PeriodAggregate,
                                AdditivityClass::ExtensiveAdditive,
                                IntervalOp::Add,
                                &fallback,
                            )
                            .unwrap();
                            fallback_engaged += 1;
                            if fb.evidence != EvidenceClass::Assumed
                                || fb.color().ok().map(|c| c.rank()) != Some(ColorRank::Estimated)
                                || !fb.fallback_active()
                                || fb.calibration_score() != 0.0
                                || !l.confidentiality.leq(&fb.confidentiality)
                                || !r.confidentiality.leq(&fb.confidentiality)
                            {
                                violations += 1;
                            }
                        }
                    }
                    Some(ColorRank::Validated) => validated += 1,
                    Some(_) => {}
                }

                nodes.push((child, child_rank));
            }
        }

        println!(
            "{{\"gate\":\"GATE-PLUMB-E1\",\"cases\":{CASES},\"violations\":{violations},\
              \"validated\":{validated},\"refused\":{refused},\"fallback_engaged\":{fallback_engaged},\
              \"attested\":{attested_checked},\"mnpi_tainted\":{mnpi_tainted},\"diamond_edges\":{diamond_edges}}}"
        );
        assert_eq!(violations, 0);
        assert!(validated > 0 && refused > 0 && fallback_engaged > 0);
        assert!(attested_checked > 0 && mnpi_tainted > 0 && diamond_edges > 0);
    }

    #[test]
    fn test_gate_plumb_e4_claim_refclass_and_g_disc_blocks_unverified_and_ancestors() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let text = std::fs::read_to_string(root.join("registries/refclass.toml"))
            .expect("read registries/refclass.toml");
        let reg = RefClassRegistry::parse(&text).expect("parse refclass registry");

        let base = leaf("greenfield_capex_usd_m", EvidenceClass::Measured);
        let original_id = base.claim_id;

        // 1. Non-zero epsilon without RefClass refuses with FMINE-REF-205.
        let no_rc = base
            .clone()
            .with_checked_epsilon(None, 1_500)
            .expect_err("non-zero epsilon without RefClass must refuse");
        assert_eq!((no_rc.code, no_rc.exit_code), ("FMINE-REF-205", 3));

        // 2. Unregistered RefClass refuses with FMINE-REF-201.
        let unknown = base
            .clone()
            .with_ref_class(&reg, "REFCLASS-DOES-NOT-EXIST", None)
            .expect_err("unknown refclass must refuse");
        assert_eq!((unknown.code, unknown.exit_code), ("FMINE-REF-201", 3));

        // 3. Verified REFCLASS-CAPEX-GREENFIELD attaches, preserves pinned claim_id, and passes G-DISC.
        let verified = base
            .clone()
            .with_ref_class(&reg, "REFCLASS-CAPEX-GREENFIELD", None)
            .expect("verified refclass attaches");
        assert_eq!(verified.claim_id, original_id);
        assert_eq!(
            verified.epsilon_contamination().status(),
            Some(RefClassStatus::Verified)
        );
        let (mu, sd) = verified
            .epsilon_contamination()
            .blend_moments(1000.0, 100.0)
            .unwrap();
        assert!((mu - 1070.0).abs() < 1e-12);
        assert!((sd - 52_825.0_f64.sqrt()).abs() < 1e-11);

        let mut dag = ClaimDag::new();
        dag.insert(&verified).unwrap();
        dag.guard_disclosure_export(&verified)
            .expect("verified refclass passes G-DISC");

        // 4. Unverified REFCLASS-TAILINGS-PRELIMINARY is usable internally, refused directly by G-DISC.
        let unverified = leaf("tailings_capex_usd_m", EvidenceClass::Measured)
            .with_ref_class(&reg, "REFCLASS-TAILINGS-PRELIMINARY", None)
            .expect("unverified refclass allowed internally");
        unverified
            .verify_export_clearance(&Confidentiality::internal())
            .expect("internal clearance allowed");
        let direct_err = dag
            .guard_disclosure_export(&unverified)
            .expect_err("G-DISC must refuse unverified refclass directly");
        assert_eq!(
            (direct_err.code, direct_err.exit_code),
            ("FMINE-REF-304", 7)
        );
        assert!(direct_err.what.contains("REFCLASS-TAILINGS-PRELIMINARY"));

        // 5. Composing unverified with verified propagates Unverified taint and refuses G-DISC.
        dag.insert(&unverified).unwrap();
        let composed = join("combined_capex_usd_m", &verified, &unverified);
        assert_eq!(
            composed.epsilon_contamination().status(),
            Some(RefClassStatus::Unverified)
        );
        let composed_err = dag
            .guard_disclosure_export(&composed)
            .expect_err("G-DISC must refuse descendant composed from unverified refclass");
        assert_eq!(
            (composed_err.code, composed_err.exit_code),
            ("FMINE-REF-304", 7)
        );

        // 6. Even a manually constructed Derived claim with no local RefClass whose DAG parent used
        //    an Unverified RefClass is caught by G-DISC's ancestor walk.
        let manual_child = stating(
            "manual_descendant",
            EvidenceClass::Derived,
            vec![unverified.claim_id],
        );
        assert_eq!(manual_child.epsilon_contamination().status(), None);
        let anc_err = dag
            .guard_disclosure_export(&manual_child)
            .expect_err("G-DISC ancestor walk must catch unverified parent refclass");
        assert_eq!((anc_err.code, anc_err.exit_code), ("FMINE-REF-304", 7));
        assert!(anc_err.what.contains("unverified reference class"));
    }
}
