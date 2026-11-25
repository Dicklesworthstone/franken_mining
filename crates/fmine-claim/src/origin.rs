//! Determinism: BitwisePortable
//! Evidence-origin accounting, typed dependence relations, and batch-bias GLS kernel (`BD-352`,
//! plan §7.1–§7.3, `E692`, `E698`).
//!
//! Enforces five core statistical integrity rules:
//! 1. **Explicit origin identities & typed relations** — matching numbers or text never trigger
//!    automatic merges; unreviewed proposals stay `ProposedAssertion` and never narrow variance.
//! 2. **Reprint invariance** — `PublicationCopy` rows collapse to their originating `OriginId`
//!    before variance aggregation so copying or permuting publications NEVER narrows measurement
//!    uncertainty.
//! 3. **Exact batch-bias variance** — for `y_i = μ + b_{g(i)} + ε_i` with `Σ = σ_ε²·I + σ_b²·Z·Zᵀ`,
//!    each batch `g` of `n_g` active observations has mean variance `v_g = σ_b² + σ_ε² / n_g`, and
//!    independent batches combine via exact GLS `Var(μ̂) = (1ᵀ Σ⁻¹ 1)⁻¹ = (Σ_g 1/v_g)⁻¹`.
//! 4. **Conservative unknown dependence** — when any active relation is `UnknownDependence`, never
//!    assume `ρ = 0`; report worst-case `ρ = 1` variance `σ_b² + σ_ε²` and state the assumption
//!    required to narrow.
//! 5. **Rounding vs. measurement separation (`E698`) & supersession (`E692`)** — publication copies
//!    may intersect deterministic printed `RoundingInterval`s without narrowing `measurement_sd`;
//!    `SupersedingPublication` retains history while elevating the latest vintage, and conflicting
//!    same-rank values stay `KnowledgeState::Conflicted` (never averaged).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fmine_core::codec::Encoder;
use fmine_core::det::{norm_ppf, sqrt};
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_evidence::{KnowledgeState, Refusal, RefusalCode};

/// Canonical identity of an originating physical sample/assay or public filing cell (`BD-352`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OriginId(String);

impl OriginId {
    /// Validate and construct an [`OriginId`] (non-empty, `<= 128` bytes, printable ASCII without
    /// control characters or surrounding whitespace).
    pub fn parse(raw: &str) -> Result<Self, Refusal> {
        let trimmed = raw.trim();
        if trimmed != raw
            || trimmed.is_empty()
            || trimmed.len() > 128
            || trimmed.bytes().any(|b| !(0x21..=0x7E).contains(&b))
        {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!(
                    "invalid OriginId {raw:?}: must be 1..=128 printable non-whitespace ASCII bytes"
                ),
            ));
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Underlying string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OriginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Canonical identifier for a declared shared systematic-error group (assay batch or sample-prep
/// lot; `BD-352`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BatchGroupId(String);

impl BatchGroupId {
    /// Validate and construct a [`BatchGroupId`].
    pub fn parse(raw: &str) -> Result<Self, Refusal> {
        let trimmed = raw.trim();
        if trimmed != raw
            || trimmed.is_empty()
            || trimmed.len() > 128
            || trimmed.bytes().any(|b| !(0x21..=0x7E).contains(&b))
        {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!(
                    "invalid BatchGroupId {raw:?}: must be 1..=128 printable non-whitespace ASCII bytes"
                ),
            ));
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Underlying string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BatchGroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Deterministic printed rounding interval `[lo, hi]` kept strictly separate from measurement
/// uncertainty (`E698`, `BD-352`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundingInterval {
    pub lo: f64,
    pub hi: f64,
}

impl RoundingInterval {
    /// Construct a validated finite rounding interval `lo <= hi`.
    pub fn new(lo: f64, hi: f64) -> Result<Self, Refusal> {
        if !lo.is_finite() || !hi.is_finite() || lo > hi {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("invalid RoundingInterval [{lo}, {hi}]"),
            ));
        }
        Ok(Self { lo, hi })
    }

    /// Intersect two rounding intervals of the same deterministic printed origin (`E698`).
    /// Returns `None` if the intervals are disjoint (contradictory printed figures).
    #[must_use]
    pub fn intersect(self, other: Self) -> Option<Self> {
        let lo = self.lo.max(other.lo);
        let hi = self.hi.min(other.hi);
        if lo <= hi + 1e-12 {
            Some(Self { lo, hi: hi.max(lo) })
        } else {
            None
        }
    }
}

/// Typed statistical dependence relation attached to an [`OriginReading`] (`BD-352`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum OriginRelation {
    /// Independent new observation (own singleton group).
    NewObservation,
    /// Declared covariance block sharing group bias `b_{group_id}`.
    SharedSystematicError { group_id: BatchGroupId },
    /// Legitimate same-sample technical replicate: shares sample-prep error `sample_id`, has
    /// independent analytical error `ε_i`; NEVER collapsed as a reprint.
    TechnicalReplicate { sample_id: BatchGroupId },
    /// Publication copy of `of_origin`: same underlying observation, adds ZERO statistical sample
    /// size; may intersect deterministic `rounding_interval` only (`E698`).
    PublicationCopy { of_origin: OriginId },
    /// Superseding publication (`E692`): later `vintage_rank` replaces `supersedes` as current
    /// while retaining the superseded row in history.
    SupersedingPublication {
        supersedes: OriginId,
        vintage_rank: u64,
    },
    /// Repeated reading of one single measurement: not independent, collapsed with `ρ = 1`
    /// (adds zero statistical precision beyond the originating measurement).
    RepeatedReading { of_origin: OriginId },
    /// Unknown dependence between readings: NEVER assumes `ρ = 0`; enforces worst-case `ρ = 1`.
    UnknownDependence,
}

