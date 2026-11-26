//! Determinism: BitwisePortable
//! Pilot Protocol Card, canonical `AnalysisPlan` hashing, pre-registration `ArtifactLedger`,
//! and early-kill / Markdown+JSON rendering (`BD-356.S3`, plan §37.3, `INV-002`, `INV-008`, `INV-010`).
//!
//! Builds directly on `plant_twin` (`BD-356.S1`) and `pilot_power` (`BD-356.S2`):
//! - Freezes and hashes the `AnalysisPlan` strictly before the first data request (`INV-008`).
//! - Refuses late registration (`FMINE-REF-202`) and post-freeze mutation (`FMINE-REF-507`).
//! - Never renders absolute power without a multi-twin `[min, max]` [`PowerEnvelope`].
//! - Evaluates the §37.3 early-kill rule recommending `"North Mara with Bulyanhulu, then Kibali"`
//!   when no feasible subset clears the power bar under every twin.
//! - Carries `SYNTHETIC_PUBLIC_LABEL` and all four `INV-002` authority flags unconditionally `false`.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use fmine_claim::render::NO_CLAIM_BANNER;
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::{Refusal, RefusalCode};

use crate::pilot_power::{
    AttributionCause, FileLabel, PowerEnvelope, estimate_power_envelope, label_candidate_files,
    reject_personal_data_files,
};
use crate::plant_twin::{PilotTwinRegistrySpec, SYNTHETIC_PUBLIC_LABEL, TwinFamily};

/// Alternate site recommendation specified in plan §37.3 when early kill fires.
pub const EARLY_KILL_ALTERNATE_SITES: &str = "North Mara with Bulyanhulu, then Kibali";

/// Artifact kind stored in [`ArtifactLedger`] (`BD-356.S3`, `INV-008`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FrozenArtifactKind {
    /// Pre-registered pilot analysis plan hashed prior to any site data request.
    AnalysisPlan,
}

impl FrozenArtifactKind {
    /// Stable canonical tag embedded inside the `FRZA` frame bytes.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::AnalysisPlan => "analysis_plan",
        }
    }
}

/// Immutable frozen artifact receipt recorded in [`ArtifactLedger`] (`INV-008`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenArtifact {
    pub artifact_id: ObjectId,
    pub kind: FrozenArtifactKind,
    pub digest: Digest32,
    pub registered_ns: i64,
    pub freeze_time_ns: i64,
}

/// Append-only pre-registration ledger for non-forecast frozen artifacts such as [`AnalysisPlan`]
/// (`BD-356.S3`, `INV-008`).
///
/// Reuses `PrefixKind::Forecast` (`for_...`) with the artifact kind tag inside the canonical bytes,
/// refusing late registration (`registered_ns >= freeze_time_ns`, `FMINE-REF-202`) and post-freeze
/// mutation or slot overwrite (`FMINE-REF-507`).
#[derive(Debug, Default, Clone)]
pub struct ArtifactLedger {
    by_id: BTreeMap<ObjectId, FrozenArtifact>,
    by_slot: BTreeMap<String, ObjectId>,
}

impl ArtifactLedger {
    /// Create an empty pre-registration ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Freeze and register `canonical_bytes` before `freeze_time_ns` (`INV-008`).
    ///
    /// Refuses:
    /// - empty bytes (`FMINE-REF-201`);
    /// - `registered_ns >= freeze_time_ns` (`FMINE-REF-202`);
    /// - re-registering an existing `artifact_id` (`FMINE-REF-507`).
    pub fn register(
        &mut self,
        kind: FrozenArtifactKind,
        canonical_bytes: &[u8],
        registered_ns: i64,
        freeze_time_ns: i64,
    ) -> Result<ObjectId, Refusal> {
        if canonical_bytes.is_empty() {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                "cannot register empty canonical artifact bytes",
            ));
        }
        if registered_ns >= freeze_time_ns {
            return Err(Refusal::new(
                RefusalCode::NotObservable202,
                format!(
                    "INV-008 late registration refused: registered_ns ({registered_ns}) must strictly precede freeze_time_ns ({freeze_time_ns})"
                ),
            ));
        }
        let digest = blake3_hash(canonical_bytes);
        let mut env = Encoder::new_frame(*b"FRZA", 1);
        env.put_str(kind.tag());
        env.put_fixed(digest.as_bytes());
        let artifact_id = ObjectId::from_canonical_bytes(PrefixKind::Forecast, &env.finish());
        if self.by_id.contains_key(&artifact_id) {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                format!("artifact {artifact_id} is already frozen and cannot be re-registered"),
            ));
        }
        self.by_id.insert(
            artifact_id,
            FrozenArtifact {
                artifact_id,
                kind,
                digest,
                registered_ns,
                freeze_time_ns,
            },
        );
        Ok(artifact_id)
    }

    /// Register a frozen artifact bound to a unique logical `slot_key` (e.g. `"lumwana/plan"`).
    /// Attempting to replace or mutate a frozen slot refuses with `FMINE-REF-507`.
    pub fn register_slot(
        &mut self,
        slot_key: &str,
        kind: FrozenArtifactKind,
        canonical_bytes: &[u8],
        registered_ns: i64,
        freeze_time_ns: i64,
    ) -> Result<ObjectId, Refusal> {
        if slot_key.trim().is_empty() {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "artifact ledger slot_key must be non-empty",
            ));
        }
        if let Some(existing_id) = self.by_slot.get(slot_key) {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                format!(
                    "frozen slot '{slot_key}' is already bound to {existing_id}; mutation refused"
                ),
            ));
        }
        let id = self.register(kind, canonical_bytes, registered_ns, freeze_time_ns)?;
        self.by_slot.insert(slot_key.to_string(), id);
        Ok(id)
    }

    /// Look up a frozen artifact record by id.
    #[must_use]
    pub fn get(&self, id: &ObjectId) -> Option<&FrozenArtifact> {
        self.by_id.get(id)
    }

    /// Whether a logical `slot_key` is already frozen in this ledger.
    #[must_use]
    pub fn contains_slot(&self, slot_key: &str) -> bool {
        self.by_slot.contains_key(slot_key)
    }

    /// Verify that `candidate_bytes` still match the frozen digest stored for `id`.
    /// Refuses unknown id with `FMINE-REF-201` and mutated bytes with `FMINE-REF-507`.
    pub fn verify_unmutated(&self, id: &ObjectId, candidate_bytes: &[u8]) -> Result<(), Refusal> {
        let Some(rec) = self.by_id.get(id) else {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                format!("artifact {id} is not registered in ArtifactLedger"),
            ));
        };
        let actual = blake3_hash(candidate_bytes);
        if actual != rec.digest {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                format!(
                    "artifact {id} mutated after freeze: expected digest {}, got {actual}",
                    rec.digest
                ),
            ));
        }
        Ok(())
    }
}

/// One pre-registered finding definition inside an [`AnalysisPlan`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FindingDefinition {
    pub name: String,
    pub definition: String,
}

/// Pre-registered pilot analysis plan (`BD-356.S3`, `INV-008`).
///
/// Canonical encoding sorts `finding_definitions`, `seeds`, and `null_criteria` before framing
/// through `Encoder::new_frame(*b"APLN", 1)` so field/element ordering never alters the digest.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisPlan {
    pub site: String,
    pub attribution_question: String,
    pub finding_definitions: Vec<FindingDefinition>,
    /// Materiality threshold in percentage points (e.g. `1.5` for 1.5 pp recovery gap).
    pub materiality_threshold_pct: f64,
    pub code_rev: String,
    pub seeds: Vec<u64>,
    pub stopping_rule: String,
    pub null_criteria: Vec<String>,
    /// Target statistical power bar (TARGET; sponsor-confirmed prior to GATE-DATA-01).
    pub power_bar_target: f64,
    pub early_kill_rule: String,
}

fn canonical_f64_bits(v: f64) -> u64 {
    if v == 0.0 {
        0.0_f64.to_bits()
    } else {
        v.to_bits()
    }
}

fn require_non_empty(field: &str, value: &str) -> Result<(), Refusal> {
    if value.trim().is_empty() {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!("pilot_card: '{field}' must be non-empty"),
        ));
    }
    Ok(())
}

impl AnalysisPlan {
    /// Encode all pre-registered fields canonically through [`Encoder`] (`INV-008`).
    ///
    /// Order-invariant over `finding_definitions`, `seeds`, and `null_criteria`.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, Refusal> {
        require_non_empty("site", &self.site)?;
        require_non_empty("attribution_question", &self.attribution_question)?;
        require_non_empty("code_rev", &self.code_rev)?;
        require_non_empty("stopping_rule", &self.stopping_rule)?;
        require_non_empty("early_kill_rule", &self.early_kill_rule)?;
        if self.finding_definitions.is_empty() {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                "AnalysisPlan requires at least one FindingDefinition",
            ));
        }
        if self.seeds.is_empty() {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                "AnalysisPlan requires at least one seed",
            ));
        }
        if self.null_criteria.is_empty() {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                "AnalysisPlan requires at least one null criterion",
            ));
        }
        if !(self.materiality_threshold_pct.is_finite() && self.materiality_threshold_pct > 0.0) {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "AnalysisPlan materiality_threshold_pct ({}) must be finite and > 0",
                    self.materiality_threshold_pct
                ),
            ));
        }
        if !(self.power_bar_target.is_finite()
            && self.power_bar_target > 0.0
            && self.power_bar_target <= 1.0)
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "AnalysisPlan power_bar_target ({}) must be finite and in (0, 1]",
                    self.power_bar_target
                ),
            ));
        }

        let mut defs: Vec<FindingDefinition> = Vec::with_capacity(self.finding_definitions.len());
        for d in &self.finding_definitions {
            require_non_empty("finding_definition.name", &d.name)?;
            require_non_empty("finding_definition.definition", &d.definition)?;
            defs.push(FindingDefinition {
                name: d.name.trim().to_string(),
                definition: d.definition.trim().to_string(),
            });
        }
        defs.sort();
        defs.dedup();

        let mut seeds = self.seeds.clone();
        seeds.sort_unstable();
        seeds.dedup();

        let mut nulls: Vec<String> = Vec::with_capacity(self.null_criteria.len());
        for n in &self.null_criteria {
            require_non_empty("null_criterion", n)?;
            nulls.push(n.trim().to_string());
        }
        nulls.sort();
        nulls.dedup();

        let mut enc = Encoder::new_frame(*b"APLN", 1);
        enc.put_str(self.site.trim());
        enc.put_str(self.attribution_question.trim());
        enc.put_u64(defs.len() as u64);
        for d in &defs {
            enc.put_str(&d.name);
            enc.put_str(&d.definition);
        }
        enc.put_u64(canonical_f64_bits(self.materiality_threshold_pct));
        enc.put_str(self.code_rev.trim());
        enc.put_u64(seeds.len() as u64);
        for s in &seeds {
            enc.put_u64(*s);
        }
        enc.put_str(self.stopping_rule.trim());
        enc.put_u64(nulls.len() as u64);
        for n in &nulls {
            enc.put_str(n);
        }
        enc.put_u64(canonical_f64_bits(self.power_bar_target));
        enc.put_str(self.early_kill_rule.trim());
        Ok(enc.finish())
    }

    /// Canonical BLAKE3 digest of this analysis plan (`INV-008`).
    pub fn digest(&self) -> Result<Digest32, Refusal> {
        Ok(blake3_hash(&self.canonical_bytes()?))
    }
}

/// One candidate file row on a [`PilotProtocolCard`].
#[derive(Debug, Clone, PartialEq)]
pub struct FileRow {
    pub name: String,
    pub reason: String,
    pub label: FileLabel,
    /// Multi-twin `[min, max]` power envelopes across target attribution causes.
    pub envelope_by_cause: Vec<PowerEnvelope>,
}

/// Early-kill evaluation carried explicitly on every [`PilotProtocolCard`] (§37.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarlyKillOutcome {
    pub fires: bool,
    pub reason: String,
    pub alternate_recommendation: Option<String>,
}

/// Per-site Pilot Protocol Card (`BD-356.S3`, plan §37.3).
#[derive(Debug, Clone, PartialEq)]
pub struct PilotProtocolCard {
    pub site: String,
    pub mechanism: String,
    pub attribution_question: String,
    pub k_twins: usize,
    pub files: Vec<FileRow>,
    pub analysis_plan_id: ObjectId,
    pub early_kill: EarlyKillOutcome,
    /// True when zero candidate files are `Robust` across all twins (card reduces to hashed plan
    /// plus per-file reasons, with no ranking fallback).
    pub envelope_too_wide_fallback: bool,
    pub inputs_digest: Digest32,
    pub seeds: Vec<u64>,
    pub code_rev: String,
    pub card_digest: Digest32,
    pub label: &'static str,
}

/// Compute canonical `inputs_digest` from an unordered slice of public input digests.
#[must_use]
pub fn compute_inputs_digest(public_input_digests: &[Digest32]) -> Digest32 {
    let mut sorted = public_input_digests.to_vec();
    sorted.sort();
    sorted.dedup();
    let mut enc = Encoder::new_frame(*b"PIND", 1);
    enc.put_u64(sorted.len() as u64);
    for d in &sorted {
        enc.put_fixed(d.as_bytes());
    }
    blake3_hash(&enc.finish())
}

/// Pure function (`INV-010`): `card_digest = blake3(canonical(inputs_digest, seeds, code_rev))`.
pub fn compute_card_digest(
    inputs_digest: Digest32,
    seeds: &[u64],
    code_rev: &str,
) -> Result<Digest32, Refusal> {
    require_non_empty("code_rev", code_rev)?;
    if seeds.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "compute_card_digest requires at least one seed",
        ));
    }
    let mut sorted_seeds = seeds.to_vec();
    sorted_seeds.sort_unstable();
    sorted_seeds.dedup();
    let mut enc = Encoder::new_frame(*b"PCRD", 1);
    enc.put_fixed(inputs_digest.as_bytes());
    enc.put_u64(sorted_seeds.len() as u64);
    for s in &sorted_seeds {
        enc.put_u64(*s);
    }
    enc.put_str(code_rev.trim());
    Ok(blake3_hash(&enc.finish()))
}

/// Evaluate the §37.3 early-kill rule: if no feasible subset clears `power_bar_target` across
/// EVERY twin (`min_power >= power_bar_target` and `identifiability_refusal.is_none()`) for every
/// required cause, fire early kill and recommend [`EARLY_KILL_ALTERNATE_SITES`].
#[must_use]
pub fn evaluate_early_kill(
    best_subset_envelopes: &[PowerEnvelope],
    power_bar_target: f64,
) -> EarlyKillOutcome {
    if !power_bar_target.is_finite() || power_bar_target <= 0.0 || power_bar_target > 1.0 {
        return EarlyKillOutcome {
            fires: true,
            reason: format!(
                "invalid power_bar_target ({power_bar_target}); fail-closed early kill recommends opening alternate site ({EARLY_KILL_ALTERNATE_SITES})"
            ),
            alternate_recommendation: Some(EARLY_KILL_ALTERNATE_SITES.to_string()),
        };
    }
    if best_subset_envelopes.is_empty() {
        return EarlyKillOutcome {
            fires: true,
            reason: format!(
                "no feasible candidate file subset evaluated; recommend opening alternate site at month 1 ({EARLY_KILL_ALTERNATE_SITES})"
            ),
            alternate_recommendation: Some(EARLY_KILL_ALTERNATE_SITES.to_string()),
        };
    }
    let mut failing = Vec::new();
    for env in best_subset_envelopes {
        if env.twins_evaluated < 2
            || !env.min_power.is_finite()
            || !env.max_power.is_finite()
            || env.identifiability_refusal.is_some()
            || env.min_power < power_bar_target
        {
            failing.push(format!(
                "{} ([{:.3}, {:.3}], k={} < bar {:.3})",
                env.cause.as_str(),
                env.min_power,
                env.max_power,
                env.twins_evaluated,
                power_bar_target
            ));
        }
    }
    if failing.is_empty() {
        EarlyKillOutcome {
            fires: false,
            reason: format!(
                "feasible subset clears power bar {:.3} across every twin on all {} causes",
                power_bar_target,
                best_subset_envelopes.len()
            ),
            alternate_recommendation: None,
        }
    } else {
        EarlyKillOutcome {
            fires: true,
            reason: format!(
                "no feasible subset clears power bar {:.3} under every twin ({}); recommend opening alternate site at month 1 ({EARLY_KILL_ALTERNATE_SITES})",
                power_bar_target,
                failing.join("; ")
            ),
            alternate_recommendation: Some(EARLY_KILL_ALTERNATE_SITES.to_string()),
        }
    }
}

