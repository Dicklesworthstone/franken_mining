//! DESIGN: a bounded manifest declares common counterfactual support and names
//! complete alternative source bundles. This parser performs no filesystem IO.
//! Determinism: Exact

use super::*;
use crate::valuation::worksheet::{fields, get, invalid};
use fmine_registry::toml_subset::parse_subset;

pub const SCHEMA: &str = "fmine.production-comparison.v1";
pub const MAX_BYTES: u64 = 64 * 1024;

/// Paths are resolved by the caller against its explicit workspace root, as in
/// the existing one-alternative command. They never become economic inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternativeSources {
    pub id: String,
    pub input: String,
    pub grinding: String,
    pub recovery: String,
    pub terms: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    declaration: Claim<ComparisonSpec>,
    sources: Vec<AlternativeSources>,
    text: String,
}

impl Manifest {
    pub fn declaration(&self) -> &Claim<ComparisonSpec> {
        &self.declaration
    }
    pub fn sources(&self) -> &[AlternativeSources] {
        &self.sources
    }
    pub fn evaluate(&self, alternatives: &[ValuationReceipt]) -> Result<ComparisonReceipt> {
        let mut receipt = super::evaluate(&self.declaration, alternatives)?;
        receipt.submitted_manifest = Some(self.text.clone());
        Ok(receipt)
    }
}

/// Bind the complete original manifest bytes and field locators. Declaring
/// common states is an assumption, not evidence of an identified causal model.
pub fn parse(text: &str, source: &SourceAnchor) -> Result<Manifest> {
    if text.len() as u64 > MAX_BYTES {
        return Err(invalid(1, "comparison manifest exceeds 64 KiB"));
    }
    if !named(&source.artifact_path) || !named(&source.locator) {
        return Err(invalid(1, "manifest source path and locator required"));
    }
    if sha256(text.as_bytes()).0 != source.artifact_sha256 {
        return Err(bad(
            RefusalCode::DigestMismatch505,
            "comparison source SHA-256 mismatch",
        ));
    }
    let doc = parse_subset(text).map_err(|e| invalid(e.line, e.reason))?;
    if !(2..=MAX_ALTERNATIVES).contains(&doc.rows.len()) {
        return Err(invalid(
            1,
            "two to sixteen alternative source bundles required",
        ));
    }
    let top = fields(
        &doc.top,
        &[
            "schema",
            "model_key",
            "question",
            "shared_scenario_basis",
            "boundary",
            "circuit",
            "material_domain",
            "dry_basis",
            "element",
            "start_ns",
            "end_ns",
            "commercial_support",
        ],
        1,
    )?;
    if get(&top, "schema")?.0 != SCHEMA {
        return Err(invalid(1, format!("schema must be {SCHEMA}")));
    }
    let mut anchors = vec![source.clone()];
    let mut sources = BTreeMap::new();
    for row in &doc.rows {
        if row.table != "alternative" {
            return Err(invalid(
                row.line,
                "only [[alternative]] tables are supported",
            ));
        }
        let f = fields(
            &row.fields,
            &["id", "input", "grinding", "recovery", "terms"],
            row.line,
        )?;
        let alternative = AlternativeSources {
            id: get(&f, "id")?.0.into(),
            input: get(&f, "input")?.0.into(),
            grinding: get(&f, "grinding")?.0.into(),
            recovery: get(&f, "recovery")?.0.into(),
            terms: get(&f, "terms")?.0.into(),
        };
        if alternative.id == DO_NOTHING
            || sources.insert(alternative.id.clone(), alternative).is_some()
        {
            return Err(invalid(
                row.line,
                "unique action ids separate from do-nothing required",
            ));
        }
        for field in &row.fields {
            anchors.push(SourceAnchor::new(
                &source.artifact_path,
                source.artifact_sha256,
                format!("line {}: alternative.{}", field.line, field.key),
            ));
        }
    }
    for field in &doc.top {
        anchors.push(SourceAnchor::new(
            &source.artifact_path,
            source.artifact_sha256,
            format!("line {}: {}", field.line, field.key),
        ));
    }
    let time = |key| {
        let (value, line) = get(&top, key)?;
        value
            .parse::<i64>()
            .map_err(|_| invalid(line, "time must be an i64 nanosecond integer"))
    };
    let spec = ComparisonSpec {
        model_key: get(&top, "model_key")?.0.into(),
        question: get(&top, "question")?.0.into(),
        shared_scenario_basis: get(&top, "shared_scenario_basis")?.0.into(),
        boundary: get(&top, "boundary")?.0.into(),
        circuit: get(&top, "circuit")?.0.into(),
        material_domain: get(&top, "material_domain")?.0.into(),
        dry_basis: get(&top, "dry_basis")?.0.into(),
        element: get(&top, "element")?.0.into(),
        start_ns: time("start_ns")?,
        end_ns: time("end_ns")?,
        commercial_support: get(&top, "commercial_support")?.0.into(),
        action_ids: sources.keys().cloned().collect(),
    };
    if spec.start_ns >= spec.end_ns {
        return Err(invalid(1, "comparison interval must have positive duration"));
    }
    let tag = spec.canonical_tag();
    let declaration = Claim::new(
        INPUT_METRIC,
        spec,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Known,
        Confidentiality::internal(),
        Vec::new(),
        Vec::new(),
        anchors,
    );
    Ok(Manifest {
        declaration,
        sources: sources.into_values().collect(),
        text: text.into(),
    })
}