impl OriginRelation {
    /// Stable canonical tag for serialization and structured logging.
    #[must_use]
    pub const fn kind_tag(&self) -> &'static str {
        match self {
            Self::NewObservation => "new_observation",
            Self::SharedSystematicError { .. } => "shared_systematic_error",
            Self::TechnicalReplicate { .. } => "technical_replicate",
            Self::PublicationCopy { .. } => "publication_copy",
            Self::SupersedingPublication { .. } => "superseding_publication",
            Self::RepeatedReading { .. } => "repeated_reading",
            Self::UnknownDependence => "unknown_dependence",
        }
    }
}

/// Review gate preventing automatic text/number matching from merging origins (`BD-352`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RelationStatus {
    /// Manually reviewed and accepted by an institutional role (never a personal name).
    Reviewed { reviewer_role: String },
    /// Unreviewed proposed relation: stays an `Assertion`, NEVER auto-merged and NEVER narrows
    /// measurement variance.
    ProposedAssertion,
}

/// Single reading entering the evidence-origin accounting kernel (`BD-352`).
#[derive(Debug, Clone, PartialEq)]
pub struct OriginReading {
    /// Unique identifier for this reading record (e.g. `"row-q2-mda"` or `"assay-001"`).
    pub reading_id: String,
    /// Canonical originating sample or publication cell id.
    pub origin_id: OriginId,
    /// Numeric observation value.
    pub value: f64,
    /// Separate deterministic rounding interval `[lo, hi]` if applicable (`E698`).
    pub rounding_interval: Option<RoundingInterval>,
    /// Typed dependence relation.
    pub relation: OriginRelation,
    /// Review status of `relation` (no automatic merges).
    pub status: RelationStatus,
    /// Optional explicit vintage rank for root rows involved in supersession chains (`E692`).
    pub vintage_rank: u64,
}

/// Parameters of the Gaussian batch-bias measurement model `y_i = μ + b_{g(i)} + ε_i` (`BD-352`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatchBiasModel {
    /// Group/batch systematic bias standard deviation `σ_b >= 0`.
    pub sigma_b: f64,
    /// Independent analytical/measurement noise standard deviation `σ_ε > 0`.
    pub sigma_eps: f64,
    /// Two-sided nominal coverage level in `(0, 1)` (e.g. `0.95`).
    pub nominal_level: f64,
}

impl BatchBiasModel {
    /// Validate finite physical parameters.
    pub fn validate(&self) -> Result<(), Refusal> {
        if !self.sigma_b.is_finite() || self.sigma_b < 0.0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("sigma_b ({}) must be finite and >= 0", self.sigma_b),
            ));
        }
        if !self.sigma_eps.is_finite() || self.sigma_eps <= 0.0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("sigma_eps ({}) must be finite and > 0", self.sigma_eps),
            ));
        }
        if !self.nominal_level.is_finite() || self.nominal_level <= 0.0 || self.nominal_level >= 1.0
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "nominal_level ({}) must be finite and in (0, 1)",
                    self.nominal_level
                ),
            ));
        }
        Ok(())
    }
}

/// Historical superseded row retained for auditability (`E692`).
#[derive(Debug, Clone, PartialEq)]
pub struct RetainedSupersession {
    pub superseded_origin: OriginId,
    pub superseded_value: f64,
    pub superseded_by: OriginId,
    pub superseded_vintage_rank: u64,
    pub current_vintage_rank: u64,
}

/// One resolved variance group after collapsing publication copies and repeated readings.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedGroupSummary {
    pub group_key: String,
    pub active_observations: usize,
    pub group_mean: f64,
    pub group_mean_var: f64,
}

/// Result of evidence-origin accounting and batch-bias GLS inference (`BD-352`).
#[derive(Debug, Clone, PartialEq)]
pub struct OriginAccountingResult {
    /// Resulting epistemic knowledge state (`Estimated` or `Conflicted`).
    pub knowledge_state: KnowledgeState,
    /// GLS estimate `μ̂` (`None` when `Conflicted`).
    pub estimate: Option<f64>,
    /// Measurement variance `Var(μ̂)` (`None` when `Conflicted`).
    pub measurement_var: Option<f64>,
    /// Measurement standard error `sqrt(Var(μ̂))` (`None` when `Conflicted`).
    pub measurement_sd: Option<f64>,
    /// Symmetric normal measurement interval `[lo, hi]` at `model.nominal_level` (`None` if
    /// `Conflicted`). Never narrowed by reprints.
    pub measurement_interval: Option<(f64, f64)>,
    /// Separate deterministic rounding interval intersected across copies of a single origin
    /// (`E698`); `None` if multiple independent origins or no rounding intervals were supplied.
    pub rounding_interval: Option<RoundingInterval>,
    /// Description of the variance model applied (e.g. `"batch_bias_gls"` or
    /// `"conservative_rho_1_unknown_dependence"`).
    pub var_model: &'static str,
    /// Explicit assumption that would license narrowing when conservative `ρ = 1` applies.
    pub assumption_to_narrow: Option<String>,
    /// Distinct active origins retained after collapsing reviewed copies/repeated readings and
    /// applying reviewed supersessions.
    pub active_origin_ids: Vec<OriginId>,
    /// Total raw input readings supplied.
    pub input_reading_count: usize,
    /// Number of reviewed publication copies collapsed before statistical aggregation.
    pub collapsed_copies: usize,
    /// Number of unreviewed `ProposedAssertion` relations held back from automatic merging.
    pub unreviewed_assertions: usize,
    /// Superseded historical rows retained (`E692`).
    pub retained_history: Vec<RetainedSupersession>,
    /// Per-group resolved summaries (sorted by `group_key`).
    pub groups: Vec<ResolvedGroupSummary>,
    /// Distinct canonical relation tags present in the input (sorted).
    pub relation_tags: Vec<&'static str>,
    /// Canonical BLAKE3 identity over all sorted origins, relations, statuses, and model inputs.
    pub result_digest: Digest32,
}