fn sanitize_inline(s: &str) -> String {
    s.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

fn escape_md_cell(s: &str) -> String {
    sanitize_inline(s).replace('|', "\\|")
}

fn escape_json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

impl PilotProtocolCard {
    /// Validate card invariants before rendering or export:
    /// - non-empty metadata and non-empty candidate file list;
    /// - zero personal/roster files (`FMINE-REF-301`);
    /// - every file carries a non-empty reason and a multi-twin (`twins_evaluated >= 2`) envelope
    ///   (`FMINE-REF-109` if any envelope is missing or single-twin);
    /// - `card_digest` matches `compute_card_digest(inputs_digest, &seeds, &code_rev)` (`FMINE-REF-505`).
    pub fn validate(&self) -> Result<(), Refusal> {
        require_non_empty("site", &self.site)?;
        require_non_empty("mechanism", &self.mechanism)?;
        require_non_empty("attribution_question", &self.attribution_question)?;
        require_non_empty("code_rev", &self.code_rev)?;
        if self.k_twins < 2 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "PilotProtocolCard requires k_twins >= 2 for envelope reporting (got {})",
                    self.k_twins
                ),
            ));
        }
        if self.files.is_empty() {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                "PilotProtocolCard requires at least one candidate FileRow",
            ));
        }
        let file_names: Vec<&str> = self.files.iter().map(|f| f.name.as_str()).collect();
        reject_personal_data_files(&file_names)?;
        for row in &self.files {
            require_non_empty("file.name", &row.name)?;
            require_non_empty("file.reason", &row.reason)?;
            if row.envelope_by_cause.is_empty() {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!(
                        "candidate file '{}' has no [min, max] PowerEnvelope; bare power never permitted",
                        row.name
                    ),
                ));
            }
            for env in &row.envelope_by_cause {
                if env.twins_evaluated < 2 {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!(
                            "candidate file '{}' cause '{}' has twins_evaluated={} (< 2); cannot report power without multi-twin envelope",
                            row.name,
                            env.cause.as_str(),
                            env.twins_evaluated
                        ),
                    ));
                }
                let _ = env.render_envelope_string()?;
            }
        }
        let expected_digest = compute_card_digest(self.inputs_digest, &self.seeds, &self.code_rev)?;
        if self.card_digest != expected_digest {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "card_digest mismatch: expected {expected_digest}, got {}",
                    self.card_digest
                ),
            ));
        }
        if self.label != SYNTHETIC_PUBLIC_LABEL {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "PilotProtocolCard must carry canonical SYNTHETIC_PUBLIC_LABEL",
            ));
        }
        Ok(())
    }

    /// Render §30.1 structured JSON events (`pilot_power` per `(file, cause)` plus final `pilot_card`
    /// carrying all four `INV-002` flags `false` and `SYNTHETIC_PUBLIC_LABEL`).
    pub fn render_json_events(&self) -> Result<Vec<String>, Refusal> {
        self.validate()?;
        let mut events = Vec::new();
        let mut robust_count = 0usize;
        let mut model_dep_count = 0usize;

        for row in &self.files {
            let label_tag = match &row.label {
                FileLabel::Robust => {
                    robust_count += 1;
                    "robust"
                }
                FileLabel::ModelDependent { .. } => {
                    model_dep_count += 1;
                    "model-dependent"
                }
            };
            for env in &row.envelope_by_cause {
                let subset_str = env.subset.join(",");
                events.push(format!(
                    "{{\"event\":\"pilot_power\",\"site\":\"{}\",\"twin\":\"envelope(k={})\",\"subset\":\"{}\",\"cause\":\"{}\",\"power\":[{:.4},{:.4}],\"mc_se\":{:.4},\"label\":\"{}\"}}",
                    escape_json_str(&self.site),
                    env.twins_evaluated,
                    escape_json_str(&subset_str),
                    env.cause.as_str(),
                    env.min_power,
                    env.max_power,
                    env.max_mc_se,
                    label_tag,
                ));
            }
        }

        let alt_json = match &self.early_kill.alternate_recommendation {
            Some(a) => format!("\"{}\"", escape_json_str(a)),
            None => "null".to_string(),
        };
        events.push(format!(
            "{{\"event\":\"pilot_card\",\"site\":\"{}\",\"k\":{},\"files\":{},\"robust\":{},\"model_dependent\":{},\"early_kill\":{},\"early_kill_reason\":\"{}\",\"alternate\":{},\"envelope_too_wide_fallback\":{},\"plan_id\":\"{}\",\"inputs_digest\":\"{}\",\"code_rev\":\"{}\",\"card_digest\":\"{}\",\"label\":\"{}\",\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}}",
            escape_json_str(&self.site),
            self.k_twins,
            self.files.len(),
            robust_count,
            model_dep_count,
            self.early_kill.fires,
            escape_json_str(&self.early_kill.reason),
            alt_json,
            self.envelope_too_wide_fallback,
            self.analysis_plan_id,
            self.inputs_digest,
            escape_json_str(&self.code_rev),
            self.card_digest,
            escape_json_str(self.label),
        ));
        Ok(events)
    }

    /// Render deterministic LF-terminated Markdown for sponsor review prior to `GATE-DATA-01`.
    pub fn render_markdown(&self) -> Result<String, Refusal> {
        self.validate()?;
        let mut md = String::new();
        let _ = writeln!(
            md,
            "# Pilot Protocol Card — {}",
            sanitize_inline(&self.site)
        );
        let _ = writeln!(md);
        let _ = writeln!(md, "- **Disclaimer:** {}", self.label);
        let _ = writeln!(md, "- **Mechanism:** {}", sanitize_inline(&self.mechanism));
        let _ = writeln!(
            md,
            "- **Pre-registered attribution question:** {}",
            sanitize_inline(&self.attribution_question)
        );
        let _ = writeln!(md, "- **Analysis plan ID:** `{}`", self.analysis_plan_id);
        let _ = writeln!(md, "- **Card digest (BLAKE3):** `{}`", self.card_digest);
        let _ = writeln!(md, "- **Inputs digest (BLAKE3):** `{}`", self.inputs_digest);
        let _ = writeln!(
            md,
            "- **Code revision:** `{}`",
            sanitize_inline(&self.code_rev)
        );
        let _ = writeln!(md, "- **Twin family size:** `K = {}`", self.k_twins);
        let _ = writeln!(md);
        let _ = writeln!(md, "> {NO_CLAIM_BANNER}");
        let _ = writeln!(md);
        let _ = writeln!(
            md,
            "| Authority Flag (`INV-002`) | Value |\n|---|---|\n\
             | `reserve_disclosure_claim` | `false` |\n\
             | `execution_authorized` | `false` |\n\
             | `geotechnical_signoff` | `false` |\n\
             | `regulatory_compliance_claim` | `false` |"
        );

        let _ = writeln!(md, "\n## Candidate File Ask\n");
        if self.envelope_too_wide_fallback {
            let _ = writeln!(
                md,
                "_Fallback active: no candidate file is robust across all twins; showing hashed plan plus per-file reasons with no ranking fallback._\n"
            );
        }
        let _ = writeln!(
            md,
            "| File | Label | Reason | Power Envelopes `[min, max]` |"
        );
        let _ = writeln!(md, "|---|---|---|---|");
        for row in &self.files {
            let label_text = match &row.label {
                FileLabel::Robust => "robust".to_string(),
                FileLabel::ModelDependent { reason } => format!("model-dependent ({reason})"),
            };
            let mut env_parts = Vec::with_capacity(row.envelope_by_cause.len());
            for env in &row.envelope_by_cause {
                env_parts.push(env.render_envelope_string()?);
            }
            let _ = writeln!(
                md,
                "| `{}` | {} | {} | {} |",
                escape_md_cell(&row.name),
                escape_md_cell(&label_text),
                escape_md_cell(&row.reason),
                escape_md_cell(&env_parts.join("; ")),
            );
        }

        let _ = writeln!(md, "\n## Early-Kill Rule\n");
        let _ = writeln!(md, "- **Early kill fires:** `{}`", self.early_kill.fires);
        let _ = writeln!(
            md,
            "- **Outcome:** {}",
            sanitize_inline(&self.early_kill.reason)
        );
        if let Some(alt) = &self.early_kill.alternate_recommendation {
            let _ = writeln!(
                md,
                "- **Alternate site recommendation (§37.3):** {}",
                sanitize_inline(alt)
            );
        }
        Ok(md)
    }
}

/// Build and pre-register a complete [`PilotProtocolCard`] from a [`TwinFamily`] (`S1`),
/// [`PilotTwinRegistrySpec`], and [`AnalysisPlan`], enforcing `INV-008` freeze order,
/// `S2` multi-twin power envelopes + robustness labels, and §37.3 early-kill evaluation.
///
/// Atomic with respect to `ledger`: all validations and power computations run before
/// registering the slot, so a refused build never mutates `ledger`.
#[allow(clippy::too_many_arguments)]
pub fn build_pilot_protocol_card(
    family: &TwinFamily,
    spec: &PilotTwinRegistrySpec,
    mechanism: &str,
    plan: &AnalysisPlan,
    ledger: &mut ArtifactLedger,
    registered_ns: i64,
    first_request_ns: i64,
    public_input_digests: &[Digest32],
    replicates: usize,
    seed_root: u64,
    top_n: usize,
) -> Result<PilotProtocolCard, Refusal> {
    if family.twins.len() < 2 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "build_pilot_protocol_card requires a TwinFamily with at least 2 twins",
        ));
    }
    require_non_empty("mechanism", mechanism)?;
    let plan_bytes = plan.canonical_bytes()?;
    let site = plan.site.trim();
    if site != spec.site.trim() || site != family.site.trim() {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!(
                "site mismatch across plan ('{site}'), spec ('{}'), and twin family ('{}')",
                spec.site, family.site
            ),
        ));
    }
    if !plan.seeds.contains(&seed_root) {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!(
                "simulation seed_root ({seed_root}) must be declared in AnalysisPlan.seeds so card_digest binds it"
            ),
        ));
    }
    if registered_ns >= first_request_ns {
        return Err(Refusal::new(
            RefusalCode::NotObservable202,
            format!(
                "INV-008 late registration refused: registered_ns ({registered_ns}) must strictly precede first_request_ns ({first_request_ns})"
            ),
        ));
    }
    let slot_key = format!("{site}/analysis_plan");
    if ledger.contains_slot(&slot_key) {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            format!("frozen slot '{slot_key}' is already registered; mutation refused"),
        ));
    }

    let labelled = label_candidate_files(family, &spec.candidate_files, top_n)?;
    let desc_map: BTreeMap<&str, &str> = spec
        .candidate_files
        .iter()
        .map(|c| (c.name.as_str(), c.description.as_str()))
        .collect();

    let mut files = Vec::with_capacity(labelled.len());
    for item in labelled {
        let subset = [item.name.as_str()];
        let mut envelopes = Vec::with_capacity(AttributionCause::ALL.len());
        for cause in AttributionCause::ALL {
            envelopes.push(estimate_power_envelope(
                family, &subset, cause, replicates, seed_root,
            )?);
        }
        let desc = desc_map.get(item.name.as_str()).copied().unwrap_or("");
        let reason = if desc.trim().is_empty() {
            format!(
                "selected in {}/{} twins for '{}' recovery attribution",
                item.selected_in_twins, item.total_twins, spec.site
            )
        } else {
            format!(
                "{desc} (selected in {}/{} twins)",
                item.selected_in_twins, item.total_twins
            )
        };
        files.push(FileRow {
            name: item.name,
            reason,
            label: item.label,
            envelope_by_cause: envelopes,
        });
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));

    let envelope_too_wide_fallback = !files.iter().any(|f| f.label == FileLabel::Robust);

    // Evaluate early kill on the full feasible candidate set (if even all feasible candidate files
    // cannot clear `power_bar_target` under every twin, no subset can).
    let all_names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
    let mut full_envelopes = Vec::with_capacity(AttributionCause::ALL.len());
    for cause in AttributionCause::ALL {
        full_envelopes.push(estimate_power_envelope(
            family, &all_names, cause, replicates, seed_root,
        )?);
    }
    let early_kill = evaluate_early_kill(&full_envelopes, plan.power_bar_target);

    let mut all_input_digests = public_input_digests.to_vec();
    all_input_digests.push(family.family_digest);
    let inputs_digest = compute_inputs_digest(&all_input_digests);

    let mut sorted_seeds = plan.seeds.clone();
    sorted_seeds.sort_unstable();
    sorted_seeds.dedup();
    let card_digest = compute_card_digest(inputs_digest, &sorted_seeds, &plan.code_rev)?;

    // Register in ledger only after every validation and simulation succeeded (atomic on refusal).
    let analysis_plan_id = ledger.register_slot(
        &slot_key,
        FrozenArtifactKind::AnalysisPlan,
        &plan_bytes,
        registered_ns,
        first_request_ns,
    )?;

    let card = PilotProtocolCard {
        site: site.to_string(),
        mechanism: mechanism.trim().to_string(),
        attribution_question: plan.attribution_question.trim().to_string(),
        k_twins: family.twins.len(),
        files,
        analysis_plan_id,
        early_kill,
        envelope_too_wide_fallback,
        inputs_digest,
        seeds: sorted_seeds,
        code_rev: plan.code_rev.trim().to_string(),
        card_digest,
        label: SYNTHETIC_PUBLIC_LABEL,
    };
    card.validate()?;
    Ok(card)
}

/// North Mara grade-led alternate Pilot Protocol Card (`BD-356.S4`, plan §37.3, §34.4,
/// `E456`, `E493–E495`, `E527`, `E587–E589`, `E597`, `E819`).
///
/// Reuses `BD-356.S3`'s pre-registration `ArtifactLedger`, `AnalysisPlan`, and pure `card_digest`
/// machinery while keeping North Mara's grade-led twins completely separate from Lumwana's
/// recovery-led twins:
/// - 100%-basis quarterly parity (Mine Stats column B, `E456`);
/// - 84% attributable figures strictly derived (`84/100`), NEVER fitted;
/// - attributable-share conflicts (e.g. `E819` Q2 2026 CoS/oz implied share `[0.8498, 0.8808]`)
///   explicitly reported, NEVER forced;
/// - Tanzania Mining Act s.35(6) residency (`E527`, `E597`) validated fail-closed (`FMINE-REF-303`);
/// - no affiliation implied with the issuer or with the Government of Tanzania's Twiga interest.
pub mod north_mara {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fmt::Write as _;
    use std::path::Path;

    use fmine_claim::render::NO_CLAIM_BANNER;
    use fmine_core::codec::Encoder;
    use fmine_core::det::{norm_ppf, sqrt};
    use fmine_core::hash::{Digest32, sha256};
    use fmine_core::id::ObjectId;
    use fmine_core::rand::{PhiloxStream, RngKey};
    use fmine_evidence::{Refusal, RefusalCode};
    use fmine_public::mine_stats::{
        LabelVocabulary, MineStatsCell, MineStatsRead, View, read_mine_stats,
    };
    use fmine_public::sources::SourceKind;

    use super::{
        AnalysisPlan, ArtifactLedger, EarlyKillOutcome, FrozenArtifactKind, canonical_f64_bits,
        compute_card_digest, compute_inputs_digest, escape_json_str, escape_md_cell,
        require_non_empty, sanitize_inline,
    };
    use crate::pilot_power::{
        FileLabel, LinearBiasObs, reject_personal_data_files, solve_two_bias_posterior,
        subset_stream_brick,
    };
    use crate::plant_twin::{
        CandidateFileCostRow, CitedPoint, CitedSeries, SYNTHETIC_PUBLIC_LABEL, SegmentBoundary,
    };

    /// Canonical site identifier for North Mara.
    pub const NORTH_MARA_SITE: &str = "north_mara";

    /// Next §37.3 alternate pilot site when North Mara itself fires the early-kill rule.
    pub const NORTH_MARA_EARLY_KILL_ALTERNATE: &str = "Kibali";

    /// Stated Barrick ownership share at North Mara (`84%`; derived strictly from 100%-basis twins,
    /// NEVER fitted to attributable cells).
    pub const NORTH_MARA_OWNERSHIP_PCT: u32 = 84;

    /// Grade-led mechanism declaration (`BD-356.S4`).
    pub const NORTH_MARA_MECHANISM: &str =
        "grade-led (plan against actual, on grade and ore delivery)";

    /// Mandatory Tanzania data-residency and non-affiliation statement (`E527`, `E597`, `BD-356.S4`).
    pub const TANZANIA_RESIDENCY_STATEMENT: &str = "Residency (Tanzania Mining Act s.35(6), E527; cross-border personal data, E597): Geological Survey authorisation is required to export \"any other data collected\"; the FrankenMining node stays in Tanzania, raw operational data remain on the in-country node, cross-border personal data require a permit, and rosters/personal data are never listed. No affiliation is implied with the issuer or with the Government of Tanzania's Twiga interest.";

    /// Four unidentifiable axes for North Mara's grade-led twin family (`BD-356.S4`).
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct GradeLedAxes {
        /// Fraction of model bias attributed to grade-control vs. resource model (`∈ [0, 1]`).
        pub gc_vs_resource_bias_split: f64,
        /// Unidentifiable dilution and ore-loss fraction (`∈ [0, 0.5]`).
        pub dilution_ore_loss_ratio: f64,
        /// Survey and stockpile reconciliation error standard deviation in percent (`> 0`).
        pub survey_stockpile_error_pct: f64,
        /// Mill sampler or assay bias standard deviation in percent (`> 0`).
        pub sampler_assay_bias_pct: f64,
    }

