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
        if env.identifiability_refusal.is_some() || env.min_power < power_bar_target {
            failing.push(format!(
                "{} ([{:.3}, {:.3}] < bar {:.3})",
                env.cause.as_str(),
                env.min_power,
                env.max_power,
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
        let _ = writeln!(md, "# Pilot Protocol Card — {}", self.site);
        let _ = writeln!(md);
        let _ = writeln!(md, "- **Disclaimer:** {}", self.label);
        let _ = writeln!(md, "- **Mechanism:** {}", self.mechanism);
        let _ = writeln!(
            md,
            "- **Pre-registered attribution question:** {}",
            self.attribution_question
        );
        let _ = writeln!(md, "- **Analysis plan ID:** `{}`", self.analysis_plan_id);
        let _ = writeln!(md, "- **Card digest (BLAKE3):** `{}`", self.card_digest);
        let _ = writeln!(md, "- **Inputs digest (BLAKE3):** `{}`", self.inputs_digest);
        let _ = writeln!(md, "- **Code revision:** `{}`", self.code_rev);
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
                row.name,
                label_text.replace('|', "\\|"),
                row.reason.replace('|', "\\|"),
                env_parts.join("; ").replace('|', "\\|"),
            );
        }

        let _ = writeln!(md, "\n## Early-Kill Rule\n");
        let _ = writeln!(md, "- **Early kill fires:** `{}`", self.early_kill.fires);
        let _ = writeln!(md, "- **Outcome:** {}", self.early_kill.reason);
        if let Some(alt) = &self.early_kill.alternate_recommendation {
            let _ = writeln!(md, "- **Alternate site recommendation (§37.3):** {alt}");
        }
        Ok(md)
    }
}

/// Build and pre-register a complete [`PilotProtocolCard`] from a [`TwinFamily`] (`S1`),
/// [`PilotTwinRegistrySpec`], and [`AnalysisPlan`], enforcing `INV-008` freeze order,
/// `S2` multi-twin power envelopes + robustness labels, and §37.3 early-kill evaluation.
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
    let plan_bytes = plan.canonical_bytes()?;
    let slot_key = format!("{}/analysis_plan", plan.site.trim());
    let analysis_plan_id = ledger.register_slot(
        &slot_key,
        FrozenArtifactKind::AnalysisPlan,
        &plan_bytes,
        registered_ns,
        first_request_ns,
    )?;

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

    let card = PilotProtocolCard {
        site: plan.site.trim().to_string(),
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
    }
}