impl OriginAccountingResult {
    /// Render structured E2E log `{origins, relations, groups, var_model, interval, ...}` (`BD-352`).
    #[must_use]
    pub fn to_json_log(&self) -> String {
        let origins_json: Vec<String> = self
            .active_origin_ids
            .iter()
            .map(|o| format!("\"{}\"", o.as_str()))
            .collect();
        let rels_json: Vec<String> = self
            .relation_tags
            .iter()
            .map(|r| format!("\"{r}\""))
            .collect();
        let groups_json: Vec<String> = self
            .groups
            .iter()
            .map(|g| {
                format!(
                    "{{\"group\":\"{}\",\"n\":{},\"mean\":{:.6},\"var\":{:.8}}}",
                    g.group_key, g.active_observations, g.group_mean, g.group_mean_var
                )
            })
            .collect();
        let interval_json = match self.measurement_interval {
            Some((lo, hi)) => format!("[{lo:.6},{hi:.6}]"),
            None => "null".to_string(),
        };
        let state_str = match self.knowledge_state {
            KnowledgeState::Estimated => "estimated",
            KnowledgeState::Conflicted => "conflicted",
            _ => "other",
        };
        format!(
            "{{\"origins\":[{}],\"relations\":[{}],\"groups\":[{}],\"var_model\":\"{}\",\"interval\":{},\"state\":\"{}\",\"collapsed_copies\":{},\"retained_history\":{},\"result_digest\":\"{}\"}}",
            origins_json.join(","),
            rels_json.join(","),
            groups_json.join(","),
            self.var_model,
            interval_json,
            state_str,
            self.collapsed_copies,
            self.retained_history.len(),
            self.result_digest,
        )
    }
}

fn canonical_f64_bits(v: f64) -> u64 {
    if v == 0.0 {
        0.0_f64.to_bits()
    } else {
        v.to_bits()
    }
}

fn canonical_relation_encoding(enc: &mut Encoder, rel: &OriginRelation) {
    enc.put_str(rel.kind_tag());
    match rel {
        OriginRelation::NewObservation | OriginRelation::UnknownDependence => {}
        OriginRelation::SharedSystematicError { group_id } => enc.put_str(group_id.as_str()),
        OriginRelation::TechnicalReplicate { sample_id } => enc.put_str(sample_id.as_str()),
        OriginRelation::PublicationCopy { of_origin }
        | OriginRelation::RepeatedReading { of_origin } => enc.put_str(of_origin.as_str()),
        OriginRelation::SupersedingPublication {
            supersedes,
            vintage_rank,
        } => {
            enc.put_str(supersedes.as_str());
            enc.put_u64(*vintage_rank);
        }
    }
}

fn reading_sort_key(r: &OriginReading) -> (String, String, &'static str, u64, u64) {
    (
        r.origin_id.as_str().to_string(),
        r.reading_id.clone(),
        r.relation.kind_tag(),
        r.vintage_rank,
        canonical_f64_bits(r.value),
    )
}

fn compute_inputs_digest(sorted_readings: &[OriginReading], model: &BatchBiasModel) -> Digest32 {
    let mut enc = Encoder::new_frame(*b"ORIG", 1);
    enc.put_u64(canonical_f64_bits(model.sigma_b));
    enc.put_u64(canonical_f64_bits(model.sigma_eps));
    enc.put_u64(canonical_f64_bits(model.nominal_level));
    enc.put_u64(sorted_readings.len() as u64);
    for r in sorted_readings {
        enc.put_str(&r.reading_id);
        enc.put_str(r.origin_id.as_str());
        enc.put_u64(canonical_f64_bits(r.value));
        enc.put_u64(r.vintage_rank);
        match r.rounding_interval {
            Some(ri) => {
                enc.put_u8(1);
                enc.put_u64(canonical_f64_bits(ri.lo));
                enc.put_u64(canonical_f64_bits(ri.hi));
            }
            None => enc.put_u8(0),
        }
        canonical_relation_encoding(&mut enc, &r.relation);
        match &r.status {
            RelationStatus::Reviewed { reviewer_role } => {
                enc.put_u8(1);
                enc.put_str(reviewer_role);
            }
            RelationStatus::ProposedAssertion => enc.put_u8(0),
        }
    }
    blake3_hash(&enc.finish())
}