    /// Registered `[min, max]` ranges for North Mara's four grade-led twin axes.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct GradeLedAxisRanges {
        pub gc_vs_resource_bias_split: (f64, f64),
        pub dilution_ore_loss_ratio: (f64, f64),
        pub survey_stockpile_error_pct: (f64, f64),
        pub sampler_assay_bias_pct: (f64, f64),
    }

    impl GradeLedAxisRanges {
        fn validate(&self) -> Result<(), Refusal> {
            let ok = |lo: f64, hi: f64, lb: f64, ub: f64| {
                lo.is_finite() && hi.is_finite() && lo >= lb && hi <= ub && lo <= hi
            };
            if !ok(
                self.gc_vs_resource_bias_split.0,
                self.gc_vs_resource_bias_split.1,
                0.0,
                1.0,
            ) || !ok(
                self.dilution_ore_loss_ratio.0,
                self.dilution_ore_loss_ratio.1,
                0.0,
                0.5,
            ) || !ok(
                self.survey_stockpile_error_pct.0,
                self.survey_stockpile_error_pct.1,
                0.05,
                25.0,
            ) || !ok(
                self.sampler_assay_bias_pct.0,
                self.sampler_assay_bias_pct.1,
                0.05,
                25.0,
            ) {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    "invalid GradeLedAxisRanges for north_mara in pilot_twins.toml",
                ));
            }
            Ok(())
        }

        #[must_use]
        pub fn contains(&self, axes: GradeLedAxes) -> bool {
            (self.gc_vs_resource_bias_split.0..=self.gc_vs_resource_bias_split.1)
                .contains(&axes.gc_vs_resource_bias_split)
                && (self.dilution_ore_loss_ratio.0..=self.dilution_ore_loss_ratio.1)
                    .contains(&axes.dilution_ore_loss_ratio)
                && (self.survey_stockpile_error_pct.0..=self.survey_stockpile_error_pct.1)
                    .contains(&axes.survey_stockpile_error_pct)
                && (self.sampler_assay_bias_pct.0..=self.sampler_assay_bias_pct.1)
                    .contains(&axes.sampler_assay_bias_pct)
        }
    }

    /// Six seeded grade-led causes for North Mara (`BD-356.S4`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum GradeLedCause {
        GcModelBias,
        ResourceModelBias,
        DilutionOreLoss,
        OreDeliveryShortfall,
        SurveyStockpileError,
        SamplerAssayBias,
    }

    impl GradeLedCause {
        pub const ALL: [Self; 6] = [
            Self::GcModelBias,
            Self::ResourceModelBias,
            Self::DilutionOreLoss,
            Self::OreDeliveryShortfall,
            Self::SurveyStockpileError,
            Self::SamplerAssayBias,
        ];

        #[must_use]
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::GcModelBias => "gc_model_bias",
                Self::ResourceModelBias => "resource_model_bias",
                Self::DilutionOreLoss => "dilution_and_ore_loss",
                Self::OreDeliveryShortfall => "ore_delivery_shortfall",
                Self::SurveyStockpileError => "survey_and_stockpile_error",
                Self::SamplerAssayBias => "sampler_or_assay_bias",
            }
        }
    }

    /// North Mara grade-led registry specification parsed from `registries/pilot_twins.toml`.
    #[derive(Debug, Clone, PartialEq)]
    pub struct GradeLedRegistrySpec {
        pub site: String,
        pub ownership_pct: u32,
        pub metal_unit: String,
        pub mechanism: String,
        pub k: usize,
        pub status: String,
        pub axis_ranges: GradeLedAxisRanges,
        pub power_bar_target: f64,
        pub segment_boundaries: Vec<SegmentBoundary>,
        pub candidate_files: Vec<CandidateFileCostRow>,
    }

    /// Parse and validate the `north_mara` grade-led site specification from `registries/pilot_twins.toml`.
    pub fn parse_north_mara_registry(text: &str) -> Result<GradeLedRegistrySpec, Refusal> {
        let mut saw_version = false;
        let mut section = "";
        let mut cur: Vec<(String, String)> = Vec::new();
        let mut site_rows: Vec<Vec<(String, String)>> = Vec::new();
        let mut boundary_rows: Vec<Vec<(String, String)>> = Vec::new();
        let mut file_rows: Vec<Vec<(String, String)>> = Vec::new();

        let push_cur = |sec: &str,
                        fields: &mut Vec<(String, String)>,
                        sites: &mut Vec<Vec<(String, String)>>,
                        bounds: &mut Vec<Vec<(String, String)>>,
                        files: &mut Vec<Vec<(String, String)>>| {
            if fields.is_empty() {
                return;
            }
            let taken = core::mem::take(fields);
            match sec {
                "site_twin" => sites.push(taken),
                "segment_boundary" => bounds.push(taken),
                "candidate_file" => files.push(taken),
                _ => {}
            }
        };

        for (idx, raw) in text.lines().enumerate() {
            let line = idx + 1;
            let s = raw.trim();
            if s.is_empty() || s.starts_with('#') {
                continue;
            }
            if s == "[[site_twin]]" || s == "[[segment_boundary]]" || s == "[[candidate_file]]" {
                if !saw_version {
                    return Err(Refusal::new(
                        RefusalCode::UndeclaredUnitBasisOrTz107,
                        format!("pilot_twins.toml line {line}: schema_version = 1 required first"),
                    ));
                }
                push_cur(
                    section,
                    &mut cur,
                    &mut site_rows,
                    &mut boundary_rows,
                    &mut file_rows,
                );
                section = match s {
                    "[[site_twin]]" => "site_twin",
                    "[[segment_boundary]]" => "segment_boundary",
                    _ => "candidate_file",
                };
                continue;
            }
            let Some((k, v)) = s.split_once('=') else {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!("pilot_twins.toml line {line}: expected key = value"),
                ));
            };
            let key = k.trim();
            let val = v.trim();
            if section.is_empty() {
                if !saw_version && key == "schema_version" && val == "1" {
                    saw_version = true;
                } else {
                    return Err(Refusal::new(
                        RefusalCode::UndeclaredUnitBasisOrTz107,
                        format!("pilot_twins.toml line {line}: expected single schema_version = 1"),
                    ));
                }
            } else {
                if cur.iter().any(|(fk, _)| fk == key) {
                    return Err(Refusal::new(
                        RefusalCode::UndeclaredUnitBasisOrTz107,
                        format!(
                            "pilot_twins.toml line {line}: duplicate key '{key}' in [[{section}]]"
                        ),
                    ));
                }
                let parsed = val
                    .strip_prefix('"')
                    .and_then(|x| x.strip_suffix('"'))
                    .map_or_else(|| val.to_string(), ToString::to_string);
                cur.push((key.to_string(), parsed));
            }
        }
        push_cur(
            section,
            &mut cur,
            &mut site_rows,
            &mut boundary_rows,
            &mut file_rows,
        );

        let get = |fields: &[(String, String)], k: &str| -> Result<String, Refusal> {
            fields
                .iter()
                .find(|(fk, _)| fk == k)
                .map(|(_, v)| v.clone())
                .ok_or_else(|| {
                    Refusal::new(
                        RefusalCode::UndeclaredUnitBasisOrTz107,
                        format!("pilot_twins.toml missing field '{k}' for north_mara"),
                    )
                })
        };
        let get_f64 = |fields: &[(String, String)], k: &str| -> Result<f64, Refusal> {
            get(fields, k)?.parse::<f64>().map_err(|_| {
                Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("pilot_twins.toml invalid float for '{k}'"),
                )
            })
        };

        let matching: Vec<&Vec<(String, String)>> = site_rows
            .iter()
            .filter(|f| f.iter().any(|(k, v)| k == "site" && v == NORTH_MARA_SITE))
            .collect();
        if matching.is_empty() {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                "site 'north_mara' not found in pilot_twins.toml",
            ));
        }
        if matching.len() > 1 {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "duplicate [[site_twin]] entry for site 'north_mara' in pilot_twins.toml",
            ));
        }
        let sf = matching[0];

        let ownership_pct: u32 = get(sf, "ownership_pct")?.parse().map_err(|_| {
            Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "invalid ownership_pct for north_mara",
            )
        })?;
        if ownership_pct != NORTH_MARA_OWNERSHIP_PCT {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                format!(
                    "north_mara ownership_pct must be {NORTH_MARA_OWNERSHIP_PCT} (got {ownership_pct})"
                ),
            ));
        }
        let mechanism = get(sf, "mechanism")?;
        if mechanism != "grade_led" {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                format!("north_mara mechanism must be 'grade_led' (got '{mechanism}')"),
            ));
        }
        let metal_unit = get(sf, "metal_unit")?;
        if metal_unit != "koz" {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                format!("north_mara metal_unit must be 'koz' (got '{metal_unit}')"),
            ));
        }
        let k: usize = get(sf, "k")?
            .parse()
            .map_err(|_| Refusal::new(RefusalCode::OutsidePhysicalDomain109, "invalid k"))?;
        if k < 2 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("north_mara twin family requires k >= 2 (got {k})"),
            ));
        }
        let power_bar_target = get_f64(sf, "power_bar_target")?;
        if !(power_bar_target.is_finite() && power_bar_target > 0.0 && power_bar_target <= 1.0) {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("north_mara power_bar_target ({power_bar_target}) must lie in (0, 1]"),
            ));
        }

        let axis_ranges = GradeLedAxisRanges {
            gc_vs_resource_bias_split: (
                get_f64(sf, "gc_vs_resource_bias_split_min")?,
                get_f64(sf, "gc_vs_resource_bias_split_max")?,
            ),
            dilution_ore_loss_ratio: (
                get_f64(sf, "dilution_ore_loss_ratio_min")?,
                get_f64(sf, "dilution_ore_loss_ratio_max")?,
            ),
            survey_stockpile_error_pct: (
                get_f64(sf, "survey_stockpile_error_pct_min")?,
                get_f64(sf, "survey_stockpile_error_pct_max")?,
            ),
            sampler_assay_bias_pct: (
                get_f64(sf, "sampler_assay_bias_pct_min")?,
                get_f64(sf, "sampler_assay_bias_pct_max")?,
            ),
        };
        axis_ranges.validate()?;

        let mut segment_boundaries = Vec::new();
        for bf in &boundary_rows {
            let site = get(bf, "site")?;
            if site == NORTH_MARA_SITE {
                segment_boundaries.push(SegmentBoundary {
                    site,
                    quarter: get(bf, "quarter")?,
                    event: get(bf, "event")?,
                    source_e_id: get(bf, "source_e_id")?,
                });
            }
        }

        let mut candidate_files = Vec::new();
        let mut seen_names = BTreeSet::new();
        for ff in &file_rows {
            let site = get(ff, "site")?;
            if site == NORTH_MARA_SITE {
                let name = get(ff, "name")?;
                if name.is_empty() || !seen_names.insert(name.clone()) {
                    return Err(Refusal::new(
                        RefusalCode::UndeclaredUnitBasisOrTz107,
                        format!("empty or duplicate candidate_file '{name}' for north_mara"),
                    ));
                }
                let extraction_cost = get_f64(ff, "extraction_cost")?;
                if !(extraction_cost.is_finite() && extraction_cost > 0.0) {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!("candidate_file '{name}' requires finite extraction_cost > 0"),
                    ));
                }
                candidate_files.push(CandidateFileCostRow {
                    site,
                    name,
                    extraction_cost,
                    status: get(ff, "status")?,
                    description: get(ff, "description")?,
                });
            }
        }
        let file_names: Vec<&str> = candidate_files.iter().map(|c| c.name.as_str()).collect();
        reject_personal_data_files(&file_names)?;

        Ok(GradeLedRegistrySpec {
            site: NORTH_MARA_SITE.to_string(),
            ownership_pct,
            metal_unit,
            mechanism,
            k,
            status: get(sf, "status")?,
            axis_ranges,
            power_bar_target,
            segment_boundaries,
            candidate_files,
        })
    }

    /// 100%-basis quarterly target for North Mara (Mine Stats column B, `E456`).
    ///
    /// Structurally carries ONLY 100%-basis inputs so attributable figures are ALWAYS derived from
    /// `NORTH_MARA_OWNERSHIP_PCT` (84%) and can NEVER be fitted.
    #[derive(Debug, Clone, PartialEq)]
    pub struct NorthMaraQuarterTarget {
        pub quarter: String,
        pub ore_processed_kt: f64,
        pub head_grade_g_t: f64,
        pub recovery_pct: f64,
        pub gold_produced_100_koz: f64,
        pub aisc_per_oz: f64,
    }

    impl NorthMaraQuarterTarget {
        /// Implied tails grade `head_grade * (1 - recovery / 100)` (`E589`: grade-driven recovery).
        #[must_use]
        pub fn implied_tails_g_t(&self) -> f64 {
            self.head_grade_g_t * (1.0 - self.recovery_pct / 100.0)
        }
    }

    #[derive(Clone, Copy)]
    struct WorkbookField {
        series: &'static str,
        unit: &'static str,
        view: View,
        label: &'static str,
        section: Option<&'static str>,
        multiplier: f64,
        rounding_step: f64,
    }

    const ORE_ATTRIBUTABLE: &str = "ore_processed_attributable";

    const PARITY_FIELDS: [WorkbookField; 4] = [
        WorkbookField {
            series: "head_grade",
            unit: "g/t",
            view: View::Minestats,
            label: "Processed",
            section: Some("Average grade (grams per tonne)"),
            multiplier: 1.0,
            rounding_step: 0.01,
        },
        WorkbookField {
            series: "recovery",
            unit: "pct",
            view: View::Minestats,
            label: "Recovery rate (percent)",
            section: None,
            multiplier: 100.0,
            rounding_step: 1.0,
        },
        WorkbookField {
            series: ORE_ATTRIBUTABLE,
            unit: "kt",
            view: View::Minestats,
            label: "Tonnes processed (thousands)",
            section: None,
            multiplier: 1.0,
            rounding_step: 1.0,
        },
        WorkbookField {
            series: "gold_produced_100",
            unit: "koz",
            view: View::Summary,
            label: "au_prod_100_koz",
            section: None,
            multiplier: 1.0,
            rounding_step: 1.0,
        },
    ];

    const AISC_CONTEXT: WorkbookField = WorkbookField {
        series: "aisc",
        unit: "USD/oz",
        view: View::Summary,
        label: "aisc_usd_per_oz",
        section: None,
        multiplier: 1.0,
        rounding_step: 1.0,
    };

    fn parity_field_name(series: &str) -> &str {
        if series == "ore_processed" {
            ORE_ATTRIBUTABLE
        } else {
            series
        }
    }

    impl WorkbookField {
        fn point(self, read: &MineStatsRead, sha256_hex: &str) -> Result<CitedPoint, Refusal> {
            let invalid = |detail: &str| {
                Refusal::new(
                    RefusalCode::SourceCellUnusable208,
                    format!("North Mara {} {}: {detail}", read.quarter, self.series),
                )
            };
            let mut matches = read.values.iter().filter(|row| {
                row.site == "North Mara"
                    && row.months == 3
                    && row.period == read.quarter
                    && row.view == self.view
                    && (self.view != View::Summary || row.sheet == "QTD")
                    && row.label == self.label
                    && row.section.as_deref() == self.section
            });
            let row = matches
                .next()
                .ok_or_else(|| invalid("quarterly cell missing"))?;
            let row_number = row
                .cell
                .trim_start_matches(|c: char| c.is_ascii_alphabetic());
            if matches.next().is_some()
                || read
                    .repeated
                    .contains(&format!("{}:{}!A{row_number}", read.quarter, row.sheet))
            {
                return Err(invalid(&format!(
                    "ambiguous quarterly cell at {}",
                    row.locator
                )));
            }
            if self.series == ORE_ATTRIBUTABLE
                && row
                    .printed_share
                    .as_deref()
                    .and_then(|s| s.parse::<f64>().ok())
                    != Some(f64::from(NORTH_MARA_OWNERSHIP_PCT))
            {
                return Err(invalid(&format!(
                    "{} does not declare the 84% share",
                    row.locator
                )));
            }
            let MineStatsCell::Number { value, .. } = row.value else {
                return Err(invalid(&format!("{} is not numeric", row.locator)));
            };
            Ok(CitedPoint {
                quarter: read.quarter.clone(),
                cell: row.locator.clone(),
                source_sha256: sha256_hex.to_owned(),
                printed_value: value * self.multiplier,
                rounding_step: self.rounding_step,
            })
        }

        fn series(self) -> CitedSeries {
            CitedSeries {
                series_name: self.series.into(),
                unit: self.unit.into(),
                points: Vec::new(),
            }
        }
    }

    /// Public quarterly inputs for the synthetic North Mara pilot. AISC retains reported cost
    /// context; the ore parity series retains attributable tonnes as printed (E109).
    #[derive(Debug, Clone, PartialEq)]
    pub struct NorthMaraPublicInputs {
        pub targets: Vec<NorthMaraQuarterTarget>,
        pub parity_series: [CitedSeries; 4],
        pub aisc_context: CitedSeries,
        pub workbook_digests: Vec<Digest32>,
    }

    /// Read one original three-month observation per registered Mine Stats quarter. The public
    /// loader verifies the registered workbook bytes before the pilot reads any target cells.
    pub fn load_north_mara_public_inputs(root: &Path) -> Result<NorthMaraPublicInputs, Refusal> {
        let mut profile = fmine_public::load_issuer_profile(root, "barrick")?;
        profile
            .sources
            .retain(|source| source.kind == SourceKind::MineStats);
        let vocabulary: LabelVocabulary = profile.mine_stats_labels.iter().cloned().collect();
        let mut sources = fmine_public::load_registered_sources(&profile, root)?;
        sources.sort_by(|a, b| a.quarter.cmp(&b.quarter));
        // E109 and the North Mara MD&A reconcile Mine Stats physical tonnage to the 84% share.
        let share = f64::from(NORTH_MARA_OWNERSHIP_PCT) / 100.0;
        let mut inputs = NorthMaraPublicInputs {
            targets: Vec::with_capacity(sources.len()),
            parity_series: PARITY_FIELDS.map(WorkbookField::series),
            aisc_context: AISC_CONTEXT.series(),
            workbook_digests: Vec::with_capacity(sources.len()),
        };
        for source in sources {
            if inputs
                .targets
                .last()
                .is_some_and(|last| last.quarter == source.quarter)
            {
                return Err(Refusal::new(
                    RefusalCode::SourceCellUnusable208,
                    format!(
                        "North Mara {} has duplicate registered Mine Stats workbooks",
                        source.quarter
                    ),
                ));
            }
            let read = read_mine_stats(
                &source.workbook,
                &source.quarter,
                (!vocabulary.is_empty()).then_some(&vocabulary),
            )?;
            let digest = Digest32(source.workbook.sha256);
            let sha256_hex = digest.to_hex();
            let [grade, recovery, ore, gold] =
                PARITY_FIELDS.map(|field| field.point(&read, &sha256_hex));
            let (grade, recovery, ore, gold) = (grade?, recovery?, ore?, gold?);
            let aisc = AISC_CONTEXT.point(&read, &sha256_hex)?;
            inputs.targets.push(NorthMaraQuarterTarget {
                quarter: source.quarter,
                ore_processed_kt: ore.printed_value / share,
                head_grade_g_t: grade.printed_value,
                recovery_pct: recovery.printed_value,
                gold_produced_100_koz: gold.printed_value,
                aisc_per_oz: aisc.printed_value,
            });
            for (series, point) in inputs
                .parity_series
                .iter_mut()
                .zip([grade, recovery, ore, gold])
            {
                series.points.push(point);
            }
            inputs.aisc_context.points.push(aisc);
            inputs.workbook_digests.push(digest);
        }
        Ok(inputs)
    }

    /// Quarterly aggregate emitted by one [`NorthMaraTwin`].
    #[derive(Debug, Clone, PartialEq)]
    pub struct NorthMaraQuarterAgg {
        pub quarter: String,
        pub ore_processed_kt: f64,
        pub head_grade_g_t: f64,
        pub recovery_pct: f64,
        pub implied_tails_g_t: f64,
        pub gold_produced_100_koz: f64,
        /// Strictly derived as `gold_produced_100_koz * (ownership_pct / 100.0)`; never fitted.
        pub derived_attributable_koz: f64,
        pub aisc_per_oz: f64,
        pub segment_boundary_reset: bool,
    }

    /// Day-level synthetic micro-state for one grade-led North Mara twin.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct NorthMaraDailyMicroPoint {
        pub quarter_index: usize,
        pub day_index: usize,
        pub ar1_grade_state: f64,
        pub daily_ore_kt: f64,
        pub daily_head_grade_g_t: f64,
        pub daily_recovery_pct: f64,
        pub daily_gc_bias_g_t: f64,
        pub daily_resource_bias_g_t: f64,
        pub daily_dilution_frac: f64,
        pub daily_survey_err_pct: f64,
        pub daily_sampler_bias_pct: f64,
    }

    /// One grade-led synthetic twin for North Mara (`BD-356.S4`, 100%-basis calibrated, 84% derived).
    #[derive(Debug, Clone, PartialEq)]
    pub struct NorthMaraTwin {
        pub k: usize,
        pub ownership_pct: u32,
        pub axes: GradeLedAxes,
        pub seed: RngKey,
        pub label: &'static str,
        daily: Vec<NorthMaraDailyMicroPoint>,
        quarterly: Vec<NorthMaraQuarterAgg>,
    }

    impl NorthMaraTwin {
        /// Build one deterministic 100%-basis North Mara twin along `axes`.
        ///
        /// `grade_shift_g_t` is `0.0` in normal operation and non-zero only for planted-mismatch
        /// refusal controls.
        pub fn build(
            k: usize,
            ownership_pct: u32,
            axes: GradeLedAxes,
            seed: RngKey,
            targets: &[NorthMaraQuarterTarget],
            boundaries: &BTreeSet<String>,
            grade_shift_g_t: f64,
        ) -> Result<Self, Refusal> {
            if ownership_pct != NORTH_MARA_OWNERSHIP_PCT {
                return Err(Refusal::new(
                    RefusalCode::BasisMismatch105,
                    format!(
                        "NorthMaraTwin requires ownership_pct={NORTH_MARA_OWNERSHIP_PCT}, got {ownership_pct}"
                    ),
                ));
            }
            if targets.is_empty() {
                return Err(Refusal::new(
                    RefusalCode::NoClaimInDomain201,
                    "NorthMaraTwin requires at least one 100%-basis quarterly target",
                ));
            }
            const DAYS_PER_QUARTER: usize = 90;
            let share = f64::from(ownership_pct) / 100.0;
            let mut rng = PhiloxStream::new(seed);
            let mut daily = Vec::with_capacity(targets.len() * DAYS_PER_QUARTER);
            let mut quarterly = Vec::with_capacity(targets.len());
            let mut ar_state = 0.0_f64;

            for (q_idx, target) in targets.iter().enumerate() {
                if !(target.ore_processed_kt.is_finite()
                    && target.ore_processed_kt > 0.0
                    && target.head_grade_g_t.is_finite()
                    && target.head_grade_g_t > 0.0
                    && target.recovery_pct.is_finite()
                    && target.recovery_pct > 0.0
                    && target.recovery_pct <= 100.0
                    && target.gold_produced_100_koz.is_finite()
                    && target.gold_produced_100_koz >= 0.0
                    && target.aisc_per_oz.is_finite()
                    && target.aisc_per_oz > 0.0)
                {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!("invalid 100%-basis target for quarter {}", target.quarter),
                    ));
                }

                let boundary_reset = boundaries.contains(&target.quarter);
                if boundary_reset {
                    ar_state = 0.0;
                }

                // Generate zero-mean intra-quarter AR(1) grade deviations so quarterly averages
                // match 100%-basis targets exactly (modulo optional planted shift).
                let mut raw_z = Vec::with_capacity(DAYS_PER_QUARTER);
                let phi = 0.35 + 0.35 * axes.gc_vs_resource_bias_split;
                let innov_scale = sqrt((1.0 - phi * phi).max(0.01));
                for _ in 0..DAYS_PER_QUARTER {
                    let u = rng.next_f64_open01().clamp(1e-12, 1.0 - 1e-12);
                    let z = norm_ppf(u);
                    ar_state = phi * ar_state + innov_scale * z;
                    raw_z.push(ar_state);
                }
                let mean_z = raw_z.iter().copied().sum::<f64>() / (DAYS_PER_QUARTER as f64);

                let daily_ore_mean = target.ore_processed_kt / (DAYS_PER_QUARTER as f64);
                let mut sum_ore = 0.0_f64;
                let mut sum_grade = 0.0_f64;
                let mut sum_rec = 0.0_f64;
                let total_model_sigma = 0.08 * target.head_grade_g_t;

                for (day_idx, &z) in raw_z.iter().enumerate() {
                    let centered = z - mean_z;
                    let gc_bias = total_model_sigma * axes.gc_vs_resource_bias_split * centered;
                    let res_bias =
                        total_model_sigma * (1.0 - axes.gc_vs_resource_bias_split) * centered;
                    let ore_dev =
                        daily_ore_mean * (axes.survey_stockpile_error_pct / 100.0) * 0.2 * centered;
                    let d_ore = (daily_ore_mean + ore_dev).max(1e-6);
                    let d_grade =
                        (target.head_grade_g_t + grade_shift_g_t + gc_bias + res_bias).max(0.05);
                    let d_rec = (target.recovery_pct
                        + 0.15 * (axes.sampler_assay_bias_pct / 100.0) * centered)
                        .clamp(1.0, 99.9);

                    sum_ore += d_ore;
                    sum_grade += d_grade;
                    sum_rec += d_rec;
                    daily.push(NorthMaraDailyMicroPoint {
                        quarter_index: q_idx,
                        day_index: day_idx,
                        ar1_grade_state: centered,
                        daily_ore_kt: d_ore,
                        daily_head_grade_g_t: d_grade,
                        daily_recovery_pct: d_rec,
                        daily_gc_bias_g_t: gc_bias,
                        daily_resource_bias_g_t: res_bias,
                        daily_dilution_frac: axes.dilution_ore_loss_ratio,
                        daily_survey_err_pct: axes.survey_stockpile_error_pct,
                        daily_sampler_bias_pct: axes.sampler_assay_bias_pct,
                    });
                }

                let agg_grade = sum_grade / (DAYS_PER_QUARTER as f64);
                let agg_rec = sum_rec / (DAYS_PER_QUARTER as f64);
                let implied_tails = agg_grade * (1.0 - agg_rec / 100.0);
                // 100%-basis gold scales proportionally when a planted shift is injected; exact at 0 shift.
                let agg_gold_100 = if target.head_grade_g_t > 0.0 {
                    target.gold_produced_100_koz * (agg_grade / target.head_grade_g_t)
                } else {
                    target.gold_produced_100_koz
                };
                // Attributable ounces are STRICTLY DERIVED from 84% share, NEVER fitted.
                let derived_attributable_koz = agg_gold_100 * share;

                quarterly.push(NorthMaraQuarterAgg {
                    quarter: target.quarter.clone(),
                    ore_processed_kt: sum_ore,
                    head_grade_g_t: agg_grade,
                    recovery_pct: agg_rec,
                    implied_tails_g_t: implied_tails,
                    gold_produced_100_koz: agg_gold_100,
                    derived_attributable_koz,
                    aisc_per_oz: target.aisc_per_oz,
                    segment_boundary_reset: boundary_reset,
                });
            }

            Ok(Self {
                k,
                ownership_pct,
                axes,
                seed,
                label: SYNTHETIC_PUBLIC_LABEL,
                daily,
                quarterly,
            })
        }

        #[must_use]
        pub fn quarterly_aggregates(&self) -> &[NorthMaraQuarterAgg] {
            &self.quarterly
        }

        #[must_use]
        pub fn daily_series(&self) -> &[NorthMaraDailyMicroPoint] {
            &self.daily
        }

        /// Verify parity against cited public quarterly cells on their declared basis, emitting
        /// one `twin_parity` NDJSON line per cell (`BD-356.S4`).
        pub fn check_parity(&self, public_series: &[CitedSeries]) -> Result<Vec<String>, Refusal> {
            let mut logs = Vec::new();
            for series in public_series {
                let expected_unit = PARITY_FIELDS.iter().find_map(|field| {
                    (field.series == parity_field_name(&series.series_name)).then_some(field.unit)
                }).ok_or_else(|| Refusal::new(
                    RefusalCode::BasisMismatch105,
                    format!(
                        "unsupported North Mara parity series '{}'; attributable production is derived, never fitted",
                        series.series_name,
                    ),
                ))?;
                if series.unit != expected_unit {
                    return Err(Refusal::new(
                        RefusalCode::BasisMismatch105,
                        format!(
                            "north_mara series '{}' expected unit '{}', got '{}'",
                            series.series_name, expected_unit, series.unit
                        ),
                    ));
                }
                for pt in &series.points {
                    let Some(agg) = self.quarterly.iter().find(|q| q.quarter == pt.quarter) else {
                        return Err(Refusal::new(
                            RefusalCode::NoClaimInDomain201,
                            format!("north_mara twin {} missing quarter {}", self.k, pt.quarter),
                        ));
                    };
                    let val = match series.series_name.as_str() {
                        "head_grade" => agg.head_grade_g_t,
                        "recovery" => agg.recovery_pct,
                        "ore_processed" => agg.ore_processed_kt,
                        ORE_ATTRIBUTABLE => {
                            agg.ore_processed_kt * (f64::from(self.ownership_pct) / 100.0)
                        }
                        "gold_produced_100" => agg.gold_produced_100_koz,
                        _ => unreachable!(),
                    };
                    let half = 0.5 * pt.rounding_step;
                    let lo = pt.printed_value - half;
                    let hi = pt.printed_value + half;
                    let ok = val >= lo - 1e-9 && val <= hi + 1e-9;
                    logs.push(format!(
                        "{{\"event\":\"twin_parity\",\"site\":\"north_mara\",\"twin\":{},\"quarter\":\"{}\",\"series\":\"{}\",\"cell\":\"{}\",\"sha256\":\"{}\",\"printed\":{:.4},\"twin_lo\":{:.4},\"twin_hi\":{:.4},\"ok\":{ok}}}",
                        self.k,
                        escape_json_str(&pt.quarter),
                        escape_json_str(&series.series_name),
                        escape_json_str(&pt.cell),
                        escape_json_str(&pt.source_sha256),
                        pt.printed_value,
                        val,
                        val,
                    ));
                    if !ok {
                        return Err(Refusal::new(
                            RefusalCode::Conservation101,
                            format!(
                                "north_mara twin {} failed parity on {} {} ({}): twin={val:.4} outside [{lo:.4}, {hi:.4}]",
                                self.k, pt.quarter, series.series_name, pt.cell
                            ),
                        ));
                    }
                }
            }
            Ok(logs)
        }
    }

    fn require_parity_population(
        targets: &[NorthMaraQuarterTarget],
        public_series: &[CitedSeries],
    ) -> Result<(), Refusal> {
        let invalid = |detail: String| Refusal::new(RefusalCode::SourceCellUnusable208, detail);
        let quarters: BTreeSet<&str> = targets
            .iter()
            .map(|target| target.quarter.as_str())
            .collect();
        if quarters.is_empty() || quarters.len() != targets.len() {
            return Err(invalid(
                "North Mara parity requires nonempty, unique target quarters".into(),
            ));
        }
        if public_series.len() != PARITY_FIELDS.len() {
            return Err(invalid(format!(
                "North Mara parity requires all {} operating series",
                PARITY_FIELDS.len()
            )));
        }
        for field in PARITY_FIELDS {
            let mut series = public_series
                .iter()
                .filter(|series| parity_field_name(&series.series_name) == field.series);
            let source = series
                .next()
                .ok_or_else(|| invalid(format!("North Mara parity is missing {}", field.series)))?;
            if series.next().is_some() {
                return Err(invalid(format!(
                    "North Mara parity has duplicate {} series",
                    field.series
                )));
            }
            let mut covered = BTreeSet::new();
            for point in &source.points {
                if !quarters.contains(point.quarter.as_str())
                    || !covered.insert(point.quarter.as_str())
                    || point.cell.trim().is_empty()
                    || point.source_sha256.trim().is_empty()
                    || !point.printed_value.is_finite()
                    || !point.rounding_step.is_finite()
                    || point.rounding_step <= 0.0
                {
                    return Err(invalid(format!(
                        "North Mara parity has unusable {} evidence for {}",
                        field.series, point.quarter
                    )));
                }
            }
            if let Some(quarter) = quarters.difference(&covered).next() {
                return Err(invalid(format!(
                    "North Mara parity is missing {} evidence for {quarter}",
                    field.series
                )));
            }
        }
        Ok(())
    }

    /// Ensemble of `K >= 2` grade-led North Mara twins spanning registered unidentifiable axes.
    #[derive(Debug, Clone, PartialEq)]
    pub struct NorthMaraTwinFamily {
        pub site: String,
        pub ownership_pct: u32,
        pub twins: Vec<NorthMaraTwin>,
        pub family_digest: Digest32,
        pub label: &'static str,
    }

    impl NorthMaraTwinFamily {
        /// Build `spec.k` twins spanning `spec.axis_ranges` and verify 100%-basis parity on every twin.
        pub fn build_and_verify(
            spec: &GradeLedRegistrySpec,
            seed_root: u64,
            targets: &[NorthMaraQuarterTarget],
            public_series: &[CitedSeries],
        ) -> Result<(Self, Vec<String>), Refusal> {
            require_parity_population(targets, public_series)?;
            spec.axis_ranges.validate()?;
            if spec.k < 2 {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("NorthMaraTwinFamily requires k >= 2 (got {})", spec.k),
                ));
            }
            let boundaries: BTreeSet<String> = spec
                .segment_boundaries
                .iter()
                .map(|b| b.quarter.clone())
                .collect();

            let interp = |(lo, hi): (f64, f64), t: f64| lo + (hi - lo) * t;
            let mut twins = Vec::with_capacity(spec.k);
            let mut all_parity_logs = Vec::new();
            let mut enc = Encoder::new_frame(*b"NMTW", 1);
            enc.put_str(&spec.site);
            enc.put_u64(u64::from(spec.ownership_pct));
            enc.put_u64(spec.k as u64);

            for idx in 0..spec.k {
                let t = (idx as f64) / ((spec.k - 1) as f64);
                let t_alt = 1.0 - t;
                let t_mid = ((idx * 2) % spec.k) as f64 / ((spec.k - 1) as f64);
                let axes = GradeLedAxes {
                    gc_vs_resource_bias_split: interp(
                        spec.axis_ranges.gc_vs_resource_bias_split,
                        t,
                    ),
                    dilution_ore_loss_ratio: interp(
                        spec.axis_ranges.dilution_ore_loss_ratio,
                        t_alt,
                    ),
                    survey_stockpile_error_pct: interp(
                        spec.axis_ranges.survey_stockpile_error_pct,
                        t_mid,
                    ),
                    sampler_assay_bias_pct: interp(spec.axis_ranges.sampler_assay_bias_pct, t),
                };
                let seed = RngKey {
                    seed_root,
                    partition: 356,
                    domain: 4,
                    var_group: 1,
                    realization: idx as u32,
                    brick: 0,
                };
                let twin = NorthMaraTwin::build(
                    idx,
                    spec.ownership_pct,
                    axes,
                    seed,
                    targets,
                    &boundaries,
                    0.0,
                )?;
                let parity_logs = twin.check_parity(public_series)?;
                all_parity_logs.extend(parity_logs);

                enc.put_u64(idx as u64);
                enc.put_u64(canonical_f64_bits(axes.gc_vs_resource_bias_split));
                enc.put_u64(canonical_f64_bits(axes.dilution_ore_loss_ratio));
                enc.put_u64(canonical_f64_bits(axes.survey_stockpile_error_pct));
                enc.put_u64(canonical_f64_bits(axes.sampler_assay_bias_pct));
                for agg in twin.quarterly_aggregates() {
                    enc.put_str(&agg.quarter);
                    enc.put_u64(canonical_f64_bits(agg.ore_processed_kt));
                    enc.put_u64(canonical_f64_bits(agg.head_grade_g_t));
                    enc.put_u64(canonical_f64_bits(agg.recovery_pct));
                    enc.put_u64(canonical_f64_bits(agg.gold_produced_100_koz));
                    enc.put_u64(canonical_f64_bits(agg.derived_attributable_koz));
                }
                twins.push(twin);
            }

            let family_digest = sha256(&enc.finish());
            Ok((
                Self {
                    site: spec.site.clone(),
                    ownership_pct: spec.ownership_pct,
                    twins,
                    family_digest,
                    label: SYNTHETIC_PUBLIC_LABEL,
                },
                all_parity_logs,
            ))
        }
    }

    /// External implied-ownership-share interval checked against North Mara's 84% share (`E819`).
    #[derive(Debug, Clone, PartialEq)]
    pub struct ImpliedShareProbe {
        pub quarter: String,
        pub cell: String,
        pub metric: String,
        pub implied_share_lo: f64,
        pub implied_share_hi: f64,
        pub source_e_id: String,
    }

    /// Reported attributable-share conflict (`E819`): logged explicitly on the card, NEVER forced.
    #[derive(Debug, Clone, PartialEq)]
    pub struct AttributableShareConflict {
        pub quarter: String,
        pub cell: String,
        pub metric: String,
        pub stated_share_pct: u32,
        pub implied_share_lo: f64,
        pub implied_share_hi: f64,
        pub source_e_id: String,
        pub detail: String,
    }

    /// Check implied-share intervals against `stated_share_pct` (84% for North Mara), reporting
    /// any interval that excludes the stated share without ever altering the 84% derivation (`E819`).
    pub fn check_attributable_share_conflicts(
        stated_share_pct: u32,
        probes: &[ImpliedShareProbe],
    ) -> Result<Vec<AttributableShareConflict>, Refusal> {
        if !(1..=100).contains(&stated_share_pct) {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("stated_share_pct ({stated_share_pct}) must lie in 1..=100"),
            ));
        }
        let stated = f64::from(stated_share_pct) / 100.0;
        let mut conflicts = Vec::new();
        for p in probes {
            if !(p.implied_share_lo.is_finite()
                && p.implied_share_hi.is_finite()
                && p.implied_share_lo >= 0.0
                && p.implied_share_hi <= 1.0
                && p.implied_share_lo <= p.implied_share_hi)
            {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!(
                        "invalid implied share interval [{}, {}] for {} {}",
                        p.implied_share_lo, p.implied_share_hi, p.quarter, p.cell
                    ),
                ));
            }
            if stated < p.implied_share_lo || stated > p.implied_share_hi {
                conflicts.push(AttributableShareConflict {
                    quarter: p.quarter.clone(),
                    cell: p.cell.clone(),
                    metric: p.metric.clone(),
                    stated_share_pct,
                    implied_share_lo: p.implied_share_lo,
                    implied_share_hi: p.implied_share_hi,
                    source_e_id: p.source_e_id.clone(),
                    detail: format!(
                        "{} {} ({}, {}): implied share [{:.4}, {:.4}] excludes stated {:.2} ({}%); reported, not forced",
                        p.quarter,
                        p.metric,
                        p.cell,
                        p.source_e_id,
                        p.implied_share_lo,
                        p.implied_share_hi,
                        stated,
                        stated_share_pct
                    ),
                });
            }
        }
        Ok(conflicts)
    }

    /// Multi-twin `[min, max]` power envelope for a grade-led cause (`BD-356.S4`).
    #[derive(Debug, Clone, PartialEq)]
    pub struct GradeLedPowerEnvelope {
        pub subset: Vec<String>,
        pub cause: GradeLedCause,
        pub twins_evaluated: usize,
        pub min_power: f64,
        pub max_power: f64,
        pub max_mc_se: f64,
        pub identifiability_refusal: Option<Refusal>,
    }

    impl GradeLedPowerEnvelope {
        pub fn render_envelope_string(&self) -> Result<String, Refusal> {
            if self.twins_evaluated < 2 {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    "cannot format grade-led power without multi-twin [min, max] envelope",
                ));
            }
            if !self.min_power.is_finite()
                || !self.max_power.is_finite()
                || self.min_power > self.max_power
            {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    "malformed grade-led power envelope bounds",
                ));
            }
            let suffix = match &self.identifiability_refusal {
                Some(r) => format!(" [non-identifiable: {}]", r.code),
                None => String::new(),
            };
            Ok(format!(
                "{}: [{:.3}, {:.3}] (k={}, max_mc_se={:.4}){suffix}",
                self.cause.as_str(),
                self.min_power,
                self.max_power,
                self.twins_evaluated,
                self.max_mc_se
            ))
        }
    }

    /// Candidate file row on [`NorthMaraPilotCard`].
    #[derive(Debug, Clone, PartialEq)]
    pub struct GradeLedFileRow {
        pub name: String,
        pub reason: String,
        pub label: FileLabel,
        pub envelope_by_cause: Vec<GradeLedPowerEnvelope>,
    }

    fn grade_led_cause_snr(
        twin: &NorthMaraTwin,
        subset: &[&str],
        cause: GradeLedCause,
    ) -> (f64, Option<Refusal>) {
        let has = |name: &str| subset.contains(&name);
        let n_q = twin.quarterly_aggregates().len().max(1) as f64;
        let ax = twin.axes;

        let base_info = match cause {
            GradeLedCause::GcModelBias => {
                let mut info = 0.0_f64;
                if has("block_and_gc_models") {
                    info += 4.2 * (0.45 + 0.7 * ax.gc_vs_resource_bias_split);
                }
                if has("mill_sampler_records") {
                    info += 1.6 / (0.6 + ax.sampler_assay_bias_pct);
                }
                if has("metallurgical_balance") {
                    info += 1.1;
                }
                if has("gate_port_02_export") {
                    info += 0.6 * ax.gc_vs_resource_bias_split;
                }
                info
            }
            GradeLedCause::ResourceModelBias => {
                let mut info = 0.0_f64;
                if has("block_and_gc_models") {
                    info += 4.0 * (0.45 + 0.7 * (1.0 - ax.gc_vs_resource_bias_split));
                }
                if has("mine_surveys") {
                    info += 1.5 / (0.8 + ax.survey_stockpile_error_pct);
                }
                if has("metallurgical_balance") {
                    info += 0.9;
                }
                info
            }
            GradeLedCause::DilutionOreLoss => {
                let mut info = 0.0_f64;
                if has("block_and_gc_models") {
                    info += 2.6;
                }
                if has("mine_surveys") {
                    info += 3.2 * (0.6 + 2.0 * ax.dilution_ore_loss_ratio);
                }
                if has("dispatch_tonnages") {
                    info += 2.2 * (0.5 + 1.8 * ax.dilution_ore_loss_ratio);
                }
                if has("metallurgical_balance") {
                    info += 1.4;
                }
                info
            }
            GradeLedCause::OreDeliveryShortfall => {
                let mut info = 0.0_f64;
                if has("dispatch_tonnages") {
                    info += 4.5;
                }
                if has("mine_surveys") {
                    info += 2.8 / (0.7 + 0.25 * ax.survey_stockpile_error_pct);
                }
                if has("metallurgical_balance") {
                    info += 2.0;
                }
                if has("cost_centre_extracts") {
                    info += 0.9;
                }
                info
            }
            GradeLedCause::SurveyStockpileError => {
                let mut info = 0.0_f64;
                if has("mine_surveys") {
                    info += 4.6 * (0.5 + 0.25 * ax.survey_stockpile_error_pct);
                }
                if has("dispatch_tonnages") {
                    info += 2.1;
                }
                if has("metallurgical_balance") {
                    info += 1.5;
                }
                info
            }
            GradeLedCause::SamplerAssayBias => {
                let mut info = 0.0_f64;
                if has("mill_sampler_records") {
                    info += 4.8 * (0.5 + 0.35 * ax.sampler_assay_bias_pct);
                }
                if has("metallurgical_balance") {
                    info += 2.4;
                }
                if has("gate_port_02_export") {
                    info += 0.8;
                }
                info
            }
        };

        // GB-25 non-identifiability check on the GC-vs-resource model bias pair: without
        // block_and_gc_models (both models separated), observations only constrain their sum.
        let identifiability_refusal = if matches!(
            cause,
            GradeLedCause::GcModelBias | GradeLedCause::ResourceModelBias
        ) && !has("block_and_gc_models")
        {
            let mut obs = Vec::new();
            if has("metallurgical_balance") || has("mill_sampler_records") || has("mine_surveys") {
                for _ in 0..4 {
                    obs.push(LinearBiasObs {
                        h1: 1.0,
                        h2: 1.0,
                        y: 0.0,
                        sigma: 0.20,
                    });
                }
            }
            match solve_two_bias_posterior(1.0, 1.0, &obs) {
                Ok(post) => post.attribution_status.err(),
                Err(e) => Some(e),
            }
        } else {
            None
        };

        let snr = sqrt(base_info * (n_q / 6.0).max(0.5));
        (snr, identifiability_refusal)
    }

    /// Estimate the `[min, max]` power envelope across all twins in `family` for `(subset, cause)`.
    pub fn estimate_grade_led_power_envelope(
        family: &NorthMaraTwinFamily,
        subset: &[&str],
        cause: GradeLedCause,
        replicates: usize,
        seed_root: u64,
    ) -> Result<GradeLedPowerEnvelope, Refusal> {
        reject_personal_data_files(subset)?;
        if family.twins.len() < 2 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "estimate_grade_led_power_envelope requires at least 2 twins",
            ));
        }
        if replicates < 20 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "replicates must be >= 20",
            ));
        }

        let z_crit = 1.959_963_984_540_054;
        let mut min_power = 1.0_f64;
        let mut max_power = 0.0_f64;
        let mut max_mc_se = 0.0_f64;
        let mut first_refusal: Option<Refusal> = None;

        for (t_idx, twin) in family.twins.iter().enumerate() {
            let (snr, refusal) = grade_led_cause_snr(twin, subset, cause);
            if refusal.is_some() && first_refusal.is_none() {
                first_refusal = refusal.clone();
            }
            let power = if refusal.is_some() || snr <= 0.0 {
                0.0
            } else {
                let mut rng = PhiloxStream::new(RngKey {
                    seed_root,
                    partition: 356,
                    domain: 40 + (cause as u32),
                    var_group: subset.len() as u32,
                    realization: t_idx as u32,
                    brick: subset_stream_brick(subset),
                });
                let mut detected = 0usize;
                for _ in 0..replicates {
                    let u = rng.next_f64_open01().clamp(1e-12, 1.0 - 1e-12);
                    let z = snr + norm_ppf(u);
                    if z.abs() >= z_crit {
                        detected += 1;
                    }
                }
                (detected as f64) / (replicates as f64)
            };
            let mc_se = sqrt((power * (1.0 - power)).max(0.0) / (replicates as f64));
            if power < min_power {
                min_power = power;
            }
            if power > max_power {
                max_power = power;
            }
            if mc_se > max_mc_se {
                max_mc_se = mc_se;
            }
        }

        Ok(GradeLedPowerEnvelope {
            subset: subset.iter().map(|s| (*s).to_string()).collect(),
            cause,
            twins_evaluated: family.twins.len(),
            min_power,
            max_power,
            max_mc_se,
            identifiability_refusal: first_refusal,
        })
    }

    /// Label each grade-led candidate file as `Robust` (in the top-`top_n` cost-adjusted set under
    /// EVERY twin) or `ModelDependent` (with the fraction of twins selecting it).
    pub fn label_grade_led_candidate_files(
        family: &NorthMaraTwinFamily,
        candidates: &[CandidateFileCostRow],
        top_n: usize,
    ) -> Result<Vec<(String, FileLabel, usize, usize)>, Refusal> {
        let names: Vec<&str> = candidates.iter().map(|c| c.name.as_str()).collect();
        reject_personal_data_files(&names)?;
        if family.twins.len() < 2 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "label_grade_led_candidate_files requires at least 2 twins",
            ));
        }
        if candidates.is_empty() || top_n == 0 {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                "label_grade_led_candidate_files requires non-empty candidates and top_n >= 1",
            ));
        }

        let mut counts: BTreeMap<&str, usize> =
            candidates.iter().map(|c| (c.name.as_str(), 0)).collect();
        for twin in &family.twins {
            let mut scored: Vec<(&str, f64)> = candidates
                .iter()
                .map(|c| {
                    let total_snr: f64 = GradeLedCause::ALL
                        .iter()
                        .map(|&cause| grade_led_cause_snr(twin, &[c.name.as_str()], cause).0)
                        .sum();
                    // Emphasize axis sensitivity via twin-specific cost weighting on auxiliary files.
                    let score = total_snr / c.extraction_cost.max(0.1);
                    (c.name.as_str(), score)
                })
                .collect();
            scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
            for &(name, _) in scored.iter().take(top_n) {
                if let Some(cnt) = counts.get_mut(name) {
                    *cnt += 1;
                }
            }
        }

        let total = family.twins.len();
        let mut out = Vec::with_capacity(candidates.len());
        for c in candidates {
            let sel = counts.get(c.name.as_str()).copied().unwrap_or(0);
            let label = if sel == total {
                FileLabel::Robust
            } else {
                FileLabel::ModelDependent {
                    reason: format!(
                        "selected in top-{top_n} under {sel}/{total} grade-led twins; depends on unidentifiable GC/dilution/survey axes"
                    ),
                }
            };
            out.push((c.name.clone(), label, sel, total));
        }
        Ok(out)
    }

    /// Evaluate the §37.3 early-kill rule for North Mara's grade-led envelopes.
    #[must_use]
    pub fn evaluate_grade_led_early_kill(
        best_subset_envelopes: &[GradeLedPowerEnvelope],
        power_bar_target: f64,
    ) -> EarlyKillOutcome {
        if !power_bar_target.is_finite() || power_bar_target <= 0.0 || power_bar_target > 1.0 {
            return EarlyKillOutcome {
                fires: true,
                reason: format!(
                    "invalid power_bar_target ({power_bar_target}); fail-closed early kill ({NORTH_MARA_EARLY_KILL_ALTERNATE})"
                ),
                alternate_recommendation: Some(NORTH_MARA_EARLY_KILL_ALTERNATE.to_string()),
            };
        }
        if best_subset_envelopes.is_empty() {
            return EarlyKillOutcome {
                fires: true,
                reason: format!(
                    "no feasible grade-led subset evaluated; recommend fallback ({NORTH_MARA_EARLY_KILL_ALTERNATE})"
                ),
                alternate_recommendation: Some(NORTH_MARA_EARLY_KILL_ALTERNATE.to_string()),
            };
        }
        let mut failing = Vec::new();
        for env in best_subset_envelopes {
            if env.twins_evaluated < 2
                || !env.min_power.is_finite()
                || !env.max_power.is_finite()
                || env.identifiability_refusal.is_some()
                || env.min_power < power_bar_target
            {
                failing.push(format!(
                    "{} ([{:.3}, {:.3}], k={} < bar {:.3})",
                    env.cause.as_str(),
                    env.min_power,
                    env.max_power,
                    env.twins_evaluated,
                    power_bar_target
                ));
            }
        }
        if failing.is_empty() {
            EarlyKillOutcome {
                fires: false,
                reason: format!(
                    "feasible grade-led subset clears power bar {:.3} across every twin on all {} causes",
                    power_bar_target,
                    best_subset_envelopes.len()
                ),
                alternate_recommendation: None,
            }
        } else {
            EarlyKillOutcome {
                fires: true,
                reason: format!(
                    "no feasible grade-led subset clears power bar {:.3} under every twin ({}); recommend alternate ({NORTH_MARA_EARLY_KILL_ALTERNATE})",
                    power_bar_target,
                    failing.join("; ")
                ),
                alternate_recommendation: Some(NORTH_MARA_EARLY_KILL_ALTERNATE.to_string()),
            }
        }
    }

    /// North Mara grade-led Pilot Protocol Card (`BD-356.S4`).
    #[derive(Debug, Clone, PartialEq)]
    pub struct NorthMaraPilotCard {
        pub site: String,
        pub ownership_pct: u32,
        pub mechanism: String,
        pub attribution_question: String,
        pub k_twins: usize,
        pub files: Vec<GradeLedFileRow>,
        pub analysis_plan_id: ObjectId,
        pub early_kill: EarlyKillOutcome,
        pub envelope_too_wide_fallback: bool,
        pub residency_statement: String,
        pub attributable_conflicts: Vec<AttributableShareConflict>,
        pub inputs_digest: Digest32,
        pub seeds: Vec<u64>,
        pub code_rev: String,
        pub card_digest: Digest32,
        pub label: &'static str,
    }

    impl NorthMaraPilotCard {
        /// Validate all North Mara Pilot Protocol Card invariants fail-closed:
        /// - site is `"north_mara"` and `ownership_pct == 84`;
        /// - Tanzania Mining Act s.35(6) residency & non-affiliation statement present (`FMINE-REF-303`);
        /// - zero personal/roster files (`FMINE-REF-301`);
        /// - every file carries multi-twin `[min, max]` envelopes (`twins_evaluated >= 2`);
        /// - `card_digest == compute_card_digest(inputs_digest, &seeds, &code_rev)` (`FMINE-REF-505`).
        pub fn validate(&self) -> Result<(), Refusal> {
            if self.site != NORTH_MARA_SITE {
                return Err(Refusal::new(
                    RefusalCode::BasisMismatch105,
                    format!(
                        "NorthMaraPilotCard site must be '{NORTH_MARA_SITE}', got '{}'",
                        self.site
                    ),
                ));
            }
            if self.ownership_pct != NORTH_MARA_OWNERSHIP_PCT {
                return Err(Refusal::new(
                    RefusalCode::BasisMismatch105,
                    format!(
                        "NorthMaraPilotCard ownership_pct must be {NORTH_MARA_OWNERSHIP_PCT}, got {}",
                        self.ownership_pct
                    ),
                ));
            }
            require_non_empty("mechanism", &self.mechanism)?;
            require_non_empty("attribution_question", &self.attribution_question)?;
            require_non_empty("code_rev", &self.code_rev)?;

            let res_lower = self.residency_statement.to_ascii_lowercase();
            let residency_ok = res_lower.contains("mining act s.35(6)")
                && res_lower.contains("geological survey")
                && res_lower.contains("tanzania")
                && res_lower.contains("roster")
                && res_lower.contains("no affiliation");
            if !residency_ok {
                return Err(Refusal::new(
                    RefusalCode::Residency303,
                    "NorthMaraPilotCard must carry the Tanzania Mining Act s.35(6) in-country residency, no-rosters, and non-affiliation statement (E527, E597)",
                ));
            }

            if self.k_twins < 2 {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!(
                        "NorthMaraPilotCard requires k_twins >= 2 (got {})",
                        self.k_twins
                    ),
                ));
            }
            if self.files.is_empty() {
                return Err(Refusal::new(
                    RefusalCode::NoClaimInDomain201,
                    "NorthMaraPilotCard requires at least one candidate file",
                ));
            }
            let file_names: Vec<&str> = self.files.iter().map(|f| f.name.as_str()).collect();
            reject_personal_data_files(&file_names)?;
            for row in &self.files {
                require_non_empty("file.name", &row.name)?;
                require_non_empty("file.reason", &row.reason)?;
                if row.envelope_by_cause.is_empty() {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!("file '{}' has no multi-twin PowerEnvelope", row.name),
                    ));
                }
                for env in &row.envelope_by_cause {
                    let _ = env.render_envelope_string()?;
                }
            }
            let expected_digest =
                compute_card_digest(self.inputs_digest, &self.seeds, &self.code_rev)?;
            if self.card_digest != expected_digest {
                return Err(Refusal::new(
                    RefusalCode::DigestMismatch505,
                    format!(
                        "NorthMaraPilotCard digest mismatch: expected {expected_digest}, got {}",
                        self.card_digest
                    ),
                ));
            }
            if self.label != SYNTHETIC_PUBLIC_LABEL {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    "NorthMaraPilotCard must carry canonical SYNTHETIC_PUBLIC_LABEL",
                ));
            }
            Ok(())
        }

        /// Render §30.1 structured NDJSON events (`pilot_power` per `(file, cause)` plus `pilot_card`
        /// summary carrying all four `INV-002` flags `false`, residency, and attributable conflicts).
        pub fn render_json_events(&self) -> Result<Vec<String>, Refusal> {
            self.validate()?;
            let mut events = Vec::new();
            let mut robust = 0usize;
            let mut model_dependent = 0usize;
            for row in &self.files {
                let label_tag = match &row.label {
                    FileLabel::Robust => {
                        robust += 1;
                        "robust"
                    }
                    FileLabel::ModelDependent { .. } => {
                        model_dependent += 1;
                        "model-dependent"
                    }
                };
                for env in &row.envelope_by_cause {
                    events.push(format!(
                        "{{\"event\":\"pilot_power\",\"site\":\"north_mara\",\"twin\":\"envelope(k={})\",\"subset\":\"{}\",\"cause\":\"{}\",\"power\":[{:.4},{:.4}],\"mc_se\":{:.4},\"label\":\"{}\"}}",
                        env.twins_evaluated,
                        escape_json_str(&env.subset.join(",")),
                        env.cause.as_str(),
                        env.min_power,
                        env.max_power,
                        env.max_mc_se,
                        label_tag,
                    ));
                }
            }
            let alternate_json = match &self.early_kill.alternate_recommendation {
                Some(alt) => format!("\"{}\"", escape_json_str(alt)),
                None => "null".to_string(),
            };
            events.push(format!(
                "{{\"event\":\"pilot_card\",\"site\":\"north_mara\",\"k\":{},\"files\":{},\"robust\":{},\"model_dependent\":{},\"early_kill\":{},\"early_kill_reason\":\"{}\",\"alternate\":{},\"envelope_too_wide_fallback\":{},\"card_digest\":\"{}\",\"plan_id\":\"{}\",\"inputs_digest\":\"{}\",\"code_rev\":\"{}\",\"ownership_pct\":{},\"attributable_conflicts\":{},\"residency\":\"{}\",\"label\":\"{}\",\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}}",
                self.k_twins,
                self.files.len(),
                robust,
                model_dependent,
                self.early_kill.fires,
                escape_json_str(&self.early_kill.reason),
                alternate_json,
                self.envelope_too_wide_fallback,
                self.card_digest,
                self.analysis_plan_id,
                self.inputs_digest,
                escape_json_str(&self.code_rev),
                self.ownership_pct,
                self.attributable_conflicts.len(),
                escape_json_str(&self.residency_statement),
                escape_json_str(self.label),
            ));
            Ok(events)
        }

        /// Render deterministic LF-terminated Markdown for sponsor review prior to `GATE-DATA-01`.
        pub fn render_markdown(&self) -> Result<String, Refusal> {
            self.validate()?;
            let mut md = String::new();
            let _ = writeln!(md, "# Pilot Protocol Card — North Mara (`north_mara`)");
            let _ = writeln!(md);
            let _ = writeln!(md, "- **Disclaimer:** {}", self.label);
            let _ = writeln!(md, "- **Mechanism:** {}", sanitize_inline(&self.mechanism));
            let _ = writeln!(
                md,
                "- **Ownership basis:** 100%-basis twins; attributable figures derived at {}% (never fitted)",
                self.ownership_pct
            );
            let _ = writeln!(
                md,
                "- **Pre-registered attribution question:** {}",
                sanitize_inline(&self.attribution_question)
            );
            let _ = writeln!(md, "- **Analysis plan ID:** `{}`", self.analysis_plan_id);
            let _ = writeln!(md, "- **Card digest (BLAKE3):** `{}`", self.card_digest);
            let _ = writeln!(md, "- **Inputs digest (BLAKE3):** `{}`", self.inputs_digest);
            let _ = writeln!(
                md,
                "- **Code revision:** `{}`",
                sanitize_inline(&self.code_rev)
            );
            let _ = writeln!(md, "- **Twin family size:** `K = {}`", self.k_twins);
            let _ = writeln!(md);
            let _ = writeln!(md, "> {NO_CLAIM_BANNER}");
            let _ = writeln!(md);
            let _ = writeln!(
                md,
                "| Authority Flag (`INV-002`) | Value |\n|---|---|\n\
                 | `reserve_disclosure_claim` | `false` |\n\
                 | `execution_authorized` | `false` |\n\
                 | `geotechnical_signoff` | `false` |\n\
                 | `regulatory_compliance_claim` | `false` |"
            );

            let _ = writeln!(md, "\n## Data Residency & Governance (Tanzania)\n");
            let _ = writeln!(md, "{}", sanitize_inline(&self.residency_statement));

            let _ = writeln!(
                md,
                "\n## Attributable-Share Diagnostics (Reported, Never Forced)\n"
            );
            if self.attributable_conflicts.is_empty() {
                let _ = writeln!(
                    md,
                    "No attributable-share conflicts reported in supplied probes."
                );
            } else {
                for c in &self.attributable_conflicts {
                    let _ = writeln!(md, "- {}", sanitize_inline(&c.detail));
                }
            }

            let _ = writeln!(md, "\n## Candidate File Ask (Grade-Led GATE-DATA-03)\n");
            if self.envelope_too_wide_fallback {
                let _ = writeln!(
                    md,
                    "_Fallback active: no candidate file is robust across all twins; showing hashed plan plus per-file reasons with no ranking fallback._\n"
                );
            }
            let _ = writeln!(
                md,
                "| File | Label | Reason | Power Envelopes `[min, max]` |"
            );
            let _ = writeln!(md, "|---|---|---|---|");
            for row in &self.files {
                let label_text = match &row.label {
                    FileLabel::Robust => "robust".to_string(),
                    FileLabel::ModelDependent { reason } => format!("model-dependent ({reason})"),
                };
                let mut env_parts = Vec::with_capacity(row.envelope_by_cause.len());
                for env in &row.envelope_by_cause {
                    env_parts.push(env.render_envelope_string()?);
                }
                let _ = writeln!(
                    md,
                    "| `{}` | {} | {} | {} |",
                    escape_md_cell(&row.name),
                    escape_md_cell(&label_text),
                    escape_md_cell(&row.reason),
                    escape_md_cell(&env_parts.join("; ")),
                );
            }

            let _ = writeln!(md, "\n## Early-Kill Rule\n");
            let _ = writeln!(md, "- **Early kill fires:** `{}`", self.early_kill.fires);
            let _ = writeln!(
                md,
                "- **Outcome:** {}",
                sanitize_inline(&self.early_kill.reason)
            );
            if let Some(alt) = &self.early_kill.alternate_recommendation {
                let _ = writeln!(
                    md,
                    "- **Alternate site recommendation (§37.3):** {}",
                    sanitize_inline(alt)
                );
            }
            Ok(md)
        }
    }

    /// Build and pre-register the complete North Mara grade-led [`NorthMaraPilotCard`] (`BD-356.S4`).
    ///
    /// Atomic with respect to `ledger`: all validations, power envelopes, and share-conflict checks
    /// complete before freezing `"north_mara/analysis_plan"`.
    #[allow(clippy::too_many_arguments)]
    pub fn build_north_mara_pilot_card(
        family: &NorthMaraTwinFamily,
        spec: &GradeLedRegistrySpec,
        plan: &AnalysisPlan,
        ledger: &mut ArtifactLedger,
        registered_ns: i64,
        first_request_ns: i64,
        public_input_digests: &[Digest32],
        share_probes: &[ImpliedShareProbe],
        replicates: usize,
        seed_root: u64,
        top_n: usize,
    ) -> Result<NorthMaraPilotCard, Refusal> {
        if family.twins.len() < 2 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "build_north_mara_pilot_card requires at least 2 twins",
            ));
        }
        let plan_bytes = plan.canonical_bytes()?;
        if plan.site.trim() != NORTH_MARA_SITE
            || spec.site.trim() != NORTH_MARA_SITE
            || family.site.trim() != NORTH_MARA_SITE
        {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!(
                    "North Mara site mismatch across plan ('{}'), spec ('{}'), family ('{}')",
                    plan.site, spec.site, family.site
                ),
            ));
        }
        if !plan.seeds.contains(&seed_root) {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!("seed_root ({seed_root}) must appear in AnalysisPlan.seeds"),
            ));
        }
        if registered_ns >= first_request_ns {
            return Err(Refusal::new(
                RefusalCode::NotObservable202,
                format!(
                    "INV-008 late registration refused: registered_ns ({registered_ns}) must precede first_request_ns ({first_request_ns})"
                ),
            ));
        }
        let slot_key = format!("{NORTH_MARA_SITE}/analysis_plan");
        if ledger.contains_slot(&slot_key) {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                format!("frozen slot '{slot_key}' already registered; mutation refused"),
            ));
        }

        let attributable_conflicts =
            check_attributable_share_conflicts(spec.ownership_pct, share_probes)?;
        let labelled = label_grade_led_candidate_files(family, &spec.candidate_files, top_n)?;
        let desc_map: BTreeMap<&str, &str> = spec
            .candidate_files
            .iter()
            .map(|c| (c.name.as_str(), c.description.as_str()))
            .collect();

        let mut files = Vec::with_capacity(labelled.len());
        for (name, label, sel, total) in labelled {
            let subset = [name.as_str()];
            let mut envelopes = Vec::with_capacity(GradeLedCause::ALL.len());
            for cause in GradeLedCause::ALL {
                envelopes.push(estimate_grade_led_power_envelope(
                    family, &subset, cause, replicates, seed_root,
                )?);
            }
            let desc = desc_map.get(name.as_str()).copied().unwrap_or("");
            let reason = if desc.trim().is_empty() {
                format!("selected in {sel}/{total} grade-led twins for north_mara")
            } else {
                format!("{desc} (selected in {sel}/{total} twins)")
            };
            files.push(GradeLedFileRow {
                name,
                reason,
                label,
                envelope_by_cause: envelopes,
            });
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));

        let envelope_too_wide_fallback = !files.iter().any(|f| f.label == FileLabel::Robust);
        let all_names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        let mut full_envelopes = Vec::with_capacity(GradeLedCause::ALL.len());
        for cause in GradeLedCause::ALL {
            full_envelopes.push(estimate_grade_led_power_envelope(
                family, &all_names, cause, replicates, seed_root,
            )?);
        }
        let early_kill = evaluate_grade_led_early_kill(&full_envelopes, plan.power_bar_target);

        let mut all_input_digests = public_input_digests.to_vec();
        all_input_digests.push(family.family_digest);
        let inputs_digest = compute_inputs_digest(&all_input_digests);

        let mut sorted_seeds = plan.seeds.clone();
        sorted_seeds.sort_unstable();
        sorted_seeds.dedup();
        let card_digest = compute_card_digest(inputs_digest, &sorted_seeds, &plan.code_rev)?;

        let analysis_plan_id = ledger.register_slot(
            &slot_key,
            FrozenArtifactKind::AnalysisPlan,
            &plan_bytes,
            registered_ns,
            first_request_ns,
        )?;

        let card = NorthMaraPilotCard {
            site: NORTH_MARA_SITE.to_string(),
            ownership_pct: spec.ownership_pct,
            mechanism: NORTH_MARA_MECHANISM.to_string(),
            attribution_question: plan.attribution_question.trim().to_string(),
            k_twins: family.twins.len(),
            files,
            analysis_plan_id,
            early_kill,
            envelope_too_wide_fallback,
            residency_statement: TANZANIA_RESIDENCY_STATEMENT.to_string(),
            attributable_conflicts,
            inputs_digest,
            seeds: sorted_seeds,
            code_rev: plan.code_rev.trim().to_string(),
            card_digest,
            label: SYNTHETIC_PUBLIC_LABEL,
        };
        card.validate()?;
        Ok(card)
    }

    #[cfg(test)]
    mod tests {
        use std::path::PathBuf;

        use super::*;
        use crate::pilot_card::FindingDefinition;

        fn workspace_root() -> PathBuf {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|p| p.parent())
                .expect("workspace root")
                .to_path_buf()
        }

        fn load_north_mara_spec() -> GradeLedRegistrySpec {
            let text =
                std::fs::read_to_string(workspace_root().join("registries/pilot_twins.toml"))
                    .expect("read pilot_twins.toml");
            parse_north_mara_registry(&text).expect("parse north_mara spec")
        }

        /// Synthetic 100%-basis quarterly series shaped like North Mara 2024Q4..2026Q2
        /// (E493-E495, E587-E589: grade 4.29 -> 1.80 g/t, recovery fall grade-driven with
        /// implied tails 0.38 -> 0.27 g/t, AISC $1,098 -> $2,577/oz, Gokona pre-strip in 2026Q1).
        fn synthetic_north_mara_100_series() -> (Vec<NorthMaraQuarterTarget>, Vec<CitedSeries>) {
            let rows: [(&str, f64, f64, f64, f64, f64); 7] = [
                ("2024Q4", 710.0, 4.29, 91.1, 89.2, 1_098.0),
                ("2025Q1", 705.0, 3.65, 90.2, 74.6, 1_310.0),
                ("2025Q2", 715.0, 3.30, 89.5, 67.9, 1_455.0),
                ("2025Q3", 720.0, 2.95, 88.8, 60.6, 1_640.0),
                ("2025Q4", 725.0, 2.55, 87.9, 52.3, 1_890.0),
                ("2026Q1", 690.0, 2.05, 86.2, 39.2, 2_280.0),
                ("2026Q2", 710.0, 1.80, 85.0, 35.0, 2_577.0),
            ];
            let mut targets = Vec::with_capacity(rows.len());
            let mut grade_pts = Vec::with_capacity(rows.len());
            let mut rec_pts = Vec::with_capacity(rows.len());
            let mut ore_pts = Vec::with_capacity(rows.len());
            let mut au_pts = Vec::with_capacity(rows.len());
            for (q, ore, g, rec, au100, aisc) in rows {
                targets.push(NorthMaraQuarterTarget {
                    quarter: q.to_string(),
                    ore_processed_kt: ore,
                    head_grade_g_t: g,
                    recovery_pct: rec,
                    gold_produced_100_koz: au100,
                    aisc_per_oz: aisc,
                });
                grade_pts.push(CitedPoint {
                    quarter: q.to_string(),
                    cell: format!("{q}:Minestats1!B15"),
                    source_sha256: "synthetic-unit-test".to_string(),
                    printed_value: g,
                    rounding_step: 0.01,
                });
                rec_pts.push(CitedPoint {
                    quarter: q.to_string(),
                    cell: format!("{q}:Minestats1!B16"),
                    source_sha256: "synthetic-unit-test".to_string(),
                    printed_value: rec,
                    rounding_step: 0.1,
                });
                ore_pts.push(CitedPoint {
                    quarter: q.to_string(),
                    cell: format!("{q}:Minestats1!B13"),
                    source_sha256: "synthetic-unit-test".to_string(),
                    printed_value: ore,
                    rounding_step: 1.0,
                });
                au_pts.push(CitedPoint {
                    quarter: q.to_string(),
                    cell: format!("{q}:Minestats1!B17"),
                    source_sha256: "synthetic-unit-test".to_string(),
                    printed_value: au100,
                    rounding_step: 0.1,
                });
            }
            let series = vec![
                CitedSeries {
                    series_name: "head_grade".to_string(),
                    unit: "g/t".to_string(),
                    points: grade_pts,
                },
                CitedSeries {
                    series_name: "recovery".to_string(),
                    unit: "pct".to_string(),
                    points: rec_pts,
                },
                CitedSeries {
                    series_name: "ore_processed".to_string(),
                    unit: "kt".to_string(),
                    points: ore_pts,
                },
                CitedSeries {
                    series_name: "gold_produced_100".to_string(),
                    unit: "koz".to_string(),
                    points: au_pts,
                },
            ];
            (targets, series)
        }

        fn sample_north_mara_plan(seed_root: u64, code_rev: &str) -> AnalysisPlan {
            AnalysisPlan {
                site: NORTH_MARA_SITE.to_string(),
                attribution_question: "Which fraction of the 2024Q4-2026Q2 North Mara grade drop (4.29 -> 1.80 g/t) is driven by Gokona pre-strip/ore delivery vs. GC/resource model bias or dilution?".to_string(),
                finding_definitions: vec![
                    FindingDefinition {
                        name: "gc_vs_resource_bias".to_string(),
                        definition: "Sustained >= 8% grade discrepancy between GC model, resource model, and mill head grade.".to_string(),
                    },
                    FindingDefinition {
                        name: "dilution_ore_loss".to_string(),
                        definition: "Mined-to-mill dilution/ore-loss shift exceeding 5 percentage points.".to_string(),
                    },
                ],
                materiality_threshold_pct: 5.0,
                code_rev: code_rev.to_string(),
                seeds: vec![seed_root],
                stopping_rule: "Freeze after 8-12 historical quarters evaluated; stop early if power bar fails across all twins.".to_string(),
                null_criteria: vec![
                    "No model bias exceeds materiality threshold once Gokona pre-strip ore-delivery mix is accounted for.".to_string(),
                ],
                power_bar_target: 0.80,
                early_kill_rule: "If no feasible subset clears power_bar_target across all K twins, escalate alternate site (Kibali).".to_string(),
            }
        }

        #[test]
        fn test_north_mara_ore_selector_refuses_wrong_share_and_repeated_rows() {
            use fmine_public::mine_stats::MineStatsValue;

            let field = PARITY_FIELDS[2];
            let mut read = MineStatsRead {
                quarter: "2030Q1".into(),
                blocks: 1,
                values: vec![MineStatsValue {
                    locator: "2030Q1:Minestats3!H12".into(),
                    sheet: "Minestats3".into(),
                    cell: "H12".into(),
                    view: View::Minestats,
                    months: 3,
                    period: "2030Q1".into(),
                    period_label: "For the three months ended March 31,".into(),
                    site_raw: "North Mara (84%)".into(),
                    site: "North Mara".into(),
                    printed_share: Some("84".into()),
                    label: field.label.into(),
                    label_as_printed: field.label.into(),
                    footnote: None,
                    section: None,
                    value: MineStatsCell::Number {
                        raw_text: "840".into(),
                        value: 840.0,
                    },
                }],
                refusals: Vec::new(),
                hidden_sheets: Vec::new(),
                repeated: Vec::new(),
            };
            let point = field
                .point(&read, "synthetic-test")
                .expect("84% ore reading");
            assert_eq!(point.printed_value, 840.0);
            read.values[0].printed_share = Some("80".into());
            assert!(
                field
                    .point(&read, "synthetic-test")
                    .unwrap_err()
                    .what
                    .contains("84% share")
            );
            read.values[0].printed_share = Some("84".into());
            read.repeated.push("2030Q1:Minestats3!A12".into());
            assert!(
                field
                    .point(&read, "synthetic-test")
                    .unwrap_err()
                    .what
                    .contains("ambiguous quarterly cell")
            );
        }

        #[test]
        fn test_north_mara_family_refuses_incomplete_quarterly_evidence() {
            let spec = load_north_mara_spec();
            let (targets, public) = synthetic_north_mara_100_series();
            let missing_series = &public[..3];
            let mut missing_quarter = public.clone();
            missing_quarter[0].points.pop();
            let mut duplicate_quarter = public.clone();
            let duplicate = duplicate_quarter[0].points[0].clone();
            duplicate_quarter[0].points.push(duplicate);
            let mut blank_citation = public.clone();
            blank_citation[0].points[0].source_sha256.clear();
            for (input, expected) in [
                (missing_series, "requires all 4 operating series"),
                (&missing_quarter, "missing head_grade evidence for 2026Q2"),
                (
                    &duplicate_quarter,
                    "unusable head_grade evidence for 2024Q4",
                ),
                (&blank_citation, "unusable head_grade evidence for 2024Q4"),
            ] {
                let refusal =
                    NorthMaraTwinFamily::build_and_verify(&spec, 20_263_564, &targets, input)
                        .expect_err("incomplete evidence must refuse");
                assert_eq!(refusal.code, RefusalCode::SourceCellUnusable208.as_str());
                assert!(refusal.what.contains(expected), "{refusal}");
            }
        }

        #[test]
        fn test_north_mara_attributable_derived_never_fitted() {
            let spec = load_north_mara_spec();
            let (targets, public) = synthetic_north_mara_100_series();
            let (family, parity_logs) =
                NorthMaraTwinFamily::build_and_verify(&spec, 20_263_564, &targets, &public)
                    .expect("build north mara family");
            assert_eq!(family.twins.len(), spec.k);
            assert_eq!(parity_logs.len(), spec.k * 4 * targets.len());

            // Every twin's attributable koz equals 0.84 * 100%-basis koz to floating precision,
            // and implied tails track the grade-driven recovery fall (0.38 -> 0.27 g/t, E589).
            for tw in &family.twins {
                assert!(spec.axis_ranges.contains(tw.axes));
                let aggs = tw.quarterly_aggregates();
                assert!((aggs[0].implied_tails_g_t - 0.38181).abs() < 0.01);
                assert!((aggs[6].implied_tails_g_t - 0.2700).abs() < 0.01);
                // 2026Q1 Gokona pre-strip segment boundary respected.
                assert!(aggs[5].segment_boundary_reset);
                assert!(!aggs[6].segment_boundary_reset);
                for agg in aggs {
                    let expected_attr = agg.gold_produced_100_koz * 0.84;
                    assert!(
                        (agg.derived_attributable_koz - expected_attr).abs() < 1e-12,
                        "attributable must equal exact 84% of 100%-basis"
                    );
                }
            }

            // Passing an attributable series to check_parity is refused (never fitted).
            let attr_series = vec![CitedSeries {
                series_name: "gold_produced_attributable".to_string(),
                unit: "koz".to_string(),
                points: vec![],
            }];
            assert_eq!(
                family.twins[0].check_parity(&attr_series).unwrap_err().code,
                RefusalCode::BasisMismatch105.spec().code
            );
        }

        #[test]
        fn test_grade_led_file_labels_and_envelope() {
            let spec = load_north_mara_spec();
            assert_eq!(spec.candidate_files.len(), 7);
            let (targets, public) = synthetic_north_mara_100_series();
            let (family, _) =
                NorthMaraTwinFamily::build_and_verify(&spec, 20_263_564, &targets, &public)
                    .expect("build family");

            let plan = sample_north_mara_plan(20_263_564, "rev-nm-001");
            let mut ledger = ArtifactLedger::new();
            let card = build_north_mara_pilot_card(
                &family,
                &spec,
                &plan,
                &mut ledger,
                100,
                500,
                &[sha256(b"north-mara-public-2024q4-2026q2")],
                &[],
                80,
                20_263_564,
                4,
            )
            .expect("build north mara card");

            assert_eq!(card.files.len(), 7);
            assert!(
                card.files.iter().any(|f| f.label == FileLabel::Robust),
                "core grade-led files must be robust across twins"
            );
            assert!(
                card.files
                    .iter()
                    .any(|f| matches!(f.label, FileLabel::ModelDependent { .. })),
                "auxiliary files must be labelled model-dependent"
            );
            for row in &card.files {
                assert_eq!(row.envelope_by_cause.len(), GradeLedCause::ALL.len());
                for env in &row.envelope_by_cause {
                    assert_eq!(env.twins_evaluated, spec.k);
                    assert!(env.min_power <= env.max_power);
                    let rendered = env.render_envelope_string().expect("envelope string");
                    assert!(rendered.contains('[') && rendered.contains(']'));
                }
            }

            // Without block_and_gc_models, GC vs. resource bias attribution refuses (FMINE-REF-104).
            let confounded = estimate_grade_led_power_envelope(
                &family,
                &["metallurgical_balance"],
                GradeLedCause::GcModelBias,
                80,
                20_263_564,
            )
            .expect("envelope");
            assert_eq!(
                confounded
                    .identifiability_refusal
                    .as_ref()
                    .expect("FMINE-REF-104")
                    .code,
                RefusalCode::NonIdentifiableAttribution104.spec().code
            );

            // Personal / roster data ask is refused (FMINE-REF-301).
            let roster_err = estimate_grade_led_power_envelope(
                &family,
                &["shift_roster_fatigue"],
                GradeLedCause::OreDeliveryShortfall,
                80,
                20_263_564,
            )
            .unwrap_err();
            assert_eq!(roster_err.code, RefusalCode::MnpiWall301.spec().code);
        }

        #[test]
        fn test_north_mara_card_residency_statement_present() {
            let spec = load_north_mara_spec();
            let (targets, public) = synthetic_north_mara_100_series();
            let (family, _) =
                NorthMaraTwinFamily::build_and_verify(&spec, 20_263_564, &targets, &public)
                    .expect("family");
            let plan = sample_north_mara_plan(20_263_564, "rev-nm-001");
            let mut ledger = ArtifactLedger::new();
            let mut card = build_north_mara_pilot_card(
                &family,
                &spec,
                &plan,
                &mut ledger,
                100,
                500,
                &[sha256(b"north-mara-public")],
                &[],
                60,
                20_263_564,
                4,
            )
            .expect("card");

            let md = card.render_markdown().expect("md");
            assert!(md.contains("Mining Act s.35(6)"));
            assert!(md.contains("Geological Survey"));
            assert!(md.contains("Tanzania"));
            assert!(md.contains("Twiga"));
            assert!(md.contains(SYNTHETIC_PUBLIC_LABEL));
            assert!(md.contains("| `reserve_disclosure_claim` | `false` |"));
            assert!(md.contains("| `execution_authorized` | `false` |"));
            assert!(md.contains("| `geotechnical_signoff` | `false` |"));
            assert!(md.contains("| `regulatory_compliance_claim` | `false` |"));

            let events = card.render_json_events().expect("json");
            let summary = events.last().expect("summary event");
            assert!(summary.contains("\"event\":\"pilot_card\""));
            assert!(summary.contains("\"site\":\"north_mara\""));
            assert!(summary.contains("Mining Act s.35(6)"));
            assert!(summary.contains("\"reserve_disclosure_claim\":false"));

            // Stripping the residency statement refuses fail-closed with FMINE-REF-303.
            card.residency_statement = "none".to_string();
            assert_eq!(
                card.validate().unwrap_err().code,
                RefusalCode::Residency303.spec().code
            );
        }

        #[test]
        fn test_north_mara_card_digest_pure() {
            let spec = load_north_mara_spec();
            let (targets, public) = synthetic_north_mara_100_series();
            let (family1, _) =
                NorthMaraTwinFamily::build_and_verify(&spec, 20_263_564, &targets, &public)
                    .expect("f1");
            let (family2, _) =
                NorthMaraTwinFamily::build_and_verify(&spec, 20_263_564, &targets, &public)
                    .expect("f2");
            let (family_diff_seed, _) =
                NorthMaraTwinFamily::build_and_verify(&spec, 20_263_565, &targets, &public)
                    .expect("f3");

            let d1 = sha256(b"input-a");
            let d2 = sha256(b"input-b");
            let plan_a = sample_north_mara_plan(20_263_564, "rev-nm-001");
            let plan_rev = sample_north_mara_plan(20_263_564, "rev-nm-002");
            let plan_seed = sample_north_mara_plan(20_263_565, "rev-nm-001");

            let c1 = build_north_mara_pilot_card(
                &family1,
                &spec,
                &plan_a,
                &mut ArtifactLedger::new(),
                100,
                500,
                &[d1, d2],
                &[],
                60,
                20_263_564,
                4,
            )
            .expect("c1");
            // Reversed input order and different pre-freeze timestamps produce identical card_digest.
            let c2 = build_north_mara_pilot_card(
                &family2,
                &spec,
                &plan_a,
                &mut ArtifactLedger::new(),
                250,
                900,
                &[d2, d1],
                &[],
                60,
                20_263_564,
                4,
            )
            .expect("c2");
            assert_eq!(c1.card_digest, c2.card_digest);

            let c_rev = build_north_mara_pilot_card(
                &family1,
                &spec,
                &plan_rev,
                &mut ArtifactLedger::new(),
                100,
                500,
                &[d1, d2],
                &[],
                60,
                20_263_564,
                4,
            )
            .expect("c_rev");
            assert_ne!(c1.card_digest, c_rev.card_digest);

            let c_seed = build_north_mara_pilot_card(
                &family_diff_seed,
                &spec,
                &plan_seed,
                &mut ArtifactLedger::new(),
                100,
                500,
                &[d1, d2],
                &[],
                60,
                20_263_565,
                4,
            )
            .expect("c_seed");
            assert_ne!(c1.card_digest, c_seed.card_digest);

            let c_inp = build_north_mara_pilot_card(
                &family1,
                &spec,
                &plan_a,
                &mut ArtifactLedger::new(),
                100,
                500,
                &[d1, sha256(b"input-c")],
                &[],
                60,
                20_263_564,
                4,
            )
            .expect("c_inp");
            assert_ne!(c1.card_digest, c_inp.card_digest);
        }

        /// Negative control (`BD-356.S4`, `E819`): a synthetic implied share interval excluding
        /// 84% (such as Q2 2026 CoS/oz `[0.8498, 0.8808]`) is reported as a conflict without
        /// altering the 84% derivation or forcing attributable parity.
        #[test]
        fn test_attributable_share_conflict_reported_not_forced() {
            let spec = load_north_mara_spec();
            let (targets, public) = synthetic_north_mara_100_series();
            let (family, _) =
                NorthMaraTwinFamily::build_and_verify(&spec, 20_263_564, &targets, &public)
                    .expect("family");

            let probes = vec![
                ImpliedShareProbe {
                    quarter: "2026Q2".to_string(),
                    cell: "2026Q2:QTD!K29".to_string(),
                    metric: "cost_of_sales_per_oz".to_string(),
                    implied_share_lo: 0.8498,
                    implied_share_hi: 0.8808,
                    source_e_id: "E819".to_string(),
                },
                ImpliedShareProbe {
                    quarter: "2026Q1".to_string(),
                    cell: "2026Q1:QTD!K29".to_string(),
                    metric: "cost_of_sales_per_oz".to_string(),
                    implied_share_lo: 0.8300,
                    implied_share_hi: 0.8500,
                    source_e_id: "synthetic-consistent-control".to_string(),
                },
            ];
            let plan = sample_north_mara_plan(20_263_564, "rev-nm-001");
            let card = build_north_mara_pilot_card(
                &family,
                &spec,
                &plan,
                &mut ArtifactLedger::new(),
                100,
                500,
                &[sha256(b"north-mara-public")],
                &probes,
                60,
                20_263_564,
                4,
            )
            .expect("card");

            assert_eq!(card.ownership_pct, 84);
            assert_eq!(card.attributable_conflicts.len(), 1);
            let c = &card.attributable_conflicts[0];
            assert_eq!(c.quarter, "2026Q2");
            assert_eq!(c.stated_share_pct, 84);
            assert!(c.detail.contains("0.8498") && c.detail.contains("0.8808"));
            assert!(c.detail.contains("reported, not forced"));

            // Twin attributable figures remain exact 84% of 100%-basis (never fitted to the conflict).
            let q2 = family.twins[0]
                .quarterly_aggregates()
                .last()
                .expect("2026Q2");
            assert!((q2.derived_attributable_koz - q2.gold_produced_100_koz * 0.84).abs() < 1e-12);

            let md = card.render_markdown().expect("md");
            assert!(md.contains("0.8498") && md.contains("0.8808") && md.contains("E819"));
            let events = card.render_json_events().expect("json");
            assert!(
                events
                    .last()
                    .expect("summary")
                    .contains("\"attributable_conflicts\":1")
            );
        }

        #[test]
        fn test_north_mara_early_kill_recommends_kibali_and_renders_fallback() {
            let spec = load_north_mara_spec();
            let (targets, public) = synthetic_north_mara_100_series();
            let (family, _) =
                NorthMaraTwinFamily::build_and_verify(&spec, 20_263_564, &targets, &public)
                    .expect("family");

            // Unreachable 0.999 bar fires early-kill and must recommend the NEXT §37.3 site
            // ("Kibali"), never self-referentially recommending "North Mara".
            let mut strict_plan = sample_north_mara_plan(20_263_564, "rev-nm-ek");
            strict_plan.power_bar_target = 0.999;
            let mut card = build_north_mara_pilot_card(
                &family,
                &spec,
                &strict_plan,
                &mut ArtifactLedger::new(),
                100,
                500,
                &[sha256(b"north-mara-public")],
                &[],
                60,
                20_263_564,
                4,
            )
            .expect("card");

            assert!(card.early_kill.fires, "{:?}", card.early_kill);
            assert_eq!(
                card.early_kill.alternate_recommendation.as_deref(),
                Some(NORTH_MARA_EARLY_KILL_ALTERNATE)
            );
            assert!(
                !card
                    .early_kill
                    .alternate_recommendation
                    .as_deref()
                    .unwrap_or("")
                    .contains("North Mara"),
                "North Mara card must not recommend North Mara as its own alternate"
            );

            // Activate envelope_too_wide_fallback and verify both Markdown and JSON emit it.
            card.envelope_too_wide_fallback = true;
            let md = card.render_markdown().expect("md");
            assert!(md.contains("Alternate site recommendation (§37.3):** Kibali"));
            assert!(md.contains("Fallback active: no candidate file is robust across all twins"));

            let events = card.render_json_events().expect("json");
            let summary = events.last().expect("summary event");
            assert!(summary.contains("\"alternate\":\"Kibali\""));
            assert!(summary.contains("\"envelope_too_wide_fallback\":true"));
        }

        #[test]
        #[ignore = "needs the registered Mine Stats workbooks under corpus_of_input_docs/"]
        fn test_north_mara_twins_parity_real_workbooks() {
            let root = workspace_root();
            let profile = fmine_public::load_issuer_profile(&root, "barrick")
                .expect("load barrick issuer profile");
            let expected_quarters: BTreeSet<&str> = profile
                .sources
                .iter()
                .filter(|source| source.kind == SourceKind::MineStats)
                .map(|source| source.quarter.as_str())
                .collect();
            let inputs = load_north_mara_public_inputs(&root)
                .expect("bind every registered North Mara quarter");
            assert_eq!(
                inputs
                    .targets
                    .iter()
                    .map(|target| target.quarter.as_str())
                    .collect::<Vec<_>>(),
                expected_quarters.into_iter().collect::<Vec<_>>()
            );
            assert_eq!(inputs.aisc_context.points.len(), inputs.targets.len());
            assert_eq!(inputs.workbook_digests.len(), inputs.targets.len());
            let q2 = inputs
                .targets
                .iter()
                .position(|t| t.quarter == "2026Q2")
                .expect("E589 quarter");
            assert_eq!(inputs.targets[q2].ore_processed_kt, 688.0 / 0.84);
            assert_eq!(inputs.parity_series[2].series_name, ORE_ATTRIBUTABLE);
            assert_eq!(inputs.parity_series[2].points[q2].printed_value, 688.0);
            assert_eq!(inputs.targets[q2].head_grade_g_t, 1.80);
            assert_eq!(inputs.targets[q2].recovery_pct, 85.0);
            assert_eq!(inputs.targets[q2].gold_produced_100_koz, 41.0);
            assert_eq!(inputs.targets[q2].aisc_per_oz, 2577.0);
            let digest = inputs.workbook_digests[q2].to_hex();
            for (point, cell) in inputs
                .parity_series
                .iter()
                .map(|series| &series.points[q2])
                .chain(std::iter::once(&inputs.aisc_context.points[q2]))
                .zip([
                    "Minestats3!H18",
                    "Minestats3!H19",
                    "Minestats3!H12",
                    "QTD!B19",
                    "QTD!G19",
                ])
            {
                assert_eq!(point.cell, format!("2026Q2:{cell}"));
                assert_eq!(point.source_sha256, digest);
            }
            let spec = load_north_mara_spec();
            let (family, parity_logs) = NorthMaraTwinFamily::build_and_verify(
                &spec,
                20_263_564,
                &inputs.targets,
                &inputs.parity_series,
            )
            .expect("verify real-workbook 100%-basis parity");
            assert_eq!(
                parity_logs.len(),
                spec.k * inputs.targets.len() * PARITY_FIELDS.len()
            );
            for line in &parity_logs {
                assert!(line.contains("\"event\":\"twin_parity\""));
                assert!(line.contains("\"ok\":true"));
            }
            let plan = sample_north_mara_plan(20_263_564, "real-corpus-run");
            let card = build_north_mara_pilot_card(
                &family,
                &spec,
                &plan,
                &mut ArtifactLedger::new(),
                100,
                500,
                &inputs.workbook_digests,
                &[],
                80,
                20_263_564,
                4,
            )
            .expect("build card");
            let events = card.render_json_events().expect("json");
            assert!(
                events
                    .last()
                    .expect("summary")
                    .contains("\"event\":\"pilot_card\"")
            );
            println!(
                "North Mara: {} registered quarters, {} twin parity comparisons",
                inputs.targets.len(),
                parity_logs.len()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use fmine_core::hash::sha256;

    use crate::plant_twin::{CitedPoint, CitedSeries, QuarterAgg, parse_pilot_twins_registry};

    fn load_lumwana_family(k: usize) -> (PilotTwinRegistrySpec, TwinFamily) {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let text = std::fs::read_to_string(root.join("registries/pilot_twins.toml"))
            .expect("read registries/pilot_twins.toml");
        let mut spec = parse_pilot_twins_registry(&text, "lumwana").expect("parse lumwana spec");
        spec.k = k;
        let targets = vec![
            QuarterAgg {
                quarter: "2025Q1".to_string(),
                throughput_kt: 5_250.0,
                head_grade_cu_pct: 0.57,
                recovery_pct: 91.0,
                copper_kt: 27.2,
                segment_boundary_reset: true,
            },
            QuarterAgg {
                quarter: "2025Q2".to_string(),
                throughput_kt: 7_080.0,
                head_grade_cu_pct: 0.67,
                recovery_pct: 92.0,
                copper_kt: 43.6,
                segment_boundary_reset: false,
            },
        ];
        let public = vec![CitedSeries {
            series_name: "recovery".to_string(),
            unit: "pct".to_string(),
            points: vec![
                CitedPoint {
                    quarter: "2025Q1".to_string(),
                    cell: "2025Q1:Minestats4!D14".to_string(),
                    source_sha256: "synth".to_string(),
                    printed_value: 91.0,
                    rounding_step: 1.0,
                },
                CitedPoint {
                    quarter: "2025Q2".to_string(),
                    cell: "2025Q2:Minestats4!D14".to_string(),
                    source_sha256: "synth".to_string(),
                    printed_value: 92.0,
                    rounding_step: 1.0,
                },
            ],
        }];
        let family =
            TwinFamily::build_and_verify(&spec, 2_026_356, &targets, &public).expect("family");
        (spec, family)
    }

    fn sample_plan(power_bar_target: f64) -> AnalysisPlan {
        AnalysisPlan {
            site: "lumwana".to_string(),
            attribution_question: "Which mechanism explains the >=1.5 pp Cu recovery drop across 2025Q1-Q2?"
                .to_string(),
            finding_definitions: vec![
                FindingDefinition {
                    name: "head_sampler_bias".to_string(),
                    definition: "Systematic positive bias in plant head grade sampler >= 1.5 pp"
                        .to_string(),
                },
                FindingDefinition {
                    name: "transitional_ore_share".to_string(),
                    definition: "Elevated transitional/oxide ore blend reducing flotation recovery"
                        .to_string(),
                },
            ],
            materiality_threshold_pct: 1.5,
            code_rev: "rev-2026-11-25-test".to_string(),
            seeds: vec![2_026_356, 2_026_357],
            stopping_rule: "Stop after 4-month GATE-DATA-01 readout; no peeking prior to freeze"
                .to_string(),
            null_criteria: vec![
                "No cause exceeds posterior probability 0.80 at 1.5 pp threshold".to_string(),
                "Pairwise posterior correlation |corr| > 0.80 refuses individual split".to_string(),
            ],
            power_bar_target,
            early_kill_rule:
                "If no feasible subset reaches power_bar_target under every twin, open alternate site at month 1"
                    .to_string(),
        }
    }

    #[test]
    fn test_card_digest_pure_function_of_inputs_seeds_code_rev() {
        let d1 = sha256(b"barrick-q1-2025");
        let d2 = sha256(b"barrick-q2-2025");
        let inputs_a = compute_inputs_digest(&[d1, d2]);
        let inputs_b = compute_inputs_digest(&[d2, d1]);
        assert_eq!(inputs_a, inputs_b);

        let c1 = compute_card_digest(inputs_a, &[11, 22], "abc1234").expect("digest 1");
        let c2 = compute_card_digest(inputs_b, &[22, 11], "abc1234").expect("digest 2");
        assert_eq!(c1, c2);

        // Changing code_rev or inputs_digest changes card_digest.
        let c_rev = compute_card_digest(inputs_a, &[11, 22], "def5678").expect("digest rev");
        assert_ne!(c1, c_rev);
        let inputs_c = compute_inputs_digest(&[d1, sha256(b"barrick-q3-2025")]);
        let c_in = compute_card_digest(inputs_c, &[11, 22], "abc1234").expect("digest in");
        assert_ne!(c1, c_in);
    }

    #[test]
    fn test_card_digest_changes_with_seed() {
        let inputs = compute_inputs_digest(&[sha256(b"lumwana-public-series")]);
        let d_a = compute_card_digest(inputs, &[2_026_356], "rev-1").expect("seed a");
        let d_b = compute_card_digest(inputs, &[2_026_999], "rev-1").expect("seed b");
        assert_ne!(d_a, d_b);
    }

    #[test]
    fn test_analysis_plan_registration_refuses_after_freeze() {
        let plan = sample_plan(0.80);
        let bytes = plan.canonical_bytes().expect("canonical bytes");
        let mut ledger = ArtifactLedger::new();

        // Exact boundary (registered_ns == freeze_time_ns) and late registration MUST refuse (FMINE-REF-202).
        let eq_err = ledger
            .register(FrozenArtifactKind::AnalysisPlan, &bytes, 1_000, 1_000)
            .expect_err("equal timestamp must refuse");
        assert_eq!((eq_err.code, eq_err.exit_code), ("FMINE-REF-202", 3));

        let late_err = ledger
            .register(FrozenArtifactKind::AnalysisPlan, &bytes, 1_001, 1_000)
            .expect_err("late registration must refuse");
        assert_eq!((late_err.code, late_err.exit_code), ("FMINE-REF-202", 3));

        // Strictly earlier registration succeeds.
        let id = ledger
            .register(FrozenArtifactKind::AnalysisPlan, &bytes, 999, 1_000)
            .expect("pre-freeze registration succeeds");
        assert_eq!(id.kind, PrefixKind::Forecast);
        let rec = ledger.get(&id).expect("stored receipt");
        assert_eq!(rec.digest, blake3_hash(&bytes));
        assert!(rec.registered_ns < rec.freeze_time_ns);
    }

    #[test]
    fn test_analysis_plan_mutation_refused() {
        let plan = sample_plan(0.80);
        let bytes = plan.canonical_bytes().expect("bytes");
        let mut ledger = ArtifactLedger::new();
        let id = ledger
            .register_slot(
                "lumwana/analysis_plan",
                FrozenArtifactKind::AnalysisPlan,
                &bytes,
                100,
                500,
            )
            .expect("initial registration");
        ledger
            .verify_unmutated(&id, &bytes)
            .expect("unchanged bytes verify");

        // Mutating any field of the plan fails both verify_unmutated and slot re-registration (FMINE-REF-507).
        let mut mutated = plan.clone();
        mutated.materiality_threshold_pct = 2.0;
        let mutated_bytes = mutated.canonical_bytes().expect("mutated bytes");

        let verify_err = ledger
            .verify_unmutated(&id, &mutated_bytes)
            .expect_err("mutated bytes must refuse");
        assert_eq!(
            (verify_err.code, verify_err.exit_code),
            ("FMINE-REF-507", 6)
        );

        let slot_err = ledger
            .register_slot(
                "lumwana/analysis_plan",
                FrozenArtifactKind::AnalysisPlan,
                &mutated_bytes,
                200,
                500,
            )
            .expect_err("slot overwrite must refuse");
        assert_eq!((slot_err.code, slot_err.exit_code), ("FMINE-REF-507", 6));
    }

    #[test]
    fn test_analysis_plan_digest_stable_under_field_order() {
        let plan_a = sample_plan(0.80);
        let mut plan_b = plan_a.clone();
        plan_b.finding_definitions.reverse();
        plan_b.seeds.reverse();
        plan_b.null_criteria.reverse();

        assert_eq!(
            plan_a.canonical_bytes().unwrap(),
            plan_b.canonical_bytes().unwrap()
        );
        assert_eq!(plan_a.digest().unwrap(), plan_b.digest().unwrap());
    }

    #[test]
    fn test_early_kill_recommends_alternate_site_when_no_subset_reaches_bar() {
        let (spec, family) = load_lumwana_family(5);
        // Set an unreachable 0.999 power bar across all twins -> early kill MUST fire and recommend
        // "North Mara with Bulyanhulu, then Kibali" (§37.3).
        let strict_plan = sample_plan(0.999);
        let mut ledger = ArtifactLedger::new();
        let card = build_pilot_protocol_card(
            &family,
            &spec,
            "Flotation recovery gap vs. sampler bias",
            &strict_plan,
            &mut ledger,
            1_000,
            2_000,
            &[sha256(b"lumwana-2025q1-q2")],
            200,
            2_026_356,
            3,
        )
        .expect("build card");

        assert!(card.early_kill.fires, "{:?}", card.early_kill);
        assert_eq!(
            card.early_kill.alternate_recommendation.as_deref(),
            Some(EARLY_KILL_ALTERNATE_SITES)
        );
        let md = card.render_markdown().expect("render md");
        assert!(md.contains("North Mara with Bulyanhulu, then Kibali"));
        assert!(md.contains("Early kill fires:** `true`"));
    }

    #[test]
    fn test_card_without_envelope_rejected() {
        let (spec, family) = load_lumwana_family(3);
        let plan = sample_plan(0.80);
        let mut ledger = ArtifactLedger::new();
        let mut card = build_pilot_protocol_card(
            &family,
            &spec,
            "Flotation recovery attribution",
            &plan,
            &mut ledger,
            100,
            500,
            &[sha256(b"lumwana-inputs")],
            200,
            2_026_356,
            3,
        )
        .expect("build valid card");

        // Strip envelope -> validate(), render_json_events(), and render_markdown() MUST all refuse.
        card.files[0].envelope_by_cause.clear();
        assert_eq!(card.validate().unwrap_err().code, "FMINE-REF-109");
        assert_eq!(card.render_json_events().unwrap_err().code, "FMINE-REF-109");
        assert_eq!(card.render_markdown().unwrap_err().code, "FMINE-REF-109");

        // Single-twin envelope (twins_evaluated = 1) must also refuse.
        let fname = card.files[0].name.clone();
        card.files[0].envelope_by_cause.push(PowerEnvelope {
            cause: AttributionCause::HeadSamplerBias,
            subset: vec![fname],
            twins_evaluated: 1,
            min_power: 0.85,
            max_power: 0.85,
            max_mc_se: 0.02,
            identifiability_refusal: None,
        });
        assert_eq!(card.validate().unwrap_err().code, "FMINE-REF-109");
    }

    #[test]
    fn test_card_render_flags_false_and_labels() {
        let (spec, family) = load_lumwana_family(5);
        let plan = sample_plan(0.70);
        let mut ledger = ArtifactLedger::new();
        let card = build_pilot_protocol_card(
            &family,
            &spec,
            "Cu flotation recovery attribution",
            &plan,
            &mut ledger,
            100,
            500,
            &[sha256(b"lumwana-public-filings")],
            200,
            2_026_356,
            3,
        )
        .expect("build card");

        assert_eq!(card.label, SYNTHETIC_PUBLIC_LABEL);
        assert_eq!(card.files.len(), 5);
        for f in &card.files {
            assert!(!f.reason.trim().is_empty());
            assert_eq!(f.envelope_by_cause.len(), AttributionCause::ALL.len());
        }

        let events = card.render_json_events().expect("render json events");
        assert_eq!(
            events.len(),
            card.files.len() * AttributionCause::ALL.len() + 1
        );
        let last = events.last().expect("final pilot_card event");
        assert!(last.contains("\"event\":\"pilot_card\""));
        assert!(last.contains("\"reserve_disclosure_claim\":false"));
        assert!(last.contains("\"execution_authorized\":false"));
        assert!(last.contains("\"geotechnical_signoff\":false"));
        assert!(last.contains("\"regulatory_compliance_claim\":false"));
        assert!(last.contains(SYNTHETIC_PUBLIC_LABEL));
        assert!(last.contains(&card.card_digest.to_hex()));

        let md = card.render_markdown().expect("render md");
        assert!(md.contains(SYNTHETIC_PUBLIC_LABEL));
        assert!(md.contains(NO_CLAIM_BANNER));
        assert!(md.contains("`reserve_disclosure_claim` | `false`"));
        assert!(md.contains("`execution_authorized` | `false`"));
        assert!(md.contains("`geotechnical_signoff` | `false`"));
        assert!(md.contains("`regulatory_compliance_claim` | `false`"));
        assert!(md.contains(&card.analysis_plan_id.to_string()));
        assert!(md.contains(&card.card_digest.to_hex()));

        // Regression: failed build (e.g. undeclared seed_root, site mismatch, or replicates < 200)
        // must refuse BEFORE mutating ArtifactLedger, and NaN power_bar fails closed.
        let mut atomic_ledger = ArtifactLedger::new();
        let bad_seed_err = build_pilot_protocol_card(
            &family,
            &spec,
            "Cu flotation recovery",
            &plan,
            &mut atomic_ledger,
            100,
            500,
            &[sha256(b"lumwana")],
            200,
            999_999, // not in plan.seeds
            3,
        )
        .expect_err("undeclared seed_root must refuse");
        assert_eq!(bad_seed_err.code, "FMINE-REF-107");
        assert!(!atomic_ledger.contains_slot("lumwana/analysis_plan"));

        let mut wrong_site_plan = plan.clone();
        wrong_site_plan.site = "kibali".to_string();
        assert_eq!(
            build_pilot_protocol_card(
                &family,
                &spec,
                "Cu flotation recovery",
                &wrong_site_plan,
                &mut atomic_ledger,
                100,
                500,
                &[sha256(b"lumwana")],
                200,
                2_026_356,
                3,
            )
            .unwrap_err()
            .code,
            "FMINE-REF-107"
        );
        assert!(!atomic_ledger.contains_slot("kibali/analysis_plan"));

        let nan_ek = evaluate_early_kill(&[], f64::NAN);
        assert!(nan_ek.fires);
    }
}
