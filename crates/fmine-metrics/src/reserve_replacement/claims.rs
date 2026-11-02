//! Source-linked reserve replacement Claims, rechecked against the retained annual readings.
//! Determinism: Exact
//! Canonical identities bind exact values, source precision, definitions and decks.

use fmine_claim::{
    AdditivityClass, Claim, ClaimId, Confidentiality, Representation, SpatialSupport,
};
use fmine_core::anchor::SourceAnchor;
use fmine_core::{Encoder, blake3_hash};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, KuKind};

use super::*;

/// The caller declares whether these are registered issuer assertions or invented test evidence.
/// Neither path can claim measured reserves, calibration, or disclosure authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimBasis {
    IssuerAssertions,
    Synthetic,
}

impl ClaimBasis {
    const fn label(self) -> &'static str {
        match self {
            Self::IssuerAssertions => "issuer_assertion",
            Self::Synthetic => "synthetic_assumption",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputRole {
    AnnualNetChange,
    AnnualDepletion,
    AggregateNetChange,
    AggregateDepletion,
    Headline,
}

impl InputRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AnnualNetChange => "annual_net_change",
            Self::AnnualDepletion => "annual_depletion",
            Self::AggregateNetChange => "aggregate_net_change",
            Self::AggregateDepletion => "aggregate_depletion",
            Self::Headline => "headline",
        }
    }
}

/// The original signed amount and its complete source context. Normalization never rewrites this value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputClaim {
    pub role: InputRole,
    pub reading: Reading,
}

/// The exact calculation, keeping independently printed aggregate totals separate from annual sums.
/// `joint` contains arithmetic rounding bounds, not a probability or a confidence interval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementValue {
    pub issuer: String,
    pub series_id: String,
    pub definition: String,
    pub start_year: u16,
    pub end_year: u16,
    pub amount_unit: String,
    pub basis: String,
    pub annual: Totals,
    pub aggregate: Option<Totals>,
    pub joint: JointBounds,
    pub headline: Option<HeadlineResult>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplacementClaims {
    pub(super) inputs: Vec<Claim<InputClaim>>,
    pub(super) derived: Claim<ReplacementValue>,
}

fn tag(kind: [u8; 4], basis: ClaimBasis, issuer: &str, parts: &[&str]) -> String {
    let mut enc = Encoder::new_frame(kind, 1);
    // Claim v2 leaves some envelope fields outside its body; this tag commits to their fixed policy.
    enc.put_str("deterministic;period_aggregate;non_additive;public;no_uu;no_calibration;not_stale;no_supersedes");
    enc.put_str(basis.label());
    enc.put_str(issuer);
    enc.put_str(NO_CLAIM);
    for part in parts {
        enc.put_str(part);
    }
    blake3_hash(&enc.finish()).to_hex()
}

fn claim<T>(
    metric: &str,
    value: T,
    value_tag: &str,
    basis: ClaimBasis,
    mut parents: Vec<ClaimId>,
    mut anchors: Vec<SourceAnchor>,
) -> Claim<T> {
    parents.sort();
    anchors.sort();
    anchors.dedup();
    let (evidence, epistemic, state) = match basis {
        ClaimBasis::IssuerAssertions => (
            EvidenceClass::ISSUER,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
        ),
        ClaimBasis::Synthetic => (
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Unknown,
        ),
    };
    Claim::new(
        metric,
        value,
        value_tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        evidence,
        epistemic,
        state,
        Confidentiality::public(),
        vec![],
        parents,
        anchors,
    )
}

fn anchor(reading: &Reading) -> Result<SourceAnchor, String> {
    let mut sha = [0; 32];
    let text = &reading.document.sha256;
    if text.len() != 64 || !text.is_ascii() {
        return Err("invalid reserve source digest".into());
    }
    for (i, byte) in sha.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16)
            .map_err(|_| "invalid reserve source digest")?;
    }
    Ok(SourceAnchor::new(
        format!("{}/{}", reading.document.dir, reading.document.file),
        sha,
        reading.source.locator.to_string(),
    ))
}