/// Run evidence-origin accounting and the exact batch-bias GLS measurement kernel (`BD-352`).
///
/// Order-invariant over `readings`: any permutation yields bit-identical estimates, intervals,
/// logs, and `result_digest`.
pub fn evaluate_origin_batch_model(
    readings: &[OriginReading],
    model: BatchBiasModel,
) -> Result<OriginAccountingResult, Refusal> {
    model.validate()?;
    if readings.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "evaluate_origin_batch_model requires at least one OriginReading",
        ));
    }

    let mut sorted = readings.to_vec();
    for r in &sorted {
        if r.reading_id.trim().is_empty() {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "OriginReading.reading_id must be non-empty",
            ));
        }
        if !r.value.is_finite() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("OriginReading '{}' has non-finite value", r.reading_id),
            ));
        }
        if let Some(ri) = r.rounding_interval {
            let _ = RoundingInterval::new(ri.lo, ri.hi)?;
        }
        if let RelationStatus::Reviewed { reviewer_role } = &r.status
            && reviewer_role.trim().is_empty()
        {
            return Err(Refusal::new(
                RefusalCode::MissingHumanToken304,
                format!(
                    "OriginReading '{}' Reviewed status requires a non-empty reviewer_role",
                    r.reading_id
                ),
            ));
        }
    }
    sorted.sort_by_key(reading_sort_key);

    let result_digest = compute_inputs_digest(&sorted, &model);
    let relation_tags: Vec<&'static str> = sorted
        .iter()
        .map(|r| r.relation.kind_tag())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    // 1. Track reviewed supersessions (E692) and detect conflicting same-rank superseding values.
    // Map: superseded OriginId -> list of reviewed superseding readings targeting it.
    let mut superseding_by_target: BTreeMap<OriginId, Vec<&OriginReading>> = BTreeMap::new();
    let mut unreviewed_assertions = 0usize;
    let mut any_unknown_dependence = false;

    for r in &sorted {
        match (&r.relation, &r.status) {
            (OriginRelation::UnknownDependence, _) => {
                any_unknown_dependence = true;
            }
            (_, RelationStatus::ProposedAssertion) => {
                // Never auto-merge and never treat unreviewed assertions as independent!
                unreviewed_assertions += 1;
                any_unknown_dependence = true;
            }
            (
                OriginRelation::SupersedingPublication { supersedes, .. },
                RelationStatus::Reviewed { .. },
            ) => {
                superseding_by_target
                    .entry(supersedes.clone())
                    .or_default()
                    .push(r);
            }
            _ => {}
        }
    }

    let mut conflicted = false;
    let mut superseded_origins: BTreeSet<OriginId> = BTreeSet::new();
    let mut dropped_lower_supersessions: BTreeSet<OriginId> = BTreeSet::new();
    let mut retained_history: Vec<RetainedSupersession> = Vec::new();

    // Index base readings by OriginId so we can retain exact superseded values.
    let mut base_value_by_origin: BTreeMap<OriginId, (f64, u64)> = BTreeMap::new();
    for r in &sorted {
        base_value_by_origin
            .entry(r.origin_id.clone())
            .or_insert((r.value, r.vintage_rank));
    }

    for (target_id, candidates) in &superseding_by_target {
        let mut max_rank = 0u64;
        for c in candidates {
            if let OriginRelation::SupersedingPublication { vintage_rank, .. } = c.relation
                && vintage_rank > max_rank
            {
                max_rank = vintage_rank;
            }
        }
        let top: Vec<&OriginReading> = candidates
            .iter()
            .copied()
            .filter(|c| {
                matches!(
                    c.relation,
                    OriginRelation::SupersedingPublication { vintage_rank, .. } if vintage_rank == max_rank
                )
            })
            .collect();

        // Check for conflicting top-rank superseding values (distinct values at the same top vintage rank).
        if top.len() > 1 {
            let v0 = top[0].value;
            if top.iter().any(|c| (c.value - v0).abs() > 1e-12) {
                conflicted = true;
            }
        }

        if let Some(winner) = top.first() {
            superseded_origins.insert(target_id.clone());
            if let Some(&(old_val, old_rank)) = base_value_by_origin.get(target_id) {
                retained_history.push(RetainedSupersession {
                    superseded_origin: target_id.clone(),
                    superseded_value: old_val,
                    superseded_by: winner.origin_id.clone(),
                    superseded_vintage_rank: old_rank,
                    current_vintage_rank: max_rank,
                });
            }
            for c in candidates {
                if let OriginRelation::SupersedingPublication { vintage_rank, .. } = c.relation
                    && vintage_rank < max_rank
                {
                    dropped_lower_supersessions.insert(c.origin_id.clone());
                    retained_history.push(RetainedSupersession {
                        superseded_origin: c.origin_id.clone(),
                        superseded_value: c.value,
                        superseded_by: winner.origin_id.clone(),
                        superseded_vintage_rank: vintage_rank,
                        current_vintage_rank: max_rank,
                    });
                }
            }
        }
    }

    // 2. Collapse reviewed PublicationCopy and RepeatedReading rows onto their canonical origin
    //    BEFORE statistical aggregation (reprints NEVER narrow measurement intervals).
    //    Intersect deterministic rounding intervals per canonical origin (E698).
    let mut rounding_by_origin: BTreeMap<OriginId, RoundingInterval> = BTreeMap::new();
    let mut collapsed_copies = 0usize;
    let mut active_rows: Vec<&OriginReading> = Vec::new();
    let mut seen_active_origins: BTreeSet<OriginId> = BTreeSet::new();

    for r in &sorted {
        if superseded_origins.contains(&r.origin_id)
            || dropped_lower_supersessions.contains(&r.origin_id)
        {
            continue;
        }
        match (&r.relation, &r.status) {
            (OriginRelation::PublicationCopy { of_origin }, RelationStatus::Reviewed { .. }) => {
                collapsed_copies += 1;
                if let Some(ri) = r.rounding_interval {
                    match rounding_by_origin.get(of_origin).copied() {
                        None => {
                            rounding_by_origin.insert(of_origin.clone(), ri);
                        }
                        Some(existing) => match existing.intersect(ri) {
                            Some(merged) => {
                                rounding_by_origin.insert(of_origin.clone(), merged);
                            }
                            None => conflicted = true,
                        },
                    }
                }
            }
            (OriginRelation::RepeatedReading { of_origin: _ }, RelationStatus::Reviewed { .. }) => {
                // Repeated reading of the same underlying measurement adds zero precision.
                collapsed_copies += 1;
            }
            _ => {
                if let Some(ri) = r.rounding_interval {
                    match rounding_by_origin.get(&r.origin_id).copied() {
                        None => {
                            rounding_by_origin.insert(r.origin_id.clone(), ri);
                        }
                        Some(existing) => match existing.intersect(ri) {
                            Some(merged) => {
                                rounding_by_origin.insert(r.origin_id.clone(), merged);
                            }
                            None => conflicted = true,
                        },
                    }
                }
                if seen_active_origins.insert(r.origin_id.clone()) {
                    active_rows.push(r);
                } else {
                    // Identical origin_id appearing twice without an explicit TechnicalReplicate or
                    // distinct origin never double-counts; conflicting values flag Conflicted.
                    if let Some(prev) = active_rows.iter().find(|p| p.origin_id == r.origin_id)
                        && (prev.value - r.value).abs() > 1e-12
                    {
                        conflicted = true;
                    }
                    collapsed_copies += 1;
                }
            }
        }
    }

    let active_origin_ids: Vec<OriginId> = active_rows
        .iter()
        .map(|r| r.origin_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let combined_rounding = if active_origin_ids.len() == 1 {
        rounding_by_origin.get(&active_origin_ids[0]).copied()
    } else {
        None
    };

    if conflicted || active_rows.is_empty() {
        return Ok(OriginAccountingResult {
            knowledge_state: KnowledgeState::Conflicted,
            estimate: None,
            measurement_var: None,
            measurement_sd: None,
            measurement_interval: None,
            rounding_interval: combined_rounding,
            var_model: "conflicted_not_averaged",
            assumption_to_narrow: Some(
                "resolve conflicting values or disjoint rounding intervals before statistical aggregation"
                    .to_string(),
            ),
            active_origin_ids,
            input_reading_count: readings.len(),
            collapsed_copies,
            unreviewed_assertions,
            retained_history,
            groups: Vec::new(),
            relation_tags,
            result_digest,
        });
    }

    let z_crit = norm_ppf(0.5 + 0.5 * model.nominal_level);
    let var_b = model.sigma_b * model.sigma_b;
    let var_eps = model.sigma_eps * model.sigma_eps;

    // 3. Conservative UnknownDependence / unreviewed assertion rule: ρ = 1 worst-case bound.
    if any_unknown_dependence {
        let n = active_rows.len() as f64;
        let mean = active_rows.iter().map(|r| r.value).sum::<f64>() / n;
        let conservative_var = var_b + var_eps;
        let conservative_sd = sqrt(conservative_var);
        let half = z_crit * conservative_sd;
        return Ok(OriginAccountingResult {
            knowledge_state: KnowledgeState::Estimated,
            estimate: Some(mean),
            measurement_var: Some(conservative_var),
            measurement_sd: Some(conservative_sd),
            measurement_interval: Some((mean - half, mean + half)),
            rounding_interval: combined_rounding,
            var_model: "conservative_rho_1_unknown_dependence",
            assumption_to_narrow: Some(
                "declaring reviewed independence (NewObservation) or reviewed batch-group relations (SharedSystematicError) would license variance reduction below rho=1 bound (sigma_b^2 + sigma_eps^2)"
                    .to_string(),
            ),
            active_origin_ids,
            input_reading_count: readings.len(),
            collapsed_copies,
            unreviewed_assertions,
            retained_history,
            groups: vec![ResolvedGroupSummary {
                group_key: "unknown_dependence_rho_1".to_string(),
                active_observations: active_rows.len(),
                group_mean: mean,
                group_mean_var: conservative_var,
            }],
            relation_tags,
            result_digest,
        });
    }

    // 4. Exact block-diagonal batch-bias GLS: Σ = σ_ε²·I + σ_b²·Z·Zᵀ.
    //    Group `g` of size `n_g` has mean variance `v_g = σ_b² + σ_ε² / n_g`.
    //    Across independent groups: Var(μ̂) = (Σ_g 1/v_g)⁻¹, μ̂ = Var(μ̂) * Σ_g (ȳ_g / v_g).
    let mut group_values: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for r in &active_rows {
        let gkey = match &r.relation {
            OriginRelation::SharedSystematicError { group_id } => {
                format!("batch:{}", group_id.as_str())
            }
            OriginRelation::TechnicalReplicate { sample_id } => {
                format!("replicate_sample:{}", sample_id.as_str())
            }
            OriginRelation::NewObservation | OriginRelation::SupersedingPublication { .. } => {
                format!("independent:{}", r.origin_id.as_str())
            }
            OriginRelation::PublicationCopy { .. }
            | OriginRelation::RepeatedReading { .. }
            | OriginRelation::UnknownDependence => {
                format!("other:{}", r.origin_id.as_str())
            }
        };
        group_values.entry(gkey).or_default().push(r.value);
    }

    let mut groups = Vec::with_capacity(group_values.len());
    let mut total_precision = 0.0_f64;
    let mut weighted_sum = 0.0_f64;

    for (group_key, vals) in group_values {
        let n_g = vals.len() as f64;
        let g_mean = vals.iter().copied().sum::<f64>() / n_g;
        let v_g = var_b + var_eps / n_g;
        let w_g = 1.0 / v_g;
        total_precision += w_g;
        weighted_sum += w_g * g_mean;
        groups.push(ResolvedGroupSummary {
            group_key,
            active_observations: vals.len(),
            group_mean: g_mean,
            group_mean_var: v_g,
        });
    }

    let measurement_var = 1.0 / total_precision;
    let estimate = weighted_sum * measurement_var;
    let measurement_sd = sqrt(measurement_var);
    let half = z_crit * measurement_sd;

    Ok(OriginAccountingResult {
        knowledge_state: KnowledgeState::Estimated,
        estimate: Some(estimate),
        measurement_var: Some(measurement_var),
        measurement_sd: Some(measurement_sd),
        measurement_interval: Some((estimate - half, estimate + half)),
        rounding_interval: combined_rounding,
        var_model: "batch_bias_gls",
        assumption_to_narrow: None,
        active_origin_ids,
        input_reading_count: readings.len(),
        collapsed_copies,
        unreviewed_assertions,
        retained_history,
        groups,
        relation_tags,
        result_digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reviewed() -> RelationStatus {
        RelationStatus::Reviewed {
            reviewer_role: "qaqc-reviewer".to_string(),
        }
    }

    fn batch_reading(id: &str, origin: &str, val: f64, batch: &str) -> OriginReading {
        OriginReading {
            reading_id: id.to_string(),
            origin_id: OriginId::parse(origin).unwrap(),
            value: val,
            rounding_interval: None,
            relation: OriginRelation::SharedSystematicError {
                group_id: BatchGroupId::parse(batch).unwrap(),
            },
            status: reviewed(),
            vintage_rank: 1,
        }
    }

    fn copy_reading(
        id: &str,
        copy_origin: &str,
        of_origin: &str,
        val: f64,
        ri: Option<RoundingInterval>,
    ) -> OriginReading {
        OriginReading {
            reading_id: id.to_string(),
            origin_id: OriginId::parse(copy_origin).unwrap(),
            value: val,
            rounding_interval: ri,
            relation: OriginRelation::PublicationCopy {
                of_origin: OriginId::parse(of_origin).unwrap(),
            },
            status: reviewed(),
            vintage_rank: 1,
        }
    }

    #[test]
    fn test_bd352_worked_example_table_and_reprint_invariance() {
        let model = BatchBiasModel {
            sigma_b: 0.05,
            sigma_eps: 0.20,
            nominal_level: 0.95,
        };

        // Case 1: n = 4, one batch -> Var(mean) = 0.0025 + 0.04/4 = 0.0125 (sd = 0.111803...).
        let b1_4: Vec<OriginReading> = (0..4)
            .map(|i| batch_reading(&format!("r{i}"), &format!("assay-b1-{i}"), 2.50, "batch-1"))
            .collect();
        let res_4 = evaluate_origin_batch_model(&b1_4, model).expect("n=4 one batch");
        assert!((res_4.measurement_var.unwrap() - 0.0125).abs() < 1e-12);
        assert!((res_4.measurement_sd.unwrap() - 0.0125_f64.sqrt()).abs() < 1e-12);

        // Case 2: n = 16, one batch -> Var(mean) = 0.0025 + 0.04/16 = 0.005 (sd = 0.07071...).
        let b1_16: Vec<OriginReading> = (0..16)
            .map(|i| batch_reading(&format!("r{i}"), &format!("assay-b1-{i}"), 2.50, "batch-1"))
            .collect();
        let res_16 = evaluate_origin_batch_model(&b1_16, model).expect("n=16 one batch");
        assert!((res_16.measurement_var.unwrap() - 0.005).abs() < 1e-12);
        assert!((res_16.measurement_sd.unwrap() - 0.005_f64.sqrt()).abs() < 1e-12);

        // Case 3: n large (10,000), one batch -> approaches floor at σ_b² = 0.0025 (sd -> 0.05).
        let b1_many: Vec<OriginReading> = (0..10_000)
            .map(|i| batch_reading(&format!("r{i}"), &format!("assay-b1-{i}"), 2.50, "batch-1"))
            .collect();
        let res_many = evaluate_origin_batch_model(&b1_many, model).expect("n=10000 one batch");
        assert!(res_many.measurement_var.unwrap() > 0.0025);
        assert!((res_many.measurement_var.unwrap() - 0.0025).abs() < 1e-5);

        // Case 4: n = 4, each reprinted once (8 rows) -> Var(mean) MUST stay 0.0125 (never 0.005!).
        let mut with_reprints = b1_4.clone();
        for i in 0..4 {
            with_reprints.push(copy_reading(
                &format!("copy-{i}"),
                &format!("pub-copy-{i}"),
                &format!("assay-b1-{i}"),
                2.50,
                None,
            ));
        }
        let res_reprint =
            evaluate_origin_batch_model(&with_reprints, model).expect("n=4 reprinted");
        assert_eq!(res_reprint.collapsed_copies, 4);
        assert_eq!(res_reprint.active_origin_ids.len(), 4);
        assert!((res_reprint.measurement_var.unwrap() - 0.0125).abs() < 1e-12);
        assert_eq!(res_reprint.measurement_interval, res_4.measurement_interval);

        // Case 5: two independent batches of 4 -> Var(mean) = 0.00625 (sd = 0.0790569...).
        let mut two_batches = b1_4.clone();
        for i in 0..4 {
            two_batches.push(batch_reading(
                &format!("b2-{i}"),
                &format!("assay-b2-{i}"),
                2.50,
                "batch-2",
            ));
        }
        let res_two = evaluate_origin_batch_model(&two_batches, model).expect("two batches of 4");
        assert!((res_two.measurement_var.unwrap() - 0.00625).abs() < 1e-12);
        assert!((res_two.measurement_sd.unwrap() - 0.00625_f64.sqrt()).abs() < 1e-12);
        assert!(res_two.measurement_var.unwrap() < res_4.measurement_var.unwrap());

        // E2E structured log contains required keys {origins, relations, groups, var_model, interval}.
        let log = res_two.to_json_log();
        assert!(log.contains("\"origins\":["));
        assert!(log.contains("\"relations\":[\"shared_systematic_error\"]"));
        assert!(log.contains("\"groups\":["));
        assert!(log.contains("\"var_model\":\"batch_bias_gls\""));
        assert!(log.contains("\"interval\":["));
    }

    #[test]
    fn test_bd352_e698_rounding_vs_measurement_and_e692_superseding_and_conflict() {
        let model = BatchBiasModel {
            sigma_b: 0.05,
            sigma_eps: 0.20,
            nominal_level: 0.95,
        };

        // E698: two publications of one origin with different rounding conventions (deck & MD&A):
        // rounding interval intersects ([3.60, 3.70] ∩ [3.645, 3.655] = [3.645, 3.655]), while
        // measurement variance stays identical to a single observation (0.0025 + 0.04 = 0.0425).
        let deck = OriginReading {
            reading_id: "e698-deck".to_string(),
            origin_id: OriginId::parse("barrick-2025-rollforward-geo").unwrap(),
            value: 3.65,
            rounding_interval: Some(RoundingInterval::new(3.60, 3.70).unwrap()),
            relation: OriginRelation::NewObservation,
            status: reviewed(),
            vintage_rank: 1,
        };
        let mda_copy = copy_reading(
            "e698-mda",
            "barrick-2025-mda-copy",
            "barrick-2025-rollforward-geo",
            3.65,
            Some(RoundingInterval::new(3.645, 3.655).unwrap()),
        );
        let single = evaluate_origin_batch_model(std::slice::from_ref(&deck), model).unwrap();
        let both = evaluate_origin_batch_model(&[deck, mda_copy], model).unwrap();
        assert_eq!(both.measurement_var, single.measurement_var);
        assert_eq!(both.measurement_interval, single.measurement_interval);
        let ri = both
            .rounding_interval
            .expect("intersected rounding interval");
        assert!((ri.lo - 3.645).abs() < 1e-12 && (ri.hi - 3.655).abs() < 1e-12);

        // E692: superseding publication — 2024 GEO net change printed 6.7, superseded by 79;
        // current = 79 and the 6.7 row is retained in history.
        let orig_67 = OriginReading {
            reading_id: "e692-v1".to_string(),
            origin_id: OriginId::parse("barrick-2024-geo-net-change-v1").unwrap(),
            value: 6.7,
            rounding_interval: None,
            relation: OriginRelation::NewObservation,
            status: reviewed(),
            vintage_rank: 1,
        };
        let sup_79 = OriginReading {
            reading_id: "e692-v2".to_string(),
            origin_id: OriginId::parse("barrick-2024-geo-net-change-v2").unwrap(),
            value: 79.0,
            rounding_interval: None,
            relation: OriginRelation::SupersedingPublication {
                supersedes: OriginId::parse("barrick-2024-geo-net-change-v1").unwrap(),
                vintage_rank: 2,
            },
            status: reviewed(),
            vintage_rank: 2,
        };
        let e692 = evaluate_origin_batch_model(&[orig_67.clone(), sup_79.clone()], model).unwrap();
        assert_eq!(e692.knowledge_state, KnowledgeState::Estimated);
        assert!((e692.estimate.unwrap() - 79.0).abs() < 1e-12);
        assert_eq!(e692.retained_history.len(), 1);
        assert!((e692.retained_history[0].superseded_value - 6.7).abs() < 1e-12);

        // Negative control: conflicting superseding values at the same vintage rank stay Conflicted,
        // NEVER averaged.
        let sup_conflict = OriginReading {
            reading_id: "e692-v2-alt".to_string(),
            origin_id: OriginId::parse("barrick-2024-geo-net-change-v2b").unwrap(),
            value: 81.5,
            rounding_interval: None,
            relation: OriginRelation::SupersedingPublication {
                supersedes: OriginId::parse("barrick-2024-geo-net-change-v1").unwrap(),
                vintage_rank: 2,
            },
            status: reviewed(),
            vintage_rank: 2,
        };
        let conf = evaluate_origin_batch_model(&[orig_67, sup_79, sup_conflict], model).unwrap();
        assert_eq!(conf.knowledge_state, KnowledgeState::Conflicted);
        assert!(conf.estimate.is_none());
        assert!(conf.measurement_var.is_none());
    }

    #[test]
    fn test_bd352_unknown_dependence_replicates_reprints_permutation_and_canonical_identity() {
        let model = BatchBiasModel {
            sigma_b: 0.05,
            sigma_eps: 0.20,
            nominal_level: 0.95,
        };

        // 1. Unknown dependence refuses to assume rho = 0; reports conservative sigma_b^2 + sigma_eps^2.
        let unk: Vec<OriginReading> = (0..5)
            .map(|i| OriginReading {
                reading_id: format!("u{i}"),
                origin_id: OriginId::parse(&format!("unk-{i}")).unwrap(),
                value: 3.0 + 0.01 * (i as f64),
                rounding_interval: None,
                relation: OriginRelation::UnknownDependence,
                status: reviewed(),
                vintage_rank: 1,
            })
            .collect();
        let res_unk = evaluate_origin_batch_model(&unk, model).unwrap();
        assert_eq!(res_unk.var_model, "conservative_rho_1_unknown_dependence");
        assert!((res_unk.measurement_var.unwrap() - (0.0025 + 0.04)).abs() < 1e-12);
        assert!(res_unk.assumption_to_narrow.is_some());

        // 2. Positive control: legitimate same-sample TechnicalReplicate rows are NOT collapsed;
        //    they narrow the analytical component σ_ε²/n while keeping sample-prep floor σ_b².
        let reps: Vec<OriginReading> = (0..4)
            .map(|i| OriginReading {
                reading_id: format!("rep-{i}"),
                origin_id: OriginId::parse(&format!("sample-42-rep-{i}")).unwrap(),
                value: 1.80,
                rounding_interval: None,
                relation: OriginRelation::TechnicalReplicate {
                    sample_id: BatchGroupId::parse("sample-42").unwrap(),
                },
                status: reviewed(),
                vintage_rank: 1,
            })
            .collect();
        let res_rep = evaluate_origin_batch_model(&reps, model).unwrap();
        assert_eq!(res_rep.collapsed_copies, 0);
        assert_eq!(res_rep.active_origin_ids.len(), 4);
        assert!((res_rep.measurement_var.unwrap() - 0.0125).abs() < 1e-12);

        // 3. Property: duplicating any subset k times leaves measurement interval identical;
        //    permuting rows produces bit-identical results and result_digest.
        let mut with_k_copies = reps.clone();
        for k in 0..7 {
            with_k_copies.push(copy_reading(
                &format!("dup-{k}"),
                &format!("dup-origin-{k}"),
                "sample-42-rep-0",
                1.80,
                None,
            ));
        }
        let res_dup = evaluate_origin_batch_model(&with_k_copies, model).unwrap();
        assert_eq!(res_dup.measurement_interval, res_rep.measurement_interval);
        assert_eq!(res_dup.collapsed_copies, 7);

        let mut permuted = with_k_copies.clone();
        permuted.reverse();
        let res_perm = evaluate_origin_batch_model(&permuted, model).unwrap();
        assert_eq!(res_perm.result_digest, res_dup.result_digest);
        assert_eq!(res_perm.measurement_var, res_dup.measurement_var);
        assert_eq!(res_perm.to_json_log(), res_dup.to_json_log());

        // 4. Canonical identity: changing OriginId or Relation changes result_digest.
        let mut renamed = reps.clone();
        renamed[0].origin_id = OriginId::parse("sample-42-rep-renamed").unwrap();
        let res_renamed = evaluate_origin_batch_model(&renamed, model).unwrap();
        assert_ne!(res_renamed.result_digest, res_rep.result_digest);

        let mut relabeled = reps.clone();
        relabeled[0].relation = OriginRelation::NewObservation;
        let res_relabeled = evaluate_origin_batch_model(&relabeled, model).unwrap();
        assert_ne!(res_relabeled.result_digest, res_rep.result_digest);
        // Adding an independent batch/observation never increases variance.
        assert!(res_relabeled.measurement_var.unwrap() <= res_rep.measurement_var.unwrap());

        // 5. Unreviewed ProposedAssertion never auto-merges or narrows variance.
        let mut unreviewed_copy = reps.clone();
        unreviewed_copy[1].status = RelationStatus::ProposedAssertion;
        let res_unrev = evaluate_origin_batch_model(&unreviewed_copy, model).unwrap();
        assert_eq!(res_unrev.unreviewed_assertions, 1);
        assert_eq!(res_unrev.var_model, "conservative_rho_1_unknown_dependence");
        assert!(res_unrev.measurement_var.unwrap() >= res_rep.measurement_var.unwrap());
    }
}
