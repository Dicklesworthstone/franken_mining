//! Canonical issuer-source and derived arithmetic Claims for published cut-offs.
//! Determinism: DeterministicPerIsa (exact canonical encoding; the inherited solver cross-check uses floats).

use fmine_claim::{
    AdditivityClass, Claim, ClaimId, Confidentiality, Representation, SpatialSupport,
};
use fmine_core::anchor::SourceAnchor;
use fmine_core::{Encoder, blake3_hash};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};

use super::*;

/// An agreed printed input, retaining the precision used in the arithmetic enclosure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputValue {
    pub role: InputRole,
    pub quantity: String,
    pub printed: Printed,
}

/// Claims obtained by rechecking the row's sources and exact result. These do not authenticate PDF bytes.
/// Use `SourceRow::check_claims` before accepting externally supplied or mutated Claims.
#[derive(Debug, Clone, PartialEq)]
pub struct CutoffClaims {
    pub inputs: Vec<Claim<InputValue>>,
    pub derived: Claim<CutoffReport>,
}

fn claim<T>(
    metric: &str,
    value: T,
    tag: &str,
    mut parents: Vec<ClaimId>,
    mut anchors: Vec<SourceAnchor>,
) -> Claim<T> {
    parents.sort();
    anchors.sort();
    anchors.dedup();
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::ISSUER,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        parents,
        anchors,
    )
}

fn tag(kind: [u8; 4], parts: &[&str]) -> String {
    let mut enc = Encoder::new_frame(kind, 1);
    // Claim v2 does not itself hash representation, support or additivity; this value tag binds them.
    enc.put_str("deterministic;period_aggregate;non_additive;issuer;known_known;known;public");
    enc.put_str(ROUNDING_RULE);
    enc.put_str(NO_CLAIM);
    for part in parts {
        enc.put_str(part);
    }
    blake3_hash(&enc.finish()).to_hex()
}

fn anchor(source: &SourceCell) -> Result<SourceAnchor, String> {
    let sha = &source.document.sha256;
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&sha[index * 2..index * 2 + 2], 16)
            .map_err(|_| "invalid source SHA-256")?;
    }
    Ok(SourceAnchor::new(
        format!("{}/{}", source.document.dir, source.document.file),
        bytes,
        source.source.locator.to_string(),
    ))
}

impl SourceRow {
    /// Reconstruct the complete Claim ancestry from the retained source readings. A caller cannot edit
    /// the public result field into a different number and obtain a valid derived Claim for it.
    pub fn claims(&self) -> Result<CutoffClaims, String> {
        let report = self.result.as_ref().map_err(Clone::clone)?;
        let (expected, mut sources) = bind(&self.declaration, &self.evidence, &self.documents)?;
        if &expected != report {
            return Err(
                "stored cutoff result differs from its exact source-bound recomputation".into(),
            );
        }
        sources.sort_by(|a, b| (a.0, &a.1.source.quantity).cmp(&(b.0, &b.1.source.quantity)));
        let declaration = declaration_json(&self.declaration);
        let mut inputs = Vec::new();
        for (role, source) in sources {
            let value = InputValue {
                role,
                quantity: source.source.quantity.clone(),
                printed: printed(&source)?,
            };
            let value_tag = tag(
                *b"CUTI",
                &[
                    &declaration,
                    role.as_str(),
                    &accepted_json(&source.source),
                    &document_json(&source.document),
                    &ratio(value.printed.value),
                    &ratio(value.printed.half_step),
                ],
            );
            inputs.push(claim(
                "cutoff.source_input",
                value,
                &value_tag,
                vec![],
                vec![anchor(&source)?],
            ));
        }
        let parents = inputs.iter().map(|c| c.claim_id).collect();
        let anchors = inputs
            .iter()
            .flat_map(|c| c.anchors.iter().cloned())
            .collect();
        let value_tag = tag(
            *b"CUTO",
            &[
                &declaration,
                &result_json(report)?,
                &ratio(report.published.half_step),
                &report.evidence,
            ],
        );
        Ok(CutoffClaims {
            inputs,
            derived: claim(
                "cutoff.reproduction",
                report.clone(),
                &value_tag,
                parents,
                anchors,
            ),
        })
    }

    /// Checks the complete typed values and envelope, beyond `Claim::check_id` (which hashes value tags).
    /// This also refuses changed ancestry, source semantics, precision and unhashed envelope fields.
    pub fn check_claims(&self, claims: &CutoffClaims) -> Result<(), String> {
        if &self.claims()? == claims {
            Ok(())
        } else {
            Err("cutoff Claims differ from their source-bound reconstruction".into())
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn envelope<T>(claim: &Claim<T>) -> String {
    let parents = claim
        .parents
        .iter()
        .map(|p| js(&p.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    let anchors = claim
        .anchors
        .iter()
        .map(|a| {
            format!(
                "{{\"artifact_path\":{},\"artifact_sha256\":{},\"locator\":{}}}",
                js(&a.artifact_path),
                js(&hex(&a.artifact_sha256)),
                js(&a.locator),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "\"claim_id\":{},\"metric_key\":{},\"value_tag\":{},\"canonical_body\":{},\"evidence\":\"issuer_assertion\",\"epistemic\":\"known_known\",\"state\":\"known\",\"representation\":\"deterministic\",\"support\":\"period_aggregate\",\"additivity\":\"non_additive\",\"confidentiality\":\"public\",\"parents\":[{parents}],\"anchors\":[{anchors}]",
        js(&claim.claim_id.to_string()),
        js(&claim.metric_key),
        js(claim.value_tag()),
        js(&hex(&claim.canonical_body())),
    )
}

impl CutoffClaims {
    pub(super) fn to_json(&self) -> Result<String, String> {
        let inputs = self
            .inputs
            .iter()
            .map(|c| {
                format!(
                    "{{{},\"role\":{},\"quantity\":{},\"printed\":{},\"half_step\":{}}}",
                    envelope(c),
                    js(c.value.role.as_str()),
                    js(&c.value.quantity),
                    ratio(c.value.printed.value),
                    ratio(c.value.printed.half_step),
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        Ok(format!(
            "{{\"inputs\":[{inputs}],\"derived\":{{{},\"value\":{}}}}}",
            envelope(&self.derived),
            result_json(&self.derived.value)?
        ))
    }
}