impl SeriesResult {
    fn readings(&self) -> Vec<(InputRole, &Reading)> {
        let mut readings = Vec::new();
        for year in &self.years {
            readings.push((InputRole::AnnualNetChange, &year.net_change));
            readings.push((InputRole::AnnualDepletion, &year.depletion));
        }
        if let Some(aggregate) = &self.aggregate {
            readings.push((InputRole::AggregateNetChange, &aggregate.net_change));
            readings.push((InputRole::AggregateDepletion, &aggregate.depletion));
        }
        if let Some(headline) = &self.headline {
            readings.push((InputRole::Headline, &headline.reading));
        }
        readings
    }

    /// Recompute before issuing identities. Mutating a cached point, bound, basis or headline verdict
    /// cannot manufacture a valid Claim. Document-byte authentication remains the caller's obligation.
    pub fn claims(&self, issuer: &str, basis: ClaimBasis) -> Result<ReplacementClaims, String> {
        let binding = ReplacementRegistry {
            issuer: issuer.into(),
            series: vec![self.selection.clone()],
        };
        binding.validate().map_err(|e| e.to_string())?;
        let mut metrics = MetricRegistry::default();
        metrics.definitions.push(self.definition.clone());
        metrics.reserve_replacements.push(self.rule.clone());
        if let Some(definition) = &self.geo_definition {
            metrics.definitions.push(definition.clone());
        }
        metrics.geo_decks = self
            .years
            .iter()
            .filter_map(|y| y.geo_deck.clone())
            .collect();
        let readings = self.readings();
        let sources: Vec<_> = readings.iter().map(|(_, r)| r.source.clone()).collect();
        let mut documents: Vec<_> = readings.iter().map(|(_, r)| r.document.clone()).collect();
        documents.sort_by_cached_key(document_json);
        documents.dedup();
        let expected = compute(&self.selection, &metrics, &sources, &documents)?;
        if &expected != self {
            return Err(
                "reserve replacement result differs from its source-bound recomputation".into(),
            );
        }
        let selection = selection_json(&self.selection);
        let mut inputs = Vec::new();
        for (role, reading) in readings {
            let value_tag = tag(
                *b"RRPI",
                basis,
                issuer,
                &[&selection, role.as_str(), &reading_json(reading)],
            );
            inputs.push(claim(
                "reserve_replacement.source_input",
                InputClaim {
                    role,
                    reading: reading.clone(),
                },
                &value_tag,
                basis,
                vec![],
                vec![anchor(reading)?],
            ));
        }
        let value = ReplacementValue {
            issuer: issuer.into(),
            series_id: self.selection.id.clone(),
            definition: self.definition.id.clone(),
            start_year: self.selection.start_year,
            end_year: self.selection.end_year,
            amount_unit: self.selection.unit.clone(),
            basis: self.selection.basis.clone(),
            annual: self.annual,
            aggregate: self.aggregate.as_ref().map(|a| a.totals),
            joint: self.joint,
            headline: self.headline.clone(),
        };
        let result = result_json(&SeriesAssessment::Computed(Box::new(self.clone())));
        let value_tag = tag(*b"RRPO", basis, issuer, &[&result, &value_json(&value)]);
        let parents = inputs.iter().map(|c| c.claim_id).collect();
        let anchors = inputs
            .iter()
            .flat_map(|c| c.anchors.iter().cloned())
            .collect();
        Ok(ReplacementClaims {
            inputs,
            derived: claim(
                "reserve_replacement.cumulative",
                value,
                &value_tag,
                basis,
                parents,
                anchors,
            ),
        })
    }

    /// Checks typed values and the full envelope, including fields not covered by Claim v2's body.
    pub fn check_claims(
        &self,
        issuer: &str,
        basis: ClaimBasis,
        claims: &ReplacementClaims,
    ) -> Result<(), String> {
        if &self.claims(issuer, basis)? == claims {
            Ok(())
        } else {
            Err("reserve replacement Claims differ from their source-bound reconstruction".into())
        }
    }
}

impl ReplacementReport {
    /// Emit Claims for computed series while refused series remain in the original report denominator.
    pub fn claims(&self, basis: ClaimBasis) -> Result<Vec<ReplacementClaims>, String> {
        let mut claims = Vec::new();
        for series in &self.series {
            let SeriesAssessment::Computed(result) = series else {
                continue;
            };
            if self.registry_refusal.is_some()
                || self
                    .definitions
                    .iter()
                    .filter(|d| d.id == result.definition.id)
                    .collect::<Vec<_>>()
                    != [&result.definition]
                || self
                    .rules
                    .iter()
                    .filter(|r| r.definition == result.rule.definition)
                    .collect::<Vec<_>>()
                    != [&result.rule]
                || result.geo_definition.as_ref().is_some_and(|d| {
                    self.definitions
                        .iter()
                        .filter(|other| other.id == d.id)
                        .collect::<Vec<_>>()
                        != [d]
                })
                || result
                    .years
                    .iter()
                    .filter_map(|y| y.geo_deck.as_ref())
                    .any(|d| {
                        self.geo_decks
                            .iter()
                            .filter(|other| {
                                other.definition == d.definition && other.year == d.year
                            })
                            .collect::<Vec<_>>()
                            != [d]
                    })
            {
                return Err(
                    "reserve replacement Claim dependencies differ from the report registry".into(),
                );
            }
            claims.push(result.claims(&self.issuer, basis)?);
        }
        Ok(claims)
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
    let synthetic = claim.evidence == EvidenceClass::Assumed;
    format!(
        "\"claim_id\":{},\"metric_key\":{},\"value_tag\":{},\"canonical_body\":{},\"evidence\":{},\"synthetic\":{synthetic},\"state\":{},\"representation\":\"deterministic\",\"support\":\"period_aggregate\",\"additivity\":\"non_additive\",\"confidentiality\":\"public\",\"parents\":{},\"anchors\":{},\"calibration_claim\":false",
        js(&claim.claim_id.to_string()),
        js(&claim.metric_key),
        js(claim.value_tag()),
        js(&hex(&claim.canonical_body())),
        js(if synthetic {
            "assumed"
        } else {
            "issuer_assertion"
        }),
        js(if synthetic { "unknown" } else { "known" }),
        array(&claim.parents, |id| js(&id.to_string())),
        array(&claim.anchors, |a| format!(
            "{{\"artifact_path\":{},\"artifact_sha256\":{},\"locator\":{}}}",
            js(&a.artifact_path),
            js(&hex(&a.artifact_sha256)),
            js(&a.locator)
        )),
    )
}

fn value_json(value: &ReplacementValue) -> String {
    format!(
        "{{\"issuer\":{},\"series_id\":{},\"definition\":{},\"start_year\":{},\"end_year\":{},\"amount_unit\":{},\"basis\":{},\"unit\":\"percent\",\"annual\":{},\"aggregate\":{},\"joint\":{{\"net_change\":{},\"depletion\":{},\"percent\":{}}},\"headline\":{}}}",
        js(&value.issuer),
        js(&value.series_id),
        js(&value.definition),
        value.start_year,
        value.end_year,
        js(&value.amount_unit),
        js(&value.basis),
        totals_json(&value.annual),
        optional(value.aggregate.as_ref(), totals_json),
        bounds(value.joint.net_change),
        bounds(value.joint.depletion),
        bounds(value.joint.percent),
        optional(value.headline.as_ref(), |h| format!(
            "{{\"reading\":{},\"kind\":{},\"state\":{}}}",
            reading_json(&h.reading),
            js(h.kind.as_str()),
            js(h.state.as_str())
        )),
    )
}

impl ReplacementClaims {
    /// The checked source Claims. Immutable access prevents relabelling before rendering.
    #[must_use]
    pub fn inputs(&self) -> &[Claim<InputClaim>] {
        &self.inputs
    }

    /// The checked cumulative result. Its exact value retains all declared rounding enclosures.
    #[must_use]
    pub fn derived(&self) -> &Claim<ReplacementValue> {
        &self.derived
    }

    /// Render the same canonical values and ancestry used by callers of the typed API.
    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"series_id\":{},\"inputs\":{},\"derived\":{{{},\"value\":{}}}}}",
            js(&self.derived.value.series_id),
            array(&self.inputs, |c| format!(
                "{{{},\"role\":{},\"reading\":{}}}",
                envelope(c),
                js(c.value.role.as_str()),
                reading_json(&c.value.reading)
            )),
            envelope(&self.derived),
            value_json(&self.derived.value),
        )
    }
}
